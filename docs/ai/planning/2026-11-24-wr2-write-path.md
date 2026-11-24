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

## 4. Verification Gates & Merged Trunk Record

- **Merged base:** Merged `origin/feat/ecological-selection` (`92bf0df0cfcb84f098c330326c6597f12affd960`, containing WR1 PR `#236` merged at `ee4fc287`) into `win/wr2` cleanly with `0` conflicts.
- `src-tauri/tests/world_record_zero_alloc_tests.rs` (`1/1`): `ThreadTrackingAllocator` asserts `allocs == 0` across quiet ticks, ticks with events & inputs, segment-seal ticks, and tiny-buffer overflow ticks, plus an explicit allocator self-control.
- `src-tauri/tests/world_record_writer_tests.rs` (`3/3`): verifies tiny-buffer overflow drop counting, atomic disk writes with synthetic `.tmp` crash residue + tamper detection, and async writer zero-feedback checksum parity across `0, 180, 1800, 18000` ticks on default and learning+reproduction worlds.
- `src-tauri/tests/world_record_format_and_immutability_tests.rs` (`7/7`) and `src-tauri/tests/world_record_no_feedback_tests.rs` (`3/3`): passing on merged trunk (`full cargo test --features desktop --no-fail-fast`: `181` test targets, `1,698` passed, `0` failed, `2` allow-listed ignored; `check_test_targets.mjs --profile desktop` exit `0`).
- Clarification to [2026-11-24-wr1-world-record-format.md](2026-11-24-wr1-world-record-format.md): ADR-0008 assigns the ring-buffered write path to `WR2` (H2, this card) and keyframe replay/fork reconstruction to `WR3` (H3).
