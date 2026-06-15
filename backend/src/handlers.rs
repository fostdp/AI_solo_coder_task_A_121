use crate::aerodynamic_model::AerodynamicModel;
use crate::genetic_optimizer::GeneticOptimizer;
use crate::influxdb_storage::InfluxDBStorage;
use crate::mqtt_alerts::AlertManager;
use crate::models::*;
use actix_web::{web, HttpResponse, Responder};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub storage: Arc<InfluxDBStorage>,
    pub alert_manager: Arc<AlertManager>,
    pub recent_results: RwLock<std::collections::HashMap<String, AerodynamicResult>>,
}

#[derive(Debug, Deserialize)]
pub struct BridgeQuery {
    pub bridge_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AeroEvalQuery {
    pub bridge_id: String,
    pub wind_speed: f64,
    pub attack_angle: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct VibrationQuery {
    pub bridge_id: String,
    pub wind_speed: f64,
    pub attack_angle: Option<f64>,
    pub duration: Option<f64>,
    pub dt: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct DeformationQuery {
    pub bridge_id: String,
    pub wind_speed: f64,
    pub attack_angle: Option<f64>,
    pub segments: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<Utc>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn err(msg: &str) -> Self {
        ApiResponse {
            success: false,
            data: None,
            error: Some(msg.to_string()),
            timestamp: Utc::now(),
        }
    }
}

fn get_bridge(bridge_id: &str) -> Option<&BridgeInfo> {
    BRIDGES.iter().find(|b| b.bridge_id == bridge_id)
}

pub async fn get_all_bridges() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::ok(BRIDGES))
}

pub async fn get_bridge_info(path: web::Path<BridgeQuery>) -> impl Responder {
    match get_bridge(&path.bridge_id) {
        Some(bridge) => HttpResponse::Ok().json(ApiResponse::ok(bridge)),
        None => HttpResponse::NotFound().json(ApiResponse::<BridgeInfo>::err("Bridge not found")),
    }
}

pub async fn receive_dtu_data(
    payload: web::Json<DTUPayload>,
    data: web::Data<Arc<AppState>>,
) -> impl Responder {
    let timestamp = payload.timestamp;
    let bridge_id = payload.bridge_id.clone();

    match data.storage.handle_dtu_payload(&payload).await {
        Ok(count) => {
            if let Some(bridge) = get_bridge(&bridge_id) {
                let model = AerodynamicModel::new(bridge);
                let wind_speed = payload.wind.speed;
                let attack_angle = payload.wind.attack_angle;
                let result = model.evaluate_aerodynamic_performance(wind_speed, attack_angle, None);

                let _ = data
                    .alert_manager
                    .check_vibration_alert(&bridge_id, result.vibration_amplitude, timestamp)
                    .await;
                let _ = data
                    .alert_manager
                    .check_flutter_alert(
                        &bridge_id,
                        result.flutter_margin,
                        result.wind_speed,
                        result.flutter_critical_speed,
                        timestamp,
                    )
                    .await;

                let _ = data.storage.write_aerodynamic_result(&result).await;
                let mut recent = data.recent_results.write().await;
                recent.insert(bridge_id.clone(), result.clone());

                let max_az = payload
                    .accelerations
                    .iter()
                    .map(|a| a.az.abs())
                    .fold(0.0_f64, f64::max);
                if max_az > 0.5 {
                    let _ = data
                        .alert_manager
                        .check_vibration_alert(
                            &bridge_id,
                            max_az / 98.1,
                            timestamp,
                        )
                        .await;
                }

                return HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
                    "written_points": count,
                    "aerodynamic_result": result
                })));
            }
            HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
                "written_points": count
            })))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<serde_json::Value>::err(&e)),
    }
}

