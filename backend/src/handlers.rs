use crate::models::*;
use crate::dtu_receiver::DTUReceiver;
use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::{info, warn};
use uuid::Uuid;

pub use crate::models::AppState;

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

fn lookup_bridge(bridge_id: &str) -> Option<&BridgeInfo> {
    BRIDGES.iter().find(|b| b.bridge_id == bridge_id)
}

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "status": "healthy",
        "service": "bridge_monitoring_backend",
        "version": "1.1.0",
        "architecture": "microservices_with_mpsc_channels"
    })))
}

pub async fn get_bridges() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(&*BRIDGES))
}

pub async fn get_bridge_handler(path: web::Path<String>) -> HttpResponse {
    let bridge_id = path.into_inner();
    match lookup_bridge(&bridge_id) {
        Some(bridge) => HttpResponse::Ok().json(ApiResponse::ok(bridge)),
        None => HttpResponse::NotFound().json(ApiResponse::<&BridgeInfo>::err("Bridge not found")),
    }
}

pub async fn receive_dtu_data(
    payload: web::Json<DTUPayload>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let bridge_id = payload.bridge_id.clone();
    info!("[API] 收到DTU上报 bridge_id={}", bridge_id);

    match DTUReceiver::validate_payload(&payload) {
        Ok(_) => {},
        Err(e) => {
            warn!("[API] DTU数据校验失败 bridge_id={}: {}", bridge_id, e);
            return HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value>::err(&format!("Validation failed: {}", e)));
        }
    }

    match data.dtu_receiver.process_payload(payload.into_inner()).await {
        Ok(count) => {
            let recent = data.recent_results.read().await;
            let result = recent.get(&bridge_id).cloned();
            drop(recent);

            if let Some(result) = result {
                HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
                    "written_points": count,
                    "aerodynamic_result": result
                })))
            } else {
                HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
                    "written_points": count,
                    "note": "Aerodynamic analysis in progress, result will be available shortly"
                })))
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<serde_json::Value>::err(&e)),
    }
}

pub async fn evaluate_aerodynamics(
    query: web::Query<AeroEvalQuery>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let attack_angle = query.attack_angle.unwrap_or(0.0);
    match data.flutter_analyzer.evaluate_external(
        &query.bridge_id, query.wind_speed, attack_angle, None
    ).await {
        Ok(result) => HttpResponse::Ok().json(ApiResponse::ok(result)),
        Err(e) => HttpResponse::NotFound().json(ApiResponse::<AerodynamicResult>::err(&e)),
    }
}

pub async fn evaluate_with_shape(
    query: web::Query<AeroEvalQuery>,
    shape: web::Json<DeckAerodynamicShape>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let attack_angle = query.attack_angle.unwrap_or(0.0);
    match data.flutter_analyzer.evaluate_external(
        &query.bridge_id, query.wind_speed, attack_angle, Some(&shape)
    ).await {
        Ok(result) => HttpResponse::Ok().json(ApiResponse::ok(result)),
        Err(e) => HttpResponse::NotFound().json(ApiResponse::<AerodynamicResult>::err(&e)),
    }
}

pub async fn get_vibration_response(
    query: web::Query<VibrationQuery>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let attack_angle = query.attack_angle.unwrap_or(0.0);
    match data.flutter_analyzer.compute_vibration_response(
        &query.bridge_id, query.wind_speed, attack_angle, query.duration, query.dt
    ).await {
        Ok(response) => HttpResponse::Ok().json(ApiResponse::ok(response)),
        Err(e) => HttpResponse::NotFound().json(ApiResponse::<VibrationResponse>::err(&e)),
    }
}

pub async fn get_deck_deformation(
    query: web::Query<DeformationQuery>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let attack_angle = query.attack_angle.unwrap_or(0.0);
    match data.flutter_analyzer.compute_deck_deformation(
        &query.bridge_id, query.wind_speed, attack_angle, query.segments
    ).await {
        Ok(deformation) => HttpResponse::Ok().json(ApiResponse::ok(deformation)),
        Err(e) => HttpResponse::NotFound().json(ApiResponse::<Vec<(f64, f64, f64)>>::err(&e)),
    }
}

pub async fn run_optimization(
    config: web::Json<OptimizationConfig>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let bridge_id = config.bridge_id.clone();
    if lookup_bridge(&bridge_id).is_none() {
        return HttpResponse::NotFound().json(ApiResponse::<OptimizationResult>::err("Bridge not found"));
    }

    let request_id = Uuid::new_v4();
    info!("[API] 收到优化请求 bridge_id={}, request_id={}", bridge_id, request_id);

    let (tx, rx) = oneshot::channel::<OptimizationResult>();
    data.pending_optimizations.lock().await.insert(request_id, tx);

    let msg = SystemMessage::OptimizationRequest {
        bridge_id: bridge_id.clone(),
        config: config.into_inner(),
        request_id,
    };

    if let Err(e) = data.optimizer_tx.send(msg).await {
        warn!("[API] 优化请求入队失败: {}", e);
        data.pending_optimizations.lock().await.remove(&request_id);
        return HttpResponse::InternalServerError().json(ApiResponse::<OptimizationResult>::err("Optimization service unavailable"));
    }

    match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
        Ok(Ok(result)) => {
            info!("[API] 优化完成 request_id={}, Ucr提升={:.1}%", request_id, result.improved_critical_speed);
            HttpResponse::Ok().json(ApiResponse::ok(result))
        }
        Ok(Err(_)) => HttpResponse::InternalServerError().json(ApiResponse::<OptimizationResult>::err("Optimization result channel closed")),
        Err(_) => {
            data.pending_optimizations.lock().await.remove(&request_id);
            HttpResponse::RequestTimeout().json(ApiResponse::<OptimizationResult>::err("Optimization timed out after 120s"))
        }
    }
}

pub async fn get_recent_aero_result(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let bridge_id = path.into_inner();
    let recent = data.recent_results.read().await;
    match recent.get(&bridge_id) {
        Some(result) => HttpResponse::Ok().json(ApiResponse::ok(result.clone())),
        None => {
            if let Some(_bridge) = lookup_bridge(&bridge_id) {
                match data.flutter_analyzer.evaluate_external(&bridge_id, 15.0, 0.0, None).await {
                    Ok(result) => HttpResponse::Ok().json(ApiResponse::ok(result)),
                    Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<AerodynamicResult>::err(&e)),
                }
            } else {
                HttpResponse::NotFound().json(ApiResponse::<AerodynamicResult>::err("Bridge not found"))
            }
        }
    }
}

pub async fn get_flutter_curve(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let bridge_id = path.into_inner();
    match data.flutter_analyzer.compute_flutter_curve(&bridge_id, None).await {
        Ok(curve) => HttpResponse::Ok().json(ApiResponse::ok(curve)),
        Err(e) => HttpResponse::NotFound().json(ApiResponse::<Vec<(f64, f64)>>::err(&e)),
    }
}
