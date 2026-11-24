//! Data types for the Anima World Record format (ADR-0008 / WR1).
//!
//! A world record is a self-describing, forward-chained provenance log of a simulation run:
//! a pinned [`WorldRecordHeader`] followed by contiguous [`SignedSegment`] slices covering
//! `[tick_start, tick_end]`. Every segment carries an `inputs` log, typed `WorldEvent` entries,
//! a closed-EU `WorldKeyframe`, an authoritative `end_checksum`, overflow drop counters
//! (`dropped_inputs_since_prev`, `dropped_events_since_prev`), and a SHA-256 link back to the
//! previous segment (or the header anchor for segment `0`).

use std::fmt;

use crate::core::causal::CauseId;
use crate::core::intervention::InterventionCommand;
use crate::core::observer::ObserverAction;
use crate::core::snapshot::BuildProvenance;
use crate::core::world_artifact::WorldIdentity;
use serde::{Deserialize, Serialize};

/// Envelope format tag stored on serialized [`WorldRecord`] files.
pub const WORLD_RECORD_FORMAT: &str = "anima.world_record";

/// Wire schema version for [`WorldRecord`] and [`WorldRecordHeader`].
pub const WORLD_RECORD_SCHEMA_VERSION: u32 = 1;

/// Fixed 32-byte zero constant mixed into segment `0`'s genesis anchor.
pub const GENESIS_HASH: [u8; 32] = [0u8; 32];

/// 32-byte SHA-256 digest used for header anchoring and segment hash chaining.
pub type SegmentHash = [u8; 32];

/// Encode raw bytes as lowercase hexadecimal.
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Decode a 64-character lowercase/uppercase hexadecimal SHA-256 string into 32 bytes.
pub fn hex_decode_32(s: &str) -> Result<SegmentHash, WorldRecordError> {
    if s.len() != 64 {
        return Err(WorldRecordError::InvalidHexDigest {
            expected_len: 64,
            actual_len: s.len(),
        });
    }
    let bytes = s.as_bytes();
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = hex_nibble(bytes[i * 2])?;
        let lo = hex_nibble(bytes[i * 2 + 1])?;
        *slot = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, WorldRecordError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(WorldRecordError::InvalidHexCharacter(b as char)),
    }
}

pub(crate) mod hex32 {
    use super::{hex_decode_32, hex_encode, SegmentHash};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(digest: &SegmentHash, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex_encode(digest))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SegmentHash, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex_decode_32(&s).map_err(serde::de::Error::custom)
    }
}

pub(crate) mod opt_hex32 {
    use super::{hex_decode_32, hex_encode, SegmentHash};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(digest: &Option<SegmentHash>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match digest {
            Some(bytes) => serializer.serialize_some(&hex_encode(bytes)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SegmentHash>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        opt.map(|s| hex_decode_32(&s).map_err(serde::de::Error::custom))
            .transpose()
    }
}

/// Host OS and CPU architecture class where the recording binary executed (ADR-0008 §2a).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformClass {
    pub os: String,
    pub arch: String,
}

impl PlatformClass {
    /// Detect the running host OS and architecture class.
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        }
    }
}

/// Sealed provenance header written once at genesis and anchored into segment `0`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldRecordHeader {
    pub schema_version: u32,
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub fork_tick: Option<u64>,
    #[serde(with = "opt_hex32")]
    pub fork_keyframe_hash: Option<SegmentHash>,
    pub manifest_fingerprint: Option<u64>,
    pub law_fingerprint: Option<u64>,
    pub registry_fingerprint: Option<u64>,
    pub seed: u64,
    pub world_identity: WorldIdentity,
    pub world_artifact_sha256: Option<String>,
    pub model_version: String,
    pub snapshot_schema_version: u32,
    pub provenance: BuildProvenance,
    pub platform_class: PlatformClass,
    /// `false` records are readable provenance archives, not guaranteed replayable (ADR-0008).
    pub deterministic: bool,
    pub initial_world_checksum: u32,
    /// Target cadence (ticks per segment); must be `>= 1`.
    pub segment_ticks: u64,
    /// Canonically sorted initial `(key, value)` manifest pairs.
    pub initial_conditions: Vec<(String, f64)>,
    /// Engine compatibility era (`0` for a fresh continuous run).
    pub era_index: u32,
    /// Tail hash of the preceding era when continuing across a schema/law boundary (`era_index > 0`).
    #[serde(with = "opt_hex32")]
    pub prev_era_tail_hash: Option<SegmentHash>,
    /// Explicit list of out-of-band state excluded from this recording (e.g. GPU microstate).
    pub omissions: Vec<String>,
}

