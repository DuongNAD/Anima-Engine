//! Versioned, checksummed, atomically-written simulation snapshots (G1.2).
//!
//! A save used to be `serde_json::to_string_pretty` piped into `std::fs::write`. That is three
//! separate problems wearing one coat:
//!
//! 1. **Not versioned.** Adding a field to [`SavedSimulationState`] silently changed what a save
//!    meant, and nothing on disk said which shape a given file was. `#[serde(default)]` made old
//!    files *load*, which is not the same as loading *correctly*.
//! 2. **Not checked.** A truncated or half-flushed file deserialized into a plausible world.
//! 3. **Not atomic.** `fs::write` truncates the target and then streams into it. A crash, a full
//!    disk, or two saves racing left a corrupt file where a good one used to be — the file you lose
//!    is the one you were trying to protect.
//!
//! [`SnapshotEnvelope`] wraps the state with a schema version, build provenance and a checksum, and
//! [`write_atomic`] writes to a temp file in the same directory, flushes, syncs, and only then
//! renames over the target. Rename within a directory is atomic on both NTFS and POSIX, so a reader
//! sees either the whole old file or the whole new one.
//!
//! # What makes a snapshot a *checkpoint*
//!
//! A checkpoint is not "enough state to draw the world again". It is enough state that resuming
//! from it is indistinguishable from never having stopped. That is a strictly larger set, and the
//! part that is easy to forget is the stream position of the RNG: restoring a seed alone restarts
//! the sequence, so a resumed run diverges on its very next draw. See
//! [`SimRng::stream_pos`](crate::core::resources::SimRng::stream_pos).
//!
//! The gate is `checksum(run N) == checksum(run K → save → load → run N−K)`, which fails loudly for
//! any piece of trajectory-relevant state the snapshot forgets.
//!
//! # Migration
//!
//! [`read`] accepts the current schema and the two before it (N−2), which is the window the G1.2
//! requirement names. Older files are rejected with a message naming the version, rather than being
//! coerced into a shape they were never written in.

use crate::core::simulation_state::SavedSimulationState;
use crate::core::world_artifact::fnv1a_32;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;

/// Current snapshot schema version.
///
/// | Version | Shape |
/// |---|---|
/// | 1 | Pre-G1.1: agents, food, lakes, trees, pheromone grid, epoch, lineage. No energy state. |
/// | 2 | G1.1: adds the closed-EU compartments and the standing crop. |
/// | 3 | G1.2: adds RNG stream position, season phase and the energy baseline, and wraps the whole thing in an envelope. |
///
/// A bare `SavedSimulationState` on disk with no envelope around it is a version-1 or version-2
/// file; [`read`] detects that and migrates it forward.
/// | 4 | Adds the aggregate LOD tier's dormant cohorts, which hold agents and their EU. |
/// | 5 | Adds the live experiment state: manifest/law/registry fingerprints, the multi-rate clock's tick, and the causal ledger. |
/// | 6 | Adds the exact evolution-worker RNG, MAP-Elites archive, identity cursors and Meta-AI state. |
/// | 7 | Adds the shared learner model, Adam state, partial/queued transitions and inference policy. |
pub const SCHEMA_VERSION: u32 = 7;

/// Oldest **enveloped** schema this build can still read. N−2 per the G1.2 requirement.
///
/// This bound applies only to files that carry a `schema_version`, which means version 3 and up.
/// Versions 1 and 2 were written as a bare `SavedSimulationState` with no envelope at all, so they
/// have no version to compare and are handled by [`from_bytes`]'s pre-envelope path regardless of
/// this constant. Raising the current version therefore does *not* strand a v1/v2 save — a claim
/// worth checking rather than assuming, and `a_bare_pre_envelope_state_still_loads_and_reports_its_schema`
/// is what checks it.
pub const MIN_SUPPORTED_SCHEMA: u32 = SCHEMA_VERSION - 2;
/// Hard bound applied before a snapshot file is read into memory.
///
/// A thousand evolved agents with both inherited and learned networks fit below this ceiling in
/// JSON, while a malformed local file can no longer ask the desktop process to allocate without
/// limit.
pub const MAX_SNAPSHOT_BYTES: usize = 256 * 1024 * 1024;
const MAX_SNAPSHOT_AGENTS: usize = 100_000;
const MAX_SNAPSHOT_WORLD_OBJECTS: usize = 1_000_000;
const MAX_SNAPSHOT_HISTORY_EVENTS: usize = 1_000_000;
const MAX_SNAPSHOT_LINEAGE_RECORDS: usize = 2_000_000;
const MAX_SNAPSHOT_FOOD_CAP: usize = 1_000_000;
const MAX_SNAPSHOT_MAP_ELITES_CELLS: usize = 1_000_000;
const MAX_SNAPSHOT_MAP_ELITES_FEATURES: usize = 64;
const MAX_SNAPSHOT_LEARNER_RECORD_BYTES: usize = 16 * 1024 * 1024;
const MAX_SNAPSHOT_SCALAR_MAGNITUDE: f32 = 1.0e9;
// Closed-resource arithmetic can land a few ULPs below zero before the next clamp. Preserve those
// checkpoints exactly while still rejecting materially impossible negative stores.
const SNAPSHOT_NEGATIVE_TOLERANCE: f64 = 1.0e-3;

fn exceeds_snapshot_upper_bound(current: f32, maximum: f32) -> bool {
    let tolerance = (SNAPSHOT_NEGATIVE_TOLERANCE as f32).max(maximum.abs() * 8.0 * f32::EPSILON);
    current > maximum + tolerance
}

fn exceeds_numeric_safety_envelope(value: glam::Vec3) -> bool {
    value.abs().max_element() > MAX_SNAPSHOT_SCALAR_MAGNITUDE
}

/// What produced a snapshot. Not used to gate loading — it is here so that a run which cannot be
/// reproduced can at least be *explained*, which is the difference between a scientific record and
/// a save file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BuildProvenance {
    /// Crate version of the engine that wrote this.
    pub engine_version: String,
    /// Target triple it was built for. Float behaviour is not identical across all targets, so a
    /// checksum mismatch between two machines is worth being able to attribute.
    pub target: String,
    /// Debug or release. Different optimisation levels can reassociate float arithmetic.
    pub profile: String,
}

impl Default for BuildProvenance {
    fn default() -> Self {
        Self::current()
    }
}

impl BuildProvenance {
    /// Provenance of the running binary.
    pub fn current() -> Self {
        Self {
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            target: std::env::consts::ARCH.to_string() + "-" + std::env::consts::OS,
            profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
        }
    }
}

/// A snapshot as it exists on disk.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SnapshotEnvelope {
    /// Which [`SCHEMA_VERSION`] the `state` below was written in.
    pub schema_version: u32,
    /// What produced it.
    pub build_provenance: BuildProvenance,
    /// FNV-1a 32 over **the exact bytes of `state` as they appear in this file**. Same primitive
    /// the World Artifact uses, so there is one checksum algorithm in the codebase rather than two.
    pub checksum: u32,
    /// The complete simulation state, held as raw JSON.
    ///
    /// Raw, rather than a typed `SavedSimulationState`, so that the bytes that were hashed, the
    /// bytes on disk and the bytes that get verified are all literally the same bytes. A checksum
    /// computed by *re-serializing* the parsed state is a checksum of a different artefact, and
    /// every difference between the two is a false integrity failure.
    ///
    /// The historical reason given here was that "serde_json's `f64` round trip is not bit-exact",
    /// citing `990.5102615356445` reading back as `...444`. The observation was real and the blame
    /// was misplaced: serde_json **writes** floats with `ryu`, which is shortest-round-trip, and it
    /// wrote that value exactly. The default **parser** was the approximate half, and `Cargo.toml`
    /// now enables `float_roundtrip` to fix it — see
    /// `snapshot_checkpoint_tests::serde_json_round_trips_every_awkward_f64_bit_for_bit`.
    ///
    /// [`RawValue`] still earns its place, because the failures it rules out are the ones that
    /// remain: map iteration order, and any future change to how the envelope is formatted. Both
    /// would move the bytes without moving the state. The state still appears as a normal nested
    /// object in the file, not an escaped string.
    pub state: Box<serde_json::value::RawValue>,
}

