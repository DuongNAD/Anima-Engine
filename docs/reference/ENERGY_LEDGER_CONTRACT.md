---
title: Hợp đồng sổ cái năng lượng (closed EU)
status: active
owner: maintainers
last_reviewed: 2026-07-25
---

# Hợp đồng sổ cái năng lượng

Tài liệu này định nghĩa **cách EU được phép di chuyển trong thế giới sống (live Bevy world)** và
tolerance của cổng bảo toàn G1.1. Nguồn code: `src-tauri/src/core/energy_ledger.rs`.

Đọc kèm: `SIMULATION_RULES.md` (bảng đơn vị), `docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md`
(invariant **D06**), `src-tauri/src/core/sim_rules.rs` (`STATE_VARIABLES`, `Conservation`).

## 1. Ba ngăn, và nơi năng lượng thực sự nằm

`sim_rules::STATE_VARIABLES` khai báo `plants`, `animals`, `detritus` là `Conservation::ClosedEnergy`.
Điểm dễ hiểu sai: **hai trong ba ngăn chỉ là bản sao hiển thị**, không phải nơi lưu trữ.

| Ngăn | Kho thẩm quyền (authoritative) | Trường `EcosystemBiomass` |
|---|---|---|
| plants | các ô `ResourceField::r` | bản sao, làm mới mỗi tick |
| animals | `HomeostaticState::energy` của từng agent | bản sao, làm mới mỗi tick |
| detritus | `EcosystemBiomass::detritus` | **thẩm quyền** |

Tổng đóng của thế giới luôn được tính từ kho thẩm quyền, không từ bản sao:

```text
closed_total_eu = ResourceField::total_biomass()
                + Σ agent.HomeostaticState.energy
                + EcosystemBiomass::detritus
```

**Không có ngăn thứ tư.** Food entity chưa ăn và quả chưa hái là *quyền rút* trên detritus, thanh
toán tại thời điểm bị ăn — không phải kho riêng. Thêm ngăn thứ tư đồng nghĩa thêm một đại lượng
chưa khai báo vào bảng đơn vị.

## 2. Quy tắc: đo đích đến, đừng giả định

`HomeostaticState::energy` và `ResourceField::r` là `f32`. Cộng một lượng nhỏ vào một reserve lớn
**làm tròn**, và bão hoà tại `cap` có thể cắt bớt. Vì vậy:

> Ghi nợ nguồn đúng bằng lượng đích **thực sự** thay đổi, đọc lại ở `f64` — không bao giờ bằng
> lượng được yêu cầu.

`EnergyLedger::transfer_into_reserve`, `credit_reserve` và `debit_reserve` đều theo quy tắc này.
Hệ quả: một giao dịch đơn lẻ là **chính xác tuyệt đối**; phần làm tròn không rơi vào đâu cả vì nó
không bao giờ được rút ra.

Điều này áp dụng cho **cả** giao dịch giữa hai kho thẩm quyền (gặm cỏ, săn mồi, tái sinh thực vật).
Trước G1.1 chúng được coi là "bảo toàn theo cấu trúc"; thực tế mỗi lần chạy mất một phần ULP, và
qua hàng trăm nghìn tick điều đó là **xu hướng, không phải nhiễu** — đo được −0.32 EU sau 120k tick.

## 3. Quy tắc: thu hồi và huỷ phải nguyên tử

Ghi có reserve của xác chết vào detritus **ngay lập tức** trong khi huỷ entity qua `Commands`
(áp dụng ở cuối schedule) tạo ra một cửa sổ: agent vẫn sống, vẫn mang reserve đã được ghi sổ.

- Hệ thống chạy sau trong cùng tick đốt reserve đó → detritus được ghi có **lần hai** → tạo EU.
- Hệ thống cho nó ăn → rút EU từ detritus vào một cơ thể sắp bị huỷ → mất EU.

Chiều nào xảy ra phụ thuộc thứ tự mà executor đa luồng của Bevy chọn, nên residual trôi mỗi lần một
kiểu và **trông giống nhiễu dấu phẩy động**. `ReclaimAndDespawnAgentCommand` làm cả hai việc tại
cùng một sync point, xoá bỏ cửa sổ đó.

Quy tắc tổng quát: **mọi thay đổi năng lượng phải nguyên tử với thay đổi vòng đời gây ra nó.**

## 4. Genesis là điều kiện biên, không phải giao dịch