pub async fn evaluate_aerodynamics(
    query: web::Query<AeroEvalQuery>,
) -> impl Responder {
    let bridge = match get_bridge(&query.bridge_id) {
        Some(b) => b,
        None => return HttpResponse::NotFound().json(ApiResponse::<AerodynamicResult>::err("Bridge not found")),
    };

    let model = AerodynamicModel::new(bridge);
    let attack_angle = query.attack_angle.unwrap_or(0.0);
    let result = model.evaluate_aerodynamic_performance(query.wind_speed, attack_angle, None);

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

pub async fn evaluate_with_shape(
    query: web::Query<AeroEvalQuery>,
    shape: web::Json<DeckAerodynamicShape>,
) -> impl Responder {
    let bridge = match get_bridge(&query.bridge_id) {
        Some(b) => b,
        None => return HttpResponse::NotFound().json(ApiResponse::<AerodynamicResult>::err("Bridge not found")),
    };

    let model = AerodynamicModel::new(bridge);
    let attack_angle = query.attack_angle.unwrap_or(0.0);
    let result = model.evaluate_aerodynamic_performance(query.wind_speed, attack_angle, Some(&shape));

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

pub async fn get_vibration_response(
    query: web::Query<VibrationQuery>,
) -> impl Responder {
    let bridge = match get_bridge(&query.bridge_id) {
        Some(b) => b,
        None => return HttpResponse::NotFound().json(ApiResponse::<VibrationResponse>::err("Bridge not found")),
    };

    let model = AerodynamicModel::new(bridge);
    let attack_angle = query.attack_angle.unwrap_or(0.0);
    let duration = query.duration.unwrap_or(10.0);
    let dt = query.dt.unwrap_or(0.01);
    let response = model.compute_vibration_response(query.wind_speed, attack_angle, duration, dt);

    HttpResponse::Ok().json(ApiResponse::ok(response))
}

pub async fn get_deck_deformation(
    query: web::Query<DeformationQuery>,
) -> impl Responder {
    let bridge = match get_bridge(&query.bridge_id) {
        Some(b) => b,
        None => return HttpResponse::NotFound().json(ApiResponse::<Vec<(f64, f64, f64)>>::err("Bridge not found")),
    };

    let model = AerodynamicModel::new(bridge);
    let attack_angle = query.attack_angle.unwrap_or(0.0);
    let segments = query.segments.unwrap_or(50);
    let deformation = model.compute_deck_deformation(query.wind_speed, attack_angle, segments);

    HttpResponse::Ok().json(ApiResponse::ok(deformation))
}

pub async fn run_optimization(
    config: web::Json<OptimizationConfig>,
) -> impl Responder {
    let bridge = match get_bridge(&config.bridge_id) {
        Some(b) => b,
        None => return HttpResponse::NotFound().json(ApiResponse::<OptimizationResult>::err("Bridge not found")),
    };

    let model = AerodynamicModel::new(bridge);
    let optimizer = GeneticOptimizer::new(&model, config.into_inner());
    let result = optimizer.run();

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

pub async fn get_recent_aerodynamic_result(
    path: web::Path<BridgeQuery>,
    data: web::Data<Arc<AppState>>,
) -> impl Responder {
    let recent = data.recent_results.read().await;
    match recent.get(&path.bridge_id) {
        Some(result) => HttpResponse::Ok().json(ApiResponse::ok(result.clone())),
        None => {
            if let Some(bridge) = get_bridge(&path.bridge_id) {
                let model = AerodynamicModel::new(bridge);
                let result = model.evaluate_aerodynamic_performance(15.0, 0.0, None);
                HttpResponse::Ok().json(ApiResponse::ok(result))
            } else {
                HttpResponse::NotFound().json(ApiResponse::<AerodynamicResult>::err("Bridge not found"))
            }
        }
    }
}

pub async fn get_flutter_critical_speed_curve(
    path: web::Path<BridgeQuery>,
) -> impl Responder {
    let bridge = match get_bridge(&path.bridge_id) {
        Some(b) => b,
        None => return HttpResponse::NotFound().json(ApiResponse::<Vec<(f64, f64, f64)>>::err("Bridge not found")),
    };

    let model = AerodynamicModel::new(bridge);
    let mut curve = Vec::new();
    for angle_i in -10..=10 {
        let attack_angle = angle_i as f64;
        let result = model.evaluate_aerodynamic_performance(30.0, attack_angle, None);
        curve.push((attack_angle, result.flutter_critical_speed, result.aerodynamic_damping));
    }

    HttpResponse::Ok().json(ApiResponse::ok(curve))
}

pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "status": "healthy",
        "service": "bridge_monitoring_backend",
        "version": "1.0.0"
    })))
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .route("/health", web::get().to(health_check))
            .route("/bridges", web::get().to(get_all_bridges))
            .route("/bridges/{bridge_id}", web::get().to(get_bridge_info))
            .route("/dtu/receive", web::post().to(receive_dtu_data))
            .route("/aerodynamics/evaluate", web::get().to(evaluate_aerodynamics))
            .route("/aerodynamics/evaluate-with-shape", web::post().to(evaluate_with_shape))
            .route("/aerodynamics/vibration-response", web::get().to(get_vibration_response))
            .route("/aerodynamics/deck-deformation", web::get().to(get_deck_deformation))
            .route("/aerodynamics/recent/{bridge_id}", web::get().to(get_recent_aerodynamic_result))
            .route("/aerodynamics/flutter-curve/{bridge_id}", web::get().to(get_flutter_critical_speed_curve))
            .route("/optimization/run", web::post().to(run_optimization)),
    );
}