/// Errors that can come out of reading a snapshot. Each one names what is wrong rather than
/// collapsing to "invalid save", because the three failures need different responses from a user.
#[derive(Debug)]
pub enum SnapshotError {
    /// The file could not be read at all.
    Io(String),
    /// The bytes are not a snapshot in any schema this build understands.
    Malformed(String),
    /// A schema older than [`MIN_SUPPORTED_SCHEMA`].
    TooOld { found: u32, minimum: u32 },
    /// A schema newer than this build — written by a later version of the engine.
    TooNew { found: u32, current: u32 },
    /// The state does not hash to the checksum recorded beside it.
    ChecksumMismatch { expected: u32, actual: u32 },
    /// The file exceeds the allocation budget before parsing begins.
    TooLarge { found: u64, limit: usize },
    /// The JSON is structurally readable but contains state the simulation cannot safely execute.
    InvalidState(String),
    /// The bounded input could not be reserved without aborting the process.
    AllocationFailed { requested: usize, reason: String },
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "snapshot could not be read: {e}"),
            Self::Malformed(e) => write!(f, "snapshot is not readable in any known schema: {e}"),
            Self::TooOld { found, minimum } => write!(
                f,
                "snapshot schema {found} is older than the minimum supported {minimum}; \
                 load it with an engine build from that era and re-save it"
            ),
            Self::TooNew { found, current } => write!(
                f,
                "snapshot schema {found} was written by a newer engine (this build understands \
                 {current}); upgrade rather than loading it"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                f,
                "snapshot checksum mismatch (recorded {expected:#010x}, computed {actual:#010x}); \
                 the file is corrupt or was modified"
            ),
            Self::TooLarge { found, limit } => write!(
                f,
                "snapshot is {found} bytes; the maximum supported size is {limit} bytes"
            ),
            Self::InvalidState(error) => {
                write!(f, "snapshot contains invalid scientific state: {error}")
            }
            Self::AllocationFailed { requested, reason } => write!(
                f,
                "snapshot needs a {requested}-byte input buffer that could not be reserved: {reason}"
            ),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// FNV-1a 32 over the exact JSON bytes of a state, as written to disk.
///
/// Deliberately takes the serialized text rather than the struct: see the note on
/// [`SnapshotEnvelope::state`] for why re-serializing to check a checksum does not work.
pub fn checksum_bytes(state_json: &str) -> u32 {
    fnv1a_32(state_json.as_bytes())
}

impl SnapshotEnvelope {
    /// Wrap a state for writing, computing its checksum and stamping the current build.
    pub fn seal(state: SavedSimulationState) -> Result<Self, SnapshotError> {
        validate_state(&state)?;
        let text = serde_json::to_string(&state)
            .map_err(|e| SnapshotError::Malformed(format!("state is not serializable: {e}")))?;
        if text.len() > MAX_SNAPSHOT_BYTES {
            return Err(SnapshotError::TooLarge {
                found: text.len() as u64,
                limit: MAX_SNAPSHOT_BYTES,
            });
        }
        let checksum = checksum_bytes(&text);
        let raw = serde_json::value::RawValue::from_string(text)
            .map_err(|e| SnapshotError::Malformed(format!("state json is not valid: {e}")))?;
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            build_provenance: BuildProvenance::current(),
            checksum,
            state: raw,
        })
    }

    /// Verify the envelope's checksum against the raw state bytes it carries.
    pub fn verify(&self) -> Result<(), SnapshotError> {
        let actual = checksum_bytes(self.state.get());
        if actual != self.checksum {
            return Err(SnapshotError::ChecksumMismatch {
                expected: self.checksum,
                actual,
            });
        }
        Ok(())
    }

    /// Parse the state out of the envelope.
    pub fn parse_state(&self) -> Result<SavedSimulationState, SnapshotError> {
        serde_json::from_str(self.state.get())
            .map_err(|e| SnapshotError::Malformed(format!("state did not parse: {e}")))
    }
}

/// Write a snapshot so that the target file is never left half-written.
///
/// Temp file in the same directory (so the rename does not cross a filesystem boundary), write,
/// flush, `sync_all`, then rename over the target. `sync_all` matters: without it the rename can
/// land before the data does, and a power loss leaves a correctly-named empty file.
pub fn write_atomic(path: &Path, envelope: &SnapshotEnvelope) -> Result<(), SnapshotError> {
    let json = serde_json::to_vec_pretty(envelope)
        .map_err(|e| SnapshotError::Malformed(format!("envelope is not serializable: {e}")))?;
    if json.len() > MAX_SNAPSHOT_BYTES {
        return Err(SnapshotError::TooLarge {
            found: json.len() as u64,
            limit: MAX_SNAPSHOT_BYTES,
        });
    }
    write_bytes_atomic(path, &json)
}

/// The bytes-in half of [`write_atomic`], for callers that are not writing a snapshot envelope.
///
/// Extracted rather than duplicated because the property that matters — a failed write leaves the
/// old file intact and no partial file behind — is easy to reimplement *almost* correctly. The tick
/// capture's export writes through here for exactly that reason.
pub fn write_bytes_atomic(path: &Path, json: &[u8]) -> Result<(), SnapshotError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    if !dir.as_os_str().is_empty() {
        std::fs::create_dir_all(dir)
            .map_err(|e| SnapshotError::Io(format!("could not create {}: {e}", dir.display())))?;
    }

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "snapshot".to_string());
    // Include the process id so two engines saving to the same path do not fight over one temp
    // file and interleave their bytes.
    let tmp_path = dir.join(format!(".{file_name}.{}.tmp", std::process::id()));

    {
        let mut file = std::fs::File::create(&tmp_path)
            .map_err(|e| SnapshotError::Io(format!("could not create temp file: {e}")))?;
        file.write_all(json)
            .map_err(|e| SnapshotError::Io(format!("could not write temp file: {e}")))?;
        file.flush()
            .map_err(|e| SnapshotError::Io(format!("could not flush temp file: {e}")))?;
        file.sync_all()
            .map_err(|e| SnapshotError::Io(format!("could not sync temp file: {e}")))?;
    }

    // Windows `rename` fails if the destination exists, unlike POSIX. Removing first opens a
    // window where neither file is at the target path, which is still strictly better than the old
    // behaviour of truncating the target before writing a single byte.
    #[cfg(windows)]
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }

    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        SnapshotError::Io(format!("could not rename temp file into place: {e}"))
    })?;

    Ok(())
}

/// Read a snapshot, verifying its checksum and migrating it forward if it is an older schema.
///
/// Accepts, in order:
/// - a version-3 envelope (checksum verified);
/// - a bare `SavedSimulationState`, which is how versions 1 and 2 were written — no envelope
///   existed then, so there is nothing to verify and `#[serde(default)]` supplies the fields those
///   files predate.
pub fn read(path: &Path) -> Result<SavedSimulationState, SnapshotError> {
    read_with_limit(path, MAX_SNAPSHOT_BYTES)
}

fn read_with_limit(path: &Path, limit: usize) -> Result<SavedSimulationState, SnapshotError> {
    let file = std::fs::File::open(path)
        .map_err(|e| SnapshotError::Io(format!("could not open {}: {e}", path.display())))?;
    let size = file
        .metadata()
        .map_err(|e| SnapshotError::Io(format!("could not inspect {}: {e}", path.display())))?
        .len();
    if size > limit as u64 {
        return Err(SnapshotError::TooLarge { found: size, limit });
    }
    let requested = size as usize;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(requested)
        .map_err(|error| SnapshotError::AllocationFailed {
            requested,
            reason: error.to_string(),
        })?;
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| SnapshotError::Io(format!("could not read {}: {e}", path.display())))?;
    if bytes.len() > limit {
        return Err(SnapshotError::TooLarge {
            found: bytes.len() as u64,
            limit,
        });
    }
    from_bytes(&bytes)
}

