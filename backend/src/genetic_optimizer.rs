use crate::aerodynamic_model::AerodynamicModel;
use crate::models::{
    DeckAerodynamicShape, DeckShapeType, OptimizationConfig, OptimizationResult,
};
use chrono::Utc;
use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::f64::consts::PI;

struct KrigingSurrogate {
    samples_x: Vec<Vec<f64>>,
    samples_y: Vec<f64>,
    kernel_sigma: f64,
    kernel_l: f64,
    noise: f64,
    mean: f64,
    std: f64,
    k_inv: Option<Vec<Vec<f64>>>,
    precomputed_base_critical: f64,
}

impl KrigingSurrogate {
    fn new(precomputed_base_critical: f64) -> Self {
        KrigingSurrogate {
            samples_x: Vec::new(),
            samples_y: Vec::new(),
            kernel_sigma: 1.0,
            kernel_l: 0.3,
            noise: 0.01,
            mean: 0.0,
            std: 1.0,
            k_inv: None,
            precomputed_base_critical,
        }
    }

    fn rbf_kernel(&self, x1: &[f64], x2: &[f64]) -> f64 {
        let dist2 = x1.iter().zip(x2.iter()).map(|(a, b)| (a - b).powi(2)).sum::<f64>();
        self.kernel_sigma * (-dist2 / (2.0 * self.kernel_l.powi(2))).exp()
    }

    fn train(&mut self) {
        if self.samples_x.is_empty() { return; }
        let n = self.samples_x.len();
        self.mean = self.samples_y.iter().sum::<f64>() / n as f64;
        let variance = self.samples_y.iter().map(|y| (y - self.mean).powi(2)).sum::<f64>() / n as f64;
        self.std = variance.sqrt().max(0.001);

        let mut k = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                k[i][j] = self.rbf_kernel(&self.samples_x[i], &self.samples_x[j]);
                if i == j { k[i][j] += self.noise; }
            }
        }
        self.k_inv = Some(Self::invert_matrix(&k));
    }

    fn invert_matrix(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let n = a.len();
        let mut aug = vec![vec![0.0; 2 * n]; n];
        for i in 0..n {
            for j in 0..n { aug[i][j] = a[i][j]; }
            aug[i][i + n] = 1.0;
        }
        for col in 0..n {
            let mut max_row = col;
            for row in col..n { if aug[row][col].abs() > aug[max_row][col].abs() { max_row = row; } }
            aug.swap(col, max_row);
            let pivot = aug[col][col];
            for j in 0..2*n { aug[col][j] /= pivot; }
            for row in 0..n {
                if row != col {
                    let factor = aug[row][col];
                    for j in 0..2*n { aug[row][j] -= factor * aug[col][j]; }
                }
            }
        }
        let mut inv = vec![vec![0.0; n]; n];
        for i in 0..n { for j in 0..n { inv[i][j] = aug[i][j + n]; } }
        inv
    }

    fn predict(&self, x: &[f64]) -> (f64, f64) {
        if self.samples_x.is_empty() || self.k_inv.is_none() { return (0.5, 0.2); }
        let k_inv = self.k_inv.as_ref().unwrap();
        let n = self.samples_x.len();
        let mut k_vec = vec![0.0; n];
        for i in 0..n { k_vec[i] = self.rbf_kernel(x, &self.samples_x[i]); }
        let y_norm: Vec<f64> = self.samples_y.iter().map(|y| (y - self.mean) / self.std).collect();

        let mut mu = 0.0;
        for i in 0..n { for j in 0..n { mu += k_vec[i] * k_inv[i][j] * y_norm[j]; } }
        mu = mu * self.std + self.mean;

        let mut s2 = self.rbf_kernel(x, x) + self.noise;
        for i in 0..n { for j in 0..n { s2 -= k_vec[i] * k_inv[i][j] * k_vec[j]; } }
        let sigma = s2.sqrt().max(0.001);

        (mu, sigma)
    }

    fn expected_improvement(&self, x: &[f64], y_best: f64) -> f64 {
        let (mu, sigma) = self.predict(x);
        let z = (mu - y_best) / sigma;
        let norm_cdf = 0.5 * (1.0 + approx_erf(z / 2.0_f64.sqrt()));
        let norm_pdf = (-z.powi(2) / 2.0).exp() / (2.0 * PI).sqrt();
        (mu - y_best) * norm_cdf + sigma * norm_pdf
    }

    fn add_sample(&mut self, x: Vec<f64>, y: f64) {
        self.samples_x.push(x);
        self.samples_y.push(y);
    }
}

