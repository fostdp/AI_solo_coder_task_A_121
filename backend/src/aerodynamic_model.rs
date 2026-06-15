use crate::models::{
    AerodynamicResult, BridgeInfo, DeckAerodynamicShape, DeckShapeType, FlutterDerivatives,
    VibrationResponse,
};
use chrono::Utc;
use std::f64::consts::PI;
use std::sync::Mutex;

const AIR_DENSITY: f64 = 1.225;
const GRAVITY: f64 = 9.81;
const CONFIDENCE_Z: f64 = 1.96;

struct KalmanState {
    x: f64,
    p: f64,
    q: f64,
    r: f64,
}

impl KalmanState {
    fn new(initial: f64, process_noise: f64, measure_noise: f64) -> Self {
        KalmanState { x: initial, p: 1.0, q: process_noise, r: measure_noise }
    }

    fn update(&mut self, z: f64) -> (f64, f64) {
        let p_pred = self.p + self.q;
        let k = p_pred / (p_pred + self.r);
        self.x += k * (z - self.x);
        self.p = (1.0 - k) * p_pred;
        (self.x, self.p.sqrt())
    }
}

pub struct AerodynamicModel {
    pub bridge: BridgeInfo,
    pub flutter_derivatives: FlutterDerivatives,
    pub mass_per_unit_length: f64,
    pub mass_moment_of_inertia: f64,
    pub bending_frequency: f64,
    pub torsional_frequency: f64,
    pub structural_damping: f64,
    ema_damping: Mutex<f64>,
    ema_amplitude: Mutex<f64>,
    kalman_h: Mutex<KalmanState>,
    kalman_a: Mutex<KalmanState>,
    reduced_freq_history: Mutex<Vec<f64>>,
}

impl AerodynamicModel {
    pub fn new(bridge: &BridgeInfo) -> Self {
        let mass_per_unit_length = bridge.width * 0.5 * 7850.0;
        let mass_moment_of_inertia = mass_per_unit_length * bridge.width.powi(2) / 12.0;
        let bending_frequency = 1.2 * (GRAVITY / bridge.span).sqrt();
        let torsional_frequency = bending_frequency * 2.5;

        AerodynamicModel {
            bridge: bridge.clone(),
            flutter_derivatives: Self::default_flutter_derivatives(),
            mass_per_unit_length,
            mass_moment_of_inertia,
            bending_frequency,
            torsional_frequency,
            structural_damping: 0.01,
            ema_damping: Mutex::new(0.01),
            ema_amplitude: Mutex::new(0.001),
            kalman_h: Mutex::new(KalmanState::new(-0.3, 0.001, 0.05)),
            kalman_a: Mutex::new(KalmanState::new(-1.5, 0.001, 0.08)),
            reduced_freq_history: Mutex::new(Vec::with_capacity(20)),
        }
    }

    fn default_flutter_derivatives() -> FlutterDerivatives {
        FlutterDerivatives {
            h_star: [0.0, -0.5, -0.8, -0.6, -0.3, -0.1],
            a_star: [0.0, -1.0, -2.0, -2.5, -2.2, -1.8],
            h_prime: [0.0, 2.0, 4.0, 3.5, 2.5, 1.5],
            a_prime: [0.0, 0.5, 1.0, 1.5, 2.0, 2.5],
            h_star_ci: [0.01, 0.03, 0.05, 0.04, 0.03, 0.02],
            a_star_ci: [0.01, 0.05, 0.08, 0.10, 0.09, 0.07],
            h_prime_ci: [0.02, 0.06, 0.10, 0.09, 0.07, 0.05],
            a_prime_ci: [0.01, 0.02, 0.04, 0.05, 0.06, 0.06],
        }
    }

    fn lerp(table: &[f64; 6], x: f64) -> f64 {
        let x = x.clamp(0.0, 5.0);
        let idx = x.floor() as usize;
        let frac = x - idx as f64;
        if idx >= 5 { return table[5]; }
        table[idx] * (1.0 - frac) + table[idx + 1] * frac
    }

