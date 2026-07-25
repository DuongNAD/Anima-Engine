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
use std::io::Write;
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
pub const SCHEMA_VERSION: u32 = 3;

/// Oldest schema this build can still read. N−2 per the G1.2 requirement.
pub const MIN_SUPPORTED_SCHEMA: u32 = SCHEMA_VERSION - 2;

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
    /// Raw, rather than a typed `SavedSimulationState`, for a reason that cost an afternoon:
    /// **serde_json's `f64` round trip is not bit-exact.** A saved `eco_animals` of
    /// `990.5102615356445` reads back as `990.5102615356444`. A checksum computed by
    /// *re-serializing* the parsed state therefore disagrees with the one computed before writing,
    /// and a perfectly good file fails its own integrity check.
    ///
    /// Holding the state as [`RawValue`] means the bytes that were hashed, the bytes on disk and
    /// the bytes that get verified are all literally the same bytes. It also makes the checksum
    /// immune to map iteration order, since whatever order was written is the order that is hashed.
    /// The state still appears as a normal nested object in the file, not an escaped string.
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
        let text = serde_json::to_string(&state)
            .map_err(|e| SnapshotError::Malformed(format!("state is not serializable: {e}")))?;
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

    let json = serde_json::to_vec_pretty(envelope)
        .map_err(|e| SnapshotError::Malformed(format!("envelope is not serializable: {e}")))?;

    {
        let mut file = std::fs::File::create(&tmp_path)
            .map_err(|e| SnapshotError::Io(format!("could not create temp file: {e}")))?;
        file.write_all(&json)
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
    let bytes = std::fs::read(path)
        .map_err(|e| SnapshotError::Io(format!("could not read {}: {e}", path.display())))?;
    from_bytes(&bytes)
}

/// The parsing half of [`read`], separated so it can be tested without touching a filesystem.
pub fn from_bytes(bytes: &[u8]) -> Result<SavedSimulationState, SnapshotError> {
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
        let state = envelope.parse_state()?;
        return Ok(migrate(envelope.schema_version, state));
    }

    // No `schema_version` key: a pre-envelope file. Those are schema 1 or 2, both inside the N−2
    // window, and both deserialize into the current struct via `#[serde(default)]`.
    let state: SavedSimulationState = serde_json::from_slice(bytes).map_err(|e| {
        SnapshotError::Malformed(format!(
            "not an envelope and not a bare saved state either: {e}"
        ))
    })?;
    Ok(migrate(legacy_schema_of(&state), state))
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

    // Standing crop, cell by cell.
    if let Some(field) = world.get_resource::<crate::core::ecology::ResourceField>() {
        bytes.extend_from_slice(&(field.r.len() as u32).to_le_bytes());
        for v in &field.r {
            bytes.extend_from_slice(&v.to_bits().to_le_bytes());
        }
    }

    // Closed-energy compartments.
    if let Some(pool) = world.get_resource::<crate::core::ecology::EcosystemBiomass>() {
        push_f64(&mut bytes, pool.detritus);
        push_f64(&mut bytes, pool.plants);
        push_f64(&mut bytes, pool.animals);
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

    fn state_fixture() -> SavedSimulationState {
        crate::core::simulation_state::empty_saved_state_for_tests()
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
        // serde_json`s f64 round trip is not bit-exact, so a checksum recomputed from a parsed
        // state can disagree with the one written beside it. Hashing the raw bytes makes a file
        // that was written correctly always verify.
        let mut state = state_fixture();
        state.eco_animals = 990.5102615356445;
        let envelope = SnapshotEnvelope::seal(state).expect("seal");
        let bytes = serde_json::to_vec_pretty(&envelope).unwrap();
        let back: SnapshotEnvelope = serde_json::from_slice(&bytes).unwrap();
        back.verify().expect("a file written correctly must verify");
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
