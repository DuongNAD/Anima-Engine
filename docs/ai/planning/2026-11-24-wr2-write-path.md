---
title: WR2 Pre-Allocated Ring Buffer and Background Writer Path (ADR-0008 Step H2)
status: active
owner: anima-case
last_reviewed: 2026-11-24
description: Architecture, overflow accounting, atomic segment persistence, and zero-allocation tick verification for World Record v1 write path (ADR-0008 step H2).
---

# WR2 Write Path: Pre-Allocated Ring Buffer & Writer Thread

Implements step **H2** of [ADR-0008](../../decisions/ADR-0008-world-past-and-time.md) on top of [WR1](2026-11-24-wr1-world-record-format.md).

## 1. Zero-Allocation Hot-Loop Design

- **Inline `Copy` ring entries (`src-tauri/src/core/world_record/ring.rs`):** All inputs (`Intervention`, `ObserverAction`, `ChronicleNote`), events (`Birth`, `Death`, `Kill`, `Migration`, `Chronicle`, `EpochReplacement`), and segment seals are transported as fixed-size `Copy` `RingEntry` values backed by stack-allocated `InlineStr<N>` and `CompactIntervention` buffers.
- **Lock-free SPSC ring (`spsc_ring`):** Sized once upfront (`ring_capacity.max(2)`, backed by a power-of-two array of `UnsafeCell<MaybeUninit<RingEntry>>` and `AtomicUsize` head/tail cursors). `RingProducer::try_push` is wait-free and never blocks or grows the backing array.
- **Pre-allocated ECS scratch (`src-tauri/src/core/world_record/system.rs`):** Replaced per-tick `BTreeMap`/`BTreeSet`/`String` usage in `WorldRecordRecorder` with pre-reserved sorted `Vec` buffers merged via `sort_unstable_by_key` (in-place pdqsort, zero heap allocations) and two-pointer scans. Live death age (`base_age + (tick - born_tick)`) is maintained across quiet ticks without re-scanning strings.
- **Direct float rejection (`encoding.rs`) & header link tightening (`chain.rs`):** `canonical_encode` rejects non-finite floats before `serde_json::to_value` coercion, and `validate_header` rejects orphan `fork_keyframe_hash` or `era_index == 0` with `prev_era_tail_hash`.

## 2. Background Writer & Atomic Disk Persistence (`writer.rs`)

- **Dedicated thread (`anima-world-record-writer`):** Drains `RingConsumer<RingEntry>`, builds `RecordedInput` / `RecordedEvent` off the simulation thread, seals hash-chained `SignedSegment` entries on `RingEntry::SegmentSeal`, and persists files when `WorldRecordWriterConfig::output_dir` is set.
- **Atomic writes (`SNAPSHOT_CONTRACT.md §4`):** Both `header.json` and `segment_{index:06}.json` write exclusively through `crate::core::snapshot::write_bytes_atomic` (PID-scoped temp file -> `write_all` -> `flush` -> `sync_all` -> `rename`), ensuring interrupted writes never corrupt committed segments or `header.json`. `load_and_verify_record_dir` ignores stale `.*.tmp` residue and verifies the entire hash chain.

## 3. Overflow Semantics (Drop-and-Count, Never Block)

- When `RingProducer::available_slots() <= 1` on a non-seal entry (one slot is reserved for segment seals) or `try_push` fails, the tick system never blocks and never reallocates: it increments `pending_dropped_inputs` / `pending_dropped_events` and cumulative counters (`WorldRecordRecorder::dropped_inputs()`, `dropped_events()`).
- If a `SegmentSeal` itself cannot be queued while the writer thread is stalled, `segment_start_tick` is preserved and `pending_dropped_events` increments so the next seal remains strictly contiguous with accurate overflow totals, marking `VerificationReport::is_complete() == false` and `replayable == false`.

## 4. Verification Gates

- `src-tauri/tests/world_record_zero_alloc_tests.rs`: `ThreadTrackingAllocator` asserts `allocs == 0` across quiet ticks, ticks with events & inputs, segment-seal ticks, and tiny-buffer overflow ticks, plus an explicit allocator self-control.
- `src-tauri/tests/world_record_writer_tests.rs`: verifies tiny-buffer overflow drop counting, atomic disk writes with synthetic `.tmp` crash residue + tamper detection, and async writer zero-feedback checksum parity across `0, 180, 1800, 18000` ticks on default and learning+reproduction worlds.
- Clarification to [2026-11-24-wr1-world-record-format.md](2026-11-24-wr1-world-record-format.md): ADR-0008 assigns the ring-buffered write path to `WR2` (H2, this card) and keyframe replay/fork reconstruction to `WR3` (H3).
