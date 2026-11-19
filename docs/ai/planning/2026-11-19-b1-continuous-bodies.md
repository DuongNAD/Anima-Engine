---
phase: planning
feature: real-world-and-bodies
title: Thẻ B1 — Thân liên tục từ hình thái học, 3 mức LOD và frustum culling (prototype)
description: Thân SkinnedMesh nối khớp mượt từ chuỗi đốt morphology, tỷ lệ hiển thị CREATURE_BODY_SCALE = 0.32 (lựa chọn hiển thị, không cùng tỷ lệ vị trí), LOD 3 mức K = 8/12/16 kèm per-creature frustum culling, và dữ liệu synthetic environmental_state
status: completed
owner: rendering-frontend
last_reviewed: 2026-11-19
---

# Thẻ B1 — Thân sinh vật liên tục & LOD 3 tầng (`prototype, to be replaced`)

> **Trạng thái nguyên mẫu (`prototype, to be replaced`):** theo chỉ đạo `PROTOTYPE-FIRST` (`MSG_20261119_040227`), mọi module mới (`creatureSkinMesh.ts`, `creatureLod.ts`) và phần nối vào `LiveAgents.tsx` mang nhãn `'prototype, to be replaced'` để đánh giá hình dáng và FPS trước khi thay bằng phiên bản hoàn chỉnh.

## 1. Kiến trúc thân liên tục (`creatureSkinMesh.ts`)

- **Dựng lưới từ hình thái thật:** `buildMorphologySkinArchetype` đọc chuỗi đốt từ pose đã scale (`renderLength`, `renderRadius`, `jointAnchor`, `parentSegmentId`) thông qua hàm thuần dùng chung `buildMorphologyRestLayout` (`agentPose.ts`), đặt mỗi đốt ở đúng vị trí cân bằng khớp (`P_child = P_parent + anchor + (0, 0, +child.length / 2)` khớp `genotype.rs:194-199` & `dynamics.rs:80-82`), tạo lưới ống khép kín hai đầu (`SKIN_RADIAL_SIDES = 8`, `SKIN_CAP_RINGS = 2`, `SKIN_BRIDGE_RINGS = 3`) có cầu Bezier nối qua khe hở khớp (với guard chống gập ngược khi `pjDotJc < 0`) và nhánh bên cho chi (`addBranchLoft`). Khi đốt con gắn lệch ngang ở gốc (như founder sống `anchor = (1, 0, 0)` trong `simulation_loop.rs:1528-1538`), gốc vẫn giữ capsule khép kín theo trục `±Z` riêng (`addAxialSegmentCapsule`); khi chuỗi đi thẳng `+Z` (`+L/2`), `headAttachment` gắn vào đốt mũi dẫn đầu để mắt và mõm luôn áp sát mặt da trên mọi hình thái thật. Kiểm tra biên hở (`countOpenBoundaryEdges`) bằng `0` trên toàn bộ mẫu 1, 3 (founder `(1,0,0)` & `genotype.rs` `[0,0,1.0]`), 5 (chi bên) và 9 đốt.
- **Thống nhất quy tắc gắn mắt và mõm trang trí giữa tầng `near` và `mid` (`D1`):** cả tầng `near` (`SkinnedMesh`) lẫn tầng `mid` (`live-agents-segments`) đều gọi chung `resolveCreatureHeadPose` + `emitHeadFeatures` trong `LiveAgents.tsx` trên cùng `headSegmentId` và `headLocalForward` từ `buildMorphologyRestLayout`, nên mắt và mõm giữ nguyên toạ độ không gian (sai số `< 1e-4`) khi sinh vật băng qua ranh giới LOD `near`/`mid` trên mọi chuỗi thẳng `+L/2` cũng như founder `(1, 0, 0)` và `RW2-3`. Nhãn bắt buộc theo quyết định trưởng nhóm (`D1`): **eyes and snout are decorative and mark the front of the body along head_direction; the simulation has no head organ and senses at the root position**.
- **Bone & Skinning theo khoảng cách:** mỗi đốt tương ứng 1 `THREE.Bone` (cập nhật trực tiếp từ pose nội suy trong `TickInterpolationBuffer`, hoàn toàn ngoài payload mô phỏng); `skinIndex`/`skinWeight` gán tối đa 4 bone gần nhất theo hàm Gaussian chuẩn hoá tổng bằng `1.0`.
- **Cache theo `morphologyKey` & Slot cố định (`N3`, `N3'`, `N4`, `N6`):** `ArchetypeMeshCache` (LRU tối đa `MAX_ARCHETYPE_CACHE = 128`) dựng hình học 1 lần cho mỗi kiểu hình thái và bảo vệ các khoá đang gắn trên slot khỏi bị LRU giải phóng sớm; khi một slot chuyển sang ẩn (`hideSlot` trong `prepareNearFrame`/`hideFrom`), slot chủ động giải phóng tham chiếu `skeleton`, `bones`, `archetype`, `archetypeKey` và trả `mesh.geometry` về `placeholderGeom` (kèm kiểm tra `cache.peek(key) !== slot.archetype`) nên không thể gắn lại hình học đã bị LRU huỷ (`N3'`). `SkinnedCreaturePool` giữ slot ổn định theo `agentId`, dựng từ `morphologySegments` đi kèm `morphologyKey` khi đổi hình thái, cập nhật `raycastSphere` mỗi khung và chặn `raycast` khi `!visible` (`R3`).
- **Vật liệu & chuyển động trang trí (R4):** `MeshStandardMaterial` tô màu theo loài + `hash(agent_id)` (`computeCreatureColor`, không bao giờ đổi màu theo biome), giữ mũi nhọn `predator`, mắt hướng mũi đầu và bóng tiếp đất khi tắt `shadowMap`. Nhịp thở tuần hoàn tất định theo `hashAgentId01(agentId, 3)` chỉ là **hiệu ứng trang trí, không phải dữ liệu mô phỏng (`decorative, not simulated`)** vì mô phỏng Rust tự tính hô hấp và chuyển hoá độc lập.

