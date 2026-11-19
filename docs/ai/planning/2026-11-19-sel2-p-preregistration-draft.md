---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ SEL2-P-draft — bản thảo đăng ký trước thí nghiệm chọn lọc sinh thái SEL2
description: Bản thảo đăng ký trước SEL2-P (chưa đóng băng, chưa chạy seed xác nhận): giả thuyết H1 hai phía trên khối lượng cơ thể kiểu gen tích phân thời gian (live.trait_mass_mean), thiết kế 3 nhánh (selected, scarce_mut0, scarce_drift SEL2-D) trên cấu hình 16 hồ rải lưới scale 0.63 trong 200.000 tick, N = 20 cặp (780001 + 1000*k), cổng smoke C1..C5 với ô tham chiếu g để trống chờ chủ dự án quyết định.
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# Thẻ SEL2-P-draft — bản thảo đăng ký trước thí nghiệm chọn lọc sinh thái SEL2

> **Bản thảo chưa đóng băng (`status: proposed`).** Tài liệu này **không** chạy bất kỳ mã mô phỏng nào và **chưa** ấn định bản đăng ký cuối cùng (`SEL2-P-freeze`). Toàn bộ chuỗi thăm dò `S6..S15` (`seed 1..=6`) và bước `--smoke` tương lai đều **mù tuyệt đối với `live.trait_mass_mean`, `live.trait_mass_var` và mọi thông số khối lượng cơ thể**. Không một hạt giống xác nhận nào được chạy hay đọc dữ liệu trước khi các phụ thuộc mở ở Mục 8 được giải quyết và bản đăng ký được đóng băng.

Tài liệu này cụ thể hoá [Đề xuất làm lại SEL2](2026-11-18-sel2-redesign-proposal.md) §5 trên nền kết quả thăm dò của [Thẻ S13](2026-11-18-s13-spread-lakes.md), [Thẻ S14](2026-11-19-s14-lakes16-scale.md) và phép kiểm tra tham chiếu `S15` (`2026-11-19-s15-reference-check.md`, đang chờ hợp nhất qua `PR #146`), đồng thời kế thừa kỷ luật thực nghiệm từ [Bản đăng ký E-SEL1](2026-11-16-experiment-e-sel1-food-scarcity-body-mass.md) và [Bản đăng ký E2](2026-07-27-experiment-e2-evolved-brain-default.md).

---

## 1. Giả thuyết & Tính trạng chịu chọn lọc

### Tính trạng mục tiêu

- **Tính trạng chịu chọn lọc:** Khối lượng cơ thể kiểu gen $z_g = \sum_u m_u$, đọc từ `MorphologyGenotype::total_mass()` / `AgentGenotype.0.total_mass()` thông qua biến quan sát `live.trait_mass_mean` (cùng phương sai quần thể kiểu gen `live.trait_mass_var`).
- **Trạng thái khởi tạo (`genesis`):** Mọi cá thể sáng lập (`live.founders = 100.0`) khởi đầu với bộ khung 3 nút đồng nhất có tổng khối lượng `1.5 + 1.0 + 0.8 = 3.3`, do đó `trait_mass_mean(0) = 3.3` và `trait_mass_var(0) = 0.0`. Biến dị di truyền phải tự tích luỹ qua đột biến sinh sản ở các nhánh bật `live.mutation_rate > 0`.

### Giả thuyết (tuyên bố trước khi có bất kỳ dữ liệu seed xác nhận nào)

- **`H1` (hai phía / `two_sided`):** Trong thế giới khan hiếm thức ăn khả thi (`16` hồ rải lưới 2-D, `live.resource_capacity_scale = 0.63`, `live.prey_cap = 320.0`, `200.000` tick), chọn lọc sinh thái làm dịch chuyển trung bình tích phân theo thời gian hậu-warmup của `live.trait_mass_mean` ở nhánh `selected` khác với đối chứng trôi dạt nhân khẩu học–đột biến đồng thời (`scarce_drift`, `SEL2-D`).
  - Vì `H1` là **hai phía**, cả hướng dịch chuyển **tăng** khối lượng lẫn **giảm** khối lượng đều có thể bác bỏ `H0` nếu đồng thời vượt các ngưỡng thống kê và thực chất ở Mục 7; không được đổi sang kiểm định một phía hoặc gán hướng giả thuyết sau khi mở dữ liệu xác nhận.
