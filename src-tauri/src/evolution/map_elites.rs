use crate::evolution::genotype::MorphologyGenotype;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct EliteIndividual {
    pub genotype: MorphologyGenotype,
    pub fitness: f32,
    pub features: Vec<f32>,
    pub lineage_id: String,
    pub generation: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedEliteIndividual {
    pub bin_x: i32,
    pub bin_y: i32,
    pub genotype: MorphologyGenotype,
    pub fitness: f32,
    pub features: Vec<f32>,
    pub lineage_id: String,
    pub generation: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedMapElitesArchive {
    pub grid_resolution: f32,
    pub elites: Vec<SavedEliteIndividual>,
}

pub struct MapElitesArchive {
    /// Ordered on purpose. Parent selection walks this collection, and `HashMap` iteration order
    /// varies per process (`RandomState` seeds itself), so a `HashMap` here would leave selection
    /// irreproducible even with a seeded RNG. `BTreeMap` gives a stable order keyed on niche
    /// coordinates; the API surface used elsewhere (`len`/`get`/`insert`/`iter`/`clear`) is identical.
    pub grid: BTreeMap<(i32, i32), EliteIndividual>,
    pub grid_resolution: f32,
}

impl MapElitesArchive {
    pub fn new(grid_resolution: f32) -> Self {
        Self {
            grid: BTreeMap::new(),
            grid_resolution,
        }
    }

    pub fn to_saved(&self) -> SavedMapElitesArchive {
        SavedMapElitesArchive {
            grid_resolution: self.grid_resolution,
            elites: self
                .grid
                .iter()
                .map(|(&(bin_x, bin_y), elite)| SavedEliteIndividual {
                    bin_x,
                    bin_y,
                    genotype: elite.genotype.clone(),
                    fitness: elite.fitness,
                    features: elite.features.clone(),
                    lineage_id: elite.lineage_id.clone(),
                    generation: elite.generation,
                })
                .collect(),
        }
    }

    pub fn from_saved(saved: SavedMapElitesArchive) -> Result<Self, String> {
        if !saved.grid_resolution.is_finite() || saved.grid_resolution <= 0.0 {
            return Err("MAP-Elites grid resolution must be finite and positive".into());
        }

        let mut grid = BTreeMap::new();
        for saved_elite in saved.elites {
            if !saved_elite.fitness.is_finite()
                || saved_elite.features.iter().any(|value| !value.is_finite())
            {
                return Err(format!(
                    "MAP-Elites niche ({}, {}) contains non-finite scientific values",
                    saved_elite.bin_x, saved_elite.bin_y
                ));
            }
            crate::core::components::validate_morphology_payload(&saved_elite.genotype).map_err(
                |error| {
                    format!(
                        "MAP-Elites niche ({}, {}) has invalid morphology: {error}",
                        saved_elite.bin_x, saved_elite.bin_y
                    )
                },
            )?;
            if saved_elite.lineage_id.trim().is_empty()
                || saved_elite.lineage_id.len() > 1_024
                || saved_elite.lineage_id.chars().any(char::is_control)
            {
                return Err(format!(
                    "MAP-Elites niche ({}, {}) has an invalid lineage id",
                    saved_elite.bin_x, saved_elite.bin_y
                ));
            }

            let coords = (saved_elite.bin_x, saved_elite.bin_y);
            let elite = EliteIndividual {
                genotype: saved_elite.genotype,
                fitness: saved_elite.fitness,
                features: saved_elite.features,
                lineage_id: saved_elite.lineage_id,
                generation: saved_elite.generation,
            };
            if grid.insert(coords, elite).is_some() {
                return Err(format!(
                    "MAP-Elites checkpoint contains duplicate niche ({}, {})",
                    coords.0, coords.1
                ));
            }
        }

        Ok(Self {
            grid,
            grid_resolution: saved.grid_resolution,
        })
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

    /// Draws from the caller's stream so selection replays: see [`crate::core::resources::SimRng`].
    pub fn select_parent(
        &self,
        selection_bias: f64,
        rng: &mut impl rand::Rng,
    ) -> Option<&EliteIndividual> {
        if self.grid.is_empty() {
            return None;
        }
        use rand::seq::IteratorRandom;
        if selection_bias <= 1.0 {
            self.grid.values().choose(rng)
        } else {
            let k = (selection_bias.ceil() as usize).max(2);
            let mut best: Option<&EliteIndividual> = None;
            for _ in 0..k {
                if let Some(candidate) = self.grid.values().choose(&mut *rng) {
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
