//! Hash chaining, sealing, and verification for [`WorldRecord`].
//!
//! Implements ADR-0008 Option C:
//! - `header_hash = SHA-256(domain_sep("anima.world_record.header.v1") || canonical(header))`
//! - `genesis_anchor = SHA-256(domain_sep("anima.world_record.genesis.v1") || GENESIS_HASH || header_hash)`
//! - `segment_hash[0] = SHA-256(domain_sep("anima.world_record.segment.v1") || genesis_anchor || canonical(body[0]))`
//! - `segment_hash[k] = SHA-256(domain_sep("anima.world_record.segment.v1") || segment_hash[k-1] || canonical(body[k]))`

use crate::core::intervention::{InterventionKind, Region, TraitMod};
use crate::core::observer::ObserverAction;
use crate::core::sha256::sha256;

use super::encoding::canonical_encode;
use super::types::{
    InputKind, SegmentBody, SegmentHash, SignedSegment, VerificationReport, WorldEvent,
    WorldRecord, WorldRecordError, WorldRecordHeader, GENESIS_HASH, WORLD_RECORD_FORMAT,
    WORLD_RECORD_SCHEMA_VERSION,
};

const DOMAIN_HEADER: &[u8] = b"anima.world_record.header.v1\0";
const DOMAIN_GENESIS: &[u8] = b"anima.world_record.genesis.v1\0";
const DOMAIN_SEGMENT: &[u8] = b"anima.world_record.segment.v1\0";

fn check_f64(value: f64, path: &str) -> Result<(), WorldRecordError> {
    if !value.is_finite() {
        return Err(WorldRecordError::NonFiniteFloat {
            path: path.to_string(),
        });
    }
    Ok(())
}

fn check_f32(value: f32, path: &str) -> Result<(), WorldRecordError> {
    if !value.is_finite() {
        return Err(WorldRecordError::NonFiniteFloat {
            path: path.to_string(),
        });
    }
    Ok(())
}

fn validate_header_floats(header: &WorldRecordHeader) -> Result<(), WorldRecordError> {
    for (idx, (key, val)) in header.initial_conditions.iter().enumerate() {
        check_f64(*val, &format!("header.initial_conditions[{idx}].{key}"))?;
    }
    Ok(())
}

fn validate_input_floats(idx: usize, kind: &InputKind) -> Result<(), WorldRecordError> {
    match kind {
        InputKind::Intervention(cmd) => {
            check_f32(cmd.intensity, &format!("body.inputs[{idx}].intensity"))?;
            match cmd.region {
                Region::Radius { cx, cy, r } => {
                    check_f32(cx, &format!("body.inputs[{idx}].region.cx"))?;
                    check_f32(cy, &format!("body.inputs[{idx}].region.cy"))?;
                    check_f32(r, &format!("body.inputs[{idx}].region.r"))?;
                }
                Region::Global | Region::Cell { .. } | Region::Rect { .. } => {}
            }
            match &cmd.kind {
                InterventionKind::ToxicityPlume {
                    center_x,
                    center_y,
                    radius,
                    toxicity_level,
                    decay_rate,
                } => {
                    check_f32(*center_x, &format!("body.inputs[{idx}].kind.center_x"))?;
                    check_f32(*center_y, &format!("body.inputs[{idx}].kind.center_y"))?;
                    check_f32(*radius, &format!("body.inputs[{idx}].kind.radius"))?;
                    check_f32(
                        *toxicity_level,
                        &format!("body.inputs[{idx}].kind.toxicity_level"),
                    )?;
                    check_f32(*decay_rate, &format!("body.inputs[{idx}].kind.decay_rate"))?;
                }
                InterventionKind::PathogenTransmission {
                    transmission_rate,
                    mortality_multiplier,
                    ..
                } => {
                    check_f32(
                        *transmission_rate,
                        &format!("body.inputs[{idx}].kind.transmission_rate"),
                    )?;
                    check_f32(
                        *mortality_multiplier,
                        &format!("body.inputs[{idx}].kind.mortality_multiplier"),
                    )?;
                }
                InterventionKind::GeneticInjection {
                    trait_modifications,
                    ..
                } => {
                    for (m_idx, m) in trait_modifications.iter().enumerate() {
                        match *m {
                            TraitMod::ScaleMorphology(v)
                            | TraitMod::MassMultiplier(v)
                            | TraitMod::MetabolicEfficiencyDelta(v)
                            | TraitMod::SensoryRangeMultiplier(v)
                            | TraitMod::BrainParameterShift { bias_delta: v, .. } => {
                                check_f32(
                                    v,
                                    &format!(
                                        "body.inputs[{idx}].kind.trait_modifications[{m_idx}]"
                                    ),
                                )?;
                            }
                        }
                    }
                }
                InterventionKind::RainfallDelta
                | InterventionKind::TemperatureDelta
                | InterventionKind::Deforest
                | InterventionKind::RemovePredators
                | InterventionKind::AddNutrient => {}
            }
        }
        InputKind::ObserverAction(ObserverAction::EvolutionSettingsChanged {
            mutation_rate,
            selection_bias,
            ..
        }) => {
            check_f64(*mutation_rate, &format!("body.inputs[{idx}].mutation_rate"))?;
            check_f64(
                *selection_bias,
                &format!("body.inputs[{idx}].selection_bias"),
            )?;
        }
        InputKind::ObserverAction(_) | InputKind::ChronicleNote { .. } => {}
    }
    Ok(())
}