pub struct GeneticOptimizer<'a> {
    model: &'a AerodynamicModel,
    config: OptimizationConfig,
    rng: rand::rngs::StdRng,
    surrogate: KrigingSurrogate,
    real_eval_count: usize,
    surrogate_eval_count: usize,
}

fn approx_erf(x: f64) -> f64 {
    const A1: f64 = 0.254829592;
    const A2: f64 = -0.284496736;
    const A3: f64 = 1.421413741;
    const A4: f64 = -1.453152027;
    const A5: f64 = 1.061405429;
    const P: f64 = 0.3275911;
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + P * x);
    let y = 1.0 - (((((A5 * t + A4) * t) + A3) * t + A2) * t + A1) * t * (-x * x).exp();
    sign * y
}

fn shape_to_vec(shape: &DeckAerodynamicShape) -> Vec<f64> {
    let type_val = match shape.deck_shape_type {
        DeckShapeType::Flat => 0.0,
        DeckShapeType::Streamlined => 1.0,
        DeckShapeType::Box => 2.0,
        DeckShapeType::Slotted => 3.0,
    };
    vec![
        shape.wind_nose_angle / 45.0,
        shape.stabilizer_plate_height / 1.5,
        shape.stabilizer_plate_count as f64 / 4.0,
        type_val / 3.0,
        shape.fairing_length / 1.0,
        shape.porosity / 0.5,
    ]
}

impl<'a> GeneticOptimizer<'a> {
    pub fn new(model: &'a AerodynamicModel, config: OptimizationConfig) -> Self {
        use rand::SeedableRng;
        let base_critical = model.compute_flutter_critical_speed(None);
        GeneticOptimizer {
            model,
            config,
            rng: rand::rngs::StdRng::seed_from_u64(42),
            surrogate: KrigingSurrogate::new(base_critical),
            real_eval_count: 0,
            surrogate_eval_count: 0,
        }
    }

    fn random_shape(&mut self) -> DeckAerodynamicShape {
        let types = [
            DeckShapeType::Flat,
            DeckShapeType::Streamlined,
            DeckShapeType::Box,
            DeckShapeType::Slotted,
        ];
        DeckAerodynamicShape {
            wind_nose_angle: self.rng.gen_range(0.0..45.0),
            stabilizer_plate_height: self.rng.gen_range(0.0..1.5),
            stabilizer_plate_count: self.rng.gen_range(0..5),
            deck_shape_type: types[self.rng.gen_range(0..4)],
            fairing_length: self.rng.gen_range(0.0..1.0),
            porosity: self.rng.gen_range(0.0..0.5),
        }
    }

    fn lhs_sample(&mut self, n: usize) -> Vec<DeckAerodynamicShape> {
        let types = [
            DeckShapeType::Flat,
            DeckShapeType::Streamlined,
            DeckShapeType::Box,
            DeckShapeType::Slotted,
        ];
        let mut samples = Vec::with_capacity(n);
        for _dim in 0..6 {
            let mut perm: Vec<usize> = (0..n).collect();
            for i in (1..n).rev() {
                let j = self.rng.gen_range(0..=i);
                perm.swap(i, j);
            }
        }
        for i in 0..n {
            let t = (i as f64 + self.rng.gen::<f64>()) / n as f64;
            let t2 = (i as f64 + self.rng.gen::<f64>()) / n as f64;
            let t3 = (i as f64 + self.rng.gen::<f64>()) / n as f64;
            let t4 = (i as f64 + self.rng.gen::<f64>()) / n as f64;
            let t5 = (i as f64 + self.rng.gen::<f64>()) / n as f64;
            let t6 = (i as f64 + self.rng.gen::<f64>()) / n as f64;
            samples.push(DeckAerodynamicShape {
                wind_nose_angle: t * 45.0,
                stabilizer_plate_height: t2 * 1.5,
                stabilizer_plate_count: (t3 * 4.0).round() as usize,
                deck_shape_type: types[(t4 * 3.0).round() as usize],
                fairing_length: t5 * 1.0,
                porosity: t6 * 0.5,
            });
        }
        samples
    }