- **`H0` (giả thuyết không):** Hiệu số bắt cặp hậu-warmup giữa `selected` và `scarce_drift` trên biến tích phân `live.trait_mass_mean` có kỳ vọng bằng `0` (áp lực thức ăn ở cấu hình này không tạo dịch chuyển chọn lọc kiểu gen thực chất vượt trội hơn trôi dạt trung tính và thiên lệch đột biến thuần).

---

## 2. Cấu hình thế giới, khoá luật & Biến quan sát

### Thế giới & phiên bản nền

- **Bộ luật cơ sở:** `WorldLawSet::baseline()` với `exotic_energy: None` (`ExoticSourceModel` trong thế giới sống chỉ hỗ trợ `Renewable` hoặc `None`; theo [ADR-0002](../../decisions/ADR-0002-world-laws-and-exotic-energy.md) và [EVOLUTION_EXPERIMENT_CONTRACT](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md), `LiveExperimentAdapter` từ chối bật năng lượng ngoại lai).
- **Trạng thái xác minh động cơ chạy:** **`headless adapter verified`** (không khẳng định cho cửa sổ đồ hoạ, GPU, bộ học hay app đa luồng).
- **Khoá phiên bản cố định:** `LIVE_MODEL_VERSION = "live-bevy-world/2"`, `SCHEMA_VERSION = 8` (giữ ổn định bất biến trên nền [Thẻ F045](2026-11-18-f045-restore-fidelity.md) và các bản vá khôi phục tiếp theo).

### Bảng tham số thế giới cố định (kế thừa từ `S13 Stage A -> S14`)

| Khoá luật / tham số | Giá trị cố định | Ghi chú |
|---|---:|---|
| `live.founders` | `100.0` | Khởi tạo tại `genesis` |
| `live.predator_fraction` | `0.0` | Không có thú săn mồi |
| `live.trees` | `8.0` | Rừng cây cố định |
| `live.lakes` | `16.0` | Số lượng hồ nước |
| `live.lake_layout` | `1.0` | Rải hồ trên lưới 2-D theo [Thẻ S12](2026-11-18-s12-lake-layout.md) |
| `live.resource_capacity_scale` | `0.63` | Lưu ý S6: nhân đồng thời đồng cỏ và tốc độ mọc quả `fruit_growth_system` |
| `live.plant_reserve` | `0.0` | Không giữ dự trữ đồng cỏ ($\rho = 0$) |
| `live.prey_cap` | `320.0` | Trần sinh sản mềm kiểm tra đầu system |
| `live.dehydration_ticks` | `960.0` | Ân hạn khát (giữ mặc định `starve_ticks = 120`, `food_cap = 50.0`) |
| `live.mortality` | `1.0` | Bật tử vong |
| `live.reproduction` | `1.0` | Bật sinh sản |
| `live.locomotion` | `1.0` | Bật vận động |
| `live.drive_floor` | `0.5` | Sàn động lực tìm ăn/nước |
| `live.thirst_steering` | `1.0` | Bật lái về phía nước |
| `live.evolved_brains` | `0.0` | Tắt tiến hoá não (`E2`), cô lập hình thái |
| `duration_ticks` | `200_000` | Độ dài mỗi lượt chạy |
| `warmup_ticks` | `20_000` | Cửa sổ làm ấm loại khỏi đầu ra chính |

- **Chu kỳ lấy mẫu (`sample_every_ticks`):**
  - `--smoke`: **`50` tick** (`4.000` mẫu/lượt, khớp độ phân giải đo `C1..C5` của `S13`/`S14`).
  - Ensemble xác nhận (`confirmation`): **`500` tick** (`400` mẫu/lượt, trong đó `360` mẫu hậu-warmup nằm trong `(20_000, 200_000]`).

### Kỷ luật mù & tập biến quan sát (`Observables`)

