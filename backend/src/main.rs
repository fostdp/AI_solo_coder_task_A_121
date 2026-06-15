pub mod models;
pub mod aerodynamic_model;
pub mod genetic_optimizer;
pub mod influxdb_storage;
pub mod mqtt_alerts;
pub mod handlers;
pub mod dtu_receiver;
pub mod flutter_analyzer;
pub mod shape_optimizer;
pub mod alarm_mqtt;

use crate::dtu_receiver::DTUReceiver;
use crate::flutter_analyzer::FlutterAnalyzer;
use crate::shape_optimizer::{ShapeOptimizer, PendingOptimizations};
use crate::alarm_mqtt::{AlarmService, mqtt_publisher_worker};
use crate::models::SystemMessage;
use crate::mqtt_alerts::{AlertManager, MQTTAlertService};
use crate::influxdb_storage::InfluxDBStorage;

use crate::models::AppState;
use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use dotenv::dotenv;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::info;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let config_path = std::env::var("CONFIG_DIR").unwrap_or_else(|_| "../config".to_string());
    info!("加载配置目录: {}", config_path);

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    let buffer_size: usize = std::env::var("CHANNEL_BUFFER")
        .unwrap_or_else(|_| "200".to_string())
        .parse()
        .unwrap_or(200);

    info!("启动服务于 {}:{}", host, port);

    let influx_url = std::env::var("INFLUXDB_URL").unwrap_or_else(|_| "http://localhost:8086".to_string());
    let influx_db = std::env::var("INFLUXDB_DB").unwrap_or_else(|_| "bridge_monitoring".to_string());
    let influx_user = std::env::var("INFLUXDB_USER").ok();
    let influx_pass = std::env::var("INFLUXDB_PASS").ok();

    let mqtt_enabled = std::env::var("MQTT_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);
    let mqtt_host = std::env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
    let mqtt_port: u16 = std::env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".to_string())
        .parse()
        .unwrap_or(1883);

    let (_dtu_tx, _dtu_rx) = mpsc::channel::<SystemMessage>(buffer_size);
    let (analyzer_tx, analyzer_rx) = mpsc::channel::<SystemMessage>(buffer_size);
    let (alarm_tx, alarm_rx) = mpsc::channel::<SystemMessage>(buffer_size);
    let (optimizer_tx, optimizer_rx) = mpsc::channel::<SystemMessage>(50);
    let (storage_tx, storage_rx) = mpsc::channel::<SystemMessage>(buffer_size * 2);
    let (mqtt_pub_tx, mqtt_pub_rx) = mpsc::channel::<crate::models::AlertMessage>(buffer_size);

    let storage = Arc::new(InfluxDBStorage::new(
        &influx_url,
        &influx_db,
        influx_user.as_deref().unwrap_or(""),
        influx_pass.as_deref().unwrap_or(""),
    ));

    let mqtt_service = Arc::new(if mqtt_enabled {
        MQTTAlertService::new(&mqtt_host, mqtt_port, "bridge_monitor").await
    } else {
        info!("MQTT已禁用，使用空服务");
        MQTTAlertService::disabled()
    });

    let alert_manager = Arc::new(AlertManager::new());

    let dtu_receiver = Arc::new(DTUReceiver::new(
        storage.clone(),
        analyzer_tx.clone(),
        storage_tx.clone(),
    ));

    let recent_results: Arc<RwLock<HashMap<String, crate::models::AerodynamicResult>>> =
        Arc::new(RwLock::new(HashMap::new()));

    let flutter_analyzer = Arc::new(FlutterAnalyzer::new(
        alarm_tx.clone(),
        storage_tx.clone(),
        recent_results.clone(),
    ));

    let pending_optimizations: PendingOptimizations =
        Arc::new(Mutex::new(HashMap::new()));

    let shape_optimizer = ShapeOptimizer::new(pending_optimizations.clone());

    let alarm_service = AlarmService::new(
        alert_manager.as_ref().clone(),
        mqtt_service.clone(),
        mqtt_pub_tx.clone(),
    );

    let storage_clone = storage.clone();
    tokio::spawn(async move {
        info!("[Storage] 存储写入Worker启动");
        let mut rx = storage_rx;
        while let Some(msg) = rx.recv().await {
            if let SystemMessage::StorageWriteRequest { measurement } = msg {
                use crate::models::StorageMeasurement::*;
                let _ = match measurement {
                    CableForce { bridge_id, cable_id, force, temp, time } => {
                        let data = crate::models::CableForceData {
                            bridge_id,
                            cable_id,
                            cable_force: force,
                            temperature: temp,
                            timestamp: time,
                        };
                        storage_clone.write_cable_force(&data).await
                    }
                    Acceleration { bridge_id, sensor_id, ax, ay, az, time } => {
                        let data = crate::models::DeckAccelerationData {
                            bridge_id,
                            sensor_id,
                            position_x: 0.0,
                            acceleration_x: ax,
                            acceleration_y: ay,
                            acceleration_z: az,
                            timestamp: time,
                        };
                        storage_clone.write_acceleration(&data).await
                    }
                    WindData { bridge_id, sensor_id, speed, dir, attack, time } => {
                        let data = crate::models::WindData {
                            bridge_id,
                            sensor_id,
                            wind_speed: speed,
                            wind_direction: dir,
                            attack_angle: attack,
                            temperature: 0.0,
                            humidity: 0.0,
                            timestamp: time,
                        };
                        storage_clone.write_wind_data(&data).await
                    }
                    AeroResult(r) => {
                        storage_clone.write_aerodynamic_result(&r).await
                    }
                };
            }
        }
        info!("[Storage] 存储写入Worker已停止");
    });

    let analyzer_clone = (*flutter_analyzer).clone();
    tokio::spawn(async move {
        analyzer_clone.run(analyzer_rx).await;
    });

    tokio::spawn(async move {
        shape_optimizer.run(optimizer_rx).await;
    });

    tokio::spawn(async move {
        alarm_service.run(alarm_rx).await;
    });

    let mqtt_clone = mqtt_service.clone();
    tokio::spawn(async move {
        mqtt_publisher_worker(mqtt_clone, mqtt_pub_rx).await;
    });

    let app_state = web::Data::new(AppState {
        storage: storage.clone(),
        dtu_receiver: dtu_receiver.clone(),
        flutter_analyzer: flutter_analyzer.clone(),
        optimizer_tx: optimizer_tx.clone(),
        pending_optimizations: pending_optimizations.clone(),
        alert_manager: alert_manager.clone(),
        mqtt_service: mqtt_service.clone(),
        recent_results: recent_results.clone(),
        storage_tx: storage_tx.clone(),
    });

    info!("模块装配完成，启动HTTP服务器...");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(Cors::permissive())
            .route("/api/v1/health", web::get().to(handlers::health_check))
            .route("/api/v1/bridges", web::get().to(handlers::get_bridges))
            .route("/api/v1/bridges/{id}", web::get().to(handlers::get_bridge_handler))
            .route("/api/analyze", web::get().to(handlers::evaluate_aerodynamics))
            .route("/api/optimize", web::post().to(handlers::run_optimization))
            .route("/api/v1/dtu/receive", web::post().to(handlers::receive_dtu_data))
            .route("/api/v1/aerodynamics/evaluate", web::get().to(handlers::evaluate_aerodynamics))
            .route("/api/v1/aerodynamics/evaluate-with-shape", web::post().to(handlers::evaluate_with_shape))
            .route("/api/v1/aerodynamics/vibration-response", web::get().to(handlers::get_vibration_response))
            .route("/api/v1/aerodynamics/deck-deformation", web::get().to(handlers::get_deck_deformation))
            .route("/api/v1/aerodynamics/recent/{id}", web::get().to(handlers::get_recent_aero_result))
            .route("/api/v1/aerodynamics/flutter-curve/{id}", web::get().to(handlers::get_flutter_curve))
            .route("/api/v1/optimization/run", web::post().to(handlers::run_optimization))
    })
    .bind((host.as_str(), port))?
    .run()
    .await?;

    info!("服务已停止");
    Ok(())
}
