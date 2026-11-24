//! Off-thread asynchronous segment persistence and atomic directory recovery for WR2 (ADR-0008 H2).
//!
//! Writes `header.json` and sealed `segment_{index:06}.json` files via
//! [`crate::core::snapshot::write_bytes_atomic`] (`write_all` -> `flush` -> `sync_all` -> `rename`),
//! off the deterministic simulation tick thread.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle, Thread};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::snapshot::write_bytes_atomic;

use super::chain::verify_record;
use super::ring::{spsc_ring, RingConsumer, RingEntry, RingProducer};
use super::types::{
    hex32, RecordedEvent, RecordedInput, SegmentBody, SegmentHash, SignedSegment,
    VerificationReport, WorldRecord, WorldRecordError, WorldRecordHeader,
};

/// Default pre-allocated ring-buffer capacity (`4,096` slots; wait-free SPSC).
pub const DEFAULT_RING_CAPACITY: usize = 4_096;

/// Default per-segment input limit applied by the background writer thread.
pub const DEFAULT_WRITER_MAX_INPUTS_PER_SEGMENT: usize = 1_024;

/// Default per-segment event limit applied by the background writer thread.
pub const DEFAULT_WRITER_MAX_EVENTS_PER_SEGMENT: usize = 4_096;

/// On-disk envelope for `<output_dir>/header.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeaderDiskEnvelope {
    pub format: String,
    pub schema_version: u32,
    pub header: WorldRecordHeader,
    #[serde(with = "hex32")]
    pub header_hash: SegmentHash,
}

/// Configuration for spawning the off-thread WR2 writer.
#[derive(Clone, Debug)]
pub struct WorldRecordWriterConfig {
    /// Fixed number of `RingEntry` slots reserved up front (`>= 2`).
    pub ring_capacity: usize,
    /// Optional output directory where `header.json` and `segment_{index:06}.json` are written atomically.
    pub output_dir: Option<PathBuf>,
    /// Upper bound on inputs assembled into one sealed segment.
    pub max_inputs_per_segment: usize,
    /// Upper bound on events assembled into one sealed segment.
    pub max_events_per_segment: usize,
    /// Optional test hook pausing the writer loop without losing shutdown drain guarantees.
    pub paused: Option<Arc<AtomicBool>>,
}

impl Default for WorldRecordWriterConfig {
    fn default() -> Self {
        Self {
            ring_capacity: DEFAULT_RING_CAPACITY,
            output_dir: None,
            max_inputs_per_segment: DEFAULT_WRITER_MAX_INPUTS_PER_SEGMENT,
            max_events_per_segment: DEFAULT_WRITER_MAX_EVENTS_PER_SEGMENT,
            paused: None,
        }
    }
}

impl WorldRecordWriterConfig {
    /// Larger declarative ring and per-segment input/event bounds sized for shared-island WR2
    /// streams (`ADR-0014` §4).
    pub fn island_default(output_dir: Option<PathBuf>) -> Self {
        Self {
            ring_capacity: 16_384,
            output_dir,
            max_inputs_per_segment: 4_096,
            max_events_per_segment: 8_192,
            paused: None,
        }
    }
}

struct WriterLoopParams {
    header: WorldRecordHeader,
    consumer: RingConsumer<RingEntry>,
    output_dir: Option<PathBuf>,
    max_inputs_per_segment: usize,
    max_events_per_segment: usize,
    shutdown: Arc<AtomicBool>,
    paused: Option<Arc<AtomicBool>>,
}

/// Owns the dedicated WR2 background writer thread cleanly across drop and explicit `finish`.
pub struct AsyncWriterHandle {
    shutdown: Arc<AtomicBool>,
    thread: Thread,
    join_handle: Option<JoinHandle<Result<WorldRecord, WorldRecordError>>>,
}

