//! Multidimensional Parameter Sweeps (Feature 5).
//!
//! Provides high-throughput parameter space exploration across multi-dimensional domains
//! supporting Grid Search (Cartesian product), Sobol low-discrepancy quasi-random sequences,
//! and Latin Hypercube Sampling (LHS).
//!
//! Enables parallel batch execution via `rayon` while maintaining bit-exact determinism
//! and closed-loop conservation laws.

use std::collections::HashMap;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::core::experiment::{
    ExperimentError, ExperimentManifest, ObservableRegistry,
};
use crate::core::experiment_runner::{
    run_ensemble, ExperimentModel, MetricSummary,
};

pub const SWEEP_SCHEMA_VERSION: u16 = 1;
pub const MAX_SWEEP_POINTS: usize = 100_000;
pub const MAX_SWEEP_DIMENSIONS: usize = 32;

/// Specification for a single dimension in parameter space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterDim {
    pub name: String,
    pub target: SweepTarget,
    pub spec: DimensionSpec,
}

impl ParameterDim {
    pub fn validate(&self) -> Result<(), ExperimentError> {
        if self.name.trim().is_empty() {
            return Err(ExperimentError::EmptyField {
                field: "parameter dimension name".into(),
            });
        }
        self.spec.validate(&self.name)
    }
}

/// Target field within the ExperimentManifest to update.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "key")]
pub enum SweepTarget {
    /// Overwrites or inserts a named initial condition in `initial_conditions.values`.
    InitialCondition(String),
    /// Updates a numeric field of the exotic energy law (e.g. "source_rate", "decay_rate", "initial_amount").
    ExoticLawField(String),
    /// Updates the intensity of a specific intervention command by id.
    InterventionIntensity(u32),
    /// Updates the start tick of an intervention command by id.
    InterventionStartTick(u32),
    /// Updates the duration of an intervention command by id.
    InterventionDuration(u32),
}

/// Mathematical domain and discretization specification for a dimension.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scale")]
pub enum DimensionSpec {
    Continuous {
        min: f64,
        max: f64,
    },
    Logarithmic {
        min: f64,
        max: f64,
    },
    IntegerRange {
        min: i64,
        max: i64,
        #[serde(default = "default_step_one")]
        step: i64,
    },
    Discrete {
        values: Vec<f64>,
    },
}

fn default_step_one() -> i64 {
    1
}