- **Pha `--smoke` (mù khối lượng, giống `S14`):** Chỉ được phép đọc tập biến nhân khẩu–tài nguyên không lộ hình thái: `live.agent_count`, `live.standing_crop`, `live.births`, `live.deaths`, `live.mean_lifespan`, `live.deaths_starvation`, `live.deaths_dehydration` (kèm các biến tích luỹ đối chứng tương ứng để kiểm `C2` ở cấu hình không bật `drift_control`). Cấm đọc `live.trait_mass_mean`, `live.trait_mass_var` hoặc bất kỳ chỉ số khối lượng/kiểu gen nào ở lượt `--smoke` hay các seed thăm dò `1..=6`.
- **Pha xác nhận (`confirmation`, chỉ mở đọc sau khi `SEL2-P-freeze` khoá manifest và qua smoke):** Đọc đầy đủ `live.trait_mass_mean`, `live.trait_mass_var`, `live.mean_energy_integral` (từ thẻ `S2`), `live.agent_count`, `live.standing_crop`, `live.births`, `live.deaths`, `live.mean_lifespan` (chú ý `evolution.births` mang `Aggregation::Instant`).
- **Ràng buộc kỹ thuật với `SEL2-D`:** Như ghi nhận ở [Thẻ SEL2-D](2026-11-18-sel2-d-drift-control.md) và `live_experiment.rs`, `ensure_observables_supported` từ chối `live.deaths_starvation` và `live.deaths_dehydration` khi `live.drift_control = 1.0`. Do đó manifest 3 nhánh cho ensemble xác nhận không đưa hai biến nguyên nhân tử vong này vào bộ observable bắt buộc của nhánh `scarce_drift` (xem Mục 8).

---

## 3. Chỉ tiêu đầu ra chính & Phân rã Robertson–Price

### Chỉ tiêu đầu ra tích phân thời gian (`Time-integrated endpoint`)

Theo [Đề xuất làm lại SEL2](2026-11-18-sel2-redesign-proposal.md) §5 và thiết kế biến tích phân từ thẻ `S2` (**đang phát triển trên nhánh `win/s2-observables`, chưa merge vào `feat/ecological-selection`**), quần thể ở cấu hình `16` hồ khan hiếm (`0.63`) trải qua các dao động mật độ; lấy tích phân thời gian trên toàn bộ hậu-warmup $t \in (T_{\text{warm}}, T] = (20\,000, 200\,000]$ giảm phụ thuộc vào pha dao động cuối lượt so với lấy trung bình 20% cuối thuần tuý.

- **Chỉ tiêu chính cho `H1`:** Giá trị trung bình tích phân thời gian hậu-warmup của khối lượng cơ thể kiểu gen (`live.trait_mass_mean`, ký hiệu $\overline{z}_g^{\text{int}}$) trên khoảng $(20\,000, 200\,000]$.
- **Chỉ tiêu năng lượng tích phân (`S2`):** Với biến tích luỹ `live.mean_energy_integral` ($I(t)$), trung bình hậu-warmup được tính đúng theo công thức hiệu tích phân:

$$\overline{E}_{\text{post}} = \frac{I(T) - I(T_{\text{warm}})}{T - T_{\text{warm}}}$$

- **Hai kiểm tra độ bền bổ trợ (`robustness checks`, chỉ báo cáo phụ — không tự đứng một mình thay thế `H1`):**
  1. Trung bình cửa sổ 20% cuối lượt chạy (`tick > 160_000`, tức `0.8 * T`, nhất quán với định nghĩa của `E-SEL1`) cho `live.trait_mass_mean`.
  2. Giá trị độ lệch chọn lọc ròng theo cửa sổ `window_net_selection_deltas["live.trait_mass_mean"]`.

### Phân rã Robertson–Price trên kiểu gen

Phương trình Robertson–Price tách biến thiên trung bình tính trạng giữa các thế hệ thành thành phần chọn lọc và thành phần truyền đạt/đột biến:

$$\Delta \overline{z}_g = \operatorname{Cov}(w, z_g) + \mathbb{E}[w \cdot \Delta z_g]$$