fn validate_event_floats(idx: usize, kind: &WorldEvent) -> Result<(), WorldRecordError> {
    match kind {
        WorldEvent::Birth {
            energy_transferred, ..
        } => {
            check_f64(
                *energy_transferred,
                &format!("body.events[{idx}].energy_transferred"),
            )?;
        }
        WorldEvent::Kill {
            damage,
            energy_transferred,
            ..
        } => {
            check_f64(*damage, &format!("body.events[{idx}].damage"))?;
            check_f64(
                *energy_transferred,
                &format!("body.events[{idx}].energy_transferred"),
            )?;
        }
        WorldEvent::Death { .. }
        | WorldEvent::Migration { .. }
        | WorldEvent::Chronicle { .. }
        | WorldEvent::EpochReplacement { .. } => {}
    }
    Ok(())
}

/// Validate static header invariants before canonical hashing.
pub fn validate_header(header: &WorldRecordHeader) -> Result<(), WorldRecordError> {
    if header.schema_version != WORLD_RECORD_SCHEMA_VERSION {
        return Err(WorldRecordError::UnsupportedSchemaVersion {
            expected: WORLD_RECORD_SCHEMA_VERSION,
            actual: header.schema_version,
        });
    }
    if header.run_id.trim().is_empty() {
        return Err(WorldRecordError::EmptyRunId);
    }
    if header.parent_run_id.is_some() != header.fork_tick.is_some()
        || header
            .parent_run_id
            .as_deref()
            .is_some_and(|p| p.trim().is_empty())
        || (header.fork_keyframe_hash.is_some() && header.parent_run_id.is_none())
    {
        return Err(WorldRecordError::InvalidForkLink);
    }
    if header.segment_ticks == 0 {
        return Err(WorldRecordError::InvalidSegmentTicks(header.segment_ticks));
    }
    if (header.era_index > 0 && header.prev_era_tail_hash.is_none())
        || (header.era_index == 0 && header.prev_era_tail_hash.is_some())
    {
        return Err(WorldRecordError::InvalidEraLink {
            era_index: header.era_index,
        });
    }
    validate_header_floats(header)?;
    let _ = canonical_encode(header)?;
    Ok(())
}

/// Compute the SHA-256 digest of a canonicalized [`WorldRecordHeader`].
pub fn header_digest(header: &WorldRecordHeader) -> Result<SegmentHash, WorldRecordError> {
    validate_header_floats(header)?;
    let body = canonical_encode(header)?;
    let mut payload = Vec::with_capacity(DOMAIN_HEADER.len() + body.len());
    payload.extend_from_slice(DOMAIN_HEADER);
    payload.extend_from_slice(&body);
    Ok(sha256(&payload))
}

/// Compute the `prev_hash` anchor required on segment `0` from `header_hash`.
pub fn genesis_anchor(header_hash: &SegmentHash) -> SegmentHash {
    let mut payload = Vec::with_capacity(DOMAIN_GENESIS.len() + 32 + 32);
    payload.extend_from_slice(DOMAIN_GENESIS);
    payload.extend_from_slice(&GENESIS_HASH);
    payload.extend_from_slice(header_hash);
    sha256(&payload)
}