impl DimensionSpec {
    pub fn validate(&self, name: &str) -> Result<(), ExperimentError> {
        match self {
            DimensionSpec::Continuous { min, max } => {
                if !min.is_finite() || !max.is_finite() {
                    return Err(ExperimentError::NotFinite {
                        field: format!("parameter '{name}' bounds"),
                    });
                }
                if min > max {
                    return Err(ExperimentError::OutOfRange {
                        field: format!("parameter '{name}' min"),
                        value: *min,
                        min: f64::NEG_INFINITY,
                        max: *max,
                    });
                }
            }
            DimensionSpec::Logarithmic { min, max } => {
                if !min.is_finite() || !max.is_finite() {
                    return Err(ExperimentError::NotFinite {
                        field: format!("parameter '{name}' bounds"),
                    });
                }
                if *min <= 0.0 {
                    return Err(ExperimentError::OutOfRange {
                        field: format!("parameter '{name}' logarithmic min must be positive"),
                        value: *min,
                        min: f64::MIN_POSITIVE,
                        max: f64::INFINITY,
                    });
                }
                if min > max {
                    return Err(ExperimentError::OutOfRange {
                        field: format!("parameter '{name}' min"),
                        value: *min,
                        min: f64::MIN_POSITIVE,
                        max: *max,
                    });
                }
            }
            DimensionSpec::IntegerRange { min, max, step } => {
                if *step <= 0 {
                    return Err(ExperimentError::OutOfRange {
                        field: format!("parameter '{name}' integer step"),
                        value: *step as f64,
                        min: 1.0,
                        max: f64::INFINITY,
                    });
                }
                if min > max {
                    return Err(ExperimentError::OutOfRange {
                        field: format!("parameter '{name}' min"),
                        value: *min as f64,
                        min: i64::MIN as f64,
                        max: *max as f64,
                    });
                }
            }
            DimensionSpec::Discrete { values } => {
                if values.is_empty() {
                    return Err(ExperimentError::EmptyField {
                        field: format!("parameter '{name}' discrete values"),
                    });
                }
                for (idx, &v) in values.iter().enumerate() {
                    if !v.is_finite() {
                        return Err(ExperimentError::NotFinite {
                            field: format!("parameter '{name}' discrete value [{idx}]"),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Map a normalized unit coordinate $u \in [0, 1)$ into the parameter domain.
    pub fn map_normalized(&self, u: f64) -> f64 {
        let u = u.clamp(0.0, 1.0 - 1e-15);
        match self {
            DimensionSpec::Continuous { min, max } => min + u * (max - min),
            DimensionSpec::Logarithmic { min, max } => min * (max / min).powf(u),
            DimensionSpec::IntegerRange { min, max, step } => {
                let count = (((max - min) / step).max(0) as usize) + 1;
                let idx = ((u * count as f64).floor() as usize).min(count - 1);
                (*min + (idx as i64) * step) as f64
            }
            DimensionSpec::Discrete { values } => {
                let idx = ((u * values.len() as f64).floor() as usize).min(values.len() - 1);
                values[idx]
            }
        }
    }

    /// Generate discretized 1D grid values for Grid search.
    pub fn grid_values(&self, default_points_per_dim: usize) -> Vec<f64> {
        match self {
            DimensionSpec::Continuous { min, max } => {
                if default_points_per_dim <= 1 || (min - max).abs() < 1e-15 {
                    vec![*min]
                } else {
                    (0..default_points_per_dim)
                        .map(|i| min + (i as f64) * (max - min) / ((default_points_per_dim - 1) as f64))
                        .collect()
                }
            }
            DimensionSpec::Logarithmic { min, max } => {
                if default_points_per_dim <= 1 || (min - max).abs() < 1e-15 {
                    vec![*min]
                } else {
                    let ratio = max / min;
                    (0..default_points_per_dim)
                        .map(|i| min * ratio.powf((i as f64) / ((default_points_per_dim - 1) as f64)))
                        .collect()
                }
            }
            DimensionSpec::IntegerRange { min, max, step } => {
                let mut vals = Vec::new();
                let mut curr = *min;
                while curr <= *max {
                    vals.push(curr as f64);
                    match curr.checked_add(*step) {
                        Some(next) => curr = next,
                        None => break,
                    }
                }
                vals
            }
            DimensionSpec::Discrete { values } => values.clone(),
        }
    }
}

/// Sampling strategy configuration for parameter exploration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy")]
pub enum SamplingStrategy {
    Grid {
        /// Number of points per continuous/logarithmic dimension (default: 5)
        points_per_dim: Option<usize>,
    },
    Sobol {
        num_samples: usize,
        #[serde(default)]
        offset: usize,
    },
    LatinHypercube {
        num_samples: usize,
        #[serde(default)]
        seed: Option<u64>,
        #[serde(default)]
        centered: bool,
    },
}

/// The declarative manifest defining a multidimensional sweep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SweepManifest {
    pub schema_version: u16,
    pub sweep_id: String,
    pub name: String,
    pub base_manifest: ExperimentManifest,
    pub parameters: Vec<ParameterDim>,
    pub sampling: SamplingStrategy,
    #[serde(default)]
    pub max_threads: Option<usize>,
}

impl SweepManifest {
    pub fn validate(&self, registry: &ObservableRegistry) -> Result<(), ExperimentError> {
        if self.schema_version != SWEEP_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedSchemaVersion {
                component: "sweep_manifest".into(),
                found: self.schema_version,
                supported: SWEEP_SCHEMA_VERSION,
            });
        }
        if self.sweep_id.trim().is_empty() {
            return Err(ExperimentError::EmptyField {
                field: "sweep_id".into(),
            });
        }
        if self.parameters.is_empty() {
            return Err(ExperimentError::EmptyField {
                field: "sweep parameters".into(),
            });
        }
        if self.parameters.len() > MAX_SWEEP_DIMENSIONS {
            return Err(ExperimentError::ResourceLimit {
                field: "sweep parameters (dimensions)".into(),
                limit: MAX_SWEEP_DIMENSIONS,
                found: self.parameters.len(),
            });
        }
        for (i, p) in self.parameters.iter().enumerate() {
            p.validate()?;
            for other in &self.parameters[i + 1..] {
                if p.name == other.name {
                    return Err(ExperimentError::DuplicateId {
                        context: "parameter dimension".into(),
                        id: p.name.clone(),
                    });
                }
            }
        }
        self.base_manifest.validate(registry)?;
        Ok(())
    }

    /// Generate the full $N \times D$ parameter matrix for this sweep.
    pub fn generate_sample_matrix(&self) -> Result<(Vec<String>, Vec<Vec<f64>>), ExperimentError> {
        let param_names: Vec<String> = self.parameters.iter().map(|p| p.name.clone()).collect();
        let num_dims = self.parameters.len();

        let matrix = match &self.sampling {
            SamplingStrategy::Grid { points_per_dim } => {
                let default_ppd = points_per_dim.unwrap_or(5).max(1);
                let dim_values: Vec<Vec<f64>> = self
                    .parameters
                    .iter()
                    .map(|p| p.spec.grid_values(default_ppd))
                    .collect();

                let mut total_points = 1usize;
                for vals in &dim_values {
                    total_points = total_points.saturating_mul(vals.len());
                }
                if total_points > MAX_SWEEP_POINTS {
                    return Err(ExperimentError::ResourceLimit {
                        field: "sweep grid total points".into(),
                        limit: MAX_SWEEP_POINTS,
                        found: total_points,
                    });
                }

                // Cartesian product
                let mut rows: Vec<Vec<f64>> = vec![Vec::with_capacity(num_dims)];
                for vals in &dim_values {
                    let mut next_rows = Vec::with_capacity(rows.len() * vals.len());
                    for row in &rows {
                        for &v in vals {
                            let mut new_row = row.clone();
                            new_row.push(v);
                            next_rows.push(new_row);
                        }
                    }
                    rows = next_rows;
                }
                rows
            }
            SamplingStrategy::Sobol {
                num_samples,
                offset,
            } => {
                if *num_samples == 0 {
                    return Err(ExperimentError::OutOfRange {
                        field: "sobol num_samples".into(),
                        value: 0.0,
                        min: 1.0,
                        max: MAX_SWEEP_POINTS as f64,
                    });
                }
                if *num_samples > MAX_SWEEP_POINTS {
                    return Err(ExperimentError::ResourceLimit {
                        field: "sobol num_samples".into(),
                        limit: MAX_SWEEP_POINTS,
                        found: *num_samples,
                    });
                }

                let mut sobol = SobolSequence::new(num_dims);
                // Advance offset
                for _ in 0..*offset {
                    sobol.next_point();
                }

                let mut rows = Vec::with_capacity(*num_samples);
                for _ in 0..*num_samples {
                    let u_vec = sobol.next_point();
                    let row: Vec<f64> = u_vec
                        .iter()
                        .enumerate()
                        .map(|(d, &u)| self.parameters[d].spec.map_normalized(u))
                        .collect();
                    rows.push(row);
                }
                rows
            }
            SamplingStrategy::LatinHypercube {
                num_samples,
                seed,
                centered,
            } => {
                if *num_samples == 0 {
                    return Err(ExperimentError::OutOfRange {
                        field: "lhs num_samples".into(),
                        value: 0.0,
                        min: 1.0,
                        max: MAX_SWEEP_POINTS as f64,
                    });
                }
                if *num_samples > MAX_SWEEP_POINTS {
                    return Err(ExperimentError::ResourceLimit {
                        field: "lhs num_samples".into(),
                        limit: MAX_SWEEP_POINTS,
                        found: *num_samples,
                    });
                }

                let seed_val = seed.unwrap_or(0x4C48535F53454544); // "LHS_SEED"
                let mut rng = StdRng::seed_from_u64(seed_val);
                let n = *num_samples;

                let mut dim_strata: Vec<Vec<f64>> = Vec::with_capacity(num_dims);
                for _ in 0..num_dims {
                    let mut strata = Vec::with_capacity(n);
                    for j in 0..n {
                        let offset: f64 = if *centered { 0.5 } else { rng.gen::<f64>() };
                        let u = ((j as f64) + offset) / (n as f64);
                        strata.push(u);
                    }
                    // Fisher-Yates shuffle
                    for i in (1..n).rev() {
                        let j = rng.gen_range(0..=i);
                        strata.swap(i, j);
                    }
                    dim_strata.push(strata);
                }

                let mut rows = Vec::with_capacity(n);
                for sample_idx in 0..n {
                    let mut row = Vec::with_capacity(num_dims);
                    for d in 0..num_dims {
                        let u = dim_strata[d][sample_idx];
                        row.push(self.parameters[d].spec.map_normalized(u));
                    }
                    rows.push(row);
                }
                rows
            }
        };

        Ok((param_names, matrix))
    }

    /// Apply a parameter vector $P_k$ to the base manifest to produce a concrete point manifest.
    pub fn apply_point(
        &self,
        point_idx: usize,
        values: &[f64],
    ) -> Result<ExperimentManifest, ExperimentError> {
        let mut manifest = self.base_manifest.clone();
        manifest.experiment_id = format!("{}_pt{:04}", self.base_manifest.experiment_id, point_idx);

        for (param, &val) in self.parameters.iter().zip(values.iter()) {
            match &param.target {
                SweepTarget::InitialCondition(key) => {
                    if let Some(pos) = manifest.initial_conditions.values.iter().position(|(k, _)| k == key) {
                        manifest.initial_conditions.values[pos].1 = val;
                    } else {
                        manifest.initial_conditions.values.push((key.clone(), val));
                    }
                }
                SweepTarget::ExoticLawField(field) => {
                    let law = manifest.laws.exotic_energy.as_mut().ok_or_else(|| {
                        ExperimentError::InvalidLaw {
                            reason: format!(
                                "sweep target refers to exotic law field '{field}', but laws.exotic_energy is None"
                            ),
                        }
                    })?;
                    match field.as_str() {
                        "source_rate" => law.source_rate = val as f32,
                        "decay_rate" => law.decay_rate = val as f32,
                        "diffusion_rate" => law.diffusion_rate = val as f32,
                        "initial_amount" => law.initial_amount = val,
                        "max_density" => law.max_density = val as f32,
                        other => {
                            return Err(ExperimentError::InvalidLaw {
                                reason: format!("unknown exotic law field target '{other}'"),
                            })
                        }
                    }
                }
                SweepTarget::InterventionIntensity(id) => {
                    let cmd = manifest
                        .interventions
                        .iter_mut()
                        .find(|c| c.id == *id)
                        .ok_or_else(|| ExperimentError::InapplicableIntervention {
                            id: *id,
                            reason: format!("target intervention id {id} not found in base manifest"),
                        })?;
                    cmd.intensity = val as f32;
                }
                SweepTarget::InterventionStartTick(id) => {
                    let cmd = manifest
                        .interventions
                        .iter_mut()
                        .find(|c| c.id == *id)
                        .ok_or_else(|| ExperimentError::InapplicableIntervention {
                            id: *id,
                            reason: format!("target intervention id {id} not found in base manifest"),
                        })?;
                    cmd.start_tick = val.max(0.0) as u64;
                }
                SweepTarget::InterventionDuration(id) => {
                    let cmd = manifest
                        .interventions
                        .iter_mut()
                        .find(|c| c.id == *id)
                        .ok_or_else(|| ExperimentError::InapplicableIntervention {
                            id: *id,
                            reason: format!("target intervention id {id} not found in base manifest"),
                        })?;
                    cmd.duration_ticks = val.max(0.0) as u64;
                }
            }
        }

        Ok(manifest)
    }
}

// -----------------------------------------------------------------------------
// Pure Rust 32-Dimensional Sobol Sequence Generator (Joe & Kuo Direction Numbers)
// -----------------------------------------------------------------------------

/// Direction numbers table for Sobol sequences in up to 32 dimensions.
pub struct SobolSequence {
    dimensions: usize,
    direction_numbers: Vec<[u32; 32]>,
    x: Vec<u32>,
    index: u32,
}

impl SobolSequence {
    pub fn new(dimensions: usize) -> Self {
        let dimensions = dimensions.clamp(1, MAX_SWEEP_DIMENSIONS);
        let direction_numbers = generate_sobol_direction_numbers(dimensions);
        Self {
            dimensions,
            direction_numbers,
            x: vec![0u32; dimensions],
            index: 0,
        }
    }

    /// Advance the sequence and return normalized quasi-random vector $u \in [0, 1)^D$.
    pub fn next_point(&mut self) -> Vec<f64> {
        let c = self.index.trailing_ones() as usize; // Index of lowest zero bit
        let c = c.min(31);

        let mut point = Vec::with_capacity(self.dimensions);
        for d in 0..self.dimensions {
            self.x[d] ^= self.direction_numbers[d][c];
            let u = (self.x[d] as f64) * (1.0 / 4294967296.0); // 2^-32
            point.push(u);
        }
        self.index = self.index.wrapping_add(1);
        point
    }
}

/// Helper to generate direction numbers for $D$ dimensions using primitive polynomials.
fn generate_sobol_direction_numbers(dimensions: usize) -> Vec<[u32; 32]> {
    // Primitive polynomials table: (degree s, polynomial integer a, initial m_i values)
    // Formula: P(x) = x^s + a_1 x^{s-1} + ... + 1
    const SOBOL_POLY_TABLE: [(usize, u32, &[u32]); 31] = [
        (1, 0, &[1]),
        (2, 1, &[1, 3]),
        (3, 1, &[1, 3, 1]),
        (3, 2, &[1, 1, 1]),
        (4, 1, &[1, 1, 3, 3]),
        (4, 4, &[1, 3, 5, 13]),
        (4, 7, &[1, 1, 5, 5]),
        (5, 2, &[1, 1, 7, 11, 19]),
        (5, 4, &[1, 1, 5, 1, 1]),
        (5, 7, &[1, 1, 1, 3, 11]),
        (5, 11, &[1, 3, 5, 5, 9]),
        (5, 13, &[1, 1, 1, 9, 23]),
        (5, 14, &[1, 3, 7, 13, 29]),
        (6, 1, &[1, 1, 5, 11, 27, 43]),
        (6, 4, &[1, 3, 1, 17, 41, 61]),
        (6, 7, &[1, 1, 1, 5, 45, 23]),
        (6, 11, &[1, 3, 3, 9, 7, 49]),
        (6, 13, &[1, 1, 1, 13, 21, 35]),
        (6, 16, &[1, 1, 7, 3, 35, 19]),
        (6, 19, &[1, 3, 1, 13, 25, 41]),
        (6, 22, &[1, 1, 3, 17, 33, 1]),
        (6, 25, &[1, 3, 3, 3, 9, 61]),
        (6, 28, &[1, 1, 7, 15, 29, 3]),
        (6, 31, &[1, 3, 5, 9, 39, 45]),
        (7, 1, &[1, 1, 3, 7, 23, 47, 95]),
        (7, 4, &[1, 3, 7, 1, 19, 49, 109]),
        (7, 7, &[1, 1, 1, 1, 11, 23, 117]),
        (7, 8, &[1, 3, 5, 9, 1, 33, 21]),
        (7, 14, &[1, 1, 7, 13, 25, 5, 87]),
        (7, 19, &[1, 3, 1, 11, 57, 125, 33]),
        (7, 21, &[1, 1, 5, 21, 61, 89, 9]),
    ];

    let mut dir_nums = Vec::with_capacity(dimensions);

    // Dimension 0: v_{0, i} = 1 << (32 - (i + 1))
    let mut v0 = [0u32; 32];
    for i in 0..32 {
        v0[i] = 1u32 << (31 - i);
    }
    dir_nums.push(v0);

    for d in 1..dimensions {
        let (s, a, init_m) = SOBOL_POLY_TABLE[d - 1];
        let mut m = [0u32; 32];
        for (i, &val) in init_m.iter().enumerate() {
            if i < 32 {
                m[i] = val;
            }
        }
        for i in s..32 {
            let mut new_m = m[i - s] ^ (m[i - s] << s);
            for k in 1..s {
                let ak = (a >> (s - 1 - k)) & 1;
                if ak != 0 {
                    new_m ^= m[i - k] << k;
                }
            }
            m[i] = new_m;
        }

        let mut v = [0u32; 32];
        for i in 0..32 {
            v[i] = m[i] << (31 - i);
        }
        dir_nums.push(v);
    }

    dir_nums
}

// -----------------------------------------------------------------------------
// Statistical Synthesis & Sensitivity Analysis
// -----------------------------------------------------------------------------

/// Summary of a single parameter point execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PointSweepSummary {
    pub point_index: usize,
    pub parameters: Vec<f64>,
    pub completed_runs: usize,
    pub failed_runs: usize,
    pub observables: HashMap<String, MetricSummary>,
}

/// Global statistics and sensitivity correlations for an observable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalMetricStats {
    pub mean: f64,
    pub std: f64,
    pub min: f64,
    pub max: f64,
    /// Pearson correlation coefficients $r(X_d, Y_O)$ against each parameter dimension.
    pub parameter_correlations: Vec<Option<f64>>,
}

/// Failed run record preserved during sweep execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailedRunSummary {
    pub point_index: usize,
    pub seed: u64,
    pub tick: u64,
    pub reason: String,
}

/// Full comprehensive report of a completed parameter sweep.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SweepResult {
    pub sweep_id: String,
    pub name: String,
    pub base_manifest_fingerprint: u64,
    pub parameter_names: Vec<String>,
    pub parameter_matrix: Vec<Vec<f64>>,
    pub total_points: usize,
    pub completed_points: usize,
    pub failed_points: usize,
    pub total_runs: usize,
    pub completed_runs: usize,
    pub failed_runs: usize,
    pub global_metrics: HashMap<String, GlobalMetricStats>,
    pub points: Vec<PointSweepSummary>,
    pub failure_records: Vec<FailedRunSummary>,
}