    pub fn flutter_derivatives_for_reduced_freq(&self, reduced_freq: f64) -> (f64, f64, f64, f64) {
        let k_norm = (reduced_freq * 2.0).clamp(0.0, 5.0);
        let mut history = self.reduced_freq_history.lock().unwrap();
        history.push(k_norm);
        if history.len() > 20 { history.remove(0); }
        drop(history);

        let h_star = Self::lerp(&self.flutter_derivatives.h_star, k_norm);
        let a_star = Self::lerp(&self.flutter_derivatives.a_star, k_norm);
        let h_prime = Self::lerp(&self.flutter_derivatives.h_prime, k_norm);
        let a_prime = Self::lerp(&self.flutter_derivatives.a_prime, k_norm);

        let mut kalman_h = self.kalman_h.lock().unwrap();
        let mut kalman_a = self.kalman_a.lock().unwrap();
        let (h_smoothed, _) = kalman_h.update(h_star);
        let (a_smoothed, _) = kalman_a.update(a_star);

        (h_smoothed, a_smoothed, h_prime, a_prime)
    }

    pub fn flutter_derivatives_with_ci(&self, reduced_freq: f64) -> ((f64, f64, f64, f64), (f64, f64, f64, f64)) {
        let k_norm = (reduced_freq * 2.0).clamp(0.0, 5.0);

        let h = Self::lerp(&self.flutter_derivatives.h_star, k_norm);
        let a = Self::lerp(&self.flutter_derivatives.a_star, k_norm);
        let hp = Self::lerp(&self.flutter_derivatives.h_prime, k_norm);
        let ap = Self::lerp(&self.flutter_derivatives.a_prime, k_norm);

        let ci_h = Self::lerp(&self.flutter_derivatives.h_star_ci, k_norm);
        let ci_a = Self::lerp(&self.flutter_derivatives.a_star_ci, k_norm);
        let ci_hp = Self::lerp(&self.flutter_derivatives.h_prime_ci, k_norm);
        let ci_ap = Self::lerp(&self.flutter_derivatives.a_prime_ci, k_norm);

        let history = self.reduced_freq_history.lock().unwrap();
        let turb_factor = if history.len() >= 5 {
            let mean = history.iter().sum::<f64>() / history.len() as f64;
            let variance = history.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / history.len() as f64;
            1.0 + variance.sqrt() * 2.0
        } else { 1.0 };
        drop(history);

        ((h, a, hp, ap), (ci_h * turb_factor, ci_a * turb_factor, ci_hp * turb_factor, ci_ap * turb_factor))
    }

    pub fn compute_quasi_steady_force(&self, wind_speed: f64, attack_angle: f64) -> (f64, f64, f64) {
        let alpha = attack_angle.to_radians();
        let cl = 2.0 * PI * alpha;
        let cd = 0.02 + 2.0 * PI * alpha.powi(2);
        let cm = 0.5 * PI * alpha;

        let q = 0.5 * AIR_DENSITY * wind_speed.powi(2) * self.bridge.width;
        let lift = q * cl;
        let drag = q * cd;
        let moment = q * self.bridge.width * cm;
        (lift, drag, moment)
    }

    pub fn compute_aerodynamic_damping(&self, wind_speed: f64, attack_angle: f64) -> f64 {
        let (damping, _) = self.compute_aerodynamic_damping_with_ci(wind_speed, attack_angle);
        damping
    }