fn validate_segment_floats(body: &SegmentBody) -> Result<(), WorldRecordError> {
    check_f64(
        body.keyframe.standing_crop_eu,
        "body.keyframe.standing_crop_eu",
    )?;
    check_f64(body.keyframe.detritus_eu, "body.keyframe.detritus_eu")?;
    check_f64(
        body.keyframe.total_agent_energy_eu,
        "body.keyframe.total_agent_energy_eu",
    )?;
    for (idx, input) in body.inputs.iter().enumerate() {
        validate_input_floats(idx, &input.kind)?;
    }
    for (idx, event) in body.events.iter().enumerate() {
        validate_event_floats(idx, &event.kind)?;
    }
    Ok(())
}

/// Compute `SHA-256(DOMAIN_SEGMENT || prev_hash || canonical(body))`.
pub fn compute_segment_hash(
    prev_hash: &SegmentHash,
    body: &SegmentBody,
) -> Result<SegmentHash, WorldRecordError> {
    validate_segment_floats(body)?;
    let encoded_body = canonical_encode(body)?;
    let mut payload = Vec::with_capacity(DOMAIN_SEGMENT.len() + 32 + encoded_body.len());
    payload.extend_from_slice(DOMAIN_SEGMENT);
    payload.extend_from_slice(prev_hash);
    payload.extend_from_slice(&encoded_body);
    Ok(sha256(&payload))
}

impl WorldRecord {
    /// Construct an empty, verified [`WorldRecord`] seeded with `header`.
    pub fn build_empty(mut header: WorldRecordHeader) -> Result<Self, WorldRecordError> {
        header
            .initial_conditions
            .sort_unstable_by(|a, b| a.0.cmp(&b.0));
        validate_header(&header)?;
        let header_hash = header_digest(&header)?;
        Ok(Self {
            format: WORLD_RECORD_FORMAT.to_string(),
            schema_version: WORLD_RECORD_SCHEMA_VERSION,
            header,
            header_hash,
            segments: Vec::new(),
        })
    }

    /// Expected `prev_hash` for the next segment to be appended.
    pub fn next_prev_hash(&self) -> SegmentHash {
        self.segments
            .last()
            .map(|seg| seg.hash)
            .unwrap_or_else(|| genesis_anchor(&self.header_hash))
    }

    /// Seal `body` onto the end of the hash chain, checking index and tick monotonicity.
    pub fn append_segment(
        &mut self,
        body: SegmentBody,
    ) -> Result<&SignedSegment, WorldRecordError> {
        let idx = self.segments.len();
        validate_segment_body(idx, self.segments.last().map(|s| s.body.tick_end), &body)?;
        let prev_hash = self.next_prev_hash();
        let hash = compute_segment_hash(&prev_hash, &body)?;
        self.segments.push(SignedSegment {
            prev_hash,
            body,
            hash,
        });
        Ok(self
            .segments
            .last()
            .expect("segment just pushed into non-empty vector"))
    }
}

fn validate_segment_body(
    segment_index: usize,
    prev_tick_end: Option<u64>,
    body: &SegmentBody,
) -> Result<(), WorldRecordError> {
    if body.index != segment_index as u64 {
        return Err(WorldRecordError::InvalidSegmentIndex {
            segment_index,
            actual_index: body.index,
        });
    }
    if body.tick_start > body.tick_end {
        return Err(WorldRecordError::InvalidTickRange {
            segment_index,
            tick_start: body.tick_start,
            tick_end: body.tick_end,
            prev_tick_end,
        });
    }
    if let Some(prev_end) = prev_tick_end {
        if body.tick_start != prev_end.saturating_add(1) {
            return Err(WorldRecordError::InvalidTickRange {
                segment_index,
                tick_start: body.tick_start,
                tick_end: body.tick_end,
                prev_tick_end: Some(prev_end),
            });
        }
    }
    validate_segment_floats(body)?;
    let mut prev_input_tick: Option<u64> = None;
    for input in &body.inputs {
        if input.tick < body.tick_start || input.tick > body.tick_end {
            return Err(WorldRecordError::RecordTickOutOfBounds {
                segment_index,
                record_tick: input.tick,
                tick_start: body.tick_start,
                tick_end: body.tick_end,
            });
        }
        if let Some(prev) = prev_input_tick {
            if input.tick < prev {
                return Err(WorldRecordError::UnsortedTicks {
                    segment_index,
                    prev_tick: prev,
                    tick: input.tick,
                });
            }
        }
        prev_input_tick = Some(input.tick);
    }
    let mut prev_event_tick: Option<u64> = None;
    for event in &body.events {
        if event.tick < body.tick_start || event.tick > body.tick_end {
            return Err(WorldRecordError::RecordTickOutOfBounds {
                segment_index,
                record_tick: event.tick,
                tick_start: body.tick_start,
                tick_end: body.tick_end,
            });
        }
        if let Some(prev) = prev_event_tick {
            if event.tick < prev {
                return Err(WorldRecordError::UnsortedTicks {
                    segment_index,
                    prev_tick: prev,
                    tick: event.tick,
                });
            }
        }
        prev_event_tick = Some(event.tick);
    }
    Ok(())
}