## 2. LOD 3 tầng, Frustum Culling & Hiệu chỉnh tỷ lệ (`creatureLod.ts`, `agentPose.ts`, `syntheticTicks.ts`)

- **LOD 3 tầng (`CreatureLodSelector`) & Cheap Win #1 (frustum culling):**
  - `WorldShowcase.tsx` truyền trực tiếp `qualityTier={effectiveQuality}` xuống `<LiveAgents>` (`N1`), và `resolveCreatureQualityTier` nhận `QualitySelection` đã phân tích sẵn ngoài `useFrame` (`N2`, 0 lần gọi `new URLSearchParams` mỗi khung).
  - Mức `near` (`<= creatureFullDetailRadius`): tối đa `K` sinh vật gần nhất mỗi khung sắp xếp gần-trước (`CREATURE_NEAR_SKINNED_CAP = { low: 8, medium: 12, high: 16 }`) dựng bằng `SkinnedMesh` liên tục; đúng 8 sinh vật gần nhất (rank `0..7`) bật `castShadow = true` để tổng số lệnh vẽ toàn cảnh trên `high` (bao gồm cả shadow pass) giữ `<= 70`. (Hysteresis ranh giới để ở bước thay thế sau prototype.)
  - Mức `mid` (`<= creatureSimplifiedRadius` hoặc vượt trần `K`): sinh vật dùng pipeline instanced cũ (`live-agents-segments` + `live-agents-spheres`).
  - Mức `far` (`> creatureSimplifiedRadius`): chỉ vẽ billboard theo loài (`live-agents-markers`).
  - **Cheap Win #1 đã làm:** culling từng sinh vật ngoài frustum camera (`THREE.Frustum.intersectsSphere` với camera đã định vị, tránh loại nhầm mock camera mặc định tại gốc trong unit test). **Cheap Win #2** (texture pose GPU cho instanced skinning) giữ lại ở bước thay thế sau prototype.
- **Lựa chọn tỷ lệ hiển thị `CREATURE_BODY_SCALE = 0.32` (R4 — không cùng tỷ lệ vị trí):**
  - Đây là **lựa chọn hiển thị, không cùng tỷ lệ với khoảng cách vị trí (`rendering choice — not to scale with positions`)**: toạ độ vị trí gốc giữ nguyên ở `simScale(1200) = 6` đơn vị vẽ / đơn vị mô phỏng trong khi kích thước thân chỉ vẽ ở `1,92` (`0,32×`) để không lấn át cây trang trí `WorldVegetation` và hồ nước. Do đó, hai sinh vật đang chạm nhau trong mô phỏng (`r = 0,2`) sẽ vẽ cách nhau khoảng `4` bán kính thân, và thân vẽ nhỏ bằng `0,32×` kích thước mô phỏng so với hồ nước mô phỏng. Sàn bán kính mắt cũng được nhân tương ứng với `CREATURE_BODY_SCALE` (`N5`).
- **Cột sống `syntheticTicks.ts` trong giới hạn Rust (R1, R2):**
  - Khôi phục đúng phân bố bán kính và số lần rút `rand()` như thân cây `86e4736` (quỹ đạo gốc và chuỗi PRNG trùng khớp với thân cây), đồng thời đặt `jointAnchor.z = +parent.length * 0.48` trên cột sống và giữ chi bên `[±0.9r, -0.35r, 0]` để mọi khớp đều nằm chặt trong giới hạn `mutation.rs` (`|x|,|y| <= r`, `|z| <= parent.length / 2`) mà vẫn nối dài mượt dọc `+Z` (thay đổi offset đốt con so với thân cây cũ, không tuyên bố giữ nguyên toàn bộ vị trí đốt).
