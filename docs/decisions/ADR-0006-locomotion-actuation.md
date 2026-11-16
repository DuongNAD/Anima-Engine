---
title: ADR-0006 — Vận động: sinh vật di chuyển bằng cách nào (F-042)
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-16
decision_date: 2026-11-16
supersedes: none
superseded_by: none
---

# ADR-0006 — Vận động của sinh vật (F-042)

> **Trạng thái: đề xuất, chờ chủ dự án.** Chưa có dòng `QĐ-LOC` nào được điền. **QĐ-LOC là quyết
> định mới**, chưa có trong `AGENT_WORKFLOW.md` §4. Số liệu ở "Bằng chứng" **chưa đăng ký trước,
> mang tính thăm dò, đo 2026-11-16 bằng prototype scratch (không nằm trong cây mã) trên `63bc0d5`**,
> build release, adapter live, 10 founder = 3 thú săn + 7 con mồi; không phải kết quả thí nghiệm.

## Bối cảnh

Sinh vật **không tịnh tiến được** (F-042, [`FINDINGS.md`](../../FINDINGS.md)). Các mỏ neo đã kiểm lại
trên nhánh này (`63bc0d5`):

- [`dynamics.rs`](../../src-tauri/src/physics/dynamics.rs) 110-111: `child_body.force += f_spring;
  parent_body.force -= f_spring`. Lực khớp là **nội lực**, tổng bằng 0, không dịch được khối tâm.
- `integrate_physics_system` (`dynamics.rs` 207-213) chỉ làm `velocity += force/mass * dt` rồi
  `pos += vel * dt`. Không có tiếp xúc nền, ma sát, cản hay lực ngoài nào. (Dòng 187-205 là nhánh
  hết năng lượng: xoá vận tốc.)
- Lực ngoài duy nhất là `(target_velocity - velocity) * mass * 2.0` (`dynamics.rs` 153-157), chỉ khi
  `target_velocity` khác 0. Trường này (`components.rs` 465, mặc định `Vec3::ZERO` ở 475) **không hệ
  nào ghi** ở `src/`.
- Nguồn dịch chuyển duy nhất là xung sinh trong `SpawnOffspringCommand::apply`
  (`agent_systems.rs` 481-494: hướng ngẫu nhiên, 2.0 thú săn / 1.5 con mồi), áp lên mọi segment
  (khoảng 569). Đã đo: xung ngẫu nhiên không phải cách sửa.
- Đo gốc: độ dịch ròng lớn nhất 0.03 qua 1.000 tick trong khi đường đi 0.53 (CPG chạy thật, biên độ
  khoảng 1.3). Test ghim: `src-tauri/tests/no_locomotion_characterization_tests.rs ::
  founders_oscillate_but_do_not_translate` (assert `max_net < 0.5`, dòng 87-91; **phải đảo** khi có
  vận động).

Hệ quả: thú săn founder không tới được con mồi, và fitness sinh thái mặc định (Locomotion) **không có
vận động để chọn lọc**. Sửa là đổi ngữ nghĩa vật lý nên cần `LIVE_MODEL_VERSION` mới
(`live_experiment.rs` 67, `"live-bevy-world/1"`) và mọi checksum thế giới live đổi.

Lịch tick hiện là **một chuỗi toàn phần** sau F-041
([`simulation_schedule.rs`](../../src-tauri/src/core/simulation_schedule.rs), `build_tick_schedule`
từ dòng 121; chuỗi đầu có `sensory_system` → `action_resolution_system` → `update_cpg_system` →
`resolve_joints_system` → `integrate_physics_system`). Thêm hệ là thêm một nút vào chuỗi đó và phải
cập nhật `schedule_ambiguity_tests` / `schedule_total_order_tests`.

## Bằng chứng (thăm dò, chưa đăng ký)

Prototype scratch, chỉ đo, không commit; adapter live, 10 founder.

