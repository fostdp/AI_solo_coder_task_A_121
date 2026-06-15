use prometheus::{
    register_int_counter_vec, register_int_gauge, register_histogram_vec,
    Encoder, HistogramVec, IntCounterVec, IntGauge, TextEncoder,
};
use std::sync::OnceLock;
use std::time::Instant;

use actix_web::{web, HttpResponse, Responder};

pub struct Metrics {
    pub dtu_payloads_received: IntCounterVec,
    pub dtu_payloads_valid: IntCounterVec,
    pub dtu_payloads_invalid: IntCounterVec,
    pub aero_analyses_total: IntCounterVec,
    pub aero_analysis_duration_ms: HistogramVec,
    pub optimization_requests: IntCounterVec,
    pub optimization_duration_ms: HistogramVec,
    pub alerts_triggered: IntCounterVec,
    pub mqtt_messages_published: IntCounterVec,
    pub influxdb_writes_total: IntCounterVec,
    pub influxdb_write_errors: IntCounterVec,
    pub active_connections: IntGauge,
    pub active_models: IntGauge,
    pub pending_optimizations: IntGauge,
}

fn default_buckets() -> Vec<f64> {
    vec![
        1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
    ]
}

impl Metrics {
    fn new() -> Self {
        Metrics {
            dtu_payloads_received: register_int_counter_vec!(
                "bridge_dtu_payloads_received_total",
                "Total number of DTU payloads received",
                &["bridge_id"]
            )
            .unwrap(),
            dtu_payloads_valid: register_int_counter_vec!(
                "bridge_dtu_payloads_valid_total",
                "Total number of valid DTU payloads",
                &["bridge_id"]
            )
            .unwrap(),
            dtu_payloads_invalid: register_int_counter_vec!(
                "bridge_dtu_payloads_invalid_total",
                "Total number of invalid DTU payloads",
                &["bridge_id", "reason"]
            )
            .unwrap(),
            aero_analyses_total: register_int_counter_vec!(
                "bridge_aero_analyses_total",
                "Total number of aerodynamic analyses",
                &["bridge_id"]
            )
            .unwrap(),
            aero_analysis_duration_ms: register_histogram_vec!(
                "bridge_aero_analysis_duration_ms",
                "Aerodynamic analysis duration in milliseconds",
                &["bridge_id"],
                default_buckets()
            )
            .unwrap(),
            optimization_requests: register_int_counter_vec!(
                "bridge_optimization_requests_total",
                "Total number of optimization requests",
                &["bridge_id", "status"]
            )
            .unwrap(),
            optimization_duration_ms: register_histogram_vec!(
                "bridge_optimization_duration_ms",
                "Optimization duration in milliseconds",
                &["bridge_id"],
                default_buckets()
            )
            .unwrap(),
            alerts_triggered: register_int_counter_vec!(
                "bridge_alerts_triggered_total",
                "Total number of alerts triggered",
                &["bridge_id", "severity"]
            )
            .unwrap(),
            mqtt_messages_published: register_int_counter_vec!(
                "bridge_mqtt_messages_published_total",
                "Total number of MQTT messages published",
                &["topic"]
            )
            .unwrap(),
            influxdb_writes_total: register_int_counter_vec!(
                "bridge_influxdb_writes_total",
                "Total number of InfluxDB writes",
                &["measurement"]
            )
            .unwrap(),
            influxdb_write_errors: register_int_counter_vec!(
                "bridge_influxdb_write_errors_total",
                "Total number of InfluxDB write errors",
                &["measurement"]
            )
            .unwrap(),
            active_connections: register_int_gauge!(
                "bridge_active_connections",
                "Number of active HTTP connections"
            )
            .unwrap(),
            active_models: register_int_gauge!(
                "bridge_active_aero_models",
                "Number of active aerodynamic models in cache"
            )
            .unwrap(),
            pending_optimizations: register_int_gauge!(
                "bridge_pending_optimizations",
                "Number of pending optimization requests"
            )
            .unwrap(),
        }
    }
}

static METRICS: OnceLock<Metrics> = OnceLock::new();

pub fn get_metrics() -> &'static Metrics {
    METRICS.get_or_init(|| Metrics::new())
}

pub async fn metrics_handler() -> impl Responder {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();

    HttpResponse::Ok()
        .content_type(encoder.format_type())
        .body(buffer)
}

pub struct TimerGuard {
    start: Instant,
    observe: Box<dyn FnOnce(f64) + Send>,
}

impl TimerGuard {
    pub fn new<F>(observe: F) -> Self
    where
        F: FnOnce(f64) + Send + 'static,
    {
        TimerGuard {
            start: Instant::now(),
            observe: Box::new(observe),
        }
    }

    pub fn histogram(histogram: &'static HistogramVec, labels: &[&str]) -> Self {
        let labels_vec: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
        Self::new(move |duration_ms| {
            histogram.with_label_values(&labels_vec.iter().map(|s| s.as_str()).collect::<Vec<_>>()).observe(duration_ms);
        })
    }
}

impl Drop for TimerGuard {
    fn drop(&mut self) {
        let duration_ms = self.start.elapsed().as_secs_f64() * 1000.0;
        let observe = std::mem::replace(&mut self.observe, Box::new(|_| {}));
        observe(duration_ms);
    }
}

pub fn init_metrics() {
    let _ = get_metrics();
}

pub fn register_metrics(cfg: &mut web::ServiceConfig) {
    cfg.route("/metrics", web::get().to(metrics_handler));
}
