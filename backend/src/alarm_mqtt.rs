use crate::models::{AlertMessage, AerodynamicResult, SystemMessage};
use crate::models::AlertSeverity;
use crate::mqtt_alerts::{AlertManager, MQTTAlertService};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error, debug};

pub struct AlarmService {
    alert_manager: AlertManager,
    mqtt_service: Arc<MQTTAlertService>,
    mqtt_publish_tx: mpsc::Sender<AlertMessage>,
}

impl AlarmService {
    pub fn new(
        alert_manager: AlertManager,
        mqtt_service: Arc<MQTTAlertService>,
        mqtt_publish_tx: mpsc::Sender<AlertMessage>,
    ) -> Self {
        AlarmService { alert_manager, mqtt_service, mqtt_publish_tx }
    }

    pub async fn run(self, mut rx: mpsc::Receiver<SystemMessage>) {
        info!("[Alarm] 告警评估与MQTT推送服务启动");
        while let Some(msg) = rx.recv().await {
            match msg {
                SystemMessage::AerodynamicResultReady { result, source_payload } => {
                    self.process_aero_result(result, source_payload).await;
                }
                SystemMessage::AlertTriggered { alert } => {
                    self.dispatch_alert(alert).await;
                }
                SystemMessage::Shutdown => {
                    info!("[Alarm] 收到关机信号，退出");
                    break;
                }
                _ => {
                    debug!("[Alarm] 忽略非告警消息: {:?}", msg);
                }
            }
        }
        info!("[Alarm] 服务已停止");
    }

    async fn process_aero_result(&self, result: AerodynamicResult, _payload: Option<crate::models::DTUPayload>) {
        let bridge_id = &result.bridge_id;
        let mut alerts = Vec::new();
        let now = chrono::Utc::now();

        if let Some(alert) = self.alert_manager.check_vibration_alert(
            bridge_id, result.vibration_amplitude, now
        ).await {
            alerts.push(alert);
        }

        if let Some(alert) = self.alert_manager.check_flutter_alert(
            bridge_id, result.flutter_margin, result.wind_speed, result.flutter_critical_speed, now
        ).await {
            alerts.push(alert);
        }

        if result.aerodynamic_damping < 0.0 {
            alerts.push(crate::models::AlertMessage {
                alert_id: uuid::Uuid::new_v4(),
                bridge_id: bridge_id.to_string(),
                alert_type: crate::models::AlertType::FlutterImminent,
                severity: AlertSeverity::Critical,
                message: format!("气动阻尼为负 ξ={:.4}, 可能发生颤振", result.aerodynamic_damping),
                current_value: result.aerodynamic_damping,
                threshold_value: 0.0,
                timestamp: now,
                acknowledged: false,
            });
        }

        if alerts.is_empty() {
            debug!("[Alarm] {}: 无告警触发, 裕度={:.1}%, 阻尼={:.4}",
                bridge_id, result.flutter_margin * 100.0, result.aerodynamic_damping);
        } else {
            for alert in alerts {
                self.dispatch_alert(alert).await;
            }
        }
    }

    async fn dispatch_alert(&self, alert: AlertMessage) {
        let severity = alert.severity.clone();
        let bridge_id = alert.bridge_id.clone();
        let message = alert.message.clone();

        if severity != AlertSeverity::Info {
            match serde_json::to_string(&alert) {
                Ok(_payload) => {
                    if let Err(e) = self.mqtt_publish_tx.send(alert.clone()).await {
                        warn!("[Alarm] MQTT发送队列失败: {}", e);
                    } else {
                        info!("[Alarm] 告警已推送 [{}] {}: {}", severity, bridge_id, message);
                    }
                }
                Err(e) => {
                    error!("[Alarm] 告警序列化失败: {}", e);
                }
            }
        }
    }
}

pub async fn mqtt_publisher_worker(
    mqtt_service: Arc<MQTTAlertService>,
    mut rx: mpsc::Receiver<AlertMessage>,
) {
    info!("[Alarm] MQTT发布Worker启动");
    while let Some(alert) = rx.recv().await {
        match serde_json::to_string(&alert) {
            Ok(_payload) => {
                if let Err(e) = mqtt_service.send_alert(&alert).await {
                    warn!("[Alarm] MQTT实际发送失败: {}", e);
                }
            }
            Err(e) => {
                error!("[Alarm] 告警序列化失败: {}", e);
            }
        }
    }
    info!("[Alarm] MQTT发布Worker已停止");
}