/// The parsing half of [`read`], separated so it can be tested without touching a filesystem.
pub fn from_bytes(bytes: &[u8]) -> Result<SavedSimulationState, SnapshotError> {
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(SnapshotError::TooLarge {
            found: bytes.len() as u64,
            limit: MAX_SNAPSHOT_BYTES,
        });
    }
    // Peek at just the version so a newer file produces a clear message instead of a confusing
    // field-level deserialization error.
    #[derive(Deserialize)]
    struct VersionPeek {
        schema_version: u32,
    }

    if let Ok(peek) = serde_json::from_slice::<VersionPeek>(bytes) {
        if peek.schema_version > SCHEMA_VERSION {
            return Err(SnapshotError::TooNew {
                found: peek.schema_version,
                current: SCHEMA_VERSION,
            });
        }
        if peek.schema_version < MIN_SUPPORTED_SCHEMA {
            return Err(SnapshotError::TooOld {
                found: peek.schema_version,
                minimum: MIN_SUPPORTED_SCHEMA,
            });
        }
        let envelope: SnapshotEnvelope = serde_json::from_slice(bytes)
            .map_err(|e| SnapshotError::Malformed(format!("envelope did not parse: {e}")))?;
        envelope.verify()?;
        let state = migrate(envelope.schema_version, envelope.parse_state()?);
        validate_state(&state)?;
        return Ok(state);
    }

    // No `schema_version` key: a pre-envelope file, so schema 1 or 2. Schema 1 is now outside the
    // declared N−2 window, and is still accepted here rather than rejected: an unversioned file
    // cannot say which of the two it is, and every field either version lacks arrives through
    // `#[serde(default)]`. Refusing on a version it never wrote down would be guessing.
    let state: SavedSimulationState = serde_json::from_slice(bytes).map_err(|e| {
        SnapshotError::Malformed(format!(
            "not an envelope and not a bare saved state either: {e}"
        ))
    })?;
    let state = migrate(legacy_schema_of(&state), state);
    validate_state(&state)?;
    Ok(state)
}

fn validate_state(state: &SavedSimulationState) -> Result<(), SnapshotError> {
    let invalid = |message: String| SnapshotError::InvalidState(message);

    if state.tick_count == u64::MAX
        || state.epoch_manager.current_epoch_ticks == u64::MAX
        || state.epoch_manager.current_epoch == u32::MAX
    {
        return Err(invalid(
            "simulation or epoch counter cannot advance without overflow".to_owned(),
        ));
    }

    let food_settings = state.food_spawn_settings;
    if food_settings.max_food_count > MAX_SNAPSHOT_FOOD_CAP
        || !food_settings.default_energy.is_finite()
        || !food_settings.default_hydration.is_finite()
        || food_settings.default_energy < 0.0
        || food_settings.default_hydration < 0.0
        || food_settings.default_energy > MAX_SNAPSHOT_SCALAR_MAGNITUDE
        || food_settings.default_hydration > MAX_SNAPSHOT_SCALAR_MAGNITUDE
    {
        return Err(invalid(format!(
            "food spawn settings exceed the supported cap of {MAX_SNAPSHOT_FOOD_CAP} or contain \
             invalid resource values"
        )));
    }
    let evolution = &state.evolution_settings;
    evolution
        .validate()
        .map_err(|error| invalid(format!("evolution settings: {error}")))?;
    let map_elites = &state.map_elites_grid;
    if map_elites.grid_resolution == 0
        || map_elites.grid_resolution > crate::commands::MAX_EVOLUTION_GRID_RESOLUTION
        || map_elites.grid.len() > MAX_SNAPSHOT_MAP_ELITES_CELLS
        || map_elites.grid.iter().any(|(key, elite)| {
            key.is_empty()
                || key.len() > 1_024
                || key.chars().any(char::is_control)
                || !elite.fitness.is_finite()
                || elite.features.len() > MAX_SNAPSHOT_MAP_ELITES_FEATURES
                || elite.features.iter().any(|value| !value.is_finite())
        })
    {
        return Err(invalid(
            "MAP-Elites archive shape or scientific values are invalid".to_owned(),
        ));
    }

    if state.agents.len() > MAX_SNAPSHOT_AGENTS {
        return Err(invalid(format!(
            "{} agents exceed the snapshot limit of {MAX_SNAPSHOT_AGENTS}",
            state.agents.len()
        )));
    }
    for (index, agent) in state.agents.iter().enumerate() {
        agent
            .validate()
            .map_err(|error| invalid(format!("agent {index}: {error}")))?;
        if exceeds_numeric_safety_envelope(agent.root_position)
            || exceeds_numeric_safety_envelope(agent.root_velocity)
            || exceeds_numeric_safety_envelope(agent.evaluation.start_position)
            || exceeds_numeric_safety_envelope(agent.evaluation.last_position)
            || agent.segments.iter().any(|segment| {
                exceeds_numeric_safety_envelope(segment.position)
                    || exceeds_numeric_safety_envelope(segment.velocity)
            })
        {
            return Err(invalid(format!(
                "agent {index} kinematics exceed the snapshot numeric safety envelope"
            )));
        }
    }

    let bounds = state.map_bounds;
    if !bounds.min.is_finite()
        || !bounds.max.is_finite()
        || exceeds_numeric_safety_envelope(bounds.min)
        || exceeds_numeric_safety_envelope(bounds.max)
        || bounds.min.x >= bounds.max.x
        || bounds.min.y > bounds.max.y
        || bounds.min.z >= bounds.max.z
    {
        return Err(invalid("map bounds are non-finite or inverted".to_owned()));
    }

    if state.pheromone_grid.values.len() != crate::ai::pheromone::CELL_COUNT
        || state
            .pheromone_grid
            .values
            .iter()
            .any(|value| !value.is_finite())
        || !state.pheromone_grid.diffusion_rate.is_finite()
        || !state.pheromone_grid.decay_rate.is_finite()
        || state.pheromone_grid.diffusion_rate < 0.0
        || state.pheromone_grid.decay_rate < 0.0
    {
        return Err(invalid(
            "pheromone grid shape or scalar state is invalid".to_owned(),
        ));
    }

    let object_counts = [
        ("foods", state.foods.len()),
        ("lakes", state.lakes.len()),
        ("trees", state.trees.len()),
    ];
    if let Some((kind, found)) = object_counts
        .into_iter()
        .find(|(_, found)| *found > MAX_SNAPSHOT_WORLD_OBJECTS)
    {
        return Err(invalid(format!(
            "{found} {kind} exceed the snapshot limit of {MAX_SNAPSHOT_WORLD_OBJECTS}"
        )));
    }
    for (index, food) in state.foods.iter().enumerate() {
        if !food.position.is_finite()
            || exceeds_numeric_safety_envelope(food.position)
            || !food.energy_value.is_finite()
            || !food.hydration_value.is_finite()
            || food.energy_value < 0.0
            || food.hydration_value < 0.0
        {
            return Err(invalid(format!("food {index} has invalid state")));
        }
    }
    for (index, lake) in state.lakes.iter().enumerate() {
        let values = [
            lake.radius,
            lake.current_water,
            lake.max_water,
            lake.replenishment_rate,
        ];
        if !lake.position.is_finite()
            || exceeds_numeric_safety_envelope(lake.position)
            || values.iter().any(|value| !value.is_finite())
            || lake.radius <= 0.0
            || lake.current_water < -(SNAPSHOT_NEGATIVE_TOLERANCE as f32)
            || lake.max_water < 0.0
            || exceeds_snapshot_upper_bound(lake.current_water, lake.max_water)
            || lake.replenishment_rate < 0.0
        {
            return Err(invalid(format!("lake {index} has invalid state")));
        }
    }
    for (index, tree) in state.trees.iter().enumerate() {
        let values = [
            tree.radius,
            tree.current_fruit,
            tree.max_fruit,
            tree.fruit_growth_rate,
            tree.time_since_last_drop,
            tree.seed_drop_cooldown,
            tree.seed_spread_radius,
        ];
        if !tree.position.is_finite()
            || exceeds_numeric_safety_envelope(tree.position)
            || values.iter().any(|value| !value.is_finite())
            || tree.radius <= 0.0
            || tree.current_fruit < -(SNAPSHOT_NEGATIVE_TOLERANCE as f32)
            || tree.max_fruit < 0.0
            || exceeds_snapshot_upper_bound(tree.current_fruit, tree.max_fruit)
            || tree.fruit_growth_rate < 0.0
            || tree.time_since_last_drop < 0.0
            || tree.seed_drop_cooldown < 0.0
            || tree.seed_spread_radius < 0.0
        {
            return Err(invalid(format!("tree {index} has invalid state")));
        }
    }

    if state.chronicle_history.len() > MAX_SNAPSHOT_HISTORY_EVENTS {
        return Err(invalid(format!(
            "{} chronicle events exceed the snapshot limit of {MAX_SNAPSHOT_HISTORY_EVENTS}",
            state.chronicle_history.len()
        )));
    }
    if state.lineage_nodes.len() > MAX_SNAPSHOT_LINEAGE_RECORDS
        || state.lineage_relations.len() > MAX_SNAPSHOT_LINEAGE_RECORDS
    {
        return Err(invalid(format!(
            "lineage records exceed the snapshot limit of {MAX_SNAPSHOT_LINEAGE_RECORDS}"
        )));
    }
    if let Some(cohorts) = &state.dormant_cohorts {
        cohorts
            .validate()
            .map_err(|error| invalid(format!("dormant population: {error}")))?;
    }
    if let Some(worker) = &state.evolution_worker {
        if worker.archive.elites.len() > MAX_SNAPSHOT_MAP_ELITES_CELLS
            || worker.meta_ai_history.len() > MAX_SNAPSHOT_HISTORY_EVENTS
            || worker
                .archive
                .elites
                .iter()
                .any(|elite| elite.features.len() > MAX_SNAPSHOT_MAP_ELITES_FEATURES)
        {
            return Err(invalid(
                "evolution worker checkpoint exceeds snapshot collection limits".to_owned(),
            ));
        }
        crate::core::simulation_state::evolution_worker_resume_state(
            Some(state),
            state.sim_rng_seed,
        )
        .map_err(|error| invalid(format!("evolution worker checkpoint: {error}")))?;
    }
    if let Some(shared) = &state.shared_learning {
        if shared.learner.training_model_record.is_empty()
            || shared.learner.training_model_record.len() > MAX_SNAPSHOT_LEARNER_RECORD_BYTES
            || shared.learner.optimizer_record.is_empty()
            || shared.learner.optimizer_record.len() > MAX_SNAPSHOT_LEARNER_RECORD_BYTES
        {
            return Err(invalid(
                "shared learner checkpoint records exceed snapshot limits".to_owned(),
            ));
        }
        shared
            .validate(state.sim_rng_seed)
            .map_err(|error| invalid(format!("shared learner checkpoint: {error}")))?;
        let pending_by_lineage: std::collections::HashMap<_, _> = shared
            .pending_inference
            .iter()
            .flat_map(|batch| batch.responses.iter())
            .map(|response| (response.lineage_id.as_str(), response.request_id))
            .collect();
        let pending_agents = state
            .agents
            .iter()
            .filter_map(|agent| match agent.cognitive_state {
                crate::core::components::CognitiveState::PendingInference(request_id) => {
                    Some((agent.lineage_id.as_str(), request_id))
                }
                _ => None,
            });
        for (lineage, request_id) in pending_agents {
            if pending_by_lineage.get(lineage) != Some(&request_id) {
                return Err(invalid(format!(
                    "pending inference response does not match agent '{lineage}'"
                )));
            }
        }
        if pending_by_lineage.len()
            != state
                .agents
                .iter()
                .filter(|agent| {
                    matches!(
                        agent.cognitive_state,
                        crate::core::components::CognitiveState::PendingInference(_)
                    )
                })
                .count()
        {
            return Err(invalid(
                "pending inference responses do not match the pending agent population".to_owned(),
            ));
        }
    }

    let ecosystem = [state.eco_detritus, state.eco_plants, state.eco_animals];
    if ecosystem
        .iter()
        .any(|value| !value.is_finite() || *value < -SNAPSHOT_NEGATIVE_TOLERANCE)
        || state
            .resource_field_r
            .iter()
            .any(|value| !value.is_finite() || *value < -(SNAPSHOT_NEGATIVE_TOLERANCE as f32))
        || !state.season_phase.is_finite()
        || !state.season_rate.is_finite()
        || state.season_rate < 0.0
        || state
            .energy_baseline
            .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(invalid(
            "ecosystem, season, or energy ledger state is invalid".to_owned(),
        ));
    }
    Ok(())
}