impl AsyncWriterHandle {
    /// Validate `header`, optionally write `header.json` atomically, and spawn the writer thread.
    pub fn spawn(
        header: WorldRecordHeader,
        config: WorldRecordWriterConfig,
    ) -> Result<(RingProducer<RingEntry>, Self), WorldRecordError> {
        let initial_record = WorldRecord::build_empty(header.clone())?;
        if let Some(ref dir) = config.output_dir {
            let env = HeaderDiskEnvelope {
                format: initial_record.format.clone(),
                schema_version: initial_record.schema_version,
                header: initial_record.header.clone(),
                header_hash: initial_record.header_hash,
            };
            let json = serde_json::to_vec_pretty(&env)
                .map_err(|err| WorldRecordError::Serialization(err.to_string()))?;
            write_bytes_atomic(&dir.join("header.json"), &json)
                .map_err(|err| WorldRecordError::Io(err.to_string()))?;
        }

        let (producer, consumer) = spsc_ring::<RingEntry>(config.ring_capacity.max(2));
        let shutdown = Arc::new(AtomicBool::new(false));
        let params = WriterLoopParams {
            header: initial_record.header,
            consumer,
            output_dir: config.output_dir,
            max_inputs_per_segment: config.max_inputs_per_segment,
            max_events_per_segment: config.max_events_per_segment,
            shutdown: Arc::clone(&shutdown),
            paused: config.paused,
        };
        let join_handle = thread::Builder::new()
            .name("anima-world-record-writer".to_string())
            .spawn(move || run_writer_loop(params))
            .map_err(|err| WorldRecordError::Io(format!("failed to spawn WR2 writer: {err}")))?;
        let thread = join_handle.thread().clone();

        Ok((
            producer,
            Self {
                shutdown,
                thread,
                join_handle: Some(join_handle),
            },
        ))
    }

    /// Borrow the writer thread handle so producers can call `unpark()` without allocating.
    pub fn writer_thread(&self) -> &Thread {
        &self.thread
    }

    /// Request graceful shutdown, drain all queued ring entries, join the thread, and yield the record.
    pub fn finish(&mut self) -> Result<WorldRecord, WorldRecordError> {
        self.shutdown.store(true, Ordering::Release);
        self.thread.unpark();
        let handle = self
            .join_handle
            .take()
            .ok_or_else(|| WorldRecordError::Serialization("writer already finished".into()))?;
        match handle.join() {
            Ok(result) => result,
            Err(_) => Err(WorldRecordError::Io(
                "world record writer thread panicked".to_string(),
            )),
        }
    }
}

impl Drop for AsyncWriterHandle {
    fn drop(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            self.shutdown.store(true, Ordering::Release);
            self.thread.unpark();
            let _ = handle.join();
        }
    }
}

