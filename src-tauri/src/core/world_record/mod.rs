//! World Record v1 (ADR-0008 / WR1) — hash-chained inputs, events, and checkpoints.

pub mod chain;
pub mod encoding;
pub mod system;
pub mod types;

pub use chain::{
    compute_segment_hash, encode_record_json, genesis_anchor, header_digest, validate_header,
    verify_record, verify_record_json,
};
pub use encoding::canonical_encode;
pub use system::{
    record_world_record_tick_system, RecordReadParams, WorldRecordRecorder,
    DEFAULT_MAX_EVENTS_PER_SEGMENT, DEFAULT_MAX_INPUTS_PER_SEGMENT,
};
pub use types::{
    hex_decode_32, hex_encode, DeathCauseTag, InputKind, PlatformClass, RecordedEvent,
    RecordedInput, SegmentBody, SegmentHash, SignedSegment, VerificationReport, WorldEvent,
    WorldKeyframe, WorldRecord, WorldRecordError, WorldRecordHeader, GENESIS_HASH,
    WORLD_RECORD_FORMAT, WORLD_RECORD_SCHEMA_VERSION,
};