/// Which pre-envelope schema a bare state came from. Version 2 introduced the closed-energy
/// fields, so a state carrying none of them is version 1.
fn legacy_schema_of(state: &SavedSimulationState) -> u32 {
    let has_energy = state.eco_detritus != 0.0
        || state.eco_plants != 0.0
        || state.eco_animals != 0.0
        || !state.resource_field_r.is_empty();
    if has_energy {
        2
    } else {
        1
    }
}

/// Bring a state forward to [`SCHEMA_VERSION`].
///
/// There is deliberately no field-rewriting here yet: every field added since version 1 is
/// `#[serde(default)]`, and each default is the documented "this save predates the feature"
/// behaviour — zero energy state means "keep what `init_world` built", a zero RNG position means
/// "restart the stream from the seed". The function exists as the single place a future schema
/// bump puts real rewriting, and so the version a file came from is recorded rather than inferred
/// twice.
fn migrate(from: u32, mut state: SavedSimulationState) -> SavedSimulationState {
    state.loaded_from_schema = from;
    state
}

/// A fingerprint of everything about a live world that affects where it goes next.
///
/// This is the instrument the G1.2 gate measures with: `run N` must fingerprint identically to
/// `run K → save → load → run N−K`. Any trajectory-relevant state the snapshot forgets shows up
/// here as a mismatch, which is the point — a checksum that only covered what the snapshot happens
/// to save would agree with itself and prove nothing.
///
/// Agents and food are sorted by their **contents**, never by entity id. Two things force that:
/// Bevy iterates in archetype order, and a world restored from a snapshot allocates entity ids in a
/// different order than one that grew into the same state. Hashing ids — or hashing in iteration
/// order — would report a difference that is not one. Sorting by content means the fingerprint
/// answers "is this the same world state", which is the question the gate asks.
pub fn world_checksum(world: &mut bevy_ecs::world::World) -> u32 {
    use bevy_ecs::prelude::With;

    let mut bytes: Vec<u8> = Vec::with_capacity(4096);

    let push_f32 = |b: &mut Vec<u8>, v: f32| b.extend_from_slice(&v.to_bits().to_le_bytes());
    let push_f64 = |b: &mut Vec<u8>, v: f64| b.extend_from_slice(&v.to_bits().to_le_bytes());

    // Agents: reserve and body state, plus position, keyed by a stable id.
    let mut agents: Vec<[u32; 6]> = Vec::new();
    {
        let mut q = world.query_filtered::<(
            bevy_ecs::entity::Entity,
            &crate::ai::hrrl::HomeostaticState,
            &crate::core::ecs::Position,
        ), With<crate::core::ecs::Agent>>();
        for (_entity, homeo, pos) in q.iter(world) {
            agents.push([
                homeo.energy.to_bits(),
                homeo.hydration.to_bits(),
                homeo.temperature.to_bits(),
                pos.0.x.to_bits(),
                pos.0.y.to_bits(),
                pos.0.z.to_bits(),
            ]);
        }
    }
    agents.sort_unstable();
    bytes.extend_from_slice(&(agents.len() as u32).to_le_bytes());
    for vals in &agents {
        for v in vals {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }

    // Standing crop, cell by cell, plus which quarter of it the strided sweep visits next. The
    // phase decides *which* cells grow on the following tick, so a world holding identical cells at
    // a different phase is not the same world — see `ResourceField::regrowth_phase`.
    if let Some(field) = world.get_resource::<crate::core::ecology::ResourceField>() {
        bytes.extend_from_slice(&(field.r.len() as u32).to_le_bytes());
        for v in &field.r {
            bytes.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        bytes.extend_from_slice(&(field.regrowth_phase as u32).to_le_bytes());
    }

    // Detritus only — the one closed-energy compartment that is an authoritative store.
    //
    // `plants` mirrors the resource field and `animals` mirrors the agent reserves, both of which
    // are already hashed above; including them would hash the same energy twice. Worse, it would
    // import an error that is not the world's: a mirror survives a save as a single `f64` through
    // JSON, and serde_json's f64 round trip is not bit-exact, so a restored `animals` can land one
    // ULP away from the value the census computed. That made the checkpoint gate fail on a
    // difference in a *derived* number while every authoritative store matched exactly.
    //
    // The rule this settles: a fingerprint of the world hashes the stores, never the views of them.
    if let Some(pool) = world.get_resource::<crate::core::ecology::EcosystemBiomass>() {
        push_f64(&mut bytes, pool.detritus);
    }

    // The stream, including how far into it we are.
    if let Some(rng) = world.get_resource::<crate::core::resources::SimRng>() {
        bytes.extend_from_slice(&rng.seed().to_le_bytes());
        bytes.extend_from_slice(&rng.stream_pos().to_le_bytes());
    }

    // Season, which scales regrowth.
    if let Some(clock) = world.get_resource::<crate::core::ecology::SeasonClock>() {
        push_f32(&mut bytes, clock.phase);
        push_f32(&mut bytes, clock.rate);
    }

    // Food on the ground, sorted for the same archetype-order reason as agents.
    let mut foods: Vec<[u32; 3]> = Vec::new();
    {
        let mut q = world.query::<(
            bevy_ecs::entity::Entity,
            &crate::core::ecs::Position,
            &crate::core::ecs::Food,
        )>();
        for (_entity, pos, food) in q.iter(world) {
            foods.push([
                pos.0.x.to_bits(),
                pos.0.z.to_bits(),
                food.energy_value.to_bits(),
            ]);
        }
    }
    foods.sort_unstable();
    bytes.extend_from_slice(&(foods.len() as u32).to_le_bytes());
    for vals in &foods {
        for v in vals {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }

    fnv1a_32(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::model::BrainModel;
    use crate::core::simulation_state::{SavedInferenceResponseBatch, SavedSharedLearningState};

    fn state_fixture() -> SavedSimulationState {
        crate::core::simulation_state::empty_saved_state_for_tests()
    }

    fn shared_learning_fixture(seed: u64) -> SavedSharedLearningState {
        use burn::backend::Autodiff;
        use burn::module::Module;
        use burn::optim::{AdamConfig, Optimizer};
        use burn::record::{BinBytesRecorder, FullPrecisionSettings, Recorder};

        type B = burn_ndarray::NdArray<f32>;
        type AB = Autodiff<B>;

        let device = burn_ndarray::NdArrayDevice::Cpu;
        let weights = BrainModel::seeded_weights(
            crate::core::training::STATE_DIM,
            crate::core::training::HIDDEN_DIM,
            crate::core::training::ACTION_DIM,
            seed,
        );
        let model = crate::ai::model::ActorCriticModel::<AB>::from_flat_weights(
            crate::core::training::STATE_DIM,
            crate::core::training::HIDDEN_DIM,
            crate::core::training::ACTION_DIM,
            &weights,
            &device,
        )
        .expect("fixture model");
        let optimizer = AdamConfig::new().init::<AB, crate::ai::model::ActorCriticModel<AB>>();
        let recorder = BinBytesRecorder::<FullPrecisionSettings>::default();
        let training_model_record =
            Recorder::<AB>::record(&recorder, model.into_record(), ()).expect("record model");
        let optimizer_record =
            Recorder::<AB>::record(&recorder, optimizer.to_record(), ()).expect("record Adam");
        let transition = crate::ai::hrrl::Transition {
            state: [0.25; 15],
            action: [0.5; 4],
            reward: 0.75,
            next_state: [1.0; 15],
        };

        SavedSharedLearningState {
            learner: crate::core::training::SavedLearnerWorkerState {
                training_model_record,
                optimizer_record,
                partial_batch: vec![transition],
            },
            inference_weights: weights,
            pending_inference_weights: Some(BrainModel::seeded_weights(15, 64, 4, seed + 1)),
            queued_transitions: vec![transition],
            pending_inference: Vec::new(),
            learning_queue_diagnostics: crate::ai::hrrl::LearningQueueSnapshot {
                queued: 10,
                full_rejections: 2,
                disconnected_rejections: 1,
                backpressure_skipped: 3,
            },
            model_update_diagnostics: crate::core::training::ModelUpdateSnapshot {
                published: 4,
                backpressured: 1,
                disconnected: 0,
            },
        }
    }

    fn unchecked_envelope(state: SavedSimulationState) -> SnapshotEnvelope {
        let text = serde_json::to_string(&state).expect("serialize adversarial fixture");
        SnapshotEnvelope {
            schema_version: SCHEMA_VERSION,
            build_provenance: BuildProvenance::current(),
            checksum: checksum_bytes(&text),
            state: serde_json::value::RawValue::from_string(text).expect("raw state"),
        }
    }

    fn agent_fixture() -> crate::core::simulation_state::SerializedAgent {
        use crate::evolution::genotype::{MorphologyGenotype, MorphologyNode};
        let mut genotype = MorphologyGenotype::new();
        genotype.add_node(MorphologyNode {
            id: 0,
            length: 1.0,
            radius: 0.25,
            mass: 1.0,
        });
        crate::core::simulation_state::SerializedAgent {
            genotype,
            class: crate::core::components::AgentClass::Prey,
            lineage_id: "snapshot-agent".to_owned(),
            generation: 1,
            parent_ids: Vec::new(),
            evaluation: crate::core::agent_systems::AgentEvaluation {
                start_position: glam::Vec3::ZERO,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                survival_ticks: 1,
                last_position: glam::Vec3::ZERO,
            },
            feature_tracker: Default::default(),
            root_position: glam::Vec3::ZERO,
            root_rotation: glam::Quat::IDENTITY,
            root_velocity: glam::Vec3::ZERO,
            homeostatic_state: crate::ai::hrrl::HomeostaticState {
                energy: 50.0,
                energy_target: 100.0,
                hydration: 50.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
            last_transition_state: crate::ai::hrrl::LastTransitionState {
                state: [0.0; 15],
                action: [0.0; 4],
                has_last: false,
            },
            cognitive_state: Default::default(),
            inertia: Default::default(),
            action_gates: None,
            segments: Vec::new(),
            brain: None,
        }
    }

    fn two_segment_agent_fixture() -> crate::core::simulation_state::SerializedAgent {
        use crate::evolution::genotype::{MorphologyEdge, MorphologyNode};
        let mut agent = agent_fixture();
        agent.genotype.add_node(MorphologyNode {
            id: 1,
            length: 0.75,
            radius: 0.2,
            mass: 0.5,
        });
        agent.genotype.add_edge(MorphologyEdge {
            source_node: 0,
            target_node: 1,
            joint_anchor: glam::Vec3::Z,
            joint_axis: glam::Vec3::Y,
        });
        agent
            .segments
            .push(crate::core::simulation_state::SerializedSegmentState {
                segment_id: 1,
                position: glam::Vec3::Z,
                rotation: glam::Quat::IDENTITY,
                velocity: glam::Vec3::ZERO,
                oscillator: Some(crate::core::simulation_state::CpgOscillatorState {
                    phase: 0.0,
                    frequency: 1.0,
                    amplitude: 1.0,
                    output: 0.0,
                }),
            });
        agent
    }

    #[test]
    fn seal_then_verify_round_trips() {
        let envelope = SnapshotEnvelope::seal(state_fixture()).expect("seal");
        assert_eq!(envelope.schema_version, SCHEMA_VERSION);
        envelope
            .verify()
            .expect("a freshly sealed envelope verifies");
    }

    #[test]
    fn a_tampered_state_fails_its_checksum() {
        let mut envelope = SnapshotEnvelope::seal(state_fixture()).expect("seal");
        let mut tampered = envelope.parse_state().unwrap();
        tampered.tick_count += 1;
        envelope.state =
            serde_json::value::RawValue::from_string(serde_json::to_string(&tampered).unwrap())
                .unwrap();
        match envelope.verify() {
            Err(SnapshotError::ChecksumMismatch { .. }) => {}
            other => panic!("expected a checksum mismatch, got {other:?}"),
        }
    }

    #[test]
    fn the_checksum_covers_the_bytes_on_disk_not_a_reserialization() {
        // Hashing the raw bytes makes a file that was written correctly always verify, whatever
        // happens between the parser and the struct. `eco_animals` is the value that used to be
        // cited as proof that serde_json could not round-trip a float; it round-trips exactly now
        // (`float_roundtrip`), and this still has to hold for the reasons `RawValue` is really
        // there — map order and formatting.
        let mut state = state_fixture();
        state.eco_animals = 990.5102615356445;
        let envelope = SnapshotEnvelope::seal(state).expect("seal");
        let bytes = serde_json::to_vec_pretty(&envelope).expect("serialize envelope");
        let back: SnapshotEnvelope = serde_json::from_slice(&bytes).expect("parse envelope");
        back.verify().expect("a file written correctly must verify");
        assert_eq!(
            back.parse_state().expect("state parses").eco_animals,
            990.5102615356445,
            "the state inside a verified envelope must survive the read"
        );
    }

    #[test]
    fn envelope_round_trips_through_bytes() {
        let mut state = state_fixture();
        state.tick_count = 4242;
        state.sim_rng_seed = 1337;
        state.sim_rng_pos = 99;
        let envelope = SnapshotEnvelope::seal(state).expect("seal");
        let bytes = serde_json::to_vec_pretty(&envelope).unwrap();
        let back = from_bytes(&bytes).expect("read back");
        assert_eq!(back.tick_count, 4242);
        assert_eq!(back.sim_rng_seed, 1337);
        assert_eq!(back.sim_rng_pos, 99);
        assert_eq!(back.loaded_from_schema, SCHEMA_VERSION);
    }

    #[test]
    fn evolution_worker_checkpoint_round_trips_and_is_validated() {
        let mut state = state_fixture();
        state.sim_rng_seed = 1_337;
        let rng_seed = crate::core::resources::derived_seed(
            state.sim_rng_seed,
            crate::core::resources::sim_stream::EVOLUTION,
        );
        state.evolution_worker = Some(crate::core::simulation_state::SavedEvolutionWorkerState {
            rng_seed,
            rng_pos: 123,
            node_id_counter: 3,
            meta_ai_epoch: 0,
            meta_ai_history: Vec::new(),
            chronicle_ids_issued: 0,
            offspring_ids_issued: 0,
            archive: crate::evolution::map_elites::SavedMapElitesArchive {
                grid_resolution: 0.25,
                elites: Vec::new(),
            },
        });

        let envelope = SnapshotEnvelope::seal(state.clone()).expect("valid worker checkpoint");
        let back = from_bytes(&serde_json::to_vec(&envelope).unwrap()).expect("read back");
        let worker = back.evolution_worker.expect("schema 6 worker state");
        assert_eq!(worker.rng_seed, rng_seed);
        assert_eq!(worker.rng_pos, 123);
        assert_eq!(worker.archive.grid_resolution, 0.25);

        state.evolution_worker.as_mut().unwrap().rng_seed ^= 1;
        assert!(matches!(
            SnapshotEnvelope::seal(state),
            Err(SnapshotError::InvalidState(_))
        ));
    }

    #[test]
    fn shared_learning_checkpoint_round_trips_every_continuation_field() {
        let mut state = state_fixture();
        state.sim_rng_seed = 1_337;
        state.shared_learning = Some(shared_learning_fixture(state.sim_rng_seed));

        let envelope = SnapshotEnvelope::seal(state).expect("valid shared learning checkpoint");
        let back = from_bytes(&serde_json::to_vec(&envelope).unwrap()).expect("read back");
        let shared = back
            .shared_learning
            .expect("schema 7 shared learning state");

        assert_eq!(shared.learner.partial_batch.len(), 1);
        assert_eq!(shared.queued_transitions.len(), 1);
        assert_eq!(
            shared.inference_weights.len(),
            crate::core::training::SHARED_MODEL_PARAMETER_COUNT
        );
        assert_eq!(
            shared.pending_inference_weights.as_ref().map(Vec::len),
            Some(crate::core::training::SHARED_MODEL_PARAMETER_COUNT)
        );
        assert_eq!(shared.learning_queue_diagnostics.queued, 10);
        assert_eq!(shared.model_update_diagnostics.published, 4);
        shared
            .validate(1_337)
            .expect("decoded Burn model and Adam records remain usable");
    }

    #[test]
    fn shared_learning_checkpoint_rejects_bad_policy_and_transition_values() {
        let mut wrong_shape = state_fixture();
        wrong_shape.sim_rng_seed = 7;
        wrong_shape.shared_learning = Some(shared_learning_fixture(7));
        wrong_shape
            .shared_learning
            .as_mut()
            .unwrap()
            .inference_weights
            .pop();
        assert!(matches!(
            SnapshotEnvelope::seal(wrong_shape),
            Err(SnapshotError::InvalidState(_))
        ));

        let mut non_finite = state_fixture();
        non_finite.sim_rng_seed = 7;
        non_finite.shared_learning = Some(shared_learning_fixture(7));
        non_finite
            .shared_learning
            .as_mut()
            .unwrap()
            .queued_transitions[0]
            .reward = f32::NAN;
        assert!(matches!(
            SnapshotEnvelope::seal(non_finite),
            Err(SnapshotError::InvalidState(_))
        ));
    }

    #[test]
    fn pending_inference_round_trips_by_lineage_and_must_match_agent_ticket() {
        let mut state = state_fixture();
        state.sim_rng_seed = 17;
        let mut agent = agent_fixture();
        agent.cognitive_state = crate::core::components::CognitiveState::PendingInference(42);
        agent.inertia.cpg_parameters = [0.1, 0.2, 0.3, 0.4];
        agent.action_gates = Some(crate::core::components::ActionGates {
            pheromone_emit: 0.25,
            attack_intent: 0.5,
            feed_intent: 0.75,
        });
        state.agents.push(agent);
        let mut shared = shared_learning_fixture(state.sim_rng_seed);
        shared.pending_inference = vec![SavedInferenceResponseBatch {
            responses: vec![crate::core::simulation_state::SavedInferenceResponse {
                lineage_id: "snapshot-agent".to_owned(),
                actions: [0.6; crate::core::agent_systems::ACTION_SLOTS],
                request_id: 42,
            }],
        }];
        state.shared_learning = Some(shared);

        let envelope = SnapshotEnvelope::seal(state.clone()).expect("matched pending response");
        let back = from_bytes(&serde_json::to_vec(&envelope).unwrap()).expect("read back");
        assert_eq!(
            back.agents[0].cognitive_state,
            crate::core::components::CognitiveState::PendingInference(42)
        );
        assert_eq!(
            back.shared_learning.unwrap().pending_inference[0].responses[0].lineage_id,
            "snapshot-agent"
        );

        state.shared_learning.as_mut().unwrap().pending_inference[0].responses[0].request_id = 41;
        assert!(matches!(
            SnapshotEnvelope::seal(state),
            Err(SnapshotError::InvalidState(_))
        ));
    }

    #[test]
    fn a_bare_pre_envelope_state_still_loads_and_reports_its_schema() {
        // Version 1: no envelope, no energy fields.
        let v1 = state_fixture();
        let bytes = serde_json::to_vec(&v1).unwrap();
        let back = from_bytes(&bytes).expect("a v1 save must still load (D09)");
        assert_eq!(back.loaded_from_schema, 1);

        // Version 2: no envelope, but energy fields present.
        let mut v2 = state_fixture();
        v2.eco_detritus = 12.5;
        let bytes = serde_json::to_vec(&v2).unwrap();
        let back = from_bytes(&bytes).expect("a v2 save must still load");
        assert_eq!(back.loaded_from_schema, 2);
        assert_eq!(back.eco_detritus, 12.5);
    }

    #[test]
    fn every_supported_envelope_schema_still_loads() {
        for schema in MIN_SUPPORTED_SCHEMA..=SCHEMA_VERSION {
            let mut state = serde_json::to_value(state_fixture()).expect("state value");
            let object = state.as_object_mut().expect("state object");
            if schema < 4 {
                object.remove("dormant_cohorts");
            }
            if schema < 5 {
                object.remove("experiment");
                object.remove("resource_field_phase");
            }
            if schema < 6 {
                object.remove("evolution_worker");
            }
            if schema < 7 {
                object.remove("shared_learning");
            }
            let text = serde_json::to_string(&state).expect("state text");
            let envelope = SnapshotEnvelope {
                schema_version: schema,
                build_provenance: BuildProvenance::current(),
                checksum: checksum_bytes(&text),
                state: serde_json::value::RawValue::from_string(text).expect("raw state"),
            };
            let bytes = serde_json::to_vec(&envelope).expect("serialize");

            let loaded = from_bytes(&bytes).expect("supported schema");
            assert_eq!(loaded.loaded_from_schema, schema);
        }
    }

    #[test]
    fn a_newer_schema_is_refused_with_a_useful_message() {
        let mut envelope = SnapshotEnvelope::seal(state_fixture()).expect("seal");
        envelope.schema_version = SCHEMA_VERSION + 1;
        let bytes = serde_json::to_vec(&envelope).unwrap();
        match from_bytes(&bytes) {
            Err(e @ SnapshotError::TooNew { .. }) => {
                assert!(e.to_string().contains("newer engine"));
            }
            other => panic!("expected TooNew, got {other:?}"),
        }
    }

    #[test]
    fn a_schema_older_than_n_minus_2_is_refused() {
        let mut envelope = SnapshotEnvelope::seal(state_fixture()).expect("seal");
        envelope.schema_version = MIN_SUPPORTED_SCHEMA - 1;
        let bytes = serde_json::to_vec(&envelope).unwrap();
        match from_bytes(&bytes) {
            Err(SnapshotError::TooOld { found, minimum }) => {
                assert_eq!(found, MIN_SUPPORTED_SCHEMA - 1);
                assert_eq!(minimum, MIN_SUPPORTED_SCHEMA);
            }
            other => panic!("expected TooOld, got {other:?}"),
        }
    }

    #[test]
    fn garbage_is_rejected_rather_than_deserialized_into_a_plausible_world() {
        match from_bytes(b"{\"not\": \"a snapshot\"}") {
            Err(SnapshotError::Malformed(_)) => {}
            other => panic!("expected Malformed, got {other:?}"),
        }
        match from_bytes(b"\x00\x01\x02 not json at all") {
            Err(SnapshotError::Malformed(_)) => {}
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn a_checksummed_snapshot_with_an_empty_agent_body_is_rejected_before_restore() {
        let mut state = state_fixture();
        let mut agent = agent_fixture();
        agent.genotype = Default::default();
        state.agents.push(agent);
        let bytes = serde_json::to_vec(&unchecked_envelope(state)).unwrap();

        match from_bytes(&bytes) {
            Err(SnapshotError::InvalidState(message)) => {
                assert!(message.contains("agent 0"), "{message}");
                assert!(message.contains("root node"), "{message}");
            }
            other => panic!("expected invalid agent rejection, got {other:?}"),
        }
    }

    #[test]
    fn a_checksummed_snapshot_with_overflowing_homeostasis_is_rejected() {
        let mut state = state_fixture();
        let mut agent = agent_fixture();
        agent.homeostatic_state.energy = f32::MAX;
        state.agents.push(agent);
        let bytes = serde_json::to_vec(&unchecked_envelope(state)).unwrap();

        match from_bytes(&bytes) {
            Err(SnapshotError::InvalidState(message)) => {
                assert!(message.contains("homeostatic"), "{message}");
            }
            other => panic!("expected invalid homeostasis, got {other:?}"),
        }
    }

    #[test]
    fn a_checksummed_snapshot_with_a_degenerate_root_rotation_is_rejected() {
        let mut state = state_fixture();
        let mut agent = agent_fixture();
        agent.root_rotation = glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
        state.agents.push(agent);
        let bytes = serde_json::to_vec(&unchecked_envelope(state)).unwrap();

        match from_bytes(&bytes) {
            Err(SnapshotError::InvalidState(message)) => {
                assert!(message.contains("root rotation"), "{message}");
            }
            other => panic!("expected invalid rotation, got {other:?}"),
        }
    }

    #[test]
    fn a_checksummed_snapshot_with_overflow_prone_kinematics_is_rejected() {
        let mut state = state_fixture();
        let mut agent = agent_fixture();
        agent.root_position = glam::Vec3::new(f32::MAX, 0.0, 0.0);
        state.agents.push(agent);
        let bytes = serde_json::to_vec(&unchecked_envelope(state)).unwrap();

        match from_bytes(&bytes) {
            Err(SnapshotError::InvalidState(message)) => {
                assert!(message.contains("numeric safety"), "{message}");
            }
            other => panic!("expected unsafe kinematics rejection, got {other:?}"),
        }
    }

    fn evolution_worker_fixture(
        run_seed: u64,
    ) -> crate::core::simulation_state::SavedEvolutionWorkerState {
        crate::core::simulation_state::SavedEvolutionWorkerState {
            rng_seed: crate::core::resources::derived_seed(
                run_seed,
                crate::core::resources::sim_stream::EVOLUTION,
            ),
            rng_pos: 0,
            node_id_counter: 3,
            meta_ai_epoch: 0,
            meta_ai_history: Vec::new(),
            chronicle_ids_issued: 0,
            offspring_ids_issued: 0,
            archive: crate::evolution::map_elites::SavedMapElitesArchive {
                grid_resolution: 0.25,
                elites: Vec::new(),
            },
        }
    }

    #[test]
    fn snapshots_reject_counters_that_cannot_advance_without_overflow() {
        let rejects = |state: SavedSimulationState, label: &str| {
            assert!(
                matches!(
                    SnapshotEnvelope::seal(state),
                    Err(SnapshotError::InvalidState(_))
                ),
                "{label} must be rejected before a restored tick can panic or wrap"
            );
        };

        let mut state = state_fixture();
        state.tick_count = u64::MAX;
        rejects(state, "simulation tick counter");

        let mut state = state_fixture();
        state.epoch_manager.current_epoch_ticks = u64::MAX;
        rejects(state, "epoch tick counter");

        let mut state = state_fixture();
        state.epoch_manager.current_epoch = u32::MAX;
        rejects(state, "epoch counter");

        let mut state = state_fixture();
        let mut agent = agent_fixture();
        agent.evaluation.survival_ticks = u32::MAX;
        state.agents.push(agent);
        rejects(state, "agent survival counter");

        let mut state = state_fixture();
        let mut agent = agent_fixture();
        agent.feature_tracker.tick_count = u32::MAX;
        state.agents.push(agent);
        rejects(state, "agent feature counter");

        let mut state = state_fixture();
        let mut agent = agent_fixture();
        agent.generation = u32::MAX;
        state.agents.push(agent);
        rejects(state, "agent generation");

        let worker_state = || {
            let mut state = state_fixture();
            state.sim_rng_seed = 1_337;
            state.evolution_worker = Some(evolution_worker_fixture(state.sim_rng_seed));
            state
        };

        let mut state = worker_state();
        state
            .evolution_worker
            .as_mut()
            .expect("worker fixture")
            .node_id_counter = u32::MAX;
        rejects(state, "evolution morphology-node counter");

        let mut state = worker_state();
        state
            .evolution_worker
            .as_mut()
            .expect("worker fixture")
            .meta_ai_epoch = u32::MAX;
        rejects(state, "Meta-AI epoch counter");

        let mut state = worker_state();
        state
            .evolution_worker
            .as_mut()
            .expect("worker fixture")
            .chronicle_ids_issued = u64::MAX;
        rejects(state, "chronicle identity counter");

        let mut state = worker_state();
        state
            .evolution_worker
            .as_mut()
            .expect("worker fixture")
            .offspring_ids_issued = u64::MAX;
        rejects(state, "offspring identity counter");

        let mut state = worker_state();
        state
            .evolution_worker
            .as_mut()
            .expect("worker fixture")
            .archive
            .elites
            .push(crate::evolution::map_elites::SavedEliteIndividual {
                bin_x: 0,
                bin_y: 0,
                genotype: agent_fixture().genotype,
                fitness: 1.0,
                features: vec![0.0, 0.0],
                lineage_id: "exhausted-elite".to_owned(),
                generation: u32::MAX,
            });
        rejects(state, "archived elite generation");
    }

    #[test]
    fn serialized_agent_rejects_a_missing_child_state() {
        let mut agent = two_segment_agent_fixture();
        agent.segments.clear();
        assert!(matches!(
            agent.validate(),
            Err(
                crate::core::simulation_state::SerializedAgentError::SegmentStateCount {
                    found: 0,
                    expected: 1
                }
            )
        ));
    }

    #[test]
    fn serialized_agent_rejects_duplicate_child_states() {
        use crate::evolution::genotype::{MorphologyEdge, MorphologyNode};
        let mut agent = two_segment_agent_fixture();
        agent.genotype.add_node(MorphologyNode {
            id: 2,
            length: 0.75,
            radius: 0.2,
            mass: 0.5,
        });
        agent.genotype.add_edge(MorphologyEdge {
            source_node: 0,
            target_node: 2,
            joint_anchor: glam::Vec3::X,
            joint_axis: glam::Vec3::Y,
        });
        agent.segments.push(agent.segments[0].clone());
        assert!(matches!(
            agent.validate(),
            Err(crate::core::simulation_state::SerializedAgentError::DuplicateSegment { id: 1 })
        ));
    }

    #[test]
    fn serialized_agent_rejects_the_root_repeated_as_a_child() {
        let mut agent = two_segment_agent_fixture();
        agent.segments[0].segment_id = 0;
        assert!(matches!(
            agent.validate(),
            Err(crate::core::simulation_state::SerializedAgentError::RootRepeatedAsChild { id: 0 })
        ));
    }

    #[test]
    fn serialized_agent_rejects_an_unknown_child() {
        let mut agent = two_segment_agent_fixture();
        agent.segments[0].segment_id = 99;
        assert!(matches!(
            agent.validate(),
            Err(crate::core::simulation_state::SerializedAgentError::UnknownSegment { id: 99 })
        ));
    }

    #[test]
    fn serialized_agent_rejects_invalid_child_kinematics() {
        let mut agent = two_segment_agent_fixture();
        agent.segments[0].rotation = glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
        assert!(matches!(
            agent.validate(),
            Err(
                crate::core::simulation_state::SerializedAgentError::InvalidSegmentKinematics {
                    id: 1
                }
            )
        ));
    }

    #[test]
    fn serialized_agent_rejects_a_non_finite_oscillator() {
        let mut agent = two_segment_agent_fixture();
        agent.segments[0]
            .oscillator
            .as_mut()
            .expect("oscillator")
            .phase = f32::NAN;
        assert!(matches!(
            agent.validate(),
            Err(crate::core::simulation_state::SerializedAgentError::InvalidOscillator { id: 1 })
        ));
    }

    #[test]
    fn read_rejects_an_oversized_sparse_file_before_allocating_it() {
        let path = std::env::temp_dir().join(format!(
            "anima_oversized_snapshot_{}.json",
            std::process::id()
        ));
        const TEST_LIMIT: usize = 1024;
        let file = std::fs::File::create(&path).expect("create sparse fixture");
        file.set_len(TEST_LIMIT as u64 + 1)
            .expect("extend sparse fixture");
        drop(file);

        let result = read_with_limit(&path, TEST_LIMIT);
        let _ = std::fs::remove_file(path);
        assert!(matches!(result, Err(SnapshotError::TooLarge { .. })));
    }

    #[test]
    fn seal_rejects_a_dormant_grid_whose_dimensions_overflow() {
        let mut state = state_fixture();
        state.dormant_cohorts = Some(crate::core::aggregate_population::SavedDormantCohorts {
            chunks: Vec::new(),
            grid: usize::MAX,
            min_x: -100.0,
            min_z: -100.0,
            max_x: 100.0,
            max_z: 100.0,
            dwell_ticks: 1,
            archive_cap: 1,
            rehydrate_per_tick: 1,
            rng_seed: 1,
            rng_pos: 0,
            dehydrated: 0,
            rehydrated: 0,
            genomes_dropped: 0,
            spilled: 0.0,
        });

        assert!(matches!(
            SnapshotEnvelope::seal(state),
            Err(SnapshotError::InvalidState(_))
        ));
    }

    #[test]
    fn seal_rejects_a_food_cap_that_would_create_an_unbounded_spawn_burst() {
        let mut state = state_fixture();
        state.food_spawn_settings.max_food_count = usize::MAX;
        assert!(matches!(
            SnapshotEnvelope::seal(state),
            Err(SnapshotError::InvalidState(_))
        ));
    }

    #[test]
    fn seal_rejects_evolution_settings_that_would_panic_the_rng() {
        let mut state = state_fixture();
        state.evolution_settings.mutation_rate = f64::NAN;
        assert!(matches!(
            SnapshotEnvelope::seal(state),
            Err(SnapshotError::InvalidState(_))
        ));
    }

    #[test]
    fn write_atomic_leaves_no_temp_file_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("anima_snap_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("run.anima.json");

        let mut state = state_fixture();
        state.tick_count = 7;
        write_atomic(&path, &SnapshotEnvelope::seal(state).unwrap()).expect("write");

        let back = read(&path).expect("read");
        assert_eq!(back.tick_count, 7);

        let strays: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(strays.is_empty(), "temp files left behind: {strays:?}");

        // Overwriting an existing snapshot must also work, and must leave the new content.
        let mut second = state_fixture();
        second.tick_count = 99;
        write_atomic(&path, &SnapshotEnvelope::seal(second).unwrap()).expect("overwrite");
        assert_eq!(read(&path).expect("read again").tick_count, 99);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
