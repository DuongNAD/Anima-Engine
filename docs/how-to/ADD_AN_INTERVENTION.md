---
title: Cách thêm một loại can thiệp (intervention) mới
status: active
owner: maintainers
last_reviewed: 2026-11-22
review_cycle: monthly
---

# Cách thêm một loại can thiệp (intervention) mới

Hướng dẫn này trình bày các bước thêm một biến thể `InterventionKind` thường dùng cho các tác động sinh học/phi sinh học qua `InterventionCommand` (áp dụng trên cả thế giới sống `LiveExperimentAdapter` lẫn thế giới tham chiếu `ReferenceEcosystem`).

> **Phân biệt với exotic forcing:** các tác động lên trường `MU` (`AddSource`, `RemoveSource`, `Pulse`) nằm riêng tại `ExoticInterventionKind` ([`src-tauri/src/core/exotic_energy/forcings.rs`](../../src-tauri/src/core/exotic_energy/forcings.rs)) và bị `LiveExperimentAdapter` từ chối. Hợp đồng chuẩn cho intervention nằm tại [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)* (`ER01`, `ER03`, `ER07`, `ER08`), [`ENERGY_LEDGER_CONTRACT.md`](../reference/ENERGY_LEDGER_CONTRACT.md) (§1–§6), và [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md) (§2, §5).

---

## Bước 1 — Thêm biến thể vào `InterventionKind`

Mở [`src-tauri/crates/anima-domain/src/intervention.rs`](../../src-tauri/crates/anima-domain/src/intervention.rs) (được re-export qua `core::intervention`) và thêm biến thể mới vào enum `InterventionKind`:

- Giữ crate `anima-domain` thuần túy, không phụ thuộc Bevy, và giữ đầy đủ derive `Clone, Debug, Serialize, Deserialize, PartialEq`.
- Tận dụng các trường bao sẵn của `InterventionCommand` (`id`, `cause_id`, `region: Region`, `start_tick`, `duration_ticks`, `intensity: f32`, `signed_negative: bool`, `curve: Curve`, `reversible: bool`) thay vì lặp lại trường thời gian hoặc vùng trong biến thể mới.

---

## Bước 2 — Xác thực đầu vào manifest tại `validate_intervention`

Mở [`src-tauri/src/core/experiment/interventions.rs`](../../src-tauri/src/core/experiment/interventions.rs). Hàm `validate_intervention(cmd, run_ticks)` đã tự động kiểm tra:
- `cmd.intensity` hữu hạn và `>= 0.0` (chiều âm dùng `signed_negative`);
- `Region::Rect` không đảo ngược biên và `Region::Radius` có tâm/bán kính hữu hạn với `r > 0.0`;
- `start_tick + effective_duration()` không tràn `u64` và cửa sổ giao với `1..=run_ticks`.

Nếu biến thể mới mang thêm tham số số học riêng (như `center_x`, `radius`, `toxicity_level`, `transmission_rate` ở các biến thể hiện có), hãy thêm nhánh kiểm tra `is_finite()` và miền giá trị hợp lệ trong `validate_intervention` để từ chối bằng `ExperimentError`.

> **Lưu ý khi viết manifest:** `ExperimentManifest::validate` từ chối mọi `InterventionCommand` có `cause_id == CAUSE_OBSERVER` (dành riêng cho hành động runtime của `ObserverPolicy::Inhabit`).

---

## Bước 3 — Gán thẻ băm chuẩn (canonical tag) duy nhất

Mở [`src-tauri/src/core/experiment/manifest.rs`](../../src-tauri/src/core/experiment/manifest.rs) và thêm nhánh tương ứng trong `canonicalize_intervention_kind`:

1. Chọn một byte tag **chưa dùng** (các biến thể hiện tại dùng `0x50..=0x57`, nên thẻ tiếp theo là **`0x58`**).
2. Tuần tự hóa từng trường payload bằng các helper chuẩn (`c.u32(...)`, `c.f32(...)`, `c.u64(...)`, thẻ độ dài/hiện diện) — không dùng `Debug` formatting.
3. Mở [`src-tauri/src/core/experiment/tests.rs`](../../src-tauri/src/core/experiment/tests.rs) và thêm biến thể mới vào mảng `kinds` trong test `intervention_kind_tags_are_distinct` để khóa chắc chắn mọi biến thể có hash riêng biệt.

---

## Bước 4 — Triển khai thực thi trong `apply_live_interventions_system`

