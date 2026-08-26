---
phase: implementation
feature: living-world
title: Task Backlog — Thế giới sống
description: Danh sách đầy đủ việc cần làm, có neo file, phụ thuộc và định nghĩa hoàn thành cho từng mục
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
requirements: ../requirements/2026-08-25-feature-living-world.md
planning: ../planning/2026-08-25-feature-living-world.md
testing: ../testing/2026-08-25-feature-living-world.md
handoff: ../planning/2026-08-25-agent-goal-living-world.md
---

# Task Backlog — Thế giới sống

Đây là danh sách **đầy đủ** việc cần làm. Mỗi mục có: neo file, phụ thuộc, và **định nghĩa hoàn
thành** (DoD) đo được. Một mục không có DoD đo được là một mục chưa sẵn sàng để giao.

> Trạng thái đo được của toàn dự án nằm ở **một chỗ duy nhất**:
> [`STATE_OF_THE_PROJECT.md` §1](../../planning/STATE_OF_THE_PROJECT.md). Đừng chép số sang đây.

**Ký hiệu:** ⬜ chưa làm · 🔒 bị chặn · ⚠️ có bẫy, đọc kỹ cột ghi chú.

---

## 0. Sự thật hiện trạng (đo 2026-08-25)

Mọi task dưới đây dựa trên các sự thật này. Kiểm lại trước khi bắt đầu; nếu lệch thì kế hoạch lệch.

| # | Sự thật | Bằng chứng |
|---|---|---|
| 1 | `SegmentState` **không** có `length`/`radius`/`color` | `src-tauri/src/core/simulation_state.rs:23-43` |
| 2 | Sinh vật 3D là hình cầu bán kính **cố định 1.6** | `src/components/Landscape/LiveAgents.tsx:44,77` |
| 3 | `EnvSample`, `develop_at_birth`, `DevelopedPhenotype`, `spawn_developed`: **0 kết quả** grep | toàn crate |
| 4 | `LocomotionMedium` **chưa tồn tại** | grep, 0 kết quả trong `src/` |
| 5 | `CpgOscillator` là **một sine** cho mỗi segment, không ghép nối | `src-tauri/src/ai/cpg.rs:4-28` |
| 6 | `ActionGates { pheromone_emit, attack_intent, feed_intent }` đã có | `src-tauri/src/core/components.rs:495` |
| 7 | Não là **bộ điều khiển gait**, ăn/tấn công tự động theo khoảng cách | comment `components.rs:486` |
| 8 | `EnergyLedger` có **ba** ngăn; không có ngăn ngủ đông | `docs/reference/ENERGY_LEDGER_CONTRACT.md §1` |
| 9 | `ShardingConfig` chỉ có `local_port`, `left_target_port`, `right_target_port` | `components.rs:441` |
| 10 | Migration WebSocket đã hoạt động: `anima-migration-v1`, ack, backpressure, trần 1 MiB / 64 kết nối | `core/networking_systems.rs` |
| 11 | ⚠️ **`src-tauri/anima-core/` KHÔNG nằm trong workspace** — nó có `Cargo.lock` và `target/` riêng, không package nào import `anima_core`, và `cargo test` từ `src-tauri/` **không chạy nó** | `cargo metadata --no-deps` chỉ liệt kê `anima-engine` và `anima-domain` |

**Sự thật số 11 là bẫy nguy hiểm nhất trong repo này.** `MorphologyGenotype` tồn tại ở **hai** nơi:
`anima-core/src/genotype.rs:21` và `src-tauri/src/core/spawn.rs:21`. **App biên dịch cái thứ hai.**
Sửa cái thứ nhất sẽ compile sạch, test xanh, và **không thay đổi gì trong ứng dụng**.

---

## 1. Nhánh A — Hình thái nhìn thấy được

### A1 — Payload chở kích thước · không phụ thuộc · **làm trước tiên**