impl SweepResult {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        // Collect all unique observable keys
        let mut obs_keys: Vec<String> = self
            .points
            .iter()
            .flat_map(|p| p.observables.keys().cloned())
            .collect();
        obs_keys.sort();
        obs_keys.dedup();

        // Header
        out.push_str("point_index");
        for p_name in &self.parameter_names {
            out.push(',');
            out.push_str(p_name);
        }
        out.push_str(",completed_runs,failed_runs");
        for obs in &obs_keys {
            out.push_str(&format!(",{obs}_mean,{obs}_std,{obs}_min,{obs}_max"));
        }
        out.push('\n');

        // Rows
        for pt in &self.points {
            out.push_str(&format!("{}", pt.point_index));
            for val in &pt.parameters {
                out.push_str(&format!(",{val}"));
            }
            out.push_str(&format!(",{},{}", pt.completed_runs, pt.failed_runs));
            for obs in &obs_keys {
                if let Some(m) = pt.observables.get(obs) {
                    let std_val = m.std.unwrap_or(0.0);
                    out.push_str(&format!(",{},{},{},{}", m.mean, std_val, m.min, m.max));


                } else {
                    out.push_str(",,,,");
                }
            }
            out.push('\n');
        }
        out
    }
}

/// Compute Pearson correlation coefficient $r(X, Y)$ between two series.
pub fn pearson_correlation(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }
    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    let mut var_x = 0.0;
    let mut var_y = 0.0;
    let mut cov_xy = 0.0;

    for (xi, yi) in x.iter().zip(y.iter()) {
        let dx = xi - mean_x;
        let dy = yi - mean_y;
        var_x += dx * dx;
        var_y += dy * dy;
        cov_xy += dx * dy;
    }

    if var_x <= 1e-15 || var_y <= 1e-15 {
        return None; // Zero variance
    }

    let r = cov_xy / (var_x.sqrt() * var_y.sqrt());
    if r.is_finite() {
        Some(r.clamp(-1.0, 1.0))
    } else {
        None
    }
}

