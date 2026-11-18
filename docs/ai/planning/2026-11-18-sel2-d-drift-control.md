---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ SEL2-D — chế độ đối chứng drift trung tính khoá bước
description: Thế giới sống nay có hai khoá opt-in live.drift_control và live.drift_reset_ticks để chạy đối chứng drift trung tính khoá bước theo đúng số ca sinh/chết từng tick của lượt chọn lọc, tách cha di truyền P_gen khỏi người trả EU P_pay, cô lập luồng RNG LIVE_DRIFT_SELECTION, và báo hụt lịch qua DriftDiagnostics + CausalLedger + warnings mà không tạo EU ảo.
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Thẻ SEL2-D — chế độ đối chứng drift trung tính khoá bước

> **Thẻ dựng cơ chế đối chứng và cổng kiểm định, chưa phải một lượt đo SEL2.** Không kết quả dịch chuyển
> khối lượng cơ thể hay chọn lọc nào của SEL2 được khẳng định ở đây.

## Mục tiêu

SEL2 hỏi liệu khối lượng cơ thể có dịch chuyển dưới khan hiếm thức ăn hay không. Để gọi một dịch chuyển tính
trạng là chọn lọc chứ không phải trôi dạt nhân khẩu học, lượt chọn lọc (`selected`) phải thắng một lượt
**đối chứng drift trung tính khoá bước** (`drift`): xuất phát từ cùng founder, tái diễn đúng số ca sinh và
số ca chết theo từng tick (hoặc từng cửa sổ đặt lại Channon `K` tick) nhưng thay bằng cái chết ngẫu nhiên
đều và cha mẹ ngẫu nhiên đều dưới cùng quy tắc đột biến.

## Cơ chế và hai khoá mới

- **Hai khoá manifest (`LIVE_KEYS.len() = 24` sau S10):**
  - `live.drift_control`: `1.0` bật phát lại drift (`0.0` hoặc vắng giữ đường chạy cũ giống hệt từng bit).
    Đòi hỏi `live.mortality = 1.0` và `live.reproduction = 1.0`; gọi thẳng `ExperimentModel::from_manifest`
    khi `live.drift_control = 1.0` bị từ chối vì bắt buộc phải có `TurnoverSchedule` đã ghi từ lượt chọn lọc.
  - `live.drift_reset_ticks`: độ dài cửa sổ đặt lại Channon `K` (mặc định `0` = phát lại từ founder đến hết
    lượt; `K > 0` đòi hỏi `live.drift_control = 1.0` và `duration_ticks % K == 0`). Ở đầu mỗi cửa sổ `w > 0`,
    nhánh drift phục hồi từ `LiveSnapshot` của lượt `selected` tại tick `w·K` (`snapshot()`/`from_snapshot()`),
    giữ nguyên `AgentBrain.genotype` & `learned` (ADR-0003 D01) và kế thừa không Lamarck (`brain.genotype`
    cho đời con).
- **Cơ chế chọn nạn nhân và cha mẹ mù tính trạng (`core::drift_control`):**
  - Nhánh `selected` gắn `DriftControlState::Record` ghi `TickTurnover { prey_deaths, pred_deaths,
    prey_births, pred_births }` vào bộ đệm cấp phát trước một lần (0 cấp phát heap mới).
  - Trong `mortality_system` (`Replay`), `lethal_cause` bị bỏ qua; ứng viên mỗi nhóm dinh dưỡng được sắp theo
    thứ tự tất định không cấp phát (`candidate_cmp`), rồi chọn không hoàn lại qua thuật toán Fisher–Yates
    một phần từ luồng `sim_stream::LIVE_DRIFT_SELECTION = 4`, sau đó giải phóng qua
    `ReclaimAndDespawnAgentCommand` để thu hồi trọn vẹn EU vào `detritus`.
  - Trong `autonomous_reproduction_system` (`Replay`), trần `prey_cap` và ngưỡng năng lượng sinh sản bị bỏ
    qua, đồng thời **tách cha di truyền `P_gen` khỏi người trả năng lượng `P_pay`**: `P_gen` được rút đều có
    hoàn lại trên **mọi cá thể sống** của nhóm dinh dưỡng (tránh rò chọn lọc sinh sản chống cá thể nặng tiêu
    hao nhanh có `energy = 0` khi tử vong do đói đã tắt), còn `P_pay` là `P_gen` nếu `energy >= 2.0` EU hoặc
    một cá thể đủ khả năng chi trả trong cùng nhóm. Cả hai nhánh gọi chung helper `execute_reproduction_birth`.
  - Khi `live.drift_control = 1.0`, `ensure_observables_supported` từ chối `live.deaths_starvation` và
    `live.deaths_dehydration`, và `read_live_observables` không phát hai observable này (vì cái chết ngẫu
    nhiên không mang nguyên nhân đói/khát).