    fn fitness_real(&mut self, shape: &DeckAerodynamicShape) -> f64 {
        self.real_eval_count += 1;
        let critical_speed = self.model.compute_flutter_critical_speed(Some(shape));
        let (wind_min, wind_max) = self.config.wind_speed_range;
        let (angle_min, angle_max) = self.config.attack_angle_range;
        let wind_steps = 10;
        let angle_steps = 5;
        let mut flutter_prob = 0.0;
        let mut total_amplitude = 0.0;

        for i in 0..=wind_steps {
            let wind = wind_min + (wind_max - wind_min) * i as f64 / wind_steps as f64;
            for j in 0..=angle_steps {
                let angle = angle_min + (angle_max - angle_min) * j as f64 / angle_steps as f64;
                let result = self
                    .model
                    .evaluate_aerodynamic_performance(wind, angle, Some(shape));
                if !result.is_safe {
                    flutter_prob += 1.0;
                }
                total_amplitude += result.vibration_amplitude;
            }
        }
        let total = (wind_steps + 1) * (angle_steps + 1);
        flutter_prob /= total as f64;
        let avg_amplitude = total_amplitude / total as f64;

        let speed_improvement = (critical_speed - self.surrogate.precomputed_base_critical)
            / self.surrogate.precomputed_base_critical;

        1.0 - flutter_prob - avg_amplitude * 0.5 + speed_improvement * 0.3
    }

    fn fitness_surrogate(&mut self, shape: &DeckAerodynamicShape) -> f64 {
        self.surrogate_eval_count += 1;
        let x = shape_to_vec(shape);
        let (mu, _) = self.surrogate.predict(&x);
        mu
    }

    fn fitness(&mut self, shape: &DeckAerodynamicShape, use_surrogate: bool) -> f64 {
        if use_surrogate { self.fitness_surrogate(shape) } else { self.fitness_real(shape) }
    }

    fn crossover(&mut self, p1: &DeckAerodynamicShape, p2: &DeckAerodynamicShape) -> DeckAerodynamicShape {
        let t1 = p1.clone();
        let t2 = p2.clone();
        DeckAerodynamicShape {
            wind_nose_angle: if self.rng.gen_bool(0.5) { t1.wind_nose_angle } else { t2.wind_nose_angle },
            stabilizer_plate_height: if self.rng.gen_bool(0.5) { t1.stabilizer_plate_height } else { t2.stabilizer_plate_height },
            stabilizer_plate_count: if self.rng.gen_bool(0.5) { t1.stabilizer_plate_count } else { t2.stabilizer_plate_count },
            deck_shape_type: if self.rng.gen_bool(0.5) { t1.deck_shape_type } else { t2.deck_shape_type },
            fairing_length: if self.rng.gen_bool(0.5) { t1.fairing_length } else { t2.fairing_length },
            porosity: if self.rng.gen_bool(0.5) { t1.porosity } else { t2.porosity },
        }
    }

    fn mutate(&mut self, shape: &DeckAerodynamicShape) -> DeckAerodynamicShape {
        let mut s = shape.clone();
        let normal = Normal::new(0.0, 1.0).unwrap();

        if self.rng.gen_bool(self.config.mutation_rate) {
            s.wind_nose_angle = (s.wind_nose_angle + normal.sample(&mut self.rng) * 5.0).clamp(0.0, 45.0);
        }
        if self.rng.gen_bool(self.config.mutation_rate) {
            s.stabilizer_plate_height = (s.stabilizer_plate_height + normal.sample(&mut self.rng) * 0.15).clamp(0.0, 1.5);
        }
        if self.rng.gen_bool(self.config.mutation_rate) {
            let delta = normal.sample(&mut self.rng).round() as i32;
            s.stabilizer_plate_count = ((s.stabilizer_plate_count as i32 + delta).max(0).min(4)) as usize;
        }
        if self.rng.gen_bool(self.config.mutation_rate) {
            let types = [DeckShapeType::Flat, DeckShapeType::Streamlined, DeckShapeType::Box, DeckShapeType::Slotted];
            s.deck_shape_type = types[self.rng.gen_range(0..4)];
        }
        if self.rng.gen_bool(self.config.mutation_rate) {
            s.fairing_length = (s.fairing_length + normal.sample(&mut self.rng) * 0.1).clamp(0.0, 1.0);
        }
        if self.rng.gen_bool(self.config.mutation_rate) {
            s.porosity = (s.porosity + normal.sample(&mut self.rng) * 0.05).clamp(0.0, 0.5);
        }
        s
    }

    fn tournament_selection(&mut self, population: &[(DeckAerodynamicShape, f64)], k: usize) -> usize {
        let mut best = self.rng.gen_range(0..population.len());
        for _ in 1..k {
            let idx = self.rng.gen_range(0..population.len());
            if population[idx].1 > population[best].1 {
                best = idx;
            }
        }
        best
    }