**(a) Cản dị hướng theo trục từng thân** (`force += −m·(c_par·v_par + c_par·ratio·v_perp)`):
**không có lực đẩy**. Độ dịch ròng tối đa giữ ≈ 0.027 sau 1.000 tick ở mọi tham số (`c_par` 1-2,
`ratio` 2-10), trong khi đường đi giảm từ 0.52 xuống 0.10-0.29. Nguyên nhân: `Rotation` của gốc cố
định và CPG 3 segment là dao động đứng quanh một trục cố định, nên lực cản triệt tiêu qua mỗi chu kỳ;
lực đẩy từ ma sát cần **sóng lan truyền lệch pha**, một thay đổi CPG lớn hơn. Với tử vong + sinh sản
(6.000 tick, seed 1-3), **con mồi tuyệt chủng ở cả 3 seed** (t ≈ 1.400-4.200) vì ma sát lấy mất sự
trôi theo xung sinh vốn là chuyển động duy nhất. (a)+(b): con mồi cũng tuyệt chủng 3/3.

**(b) Bộ lái ghi `target_velocity`**: một hệ trước `integrate_physics_system` ghi
`target_velocity` = vector đơn vị tới mục tiêu gần nhất (thú săn → con mồi, con mồi → thức ăn; khoảng
cách tối thiểu trên torus; dừng trong 0.3) × tốc độ tối đa của lớp (2.0 / 1.5) × hệ số `s`. Lực hướng
tới `target_velocity` có sẵn đẩy gốc; chi phí hoạt động đã chảy qua `total_cost`
(`k_velocity * speed`, `agent_systems.rs` 852-861). Đường đi trung bình mỗi 1.000 tick:

| s | 0.5 | 1 | 2 | 3 |
|---|---|---|---|---|
| Thú săn | 15.6 | 98 | 125 | 141 |
| Con mồi | 11.5 | 21.7 | 42 | 62 |

(số ròng khi s ≥ 1 bị thổi phồng do vòng torus). Con mồi sống sót ở mọi seed khi s ≥ 1 (4-55 con tại
t=6.000); ở s=0.5 tuyệt chủng ở seed 1 và 3 (t ≈ 3.600-3.800). Thú săn **tới được** con mồi (năng
lượng thu gộp 12-213 EU so với 0 ở nền) nhưng vẫn chết ở 8/9 run vào t ≈ 1.500-2.400: giới hạn bây
giờ là **cân bằng năng lượng của thú săn**, không phải khả năng tiếp cận.

**Chung cho mọi prototype:** `closed_eu_total` hằng (biên độ ≤ 2.4e-9 trên 60 mẫu, nhiễu f64); cùng
seed tất định (hai lần chạy trong tiến trình cho cùng checksum; giá trị tuyệt đối không ghim). Với
(b) s=1, toàn bộ bộ test desktop chỉ có **đúng một test đỏ: test F-042 (đảo có chủ đích)**; không
golden hay test ghim nào đổi. **Ghi chú độ phủ:** nhiều khả năng vì các test ghim dùng lịch dựng tay
hoặc sinh vật không có mục tiêu, nên "không đổi" ở đây *không* chứng minh các golden live được phủ.

**Giới hạn của prototype (b):** quét mọi con mồi/thức ăn O(n·m) mỗi tick (bản thật phải dùng lại bộ
đệm của `sensory_system` hoặc lưới không gian); `sensory_system` (`agent_systems.rs` 1205) **không**
dùng khoảng cách tối thiểu trên torus còn prototype thì có (phải thống nhất); tốc độ cố định thì chọn
lọc không có biến thiên di truyền để tác động.

## Quyết định cần đưa ra

**QĐ-LOC** (mới) — "Sinh vật tạo lực đẩy ngoài bằng cơ chế nào, nếu có?"

### Phương án A — Bộ lái ghi `target_velocity`, tốc độ lấy từ genome/não (khuyến nghị)

- *Mã*: hệ mới (ví dụ `locomotion_drive_system`) trong `agent_systems.rs`, ghi
  `InertiaComponent.target_velocity` từ mục tiêu tri giác (tái dùng đầu ra của `sensory_system` thay
  vì quét lại) × **tốc độ di truyền**: biên độ CPG hoặc một đầu ra của não (`BrainGenotype`) thay cho
  hằng 2.0/1.5. Không đổi `integrate_physics_system`: dùng lại lực `dynamics.rs` 153-157. Thống nhất
  quy ước khoảng cách torus giữa `sensory_system` và bộ lái.
