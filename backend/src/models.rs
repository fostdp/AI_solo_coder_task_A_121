use crate::dtu_receiver::DTUReceiver;
use crate::flutter_analyzer::FlutterAnalyzer;
use crate::influxdb_storage::InfluxDBStorage;
use crate::mqtt_alerts::{AlertManager, MQTTAlertService};
use crate::shape_optimizer::PendingOptimizations;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use validator::Validate;

pub struct AppState {
    pub storage: Arc<InfluxDBStorage>,
    pub dtu_receiver: Arc<DTUReceiver>,
    pub flutter_analyzer: Arc<FlutterAnalyzer>,
    pub optimizer_tx: mpsc::Sender<SystemMessage>,
    pub pending_optimizations: PendingOptimizations,
    pub alert_manager: Arc<AlertManager>,
    pub mqtt_service: Arc<MQTTAlertService>,
    pub recent_results: Arc<RwLock<HashMap<String, AerodynamicResult>>>,
    pub storage_tx: mpsc::Sender<SystemMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemMessage {
    DTUPayloadReceived {
        payload: DTUPayload,
        received_at: DateTime<Utc>,
    },
    AerodynamicResultReady {
        result: AerodynamicResult,
        source_payload: Option<DTUPayload>,
    },
    OptimizationRequest {
        bridge_id: String,
        config: OptimizationConfig,
        request_id: Uuid,
    },
    OptimizationResultReady {
        request_id: Uuid,
        result: OptimizationResult,
    },
    AlertTriggered {
        alert: AlertMessage,
    },
    StorageWriteRequest {
        measurement: StorageMeasurement,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageMeasurement {
    CableForce { bridge_id: String, cable_id: String, force: f64, temp: f64, time: DateTime<Utc> },
    Acceleration { bridge_id: String, sensor_id: String, ax: f64, ay: f64, az: f64, time: DateTime<Utc> },
    WindData { bridge_id: String, sensor_id: String, speed: f64, dir: f64, attack: f64, time: DateTime<Utc> },
    AeroResult(AerodynamicResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeInfo {
    pub bridge_id: String,
    pub name: String,
    pub location: String,
    pub latitude: f64,
    pub longitude: f64,
    pub length: f64,
    pub span: f64,
    pub width: f64,
    pub cable_count: usize,
    pub construction_year: u32,
    pub material: String,
    pub deck_height: f64,
    pub design_wind_speed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CableForceData {
    #[validate(length(min = 3, max = 10))]
    pub bridge_id: String,
    pub cable_id: String,
    pub cable_force: f64,
    pub temperature: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DeckAccelerationData {
    pub bridge_id: String,
    pub sensor_id: String,
    pub position_x: f64,
    pub acceleration_x: f64,
    pub acceleration_y: f64,
    pub acceleration_z: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct WindData {
    pub bridge_id: String,
    pub sensor_id: String,
    pub wind_speed: f64,
    pub wind_direction: f64,
    pub attack_angle: f64,
    pub temperature: f64,
    pub humidity: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DTUPayload {
    pub device_id: String,
    pub bridge_id: String,
    pub timestamp: DateTime<Utc>,
    pub cable_forces: Vec<CableForceReading>,
    pub accelerations: Vec<AccelerationReading>,
    #[serde(default)]
    pub wind: Option<WindReading>,
    #[serde(default)]
    pub winds: Vec<WindReading>,
    #[serde(default)]
    pub event_type: String,
}

impl DTUPayload {
    pub fn all_winds(&self) -> Vec<&WindReading> {
        let mut result: Vec<&WindReading> = self.winds.iter().collect();
        if let Some(w) = &self.wind {
            result.push(w);
        }
        result
    }

    pub fn max_wind_speed(&self) -> f64 {
        self.all_winds().iter().map(|w| w.speed).fold(0.0, f64::max)
    }

    pub fn avg_turbulence(&self) -> f64 {
        let winds = self.all_winds();
        if winds.is_empty() { return 0.1; }
        winds.iter().map(|w| w.turbulence_intensity).sum::<f64>() / winds.len() as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CableForceReading {
    pub cable_id: String,
    pub force: f64,
    pub temperature: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerationReading {
    pub sensor_id: String,
    pub position_x: f64,
    pub ax: f64,
    pub ay: f64,
    pub az: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindReading {
    pub sensor_id: String,
    pub speed: f64,
    pub direction: f64,
    pub attack_angle: f64,
    pub temperature: f64,
    pub humidity: f64,
    #[serde(default = "default_turbulence")]
    pub turbulence_intensity: f64,
}

fn default_turbulence() -> f64 { 0.1 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlutterDerivatives {
    pub h_star: [f64; 6],
    pub a_star: [f64; 6],
    pub h_prime: [f64; 6],
    pub a_prime: [f64; 6],
    pub h_star_ci: [f64; 6],
    pub a_star_ci: [f64; 6],
    pub h_prime_ci: [f64; 6],
    pub a_prime_ci: [f64; 6],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AerodynamicResult {
    pub bridge_id: String,
    pub wind_speed: f64,
    pub attack_angle: f64,
    pub aerodynamic_damping: f64,
    pub vibration_amplitude: f64,
    pub flutter_critical_speed: f64,
    pub flutter_margin: f64,
    pub is_safe: bool,
    pub timestamp: DateTime<Utc>,
    pub damping_confidence_interval: (f64, f64),
    pub amplitude_confidence_interval: (f64, f64),
    pub turbulence_intensity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConfig {
    pub bridge_id: String,
    pub population_size: usize,
    pub generations: usize,
    pub mutation_rate: f64,
    pub crossover_rate: f64,
    pub wind_speed_range: (f64, f64),
    pub attack_angle_range: (f64, f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckAerodynamicShape {
    pub wind_nose_angle: f64,
    pub stabilizer_plate_height: f64,
    pub stabilizer_plate_count: usize,
    pub deck_shape_type: DeckShapeType,
    pub fairing_length: f64,
    pub porosity: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DeckShapeType {
    Flat,
    Streamlined,
    Box,
    Slotted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub bridge_id: String,
    pub best_shape: DeckAerodynamicShape,
    pub best_fitness: f64,
    pub improved_critical_speed: f64,
    pub flutter_probability_reduction: f64,
    pub generation_history: Vec<f64>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMessage {
    pub alert_id: Uuid,
    pub bridge_id: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub current_value: f64,
    pub threshold_value: f64,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AlertType {
    VibrationExceeded,
    WindSpeedCritical,
    CableForceAnomaly,
    FlutterImminent,
    SensorOffline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Ord, PartialOrd, Eq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationResponse {
    pub bridge_id: String,
    pub time_points: Vec<f64>,
    pub displacement: Vec<f64>,
    pub velocity: Vec<f64>,
    pub acceleration: Vec<f64>,
    pub frequency: f64,
    pub damping_ratio: f64,
    pub rms_acceleration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckDeformationPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub displacement: f64,
    pub color_value: f64,
}

impl Default for DeckAerodynamicShape {
    fn default() -> Self {
        DeckAerodynamicShape {
            wind_nose_angle: 15.0,
            stabilizer_plate_height: 0.5,
            stabilizer_plate_count: 2,
            deck_shape_type: DeckShapeType::Flat,
            fairing_length: 0.3,
            porosity: 0.0,
        }
    }
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        OptimizationConfig {
            bridge_id: "BS001".to_string(),
            population_size: 50,
            generations: 100,
            mutation_rate: 0.1,
            crossover_rate: 0.8,
            wind_speed_range: (10.0, 60.0),
            attack_angle_range: (-10.0, 10.0),
        }
    }
}

use lazy_static::lazy_static;

lazy_static! {
    pub static ref BRIDGES: Vec<BridgeInfo> = vec![
        BridgeInfo {
            bridge_id: "BS001".to_string(), name: "泸定桥".to_string(), location: "四川甘孜泸定县".to_string(),
            latitude: 29.9092, longitude: 102.2374, length: 103.67, span: 100.0, width: 2.8, cable_count: 13,
            construction_year: 1706, material: "铁索".to_string(), deck_height: 14.5, design_wind_speed: 35.0,
        },
        BridgeInfo {
            bridge_id: "BS002".to_string(), name: "霁虹桥".to_string(), location: "云南保山澜沧江".to_string(),
            latitude: 25.4833, longitude: 99.4167, length: 113.4, span: 106.0, width: 3.7, cable_count: 18,
            construction_year: 1475, material: "铁索".to_string(), deck_height: 21.0, design_wind_speed: 32.0,
        },
        BridgeInfo {
            bridge_id: "BS003".to_string(), name: "云龙桥".to_string(), location: "贵州镇远舞阳河".to_string(),
            latitude: 27.0500, longitude: 108.4167, length: 95.0, span: 88.0, width: 3.2, cable_count: 12,
            construction_year: 1520, material: "铁索".to_string(), deck_height: 18.0, design_wind_speed: 30.0,
        },
        BridgeInfo {
            bridge_id: "BS004".to_string(), name: "重安江铁索桥".to_string(), location: "贵州黄平重安江".to_string(),
            latitude: 26.5833, longitude: 107.9167, length: 42.0, span: 36.5, width: 2.5, cable_count: 15,
            construction_year: 1871, material: "铁索".to_string(), deck_height: 10.0, design_wind_speed: 28.0,
        },
        BridgeInfo {
            bridge_id: "BS005".to_string(), name: "盘江铁索桥".to_string(), location: "贵州安顺盘江".to_string(),
            latitude: 25.7500, longitude: 104.7500, length: 78.0, span: 71.0, width: 2.9, cable_count: 14,
            construction_year: 1638, material: "铁索".to_string(), deck_height: 25.0, design_wind_speed: 38.0,
        },
        BridgeInfo {
            bridge_id: "BS006".to_string(), name: "程阳桥".to_string(), location: "广西柳州三江".to_string(),
            latitude: 25.9833, longitude: 109.6667, length: 64.4, span: 58.0, width: 3.4, cable_count: 10,
            construction_year: 1916, material: "铁木混合".to_string(), deck_height: 12.0, design_wind_speed: 25.0,
        },
        BridgeInfo {
            bridge_id: "BS007".to_string(), name: "金龙桥".to_string(), location: "云南丽江金沙江".to_string(),
            latitude: 27.0333, longitude: 100.4500, length: 116.0, span: 108.0, width: 3.2, cable_count: 16,
            construction_year: 1878, material: "铁索".to_string(), deck_height: 28.0, design_wind_speed: 40.0,
        },
        BridgeInfo {
            bridge_id: "BS008".to_string(), name: "豆沙关铁索桥".to_string(), location: "云南盐津豆沙关".to_string(),
            latitude: 28.2000, longitude: 104.2333, length: 55.0, span: 49.0, width: 2.6, cable_count: 11,
            construction_year: 1560, material: "铁索".to_string(), deck_height: 16.0, design_wind_speed: 33.0,
        },
        BridgeInfo {
            bridge_id: "BS009".to_string(), name: "普安桥".to_string(), location: "四川雅安天全".to_string(),
            latitude: 30.0833, longitude: 102.7833, length: 48.0, span: 42.0, width: 2.7, cable_count: 9,
            construction_year: 1812, material: "铁索".to_string(), deck_height: 11.0, design_wind_speed: 29.0,
        },
        BridgeInfo {
            bridge_id: "BS010".to_string(), name: "安顺场铁索桥".to_string(), location: "四川石棉安顺场".to_string(),
            latitude: 29.3333, longitude: 102.3833, length: 68.0, span: 62.0, width: 2.8, cable_count: 12,
            construction_year: 1780, material: "铁索".to_string(), deck_height: 13.0, design_wind_speed: 31.0,
        },
    ];
}
