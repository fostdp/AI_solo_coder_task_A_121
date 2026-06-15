use crate::aerodynamic_model::AerodynamicModel;
use crate::models::{
    AerodynamicResult, DeckAerodynamicShape, DTUPayload, StorageMeasurement,
    SystemMessage, VibrationResponse, BRIDGES,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, warn, debug};

type ModelCache = Arc<Mutex<HashMap<String, AerodynamicModel>>>;

#[derive(Clone)]
pub struct FlutterAnalyzer {
    model_cache: ModelCache,
    alarm_tx: mpsc::Sender<SystemMessage>,
    storage_tx: mpsc::Sender<SystemMessage>,
    recent_results: Arc<RwLock<HashMap<String, AerodynamicResult>>>,
}

impl FlutterAnalyzer {
    pub fn new(
        alarm_tx: mpsc::Sender<SystemMessage>,
        storage_tx: mpsc::Sender<SystemMessage>,
        recent_results: Arc<RwLock<HashMap<String, AerodynamicResult>>>,
    ) -> Self {
        FlutterAnalyzer {
            model_cache: Arc::new(Mutex::new(HashMap::new())),
            alarm_tx,
            storage_tx,
            recent_results,
        }
    }

    pub async fn run(self, mut rx: mpsc::Receiver<SystemMessage>) {
        info!("[Analyzer] 颤振分析服务启动");
        while let Some(msg) = rx.recv().await {
            match msg {
                SystemMessage::DTUPayloadReceived { payload, received_at: _ } => {
                    let result = self.process_dtu_payload(payload).await;
                    if let Ok(result) = result {
                        self.broadcast_result(result).await;
                    }
                }
                SystemMessage::Shutdown => {
                    info!("[Analyzer] 收到关机信号，退出");
                    break;
                }
                _ => {
                    debug!("[Analyzer] 忽略非DTU消息: {:?}", msg);
                }
            }
        }
        info!("[Analyzer] 服务已停止");
    }

    async fn get_or_create_model(&self, bridge_id: &str) -> Option<AerodynamicModel> {
        let mut cache = self.model_cache.lock().await;
        if let Some(model) = cache.get(bridge_id) {
            return Some(model.clone());
        }
        let bridge = BRIDGES.iter().find(|b| b.bridge_id == bridge_id)?;
        let model = AerodynamicModel::new(bridge);
        cache.insert(bridge_id.to_string(), model.clone());
        info!("[Analyzer] 为桥梁 {} 创建气动模型实例", bridge_id);
        Some(model)
    }

    async fn process_dtu_payload(&self, payload: DTUPayload) -> Result<AerodynamicResult, String> {
        let bridge_id = payload.bridge_id.clone();
        let wind_speed = payload.wind.speed;
        let attack_angle = payload.wind.attack_angle;

        let model = self.get_or_create_model(&bridge_id).await
            .ok_or_else(|| format!("Bridge not found: {}", bridge_id))?;

        let result = model.evaluate_aerodynamic_performance(wind_speed, attack_angle, None);
        debug!("[Analyzer] {}: U={:.1}m/s, α={:.1}°, ξ={:.4}, A={:.3}m, Ucr={:.1}m/s",
            bridge_id, wind_speed, attack_angle,
            result.aerodynamic_damping, result.vibration_amplitude, result.flutter_critical_speed);

        Ok(result)
    }

    async fn broadcast_result(&self, result: AerodynamicResult) {
        let bridge_id = result.bridge_id.clone();

        self.recent_results.write().await.insert(bridge_id.clone(), result.clone());

        if let Err(e) = self.storage_tx.send(SystemMessage::StorageWriteRequest {
            measurement: StorageMeasurement::AeroResult(result.clone()),
        }).await {
            warn!("[Analyzer] 气动结果写入队列失败: {}", e);
        }

        if let Err(e) = self.alarm_tx.send(SystemMessage::AerodynamicResultReady {
            result: result.clone(),
            source_payload: None,
        }).await {
            warn!("[Analyzer] 气动结果告警队列失败: {}", e);
        }

        info!("[Analyzer] {} 气动分析完成: safe={}, margin={:.1}%",
            bridge_id, result.is_safe, result.flutter_margin * 100.0);
    }

    pub async fn evaluate_external(
        &self,
        bridge_id: &str,
        wind_speed: f64,
        attack_angle: f64,
        shape: Option<&DeckAerodynamicShape>,
    ) -> Result<AerodynamicResult, String> {
        let model = self.get_or_create_model(bridge_id).await
            .ok_or_else(|| format!("Bridge not found: {}", bridge_id))?;
        Ok(model.evaluate_aerodynamic_performance(wind_speed, attack_angle, shape))
    }

    pub async fn compute_vibration_response(
        &self,
        bridge_id: &str,
        wind_speed: f64,
        attack_angle: f64,
        duration: Option<f64>,
        dt: Option<f64>,
    ) -> Result<VibrationResponse, String> {
        let model = self.get_or_create_model(bridge_id).await
            .ok_or_else(|| format!("Bridge not found: {}", bridge_id))?;
        Ok(model.compute_vibration_response(wind_speed, attack_angle, duration.unwrap_or(10.0), dt.unwrap_or(0.01)))
    }

    pub async fn compute_deck_deformation(
        &self,
        bridge_id: &str,
        wind_speed: f64,
        attack_angle: f64,
        points: Option<usize>,
    ) -> Result<Vec<(f64, f64, f64)>, String> {
        let model = self.get_or_create_model(bridge_id).await
            .ok_or_else(|| format!("Bridge not found: {}", bridge_id))?;
        Ok(model.compute_deck_deformation(wind_speed, attack_angle, points.unwrap_or(20)))
    }

    pub async fn compute_flutter_curve(
        &self,
        bridge_id: &str,
        shape: Option<DeckAerodynamicShape>,
    ) -> Result<Vec<(f64, f64)>, String> {
        let model = self.get_or_create_model(bridge_id).await
            .ok_or_else(|| format!("Bridge not found: {}", bridge_id))?;
        let mut curve = Vec::with_capacity(21);
        for i in 0..=20 {
            let attack_angle = -10.0 + i as f64;
            let critical_speed = model.compute_flutter_critical_speed(shape.as_ref());
            curve.push((attack_angle, critical_speed));
        }
        Ok(curve)
    }
}
