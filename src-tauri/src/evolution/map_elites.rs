use crate::evolution::genotype::MorphologyGenotype;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct EliteIndividual {
    pub genotype: MorphologyGenotype,
    pub fitness: f32,
    pub features: Vec<f32>,
    pub lineage_id: String,
    pub generation: u32,
}

pub struct MapElitesArchive {
    pub grid: HashMap<(i32, i32), EliteIndividual>,
    pub grid_resolution: f32,
}

impl MapElitesArchive {
    pub fn new(grid_resolution: f32) -> Self {
        Self {
            grid: HashMap::new(),
            grid_resolution,
        }
    }

    // Chuyển đổi feature vector thành tọa độ ô lưới (Niche coordination)
    pub fn get_bin_coords(&self, features: &[f32]) -> (i32, i32) {
        let f0 = features.first().cloned().unwrap_or(0.0);
        let f1 = features.get(1).cloned().unwrap_or(0.0);
        (
            (f0 / self.grid_resolution).floor() as i32,
            (f1 / self.grid_resolution).floor() as i32,
        )
    }

    // Cập nhật cá thể ưu tú vào ô lưới nếu có fitness cao hơn
    pub fn add_individual(&mut self, individual: EliteIndividual) -> bool {
        let coords = self.get_bin_coords(&individual.features);
        if let Some(existing) = self.grid.get(&coords) {
            if individual.fitness > existing.fitness {
                self.grid.insert(coords, individual);
                true
            } else {
                false
            }
        } else {
            self.grid.insert(coords, individual);
            true
        }
    }

    pub fn select_parent(&self, selection_bias: f64) -> Option<&EliteIndividual> {
        if self.grid.is_empty() {
            return None;
        }
        use rand::seq::IteratorRandom;
        let mut rng = rand::thread_rng();
        if selection_bias <= 1.0 {
            self.grid.values().choose(&mut rng)
        } else {
            let k = (selection_bias.ceil() as usize).max(2);
            let mut best: Option<&EliteIndividual> = None;
            for _ in 0..k {
                if let Some(candidate) = self.grid.values().choose(&mut rng) {
                    match best {
                        Some(b) => {
                            if candidate.fitness > b.fitness {
                                best = Some(candidate);
                            }
                        }
                        None => {
                            best = Some(candidate);
                        }
                    }
                }
            }
            best
        }
    }
}