fn run_writer_loop(mut params: WriterLoopParams) -> Result<WorldRecord, WorldRecordError> {
    let mut record = WorldRecord::build_empty(params.header)?;
    let mut pending_inputs: Vec<RecordedInput> = Vec::new();
    let mut pending_events: Vec<RecordedEvent> = Vec::new();
    let mut writer_dropped_inputs = 0u64;
    let mut writer_dropped_events = 0u64;
    let mut first_error: Option<WorldRecordError> = None;

    loop {
        let shutting_down = params.shutdown.load(Ordering::Acquire);
        if !shutting_down
            && params
                .paused
                .as_deref()
                .is_some_and(|p| p.load(Ordering::Acquire))
        {
            thread::park_timeout(Duration::from_millis(2));
            continue;
        }

        let mut popped_any = false;
        while let Some(entry) = params.consumer.try_pop() {
            popped_any = true;
            if let RingEntry::SegmentSeal {
                tick_start,
                tick_end,
                end_checksum,
                keyframe,
                dropped_inputs_since_prev,
                dropped_events_since_prev,
            } = entry
            {
                let mut inputs = std::mem::take(&mut pending_inputs);
                let mut events = std::mem::take(&mut pending_events);
                inputs.sort_by_key(|i| i.tick);
                events.sort_by_key(|e| e.tick);
                let body = SegmentBody {
                    index: record.segments.len() as u64,
                    tick_start,
                    tick_end,
                    end_checksum,
                    keyframe,
                    keyframe_snapshot_sha256: None,
                    inputs,
                    events,
                    dropped_inputs_since_prev: dropped_inputs_since_prev
                        .saturating_add(std::mem::take(&mut writer_dropped_inputs)),
                    dropped_events_since_prev: dropped_events_since_prev
                        .saturating_add(std::mem::take(&mut writer_dropped_events)),
                };
                if first_error.is_none() {
                    match record.append_segment(body) {
                        Ok(sealed) => {
                            if let Some(ref dir) = params.output_dir {
                                match serde_json::to_vec_pretty(sealed) {
                                    Ok(json) => {
                                        let seg_path = dir
                                            .join(format!("segment_{:06}.json", sealed.body.index));
                                        if let Err(err) = write_bytes_atomic(&seg_path, &json) {
                                            first_error =
                                                Some(WorldRecordError::Io(err.to_string()));
                                        }
                                    }
                                    Err(err) => {
                                        first_error =
                                            Some(WorldRecordError::Serialization(err.to_string()));
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            first_error = Some(err);
                        }
                    }
                }
            } else if let Some(input) = entry.into_recorded_input() {
                if pending_inputs.len() >= params.max_inputs_per_segment {
                    writer_dropped_inputs = writer_dropped_inputs.saturating_add(1);
                } else {
                    pending_inputs.push(input);
                }
            } else if let Some(event) = entry.into_recorded_event() {
                if pending_events.len() >= params.max_events_per_segment {
                    writer_dropped_events = writer_dropped_events.saturating_add(1);
                } else {
                    pending_events.push(event);
                }
            }
        }

        if shutting_down {
            break;
        }
        if !popped_any {
            thread::park_timeout(Duration::from_millis(2));
        }
    }

    if let Some(err) = first_error {
        return Err(err);
    }
    Ok(record)
}

fn parse_segment_file_index(file_name: &str) -> Option<u64> {
    let middle = file_name.strip_prefix("segment_")?.strip_suffix(".json")?;
    if middle.is_empty() || !middle.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    middle.parse::<u64>().ok()
}

/// Reconstruct and verify a persisted World Record directory (`header.json` + `segment_*.json`).
///
/// Unfinished temp files (`.*.tmp`) left behind by interrupted atomic writes are ignored cleanly.
pub fn load_and_verify_record_dir(
    dir: &Path,
) -> Result<(WorldRecord, VerificationReport), WorldRecordError> {
    let header_path = dir.join("header.json");
    let header_bytes = std::fs::read(&header_path)
        .map_err(|err| WorldRecordError::Io(format!("read {}: {err}", header_path.display())))?;
    let env: HeaderDiskEnvelope = serde_json::from_slice(&header_bytes)
        .map_err(|err| WorldRecordError::Serialization(err.to_string()))?;

    let entries = std::fs::read_dir(dir)
        .map_err(|err| WorldRecordError::Io(format!("read_dir {}: {err}", dir.display())))?;
    let mut segment_paths: Vec<(u64, PathBuf)> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| {
            WorldRecordError::Io(format!("dir entry in {}: {err}", dir.display()))
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Some(idx) = parse_segment_file_index(name) {
            segment_paths.push((idx, path));
        }
    }
    segment_paths.sort_unstable_by_key(|(idx, _)| *idx);

    let mut segments: Vec<SignedSegment> = Vec::with_capacity(segment_paths.len());
    for (_idx, path) in segment_paths {
        let bytes = std::fs::read(&path)
            .map_err(|err| WorldRecordError::Io(format!("read {}: {err}", path.display())))?;
        let seg: SignedSegment = serde_json::from_slice(&bytes)
            .map_err(|err| WorldRecordError::Serialization(err.to_string()))?;
        segments.push(seg);
    }

    let record = WorldRecord {
        format: env.format,
        schema_version: env.schema_version,
        header: env.header,
        header_hash: env.header_hash,
        segments,
    };
    let report = verify_record(&record)?;
    Ok((record, report))
}