| ID | Việc | Neo file | DoD |
|---|---|---|---|
| ⬜ A1-1 | Thêm `length: f32`, `radius: f32` vào `SegmentState` | `src-tauri/src/core/simulation_state.rs:23` | struct compile; serde snapshot cập nhật |
| ⬜ A1-2 | Điền hai field từ component `Segment` khi dựng payload | `src-tauri/src/core/simulation_loop.rs` (chỗ `state_buffer.push(SegmentState { … })`) | test khẳng định giá trị payload == `Segment.length/radius` |
| ⬜ A1-3 | Sinh lại binding ts-rs | `src/types/generated/SegmentState.ts` | `cargo test` rồi `git diff --exit-code -- src/types/generated` **fail trước, pass sau khi commit binding** |
| ⬜ A1-4 | Bỏ hằng `SEGMENT_RADIUS`; scale instance theo `radius`, kéo dài theo `length` | `src/components/Landscape/LiveAgents.tsx:44,47,77` | Vitest khẳng định hai agent khác `radius` cho hai matrix khác nhau |
| ⬜ A1-5 | `PixiViewport` 2D vẽ theo cùng số đo | `src/PixiViewport.tsx` | Vitest draw-spy |
| ⬜ A1-6 | Cập nhật mô tả IPC | `PROJECT.md` §Interface Contracts | review link/schema |

**Gate A1:** `cargo test --features desktop` · `npm run test` · `npm run test:frontend` · `npm run build` · parity binding.

### A2 — `MorphoSpec` và mesh procedural · phụ thuộc A1

| ID | Việc | Neo file | DoD |
|---|---|---|---|
| ⬜ A2-1 | Crate thuần mới `crates/anima-morpho` — **không** `bevy_ecs`, **không** `three` | `src-tauri/crates/anima-morpho/` | `cargo tree` cho crate này không chứa bevy |
| ⬜ A2-2 | Phân loại `BodyPlan` từ graph: axial chain + limb chains | crate mới | fixture 0/2/4/6/8 chi ⇒ đúng nhãn |
| ⬜ A2-3 | Band hoá: size · build · limb · covering · color · pattern · head · thermo · distinctive | crate mới | mọi nhánh có ≥1 fixture (PR-S04) |
| ⬜ A2-4 | `MorphoSpec { version, fingerprint, … }` | crate mới | GM-S01 byte-exact |
| ⬜ A2-5 | Compiler mesh: capsule ring stitching, bone tại node origin, skin weight 2 bone gần nhất | crate mới | GM-S02 byte-exact; GM-S03 khớp `Segment` |
| ⬜ A2-6 | IPC `get_archetype_mesh(key)` — trả mesh **một lần cho mỗi archetype**, client cache | `src-tauri/src/lib.rs` commands + `PROJECT.md` | mesh không đi trong payload mỗi tick |
| ⬜ A2-7 | Renderer thay `SphereGeometry` bằng mesh archetype + scale per-individual | `LiveAgents.tsx` | ảnh before/after; benchmark 1.000 agent |

**Gate A2:** GM-S01…GM-S05.

⚠️ A2-6 là quyết định kiến trúc: mesh là **per-archetype**, không per-tick và không per-agent. Đưa
vertex buffer vào `SimulationTickPayload` sẽ làm payload nổ và là sai lầm không quay lại được rẻ.

### A3 — Prompt, kho asset, API · phụ thuộc A2

| ID | Việc | Neo | DoD |
|---|---|---|---|
| ⬜ A3-1 | `build_prompt` đúng [hợp đồng prompt](../../reference/CREATURE_PROMPT_CONTRACT.md) | crate `anima-morpho` | PR-S01, PR-S02, PR-S03 |
| ⬜ A3-2 | `Lexicon` là **dữ liệu versioned**, không hằng số rải rác | file dữ liệu trong crate | PR-S06 |
| ⬜ A3-3 | `ArchetypeKey = blake3(prompt‖negative‖params‖model‖seed)` | crate | PR-S05 |
| ⬜ A3-4 | Kho `artifacts/creatures/<lexicon_version>/` + `manifest.json` khoá style lock và model pin | `artifacts/` | manifest tồn tại, có hash toàn kho |
| ⬜ A3-5 | **Đo** 2–3 nhà cung cấp theo 6 tiêu chí design §6, chốt một, ghi bảng đo | ADR mới | bảng có số thật, không chép marketing |
| ⬜ A3-6 | Worker sinh asset **ngoài tick loop**, backpressure, trần `max_archetypes_per_run` | module mới | PR-S09 dưới quần thể tổng hợp 10⁵ |
| ⬜ A3-7 | Fallback procedural ở **mọi** nhánh lỗi | | PR-S08, GM-S06 |
| ⬜ A3-8 | Danh sách cấm: prompt không chứa thuộc tính đã có tham số riêng | test | PR-S07 |

**Gate A3:** PR-S01…PR-S10 + GM-S06…GM-S08.

### A4 — Cổng nghiệm thu asset · phụ thuộc A3