1. **Đo trực tiếp trên kiểu gen thay vì kiểu hình:** Vì thế giới sống hiện chưa kích hoạt lớp `DevelopedPhenotype` tách biệt co giãn phát triển khỏi kiểu gen, `AgentGenotype.0.total_mass()` đo đúng giá trị di truyền cộng gộp $z_g$. Biến `live.trait_mass_mean` phản ánh thuần sự thay đổi phân bố kiểu gen quần thể, không bị lẫn biến động tạm thời theo trạng thái no/đói.
2. **Cách ly $\operatorname{Cov}(w, z_g)$ nhờ ghép cặp `selected - scarce_drift`:** Cặp đối chứng `SEL2-D` (`scarce_drift`) giữ nguyên khung sinh/tử điều chỉnh (`LIVE_DRIFT_SELECTION`), tốc độ đột biến (`0.05`) và nhịp độ sinh thái vĩ mô nhưng cắt bỏ sự phụ thuộc sinh tồn/sinh sản vào mức năng lượng hay cơn khát riêng lẻ (`P_gen`/`P_pay`), nên phản ánh nền trôi dạt + sai lệch truyền đạt $\mathbb{E}[w \cdot \Delta z_g]$. Khi trừ kết quả của `scarce_drift` khỏi `selected` trên cùng một hạt giống, thành phần $\mathbb{E}[w \cdot \Delta z_g]$ và nhiễu khởi đầu được khử bớt để cô lập phần đóng góp của chọn lọc sinh thái $\operatorname{Cov}(w, z_g)$.

---

## 4. Thiết kế ba nhánh thực nghiệm (`Arms`)

Cả ba nhánh dùng cùng một thế giới cơ sở ở Mục 2 và cùng tập hạt giống xác nhận:

| Nhánh | `live.mutation_rate` | `live.drift_control` | `live.drift_reset_ticks` | Vai trò thực nghiệm |
|---|---:|---:|---:|---|
| `selected` | `0.05` | `0.0` | `0.0` | Nhánh chính: đủ chọn lọc sinh thái, đột biến và trôi dạt |
| `scarce_mut0` | `0.0` | `0.0` | `0.0` | Đối chứng đóng băng di truyền (`trait_mass_mean ≡ 3.3`, `trait_mass_var ≡ 0.0`) |
| `scarce_drift` (`SEL2-D`) | `0.05` | `1.0` | `0.0` | Đối chứng trôi dạt trung tính chạy qua `run_live_drift_ensemble` |

> Lưu ý triển khai nhánh `scarce_drift`: tuân thủ đầy đủ hợp đồng [Thẻ SEL2-D](2026-11-18-sel2-d-drift-control.md) — nếu xuất hiện thiếu hụt ngân sách (`shortfall`), runner trả về `RunStatus::Completed` kèm `DriftDiagnostics`, `CausalLedger` và `warnings` để kiểm định minh bạch ở phân tích cuối, không che giấu.

### Ý nghĩa của ba phép đối chiếu bắt cặp

1. **`selected - scarce_drift` (kiểm định giả thuyết chính `H1`):** Tách chọn lọc tự nhiên $\operatorname{Cov}(w, z_g)$ khỏi nhiễu trôi dạt và áp lực đột biến ở cùng `mutation_rate = 0.05`.
2. **`selected - scarce_mut0` (đối chiếu tổng dịch chuyển từ giá trị sáng lập `3.3`):** Vì `scarce_mut0` ghim chặt `live.trait_mass_mean ≡ 3.3`, hiệu số này đo toàn bộ độ lệch tích luỹ của quần thể có tiến hoá (`selected`) so với mức khởi đầu bất biến, đồng thời kiểm tra ảnh hưởng ngược của tiến hoá hình thái lên quy mô quần thể và tiêu thụ cỏ.
3. **`scarce_drift - scarce_mut0` (chẩn đoán thiên lệch đột biến & trôi dạt thuần):** Khi không có chọn lọc (`drift_control = 1.0`), hiệu số này đo thiên lệch truyền đạt $\mathbb{E}[w \cdot \Delta z_g]$ tự thân của phép đột biến và phạm vi trôi dạt ngẫu nhiên quanh mức `3.3`.

---

## 5. Bộ hạt giống, Lực thống kê & Dự toán thời gian chạy

### Tách biệt hạt giống tuyệt đối