- *Luật cứng CLAUDE.md*: tốc độ ở genome thì không chạm ADR-0003 (không Lamarck: không ghi `learned`
  ngược vào `.genotype`); nếu dùng đầu ra não thì não vẫn bốc theo genotype, restore/migration mang
  nguyên não; đường legacy phải còn với `AgentBrain` vắng (tốc độ lớp mặc định). Không cấp phát heap
  trong tick: dùng buffer có sẵn/`Local` (prototype O(n·m) chưa đạt).
- *Năng lượng / EU kín*: không khoản trừ mới; chi phí vận động đã nằm trong `total_cost` của
  `metabolic_decay_system` (đo: EU kín hằng).
- *Tất định*: không dùng RNG; mục tiêu gần nhất cần phá hoà bằng thứ tự ổn định (id dòng dõi) để
  cùng seed cùng thế giới (đã đo hai lần chạy khớp).
- *Lịch (F-041)*: một nút mới, đặt sau `sensory_system` và trước `integrate_physics_system` trong chuỗi
  toàn phần duy nhất; cập nhật thứ tự khai báo và các cổng thứ tự/ambiguity; không được tạo cặp mới
  không xác định thứ tự.
- *Test*: **đảo** `founders_oscillate_but_do_not_translate` thành "founder tịnh tiến" (ngưỡng chọn
  sau khi đo, không lấy số của prototype); thêm: thú săn tới con mồi trong N tick (năng lượng thu
  từ con mồi > 0); đường đi con mồi > X; đột biến tốc độ di truyền thì đường đi đổi; hướng đúng qua
  biên torus; checksum nhiều tiến trình; EU kín.
- *Bằng chứng*: (b). Có vận động thật, con mồi sống ở s ≥ 1.
- *Rủi ro*: `LIVE_MODEL_VERSION` mới, mọi checksum live đổi; nếu tốc độ cố định thì di chuyển là
  hành vi viết tay (nên tốc độ phải di truyền); thú săn vẫn đói (ngoài phạm vi).

### Phương án B — Ma sát/cản dị hướng, sau khi có CPG sóng lan truyền

- *Mã*: lực cản theo trục từng thân trong `dynamics.rs` (sau `resolve_joints_system`, trước tích
  phân) **và** viết lại `update_cpg_system` thành sóng lan truyền lệch pha giữa các segment (điều
  kiện cần: bằng chứng (a) cho thấy dao động đứng không sinh lực đẩy).
- *Luật cứng*: chạm CREATURE_DEVELOPMENT_CONTRACT (thân thể/CPG là phần phenotype); không cấp phát;
  chi phí vẫn qua `total_cost`.
- *Tất định / EU*: không RNG, EU kín hằng (đo). Lịch: có thể không thêm nút nếu nằm trong
  `integrate_physics_system`.
- *Test*: đảo test F-042; thêm test tịnh tiến từ sóng lan truyền; phản ví dụ "dao động đứng ⇒ ròng 0".
- *Bằng chứng*: (a): **một mình ma sát cho 0 lực đẩy và làm con mồi tuyệt chủng 3/3**. Phạm vi lớn,
  chưa có prototype CPG.
- *Lợi*: vận động nổi lên từ thân thể (đúng tinh thần artificial life).

### Phương án C — Cả hai

B cho lực đẩy cơ bản, A cho điều hướng. Gồm toàn bộ chi phí của B (CPG mới) cộng A. Bằng chứng
(a)+(b) mới đo ở dạng ma sát-không-đẩy cộng lái, nơi con mồi tuyệt chủng 3/3. Hoãn tới khi A có kết
quả và B có nhu cầu cụ thể.

### Phương án D — Giữ nguyên

Không có vận động: thú săn không tới được con mồi, fitness Locomotion không có gì để chọn lọc, test
F-042 giữ nguyên là test ghim; chuyển động duy nhất là xung sinh. Mọi thí nghiệm chọn lọc trên thế
giới live đo quần thể không chọn lọc theo vận động. Không khuyến nghị.

## Quyết định đề xuất