- **Bổ sung `environmental_state` synthetic (`syntheticTicks.ts`):** thêm 3 hồ (`lake`), 8 cây (`tree`) và 10 mồi (`food`) từ luồng RNG độc lập `(seed ^ 0x517e0e1e) >>> 0` quanh `LAND_ANCHORS` không ảnh hưởng quỹ đạo gốc, giữ rõ nhãn `"SYNTHETIC FIXTURE (synthetic) — not simulation output"`. Khi V1 (`LiveEnvironment`) gộp vào thân cây, các thực thể này sẽ hiển thị cùng sinh vật trong trình duyệt.

## 3. Lý do cập nhật test `rwCreatures*` & Ngân sách bundle

- `tests/frontend/rwCreaturesSegmentPose.test.tsx`: thêm camera mid-LOD `(0, 120, 240)` nhìn `(0, 60, 0)` cho `RW2-2`, `RW2-3`, `RW2-7` vì tầng `near` của B1 đưa sinh vật trong bán kính `160` về `SkinnedMesh` thay vì `live-agents-segments`; cập nhật hệ số kỳ vọng `CREATURE_BODY_SCALE` ở `RW2-2` (`0.2/0.45 * 6 * CREATURE_BODY_SCALE`) và `RW2-3` (`> 2 * CREATURE_BODY_SCALE`, `> 0.5 * CREATURE_BODY_SCALE`). Toàn bộ `RW2-1`, `RW2-4`, `RW2-5`, `RW2-6`, `RW2-8` giữ nguyên không đổi.
- `tests/frontend/rwCreaturesTickSources.test.tsx`: đặt camera mid-LOD `(-140, 250, -40)` nhìn `(-140, 0, -40)` (`fov=120`) trong `advanceFrames()` để `RW0-1`/`RW0-2`/`RW0-3` kiểm tra đúng tầng `mid` (`live-agents-segments`). `RW3-3` (cấm cấp phát trong `useFrame`) giữ nguyên 100%.
- `scripts/check_bundle_budget.mjs`: nâng trần `WorldShowcase` từ `130 -> 155 KiB` (đo thực tế sau sửa vòng 1 là `145.8 KiB`, chừa dư địa cho V1) với lý do `'prototype'`.

## 4. Đo WebGL & Các cổng đã kiểm tra (`RTX 5060 Ti`, Chromium ANGLE D3D11 — sau Fix Round 1)

- Chạy `vite preview` (`1280x800`, `?replay=synthetic&agents=200&segments=9&quality=high`):
  - Pose `spawn`: **60.4 FPS** (73 frame / 1.21s), `total.calls = 68` (đạt chuẩn `<= 70`), `total.triangles = 2,291,726`, `creatures.calls = 28`, `creatures.triangles = 46,090`, `0` console error, `0` page error.
  - Pose `overview`: `total.calls = 22`, `creatures.calls = 1`, `creatures.triangles = 398`.
  - Ảnh kiểm chứng lưu tại hub (`test_runs/anima-case_B1_screenshot.png`).

| Cổng | Kết quả | Ghi chú |
|---|---|---|
| `npm run typecheck:tests` | Đạt (`0`) | 0 lỗi TypeScript trên `src` và `tests` |
| `npm run lint` & `node scripts/eslint_ratchet.mjs` | Đạt (`0`) | 0 error, 0 warning (baseline `0`) |
| `npm run test` | Đạt (`0`) | 14/14 file, 109/109 test (chạy đầy đủ sau Fix Round 1) |
| `npm run test:frontend` | Đạt (`0`) | 69/69 file, 710/710 test (gồm 10 test R1–R4, N1–N6, M4 & raycast trong `b1ContinuousBodies.test.tsx`) |
| `npm run build` & `npm run check:bundle` | Đạt (`0`) | `WorldShowcase 145.8 / 155 KiB`, tổng `1910.9 / 2000 KiB` |
| `npm run check:ipc`, `check:csp`, `check:docs` | Đạt (`0`) | Kiểm tra sau khi ghi tài liệu này |

## 5. Không khẳng định

- Kết quả đo WebGL và test phía trên chỉ chạy trên **dữ liệu synthetic 200 sinh vật** và `RTX 5060 Ti` (`Chromium` headless GPU ANGLE D3D11), không phải số liệu mô phỏng sinh học thật hay benchmark trên GPU tích hợp Iris Xe.
- Hiệu năng trên `10/200/1000` đầy đủ theo ma trận từng mức chất lượng, hysteresis ở ranh giới K/bán kính (`N4` tuỳ chọn), và tối ưu texture GPU (cheap win #2) chưa làm trong lượt `PROTOTYPE-FIRST` này; đây là bản thử nghiệm `prototype, to be replaced`.
- Dữ liệu `environmental_state` thêm trong `syntheticTicks.ts` chỉ là bố cục giả lập phục vụ kiểm tra hiển thị trình duyệt khi chưa gắn backend Tauri, không phản ánh phân bố tài nguyên của mô phỏng Rust.
