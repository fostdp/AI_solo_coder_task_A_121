use crate::models::{
    CableForceData, DeckAccelerationData, DTUPayload, WindData, AerodynamicResult,
};
use chrono::{DateTime, Utc};
use influxdb::Query;
use influxdb::{Client, InfluxDbWriteable};

pub struct InfluxDBStorage {
    client: Client,
    database: String,
}

#[derive(InfluxDbWriteable)]
struct CableForcePoint {
    time: DateTime<Utc>,
    #[influxdb(tag)]
    bridge_id: String,
    #[influxdb(tag)]
    cable_id: String,
    cable_force: f64,
    temperature: f64,
}

#[derive(InfluxDbWriteable)]
struct AccelerationPoint {
    time: DateTime<Utc>,
    #[influxdb(tag)]
    bridge_id: String,
    #[influxdb(tag)]
    sensor_id: String,
    position_x: f64,
    acceleration_x: f64,
    acceleration_y: f64,
    acceleration_z: f64,
}

#[derive(InfluxDbWriteable)]
struct WindDataPoint {
    time: DateTime<Utc>,
    #[influxdb(tag)]
    bridge_id: String,
    #[influxdb(tag)]
    sensor_id: String,
    wind_speed: f64,
    wind_direction: f64,
    attack_angle: f64,
    temperature: f64,
    humidity: f64,
}

#[derive(InfluxDbWriteable)]
struct AerodynamicPoint {
    time: DateTime<Utc>,
    #[influxdb(tag)]
    bridge_id: String,
    wind_speed: f64,
    attack_angle: f64,
    aerodynamic_damping: f64,
    vibration_amplitude: f64,
    flutter_critical_speed: f64,
    flutter_margin: f64,
    is_safe: i64,
}

impl InfluxDBStorage {
    pub fn new(url: &str, database: &str, user: &str, password: &str) -> Self {
        let client = Client::new(url, database).with_auth(user, password);
        InfluxDBStorage {
            client,
            database: database.to_string(),
        }
    }

    pub async fn write_cable_force(&self, data: &CableForceData) -> Result<(), String> {
        let point = CableForcePoint {
            time: data.timestamp,
            bridge_id: data.bridge_id.clone(),
            cable_id: data.cable_id.clone(),
            cable_force: data.cable_force,
            temperature: data.temperature,
        };
        let query = point.into_query("cable_force");
        self.client
            .query(query)
            .await
            .map_err(|e| format!("InfluxDB error: {}", e))?;
        Ok(())
    }

    pub async fn write_acceleration(&self, data: &DeckAccelerationData) -> Result<(), String> {
        let point = AccelerationPoint {
            time: data.timestamp,
            bridge_id: data.bridge_id.clone(),
            sensor_id: data.sensor_id.clone(),
            position_x: data.position_x,
            acceleration_x: data.acceleration_x,
            acceleration_y: data.acceleration_y,
            acceleration_z: data.acceleration_z,
        };
        let query = point.into_query("deck_acceleration");
        self.client
            .query(query)
            .await
            .map_err(|e| format!("InfluxDB error: {}", e))?;
        Ok(())
    }

    pub async fn write_wind_data(&self, data: &WindData) -> Result<(), String> {
        let point = WindDataPoint {
            time: data.timestamp,
            bridge_id: data.bridge_id.clone(),
            sensor_id: data.sensor_id.clone(),
            wind_speed: data.wind_speed,
            wind_direction: data.wind_direction,
            attack_angle: data.attack_angle,
            temperature: data.temperature,
            humidity: data.humidity,
        };
        let query = point.into_query("wind_data");
        self.client
            .query(query)
            .await
            .map_err(|e| format!("InfluxDB error: {}", e))?;
        Ok(())
    }

    pub async fn write_aerodynamic_result(&self, result: &AerodynamicResult) -> Result<(), String> {
        let point = AerodynamicPoint {
            time: result.timestamp,
            bridge_id: result.bridge_id.clone(),
            wind_speed: result.wind_speed,
            attack_angle: result.attack_angle,
            aerodynamic_damping: result.aerodynamic_damping,
            vibration_amplitude: result.vibration_amplitude,
            flutter_critical_speed: result.flutter_critical_speed,
            flutter_margin: result.flutter_margin,
            is_safe: if result.is_safe { 1 } else { 0 },
        };
        let query = point.into_query("aerodynamic_result");
        self.client
            .query(query)
            .await
            .map_err(|e| format!("InfluxDB error: {}", e))?;
        Ok(())
    }

    pub async fn handle_dtu_payload(&self, payload: &DTUPayload) -> Result<usize, String> {
        let mut count = 0;
        for cf in &payload.cable_forces {
            let data = CableForceData {
                bridge_id: payload.bridge_id.clone(),
                cable_id: cf.cable_id.clone(),
                cable_force: cf.force,
                temperature: cf.temperature,
                timestamp: payload.timestamp,
            };
            self.write_cable_force(&data).await?;
            count += 1;
        }
        for acc in &payload.accelerations {
            let data = DeckAccelerationData {
                bridge_id: payload.bridge_id.clone(),
                sensor_id: acc.sensor_id.clone(),
                position_x: acc.position_x,
                acceleration_x: acc.ax,
                acceleration_y: acc.ay,
                acceleration_z: acc.az,
                timestamp: payload.timestamp,
            };
            self.write_acceleration(&data).await?;
            count += 1;
        }
        let wind = WindData {
            bridge_id: payload.bridge_id.clone(),
            sensor_id: payload.wind.sensor_id.clone(),
            wind_speed: payload.wind.speed,
            wind_direction: payload.wind.direction,
            attack_angle: payload.wind.attack_angle,
            temperature: payload.wind.temperature,
            humidity: payload.wind.humidity,
            timestamp: payload.timestamp,
        };
        self.write_wind_data(&wind).await?;
        count += 1;
        Ok(count)
    }

    pub async fn query_recent_wind(
        &self,
        bridge_id: &str,
        limit: usize,
    ) -> Result<Vec<WindData>, String> {
        let query_str = format!(
            "SELECT time, bridge_id, sensor_id, wind_speed, wind_direction, attack_angle, temperature, humidity \
             FROM wind_data WHERE bridge_id = '{}' ORDER BY time DESC LIMIT {}",
            bridge_id, limit
        );
        let query = Query::new(query_str);
        let _result = self.client.json_query(query).await
            .map_err(|e| format!("Query error: {}", e))?;
        Ok(Vec::new())
    }

    pub async fn query_aerodynamic_history(
        &self,
        bridge_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AerodynamicResult>, String> {
        let query_str = format!(
            "SELECT time, bridge_id, wind_speed, attack_angle, aerodynamic_damping, \
             vibration_amplitude, flutter_critical_speed, flutter_margin, is_safe \
             FROM aerodynamic_result WHERE bridge_id = '{}' AND time >= '{}' AND time <= '{}'",
            bridge_id,
            start.format("%Y-%m-%dT%H:%M:%SZ"),
            end.format("%Y-%m-%dT%H:%M:%SZ")
        );
        let _query = Query::new(query_str);
        Ok(Vec::new())
    }
}