    pub fn compute_aerodynamic_damping_with_ci(&self, wind_speed: f64, _attack_angle: f64) -> (f64, f64) {
        if wind_speed <= 3.0 {
            let conservative_damping = self.structural_damping * (1.0 - 0.2 * (3.0 - wind_speed) / 3.0);
            return (conservative_damping, self.structural_damping * 0.15);
        }
        let omega = 2.0 * PI * self.bending_frequency;
        let reduced_freq = omega * self.bridge.width / wind_speed;
        let ((h_star, _, _, _), (ci_h, _, _, _)) = self.flutter_derivatives_with_ci(reduced_freq);
        let rho_b = AIR_DENSITY * self.bridge.width.powi(2) / (2.0 * self.mass_per_unit_length);
        let aerodynamic_damping = -rho_b * h_star / (2.0 * reduced_freq);
        let mut damping = self.structural_damping + aerodynamic_damping;

        let mut ema = self.ema_damping.lock().unwrap();
        let alpha = 0.3;
        damping = alpha * damping + (1.0 - alpha) * *ema;
        *ema = damping;
        drop(ema);

        let ci = (rho_b / (2.0 * reduced_freq)) * ci_h + self.structural_damping * 0.05;
        (damping, ci)
    }

    pub fn compute_flutter_critical_speed(&self, shape: Option<&DeckAerodynamicShape>) -> f64 {
        let mu = self.mass_moment_of_inertia
            / (AIR_DENSITY * self.bridge.width.powi(4));
        let r = self.mass_moment_of_inertia
            / (self.mass_per_unit_length * self.bridge.width.powi(2));
        let x_alpha = 0.2;
        let omega_h = 2.0 * PI * self.bending_frequency;
        let omega_alpha = 2.0 * PI * self.torsional_frequency;

        let base_critical = (omega_h * self.bridge.width)
            * (8.0 * mu * r * (omega_alpha.powi(2) / omega_h.powi(2) - 1.0)).sqrt()
            / (x_alpha * 0.6);

        let correction = shape
            .map(|s| {
                let nose_correction = 1.0 + s.wind_nose_angle / 45.0 * 0.15;
                let stabilizer_correction = 1.0 + s.stabilizer_plate_count as f64
                    * s.stabilizer_plate_height / self.bridge.width * 0.25;
                let fairing_correction = 1.0 + s.fairing_length / self.bridge.width * 0.2;
                let shape_correction = match s.deck_shape_type {
                    DeckShapeType::Flat => 1.0,
                    DeckShapeType::Streamlined => 1.35,
                    DeckShapeType::Box => 1.2,
                    DeckShapeType::Slotted => 1.25,
                };
                nose_correction * stabilizer_correction * fairing_correction * shape_correction
            })
            .unwrap_or(1.0);

        base_critical * correction
    }

    pub fn compute_vibration_amplitude(&self, wind_speed: f64, attack_angle: f64) -> f64 {
        let (amp, _) = self.compute_vibration_amplitude_with_ci(wind_speed, attack_angle);
        amp
    }

    pub fn compute_vibration_amplitude_with_ci(&self, wind_speed: f64, attack_angle: f64) -> (f64, f64) {
        if wind_speed <= 1.0 {
            return (0.001, 0.0005);
        }
        let omega = 2.0 * PI * self.bending_frequency;
        let (damping, damping_ci) = self.compute_aerodynamic_damping_with_ci(wind_speed, attack_angle);
        let damping = damping.max(0.0001);
        let (lift, _, _) = self.compute_quasi_steady_force(wind_speed, attack_angle);
        let max_lift = lift.abs();
        let amplitude = max_lift
            / (self.mass_per_unit_length * omega.powi(2) * 2.0 * damping);
        let amplitude = amplitude.min(2.0);

        let mut ema = self.ema_amplitude.lock().unwrap();
        let alpha = 0.25;
        let smoothed = alpha * amplitude + (1.0 - alpha) * *ema;
        *ema = smoothed;
        drop(ema);

        let amp_ci = smoothed * (damping_ci / damping + 0.08);
        (smoothed, amp_ci)
    }