/// External input recorded at the tick it acted on the world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedInput {
    pub tick: u64,
    pub cause_id: CauseId,
    pub kind: InputKind,
}

/// Typed input sources supported by World Record v1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "input_type", rename_all = "snake_case")]
pub enum InputKind {
    Intervention(InterventionCommand),
    ObserverAction(ObserverAction),
    ChronicleNote {
        event_type: String,
        title: String,
        description: String,
    },
}

/// Normalized v1 death causes required by ADR-0008 (`starvation|dehydration|predation|reclaim|epoch_replacement`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathCauseTag {
    Starvation,
    Dehydration,
    Predation,
    Reclaim,
    EpochReplacement,
}

/// An observable discrete event appended within a segment at `tick`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordedEvent {
    pub tick: u64,
    pub kind: WorldEvent,
}

/// The closed six-variant World Record v1 event set (ADR-0008).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum WorldEvent {
    Birth {
        agent_id: String,
        parent_id: Option<String>,
        generation: u32,
        energy_transferred: f64,
    },
    Death {
        agent_id: String,
        cause: DeathCauseTag,
        age_ticks: u64,
    },
    Kill {
        predator_id: String,
        prey_id: String,
        damage: f64,
        energy_transferred: f64,
    },
    Migration {
        agent_id: String,
        from_shard: String,
        to_shard: String,
    },
    Chronicle {
        category: String,
        summary: String,
    },
    EpochReplacement {
        epoch: u32,
        replaced_count: u32,
    },
}

/// Macroscopic EU and population summary checkpoint at `tick_end`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldKeyframe {
    pub tick: u64,
    pub agent_count: u32,
    pub predator_count: u32,
    pub prey_count: u32,
    pub standing_crop_eu: f64,
    pub detritus_eu: f64,
    pub total_agent_energy_eu: f64,
    pub total_births: u64,
    pub total_deaths: u64,
}

/// Canonical payload hashed into a segment's digest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SegmentBody {
    pub index: u64,
    pub tick_start: u64,
    pub tick_end: u64,
    pub end_checksum: u32,
    pub keyframe: WorldKeyframe,
    /// Optional SHA-256 of an out-of-band snapshot envelope at `tick_end` (reserved for H3).
    #[serde(with = "opt_hex32")]
    pub keyframe_snapshot_sha256: Option<SegmentHash>,
    pub inputs: Vec<RecordedInput>,
    pub events: Vec<RecordedEvent>,
    pub dropped_inputs_since_prev: u64,
    pub dropped_events_since_prev: u64,
}

/// One hash-linked segment in [`WorldRecord::segments`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignedSegment {
    #[serde(with = "hex32")]
    pub prev_hash: SegmentHash,
    pub body: SegmentBody,
    #[serde(with = "hex32")]
    pub hash: SegmentHash,
}

/// Top-level forward-chained World Record document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldRecord {
    pub format: String,
    pub schema_version: u32,
    pub header: WorldRecordHeader,
    #[serde(with = "hex32")]
    pub header_hash: SegmentHash,
    pub segments: Vec<SignedSegment>,
}

/// Summary returned by chain verification when every hash and invariant holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationReport {
    pub segment_count: usize,
    pub first_tick: Option<u64>,
    pub last_tick: Option<u64>,
    pub total_inputs: usize,
    pub total_events: usize,
    pub total_dropped_inputs: u64,
    pub total_dropped_events: u64,
    pub tail_hash: SegmentHash,
    pub replayable: bool,
}

impl VerificationReport {
    /// Whether zero inputs and zero events were dropped due to buffer bounds across all segments.
    pub fn is_complete(&self) -> bool {
        self.total_dropped_inputs == 0 && self.total_dropped_events == 0
    }
}