Theo **D06**: tại `t = 0`, năng lượng quần thể *là* điều kiện biên; baseline closed-EU được chốt
**sau** khi khởi tạo plants + animals + detritus. Cá thể sáng lập vì vậy không được cấp vốn từ
detritus — chúng là một phần của cái mà baseline đo.

`EnergyLedger::lock_baseline` chốt một lần duy nhất và bỏ qua lần gọi thứ hai, nên một lần load
không thể "chốt lại" baseline để che một rò rỉ đã xảy ra. Baseline được chốt lazily ở census đầu
tiên, nên cả đường genesis lẫn đường restore đều nhận baseline của một thế giới đã dựng xong.

Epoch replacement **không phải** là sinh sản (D06): cá thể thay thế lấy vốn từ detritus — đúng nơi
reserve của cá thể bị thay vừa được trả về. Pool không đủ ⇒ cá thể mới bắt đầu trong tình trạng
đói, và phần thiếu được đếm vào `EnergyLedger::refused`, không được tạo ra.

## 5. Tolerance

`RESIDUAL_ABS_TOLERANCE_EU = 1e-3` EU (tuyệt đối).

**Đây là thuộc tính của số học, không phải một núm vặn.** Vì mỗi giao dịch ghi nợ nguồn đúng bằng
delta `f64` đo được ở đích, giao dịch đơn lẻ không đóng góp trôi dạt dù `f32` làm tròn tệ đến đâu.
Sai số còn lại chỉ đến từ việc mở rộng `f32`→`f64` khi cộng census: quy mô thế giới MVP ~10^4 EU
trên ~10^2 kho, epsilon `f64` là 2.2e-16, qua ~10^6 bước cộng ⇒ chặn sai số trung thực quanh 1e-6 EU.
1e-3 EU là ba bậc dự phòng phía trên, đồng thời vẫn chỉ ~10^-7 tổng thế giới — nên một rò rỉ thật
dù chỉ bằng **một** food item (hàng chục EU) vẫn bị bắt với biên độ rất lớn.

Thực đo: sau 120.000 tick với 240 lần sinh/tử và một chu kỳ save/load, residual là **0.000000000** —
chính xác từng bit, không chỉ "trong tolerance".

> **Không được nâng giá trị này để test xanh.** Residual vượt ngưỡng nghĩa là năng lượng đang di
> chuyển ở đâu đó ngoài `EnergyLedger`. Cách sửa là định tuyến điểm đó qua ledger, không phải nới
> ngưỡng. Cũng không được nâng một assertion `1e-4` cục bộ thành tolerance toàn sản phẩm.

## 6. Cổng G1.1

`src-tauri/tests/energy_conservation_tests.rs`:

| Test | Chứng minh |
|---|---|
| `live_world_conserves_energy_across_births_deaths_and_a_save_load_cycle` | Toàn bộ vòng lặp năng lượng sống: sinh, tử, ăn, gặm cỏ, săn mồi, ra quả, một chu kỳ save/load |
| `eating_food_moves_energy_out_of_detritus_instead_of_minting_it` | Food là quyền rút trên detritus, không phải nguồn |
| `epoch_replacement_does_not_create_energy` | D06 cho epoch replacement |
| `diagnose_which_system_moves_the_closed_total` (`#[ignore]`) | Bộ dò tìm hệ thống làm lệch tổng đóng |

Số tick mặc định là 120.000 để `cargo test` không phải trả giá cho lần chạy dài; đặt
`ANIMA_ENERGY_GATE_TICKS` để tái lập lần chạy hàng triệu tick.

Cổng phải chạy với **thread mặc định**, không phải `--test-threads=1`. Rò rỉ phụ thuộc thứ tự ở
mục 3 chỉ lộ ra khi executor đa luồng của Bevy tự do chọn thứ tự; ép tuần tự sẽ giấu đúng loại lỗi
mà cổng này tồn tại để bắt.

## 7. Chưa đóng

- `SavedSimulationState` mới chỉ mang ba scalar closed-EU và vector `ResourceField::r`. Đây là mức
  tối thiểu để cổng save/load chạy được; **G1.2** thay bằng `SnapshotEnvelope` có version + checksum.
- Ledger chưa được nối vào `intervention`/`CauseId`, nên `EnergyEvent::Intervention` hiện chỉ là
  chỗ dành sẵn.
- `sim_rules::STATE_VARIABLES` chưa có mục nào cho phần EU đang nằm trong food/fruit chưa tiêu thụ,
  vì thiết kế coi chúng là quyền rút trên detritus chứ không phải kho. Nếu sau này chúng trở thành
  kho thật, bảng đơn vị phải được cập nhật cùng lúc.
