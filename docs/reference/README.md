---
title: Chỉ mục reference
status: active
owner: architecture
last_reviewed: 2026-11-17
review_cycle: per-release
---

# Reference

Đây là chỉ mục tới các hợp đồng đang có hiệu lực. Không sao chép quy tắc định lượng
vào file này.

| Hợp đồng | Nguồn chuẩn | Implementation/validation liên quan |
|---|---|---|
| Luật mô phỏng, đơn vị, bảo toàn, tick order | [`SIMULATION_RULES.md`](../../SIMULATION_RULES.md) | `src-tauri/src/core/`, test Rust |
| Phân loại và chuyển biome | [`BIOME_TAXONOMY.md`](../../BIOME_TAXONOMY.md) | world generation, renderer mapping |
| Tọa độ, transform, chunk | [`COORDINATE_CONTRACT.md`](../../COORDINATE_CONTRACT.md) | Rust/TS parity tests |
| World artifact | [`PROJECT.md`](../../PROJECT.md), schema/fixtures trong code | Rust/TS codec và fixtures |
| Map artifact/canonical views | [`MAP_MANIFEST.md`](../../MAP_MANIFEST.md) | [`map_manifest.schema.json`](../../map_manifest.schema.json) |
| Benchmark report | [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) | [`benchmark_report.schema.json`](../../benchmark_report.schema.json) |
| Test infrastructure | [`TEST_INFRA.md`](../ai/TEST_INFRA.md) | package scripts, Cargo tests |
| Genotype → phenotype → spawn/save/migration | [`CREATURE_DEVELOPMENT_CONTRACT.md`](CREATURE_DEVELOPMENT_CONTRACT.md) | ADR-0001, CM-S01…CM-S11 |
| World-law fork, exotic energy, experiment và species evidence | [`EVOLUTION_EXPERIMENT_CONTRACT.md`](EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)* | ADR-0002, AE-S01…AE-S15 |
| Lệnh và event Tauri IPC (số lệnh và số event do gate in ra, không chép ở đây) | [`IPC_CONTRACT.md`](IPC_CONTRACT.md) | `src-tauri/src/lib.rs`, `commands/`; gate `scripts/check_ipc_reference.mjs` |
| Tất định (deterministic mode) | [`DETERMINISM_CONTRACT.md`](DETERMINISM_CONTRACT.md) | `core/determinism.rs`, `core/simulation_schedule.rs`; `tests/determinism_gate_tests.rs`, `tests/schedule_ambiguity_tests.rs`, `tests/schedule_total_order_tests.rs` |
| Sổ cái năng lượng, closed EU, cổng bảo toàn G1.1 | [`ENERGY_LEDGER_CONTRACT.md`](ENERGY_LEDGER_CONTRACT.md) | `crates/anima-domain/src/energy.rs` (re-export ở `core/energy_ledger.rs`) |
| Snapshot / checkpoint khoa học (khác file save) | [`SNAPSHOT_CONTRACT.md`](SNAPSHOT_CONTRACT.md) | `core/snapshot.rs`, `tests/snapshot_checkpoint_tests.rs` |
| Genotype → prompt text-to-3D → asset | [`CREATURE_PROMPT_CONTRACT.md`](CREATURE_PROMPT_CONTRACT.md) *(proposed)* | — |

Nếu một contract thay đổi, cập nhật test/schema/fixture và các consumer được liệt kê
trong [ma trận thay đổi](../governance/DOCUMENTATION_POLICY.md#ma-trận-thay-đổi).