// -----------------------------------------------------------------------------
// Parallel Sweep Execution Engine
// -----------------------------------------------------------------------------

/// Execute a full parameter sweep over the declared model `M`.
pub fn run_sweep<M: ExperimentModel>(
    manifest: &SweepManifest,
    registry: &ObservableRegistry,
) -> Result<SweepResult, ExperimentError> {
    registry.validate()?;
    manifest.validate(registry)?;

    let (param_names, matrix) = manifest.generate_sample_matrix()?;
    let total_points = matrix.len();
    let num_dims = param_names.len();

    let point_manifests: Vec<ExperimentManifest> = matrix
        .iter()
        .enumerate()
        .map(|(idx, row)| manifest.apply_point(idx, row))
        .collect::<Result<_, _>>()?;

    let run_point = |(idx, pm): (usize, &ExperimentManifest)| -> (PointSweepSummary, Vec<FailedRunSummary>) {
        let params = matrix[idx].clone();
        match run_ensemble::<M>(pm, registry) {
            Ok(summary) => {
                let mut obs_map = HashMap::new();
                for m in summary.metrics {
                    obs_map.insert(m.observable.clone(), m);
                }
                let point_summary = PointSweepSummary {
                    point_index: idx,
                    parameters: params,
                    completed_runs: summary.completed_runs,
                    failed_runs: summary.failed.len(),
                    observables: obs_map,
                };
                let fails = summary
                    .failed
                    .into_iter()
                    .map(|(seed, reason)| FailedRunSummary {
                        point_index: idx,
                        seed,
                        tick: 0,
                        reason,
                    })
                    .collect();
                (point_summary, fails)
            }

            Err(e) => {
                let point_summary = PointSweepSummary {
                    point_index: idx,
                    parameters: params,
                    completed_runs: 0,
                    failed_runs: pm.seeds.len(),
                    observables: HashMap::new(),
                };
                let fails = pm
                    .seeds
                    .iter()
                    .map(|&s| FailedRunSummary {
                        point_index: idx,
                        seed: s,
                        tick: 0,
                        reason: format!("ensemble rejected: {e}"),
                    })
                    .collect();
                (point_summary, fails)
            }
        }
    };

    let results: Vec<(PointSweepSummary, Vec<FailedRunSummary>)> = if let Some(threads) = manifest.max_threads {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|_| ExperimentError::ResourceLimit {
                field: "rayon thread pool".into(),
                limit: threads,
                found: 0,
            })?;
        pool.install(|| {
            point_manifests
                .par_iter()
                .enumerate()
                .map(run_point)
                .collect()
        })
    } else {
        point_manifests
            .par_iter()
            .enumerate()
            .map(run_point)
            .collect()
    };

    let mut points = Vec::with_capacity(total_points);
    let mut failure_records = Vec::new();
    let mut total_completed_runs = 0usize;
    let mut total_failed_runs = 0usize;
    let mut completed_points = 0usize;
    let mut failed_points = 0usize;

    for (pt_summary, fails) in results {
        total_completed_runs += pt_summary.completed_runs;
        total_failed_runs += pt_summary.failed_runs;
        if pt_summary.completed_runs > 0 {
            completed_points += 1;
        } else {
            failed_points += 1;
        }
        points.push(pt_summary);
        failure_records.extend(fails);
    }

    // Global metric aggregation & sensitivity analysis
    let mut all_obs_keys: Vec<String> = points
        .iter()
        .flat_map(|p| p.observables.keys().cloned())
        .collect();
    all_obs_keys.sort();
    all_obs_keys.dedup();

    let mut global_metrics = HashMap::new();

    for obs_id in all_obs_keys {
        let mut obs_means = Vec::with_capacity(total_points);
        let mut valid_point_indices = Vec::with_capacity(total_points);

        for (idx, pt) in points.iter().enumerate() {
            if let Some(metric) = pt.observables.get(&obs_id) {
                obs_means.push(metric.mean);
                valid_point_indices.push(idx);
            }
        }

        if obs_means.is_empty() {
            continue;
        }

        let n = obs_means.len() as f64;
        let mean = obs_means.iter().sum::<f64>() / n;
        let var = if obs_means.len() > 1 {
            obs_means.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)
        } else {
            0.0
        };
        let std = var.sqrt();
        let min = obs_means.iter().copied().fold(f64::INFINITY, f64::min);
        let max = obs_means.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        // Parameter correlations $r(X_d, Y_O)$
        let mut parameter_correlations = Vec::with_capacity(num_dims);
        for d in 0..num_dims {
            let x_vals: Vec<f64> = valid_point_indices
                .iter()
                .map(|&idx| matrix[idx][d])
                .collect();
            parameter_correlations.push(pearson_correlation(&x_vals, &obs_means));
        }

        global_metrics.insert(
            obs_id,
            GlobalMetricStats {
                mean,
                std,
                min,
                max,
                parameter_correlations,
            },
        );
    }

    let total_runs = total_completed_runs + total_failed_runs;

    Ok(SweepResult {
        sweep_id: manifest.sweep_id.clone(),
        name: manifest.name.clone(),
        base_manifest_fingerprint: manifest.base_manifest.fingerprint(),
        parameter_names: param_names,
        parameter_matrix: matrix,

        total_points,
        completed_points,
        failed_points,
        total_runs,
        completed_runs: total_completed_runs,
        failed_runs: total_failed_runs,
        global_metrics,
        points,
        failure_records,
    })
}