/// Errors raised when encoding, appending to, or verifying a [`WorldRecord`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorldRecordError {
    UnsupportedFormat {
        expected: String,
        actual: String,
    },
    UnsupportedSchemaVersion {
        expected: u32,
        actual: u32,
    },
    EmptyRunId,
    InvalidForkLink,
    InvalidSegmentTicks(u64),
    InvalidEraLink {
        era_index: u32,
    },
    NonFiniteFloat {
        path: String,
    },
    HeaderHashMismatch {
        expected: SegmentHash,
        actual: SegmentHash,
    },
    InvalidSegmentIndex {
        segment_index: usize,
        actual_index: u64,
    },
    InvalidTickRange {
        segment_index: usize,
        tick_start: u64,
        tick_end: u64,
        prev_tick_end: Option<u64>,
    },
    UnsortedTicks {
        segment_index: usize,
        prev_tick: u64,
        tick: u64,
    },
    RecordTickOutOfBounds {
        segment_index: usize,
        record_tick: u64,
        tick_start: u64,
        tick_end: u64,
    },
    BrokenChain {
        segment_index: usize,
        expected_prev_hash: SegmentHash,
        actual_prev_hash: SegmentHash,
    },
    SegmentHashMismatch {
        segment_index: usize,
        expected_hash: SegmentHash,
        actual_hash: SegmentHash,
    },
    InvalidHexDigest {
        expected_len: usize,
        actual_len: usize,
    },
    InvalidHexCharacter(char),
    Serialization(String),
}

impl WorldRecordError {
    /// If the error pinpoints a specific segment in the chain, returns its zero-based index.
    pub fn broken_segment_index(&self) -> Option<usize> {
        match *self {
            Self::InvalidSegmentIndex { segment_index, .. }
            | Self::InvalidTickRange { segment_index, .. }
            | Self::UnsortedTicks { segment_index, .. }
            | Self::RecordTickOutOfBounds { segment_index, .. }
            | Self::BrokenChain { segment_index, .. }
            | Self::SegmentHashMismatch { segment_index, .. } => Some(segment_index),
            _ => None,
        }
    }
}

impl fmt::Display for WorldRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat { expected, actual } => {
                write!(f, "unsupported format '{actual}', expected '{expected}'")
            }
            Self::UnsupportedSchemaVersion { expected, actual } => {
                write!(
                    f,
                    "unsupported world record schema version {actual}, expected {expected}"
                )
            }
            Self::EmptyRunId => {
                write!(f, "world record header run_id must be non-empty")
            }
            Self::InvalidForkLink => {
                write!(
                    f,
                    "world record header parent_run_id and fork_tick must both be set or both absent"
                )
            }
            Self::InvalidSegmentTicks(ticks) => {
                write!(f, "segment_ticks must be >= 1, got {ticks}")
            }
            Self::InvalidEraLink { era_index } => {
                write!(
                    f,
                    "era_index {era_index} requires prev_era_tail_hash when > 0"
                )
            }
            Self::NonFiniteFloat { path } => {
                write!(f, "non-finite floating-point value at '{path}'")
            }
            Self::HeaderHashMismatch { expected, actual } => write!(
                f,
                "header hash mismatch: expected {}, got {}",
                hex_encode(expected),
                hex_encode(actual)
            ),
            Self::InvalidSegmentIndex {
                segment_index,
                actual_index,
            } => write!(
                f,
                "segment {segment_index} carries body.index={actual_index}"
            ),
            Self::InvalidTickRange {
                segment_index,
                tick_start,
                tick_end,
                prev_tick_end,
            } => write!(
                f,
                "segment {segment_index} has invalid tick range [{tick_start}, {tick_end}] \
                 after prev_tick_end={prev_tick_end:?}"
            ),
            Self::UnsortedTicks {
                segment_index,
                prev_tick,
                tick,
            } => write!(
                f,
                "segment {segment_index} has decreasing tick order: {tick} after {prev_tick}"
            ),
            Self::RecordTickOutOfBounds {
                segment_index,
                record_tick,
                tick_start,
                tick_end,
            } => write!(
                f,
                "segment {segment_index} contains entry at tick {record_tick} outside \
                 [{tick_start}, {tick_end}]"
            ),
            Self::BrokenChain {
                segment_index,
                expected_prev_hash,
                actual_prev_hash,
            } => write!(
                f,
                "hash chain broken at segment {segment_index}: expected prev_hash {}, got {}",
                hex_encode(expected_prev_hash),
                hex_encode(actual_prev_hash)
            ),
            Self::SegmentHashMismatch {
                segment_index,
                expected_hash,
                actual_hash,
            } => write!(
                f,
                "segment {segment_index} hash mismatch: expected {}, got {}",
                hex_encode(expected_hash),
                hex_encode(actual_hash)
            ),
            Self::InvalidHexDigest {
                expected_len,
                actual_len,
            } => write!(
                f,
                "invalid SHA-256 hex digest length {actual_len}, expected {expected_len}"
            ),
            Self::InvalidHexCharacter(ch) => {
                write!(f, "invalid hexadecimal character '{ch}' in digest")
            }
            Self::Serialization(msg) => write!(f, "world record serialization error: {msg}"),
        }
    }
}

impl std::error::Error for WorldRecordError {}