- **Cấm dùng:** Bộ hạt giống thăm dò `1..=6` (đã dùng xuyên suốt `S6..S15`).
- **Tránh trùng mọi dải đã đăng ký trước:**
  - `E2`: `700001..`
  - `E3`: `730001..`
  - `E2R`: `750001..`
  - `E-SEL1`: `760001..`
- **Hạt giống `--smoke`:** **`999961`** (`SeedNotInManifest` — không thuộc tập seed xác nhận, không thuộc `1..=6`, chỉ chạy ở chế độ mù khối lượng để kiểm `C1..C5`).
- **Bộ hạt giống xác nhận đề xuất (`N = 20` cặp):** Dãy `780001 + 1000 * k` với `k = 0..19` (tức `780001, 781001, ..., 799001`), cách đều `1.000` giữa các seed `ChaCha12Rng` và hoàn toàn rời khỏi các thí nghiệm trước.
- **Số cặp hoàn chỉnh tối thiểu:** `min_complete_pairs = 16` (Mục 7).

### Bảng lực thống kê (`Statistical Power`)

Với kiểm định hai phía $\alpha = 0.05$, lực thống kê $1 - \beta = 0.80$ trên sai khác bắt cặp (từ [Đề xuất làm lại SEL2](2026-11-18-sel2-redesign-proposal.md)):

| Số cặp hoàn chỉnh `N` | Cỡ hiệu ứng phát hiện được ($d_z$) | Ghi chú |
|---:|---:|---|
| `12` | `≈ 0.89` | Dưới mức `min_complete_pairs` đề xuất |
| `16` | `≈ 0.75` | Ngưỡng sàn chấp nhận phân tích (`min_complete_pairs = 16`) |
| **`20`** | **`≈ 0.66`** | **Mức đề xuất chính của bản đăng ký (`SEL2-P`)** |
| `24` | `≈ 0.60` | Tuỳ chọn mở rộng nếu chủ dự án muốn nhạy hơn |

### Dự toán thời gian chạy thực tế (dựa trên số đo Dell `200.000` tick)

- Số đo nền trên Dell:
  - `S13 Stage A`: `48` lượt × `200.000` tick = `2.759,81 giây` = `46,00 phút` (`≈ 0,96 phút/lượt`).
  - `S14` (`16` hồ, `layout = 1`, `scale ∈ {0.63, 0.65, 0.67}`): `24` lượt × `200.000` tick = `2.817,63 giây` = `46,96 phút` (`≈ 1,96 phút/lượt` ở mật độ sống cao của `16` hồ).
- Dự toán cho `N = 20` cặp × `3` nhánh = **`60` lượt chạy `200.000` tick**:
  - Trên **Dell**: khoảng **`58–117 phút`** (`~1–2 giờ`, nghiêng về đầu trên `~117 phút` vì mật độ ở `16` hồ `scale = 0.63` tương đương `S14`).
  - Trên **Case**: dự kiến **`< 1 giờ`** nhờ số nhân luồng cao hơn.

---

## 6. Cổng khả thi Smoke (`C1..C5`) & Ô tham chiếu `g` đang để trống

Mọi cấu hình chính thức phải vượt qua **cả 5 điều kiện `C1..C5`** trên lượt `--smoke` (`seed = 999961`, lấy mẫu mỗi `50` tick, **mù với `trait_mass_*`**) trước khi được phép chạy bất kỳ hạt giống xác nhận nào:

| Điều kiện | Định nghĩa gating trên `--smoke` (`seed = 999961`, `200.000` tick) |
|---|---|
| **`C1`** | `min(live.agent_count) >= 5` trên toàn bộ lượt `selected` và `scarce_mut0` (không nhánh nào tuyệt chủng). |
| **`C2`** | Hệ số gặm đồng cỏ nửa sau (`100.001..200.000` tick) $g = (\overline{C} - \rho \cdot \Sigma r_{\max}) / (\overline{C}_{\text{nền}} - \rho \cdot \Sigma r_{\max}) \in [0.4, 0.6]$ (với $\rho = 0.0$ khi `plant_reserve = 0.0`); **cách xác định $\overline{C}_{\text{nền}}$ xem ô quyết định bên dưới**. |
| **`C3`** | $\min(200\,000 / \overline{L},\; \text{births} / \overline{N}) \ge 30$ thế hệ trên `selected` và `scarce_mut0`. |
| **`C4`** | Ở nhánh `selected` và `scarce_mut0`, tỉ lệ chết vì đói `deaths_starvation / deaths > 0.50` cùng bất biến `deaths_starvation + deaths_dehydration == deaths` (không áp cho `scarce_drift` vì `SEL2-D` không đọc 2 observable nguyên nhân chết). |
| **`C5`** | Trong một phần tư cuối lượt (`150.001..200.000` tick), cả `births_last_q > 0` lẫn `deaths_last_q > 0` đều khác `0`. |