| ID | Việc | DoD |
|---|---|---|
| ⬜ A4-1 | A1 chuẩn hoá scale về chiều cao mét thật | luôn chạy trước mọi cổng khác |
| ⬜ A4-2 | A2 aspect ratio bbox lệch ≤ 25% | trượt ⇒ retry |
| ⬜ A4-3 | A3 đối xứng qua mặt phẳng dọc, Chamfer dưới ngưỡng | trượt ⇒ retry |
| ⬜ A4-4 | A4 trần tam giác + decimation | ≤ 20.000 sau decimate |
| ⬜ A4-5 | A5 connected component ≤ 2 | |
| ⬜ A4-6 | A6 tỉ lệ mặt suy biến dưới ngưỡng | |
| ⬜ A4-7 | A7 auto-skin vào bone graph phenotype, ≤ 4 bone/vertex | mesh AI cử động bằng CPG hiện có |
| ⬜ A4-8 | A8 UI duyệt một lần mỗi archetype + `rejected.jsonl` | |
| ⬜ A4-9 | Retry K seed (mặc định 3), hết K ⇒ procedural + ghi sổ | không nhánh nào dẫn tới "không có hình" |

---

## 2. Nhánh N — Animation

### N1 — Nội suy tick ↔ frame · không phụ thuộc · **tác động thị giác lớn nhất trên mỗi dòng code**

| ID | Việc | Neo | DoD |
|---|---|---|---|
| ⬜ N1-1 | Giữ **hai** payload gần nhất; lerp vị trí, **slerp** xoay theo `alpha` phân số theo thời gian | `LiveAgents.tsx` (`segmentsRef`, `useFrame`) | AN-S02: giống nhau ở 30/60/144 Hz |
| ⬜ N1-2 | Xử lý payload trễ/thiếu/không đúng thứ tự: kẹp `alpha`, không ngoại suy quá 1 tick | | test payload gap |
| ⬜ N1-3 | Không cấp phát trong `useFrame` | | dùng scratch object đã có sẵn ở file |

### N2 — CPG ghép nối và gait · phụ thuộc A2 (bộ xương)

| ID | Việc | Neo | DoD |
|---|---|---|---|
| ⬜ N2-1 | CPG **có ghép nối**: lệch pha giữa các chi suy từ topology | `src-tauri/src/ai/cpg.rs` | AN-S03 fixture 0/2/4/6/8 chi |
| ⬜ N2-2 | Gait đọc `InertiaComponent.cpg_parameters` | `components.rs:460` | tham số não thực sự lái gait |
| ⬜ N2-3 | Blend `Idle↔Walk↔Run` theo tốc độ, **giữ `phase` liên tục** | | AN-S01: không reset pha ở mọi chuyển |
| ⬜ N2-4 | `Sleep`/`Rest`: biên độ → 0 **cộng dao động thở** biên độ nhỏ | | con vật ngủ không đứng hình tuyệt đối |
| ⬜ N2-5 | Giữ nguyên hành vi `energy ≤ 0 ⇒ output = 0` | `cpg.rs:47-52` | AN-S04 regression |

### N3 — Action overlay và `AgentCue` · phụ thuộc N2

| ID | Việc | Neo | DoD |
|---|---|---|---|
| ⬜ N3-1 | `AgentCue { agent_id, locomotion, action, gait_phase, gait_speed, energy01 }` + ts-rs | `core/simulation_state.rs` | ≤ 20 byte/agent (AN-S08) |
| ⬜ N3-2 | Dẫn xuất cue bằng **hàm thuần** từ `ActionGates` + `HomeostaticState` + `Velocity` | backend | AN-S05 khớp trạng thái mô phỏng |
| ⬜ N3-3 | Mảng **song song per-agent** trong `SimulationTickPayload`, không nhồi vào `SegmentState` | | payload không nhân theo số segment |
| ⬜ N3-4 | Overlay additive trên chuỗi trục + đầu; chuỗi chi để cho base locomotion | frontend | AN-S06 |
| ⬜ N3-5 | Ưu tiên `Die > Hurt > Attack > Feed > Drink` | | AN-S06 |
| ⬜ N3-6 | Hỗ trợ giãn cách K tick | | vẫn mượt vì frontend nối pha |

### N4 — IK và LOD animation · phụ thuộc N3