    pub fn run(mut self) -> OptimizationResult {
        let pop_size = self.config.population_size;
        let generations = self.config.generations;
        let _base_critical = self.surrogate.precomputed_base_critical;

        println!("[GA] 初始化克里金代理模型...");
        let initial_samples = self.lhs_sample(25);
        let mut y_best = f64::NEG_INFINITY;
        for s in &initial_samples {
            let f = self.fitness_real(s);
            y_best = y_best.max(f);
            self.surrogate.add_sample(shape_to_vec(s), f);
        }
        self.surrogate.train();
        println!("[GA] 初始采样完成: 25 个真实评估, 训练代理模型");

        let mut population: Vec<(DeckAerodynamicShape, f64)> = (0..pop_size)
            .map(|_| {
                let s = self.random_shape();
                let f = self.fitness(&s, true);
                (s, f)
            })
            .collect();

        let mut generation_history = Vec::with_capacity(generations);

        for gen in 0..generations {
            population.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let best_fitness = population[0].1;
            generation_history.push(best_fitness);

            if gen % 5 == 0 && gen > 0 {
                self.surrogate.train();
                println!("[GA] 第 {} 代: 重新训练代理模型 (样本数={})", gen, self.surrogate.samples_x.len());
            }

            let mut new_population = Vec::with_capacity(pop_size);
            new_population.push(population[0].clone());
            new_population.push(population[1].clone());

            while new_population.len() < pop_size {
                let p1_idx = self.tournament_selection(&population, 3);
                let p2_idx = self.tournament_selection(&population, 3);
                let p1 = &population[p1_idx].0;
                let p2 = &population[p2_idx].0;

                let child = if self.rng.gen_bool(self.config.crossover_rate) {
                    self.crossover(p1, p2)
                } else if self.rng.gen_bool(0.5) {
                    p1.clone()
                } else {
                    p2.clone()
                };
                let child = self.mutate(&child);

                let x = shape_to_vec(&child);
                let ei = self.surrogate.expected_improvement(&x, y_best);
                let use_surrogate = ei < 0.01 && self.rng.gen_bool(0.85);
                let fitness = self.fitness(&child, use_surrogate);

                if !use_surrogate {
                    y_best = y_best.max(fitness);
                    self.surrogate.add_sample(x, fitness);
                }

                new_population.push((child, fitness));
            }

            population = new_population;
        }

        println!("[GA] 优化完成: 真实评估={}, 代理评估={}, 加速比≈{:.1}x",
            self.real_eval_count, self.surrogate_eval_count,
            (self.real_eval_count + self.surrogate_eval_count) as f64 / self.real_eval_count.max(1) as f64);

        population.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let best_shape = population[0].0.clone();
        let best_fitness = self.fitness_real(&best_shape);
        let improved_critical = self.model.compute_flutter_critical_speed(Some(&best_shape));

        let (wind_min, wind_max) = self.config.wind_speed_range;
        let (angle_min, angle_max) = self.config.attack_angle_range;
        let wind_steps = 20;
        let angle_steps = 10;
        let mut base_prob = 0.0;
        let mut opt_prob = 0.0;
        let total = (wind_steps + 1) * (angle_steps + 1);

        for i in 0..=wind_steps {
            let wind = wind_min + (wind_max - wind_min) * i as f64 / wind_steps as f64;
            for j in 0..=angle_steps {
                let angle = angle_min + (angle_max - angle_min) * j as f64 / angle_steps as f64;
                let base = self.model.evaluate_aerodynamic_performance(wind, angle, None);
                let opt = self.model.evaluate_aerodynamic_performance(wind, angle, Some(&best_shape));
                if !base.is_safe {
                    base_prob += 1.0;
                }
                if !opt.is_safe {
                    opt_prob += 1.0;
                }
            }
        }
        base_prob /= total as f64;
        opt_prob /= total as f64;
        let flutter_prob_reduction = if base_prob > 0.0 {
            (base_prob - opt_prob) / base_prob
        } else {
            0.0
        };

        OptimizationResult {
            bridge_id: self.config.bridge_id.clone(),
            best_shape,
            best_fitness,
            improved_critical_speed: improved_critical,
            flutter_probability_reduction: flutter_prob_reduction,
            generation_history,
            completed_at: Utc::now(),
        }
    }
}
