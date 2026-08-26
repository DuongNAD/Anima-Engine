---
phase: design
feature: creature-animation
title: Design — Animation sinh vật
description: Gait thủ tục từ CPG, action overlay, và làm mượt giữa tick rate và frame rate
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
design: ./2026-08-25-feature-genome-to-mesh.md
requirements: ../requirements/2026-08-25-feature-living-world.md
testing: ../testing/2026-08-25-feature-living-world.md
---

# Design — Animation sinh vật

> Bộ tài liệu đầy đủ: [Requirements — Thế giới sống](../requirements/2026-08-25-feature-living-world.md).

Sinh vật phải **đi, ăn, uống, đánh nhau, ngủ, nghỉ**, và các chuyển động đó phải mượt.

---

## 1. Ràng buộc quyết định mọi thứ: hình thái là biến, không phải hằng

Trong một game thông thường, animator dựng clip cho từng nhân vật đã biết trước. Ở đây **không
biết trước con vật nào sẽ tồn tại**: `MorphologyGenotype` là graph tiến hoá, mutation có thể tạo
con ba chi, sáu chi, hoặc không chi nào (`MAX_MIGRATION_MORPHOLOGY_NODES = 128`).

Không thể author clip cho một hình thái chưa ra đời. Vì vậy:

> **Gait là thủ tục, sinh từ CPG và topology. Action là overlay ngắn, cũng thủ tục.**
> Không có thư viện clip nào trên đường sống.

Đây cũng là lý do bước bind skeleton ở
[thiết kế genome-to-mesh §4](./2026-08-25-feature-genome-to-mesh.md) quan trọng: bộ xương đến từ
`nodes`/`edges` của phenotype, nên **mesh sinh bởi AI cử động bằng chính CPG đang chạy**, không
phụ thuộc auto-rigging của nhà cung cấp.

---

## 2. Animation không được quyết định gì

Nguyên tắc cứng: **tầng animation đọc trạng thái mô phỏng và chọn cách vẽ; nó không sinh ra trạng
thái.** Nếu animation tự giữ state riêng, state đó sẽ trôi khỏi cái con vật thực sự đang làm, và
người xem sẽ thấy một con vật gặm cỏ trong khi ledger ghi nó đang chết đói.

Trạng thái đã có sẵn trong engine để dẫn xuất:

| Nguồn | Symbol | File |
|---|---|---|
| Pha dao động chi | `CpgOscillator { phase, frequency, amplitude, output }` | `src/ai/cpg.rs:4` |
| Tham số gait do não xuất | `InertiaComponent.cpg_parameters: [f32; 4]` | `src/core/components.rs:460` |
| Ý định hành động | `ActionGates { pheromone_emit, attack_intent, feed_intent }` | `src/core/components.rs:495` |
| Sinh lý | `HomeostaticState { energy, hydration, temperature }` | `src/ai/hrrl.rs` |
| Vận tốc | `Velocity`, `RigidBody.velocity` | `src/core/ecs.rs` |
| Mức chi tiết | `LodTier { Hot, Warm, Cold }` | `src/core/simulation_lod.rs:49` |

Ghi chú đúng với code hiện tại: comment tại `components.rs:486` nói rõ **não là bộ điều khiển
gait, không phải bộ ra quyết định** — ăn và tấn công xảy ra tự động theo khoảng cách, gate chỉ
điều biến cường độ. Animation phải phản ánh đúng ngữ nghĩa đó, không được vẽ một cú tấn công có
chủ đích khi thực chất là va chạm theo khoảng cách.

---

## 3. Hai tầng trạng thái

### 3.1 Locomotion base — liên tục, do CPG lái

| State | Điều kiện dẫn xuất |
|---|---|
| `Sleep` | `speed ≈ 0` ∧ `energy` thấp ∧ đã đứng yên > N tick |
| `Rest` | `speed ≈ 0` ∧ `energy` trung bình |
| `Idle` | `speed ≈ 0` ∧ `energy` cao |
| `Walk` | `0 < speed ≤ walk_max` |
| `Run` | `speed > walk_max` |
| `Swim` | `medium = Aquatic/Amphibious` ∧ `speed > 0` |
| `Fly` | `medium = Aerial` |

