mod aerodynamic_model;
mod genetic_optimizer;
mod handlers;
mod influxdb_storage;
mod models;
mod mqtt_alerts;

use crate::handlers::{configure_routes, AppState};
use crate::influxdb_storage::InfluxDBStorage;
use crate::mqtt_alerts::{AlertManager, MQTTAlertService};
use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let influxdb_url = std::env::var("INFLUXDB_URL").unwrap_or_else(|_| "http://localhost:8086".to_string());
    let influxdb_db = std::env::var("INFLUXDB_DB").unwrap_or_else(|_| "bridge_monitoring".to_string());
    let influxdb_user = std::env::var("INFLUXDB_USER").unwrap_or_else(|_| "bridge_writer".to_string());
    let influxdb_pass = std::env::var("INFLUXDB_PASS").unwrap_or_else(|_| "bridge_write_2024".to_string());

    let mqtt_host = std::env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
    let mqtt_port: u16 = std::env::var("MQTT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(1883);
    let mqtt_topic = std::env::var("MQTT_ALERT_TOPIC")
        .unwrap_or_else(|_| "heritage_center/bridge_alerts".to_string());
    let mqtt_enabled: bool = std::env::var("MQTT_ENABLED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(false);

    let server_host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let server_port: u16 = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    println!("================================================");
    println!("  古代悬索桥风致振动监测与气动优化系统");
    println!("  Ancient Suspension Bridge Wind-Induced");
    println!("  Vibration Monitoring & Aerodynamic Optimization");
    println!("================================================");
    println!("");
    println!("InfluxDB: {} / {}", influxdb_url, influxdb_db);
    println!("MQTT: {}:{} (enabled: {})", mqtt_host, mqtt_port, mqtt_enabled);
    println!("Server: {}:{}", server_host, server_port);
    println!("");

    let storage = Arc::new(InfluxDBStorage::new(
        &influxdb_url,
        &influxdb_db,
        &influxdb_user,
        &influxdb_pass,
    ));

    let mqtt_service = if mqtt_enabled {
        match MQTTAlertService::new(
            &mqtt_host,
            mqtt_port,
            "bridge_monitoring_backend",
            &mqtt_topic,
        ) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to initialize MQTT: {}, using disabled mode", e);
                MQTTAlertService::disabled()
            }
        }
    } else {
        MQTTAlertService::disabled()
    };

    let alert_manager = Arc::new(AlertManager::new(mqtt_service));

    let app_state = Arc::new(AppState {
        storage,
        alert_manager,
        recent_results: RwLock::new(HashMap::new()),
    });

    println!("Starting HTTP server on http://{}:{}", server_host, server_port);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(middleware::Logger::default())
            .wrap(cors)
            .configure(configure_routes)
            .route("/", web::get().to(|| async {
                "古代悬索桥风致振动监测与气动优化系统 API v1.0"
            }))
    })
    .bind((server_host.as_str(), server_port))?
    .run()
    .await
}
