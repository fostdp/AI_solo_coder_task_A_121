use crate::models::{DTUPayload, StorageMeasurement, SystemMessage};
use crate::influxdb_storage::InfluxDBStorage;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::mpsc;

use tracing::{info, warn, error};

pub struct DTUReceiver {
    storage: Arc<InfluxDBStorage>,
    analyzer_tx: mpsc::Sender<SystemMessage>,
    storage_tx: mpsc::Sender<SystemMessage>,
}

impl DTUReceiver {
    pub fn new(
        storage: Arc<InfluxDBStorage>,
        analyzer_tx: mpsc::Sender<SystemMessage>,
        storage_tx: mpsc::Sender<SystemMessage>,
    ) -> Self {
        DTUReceiver { storage, analyzer_tx, storage_tx }
    }

    pub async fn process_payload(&self, payload: DTUPayload) -> Result<usize, String> {
        let received_at = Utc::now();

        match Self::validate_payload(&payload) {
            Ok(_) => {},
            Err(e) => {
                warn!("[DTU] 数据校验失败 bridge_id={}: {}", payload.bridge_id, e);
                return Err(format!("Validation failed: {}", e));
            }
        }

        let count = self.write_to_storage(&payload).await?;

        let bridge_id = payload.bridge_id.clone();
        let msg = SystemMessage::DTUPayloadReceived { payload: payload.clone(), received_at };
        if let Err(e) = self.analyzer_tx.send(msg).await {
            error!("[DTU] 发送到分析器失败 bridge_id={}: {}", bridge_id, e);
        } else {
            info!("[DTU] 数据已入站 bridge_id={}, cable_forces={}, accelerations={}",
                bridge_id, payload.cable_forces.len(), payload.accelerations.len());
        }

        Ok(count)
    }

    async fn write_to_storage(&self, payload: &DTUPayload) -> Result<usize, String> {
        let bridge_id = &payload.bridge_id;
        let timestamp = payload.timestamp;
        let mut count = 0;

        for cf in &payload.cable_forces {
            let msg = SystemMessage::StorageWriteRequest {
                measurement: StorageMeasurement::CableForce {
                    bridge_id: bridge_id.clone(),
                    cable_id: cf.cable_id.clone(),
                    force: cf.force,
                    temp: cf.temperature,
                    time: timestamp,
                },
            };
            if let Err(e) = self.storage_tx.send(msg).await {
                warn!("[DTU] 索力写入队列失败: {}", e);
            } else { count += 1; }
        }

        for acc in &payload.accelerations {
            let msg = SystemMessage::StorageWriteRequest {
                measurement: StorageMeasurement::Acceleration {
                    bridge_id: bridge_id.clone(),
                    sensor_id: acc.sensor_id.clone(),
                    ax: acc.ax,
                    ay: acc.ay,
                    az: acc.az,
                    time: timestamp,
                },
            };
            if let Err(e) = self.storage_tx.send(msg).await {
                warn!("[DTU] 加速度写入队列失败: {}", e);
            } else { count += 1; }
        }

        for wd in payload.all_winds() {
            let msg = SystemMessage::StorageWriteRequest {
                measurement: StorageMeasurement::WindData {
                    bridge_id: bridge_id.clone(),
                    sensor_id: wd.sensor_id.clone(),
                    speed: wd.speed,
                    dir: wd.direction,
                    attack: wd.attack_angle,
                    time: timestamp,
                },
            };
            if let Err(e) = self.storage_tx.send(msg).await {
                warn!("[DTU] 风数据写入队列失败: {}", e);
            } else { count += 1; }
        }

        Ok(count)
    }

    pub fn validate_payload(payload: &DTUPayload) -> Result<(), String> {
        if payload.bridge_id.is_empty() {
            return Err("bridge_id cannot be empty".to_string());
        }
        if payload.cable_forces.is_empty() && payload.accelerations.is_empty() && payload.all_winds().is_empty() {
            return Err("payload has no sensor data".to_string());
        }
        for cf in &payload.cable_forces {
            if cf.force < 0.0 || cf.force > 10_000_000.0 {
                return Err(format!("cable_force out of range: {}", cf.force));
            }
        }
        for acc in &payload.accelerations {
            if acc.az.abs() > 10.0 {
                return Err(format!("acceleration out of range: {}g", acc.az));
            }
        }
        for wd in payload.all_winds() {
            if wd.speed < 0.0 || wd.speed > 150.0 {
                return Err(format!("wind_speed out of range: {} m/s", wd.speed));
            }
            if wd.attack_angle < -45.0 || wd.attack_angle > 45.0 {
                return Err(format!("attack_angle out of range: {} deg", wd.attack_angle));
            }
            if wd.turbulence_intensity < 0.0 || wd.turbulence_intensity > 1.0 {
                return Err(format!("turbulence_intensity out of range: {}", wd.turbulence_intensity));
            }
        }
        Ok(())
    }
}