Mở [`src-tauri/src/core/live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs), tại hàm `apply_live_interventions_system` (chạy trong chuỗi lịch trình duy nhất [`simulation_schedule::build_tick_schedule`](../../src-tauri/src/core/simulation_schedule.rs)):

1. **Phân loại rate forcing vs stock/entity forcing:**
   - **Rate forcing** (như `RainfallDelta`, `TemperatureDelta`): tính lại từ giá trị trung hòa (`1.0` / `0.0`) trên **mọi tick** vào `LiveForcings` để khi hết cửa sổ tác động dừng ngay lập tức, và chỉ ghi `state.ledger.record(...)` khi `SimClock::fires(tick, ECOLOGY_PERIOD)`.
   - **Stock / entity forcing** (như `Deforest`, `AddNutrient`, `RemovePredators`, `ToxicityPlume`, `PathogenTransmission`): đặt bên trong nhánh `if !SimClock::fires(tick, ECOLOGY_PERIOD) { return; }` (`ECOLOGY_PERIOD = 60`).
2. **Bảo đảm tính tất định (determinism):**
   - Mọi phép lặp query ECS (`Entity`) phải thu thập vào buffer và `sort_unstable_by_key(|e| (e.index(), e.generation()))` trước khi tác động — tuyệt đối không phụ thuộc thứ tự archetype của Bevy.
   - Khi phân phối lên ô lưới (như `fertilise_region`), bảo đảm kết quả không phụ thuộc thứ tự duyệt ô.
3. **Bảo toàn Closed-EU qua `EnergyLedger`:**
   - **Tuyệt đối không** sửa trực tiếp `EcosystemBiomass::plants` hay `EcosystemBiomass::animals` (đây chỉ là bản sao census hiển thị!).
   - Luôn tác động lên kho thẩm quyền (`ResourceField::r`, `HomeostaticState::energy`, `EcosystemBiomass::detritus`), đo lại phần chênh lệch thực tế ở `f64` (`f32 -> f64`), và định tuyến qua `EnergyLedger::transfer(..., EnergyEvent::Intervention)`, `energy_ledger::debit_reserve` + `settle_reserve_outflow_to_detritus`, hoặc `ReclaimAndDespawnAgentCommand`. Sai số sau cùng phải nằm trong `RESIDUAL_ABS_TOLERANCE_EU = 1e-3`.
4. **Ghi nhận nhân–quả:** ghi dấu vết vào `state.ledger.record(cmd.cause_id, None, tick, ...)` và tăng `state.applied_interventions += 1`.

> **Ghi chú trung thực về `GeneticInjection` trên thân cây hiện tại:** trong `apply_live_interventions_system`, `GeneticInjection` hiện ghi bản ghi `live.genetic_injection` vào `state.ledger` và tăng `applied_interventions` chứ chưa sửa trực tiếp genotype của agent đang sống (xem quy tắc `ER03` và `D06`).

---

## Bước 5 — Triển khai tương ứng trong `ReferenceEcosystem::step`

Mở [`src-tauri/src/core/scenario.rs`](../../src-tauri/src/core/scenario.rs) và thêm nhánh match cho biến thể mới trong `ReferenceEcosystem::step` (chạy ở `ECOLOGY_PERIOD` cho những lệnh thỏa `cmd.affects(0, 0)`). Giữ hướng tác động nhất quán với thế giới sống và ghi bản ghi vào `CausalLedger` dưới `cmd.cause_id`.

---

## Bước 6 — Chạy bộ kiểm thử xác minh

Mở rộng test cho:
- Validate đầu vào lỗi (`core/experiment/tests.rs`);
- Khác biệt tag hash (`intervention_kind_tags_are_distinct`);
- Bảo toàn closed-EU (`src-tauri/tests/ledger_bypass_tests.rs`);
- Hành vi kịch bản tham chiếu (`core::scenario::tests`) và fork sau checkpoint (`core/experiment_runner/fork.rs`).

Chạy các lệnh kiểm chứng từ thư mục gốc repository:

<!-- doc-test -->
```bash
cargo test --manifest-path src-tauri/Cargo.toml -p anima-domain
cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment::tests::defect_b
cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment::tests::intervention_kind_tags_are_distinct
cargo test --manifest-path src-tauri/Cargo.toml --lib core::scenario::tests
cargo test --manifest-path src-tauri/Cargo.toml --test ledger_bypass_tests
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` bằng 5 lệnh `cargo test` ở trên; tổng số test toàn dự án có thẩm quyền xem tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```text
running 23 tests (anima-domain) ... ok. 23 passed; 0 failed; 0 ignored
running 1 test (core::experiment::tests::defect_b_manifest_rejects_invalid_intervention_values) ... ok
running 1 test (core::experiment::tests::intervention_kind_tags_are_distinct) ... ok
running 11 tests (core::scenario::tests) ... ok. 11 passed; 0 failed; 0 ignored
running 5 tests (ledger_bypass_tests) ... ok. 5 passed; 0 failed; 0 ignored
```

---

## Bước tiếp theo

- Khai báo `InterventionCommand` trong JSON manifest: [WRITE_A_MANIFEST.md](WRITE_A_MANIFEST.md).
- Thêm biến quan sát mới: [ADD_AN_OBSERVABLE.md](ADD_AN_OBSERVABLE.md).