| ID | Việc | Neo | DoD |
|---|---|---|---|
| ⬜ N4-1 | IK hai xương từ chuỗi chi xuống `surfaceHeight` | `src/components/Landscape/utils/worldSample.ts` | hết trượt chân ở `Hot` |
| ⬜ N4-2 | Gắn vào `LodTier` qua `tier_at`, **không** dựng hệ LOD thứ hai | `src-tauri/src/core/simulation_lod.rs:225` | Hot/Warm/Cold khác nhau đo được |
| ⬜ N4-3 | Nhiễu pha/biên độ **tất định theo `agent_id`**, không RNG toàn cục | | đàn không đồng pha; cùng seed cùng kết quả |
| ⬜ N4-4 | Vị trí vẽ == `Position` ± tolerance nội suy | | AN-S07 — animation không được tịnh tiến gốc |

---

## 3. Nhánh F — Tính toán phân tán

### F1 — Ngăn `dormant` · 🔒 **chặn toàn bộ nhánh F** · không rollback được sau khi có agent dormant

| ID | Việc | Neo | DoD |
|---|---|---|---|
| ⬜ F1-1 | Thêm ngăn `dormant` vào `EnergyLedger`; đưa vào baseline | ledger + `lock_baseline` | G1.1 xanh trong cấu hình mới |
| ⬜ F1-2 | `HibernateAgentCommand`: chuyển ngăn **và** despawn tại **cùng một sync point** | cùng khuôn `ReclaimAndDespawnAgentCommand` | FD-S01 residual ≤ `1e-3` |
| ⬜ F1-3 | `WakeAgentCommand`: trừ `dormant`, spawn từ phenotype đã lưu, **KHÔNG** develop lại | | FD-S02 (D01/D02) |
| ⬜ F1-4 | `dormant` vào save/restore, `#[serde(default)]` cho save cũ | `core/simulation_state.rs` | FD-S03 |
| ⬜ F1-5 | Ngủ đông **không đi qua detritus** — đây không phải cái chết | | test khẳng định detritus không đổi |

⚠️ F1-2 là chỗ chính xác mà [ledger contract §3](../../reference/ENERGY_LEDGER_CONTRACT.md) mô tả
một lỗi đã xảy ra: ghi có ngăn trước rồi despawn qua `Commands` tạo một cửa sổ mà hệ thống chạy sau
trong cùng tick ghi có **lần hai**. Làm cả hai ở một sync point, không phải hai.

### F2 — Probe, ngân sách, phân dải · phụ thuộc F1

| ID | Việc | Neo | DoD |
|---|---|---|---|
| ⬜ F2-1 | Tiểu thế giới benchmark chuẩn, versioned, giống nhau cho mọi thiết bị | module mới | cùng máy chạy hai lần lệch < 10% |
| ⬜ F2-2 | `DeviceCapability { probe_version, sustained_tps, cost_per_agent_tick, declared_threads }` | | `declared_threads` **chỉ để chẩn đoán** |
| ⬜ F2-3 | `agent_budget` clamp `[1, max]` + trung bình trượt + hysteresis | | FD-S09 (≥1), FD-S10 (±10% ⇒ không đổi) |
| ⬜ F2-4 | Ngân sách ⇒ **bề rộng dải**, mở rộng `ShardingConfig` | `components.rs:441` | giữ mô hình 1-D |
| ⬜ F2-5 | Host chạy contributor cục bộ theo sức của chính nó | | host cũng có ngân sách |

### F3 — Lockstep và tin cậy · phụ thuộc F2

| ID | Việc | DoD |
|---|---|---|
| ⬜ F3-1 | Tick barrier: host sang `T+1` khi mọi shard nộp `T` | |
| ⬜ F3-2 | Gộp theo **`shard_id`**, không theo thứ tự đến | FD-S06: hoán vị thứ tự gói ⇒ cùng checksum |
| ⬜ F3-3 | RNG stream riêng mỗi shard từ `seed + shard_id + tick` | không rút từ stream sinh thái chung |
| ⬜ F3-4 | T1 trần cấu trúc — **dùng lại** `MAX_MIGRATION_*` | `components.rs:186,190` |
| ⬜ F3-5 | T2 bảo toàn EU theo batch, tolerance `1e-3` | FD-S07 |
| ⬜ F3-6 | T3 động học: `Δpos ≤ v_max·Δt`, vị trí hữu hạn | FD-S07 |
| ⬜ F3-7 | T4 thẩm quyền: shard chỉ ghi agent được giao; không tạo agent/EU/lineage | FD-S07 |
| ⬜ F3-8 | T5 kiểm chéo ~1% batch, lệch ⇒ cách ly | FD-S08 |
| ⬜ F3-9 | Bản bóng ở host, cập nhật mỗi barrier; nguồn khôi phục cho ngắt đột ngột | FD-S04 |
| ⬜ F3-10 | `FederationPolicy::{Solo, Contribute, Inhabit}` + ghi vào manifest | ADR |
| ⬜ F3-11 | Gate `contribute_matches_solo` | **FD-S05 — gate quan trọng nhất nhánh F** |