- **Quy tắc hụt lịch (shortfall rule):**
  - Không bao giờ bỏ qua âm thầm, không bao giờ tự chế EU, không rút EU từ detritus.
  - Khi `scheduled_deaths > N_alive`, giết toàn bộ `N_alive` và cộng phần dư vào
    `DriftDiagnostics::missed_deaths`. Khi không còn cá thể (`missed_births_extinct`) hoặc không còn cá thể
    đủ `2.0` EU (`missed_births_insolvent`), bỏ qua ca sinh đó.
  - Ghi nhận qua ba lớp: `DriftDiagnostics` trong `DriftPairReport` / `DriftEnsembleReport`, bản ghi
    `CausalLedger` (`"live.drift_shortfall_deaths"`, `"live.drift_shortfall_births"`), và chuỗi cảnh báo trong
    `RunResult::warnings`, trong khi giữ `RunStatus::Completed` để tránh thiên lệch sống sót trong
    `PairedEnsembleReport`.

## Cổng kiểm định (`DONE WHEN`)

Toàn bộ năm điều kiện hoàn thành của thẻ SEL2-D đều đạt trên macOS arm64 tại nhánh `mac/sel2-d-drift-control`:

| # | Điều kiện | Kết quả đo |
|---|---|---|
| 1 | 8 test §8C trong `src-tauri/tests/sel2_drift_control_tests.rs` (0 `#[ignore]`) | `9 passed; 0 failed; 0 ignored` (gồm `sel2d_drift_child_entry_point`), `absent == zero == recorded` bit-identical (`0x57792c02`, 50 mẫu) |
| 2 | `cargo fmt --check`, `clippy --features desktop` & `--no-default-features` (`-D warnings`), `cargo test --features desktop` + `check_test_targets.mjs --profile desktop` | Tất cả đạt (`0 failed`, 152 target / 1.499 test pass, 2 allow-listed ignored, 0 cảnh báo) |
| 3 | 6 suite hồi quy giữ nguyên + 2 suite đếm khoá (`LIVE_KEYS.len() = 24`) | `birth_death_zero_alloc_tests` (2/2), `schedule_ambiguity_tests` (6/6), `ledger_bypass_tests` (5/5), `death_cause_split_tests` (8/8), `plant_reserve_tests` (11/11), `energy_conservation_tests` (3/3 + 1 allow-listed), `e2_seam_tests` (9/9), `eco_card_b_tests` (12/12) |
| 4 | `cargo modules dependencies` + `scripts/verify_dataflow.py dataflow.json <dot> 1` | `QUA CONG — 16 edge khop hoan toan voi cargo-modules` (không cần sửa `dataflow.json`) |
| 5 | Ghi chú kết quả + chỉ mục `docs/ai/README.md` + `npm run check:docs` | Đạt (0 liên kết gãy, 0 lỗi metadata, 0 lệch IPC) |

Đầu ra thô từ `cargo test --features desktop --test sel2_drift_control_tests -- --nocapture --test-threads=1`:

```text
SEL2D_T1 absent=0x57792c02 zero=0x57792c02 recorded=0x57792c02 samples=50
SEL2D_T2 sched_b=48 actual_b=48 sched_d=22 actual_d=22 missed_b=0 missed_d=0 sel_sum=0xd56ace4d drf_sum=0xdd3ae499
SEL2D_T3 seeds=20 deaths=68/120 (0.567) births_from_bankrupt_heavy=54/120 (0.450)
SEL2D_T4 ticks=1500 actual_deaths=22 actual_births=42 worst_abs_eu=4.656612873077393e-10 worst_rel_eu=2.966528304548834e-15 scratch_caps=(4096, 4096, 4096)
SEL2D_T5 zero_deaths(alive=12, sim_pos=100, drift_pos=0) five_deaths(alive=7, sim_pos=100, drift_pos=18)
SEL2D_T6A first_div=Some(1) missed_insolvent=3 missed_extinct=2 missed_deaths=2 actual_deaths=4 ledger_b=2 ledger_d=1 eu_delta=0e0
SEL2D_T6B pair_status=Completed sched_d=30 missed_d=0 warnings=[]
SEL2D_T7A windows=3 samples=6 worst_eu_drift=3.7834979593753815e-10 mass_net_delta=Some(0.0)
SEL2D_T8_PROCS unreset=[(0x5e310618, 0x7994dfc6) x3] windowed_k150=[(0x5e310618, 0x5b960e59) x3]
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 76.22s
```

## Đề xuất dòng cho tài liệu chủ dự án (không sửa trực tiếp trong thẻ này)

- **STATE / AGENT_WORKFLOW:** `SEL2-D ✅ #<PR> (2026-11-18): live.drift_control + live.drift_reset_ticks; đối chứng drift trung tính khoá bước tách P_gen/P_pay, luồng RNG riêng (id=4), hụt lịch ghim vật lý + báo qua DriftDiagnostics/CausalLedger/warnings`.

## Không khẳng định

- Chưa có số đo SEL2 nào về việc khối lượng cơ thể ở nhánh `selected` có vượt trội nhóm đối chứng `drift` hay
  không — đây mới là hạ tầng đối chứng và bộ 8 test kiểm định của thẻ SEL2-D.
- Tính tất định checksum nhiều tiến trình được kiểm trên một máy (macOS arm64); tương thích chéo nền tảng
  thuộc phạm vi thẻ khác.