### Chỗ quyết định của chủ dự án — định nghĩa tham chiếu cho `g` (`C2`)

[OWNER DECISION 2026-11-19 12:00: reference for g]

Phần này cố ý **để trống** để chủ dự án quyết định phương án định nghĩa $\overline{C}_{\text{nền}}$ trong công thức `C2`. Dưới đây là toàn bộ số liệu đối chiếu từ `S14` (`2026-11-19-s14-lakes16-scale.md`) và `S15` (`2026-11-19-s15-reference-check.md`, đang chờ `PR #146`) cho cấu hình ứng viên (`16` hồ, `lake_layout = 1`, `reserve = 0`, `scale = 0.63`, `200.000` tick, `seeds 1..=6`), trình bày trung lập cả ba phương án không lựa chọn thay:

1. **Phương án (a) — Đối chứng cùng seed với `prey_cap = 80, scale = 1.0`:**
   - **Tình trạng đối chứng:** Chỉ `4/6` seed đối chứng sống tới `200.000` tick; `seed 1` tuyệt chủng tại tick `8.545` và `seed 3` tuyệt chủng tại tick `20.787`.
   - **Standing crop đối chứng nửa sau:** `113.036,9..114.042` ở 4 seed sống (`seeds 2, 4, 5, 6`), vọt lên `153.586,9` (`sum_r_max` khi đồng cỏ không còn ai ăn) ở 2 seed đối chứng tuyệt chủng sớm (`seeds 1, 3`).
   - **Giá trị `g` thu được ở `scale = 0.63` (`seeds 1..=6`):** `0.322, 0.528, 0.299, 0.520, 0.546, 0.558` (khoảng `0.299..0.558`; **`4/6` seed sống nằm trong `[0.4, 0.6]`**, chỉ 2 seed có đối chứng tắt sớm rơi xuống dưới `0.4`).

2. **Phương án (b) — Trung bình gộp các đối chứng sống `prey_cap = 80, scale = 1.0`:**
   - **Tình trạng đối chứng:** Lấy trung bình standing crop nửa sau trên `4` seed đối chứng sống (`seeds 2, 4, 5, 6`, độ phân tán giữa 4 seed sống `< 1%`): **$\overline{C}_{\text{nền, pooled}} = 113\,430.7$**.
   - **Giá trị `g` thu được ở `scale = 0.63` (`seeds 1..=6`):** `0.436, 0.531, 0.405, 0.518, 0.544, 0.559` (khoảng `0.405..0.559`; **`6/6` seed nằm trong `[0.4, 0.6]`**).

3. **Phương án (c) — Đối chứng cùng seed với `prey_cap` cao hơn ở `scale = 1.0` (kèm số đo chẩn đoán `g_ungrazed`):**
   - **Với `prey_cap = 120, scale = 1.0`:** `6/6` seed đối chứng sống; standing crop đối chứng `88.013,1..98.915,1`; cho `g = 0.552..0.648` (trung vị `0.604`; **`3/6` seed nằm trong `[0.4, 0.6]`**).
   - **Với `prey_cap = 160, scale = 1.0`:** `6/6` seed đối chứng sống; standing crop đối chứng `73.562,6..89.716,0`; cho `g = 0.612..0.707` (trung vị `0.683`; **`0/6` seed nằm trong `[0.4, 0.6]`**).
   - **Số đo chẩn đoán `g_ungrazed` (mẫu số là $\sum r_{\max}$ của đồng cỏ không gặm ở `scale = 1.0`, chỉ mang tính tham khảo):** cho `0.299..0.413` ở `scale = 0.63` (`2/6` seed trong `[0.4, 0.6]`).