/// Verify header integrity, segment contiguity, and the entire SHA-256 hash chain.
pub fn verify_record(record: &WorldRecord) -> Result<VerificationReport, WorldRecordError> {
    if record.format != WORLD_RECORD_FORMAT {
        return Err(WorldRecordError::UnsupportedFormat {
            expected: WORLD_RECORD_FORMAT.to_string(),
            actual: record.format.clone(),
        });
    }
    if record.schema_version != WORLD_RECORD_SCHEMA_VERSION {
        return Err(WorldRecordError::UnsupportedSchemaVersion {
            expected: WORLD_RECORD_SCHEMA_VERSION,
            actual: record.schema_version,
        });
    }
    validate_header(&record.header)?;
    let expected_header_hash = header_digest(&record.header)?;
    if record.header_hash != expected_header_hash {
        return Err(WorldRecordError::HeaderHashMismatch {
            expected: expected_header_hash,
            actual: record.header_hash,
        });
    }

    let mut expected_prev = genesis_anchor(&record.header_hash);
    let mut prev_tick_end: Option<u64> = None;
    let mut total_inputs = 0usize;
    let mut total_events = 0usize;
    let mut total_dropped_inputs = 0u64;
    let mut total_dropped_events = 0u64;

    for (idx, seg) in record.segments.iter().enumerate() {
        if seg.prev_hash != expected_prev {
            return Err(WorldRecordError::BrokenChain {
                segment_index: idx,
                expected_prev_hash: expected_prev,
                actual_prev_hash: seg.prev_hash,
            });
        }
        validate_segment_body(idx, prev_tick_end, &seg.body)?;
        let expected_hash = compute_segment_hash(&seg.prev_hash, &seg.body)?;
        if seg.hash != expected_hash {
            return Err(WorldRecordError::SegmentHashMismatch {
                segment_index: idx,
                expected_hash,
                actual_hash: seg.hash,
            });
        }
        expected_prev = seg.hash;
        prev_tick_end = Some(seg.body.tick_end);
        total_inputs = total_inputs.saturating_add(seg.body.inputs.len());
        total_events = total_events.saturating_add(seg.body.events.len());
        total_dropped_inputs =
            total_dropped_inputs.saturating_add(seg.body.dropped_inputs_since_prev);
        total_dropped_events =
            total_dropped_events.saturating_add(seg.body.dropped_events_since_prev);
    }

    Ok(VerificationReport {
        segment_count: record.segments.len(),
        first_tick: record.segments.first().map(|s| s.body.tick_start),
        last_tick: record.segments.last().map(|s| s.body.tick_end),
        total_inputs,
        total_events,
        total_dropped_inputs,
        total_dropped_events,
        tail_hash: expected_prev,
        replayable: record.header.deterministic && total_dropped_inputs == 0,
    })
}

/// Serialize a [`WorldRecord`] to pretty UTF-8 JSON bytes for inspection or disk persistence.
pub fn encode_record_json(record: &WorldRecord) -> Result<Vec<u8>, WorldRecordError> {
    serde_json::to_vec_pretty(record)
        .map_err(|err| WorldRecordError::Serialization(err.to_string()))
}

/// Deserialize and verify a [`WorldRecord`] from UTF-8 JSON bytes.
pub fn verify_record_json(
    bytes: &[u8],
) -> Result<(WorldRecord, VerificationReport), WorldRecordError> {
    let record: WorldRecord = serde_json::from_slice(bytes)
        .map_err(|err| WorldRecordError::Serialization(err.to_string()))?;
    let report = verify_record(&record)?;
    Ok((record, report))
}
