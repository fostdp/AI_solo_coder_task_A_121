use crate::aerodynamic_model::AerodynamicModel;
use crate::models::{
    BridgeInfo, DeckAerodynamicShape, DeckShapeType, OptimizationConfig, OptimizationResult,
};
use chrono::Utc;
use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::f64::consts::PI;

pub struct GeneticOptimizer<'a> {
    model: &'a AerodynamicModel,
    config: OptimizationConfig,
    rng: rand::rngs::StdRng,
}

impl<'a> GeneticOptimizer<'a> {
    pub fn new(model: &'a AerodynamicModel, config: OptimizationConfig) -> Self {
        use rand::SeedableRng;
        GeneticOptimizer {
            model,
            config,
            rng: rand::rngs::StdRng::seed_from_u64(42),
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

    fn fitness(&self, shape: &DeckAerodynamicShape) -> f64 {
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

        let base_critical = self.model.compute_flutter_critical_speed(None);
        let speed_improvement = (critical_speed - base_critical) / base_critical;

        1.0 - flutter_prob - avg_amplitude * 0.5 + speed_improvement * 0.3
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

        let mut population: Vec<(DeckAerodynamicShape, f64)> = (0..pop_size)
            .map(|_| {
                let s = self.random_shape();
                let f = self.fitness(&s);
                (s, f)
            })
            .collect();

        let mut generation_history = Vec::with_capacity(generations);

        for gen in 0..generations {
            population.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let best_fitness = population[0].1;
            generation_history.push(best_fitness);

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
                let fitness = self.fitness(&child);
                new_population.push((child, fitness));
            }

            population = new_population;
        }

        population.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let best_shape = population[0].0.clone();
        let best_fitness = population[0].1;

        let base_critical = self.model.compute_flutter_critical_speed(None);
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