Đây **không** phải các clip rời. Chúng là một không gian liên tục tham số hoá bởi
`(frequency, amplitude, duty, phase_offset_per_limb)`; tên state chỉ là nhãn cho vùng tham số.
`Walk → Run` là tăng `frequency` và `amplitude`, không phải đổi clip.

`Sleep`/`Rest` khác `Idle` ở chỗ `amplitude → 0` cộng một dao động thở biên độ rất nhỏ, tần số
thấp, lệch pha theo `agent_id`. Một con vật ngủ mà đứng hình tuyệt đối trông như chết.

### 3.2 Action overlay — rời rạc, ngắn, cộng thêm

| State | Điều kiện | Thời lượng |
|---|---|---|
| `Feed` | `gates.feeds()` ∧ có `Food` trong tầm | ~0,6 s |
| `Drink` | thiếu `hydration` ∧ đứng trên ô nước ∧ `speed ≈ 0` | ~0,8 s |
| `Attack` | `gates.attacks()` ∧ có mục tiêu trong tầm | ~0,4 s |
| `Hurt` | `energy` giảm > ngưỡng trong N tick | ~0,3 s |
| `Die` | `energy ≤ 0` | một lần, không lặp |

Overlay **cộng vào** base, không thay thế: một con vật vừa chạy vừa đớp vẫn phải giữ nhịp chân.
Kỹ thuật: additive trên xoay khớp của **chuỗi trục và chuỗi đầu**, để nguyên chuỗi chi cho base
locomotion. Xung đột ưu tiên: `Die` > `Hurt` > `Attack` > `Feed` > `Drink`.

`update_cpg_system` hiện đặt `output = 0` khi `energy ≤ 0` (`cpg.rs`). Đó là hành vi đúng và
animation dựa vào nó: con vật hết năng lượng **tự nhiên** ngừng cử động, không cần state máy nào
ra lệnh.

---

## 4. Bảy điều làm nên "mượt"

Đây là phần kỹ thuật thật; thiếu bất kỳ mục nào là thấy được ngay bằng mắt.

1. **Không bao giờ reset `phase` khi đổi state.** Blend `frequency`/`amplitude`, giữ `phase` liên
   tục. Reset pha tạo cú giật chân rõ rệt ở mỗi lần chuyển gait — đây là lỗi phổ biến nhất.
2. **Cross-fade bằng hằng số thời gian cố định**, làm mượt mũ, τ ≈ 120–200 ms. Blend theo *thời
   gian*, không theo số tick, để không phụ thuộc tần số tick.
3. **Nội suy giữa tick và frame.** Mô phỏng chạy `TICK_HZ`, màn hình chạy tần số khác. Giữ **hai**
   payload gần nhất, nội suy vị trí tuyến tính và xoay bằng **slerp** theo `alpha` phân số. Hiện
   `LiveAgents.tsx` ghi payload vào ref rồi đọc trong `useFrame` — không có nội suy, nên chuyển
   động giật theo nhịp tick. Đây là sửa đổi có tác động thị giác lớn nhất trên mỗi dòng code.
4. **Root motion thuộc về mô phỏng, không thuộc animation.** Animation không bao giờ tịnh tiến
   gốc. Nếu nó tịnh tiến, hình vẽ và collider tách nhau và không test nào bắt được.
5. **IK chân bám đất.** Gait thủ tục thuần cho hiện tượng trượt chân. IK hai xương từ chuỗi chi
   xuống `surfaceHeight` của terrain sửa phần lớn cảm giác sai với chi phí rất nhỏ. Chỉ chạy ở
   `LodTier::Hot`.
6. **Nhiễu tất định theo cá thể.** Lệch pha và biên độ nhỏ băm từ `agent_id`, không lấy từ RNG
   toàn cục. Một đàn cùng archetype mà đồng pha tuyệt đối trông như quân duyệt binh.
