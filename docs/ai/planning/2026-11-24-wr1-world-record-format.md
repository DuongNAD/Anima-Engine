---
title: WR1 World Record Format and No-Feedback Gate (ADR-0008 Step H1)
status: active
description: Design and verification specification for the hash-chained Anima World Record v1 format and its end-of-tick zero-feedback ECS recorder.
---

# WR1 World Record Format and No-Feedback Gate

Implements step **H1** of [ADR-0008](../decisions/ADR-0008-world-past-and-time.md): a self-describing, hash-chained provenance format for simulation runs (`src-tauri/src/core/world_record/`) paired with a strictly non-intrusive end-of-tick ECS recorder (`record_world_record_tick_system`).

## 1. Format Summary (Option C: Segmented Hash Chain)

A serialized World Record uses format tag `"anima.world_record"` at `WORLD_RECORD_SCHEMA_VERSION = 1`:

1. **Pinned Header (`WorldRecordHeader`)** — written at genesis before tick `1`:
   - `seed`, `world_identity`, `world_artifact_sha256`, `model_version` (`LIVE_MODEL_VERSION`), `snapshot_schema_version` (`SCHEMA_VERSION`), `BuildProvenance`, `PlatformClass`, `deterministic`, `initial_world_checksum`, `segment_ticks`, sorted `initial_conditions`, compatibility era links (`era_index`, `prev_era_tail_hash`), and declared `omissions`.
2. **Domain-Separated SHA-256 Chain**:
   - `header_hash = SHA-256("anima.world_record.header.v1\0" || canonical(header))`
   - `genesis_anchor = SHA-256("anima.world_record.genesis.v1\0" || [0; 32] || header_hash)`
   - Segment `0` sets `prev_hash = genesis_anchor`; segment `k > 0` sets `prev_hash = segment[k - 1].hash`.
   - `segment[k].hash = SHA-256("anima.world_record.segment.v1\0" || prev_hash || canonical(body))`
3. **Canonical Byte Encoding (`encoding.rs`)**:
   - Rejects non-finite floating-point values (`NaN`, `+/-inf`) prior to canonical encoding via typed payload traversal (`WorldRecordError::NonFiniteFloat`).
   - Normalizes `-0.0` to `+0.0`.
   - Recursively sorts all JSON object keys in UTF-8 byte order cleanly independent of map iteration order and emits compact UTF-8 JSON using `serde_json` (`float_roundtrip`).
4. **Segmented Body (`SegmentBody`)**:
   - `index`, contiguous `[tick_start, tick_end]`, `end_checksum` (bit-exact `world_checksum`), closed-EU `WorldKeyframe`, non-decreasing `inputs` (`Intervention`, `ObserverAction`, `ChronicleNote`), the six ADR-0008 v1 `WorldEvent` variants (`Birth`, `Death` with 5 causes `starvation|dehydration|predation|reclaim|epoch_replacement`, `Kill`, `Migration`, `Chronicle`, `EpochReplacement`), and overflow markers (`dropped_inputs_since_prev`, `dropped_events_since_prev`).
   - Records with `deterministic = false` remain readable and hash-verifiable while `VerificationReport::replayable == false`.

## 2. Zero-Feedback Architecture

`record_world_record_tick_system` is appended at the very end of Group 4 `.chain()` inside `crate::core::simulation_schedule::build_tick_schedule`:

- **Default rollback:** When `WorldRecordRecorder` is absent, the system returns on its first line without touching any query, resource, allocator, or RNG stream — leaving the default trajectory bit-identical to trunk.
- **Read-only observation:** When `WorldRecordRecorder` is present, all simulation compartments are accessed strictly immutably (`RecordReadParams`), with zero `Commands`, zero `ResMut` on world state, and zero `SimRng` draws.
- **Deterministic summation:** `WorldKeyframe::total_agent_energy_eu` sums agent energies in sorted component-tuple order alongside `world_checksum` calculation.
- **Zero new `LIVE_KEYS`:** `LIVE_KEYS.len()` stays unchanged at `31`.

## 3. Explicit Non-Goals (Deferred to WR2+)

- Replay or time-travel loading from a `WorldRecord` (WR2).
- Desktop app UI timeline controls, persistence IPC commands, or background file streaming.
- Cross-era continuation helpers beyond header schema fields (`era_index`, `prev_era_tail_hash`).

## 4. Verification Gates

- `src-tauri/tests/world_record_format_and_immutability_tests.rs`: round-trip JSON, canonical determinism, finite-float validation, buffer overflow flags, and exact `k` vs `k + 1` chain failure pinpointing across all segments and header.
- `src-tauri/tests/world_record_no_feedback_tests.rs`: ON vs OFF equal `world_checksum` at ticks `0, 180, 1800, 18000` on both the default world and a non-vacuous world with in-life `LifetimeLearning` + autonomous reproduction + mortality, plus `LiveInterventions` and `ObserverTrace` input capture without trajectory perturbation.
- Schedule safety: `schedule_ambiguity_tests` and `schedule_total_order_tests` confirm zero ambiguous pairs remain after wiring `record_world_record_tick_system`.
