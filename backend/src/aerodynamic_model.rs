use crate::models::{
    AerodynamicResult, BridgeInfo, DeckAerodynamicShape, DeckShapeType, FlutterDerivatives,
    VibrationResponse,
};
use chrono::{DateTime, Utc};
use std::f64::consts::PI;

const AIR_DENSITY: f64 = 1.225;
const GRAVITY: f64 = 9.81;

pub struct AerodynamicModel {
    pub bridge: BridgeInfo,
    pub flutter_derivatives: FlutterDerivatives,
    pub mass_per_unit_length: f64,
    pub mass_moment_of_inertia: f64,
    pub bending_frequency: f64,
    pub torsional_frequency: f64,
    pub structural_damping: f64,
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
        }
    }

    fn default_flutter_derivatives() -> FlutterDerivatives {
        FlutterDerivatives {
            h_star: [0.0, -0.5, -0.8, -0.6, -0.3, -0.1],
            a_star: [0.0, -1.0, -2.0, -2.5, -2.2, -1.8],
            h_prime: [0.0, 2.0, 4.0, 3.5, 2.5, 1.5],
            a_prime: [0.0, 0.5, 1.0, 1.5, 2.0, 2.5],
        }
    }

    pub fn flutter_derivatives_for_reduced_freq(&self, reduced_freq: f64) -> (f64, f64, f64, f64) {
        let idx = (reduced_freq * 2.0).clamp(0.0, 5.0).round() as usize;
        let idx = idx.min(5);
        (
            self.flutter_derivatives.h_star[idx],
            self.flutter_derivatives.a_star[idx],
            self.flutter_derivatives.h_prime[idx],
            self.flutter_derivatives.a_prime[idx],
        )
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
        if wind_speed <= 1.0 {
            return self.structural_damping;
        }
        let omega = 2.0 * PI * self.bending_frequency;
        let reduced_freq = omega * self.bridge.width / wind_speed;
        let (h_star, _, _, _) = self.flutter_derivatives_for_reduced_freq(reduced_freq);
        let rho_b = AIR_DENSITY * self.bridge.width.powi(2) / (2.0 * self.mass_per_unit_length);
        let aerodynamic_damping = -rho_b * h_star / (2.0 * reduced_freq);
        self.structural_damping + aerodynamic_damping
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
        if wind_speed <= 1.0 {
            return 0.001;
        }
        let omega = 2.0 * PI * self.bending_frequency;
        let damping = self.compute_aerodynamic_damping(wind_speed, attack_angle);
        let damping = damping.max(0.0001);
        let (lift, _, _) = self.compute_quasi_steady_force(wind_speed, attack_angle);
        let max_lift = lift.abs();
        let amplitude = max_lift
            / (self.mass_per_unit_length * omega.powi(2) * 2.0 * damping);
        amplitude.min(2.0)
    }

    pub fn evaluate_aerodynamic_performance(
        &self,
        wind_speed: f64,
        attack_angle: f64,
        shape: Option<&DeckAerodynamicShape>,
    ) -> AerodynamicResult {
        let critical_speed = self.compute_flutter_critical_speed(shape);
        let damping = self.compute_aerodynamic_damping(wind_speed, attack_angle);
        let amplitude = self.compute_vibration_amplitude(wind_speed, attack_angle);
        let flutter_margin = if wind_speed > 0.0 {
            (critical_speed - wind_speed) / critical_speed
        } else {
            1.0
        };
        let is_safe = damping > 0.0 && flutter_margin > 0.1;

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