7. **Phân cấp theo `LodTier`.** Dùng lại `tier_at` đã có, không phát minh hệ LOD thứ hai:

| Tier | Animation |
|---|---|
| `Hot` | gait đầy đủ + IK + overlay + thở |
| `Warm` | gait, không IK, overlay rút gọn, cập nhật giãn cách |
| `Cold` | tư thế tĩnh, không overlay |

---

## 5. Truyền qua IPC

Frontend **không** suy được state từ vị trí: nó không thấy `feed_intent` hay `energy`. Backend
phải gửi, nhưng phải rẻ.

Hiện `SegmentState` là **per-segment** và không có trường animation
(`src-tauri/src/core/simulation_state.rs:23`). Thêm vào đó sẽ nhân bản dữ liệu per-agent lên số
segment. Vì vậy thêm một mảng **song song, per-agent** trong `SimulationTickPayload`:

```rust
#[derive(ts_rs::TS, serde::Serialize, Clone, Debug)]
pub struct AgentCue {
    pub agent_id: u32,
    pub locomotion: LocomotionCue, // u8 trên dây
    pub action: Option<ActionCue>, // u8, None phần lớn thời gian
    pub gait_phase: f32,           // [0, 2π), để frontend nối pha liên tục
    pub gait_speed: f32,           // chuẩn hoá, lái blend Walk↔Run
    pub energy01: f32,             // lái thở, sụp người, màu nhợt
}
```

Chi phí: **20 byte/agent/tick**, 1.000 agent ⇒ ~20 KB/tick. Nếu đo thấy đắt, `AgentCue` chịu được
giãn cách (gửi mỗi K tick) vì frontend đã nối pha liên tục ở mục 4.1 — cue là *mục tiêu blend*,
không phải khung hình.

Bắt buộc theo `Gate P5` của [morphogenesis plan](../../planning/CREATURE_MORPHOGENESIS_PLAN.md):
frontend **không** được suy lại cue từ biome hay từ vị trí. Nó vẽ cái backend nói.

---

## 6. Ngân sách

| Đại lượng | Ngân sách |
|---|---|
| Tính cue ở backend | trong tick, không cấp phát heap (`allocs == 0`) |
| `AgentCue` trên dây | ≤ 20 byte/agent/tick, giãn cách được |
| Gait + skinning ở frontend, 1.000 agent | trong ngân sách frame hiện có, dùng instancing + LOD |
| IK | chỉ `Hot`, ≤ 2 xương/chi |

---

## 7. Rủi ro

| Rủi ro | Dấu hiệu sớm | Ứng phó |
|---|---|---|
| Animation state trôi khỏi trạng thái mô phỏng | con vật "ăn" khi ledger ghi nó đói | cue sinh ở backend, frontend không có state riêng |
| Root motion lọt vào animation | hình và collider tách nhau | gate so vị trí vẽ với `Position` |
| Trượt chân | thấy ngay bằng mắt | IK mục 4.5 |
| Giật theo nhịp tick | thấy ngay bằng mắt | nội suy mục 4.3 |
| Payload phình | frame time tăng theo số agent | `AgentCue` per-agent, giãn cách được |
| Gait sai cho hình thái lạ | con sáu chi đi như con bốn chi | pha chi suy từ topology, có fixture cho 2/4/6/0 chi |

---

## 8. Điều thiết kế này **không** tuyên bố

- Không tuyên bố `LocomotionMedium` đã tồn tại — kiểm 2026-08-25: **chưa**. Tới khi có, `Swim`/`Fly`
  suy tạm từ ô địa hình dưới chân, và đó là **giải pháp tạm vi phạm tinh thần D04**; phải gỡ khi
  P1–P2 morphogenesis xong.
- Không tuyên bố CPG hiện tại đủ cho gait nhiều chi. Nó là **một** dao động sine cho mỗi segment
  (`cpg.rs:23`); gait cần lệch pha giữa các chi, tức một CPG **có ghép nối**. Đây là công việc
  thật, nằm ở Slice 2 của kế hoạch.
- Không tuyên bố có clip authoring. Không có, và cố ý không có.