**QĐ-LOC = A, với tốc độ di truyền (từ genome hoặc đầu ra não, không phải hằng theo lớp).** Lý do: A là
phương án duy nhất đã **tạo ra vận động thật** trong đo thăm dò, tái dùng lực `target_velocity` đã có
(không động tới tích phân vật lý), giữ EU kín qua `total_cost` và chỉ thêm một nút vào chuỗi toàn
phần. B không tạo lực đẩy nếu không viết lại CPG; làm B trước là đặt cược vào một thay đổi lớn mà
bằng chứng hôm nay chưa ủng hộ. Tốc độ phải di truyền, nếu không chọn lọc không có biến thiên để tác
động và fitness Locomotion vẫn vô nghĩa.

**ADR này KHÔNG quyết:**

- **Cân bằng năng lượng của thú săn**: thú săn tới được con mồi nhưng chết ở 8/9 run (t ≈
  1.500-2.400). Hướng để *nghiên cứu, chưa quyết*: lượng EU thu khi giết, chuyển hoá thú săn, mật độ
  con mồi, dự trữ founder. Thẻ riêng sau khi A vào.
- Bất kỳ khẳng định thích nghi, hình thành loài hay "vận động tiến hóa"; theo cổng AE-S11/AE-S14.
- Giá trị tham số `s`/hằng tốc độ (0.5-3 chỉ là khảo sát), cách nối đầu ra não, và việc đưa vào
  manifest một khoá `live.locomotion` hay làm mặc định (đề xuất: mặc định, cùng `LIVE_MODEL_VERSION` mới).
- S43 (Red-Queen), sinh sản hữu tính, phương án B/C.

## Hệ quả

- **Tích cực**: Locomotion thành tính trạng chọn lọc được; thú săn có thể tới con mồi; đảo được test
  ghim F-042.
- **Tiêu cực**: mọi checksum live đổi (bump `LIVE_MODEL_VERSION`); cổng lịch phải sửa; thú săn vẫn dễ
  tuyệt chủng đến khi giải quyết cân bằng năng lượng; độ phủ của phát hiện "không golden nào đổi" chưa rõ.
- **Thẻ tiếp theo**
  1. Thẻ A: hệ lái + tốc độ di truyền + bump `LIVE_MODEL_VERSION` + cập nhật cổng lịch + test (đảo
     F-042, thú săn tới con mồi, đường đi con mồi > X đo trước, tất định nhiều tiến trình, EU kín).
  2. Thẻ cân bằng năng lượng thú săn (nghiên cứu, đăng ký trước).
  3. Tuỳ chọn: B, sau khi CPG sóng lan truyền được thiết kế riêng.

## Dòng quyết định để dán

Thay `<ngày>`. Phương án A (khuyến nghị):

```text
QĐ-LOC (owner, <ngày>) = A: a steering drive writes InertiaComponent.target_velocity toward the
            nearest target; speed derives from the genome/brain (heritable), not a per-class
            constant; new LIVE_MODEL_VERSION; ADR-0006
```

Phương án B:

```text
QĐ-LOC (owner, <ngày>) = B: anisotropic resistive contact, preceded by a travelling-wave CPG
            redesign; no steering drive; ADR-0006
```

Phương án C:

```text
QĐ-LOC (owner, <ngày>) = C: travelling-wave CPG + anisotropic friction for thrust, plus a steering
            drive for heading; ADR-0006
```

Phương án D:

```text
QĐ-LOC (owner, <ngày>) = D: keep as is; creatures do not translate; F-042 stays a pinned
            characterization; no selection on movement; ADR-0006
```

Sau khi chủ dự án điền: đổi `status` thành `accepted`, điền `decision_date`, cập nhật chỉ mục.

## Quyết định (2026-11-16)

**QĐ-LOC = A**, đúng phương án khuyến nghị ở trên. Quyết định do phiên Claude lead đưa ra theo uỷ quyền của chủ
dự án trong chat ngày 2026-11-16 ("bạn được phép tự làm theo bản kế hoạch và xử lý sao cho tốt nhất").
Chủ dự án có thể đảo lại bất cứ lúc nào: đổi `status`, ghi dòng quyết định mới vào khối `DECISIONS` của
[`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0).