### F4 — Vận hành · phụ thuộc F3

| ID | Việc | DoD |
|---|---|---|
| ⬜ F4-1 | Xác thực contributor bằng token | |
| ⬜ F4-2 | TLS cho transport migration | |
| ⬜ F4-3 | Rate limit theo contributor | |
| ⬜ F4-4 | Bảng đóng góp (ai góp bao nhiêu agent, thông lượng) | công cụ chẩn đoán khi một shard chậm |
| ⬜ F4-5 | Đăng ký target feature-gated mới vào policy | `scripts/test_target_policy.mjs` |

⚠️ **Không mở host ra internet công cộng trước khi F4-1 và F4-2 xong.** Transport hiện tại (64 kết
nối, 1 MiB/frame) đủ cho LAN và nhóm tin nhau, chưa đủ cho mạng mở.

---

## 4. Việc chéo (X)

| ID | Việc | Neo | Khi nào |
|---|---|---|---|
| ⬜ X1 | ADR: nguồn rò rỉ **thứ sáu** (API 3D ngoài) và cách nó bị chặn | `docs/decisions/ADR-0005-…` | **trước** khi bật A3 mặc định |
| ⬜ X2 | ADR: `FederationPolicy`, hoặc mở rộng ADR-0004 | `docs/decisions/` | **trước** F3-10 |
| ⬜ X3 | Cập nhật IPC contract | `PROJECT.md` | cùng PR với A1-1, A2-6, N3-1 |
| ⬜ X4 | Thêm mục vào backlog trạng thái | `docs/planning/STATE_OF_THE_PROJECT.md` §3 | khi bắt đầu |
| ⬜ X5 | `node scripts/gen_notice.mjs` nếu thêm dependency (blake3, gltf, …) | `NOTICE` | cùng PR thêm dep |
| ⬜ X6 | Đăng ký target rỗng / `#[ignore]` mới | `scripts/test_target_policy.mjs` | cùng PR tạo target |
| ⬜ X7 | `npm run check:bundle` nếu frontend phình | | trước khi merge frontend lớn |
| ⬜ X8 | Quyết định số phận `src-tauri/anima-core/` — hợp nhất, đưa vào workspace, hay ghi rõ là thí nghiệm | `src-tauri/anima-core/` | **sớm**, xem §0 sự thật 11 |

---

## 5. Thứ tự thực thi đề nghị

```text
Ngày 1–2   A1 (toàn bộ) ................ sinh vật có kích thước thật
           N1 (toàn bộ) ................ hết giật
Ngày 3–5   F1 (toàn bộ) ................ mở khoá nhánh F, phải đúng ngay lần đầu
Tuần 2     A2 ......................... mesh procedural, bộ xương
           N2 ......................... gait thủ tục
Tuần 3     N3 + N4 .................... overlay, IK, LOD
           F2 ......................... probe + ngân sách
Tuần 4     A3 + A4 .................... prompt + API + nghiệm thu
           F3 ......................... lockstep + tin cậy
Sau đó     F4, X1–X8
```

A1 và N1 độc lập hoàn toàn — làm song song được. F1 nên bắt đầu sớm dù không nhìn thấy được, vì
nó là mục duy nhất **không rollback được**.

---

## 6. Định nghĩa hoàn thành toàn feature

Không tuyên bố xong nếu thiếu bất kỳ mục nào:

- [ ] Ảnh before/after: sinh vật khác kích thước, khác hình theo gene và môi trường.
- [ ] Clip: sáu nhóm trạng thái, chuyển mượt, trên ≥3 hình thái khác nhau.
- [ ] Hai máy chạy một thế giới; **`kill -9`** một máy giữa chừng; residual vẫn ≤ `1e-3`.
- [ ] `contribute_matches_solo` xanh.
- [ ] Xoá toàn bộ kho asset ⇒ app chạy đủ, checksum run **không đổi**.
- [ ] Toàn bộ gate hiện có vẫn xanh, **không nới ngưỡng nào**.
- [ ] `STATE_OF_THE_PROJECT.md` cập nhật, có lệnh + ngày + cấu hình cho mỗi số.