---

## 7. Kế hoạch phân tích, Quy tắc dừng, Kết quả rỗng & Phạm vi tuyên bố

### Thống kê bắt cặp & quy tắc loại cặp không toàn vẹn

- **Thống kê bắt cặp (`PairedEffect`):** Tính hiệu theo từng hạt giống hoàn chỉnh (`selected - scarce_drift` cho `H1`, cùng `selected - scarce_mut0` và `scarce_drift - scarce_mut0` cho phân tích bổ trợ) trên chỉ tiêu tích phân thời gian hậu-warmup của `live.trait_mass_mean`.
- **Tuyệt chủng & lỗi kỹ thuật:** Bất kỳ lượt nào có `live.agent_count == 0` ở bất kỳ mẫu nào hoặc ở cuối lượt đều bị coi là **tuyệt chủng**, và bất kỳ lỗi runner nào đều **loại toàn bộ cặp chứa seed đó khỏi thống kê bắt cặp** nhưng phải **ghi nhận đầy đủ (`seed`, `arm`, `tick`)** trong bảng chạy (`runs.csv`) và báo cáo tổng hợp (`effects.json` / `paired-report.json`); tuyệt đối không bỏ qua âm thầm.

### Tiêu chí bác bỏ `H0` (`Materiality`)

`H1` chỉ được chấp nhận khi **cả ba điều kiện** sau đồng thời thoả trên phép so sánh chính `selected - scarce_drift` với tối thiểu `16` cặp hoàn chỉnh:

1. Khoảng tin cậy `95% CI` của `PairedEffect` không chứa `0`;
2. Cỡ hiệu ứng chuẩn hoá `|d_z| >= 0.66` (tương ứng lực thiết kế ở `N = 20`);
3. Độ lớn hiệu ứng tuyệt đối đạt ít nhất `|delta| >= 5% * |control_mean|` (trong đó `control_mean` là trung bình trên các cặp hoàn chỉnh của nhánh đối chứng `scarce_drift`).

### Quy tắc dừng & chân trời cố định (`Stopping Rules`)

1. **Chân trời cố định (`fixed horizon`):** Đúng `N = 20` cặp hạt giống (`780001 + 1000 * k`), không nhìn trước kết quả giữa chừng để dừng sớm hay chạy bù tăng `N`.
2. **Dừng cứng ở `--smoke`:** Nếu lượt `--smoke` (`seed = 999961`) trượt bất kỳ điều kiện `C1..C5` nào (hoặc `SEL2-D` không hoàn tất hợp lệ), dừng cứng toàn bộ thí nghiệm — **`0` hạt giống xác nhận được phép chạy**.
3. **Thiếu chứng cứ (`insufficient_evidence`):** Nếu số cặp hoàn chỉnh ở ensemble xác nhận `< 16` (`min_complete_pairs = 16`), dừng diễn giải suy luận và kết luận **`insufficient_evidence`**.

### Định nghĩa kết quả rỗng (`Null Result`) & Nghĩa vụ công bố

- Nếu thu được `>= 16` cặp hoàn chỉnh nhưng không thoả đồng thời cả 3 tiêu chí bác bỏ `H0` nêu trên, kết luận chính thức là **kết quả rỗng / âm tính (`null`)**: chưa thấy bằng chứng chọn lọc sinh thái dịch chuyển thực chất khối lượng cơ thể kiểu gen vượt trên mức trôi dạt + đột biến ở cấu hình này.
- Bắt buộc xuất bản `RESULT.md` và bộ tạo tác kiểm chứng dù kết quả dương tính, âm tính (`null`) hay `insufficient_evidence`.

### Không khẳng định (`Out of scope`)

Thống nhất với [CLAUDE.md](../../../CLAUDE.md), [Bản đăng ký E-SEL1](2026-11-16-experiment-e-sel1-food-scarcity-body-mass.md) và bài học đa máy từ `F045`/`F047`/`F048`, thí nghiệm này **không khẳng định**:

