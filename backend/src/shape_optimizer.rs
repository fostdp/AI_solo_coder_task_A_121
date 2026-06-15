use crate::aerodynamic_model::AerodynamicModel;
use crate::genetic_optimizer::GeneticOptimizer;
use crate::models::{
    OptimizationConfig, OptimizationResult, SystemMessage, BRIDGES,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, oneshot};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

pub type PendingOptimizations = Arc<Mutex<HashMap<Uuid, oneshot::Sender<OptimizationResult>>>>;

pub struct ShapeOptimizer {
    pending: PendingOptimizations,
    model_cache: Arc<Mutex<HashMap<String, AerodynamicModel>>>,
}

impl ShapeOptimizer {
    pub fn new(pending: PendingOptimizations) -> Self {
        ShapeOptimizer {
            pending,
            model_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(self, mut rx: mpsc::Receiver<SystemMessage>) {
        info!("[Optimizer] 气动外形优化服务启动");
        while let Some(msg) = rx.recv().await {
            match msg {
                SystemMessage::OptimizationRequest { bridge_id, config, request_id } => {
                    info!("[Optimizer] 收到优化请求 request_id={}, bridge={}", request_id, bridge_id);
                    let this = self.clone();
                    tokio::spawn(async move {
                        match this.process_optimization(&bridge_id, config).await {
                            Ok(result) => {
                                this.send_result(request_id, result).await;
                            }
                            Err(e) => {
                                error!("[Optimizer] 优化失败 request_id={}: {}", request_id, e);
                            }
                        }
                    });
                }
                SystemMessage::Shutdown => {
                    info!("[Optimizer] 收到关机信号，退出");
                    break;
                }
                _ => {
                    debug!("[Optimizer] 忽略非优化消息: {:?}", msg);
                }
            }
        }
        info!("[Optimizer] 服务已停止");
    }

    async fn get_or_create_model(&self, bridge_id: &str) -> Option<AerodynamicModel> {
        let mut cache = self.model_cache.lock().await;
        if let Some(model) = cache.get(bridge_id) {
            return Some(model.clone());
        }
        let bridge = BRIDGES.iter().find(|b| b.bridge_id == bridge_id)?;
        let model = AerodynamicModel::new(bridge);
        cache.insert(bridge_id.to_string(), model.clone());
        info!("[Optimizer] 为桥梁 {} 创建气动模型实例", bridge_id);
        Some(model)
    }

    async fn process_optimization(
        &self,
        bridge_id: &str,
        config: OptimizationConfig,
    ) -> Result<OptimizationResult, String> {
        let model = self.get_or_create_model(bridge_id).await
            .ok_or_else(|| format!("Bridge not found: {}", bridge_id))?;

        let pop_size = config.population_size;
        let gen_count = config.generations;
        let optimizer = GeneticOptimizer::new(&model, config);
        info!("[Optimizer] 开始GA优化: pop={}, gen={}", pop_size, gen_count);
        let result = optimizer.run();
        info!("[Optimizer] 优化完成: Ucr提升 {:.1}%, 颤振概率降低 {:.1}%",
            result.improved_critical_speed, result.flutter_probability_reduction);
        Ok(result)
    }

    async fn send_result(&self, request_id: Uuid, result: OptimizationResult) {
        let mut pending = self.pending.lock().await;
        if let Some(tx) = pending.remove(&request_id) {
            match tx.send(result) {
                Ok(_) => {
                    info!("[Optimizer] 优化结果已发送 request_id={}", request_id);
                }
                Err(_) => {
                    warn!("[Optimizer] 优化结果发送失败，接收方已断开 request_id={}", request_id);
                }
            }
        } else {
            warn!("[Optimizer] 未找到等待中的请求 request_id={}", request_id);
        }
    }
}

impl Clone for ShapeOptimizer {
    fn clone(&self) -> Self {
        ShapeOptimizer {
            pending: self.pending.clone(),
            model_cache: self.model_cache.clone(),
        }
    }
}
