use crate::models::{AlertMessage, AlertSeverity, AlertType};
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::time::Duration;
use tokio::time;

pub struct MQTTAlertService {
    client: AsyncClient,
    alert_topic: String,
    enabled: bool,
}

impl MQTTAlertService {
    pub fn new(host: &str, port: u16, client_id: &str, alert_topic: &str) -> Result<Self, String> {
        let mut mqtt_options = MqttOptions::new(client_id, host, port);
        mqtt_options.set_keep_alive(Duration::from_secs(30));
        mqtt_options.set_reconnect_delay(Duration::from_secs(5));

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 100);
        let alert_topic = alert_topic.to_string();

        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_notification) => {}
                    Err(e) => {
                        eprintln!("MQTT Eventloop error: {}", e);
                        time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Ok(MQTTAlertService {
            client,
            alert_topic,
            enabled: true,
        })
    }

    pub fn disabled() -> Self {
        MQTTAlertService {
            client: AsyncClient::new(MqttOptions::new("disabled", "localhost", 1883), 1).0,
            alert_topic: String::new(),
            enabled: false,
        }
    }

    pub async fn send_alert(&self, alert: &AlertMessage) -> Result<(), String> {
        if !self.enabled {
            eprintln!("[ALERT] {} - {}: {}", alert.bridge_id, alert.alert_type.as_str(), alert.message);
            return Ok(());
        }

        let payload = serde_json::to_string(alert)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let topic = format!(
            "{}/{}/{}",
            self.alert_topic,
            alert.severity.as_str(),
            alert.bridge_id
        );

        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload.as_bytes())
            .await
            .map_err(|e| format!("MQTT publish error: {}", e))?;

        Ok(())
    }

    pub async fn send_batch_alerts(&self, alerts: &[AlertMessage]) -> Result<usize, String> {
        let mut sent = 0;
        for alert in alerts {
            match self.send_alert(alert).await {
                Ok(()) => sent += 1,
                Err(e) => eprintln!("Failed to send alert: {}", e),
            }
        }
        Ok(sent)
    }
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertType::VibrationExceeded => "vibration_exceeded",
            AlertType::WindSpeedCritical => "wind_speed_critical",
            AlertType::CableForceAnomaly => "cable_force_anomaly",
            AlertType::FlutterImminent => "flutter_imminent",
            AlertType::SensorOffline => "sensor_offline",
        }
    }
}

impl AlertSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertSeverity::Info => "info",
            AlertSeverity::Warning => "warning",
            AlertSeverity::Critical => "critical",
            AlertSeverity::Emergency => "emergency",
        }
    }
}

pub struct AlertManager {
    mqtt_service: MQTTAlertService,
    vibration_threshold: f64,
    flutter_margin_threshold: f64,
    cable_force_deviation: f64,
}

impl AlertManager {
    pub fn new(mqtt_service: MQTTAlertService) -> Self {
        AlertManager {
            mqtt_service,
            vibration_threshold: 0.15,
            flutter_margin_threshold: 0.15,
            cable_force_deviation: 0.25,
        }
    }

    pub fn with_thresholds(
        mqtt_service: MQTTAlertService,
        vibration_threshold: f64,
        flutter_margin_threshold: f64,
        cable_force_deviation: f64,
    ) -> Self {
        AlertManager {
            mqtt_service,
            vibration_threshold,
            flutter_margin_threshold,
            cable_force_deviation,
        }
    }

    pub async fn check_vibration_alert(
        &self,
        bridge_id: &str,
        amplitude: f64,
        time: chrono::DateTime<chrono::Utc>,
    ) -> Option<AlertMessage> {
        if amplitude > self.vibration_threshold {
            let severity = if amplitude > self.vibration_threshold * 2.0 {
                AlertSeverity::Emergency
            } else if amplitude > self.vibration_threshold * 1.5 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            };
            let alert = AlertMessage {
                alert_id: uuid::Uuid::new_v4(),
                bridge_id: bridge_id.to_string(),
                alert_type: AlertType::VibrationExceeded,
                severity,
                message: format!(
                    "桥面振动幅值 {:.3}m 超过安全阈值 {:.3}m",
                    amplitude, self.vibration_threshold
                ),
                current_value: amplitude,
                threshold_value: self.vibration_threshold,
                timestamp: time,
                acknowledged: false,
            };
            let _ = self.mqtt_service.send_alert(&alert).await;
            return Some(alert);
        }
        None
    }

    pub async fn check_flutter_alert(
        &self,
        bridge_id: &str,
        flutter_margin: f64,
        wind_speed: f64,
        critical_speed: f64,
        time: chrono::DateTime<chrono::Utc>,
    ) -> Option<AlertMessage> {
        if flutter_margin < self.flutter_margin_threshold {
            let severity = if flutter_margin < 0.0 {
                AlertSeverity::Emergency
            } else if flutter_margin < 0.05 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            };
            let alert = AlertMessage {
                alert_id: uuid::Uuid::new_v4(),
                bridge_id: bridge_id.to_string(),
                alert_type: if flutter_margin < 0.0 {
                    AlertType::FlutterImminent
                } else {
                    AlertType::WindSpeedCritical
                },
                severity,
                message: format!(
                    "风速 {:.1}m/s 接近临界风速 {:.1}m/s，颤振裕度 {:.2}",
                    wind_speed, critical_speed, flutter_margin
                ),
                current_value: wind_speed,
                threshold_value: critical_speed * (1.0 - self.flutter_margin_threshold),
                timestamp: time,
                acknowledged: false,
            };
            let _ = self.mqtt_service.send_alert(&alert).await;
            return Some(alert);
        }
        None
    }

    pub async fn check_cable_force_alert(
        &self,
        bridge_id: &str,
        cable_id: &str,
        current_force: f64,
        nominal_force: f64,
        time: chrono::DateTime<chrono::Utc>,
    ) -> Option<AlertMessage> {
        if nominal_force <= 0.0 {
            return None;
        }
        let deviation = (current_force - nominal_force).abs() / nominal_force;
        if deviation > self.cable_force_deviation {
            let severity = if deviation > self.cable_force_deviation * 1.5 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            };
            let alert = AlertMessage {
                alert_id: uuid::Uuid::new_v4(),
                bridge_id: bridge_id.to_string(),
                alert_type: AlertType::CableForceAnomaly,
                severity,
                message: format!(
                    "索缆{}索力偏差 {:.1}%，当前 {:.1}kN，标称 {:.1}kN",
                    cable_id,
                    deviation * 100.0,
                    current_force / 1000.0,
                    nominal_force / 1000.0
                ),
                current_value: current_force,
                threshold_value: nominal_force * (1.0 + self.cable_force_deviation),
                timestamp: time,
                acknowledged: false,
            };
            let _ = self.mqtt_service.send_alert(&alert).await;
            return Some(alert);
        }
        None
    }
}