1. Khớp hành vi hay hiệu năng trong ứng dụng cửa sổ GUI, GPU hay đa luồng trực tiếp (trạng thái chỉ là **`headless adapter verified`**).
2. Tương đương với quần thể tham chiếu `AE3` hay chế độ năng lượng ngoại lai (`exotic_energy: None`).
3. Sự tiến hoá của não (`live.evolved_brains = 0.0`) hay sự tách biệt giữa kiểu hình phát triển và kiểu gen (`DevelopedPhenotype` chưa độc lập với `MorphologyGenotype::total_mass()`).
4. Khả năng tổng quát hoá ra ngoài lát cắt tham số cố định (`16 lakes, lake_layout = 1, scale = 0.63, reserve = 0, prey_cap = 320, founders = 100, dehydration_ticks = 960`).
5. Tính tương đương xuyên máy hoặc xuyên commit khi có khác biệt nhị phân trước `F045`/`F048`: chỉ được phép phân tích các lượt chạy từ cùng `LIVE_MODEL_VERSION = "live-bevy-world/2"`, `SCHEMA_VERSION = 8` và đã qua cổng xác nhận khôi phục `checkpoint/restore` không lệch quỹ đạo trên đúng phiên bản mã đóng băng.

---

## 8. Các phụ thuộc mở trước khi đóng băng (`SEL2-P-freeze`)

Không thẻ `SEL2-P-freeze` hoặc lượt chạy xác nhận nào được mở trước khi giải quyết xong sáu điểm sau:

1. **Quyết định của chủ dự án về tham chiếu `g`:** Điền lựa chọn chính thức vào `[OWNER DECISION 2026-11-19 12:00: reference for g]` tại Mục 6 (`(a)`, `(b)`, `(c)` hoặc quy tắc mới) để khoá định nghĩa `C2`.
2. **Hợp nhất thẻ `S2` (`win/s2-observables`) vào `feat/ecological-selection`:** Cần chốt tên/ngữ nghĩa observable `live.mean_energy_integral`, chứng minh bộ tích luỹ năng lượng được giữ toàn vẹn khi dùng `from_snapshot_with_drift_schedule` ở nhánh `SEL2-D` (`scarce_drift`), và xác nhận phép tích phân hậu-warmup tương tự cho `live.trait_mass_mean`.
3. **Hợp nhất `PR #146` (`F048` + `S15`) vào `feat/ecological-selection`:** Khoá ổn định bản ghi đối chứng `2026-11-19-s15-reference-check.md` và các bản vá trung thực khôi phục (`F047`/`F048` nối tiếp [Thẻ F045](2026-11-18-f045-restore-fidelity.md)) trước khi chạy phân tán.
4. **Tương thích danh sách observable giữa `validate_drift_pair_manifests` và `SEL2-D`:** Hiện `validate_drift_pair_manifests` kiểm tra `FactorDiff` rồi gọi `ensure_observables_supported`, vốn từ chối `live.deaths_starvation` và `live.deaths_dehydration` khi `live.drift_control = 1.0`. Ở bước `SEL2-P-freeze`, cần xác nhận việc lọc hai ID này khỏi manifest xác nhận 3 nhánh (trong khi lượt kiểm `--smoke` cho `C4` đọc trên nhánh không drift `selected`/`scarce_mut0`).
5. **Chốt có hay không bổ sung observable hiệp phương sai Robertson–Price trực tiếp:** Quyết định xem `SEL2-P-freeze` chỉ dùng ước lượng qua cặp đối chứng `selected - scarce_drift` + `live.trait_mass_var`, hay bổ sung thêm observable ghi trực tiếp hiệp phương sai $\operatorname{Cov}(w, z_g)$ trên từng sự kiện sinh/tử.
6. **Mở thẻ thực thi `SEL2-P-freeze` sau khi mục 1–5 hoàn tất:** Nâng `status` bản đăng ký chính thức (`accepted` / `active`), commit bộ fixture manifest JSON cho `selected` / `scarce_mut0` / `scarce_drift`, bổ sung test kiểm định cấu trúc manifest & quy tắc mù smoke, và triển khai runner `run_sel2_experiment` + script kiểm chứng `verify_sel2_artifacts.mjs`.