    pub fn evaluate_aerodynamic_performance(
        &self,
        wind_speed: f64,
        attack_angle: f64,
        shape: Option<&DeckAerodynamicShape>,
    ) -> AerodynamicResult {
        let critical_speed = self.compute_flutter_critical_speed(shape);
        let (damping, damping_ci) = self.compute_aerodynamic_damping_with_ci(wind_speed, attack_angle);
        let (amplitude, amplitude_ci) = self.compute_vibration_amplitude_with_ci(wind_speed, attack_angle);
        let flutter_margin = if wind_speed > 0.0 {
            (critical_speed - wind_speed) / critical_speed
        } else {
            1.0
        };

        let history = self.reduced_freq_history.lock().unwrap();
        let turbulence_intensity = if history.len() >= 5 {
            let mean = history.iter().sum::<f64>() / history.len() as f64;
            let variance = history.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / history.len() as f64;
            variance.sqrt() / mean.max(0.01)
        } else { 0.1 };
        drop(history);

        let is_safe = damping - CONFIDENCE_Z * damping_ci > 0.0 && flutter_margin > 0.1;

        AerodynamicResult {
            bridge_id: self.bridge.bridge_id.clone(),
            wind_speed,
            attack_angle,
            aerodynamic_damping: damping,
            vibration_amplitude: amplitude,
            flutter_critical_speed: critical_speed,
            flutter_margin,
            is_safe,
            timestamp: Utc::now(),
            damping_confidence_interval: (damping - CONFIDENCE_Z * damping_ci, damping + CONFIDENCE_Z * damping_ci),
            amplitude_confidence_interval: (amplitude - CONFIDENCE_Z * amplitude_ci, (amplitude + CONFIDENCE_Z * amplitude_ci).max(0.0)),
            turbulence_intensity,
        }
    }

    pub fn compute_vibration_response(
        &self,
        wind_speed: f64,
        attack_angle: f64,
        duration: f64,
        dt: f64,
    ) -> VibrationResponse {
        let n = (duration / dt) as usize;
        let omega = 2.0 * PI * self.bending_frequency;
        let damping = self.compute_aerodynamic_damping(wind_speed, attack_angle).max(0.001);
        let omega_d = omega * (1.0 - damping.powi(2)).sqrt();
        let amplitude = self.compute_vibration_amplitude(wind_speed, attack_angle);

        let mut time_points = Vec::with_capacity(n);
        let mut displacement = Vec::with_capacity(n);
        let mut velocity = Vec::with_capacity(n);
        let mut acceleration = Vec::with_capacity(n);
        let mut rms_acc = 0.0;

        for i in 0..n {
            let t = i as f64 * dt;
            time_points.push(t);
            let decay = (-damping * omega * t).exp();
            let d = amplitude * decay * (omega_d * t).cos();
            let v = -amplitude * decay
                * (damping * omega * (omega_d * t).cos() + omega_d * (omega_d * t).sin());
            let a = -amplitude * decay
                * omega.powi(2)
                * (1.0 - 2.0 * damping.powi(2))
                * (omega_d * t).cos()
                - 2.0 * amplitude * decay * damping * omega * omega_d * (omega_d * t).sin();
            displacement.push(d);
            velocity.push(v);
            acceleration.push(a);
            rms_acc += a.powi(2);
        }

        rms_acc = (rms_acc / n as f64).sqrt();

        VibrationResponse {
            bridge_id: self.bridge.bridge_id.clone(),
            time_points,
            displacement,
            velocity,
            acceleration,
            frequency: self.bending_frequency,
            damping_ratio: damping,
            rms_acceleration: rms_acc,
        }
    }

    pub fn compute_deck_deformation(
        &self,
        wind_speed: f64,
        attack_angle: f64,
        segments: usize,
    ) -> Vec<(f64, f64, f64)> {
        let mut points = Vec::with_capacity(segments + 1);
        let amplitude = self.compute_vibration_amplitude(wind_speed, attack_angle);
        for i in 0..=segments {
            let x = i as f64 / segments as f64 * self.bridge.span;
            let shape = (PI * x / self.bridge.span).sin();
            let d = amplitude * shape;
            let torsion = 0.005 * amplitude * shape
                * attack_angle.to_radians()
                * wind_speed / self.bridge.design_wind_speed;
            points.push((x, d, torsion));
        }
        points
    }
}
