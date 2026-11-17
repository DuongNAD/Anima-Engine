---
phase: monitoring
title: Triage báo cáo AUDIT_REPORT.md trên origin/feat/ecological-selection
description: Đối chiếu hai bản sao AUDIT_REPORT.md trên main, xác minh 27 mục trên thân cây 9dd4214c và đề xuất 21 thẻ tiếp theo theo mẫu §6
status: completed
owner: maintainers
last_reviewed: 2026-11-14
---

# Triage `AUDIT_REPORT.md` trên thân cây `origin/feat/ecological-selection`

Tài liệu này thực hiện `CARD D1` (bổn phận **chỉ đọc và đối chiếu**, không sửa code sản phẩm hay viết test): đối chiếu hai bản sao `AUDIT_REPORT.md` trên `origin/main` ở commit `a43872c5e1db0b296d1010795d2d568d9340713b` (`a43872c5`), xác minh trực tiếp từng phát hiện trên thân cây `origin/feat/ecological-selection` ở commit `9dd4214c03c201ea19529d42fcde33e6c6f602b8` (`9dd4214c`, gộp PR `#42` / `c099e216` lên nền `491ec61e149d3661b5c1790ce9335d6e25d4cd2d`), phân loại trạng thái, và đề xuất đủ 21 thẻ tiếp theo theo đúng mẫu `§6` trong [2026-11-05-swarm-prompts.md](../planning/2026-11-05-swarm-prompts.md).

> [!IMPORTANT]
> Mọi phát hiện trong báo cáo audit ban đầu đều được coi là giả thuyết (`BLOCK 0` R9). Không có thẻ `§6` nào bên dưới được phép chạy tự động; mọi thẻ Wave 2 trở đi đều phải chờ `T2` nhập vào `origin/feat/ecological-selection` (`WORK_PLAN.md`) và được chủ dự án thêm vào kế hoạch chạy.

---

## 1. Đối chiếu hai bản sao trên `origin/main` (`a43872c5`) và thân cây (`9dd4214c`)

- **Trên thân cây `origin/feat/ecological-selection` (`9dd4214c`)**: `git ls-files | grep -i audit_report` trả về rỗng — **không bản sao nào của `AUDIT_REPORT.md` tồn tại trên thân cây**.
- **Liên hệ giữa `491ec61e` và `9dd4214c`**: `git diff --name-only 491ec61e149d3661b5c1790ce9335d6e25d4cd2d..9dd4214c03c201ea19529d42fcde33e6c6f602b8` chỉ chạm đúng 2 file tài liệu của `D0` (`PR #42`, `c099e216`): `docs/ai/README.md` và `docs/ai/monitoring/pr-branch-audit-2026-11-14.md`. Toàn bộ mã nguồn Rust, frontend, test, schema và cấu hình trên `9dd4214c` giữ nguyên byte-for-byte so với `491ec61e`.
- **Trên `origin/main` (`a43872c5`)**:
  - `origin/main:AUDIT_REPORT.md` có **372 dòng**.
  - `origin/main:docs/AUDIT_REPORT.md` có **351 dòng**.
  - Lệnh `git diff a43872c5:docs/AUDIT_REPORT.md a43872c5:AUDIT_REPORT.md` xác nhận **dòng 1–351 giống nhau từng byte** (chứa toàn bộ 26 phát hiện được đánh mã: `INFRA-01..07`, `SIM-01..08`, `REND-01..07`, `RUST-01..02`, `DATA-01..02`, cùng các mục `Roadmap` `Fix 1.1..1.6`, `Fix 2.1..2.7`, `Fix 3.1..3.5`).
  - Khác biệt duy nhất: `origin/main:AUDIT_REPORT.md:352-372` nối thêm mục `## Audit Track Inventory` (bảng tóm tắt 6 dòng theo track; **không phát sinh mã phát hiện mới**).
  - Trong `Roadmap` của cả hai bản sao có thêm 1 mục không gắn mã `INFRA`/`SIM`/`REND`/`RUST`/`DATA` là `Fix 3.2` (`AUDIT_REPORT.md:340`, `whittaker_biome`), nâng tổng số mục được triage bên dưới lên **27 mục** (`2 FIXED`, `4 FALSE`, `20 OPEN`, `1 NEEDS-TEST`).

---

## 2. Bảng tổng hợp phân loại (27 mục)

| Mã | Khu vực / tệp chính trên thân cây | Trạng thái (`9dd4214c`) | Tóm tắt bằng chứng xác minh & lưu ý trùng lặp |
| --- | --- | --- | --- |
| `INFRA-01` | `src-tauri/tests/common/allocator.rs:21-55` | `FIXED (770b5f3)` | Conflict markers đã được gỡ; `git grep` trên `src-tauri/tests` ra 0 dòng khớp |
| `INFRA-02` | `scripts/check_test_targets.mjs:78-266` | `FIXED (bef7768)` | Conflict markers đã được gỡ; script phân tích cú pháp sạch |
| `INFRA-03` | `package.json:71`, `scripts/check_sbom_schema.mjs:38` | `FALSE` | `ajv-formats` `3.0.1` đã khai báo; `npm ci` + `npm run check:sbom-schema` exit `0` |
| `INFRA-04` | `NOTICE:1-62`, `tests/frontend/noticeInventory.test.ts` | `OPEN` | Đã có `HT-A7` / `B3b` (`T1`) Task 4; chờ `src-tauri/Cargo.lock` ổn định + chạy trên máy Windows có Cargo |
| `INFRA-05` | `tsconfig.json:27`, `tests/mocks/react-three-fiber-mock.ts:43-72` | `OPEN` | `9f86e73` đã bỏ `any`, nhưng `tsconfig.json` vẫn trỏ `@react-three/fiber` vào mock |
| `INFRA-06` | `src-tauri/tests/challenger_meta_ai_tests.rs:108-129` | `OPEN` | Vẫn ngủ `16ms * 50` + check `< 20ms` / `< 1200ms`; sửa phải giữ bất biến hiệu năng (`R2`) |
| `INFRA-07` | `src/components/Landscape/utils/worldGen.worker.ts`, `WorldTerrain.tsx` | `OPEN` | Đường dẫn worker trong audit bị sai; audio/`WorldWater.tsx` đã có test một phần, worker/`WorldTerrain.tsx` chưa có test |
| `SIM-01` | `src-tauri/src/physics/spatial.rs:135-343` | `OPEN` | **Rủi ro `BLOCK 0 R9`**: sửa ngây thơ `Some(0.0)` sẽ khiến tác tử tự raycast trúng collider gốc của chính mình và bị mù; trùng PR `#35` (`3560faa6`, chưa gộp) |
| `SIM-02` | `src-tauri/src/physics/dynamics.rs:48-55` | `OPEN` | `axis_val.normalize()` chưa an toàn trước trục 0; `decode_genotype`/`crossover_genotypes` copy `joint_axis` không kiểm tra; trùng PR `#37` (`df5f3336`, chưa gộp) |
| `SIM-03` | `src-tauri/src/physics/dynamics.rs:146-157` | `OPEN` | **Đổi ngữ nghĩa (`R8`)**: predator gốc có `energy <= 0.0` không bị gating chết/đông cứng như con non/CPG; gắn với `QĐ-A'` / `MortalityPolicy`; trùng PR `#37` (`41d30dec`, chưa gộp) |
| `SIM-04` | `src-tauri/src/core/simulation_loop.rs:2265-2295` | `OPEN` | `actual_fps` tính trước `thread::sleep`; đổi bước thời gian/accumulator chạm bất biến xác định (`R8`); phần FPS trùng PR `#37` (`c7829689`, chưa gộp) |
| `SIM-05` | `src-tauri/src/ai/cpg.rs:21-27` | `OPEN` | Dùng một phép trừ `2π` thay `rem_euclid`; sửa phải kiểm 180-tick checksum (`R8`); trùng PR `#37` (`0ced745f`, chưa gộp) |
| `SIM-06` | `src-tauri/src/evolution/genotype.rs:30-32`, `src-tauri/src/core/spawn.rs:30-32` | `OPEN` | `total_mass` (trùng lặp ở 2 file) cộng cả node không đi tới được từ `root_node` (`decode_genotype` dùng BFS); kiểm `R8`; trùng một phần PR `#37` (`d3965557`, chưa gộp) |
| `SIM-07` | `src-tauri/src/ai/pheromone.rs:68-71, 102-125` | `OPEN` | **Đổi ngữ nghĩa (`R8`)**: `pos_to_index` dùng tâm ô `(i + 0.5)` còn `sample_bilinear` nội suy tại lưới nguyên `(i, j)` |
| `SIM-08` | `src/components/Landscape/LiveAgents.tsx:77, 136-146` | `OPEN` | **Một phần `FALSE`**: `ry` cố ý dùng `surfaceHeight + GROUND_OFFSET`; nhưng vẫn bỏ qua `pitch`, `roll`, `length`, `radius` của `SegmentState` |
| `REND-01` | `src/components/Landscape/Terrain.tsx:147-184` | `OPEN` | Chỉ nằm trong `LandscapeShowcase.tsx` cũ (không dùng bởi M0 `WorldShowcase.tsx`); `animateLOD` gọi `computeVertexNormals()` trong `useFrame`; trùng PR `#36` (`a5b7960d`, chưa gộp) |
| `REND-02` | `Vegetation.tsx:355-474`, `WorldVegetation.tsx:166`, `Water.tsx:684-750`, `Sky.tsx` | `OPEN` | `Vegetation`/`WorldVegetation` chưa `.dispose()` geometry/material cấp phát thủ công; `Water.tsx` đã dispose geometry/texture (`:684-692`), chỉ còn `lakeUniforms` tạo lại mỗi render; `Sky` dùng JSX khai báo; trùng PR `#36` |
| `REND-03` | `playground/RabbitVisualizer.tsx:44, 240-360` | `OPEN` | Tệp scratch ngoài sản phẩm (`QĐ-D` / `26c8a96`): vừa chạy `rAF` vòng lặp async vừa chạy `useFrame` cấp `Float32Array(130)` mỗi frame |
| `REND-04` | `WorldVegetation.tsx:146-153`, `Vegetation.tsx:639-784` | `OPEN` | Thiếu `frustumCulled={false}` (các mesh instanced khác đã có) và không mở rộng `boundingSphere`; trùng PR `#36` |
| `REND-05` | `WorldMinimap.tsx:132`, `Minimap.tsx:100`, `WorldCameraRig.tsx:358-371`, `WorldWildlife.tsx:274,295` | `OPEN` | Còn nguyên 4 điểm cấp phát trong loop mỗi frame (`createImageData`, `new THREE.Euler/Vector3/Object3D/Color`); trùng PR `#36` |
| `REND-06` | `src/components/Landscape/utils/audioManager.ts:168-234`, `PositionalAudio.tsx:18-25` | `OPEN` | `connectSynthesizer` khởi tạo node âm thanh không theo dõi/dừng khi unmount; trùng PR `#36` |
| `REND-07` | `src/App.tsx:396-425, 446-492, 841-873` | `NEEDS-TEST` | `hierarchies`/`rootSegments` đã throttle 200 ms (`:481-489`), còn `environmentalState`/`headDirections` vẫn `setState` mỗi tick; mức độ drop UI thực tế chỉ kết luận được qua test profiler/render-count |
| `RUST-01` | `src-tauri/src/commands/simulation.rs:593-612` | `FALSE` | `get_test_rabbit_state` chỉ được gọi từ scratch `playground/RabbitVisualizer.tsx:259` (`0` caller sản phẩm, không thuộc tick loop per `R10`) và `tauri::ipc::Response::new` đòi buffer sở hữu `Vec<u8>` |
| `RUST-02` | `src-tauri/src/networking/sharding.rs` (không tồn tại) | `FALSE` | `sharding.rs` không tồn tại trên cả `feat/ecological-selection` lẫn `main`; code mạng thật ở `src-tauri/src/core/networking_systems.rs:33,361,377,388` dùng `500ms` làm async timeout |
| `DATA-01` | `tests/e2e/tauri-mock.ts:92` vs `src/types/generated/LineageGraphPayload.ts:8-12` | `OPEN` | Mock dùng `{ nodes: [], edges: [] }`, thiếu `links`, `db_connected` và `satisfies LineageGraphPayload`; trùng PR `#36` |
| `DATA-02` | `map_manifest.schema.json:7,17,56,85,108,138` vs `map_manifest.json` | `OPEN` | Sub-object dùng `"additionalProperties": true`; **bắt buộc** khai báo các field thật đang có trong `map_manifest.json` (`_generated`, `bytes`, `unitsPerCell`, `worldScale`, `checksum`) trước khi siết `false` |
| Roadmap `Fix 3.2` | `src-tauri/src/core/terrain.rs:197-214`, `BIOME_TAXONOMY.md:10-35` | `FALSE` | Hợp đồng hai taxonomy có chủ đích (11 `BiomeType` backend <-> 22 `Biome` frontend), đã được kiểm chứng qua các test round-trip |

---

## 3. Phân tích chi tiết từng phát hiện trên thân cây `9dd4214c`

### Track 1: Infrastructure, Tooling & Test Hygiene (`INFRA-01..07`)

#### `INFRA-01` — `FIXED (770b5f3)`
- **Báo cáo gốc**: Nghi có conflict markers trong `src-tauri/tests/common/allocator.rs`.
- **Thực tế tại `9dd4214c`**: Commit `770b5f3` (`chore(tests): gom don dep sau merge 157f85d`) đã dọn sạch tệp. Đọc `src-tauri/tests/common/allocator.rs:21-55` chỉ còn comment giải thích hợp lệ (`There are two honest fixes...`). Quét `git grep -nE "^<<<<<<< |^=======$|^>>>>>>> " -- src/ src-tauri/src src-tauri/tests tests/ scripts/` cho **0 dòng khớp**.

#### `INFRA-02` — `FIXED (bef7768)`
- **Báo cáo gốc**: Nghi có conflict markers trong `scripts/check_test_targets.mjs:78-273`.
- **Thực tế tại `9dd4214c`**: Commit `bef7768` (`fix(ci): remove the conflict markers from the test-target gate (F-017)`) đã xóa conflict markers. `scripts/check_test_targets.mjs:78-266` sạch và không có dấu `<<<<<<<`/`=======`/`>>>>>>>`.

#### `INFRA-03` — `FALSE`
- **Báo cáo gốc**: Nghi thiếu `ajv-formats` khiến `npm run check:sbom-schema` lỗi `Cannot find package 'ajv-formats'`.
- **Thực tế tại `9dd4214c`**: `package.json:71` đã khai báo `"ajv-formats": "3.0.1"`. Triệu chứng báo cáo chỉ xảy ra trên cây làm việc chưa chạy `npm ci`. Trên worktree đã cài đặt đủ `node_modules` bằng `npm ci`, chạy `npm run check:sbom-schema` thoát mã `0` với kết quả quan sát được:
  `SBOM schema check: valid CycloneDX 1.5 — 458 components, 459 dependency records, 458 unique purls, 1 documented licence gaps, schemas pinned at c320fc0f0b46`.

#### `INFRA-04` — `OPEN` (đã theo dõi bởi `HT-A7` / `B3b` Task 4)
- **Thực tế tại `9dd4214c`**: `NOTICE:1-62` (từ commit `043930c`) hiện mới liệt kê thủ công các dependency trực tiếp (`:47`), thiếu các package chuyển tiếp (`scheduler`, `earcut`, `eventemitter3`, `zustand`) và các đề mục sinh tự động (`## JavaScript components — distributed`, `licensing/THIRD_PARTY_LICENSES.txt`) mà `tests/frontend/noticeInventory.test.ts:51-53, 70-74, 115-119` khẳng định.
- **Lưu ý quy trình**: Commit `a791248` đã sửa phần header trong `scripts/gen_notice.mjs:99-105`. Theo `BLOCK 0` R12 và bảng CPU, việc tái sinh `NOTICE` phải chạy trên máy Windows có Cargo sau khi `src-tauri/Cargo.lock` ổn định (sau `T1`/`B3b` và `T2`). Lưu ý: `npm run check:notice` **không được chạy** trên `anima-dell` vì `scripts/gen_notice.mjs:34,40` gọi `cargoInventory()`.

#### `INFRA-05` — `OPEN` (báo cáo thổi phồng phần `any` sau `9f86e73`)
- **Thực tế tại `9dd4214c`**: `tsconfig.json:27` vẫn ánh xạ `"@react-three/fiber": ["tests/mocks/react-three-fiber-mock.ts"]` (trong khi `vite.config.ts:199-203` chỉ bật alias này khi `mode === 'test'`), nên `tsc --noEmit` kiểm kiểu `Canvas`, `useFrame`, `useThree`, `extend` theo mock thay vì package `@react-three/fiber` thật.
- **Phần đã cải thiện**: Commit `9f86e73` (`refactor(types): type the 3D scene for real`) đã định nghĩa chặt các phần tử JSX 3D trong `src/r3f-intrinsics.d.ts` và thay `any` bằng `unknown` cho chữ ký chỉ mục trong `RootState`/`MockRenderer` (`tests/mocks/react-three-fiber-mock.ts:43-72`). Khoảng mù còn lại là chữ ký API export từ `@react-three/fiber` và `[key: string]: unknown` trên `RootState`.

#### `INFRA-06` — `OPEN`
- **Thực tế tại `9dd4214c`**: `src-tauri/tests/challenger_meta_ai_tests.rs:108-129` (audit ghi `:111-129`): vòng lặp `:111-122` gọi `std::thread::sleep(Duration::from_millis(16))` (`:121`, nền tối thiểu 800 ms) 50 lần và kiểm `tick_duration < Duration::from_millis(20)` (`:117`) cùng `total_elapsed < Duration::from_millis(1200)` (`:126`). Trên máy tải cao hoặc CI có scheduler jitter, OS timer overrun có thể làm bài test gãy giả.
- **Ràng buộc (`BLOCK 0` R2)**: Tuyệt đối không nới ngưỡng tường thời gian một cách mù quáng để che giấu suy giảm hiệu năng; phải tách thời gian đo `tick` tính toán khỏi `std::thread::sleep` hoặc đo ngân sách tính toán thuần.

#### `INFRA-07` — `OPEN` (một phần `FALSE` về mức độ bao phủ hiện có; sai đường dẫn worker)
- **Kiểm tra đường dẫn**: File worker thực tế nằm ở `src/components/Landscape/utils/worldGen.worker.ts` (được nạp từ `src/components/Landscape/utils/worldCache.ts:129`), không phải `utils/worldGen.worker.ts` ở gốc.
- **Kiểm tra phạm vi test thực tế**:
  - `src/components/Landscape/utils/worldGen.worker.ts` và `WorldTerrain.tsx`: hiện có **0** test tham chiếu trong `tests/` hoặc `src/__tests__/`. (`OPEN`).
  - Âm thanh & mặt nước (audit đánh đồng không có test): **không chính xác hoàn toàn** — `tests/frontend/landscape_showcase.test.tsx:319-785` và `tests/frontend/landscape_showcase_adversarial.test.tsx:364-441` đã kiểm `audioManager`/`PositionalAudio`; `tests/frontend/oceanShelf.test.ts:26` và `tests/frontend/sceneClock.test.ts:147` có kiểm tĩnh `WorldWater.tsx`. Tuy nhiên biên dịch shader GLSL WebGL thực sự vẫn không chạy được dưới jsdom (`tests/mocks/react-three-fiber-mock.ts:108-112`).

---

### Track 2: Core Simulation & Physics (`SIM-01..08`)

> [!NOTE]
> Các mục `SIM-01` (PR `#35` `3560faa6`) và `SIM-02..06` (PR `#37` `c7829689`: gồm `d3965557`, `41d30dec`, `df5f3336`, `0ced745f`, `c7829689`) đã có nhánh PR mở trên `origin/main` nhưng **chưa gộp** vào `origin/feat/ecological-selection` (`9dd4214c`). Nếu chủ dự án gộp/rebase PR `#35` hoặc `#37` vào thân cây trước khi mở Wave 2, các thẻ tương ứng phải kiểm lại trước khi thực thi.

#### `SIM-01` — `OPEN` (**CẢNH BÁO `BLOCK 0` R9 NGHIÊM TRỌNG TRONG ĐỀ XUẤT SỬA CỦA AUDIT**)
- **Thực tế tại `9dd4214c`**: Trong `src-tauri/src/physics/spatial.rs:321-343`, `intersect_sphere` (`:338-342`) chỉ kiểm `if t1 >= 0.0 { Some(t1) } else { None }`. Khi gốc tia nằm trong hình cầu (`c < 0`, `t1 < 0 <= t2`), hàm trả `None`.
- **Vì sao giải pháp audit đề xuất sẽ làm hỏng toàn bộ thị giác tác tử (`BLOCK 0` R9)**:
  - `SpatialHashGrid::raycast` (`src-tauri/src/physics/spatial.rs:135-308`) **không có tham số loại trừ thực thể tự thân** và chỉ trả về **một** `closest_hit`.
  - Hai caller chính `src-tauri/src/core/agent_systems.rs:966-978` và `src-tauri/src/ai/model.rs:934-947` phóng tia từ `origin: agent_pos.0` — nằm **ngay tâm** `SpatialCollider` gốc của chính tác tử phóng tia — rồi mới lọc `if root_agent_id != entity` **sau khi** `raycast` trả kết quả.
  - Nếu áp dụng mù quáng câu sửa `if t2 >= 0.0 { Some(0.0) }` từ audit mà không loại trừ tự thân/nhóm đoạn thân trong `raycast`, tia sẽ luôn chạm collider của chính tác tử tại `t = 0.0`, thắng `closest_hit`, bị caller loại bỏ ở hậu kiểm, và **mọi tác tử bị mù hoàn toàn**.
- **Trùng lặp**: Ghi nhận có PR `#35` (`fix/ht0-5a-sensor-raycast`, commit `3560faa6`, chưa gộp vào `9dd4214c`).

#### `SIM-02` — `OPEN` (trùng PR `#37` `df5f3336`, chưa gộp)
- **Thực tế tại `9dd4214c`**: `resolve_joints_system` (`src-tauri/src/physics/dynamics.rs:48-55`) gọi thẳng `Quat::from_axis_angle(axis_val.normalize(), cpg.output)` (`:50`). Dù `src-tauri/src/evolution/mutation.rs:139,177` dùng `.try_normalize().unwrap_or(Vec3::Y)` và `src-tauri/src/core/components.rs:282-290` kiểm cạnh khi migrate, cả `decode_genotype` (`src-tauri/src/evolution/genotype.rs:195`, `src-tauri/src/core/spawn.rs:195`) lẫn `crossover_genotypes` (`src-tauri/src/evolution/crossover.rs:88,106,145`) vẫn chép trực tiếp `edge.joint_axis` không chuẩn hóa/chặn vector 0. Vector `0.0` (hoặc gần 0) gây `NaN` truyền lan toàn cây khớp.
- **Trùng lặp**: PR `#37` commit `df5f3336` sửa `.try_normalize().unwrap_or(Vec3::Y)` trong `dynamics.rs` (cần kiểm thêm biên `decode_genotype` và `crossover_genotypes` nếu rebase PR `#37`).

#### `SIM-03` — `OPEN` (**Đổi ngữ nghĩa mô phỏng — `BLOCK 0` R8**; trùng PR `#37` `41d30dec`, chưa gộp)
- **Thực tế tại `9dd4214c`**: Trong `src-tauri/src/physics/dynamics.rs:146-157`, đoạn thân con (`:146-151`) và `update_cpg_system` (`src-tauri/src/ai/cpg.rs:49-53`) đều kiểm `homeo.energy <= 0.0` độc lập với loài, trong khi phân nhánh root segment (`dynamics.rs:152-155`) tính `let is_prey = prey_query.get(entity).is_ok(); is_prey && homeo.energy <= 0.0`, khiến root segment của predator không dừng khi cạn năng lượng.
- **Ràng buộc & trùng lặp**: Đổi logic này làm thay đổi động lực sinh thái và checksum mô phỏng (`BLOCK 0` R8); cần đối chiếu với thiết kế `MortalityPolicy` / `QĐ-A'` và ghi số checksum 180-tick trước/sau. PR `#37` commit `41d30dec` có bản sửa trên nhánh `main`-based nhưng chưa lên `9dd4214c`.

#### `SIM-04` — `OPEN` (tách telemetry khỏi accumulator theo `BLOCK 0` R8; phần FPS trùng PR `#37` `c7829689`, chưa gộp)
- **Thực tế tại `9dd4214c`**: `src-tauri/src/core/simulation_loop.rs:2265-2295` (audit ghi `:2243-2274`) đo `let actual_fps = 1.0 / elapsed.as_secs_f64();` tại `:2270`, **trước** `thread::sleep(target_frame_duration - elapsed);` ở `:2293-2295`. Kết quả là telemetry báo `1 / thời_gian_tính` (hàng nghìn FPS giả tạo) thay vì nhịp vòng lặp thật, và không có accumulator bù tick bị trễ khi frame vượt ngân sách.
- **Lưu ý thiết kế & trùng lặp**: Sửa phép đo `actual_fps` (sau sleep / `compute_loop_fps` như PR `#37` commit `c7829689`) độc lập với bất kỳ thay đổi accumulator bước tick nào; accumulator chạm thẳng vào determinism và cần `BLOCK 0` R8.

#### `SIM-05` — `OPEN` (trùng PR `#37` `0ced745f`, chưa gộp)
- **Thực tế tại `9dd4214c`**: `src-tauri/src/ai/cpg.rs:21-27` bọc pha bằng một lần trừ duy nhất `if self.phase > 2.0 * std::f32::consts::PI { self.phase -= 2.0 * std::f32::consts::PI; }` (`:22-25`). Nếu `dt` hoặc `freq` làm `phase >= 4π` (hoặc pha âm), pha thoát khỏi `[0, 2π)`. Chuyển sang `rem_euclid(2.0 * std::f32::consts::PI)` (như PR `#37` commit `0ced745f`, chưa gộp) cần chạy kiểm checksum 180-tick (`BLOCK 0` R8) vì sai số dấu chấm động biên giới.

#### `SIM-06` — `OPEN` (trùng một phần PR `#37` `d3965557`, chưa gộp)
- **Thực tế tại `9dd4214c`**: `MorphologyGenotype::total_mass` bị lặp nguyên văn ở **hai nơi**: `src-tauri/src/evolution/genotype.rs:30-32` và `src-tauri/src/core/spawn.rs:30-32`. Cả hai đều cộng toàn bộ `self.nodes.iter().map(|n| n.mass).sum()`, trong khi `decode_genotype` (`:80-213` ở cả hai tệp) chỉ sinh các node đi tới được theo BFS từ `root_node`. `src-tauri/src/evolution/mutation.rs:28-67` (`is_valid_genotype`) và `src-tauri/src/core/components.rs:269` chỉ chặn phần nào qua đường mutation/crossover/migration, không chặn deserialize/khởi tạo trực tiếp.
- **Trùng lặp**: PR `#37` commit `d3965557` sửa `genotype.rs` (cần kiểm đồng bộ với bản lặp trong `core/spawn.rs`).

#### `SIM-07` — `OPEN` (**Đổi ngữ nghĩa mô phỏng — `BLOCK 0` R8**)
- **Thực tế tại `9dd4214c`**: Trong `src-tauri/src/ai/pheromone.rs`: `pos_to_index` (`:68-69`) ánh xạ tọa độ bằng `(norm * GRID_SIZE as f32).floor()` (tâm ô nằm ở `i + 0.5`), nhưng `sample_bilinear` (`:102-113`) tính `gx = (norm_x * GRID_SIZE as f32).rem_euclid(GRID_SIZE as f32)`, `gx0 = (gx.floor() as usize) % GRID_SIZE`, `tx = (gx - gx.floor()).clamp(0.0, 1.0)` — tức coi tâm ô nằm ở nút nguyên `(i, j)` và nội suy lệch nửa ô + quấn biên không đồng nhất với ghi nhận pheromone. Đổi công thức làm thay đổi trường hóa học tác tử đọc được và đổi 180-tick checksum (`BLOCK 0` R8).

#### `SIM-08` — `OPEN` (một phần `FALSE` về việc "clamp Y sai")
- **Phần `FALSE`**: `src/components/Landscape/LiveAgents.tsx:138-142` cố ý đặt `ry = surfaceHeight(...) + GROUND_OFFSET` kèm chú thích hợp đồng rõ ràng — trục `y` trong `[0, 10]` của vật lý backend không tương ứng độ cao địa hình 3D.
- **Phần `OPEN`**: `src-tauri/src/core/simulation_loop.rs:2007, 2045-2052` đã tính và gửi đủ `pitch`, `roll`, `length`, `radius` trong `SegmentState`, nhưng `LiveAgents.tsx:77, 145` vẫn dựng cùng một `SphereGeometry(SEGMENT_RADIUS, 10, 8)` và chỉ gán `dummy.rotation.set(0, seg.yaw, 0)`, bỏ phí `pitch`, `roll`, `length` và `radius`.

---

### Track 3: Frontend 3D Rendering & WebAudio (`REND-01..07`)

> [!NOTE]
> Các mục `REND-01`, `REND-02`, `REND-04`, `REND-05`, `REND-06` và `DATA-01` đều có phần chồng lấn với PR `#36` (`fix/ht0-5b-landscape-dispose`, commit `a5b7960d`, **chưa gộp** vào `origin/feat/ecological-selection` tại `9dd4214c`). Nếu PR `#36` được gộp trước khi Wave 2 mở thẻ tương ứng, chủ dự án cần đánh dấu thẻ đó là trùng/đã xử lý và chỉ làm phần dư (nếu có).

#### `REND-01` — `OPEN`
- **Thực tế tại `9dd4214c`**: `src/components/Landscape/Terrain.tsx:147-184` vẫn chạy `animateLOD` bên trong `useFrame` (cho step 1, 4, 16) và gọi `geom.computeVertexNormals()` (`:177`) mỗi khung hình khi `cell.elevation < 3.0 && !cell.isRiver` (mặc định `width=500, height=500` ở `:23` -> tới 250.000 đỉnh ở LOD 0).
- **Phạm vi**: `Terrain.tsx` được dùng bởi `src/components/Landscape/LandscapeShowcase.tsx:208` cũ, **không** nằm trong scene M0 hiện hành `src/components/Landscape/WorldShowcase.tsx:461-480` (mount `<WorldTerrainLod>` ở `:462` hoặc `<WorldTerrain>` ở `:474`, định nghĩa tại `src/components/Landscape/WorldTerrain.tsx:1-158`).

#### `REND-02` — `OPEN` (kết quả khác nhau theo từng component)
- **Thực tế tại `9dd4214c`**:
  - `src/components/Landscape/Vegetation.tsx:355-474`: cấp phát 17 geometry và 17 material trong `useMemo` rồi mount qua `<primitive>` (`:644-782`) với **0** lệnh `.dispose()`. (`OPEN`).
  - `src/components/Landscape/WorldVegetation.tsx:166`: cấp phát 14 geometry qua `makeFloraGeometry` với **0** lệnh `.dispose()` (`:152` material dùng JSX khai báo). (`OPEN`).
  - `src/components/Landscape/Water.tsx:684-692, 724-750`: **đã có** `useEffect` gọi `.dispose()` cho `heightMapTexture`, `oceanGeometry` và `riverData.geom` (`:684-692`), đồng thời `<circleGeometry>` / `<shaderMaterial>` của hồ (`:738-746`) dùng JSX khai báo để R3F tự dọn; điểm còn tồn tại ở `:726` là tạo mới `lakeUniforms` mỗi lần render trong `.map()`. (Phần rò rỉ geometry/texture chính trong `Water.tsx` đã đóng; còn việc memoize `lakeUniforms`).
  - `src/components/Landscape/Sky.tsx:115-205`: dùng JSX R3F khai báo cho geometry/material nên R3F tự dọn khi unmount. (Đóng phần leak ở `Sky.tsx`).

#### `REND-03` — `OPEN` (tệp scratch không thuộc sản phẩm — `QĐ-D` / `26c8a96`)
- **Thực tế tại `9dd4214c`**: `playground/RabbitVisualizer.tsx:240-322` khởi chạy vòng lặp `requestAnimationFrame(fetchAndRenderRabbit)` song song với `useFrame` (`:340-360`), trong đó nhánh mock gọi `generateSingleRabbitJS` (`:44`) cấp phát `new Float32Array(TOTAL_PARTS * 13)` (130 float) mỗi khung hình. File thuộc scratch thử nghiệm theo `QĐ-D`; khi mở thẻ cần xác nhận với chủ dự án là sửa gọn vòng lặp/bỏ cấp phát hay giữ trạng thái playground.

#### `REND-04` — `OPEN`
- **Thực tế tại `9dd4214c`**: Kiểm bằng `git grep frustumCulled` cho thấy `LiveAgents.tsx:176`, `WorldBirds.tsx:76`, `WorldCaves.tsx:42`, `WorldFish.tsx:172`, `WorldWater.tsx:383` và `WorldWildlife.tsx:356` đều đã đặt `frustumCulled={false}`. Riêng `src/components/Landscape/WorldVegetation.tsx:146-153` (audit trích `:191-210`) và `src/components/Landscape/Vegetation.tsx:639-784` (audit trích `:480-520`) vẫn thiếu cả `frustumCulled={false}` lẫn cập nhật `geometry.boundingSphere`, khiến cây cỏ instanced biến mất khi bounding sphere gốc của 1 tán cây ra khỏi frustum camera.

#### `REND-05` — `OPEN`
- **Thực tế tại `9dd4214c`**: Tất cả 4 điểm cấp phát trong vòng lặp per-frame được audit nêu đều còn nguyên trên thân cây:
  1. `src/components/Landscape/WorldMinimap.tsx:132`: `ctx.createImageData(size, size)`.
  2. `src/components/Landscape/Minimap.tsx:100`: `ctx.createImageData(180, 180)`.
  3. `src/components/Landscape/WorldCameraRig.tsx:358,363,369,371`: `new THREE.Euler` và `new THREE.Vector3` trong `useFrame`.
  4. `src/components/Landscape/WorldWildlife.tsx:274,295`: `new THREE.Object3D()` và `new THREE.Color()` trong `useFrame`.

#### `REND-06` — `OPEN`
- **Thực tế tại `9dd4214c`**: Trong `src/components/Landscape/utils/audioManager.ts:168-234`, `connectSynthesizer` (`:186-233`) gọi `.start()` trên `AudioBufferSourceNode` / `OscillatorNode` (`:206,217,228`) mà không lưu tham chiếu để `.stop()` / `.disconnect()`, và class `AudioManager` không có phương thức `dispose`. Ở phía React, `src/components/Landscape/PositionalAudio.tsx:18-20` không có `useEffect` cleanup khi unmount.

#### `REND-07` — `NEEDS-TEST`
- **Thực tế tại `9dd4214c`**: `src/App.tsx:481-489` đã throttle `setHierarchies` và `setRootSegments` ở ngưỡng 200 ms. Tuy nhiên `:456-475` vẫn gọi `setEnvironmentalState` và `setHeadDirections` **không throttle** trong handler `useTauriEvent("simulation-tick", ...)`, khiến component gốc `<App>` có thể re-render mỗi tick cùng `renderGrid()` chưa memoize (`:396-425`) và khối IIFE SVG (`:841-873`). Hiện tượng `setState` mỗi tick tồn tại rõ ràng trong mã, nhưng mức độ suy giảm FPS thực tế trên UI khi chỉ còn hai state này cần chạy test render-count / profiler trước và sau mới kết luận được (`NEEDS-TEST`).

---

### Track 4: Rust Commands & Networking (`RUST-01..02`)

#### `RUST-01` — `FALSE`
- **Thực tế tại `9dd4214c`**: Lệnh `get_test_rabbit_state` (`src-tauri/src/commands/simulation.rs:593-612`) qua `git grep get_test_rabbit_state` chỉ có **1** caller là tệp scratch `playground/RabbitVisualizer.tsx:259` (0 caller sản phẩm, không thuộc tick loop theo `BLOCK 0` R10). Hơn nữa, chữ ký lệnh Tauri trả `tauri::ipc::Response::new(buffer)` đòi hỏi sở hữu bộ đệm trả về (`Vec<u8>`), nên không thể né cấp phát chỉ bằng `thread_local!` mà không thay đổi kiến trúc/hợp đồng IPC.

#### `RUST-02` — `FALSE`
- **Thực tế tại `9dd4214c`**: Tệp `src-tauri/src/networking/sharding.rs` **không tồn tại** trên `origin/feat/ecological-selection`, trên `origin/main` (`a43872c5`), hay bất kỳ `sharding.rs` nào trong cây thư mục hiện hành. Code di trú mạng WebSocket thực sự nằm tại `src-tauri/src/core/networking_systems.rs:88-504`, sử dụng `Duration::from_millis(500)` làm **async timeout** khi gửi/nhận (`:33, :361, :377, :388`), không phải vòng ngủ polling reconnect 500 ms (`:466-494` xử lý bounce các gói gửi lỗi về cục bộ).

---

### Track 5: Data Schemas & Mock Alignment (`DATA-01..02`) + Roadmap `Fix 3.2`

#### `DATA-01` — `OPEN`
- **Thực tế tại `9dd4214c`**: `tests/e2e/tauri-mock.ts:92` vẫn ghi `get_lineage_graph: { nodes: [], edges: [] },` — khác với hợp đồng `src/types/generated/LineageGraphPayload.ts:8-12` (yêu cầu `links` và `db_connected`, cũng là trường `src/App.tsx:846` đọc qua `lineageGraph?.links`) và thiếu `satisfies LineageGraphPayload` mà các dòng lân cận `:80-107` đang dùng. (Trùng PR `#36`).

#### `DATA-02` — `OPEN` (**bắt buộc khai báo đủ thuộc tính thật trước khi siết schema**)
- **Thực tế tại `9dd4214c`**: `map_manifest.schema.json:7,17,56,85,108,138` (audit trích `:45-120`) đặt `"additionalProperties": true` ở cấp gốc và cả 5 sub-object `worldArtifact`, `coordinateSystem`, `biomeTaxonomy`, `view`, `camera`.
- **Cảnh báo kiểm chứng thực tế**: `map_manifest.json:4-20, 28-29, 52-53` (được sinh bởi `scripts/gen_world_manifest.ts:125`) hiện đang chứa nhiều thuộc tính sinh thật nhưng **chưa có trong `map_manifest.schema.json`**: `_generated`, `worldArtifact.bytes`, `coordinateSystem.unitsPerCell`, `coordinateSystem.worldScale`, `view.bytes`, `view.checksum`. Nếu chỉ đổi hàng loạt sang `"additionalProperties": false` mà không cập nhật phần `properties` của schema (hoặc script sinh), kiểm schema sẽ gãy ngay lập tức.

#### Roadmap `Fix 3.2` (`whittaker_biome`) — `FALSE`
- **Báo cáo gốc**: Cho rằng `whittaker_biome` (`src-tauri/src/core/terrain.rs:197-214`) lệch với bảng `Biome` ở frontend.
- **Thực tế tại `9dd4214c`**: Đây là kiến trúc **hai taxonomy có chủ đích** đã được tài liệu hóa đầy đủ trong [BIOME_TAXONOMY.md](../../../BIOME_TAXONOMY.md) (`:10-35`), `src-tauri/src/core/world_artifact.rs:1-35`, và `map_manifest.schema.json:82-96` (`canonicalCount: 22`, `legacyCount: 11`): frontend dùng `Biome` 22 biến thể (`src/components/Landscape/utils/worldGen.ts:17-41`), backend dùng `BiomeType` 11 biến thể, cầu nối bằng `map_biome_backend_to_frontend` và `map_biome_frontend_to_backend`, được kiểm vét cạn trong `src-tauri/tests/empirical_m1_challenger_tests.rs:293-345` và test `s02_legacy_biome_lift_is_total_and_round_trips`.

---

## 4. Đề xuất 21 thẻ tiếp theo theo mẫu `§6` (failing-test-first)

Toàn bộ 21 thẻ dưới đây tương ứng 1–1 với **20 mục `OPEN`** và **1 mục `NEEDS-TEST`** (`REND-07`), tuân thủ đúng khung trường của `§6` trong [2026-11-05-swarm-prompts.md](../planning/2026-11-05-swarm-prompts.md). Theo `WORK_PLAN.md`: **mọi thẻ Wave 2+ đều chờ `T2` gộp xong vào `origin/feat/ecological-selection`** và chỉ bắt đầu khi chủ dự án thêm vào `WORK_PLAN.md` (kèm `R5-AUTH` nếu thẻ động đến `src-tauri/src/**` hoặc tệp được bảo vệ). Không viết test hay sửa code sớm hơn khi thẻ được giao.

### 1. `CARD F-INFRA-04` — Regenerate `NOTICE` and third-party license bundle on Windows after `src-tauri/Cargo.lock` stabilizes

```text
CARD F-INFRA-04 — Regenerate NOTICE and third-party license bundle on Windows after src-tauri/Cargo.lock stabilizes
WAVE 2 · [RUST] · MAX FILES 3
DEPENDS ON  T1 (`git merge-base --is-ancestor <T1_merge_sha> origin/fix/ci-gates-green` exits 0);
            T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0)
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-infra-04-notice-bundle
PR TARGET   feat/ecological-selection (draft)
DECISION    QD-C (must match active licensing policy)
AUTHORIZATION  R5-AUTH if modifying NOTICE / licensing/ outside T1 (per WORK_PLAN.md)
ALLOWED PATHS  NOTICE, licensing/THIRD_PARTY_LICENSES.txt, licensing/third-party-index.json

GOAL
Bring `NOTICE` and the committed third-party licence inventory in sync with `cargo tree` and transitive JavaScript packages on Windows so `tests/frontend/noticeInventory.test.ts` and `npm run check:notice` pass without loosening inventory assertions. Note: T1 (`B3b` Task 4) already regenerates A7 compliance artifacts on `fix/ci-gates-green` before T2; execute this card only if residual drift remains after T2 lands on trunk.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `NOTICE:1-62` (from commit `043930c`) manually lists only direct dependencies (`NOTICE:47`) and omits transitive JS packages (`scheduler`, `earcut`, `eventemitter3`, `zustand`) plus generated headings (`## JavaScript components — distributed`, `licensing/THIRD_PARTY_LICENSES.txt`) asserted by `tests/frontend/noticeInventory.test.ts:51-53, 70-74, 115-119`.
- Commit `a791248` updated header formatting in `scripts/gen_notice.mjs:99-105`. Per `BLOCK 0` R12, Cargo-backed notice/SBOM scripts must run on Windows (`cargo tree` host-target caveat in `scripts/lib/licensing.mjs`).
- Behavior hypothesis (not run on Mac/Linux or after T1/T2 merge): whether `T1` + `T2` leaves any residual discrepancy in `NOTICE`.

STEPS
1. First prove `T1` and `T2` are merged (`DEPENDS ON`), then run `npm ci`, `npx vitest run tests/frontend/noticeInventory.test.ts`, and (on a Cargo-capable Windows host/grid) `npm run check:notice`. If all pass already, stop and report as superseded by T1/T2.
2. Otherwise record exact failing assertions from `noticeInventory.test.ts` and `npm run check:notice` before regenerating via `npm run gen:notice` on Windows Cargo host.
3. Do NOT edit `tests/frontend/noticeInventory.test.ts` assertions or `scripts/lib/licensing.mjs` target rules (`BLOCK 0` R2/R12), and do NOT run notice generators on macOS/Linux.

DONE WHEN
- npx vitest run tests/frontend/noticeInventory.test.ts exits 0
- npm run check:notice exits 0 (on Windows Cargo host / grid)
- npm run check:sbom-schema exits 0

VERIFY EXTRA
Verifier confirms `git log -1 --stat` touches only the regenerated licensing files under ALLOWED PATHS and re-runs `npx vitest run tests/frontend/noticeInventory.test.ts` plus Windows `npm run check:notice`.
```

### 2. `CARD F-INFRA-05` — Remove global `@react-three/fiber` mock alias from root `tsconfig.json` and check type parity

```text
CARD F-INFRA-05 — Remove global @react-three/fiber mock alias from root tsconfig.json and check type parity
WAVE 2 · MAX FILES 8
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0)
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-infra-05-r3f-type-parity
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (frontend, test, and tsconfig paths only; verify WORK_PLAN.md at dispatch)
ALLOWED PATHS  tsconfig.json, tests/tsconfig.json, tests/mocks/react-three-fiber-mock.ts,
               src/r3f-intrinsics.d.ts, src/components/Landscape/*.tsx,
               tests/frontend/r3fTypeParity.test.ts (new)

GOAL
Ensure production TypeScript checking resolves `@react-three/fiber` against the real installed package types rather than the test mock, while keeping the Vitest test mock strictly typed without `any`.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `tsconfig.json:27` still maps `"@react-three/fiber": ["tests/mocks/react-three-fiber-mock.ts"]` globally while `vite.config.ts:199-203` enables that alias only when `mode === 'test'`.
- Commit `9f86e73` tightened `src/r3f-intrinsics.d.ts` and replaced `any` with `unknown` on `RootState`/`MockRenderer` (`tests/mocks/react-three-fiber-mock.ts:43-72`), but production `npx tsc --noEmit` still checks `Canvas`, `useFrame`, `useThree`, and `extend` against the mock instead of `@react-three/fiber`.
- Behavior hypothesis (read, not run without alias): removing line 27 in `tsconfig.json` may expose concrete type errors in `src/` or test mock signature drift.

STEPS
1. Move the `@react-three/fiber` mock path alias out of `tsconfig.json:27` (keeping it scoped to `tests/tsconfig.json` / Vitest) and add `tests/frontend/r3fTypeParity.test.ts` verifying export/type parity between mock exports and `@react-three/fiber`. Observe exact `npx tsc --noEmit` / `tests/frontend/r3fTypeParity.test.ts` failures first.
2. Fix revealed real type mismatches in `src/` components and align `tests/mocks/react-three-fiber-mock.ts` signatures (without adding `any` or weakening types).
3. Do NOT re-add `any`, suppress errors with `@ts-ignore` / `@ts-expect-error`, or narrow `typecheck` scripts (`BLOCK 0` R2/R4).

DONE WHEN
- npx tsc --noEmit exits 0 (against real `@react-three/fiber` types)
- npm run typecheck:tests exits 0
- npm run test:frontend exits 0

VERIFY EXTRA
Verifier runs `git grep -n 'react-three-fiber-mock' tsconfig.json tests/tsconfig.json` and confirms the mock alias is absent from root `tsconfig.json` while `npx tsc --noEmit` exits 0.
```

### 3. `CARD F-INFRA-06` — Remove wall-clock sleep jitter flakiness from `challenger_meta_ai_tests`

```text
CARD F-INFRA-06 — Remove wall-clock sleep jitter flakiness from challenger_meta_ai_tests
WAVE 2 · [RUST] · MAX FILES 1
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0)
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-infra-06-sleep-jitter-test
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (`src-tauri/tests/` only; `src-tauri/src` untouched)
ALLOWED PATHS  src-tauri/tests/challenger_meta_ai_tests.rs

GOAL
Eliminate Windows/CI scheduler-wakeup flakiness in `challenger_meta_ai_tests` caused by `std::thread::sleep(Duration::from_millis(16))` inside strict wall-clock assertions while keeping the compute-latency invariant strictly enforced (`BLOCK 0` R2).

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `src-tauri/tests/challenger_meta_ai_tests.rs:108-129` loops 50 iterations calling `std::thread::sleep(Duration::from_millis(16))` (`:121`, 800 ms baseline sleep) and asserts `tick_duration < Duration::from_millis(20)` (`:117`) and `total_elapsed < Duration::from_millis(1200)` (`:126`).
- Hypothesis (read, not run): OS sleep timer overshoot under CPU load triggers flaky failures unrelated to Rust tick compute time.

STEPS
1. Write/update a unit assertion in `src-tauri/tests/challenger_meta_ai_tests.rs` demonstrating how separating pure tick compute elapsed time from synthetic pacing (`sleep`) catches a synthetic slow-tick regression without depending on OS timer resolution. Observe failure if pure compute exceeds budget.
2. Refactor the test in `challenger_meta_ai_tests.rs:108-129` to measure cumulative/per-tick compute time excluding OS sleep overrun while asserting the strict compute ceiling (`BLOCK 0` R2).
3. Do NOT widen timing limits to multi-second values or add `#[ignore]` (`BLOCK 0` R2).

DONE WHEN
- cargo test --features desktop --test challenger_meta_ai_tests exits 0 (via grid / Windows Cargo host)
- cargo clippy --features desktop --tests -- -D warnings exits 0

VERIFY EXTRA
Verifier confirms the pure compute assertion remains strict (testing or inspecting the synthetic overrun threshold) and checks `git diff` has no loosened `>=`, `#[ignore]`, or removed assertions.
```

### 4. `CARD F-INFRA-07` — Add deterministic frontend tests for `worldGen.worker.ts` and `WorldTerrain.tsx`

```text
CARD F-INFRA-07 — Add deterministic frontend tests for worldGen.worker.ts and WorldTerrain.tsx
WAVE 2 · MAX FILES 3
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check T1 (`swarm/t1-b3b`) does not overlap `WorldTerrain.tsx` or `worldGen.worker.ts`
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-infra-07-worldgen-terrain-tests
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (frontend test files + optional pure helper export only)
ALLOWED PATHS  tests/frontend/worldGenWorker.test.ts (new), tests/frontend/worldTerrain.test.tsx (new),
               src/components/Landscape/utils/worldGen.worker.ts (pure export only if needed)

GOAL
Cover `src/components/Landscape/utils/worldGen.worker.ts` and `src/components/Landscape/WorldTerrain.tsx` with deterministic Vitest unit/component tests, closing the 0-test gap identified in `INFRA-07` while accurately documenting the jsdom WebGL shader boundary.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `src/components/Landscape/utils/worldGen.worker.ts` (loaded by `src/components/Landscape/utils/worldCache.ts:129`) and `src/components/Landscape/WorldTerrain.tsx` have 0 references in `tests/` or `src/__tests__/`.
- Note: `INFRA-07` audit inaccurately cited `utils/worldGen.worker.ts` and missed existing partial tests for `audioManager`/`PositionalAudio` (`tests/frontend/landscape_showcase.test.tsx:319-785`, `landscape_showcase_adversarial.test.tsx:364-441`) and `WorldWater.tsx` (`oceanShelf.test.ts:26`, `sceneClock.test.ts:147`).

STEPS
1. Write `tests/frontend/worldGenWorker.test.ts` with a minimal `self` / `postMessage` worker message harness testing deterministic heightfield/biome output, buffer dimensions, and invalid-payload error handling; observe initial import/harness failure before wiring any needed pure export.
2. Write `tests/frontend/worldTerrain.test.tsx` testing geometry/LOD/material prop invariants and verifying explicit documentation that actual WebGL GLSL compilation does not execute under jsdom (`tests/mocks/react-three-fiber-mock.ts:108-112`).
3. Do NOT weaken `react-three-fiber-mock.ts` or claim jsdom validates GLSL shader compilation (`BLOCK 0` R3).

DONE WHEN
- npx vitest run --dir tests worldGenWorker worldTerrain exits 0
- npm run test:frontend exits 0
- npm run typecheck:tests exits 0

VERIFY EXTRA
Verifier injects a deterministic seed/dimension mismatch in a scratch run (or checks negative assertion inside `worldGenWorker.test.ts`) to prove tests fail on broken worker payloads.
```

### 5. `CARD F-SIM-01` — Handle inside-sphere ray origins in `SpatialHashGrid::raycast` without self-collider blinding (`BLOCK 0` R9)

```text
CARD F-SIM-01 — Handle inside-sphere ray origins in SpatialHashGrid::raycast without self-collider blinding
WAVE 2 · [RUST] · MAX FILES 4
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #35 (`3560faa6`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-sim-01-raycast-inside-sphere
PR TARGET   feat/ecological-selection (draft)
DECISION    QD-B (owner decision on `src-tauri/src` write policy + `BLOCK 0` R8 if sensory hits alter trajectories)
AUTHORIZATION  R5-AUTH required for `src-tauri/src/physics/spatial.rs`, `src-tauri/src/core/agent_systems.rs`,
               `src-tauri/src/ai/model.rs` (AGENTS.md)
ALLOWED PATHS  src-tauri/src/physics/spatial.rs, src-tauri/src/core/agent_systems.rs,
               src-tauri/src/ai/model.rs, src-tauri/tests/physics_tests.rs

GOAL
Fix inside-sphere ray origin detection in `intersect_sphere` safely by adding explicit caster/root-agent exclusion inside `SpatialHashGrid::raycast`, preventing the `BLOCK 0` R9 failure mode where naive `Some(0.0)` makes every agent raycast hit itself at `t = 0.0` and go completely blind.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `intersect_sphere` (`src-tauri/src/physics/spatial.rs:321-343`) checks `if t1 >= 0.0 { Some(t1) } else { None }`, returning `None` when the ray origin lies inside a sphere (`c < 0`, `t1 < 0 <= t2`).
- Critical `BLOCK 0` R9 hazard: `SpatialHashGrid::raycast` (`src-tauri/src/physics/spatial.rs:135-308`) has NO self-exclusion filter and returns only one `closest_hit`. Both callers (`src-tauri/src/core/agent_systems.rs:966-978` and `src-tauri/src/ai/model.rs:934-947`) cast rays from `origin: agent_pos.0` (the center of the caster's own root `SpatialCollider`) and only check `if root_agent_id != entity` AFTER `raycast` returns.
- Open PR `#35` (`fix/ht0-5a-sensor-raycast`, `3560faa6`, unmerged on `9dd4214c`) overlaps this item.

STEPS
1. Write two failing unit/integration tests FIRST:
   (a) Ray starting inside an external target sphere hits that external sphere (`Some(0.0)` / hit reported);
   (b) Agent raycast from `agent_pos.0` inside its own root `SpatialCollider` skips self/owned segments inside `raycast` and still detects an external target (both overlapping and distant). Confirm tests fail on trunk.
2. Add explicit self/root-agent exclusion parameter/filter to `SpatialHashGrid::raycast` alongside safe inside-sphere handling in `intersect_sphere`, and pass caster `root_agent_id` from `agent_systems.rs:966-978` and `ai/model.rs:934-947`.
3. Measure before/after 180-tick deterministic simulation checksum (`BLOCK 0` R8) and document any checksum shift caused by detecting overlapping external targets; zero per-tick allocations (`BLOCK 0` R10). Do NOT change `intersect_sphere` without `raycast` exclusion.

DONE WHEN
- cargo test --features desktop exits 0 (via grid)
- zero per-tick allocation gates pass
- 180-tick determinism checksum before/after recorded and deterministic across 6 processes

VERIFY EXTRA
Verifier reverts self-exclusion (leaving only `Some(0.0)` in `intersect_sphere`) in a scratch copy and confirms test (b) turns RED with self-blinding.
```

### 6. `CARD F-SIM-02` — Guard degenerate `joint_axis` across decode, crossover, and `resolve_joints_system`

```text
CARD F-SIM-02 — Guard degenerate joint_axis across decode, crossover, and resolve_joints_system
WAVE 2 · [RUST] · MAX FILES 5
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #37 (`df5f3336`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-sim-02-joint-axis-guard
PR TARGET   feat/ecological-selection (draft)
DECISION    QD-B (`src-tauri/src` write authorization)
AUTHORIZATION  R5-AUTH required for `src-tauri/src/physics/dynamics.rs`, `src-tauri/src/evolution/genotype.rs`,
               `src-tauri/src/core/spawn.rs`, `src-tauri/src/evolution/crossover.rs` (AGENTS.md)
ALLOWED PATHS  src-tauri/src/physics/dynamics.rs, src-tauri/src/evolution/genotype.rs,
               src-tauri/src/core/spawn.rs, src-tauri/src/evolution/crossover.rs,
               src-tauri/tests/physics_tests.rs

GOAL
Eliminate `NaN` quaternion propagation when an edge has zero or near-zero `joint_axis` by guarding `resolve_joints_system` and normalizing/sanitizing axes consistently at `decode_genotype` and `crossover_genotypes`.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `resolve_joints_system` (`src-tauri/src/physics/dynamics.rs:48-55`) calls `Quat::from_axis_angle(axis_val.normalize(), cpg.output)` (`:50`).
- While `src-tauri/src/evolution/mutation.rs:139,177` uses `.try_normalize().unwrap_or(Vec3::Y)` and `src-tauri/src/core/components.rs:282-290` validates edges on migration, `decode_genotype` (`src-tauri/src/evolution/genotype.rs:195`, `src-tauri/src/core/spawn.rs:195`) and `crossover_genotypes` (`src-tauri/src/evolution/crossover.rs:88,106,145`) copy raw `edge.joint_axis` without clamping/fallback.
- Open PR `#37` commit `df5f3336` fixes `dynamics.rs` only (unmerged on `9dd4214c`); `genotype.rs`/`spawn.rs`/`crossover.rs` boundaries should also be checked.

STEPS
1. Write a failing test in `src-tauri/tests/physics_tests.rs` constructing a joint edge with `Vec3::ZERO` (and sub-epsilon axis) through `decode_genotype`, `crossover_genotypes`, and `resolve_joints_system`, asserting finite non-`NaN` rotations and `Vec3::Y` fallback. Observe failure (`NaN`) first.
2. Replace `.normalize()` with `.try_normalize().unwrap_or(Vec3::Y)` in `dynamics.rs:50` and apply a shared sanitization helper across `genotype.rs`, `spawn.rs`, and `crossover.rs` without per-tick heap allocation (`BLOCK 0` R10).
3. Verify standard 180-tick checksum invariance for valid non-degenerate genotypes (`BLOCK 0` R8).

DONE WHEN
- cargo test --features desktop exits 0 (via grid)
- cargo clippy --features desktop --all-targets -- -D warnings exits 0

VERIFY EXTRA
Verifier runs the new degenerate-axis unit/integration tests and confirms 180-tick deterministic checksum is unchanged for standard non-degenerate runs.
```

### 7. `CARD F-SIM-03` — Reconcile predator root energy-zero locomotion gate (`BLOCK 0` R8 + `QD-A'`)

```text
CARD F-SIM-03 — Reconcile predator root energy-zero locomotion gate
WAVE 2 · [RUST] · MAX FILES 2
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #37 (`41d30dec`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-sim-03-predator-energy-gate
PR TARGET   feat/ecological-selection (draft)
DECISION    QD-B, QD-A' (predator root zero-energy behavior vs active MortalityPolicy)
AUTHORIZATION  R5-AUTH required for `src-tauri/src/physics/dynamics.rs` (AGENTS.md)
ALLOWED PATHS  src-tauri/src/physics/dynamics.rs, src-tauri/tests/physics_tests.rs

GOAL
Resolve the asymmetry where child segments and CPG oscillators halt at `homeo.energy <= 0.0` for all agents while root segments only halt for `Prey`, after owner sign-off under `QD-A'` and `BLOCK 0` R8 checksum re-measurement.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: in `src-tauri/src/physics/dynamics.rs:146-157`, child segments (`:146-151`) and `update_cpg_system` (`src-tauri/src/ai/cpg.rs:49-53`) check `homeo.energy <= 0.0` regardless of trophic role, whereas the root segment branch (`dynamics.rs:152-155`) computes `let is_prey = prey_query.get(entity).is_ok(); is_prey && homeo.energy <= 0.0`, leaving zero-energy predator roots moving.
- Changing this alters ecological dynamics and 180-tick checksums (`BLOCK 0` R8). Open PR `#37` commit `41d30dec` (unmerged on `9dd4214c`) proposes stopping predators as well.

STEPS
1. Confirm `QD-A'` / owner intent before editing simulation semantics (`WORK_PLAN.md`).
2. Add a failing test in `src-tauri/tests/physics_tests.rs` spawning a Predator with `homeo.energy = 0.0` and proving root-segment velocity/actuation differs inconsistently from child segments / CPG freeze; observe failure first.
3. Update `dynamics.rs:152-155` uniformly (`homeo.energy <= 0.0` or owner-selected policy) with zero per-tick allocations (`R10`), and measure before/after 180-tick checksum plus impact on published experiment baselines (`BLOCK 0` R8).

DONE WHEN
- cargo test --features desktop exits 0 (via grid)
- Before/after 180-tick checksum delta and `BLOCK 0` R8 impact statement recorded in report

VERIFY EXTRA
Verifier verifies 180-tick determinism across >= 6 grid processes on the new semantics and confirms no hidden changes to other tick systems.
```

### 8. `CARD F-SIM-04` — Fix loop FPS telemetry calculation after `thread::sleep` (`BLOCK 0` R8 separated)

```text
CARD F-SIM-04 — Fix loop FPS telemetry calculation after thread::sleep
WAVE 2 · [RUST] · MAX FILES 2
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #37 (`c7829689`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-sim-04-loop-fps-telemetry
PR TARGET   feat/ecological-selection (draft)
DECISION    QD-B (`src-tauri/src` write authorization); separate owner decision if adding tick accumulator
AUTHORIZATION  R5-AUTH required for `src-tauri/src/core/simulation_loop.rs` (AGENTS.md)
ALLOWED PATHS  src-tauri/src/core/simulation_loop.rs, src-tauri/tests/physics_tests.rs (only if unit test lives outside `simulation_loop.rs`)

GOAL
Fix `simulation_loop.rs` FPS telemetry so reported `actual_fps` reflects total frame period (including `thread::sleep` / target frame budget) rather than raw compute-only `1.0 / elapsed`, while keeping tick stepping deterministic unless an accumulator change is separately authorized under `BLOCK 0` R8.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `src-tauri/src/core/simulation_loop.rs:2265-2295` (audit cited `:2243-2274`) computes `let actual_fps = 1.0 / elapsed.as_secs_f64();` (`:2270`) BEFORE `thread::sleep(target_frame_duration - elapsed);` (`:2293-2295`), reporting synthetic compute speed (thousands of FPS) instead of actual ~60 FPS cadence.
- Open PR `#37` commit `c7829689` (unmerged on `9dd4214c`) introduces `compute_loop_fps` taking `compute_elapsed.max(target_frame_duration)`.
- Any frame-lag tick accumulator directly alters simulation step count/determinism and MUST remain out of this telemetry PR unless separately authorized under `BLOCK 0` R8.

STEPS
1. Extract a pure frame-rate helper in `src-tauri/src/core/simulation_loop.rs` and write a failing unit test showing that a fast compute pass (e.g., 1 ms compute with 16.67 ms target frame duration) currently yields ~1000 FPS instead of ~60 FPS (and verify overload case < 60 FPS). Observe failure first.
2. Update `actual_fps` using total frame period (`compute.max(target_frame_duration)` or measured post-sleep period) guarded against zero division.
3. Verify the 180-tick simulation checksum is bit-identical before and after (`BLOCK 0` R8); do NOT alter tick-stepping or add an accumulator in this card.

DONE WHEN
- cargo test --features desktop exits 0 (via grid)
- Before/after 180-tick simulation checksums are bit-identical

VERIFY EXTRA
Verifier confirms checksum bit-identity across >= 6 processes on grid and verifies `git diff` touches only FPS telemetry computation (no tick-count loop changes).
```

### 9. `CARD F-SIM-05` — Wrap `CpgOscillator::phase` with `rem_euclid` and multi-period invariant

```text
CARD F-SIM-05 — Wrap CpgOscillator::phase with rem_euclid and multi-period invariant
WAVE 2 · [RUST] · MAX FILES 1
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #37 (`0ced745f`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-sim-05-cpg-rem-euclid
PR TARGET   feat/ecological-selection (draft)
DECISION    QD-B (`src-tauri/src` write authorization)
AUTHORIZATION  R5-AUTH required for `src-tauri/src/ai/cpg.rs` (AGENTS.md)
ALLOWED PATHS  src-tauri/src/ai/cpg.rs

GOAL
Guarantee `CpgOscillator::phase` always remains bounded in `[0, TAU)` under arbitrarily large positive or negative phase steps using `rem_euclid(std::f32::consts::TAU)`, and record the 180-tick determinism impact (`BLOCK 0` R8).

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `src-tauri/src/ai/cpg.rs:21-27` wraps `self.phase` using a single conditional subtraction `if self.phase > 2.0 * std::f32::consts::PI { self.phase -= 2.0 * std::f32::consts::PI; }` (`:22-25`), leaving `phase >= TAU` when `phase + dphi >= 4π` or negative for negative inputs.
- Open PR `#37` commit `0ced745f` (unmerged on `9dd4214c`) implements `rem_euclid(two_pi)` + unit test.

STEPS
1. Write a failing unit test in `src-tauri/src/ai/cpg.rs` stepping `CpgOscillator` with `freq * dt` > 1 full period (and negative delta / initial phase `>= 4π`) asserting `0.0 <= phase && phase < TAU`. Observe failure first.
2. Replace single subtraction (`cpg.rs:22-25`) with `self.phase.rem_euclid(std::f32::consts::TAU)` without adding allocations (`R10`).
3. Measure and report before/after 180-tick deterministic simulation checksum (`BLOCK 0` R8) in case boundary floating-point rounding differs.

DONE WHEN
- cargo test --features desktop exits 0 (via grid)
- Before/after 180-tick determinism checksum comparison recorded

VERIFY EXTRA
Verifier runs the `cpg` unit tests plus 6-process 180-tick determinism check on grid and checks `git grep` confirms single-subtraction wrap is gone.
```

### 10. `CARD F-SIM-06` — Align `MorphologyGenotype::total_mass` across `genotype.rs` and `spawn.rs` with BFS-reachable nodes

```text
CARD F-SIM-06 — Align MorphologyGenotype::total_mass across genotype.rs and spawn.rs with BFS-reachable nodes
WAVE 2 · [RUST] · MAX FILES 3
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #37 (`d3965557`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-sim-06-bfs-total-mass
PR TARGET   feat/ecological-selection (draft)
DECISION    QD-B (`src-tauri/src` write authorization)
AUTHORIZATION  R5-AUTH required for `src-tauri/src/evolution/genotype.rs` and `src-tauri/src/core/spawn.rs` (AGENTS.md)
ALLOWED PATHS  src-tauri/src/evolution/genotype.rs, src-tauri/src/core/spawn.rs,
               src-tauri/tests/physics_tests.rs (optional integration check)

GOAL
Ensure `MorphologyGenotype::total_mass` counts only nodes reachable by BFS from `root_node` (matching `decode_genotype`) and eliminate or synchronize the duplicated implementation across `evolution/genotype.rs` and `core/spawn.rs`.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `MorphologyGenotype::total_mass` is duplicated verbatim in `src-tauri/src/evolution/genotype.rs:30-32` and `src-tauri/src/core/spawn.rs:30-32`, summing every `self.nodes` element (`self.nodes.iter().map(|n| n.mass).sum()`), while `decode_genotype` (`:80-213` in both files) spawns only BFS-reachable nodes from `root_node`.
- Although `is_valid_genotype` (`src-tauri/src/evolution/mutation.rs:28-67`) and `core/components.rs:269` catch some malformed offspring, manually constructed/deserialized genotypes with orphan nodes misreport `total_mass` vs actual decoded body mass.
- Open PR `#37` commit `d3965557` updates `genotype.rs`; `core/spawn.rs` duplicate must not diverge.

STEPS
1. Write a failing unit test constructing a `MorphologyGenotype` containing a disconnected orphan node (`root_node` does not reach it) and asserting `total_mass()` equals the summed mass of segments produced by `decode_genotype`. Observe failure first.
2. Traverse only BFS-reachable nodes from `root_node` (without heap allocations inside tick paths per `BLOCK 0` R10 — e.g. fixed stack/bitset bounds matching node cap) in a single shared helper or synchronized pair across `genotype.rs` and `spawn.rs`.
3. Measure and report before/after 180-tick deterministic simulation checksum (`BLOCK 0` R8).

DONE WHEN
- cargo test --features desktop exits 0 (via grid)
- `total_mass` test covers empty, connected, unreachable/orphan, diamond, cycle, and `decode_genotype` parity

VERIFY EXTRA
Verifier confirms zero divergent `total_mass` formula between `src-tauri/src/evolution/genotype.rs` and `src-tauri/src/core/spawn.rs` and verifies zero allocations in tick paths (`BLOCK 0` R10).
```

### 11. `CARD F-SIM-07` — Align `PheromoneGrid::sample_bilinear` with `(i + 0.5)` cell-center coordinates (`BLOCK 0` R8)

```text
CARD F-SIM-07 — Align PheromoneGrid::sample_bilinear with (i + 0.5) cell-center coordinates
WAVE 2 · [RUST] · MAX FILES 1
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0)
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-sim-07-pheromone-cell-center
PR TARGET   feat/ecological-selection (draft)
DECISION    QD-B (`src-tauri/src` write authorization) + explicit owner R8 sign-off for sensory coordinate shift
AUTHORIZATION  R5-AUTH required for `src-tauri/src/ai/pheromone.rs` (AGENTS.md)
ALLOWED PATHS  src-tauri/src/ai/pheromone.rs

GOAL
Fix the half-cell spatial offset and boundary wrap mismatch between `PheromoneGrid::pos_to_index` (cell `i` spanning `[i, i+1)` with center `i + 0.5`) and `PheromoneGrid::sample_bilinear` (which currently treats integer lattice vertices `(i, j)` as cell centers), and document the `BLOCK 0` R8 checksum impact.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: in `src-tauri/src/ai/pheromone.rs`, `pos_to_index` (`:68-69`) computes `(norm * GRID_SIZE as f32).floor()` (centering cell `i` at `i + 0.5`), whereas `sample_bilinear` (`:102-113`) computes `gx = (norm_x * GRID_SIZE as f32).rem_euclid(GRID_SIZE as f32)`, `gx0 = (gx.floor() as usize) % GRID_SIZE`, `tx = (gx - gx.floor()).clamp(0.0, 1.0)`.
- Because pheromone deposition and sampling disagree by `0.5` cells, every chemical gradient sample shifts spatially and wraps asymmetrically; fixing this changes agent sensory inputs and 180-tick checksums (`BLOCK 0` R8).

STEPS
1. Write a failing unit test depositing pheromone via world coordinates into cell `(i, j)` and sampling at exact cell center `(i + 0.5) / GRID_SIZE`, `(j + 0.5) / GRID_SIZE`, asserting `sample_bilinear` returns the exact unblended concentration stored in `(i, j)` (and tests symmetric midpoint / wrap around boundaries). Observe failure first.
2. Shift continuous sample grid coordinate by `- 0.5` (`((norm * GRID_SIZE as f32) - 0.5).rem_euclid(GRID_SIZE as f32)`) before splitting `gx0`, `gx1`, `tx` (same for `y`), keeping zero tick allocations (`R10`).
3. Report before/after 180-tick simulation checksums and impact on published ecological baselines (`BLOCK 0` R8).

DONE WHEN
- cargo test --features desktop exits 0 (via grid)
- Before/after 180-tick determinism checksums and symmetry assertions recorded

VERIFY EXTRA
Verifier confirms `sample_bilinear` at cell centers returns pure single-cell values and runs 6-process determinism verification on grid.
```

### 12. `CARD F-SIM-08` — Render streamed `pitch`, `roll`, `length`, and `radius` in `LiveAgents.tsx`

```text
CARD F-SIM-08 — Render streamed pitch, roll, length, and radius in LiveAgents.tsx
WAVE 2 · MAX FILES 2
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0)
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-sim-08-live-agents-geometry
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (frontend component + frontend test only)
ALLOWED PATHS  src/components/Landscape/LiveAgents.tsx, tests/frontend/liveAgentsOrientation.test.ts (new)

GOAL
Consume `pitch`, `roll`, `length`, and `radius` already streamed by the backend in `SegmentState` inside `src/components/Landscape/LiveAgents.tsx` instead of uniform yaw-only spheres, while preserving the intentional `surfaceHeight(...) + GROUND_OFFSET` vertical anchor.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `src-tauri/src/core/simulation_loop.rs:2007, 2045-2052` computes and streams `pitch`, `roll`, `length`, and `radius` on `SegmentState`, yet `src/components/Landscape/LiveAgents.tsx:77, 145` constructs a fixed `SphereGeometry(SEGMENT_RADIUS, 10, 8)` and applies only `dummy.rotation.set(0, seg.yaw, 0)`.
- Note (`SIM-08` partial `FALSE` finding): `LiveAgents.tsx:138-142` intentionally anchors `ry = surfaceHeight(...) + GROUND_OFFSET` because backend `[0, 10]` physics Y does not represent 3D terrain elevation. Keep that documented contract intact.

STEPS
1. Extract a pure transform helper in `LiveAgents.tsx` and write a failing test `tests/frontend/liveAgentsOrientation.test.ts` verifying distinct `pitch`, `roll`, `length`, and `radius` inputs produce distinct rotation/scale transforms while keeping `surfaceHeight(...) + GROUND_OFFSET` Y clamping intact. Observe failure first.
2. Apply backend rotation order (`yaw`, `pitch`, `roll`) and scale/capsule segment dimensions by `length` and `radius` without per-frame allocations (`R10` / zero GC churn in `useFrame`).
3. Do NOT change `ry = surfaceHeight(...) + GROUND_OFFSET` (`SIM-08` contract).

DONE WHEN
- npx vitest run --dir tests liveAgentsOrientation exits 0
- npm run test:frontend exits 0
- npx tsc --noEmit exits 0

VERIFY EXTRA
Verifier checks `git diff src/components/Landscape/LiveAgents.tsx` preserves `surfaceHeight(...) + GROUND_OFFSET` and confirms no per-frame `new THREE.*` allocations inside `useFrame`.
```

### 13. `CARD F-REND-01` — Remove per-frame `computeVertexNormals()` from legacy `Terrain.tsx`

```text
CARD F-REND-01 — Remove per-frame computeVertexNormals() from legacy Terrain.tsx
WAVE 2 · MAX FILES 2
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #36 (`a5b7960d`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-rend-01-terrain-normals
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (frontend component + frontend test only)
ALLOWED PATHS  src/components/Landscape/Terrain.tsx, tests/frontend/landscape_showcase.test.tsx (or new `tests/frontend/terrainLod.test.tsx`)

GOAL
Stop `Terrain.tsx` from mutating geometry vertices and calling `geom.computeVertexNormals()` inside `useFrame` every frame across legacy LOD meshes (up to 250k vertices at `width=500, height=500`).

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `src/components/Landscape/Terrain.tsx:147-184` executes `animateLOD` inside `useFrame` (steps 1, 4, 16) and invokes `geom.computeVertexNormals()` (`:177`) each frame whenever `cell.elevation < 3.0 && !cell.isRiver`.
- `Terrain.tsx` is mounted by legacy `src/components/Landscape/LandscapeShowcase.tsx:208`, NOT by the active M0 showcase `src/components/Landscape/WorldShowcase.tsx:461-480` (which mounts `<WorldTerrainLod>` at `:462` or `<WorldTerrain>` at `:474`, defined in `src/components/Landscape/WorldTerrain.tsx:1-158`).
- Open PR `#36` (`fix/ht0-5b-landscape-dispose`, `a5b7960d`, unmerged on `9dd4214c`) overlaps this item.

STEPS
1. Write a failing test spying on `computeVertexNormals` across simulated `useFrame` ticks of `Terrain.tsx` asserting 0 calls inside `useFrame`. Observe failure first.
2. Remove per-frame `computeVertexNormals()` / CPU vertex mutation from `animateLOD` (keeping normal computation at geometry creation or moving shimmer to shader uniforms).
3. Verify legacy showcase tests and typechecks pass without altering `WorldTerrain.tsx`.

DONE WHEN
- npm run test:frontend exits 0
- npx tsc --noEmit exits 0
- npm run build exits 0

VERIFY EXTRA
Verifier runs `git grep -n 'computeVertexNormals' src/components/Landscape/Terrain.tsx` and confirms it does not execute inside `useFrame`.
```

### 14. `CARD F-REND-02` — Dispose manual geometries/materials in `Vegetation.tsx` & `WorldVegetation.tsx` and memoize `Water.tsx` `lakeUniforms`

```text
CARD F-REND-02 — Dispose manual geometries/materials in Vegetation & WorldVegetation and memoize lakeUniforms
WAVE 2 · MAX FILES 4
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #36 (`a5b7960d`, unmerged on 9dd4214c) before starting;
            serialize with F-REND-04 (shared `Vegetation.tsx` and `WorldVegetation.tsx`)
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-rend-02-vegetation-dispose
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (frontend components + new test file only)
ALLOWED PATHS  src/components/Landscape/Vegetation.tsx, src/components/Landscape/WorldVegetation.tsx,
               src/components/Landscape/Water.tsx, tests/frontend/vegetationDispose.test.tsx (new)

GOAL
Close remaining GPU resource disposal leaks in `Vegetation.tsx` and `WorldVegetation.tsx` on unmount and stop recreating `lakeUniforms` on every re-render in `Water.tsx`.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`:
  1. `src/components/Landscape/Vegetation.tsx:355-474` allocates 17 geometries and 17 materials inside `useMemo` rendered via `<primitive>` (`:644-782`) with 0 `.dispose()` calls on unmount.
  2. `src/components/Landscape/WorldVegetation.tsx:166` allocates 14 geometries via `makeFloraGeometry` with 0 `.dispose()` calls (`:152` material uses declarative JSX).
  3. Already fixed/false parts of `REND-02`: `Water.tsx:684-692` already disposes `heightMapTexture`, `oceanGeometry`, and `riverData.geom` in `useEffect` (and `:738-746` lake `<circleGeometry>`/`<shaderMaterial>` use declarative JSX), leaving only `lakeUniforms` recreation per render at `:726`; `Sky.tsx:115-205` uses declarative R3F JSX.
- Open PR `#36` (`a5b7960d`, unmerged on `9dd4214c`) overlaps `REND-02`.

STEPS
1. Write `tests/frontend/vegetationDispose.test.tsx` spying on `.dispose()` for every manual geometry/material allocated by `Vegetation.tsx` and `WorldVegetation.tsx` on unmount, and asserting `lakeUniforms` reference stability across `Water.tsx` re-renders. Observe failures first.
2. Add cleanup `useEffect` hooks calling `.dispose()` on memoized manual geometries/materials in `Vegetation.tsx` and `WorldVegetation.tsx` (guarding against premature disposal of still-mounted swaps), and memoize `lakeUniforms` per lake/props in `Water.tsx:726`.
3. Leave `Sky.tsx` and `Water.tsx:684-692` existing disposal intact.

DONE WHEN
- npx vitest run --dir tests vegetationDispose exits 0
- npm run test:frontend exits 0
- npx tsc --noEmit exits 0

VERIFY EXTRA
Verifier verifies unmount call count equals allocation count (17+17 in `Vegetation.tsx` and 14 in `WorldVegetation.tsx`) in the test run without mocking away cleanup.
```

### 15. `CARD F-REND-03` — Clean up scratch `playground/RabbitVisualizer.tsx` render loops (`QĐ-D`)

```text
CARD F-REND-03 — Clean up scratch playground/RabbitVisualizer.tsx render loops
WAVE 2 · MAX FILES 2
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0)
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-rend-03-playground-rabbit-loops
PR TARGET   feat/ecological-selection (draft)
DECISION    QĐ-D (owner confirmation on fixing vs archiving/retiring scratch `playground/RabbitVisualizer.tsx`)
AUTHORIZATION  none (playground scratch + test file only)
ALLOWED PATHS  playground/RabbitVisualizer.tsx, tests/frontend/rabbitVisualizer.test.ts (new, if kept)

GOAL
Eliminate the duplicate `requestAnimationFrame` polling loop running in parallel with R3F `useFrame` and hoist the per-frame `new Float32Array(TOTAL_PARTS * 13)` mock buffer in `playground/RabbitVisualizer.tsx` (or retire the scratch component cleanly per `QĐ-D`).

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `playground/RabbitVisualizer.tsx:240-360` launches an async `requestAnimationFrame(fetchAndRenderRabbit)` loop simultaneously with R3F `useFrame` (`:340-360`), and its mock path `generateSingleRabbitJS` (`:44`) allocates `new Float32Array(TOTAL_PARTS * 13)` (130 floats) every frame.
- `playground/RabbitVisualizer.tsx` is a scratch file under `QĐ-D` (`26c8a96`). Hypothesis: never shipped in production bundle (`src/App.tsx`).

STEPS
1. Confirm `QĐ-D` owner decision (`fix` vs `retire`). If owner confirms `fix`:
   Write a failing test `tests/frontend/rabbitVisualizer.test.ts` verifying zero per-frame `Float32Array` allocations across repeated mock steps and verifying cleanup stops any scheduled `requestAnimationFrame` on unmount; observe failure first.
2. Remove the redundant loop and hoist a reusable `Float32Array(TOTAL_PARTS * 13)` scratch buffer (or archive per `QĐ-D`).
3. Do NOT touch `src-tauri/src/commands/simulation.rs` (`get_test_rabbit_state`, `RUST-01` `FALSE`).

DONE WHEN
- npm run test:frontend exits 0
- npx tsc --noEmit exits 0

VERIFY EXTRA
Verifier checks `git grep -n 'requestAnimationFrame\|new Float32Array' playground/RabbitVisualizer.tsx` to confirm no per-frame allocation or leaked duplicate loop remains.
```

### 16. `CARD F-REND-04` — Set `frustumCulled={false}` and consistent bounds on instanced flora meshes

```text
CARD F-REND-04 — Set frustumCulled={false} and consistent bounds on instanced flora meshes
WAVE 2 · MAX FILES 3
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #36 (`a5b7960d`, unmerged on 9dd4214c) before starting;
            serialize with F-REND-02 (shared `Vegetation.tsx` and `WorldVegetation.tsx`)
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-rend-04-flora-frustum-culling
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (frontend components + new test file only)
ALLOWED PATHS  src/components/Landscape/WorldVegetation.tsx, src/components/Landscape/Vegetation.tsx,
               tests/frontend/floraFrustumCulling.test.tsx (new)

GOAL
Prevent instanced vegetation in `WorldVegetation.tsx` and `Vegetation.tsx` from vanishing when the camera moves the single prototype geometry bounding sphere outside the view frustum, matching all other instanced meshes in the showcase.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `git grep frustumCulled` shows `LiveAgents.tsx:176`, `WorldBirds.tsx:76`, `WorldCaves.tsx:42`, `WorldFish.tsx:172`, `WorldWater.tsx:383`, and `WorldWildlife.tsx:356` already specify `frustumCulled={false}`.
- Only `src/components/Landscape/WorldVegetation.tsx:146-153` (audit cited `:191-210`) and `src/components/Landscape/Vegetation.tsx:639-784` (audit cited `:480-520`) omit `frustumCulled={false}` and do not expand `boundingSphere` / call `computeBoundingSphere()`.
- Open PR `#36` (`a5b7960d`, unmerged on `9dd4214c`) overlaps `REND-04`.

STEPS
1. Write a failing test `tests/frontend/floraFrustumCulling.test.tsx` rendering/inspecting instanced meshes emitted by `WorldVegetation.tsx` and `Vegetation.tsx` asserting `frustumCulled === false` (or world-covering bounding sphere). Observe failure first.
2. Add `frustumCulled={false}` consistently to all instanced flora `<instancedMesh>` / `<primitive>` instances in `WorldVegetation.tsx` and `Vegetation.tsx`.
3. Preserve all existing instance color/matrix attributes (`WorldVegetation.tsx:152-170`, `Vegetation.tsx:644-782`).

DONE WHEN
- npx vitest run --dir tests floraFrustumCulling exits 0
- npm run test:frontend exits 0
- npx tsc --noEmit exits 0

VERIFY EXTRA
Verifier runs `git grep -n 'frustumCulled' src/components/Landscape/` confirming flora meshes match all sibling instanced layers (`LiveAgents`, `WorldBirds`, `WorldCaves`, `WorldFish`, `WorldWater`, `WorldWildlife`).
```

### 17. `CARD F-REND-05` — Eliminate per-frame allocations in Minimap, CameraRig, and Wildlife

```text
CARD F-REND-05 — Eliminate per-frame allocations in Minimap, CameraRig, and Wildlife
WAVE 2 · MAX FILES 5
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #36 (`a5b7960d`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-rend-05-zero-frame-allocs
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (frontend components + new test file only)
ALLOWED PATHS  src/components/Landscape/WorldMinimap.tsx, src/components/Landscape/Minimap.tsx,
               src/components/Landscape/WorldCameraRig.tsx, src/components/Landscape/WorldWildlife.tsx,
               tests/frontend/noFrameAllocations.test.ts (new)

GOAL
Remove the four verified per-frame JS/Canvas heap allocations across `WorldMinimap.tsx`, `Minimap.tsx`, `WorldCameraRig.tsx`, and `WorldWildlife.tsx` to eliminate GC pauses during 60 FPS camera/render updates.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`:
  1. `src/components/Landscape/WorldMinimap.tsx:132`: `ctx.createImageData(size, size)` inside draw path.
  2. `src/components/Landscape/Minimap.tsx:100`: `ctx.createImageData(180, 180)` inside draw path.
  3. `src/components/Landscape/WorldCameraRig.tsx:358,363,369,371`: `new THREE.Euler` and `new THREE.Vector3` inside `useFrame`.
  4. `src/components/Landscape/WorldWildlife.tsx:274,295`: `new THREE.Object3D()` and `new THREE.Color()` inside `useFrame`.
- Open PR `#36` (`a5b7960d`, unmerged on `9dd4214c`) overlaps `REND-05`.

STEPS
1. Write failing tests `tests/frontend/noFrameAllocations.test.ts` spying on `ctx.createImageData` across repeated minimap redraws and verifying hoisted scratch reuse across `WorldCameraRig` / `WorldWildlife` frame helpers. Observe failures first.
2. Cache `ImageData` in refs keyed by dimensions (`WorldMinimap.tsx`, `Minimap.tsx`) and hoist module/ref scratch `Euler`, `Vector3`, `Object3D`, and `Color` instances outside per-frame callbacks (`WorldCameraRig.tsx`, `WorldWildlife.tsx`).
3. Ensure scratch objects are reset cleanly before reuse so no camera/wildlife state leaks between calls.

DONE WHEN
- npx vitest run --dir tests noFrameAllocations exits 0
- npm run test:frontend exits 0
- npx tsc --noEmit exits 0

VERIFY EXTRA
Verifier greps `useFrame` bodies and draw helpers in all 4 files for `createImageData`, `new THREE.Euler`, `new THREE.Vector3`, `new THREE.Object3D`, and `new THREE.Color`, confirming zero matches in frame hot paths.
```

### 18. `CARD F-REND-06` — Track and stop WebAudio synthesizer nodes in `AudioManager` and `PositionalAudio` on unmount

```text
CARD F-REND-06 — Track and stop WebAudio synthesizer nodes on unmount
WAVE 2 · MAX FILES 3
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #36 (`a5b7960d`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-rend-06-webaudio-dispose
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (frontend audio utilities/components + existing test file only)
ALLOWED PATHS  src/components/Landscape/utils/audioManager.ts, src/components/Landscape/PositionalAudio.tsx,
               tests/frontend/landscape_showcase.test.tsx

GOAL
Track every started `AudioBufferSourceNode` and `OscillatorNode` created inside `AudioManager::connectSynthesizer`, implement `AudioManager::dispose()`, and invoke cleanup from `PositionalAudio.tsx` unmount so orphaned WebAudio nodes do not continue synthesizing after route/showcase unmount.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `src/components/Landscape/utils/audioManager.ts:168-234` (`connectSynthesizer` `:186-233`) calls `.start()` on `AudioBufferSourceNode` and `OscillatorNode` (`:206,217,228`) without storing node handles or offering `dispose()`. `src/components/Landscape/PositionalAudio.tsx:18-20` lacks a `useEffect` unmount teardown.
- `tests/frontend/landscape_showcase.test.tsx:319-785` already tests `audioManager`/`PositionalAudio` construction. Open PR `#36` (`a5b7960d`, unmerged on `9dd4214c`) overlaps `REND-06`.

STEPS
1. Add a failing test in `tests/frontend/landscape_showcase.test.tsx` mounting and unmounting `PositionalAudio` / calling `audioManager.dispose()`, asserting `.stop()` and `.disconnect()` are invoked on all started synthesizer source/oscillator nodes. Observe failure first.
2. Store started synthesizer node references in `AudioManager`, guard double-stop safely, expose `dispose()`, and call teardown on `PositionalAudio.tsx` unmount (`useEffect` return).
3. Preserve existing adversarial audio tests (`landscape_showcase_adversarial.test.tsx:364-441`) without weakening mocks (`R2`).

DONE WHEN
- npx vitest run --dir tests landscape_showcase landscape_showcase_adversarial exits 0
- npm run test:frontend exits 0
- npx tsc --noEmit exits 0

VERIFY EXTRA
Verifier checks double-`dispose()` idempotence (calling `.stop()` safely even if already stopped) and verifies node tracking arrays are cleared on `dispose()`.
```

### 19. `CARD F-REND-07` — Measure UI re-render load (`NEEDS-TEST`) and throttle/memoize `App.tsx` tick updates

```text
CARD F-REND-07 — Profile App.tsx tick re-renders (NEEDS-TEST) and throttle/memoize hot UI paths
WAVE 2 · MAX FILES 2
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            coordinate with T1/T2 telemetry contracts on `src/App.tsx`
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-rend-07-app-tick-throttle
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (frontend `src/App.tsx` + new frontend test only)
ALLOWED PATHS  src/App.tsx, tests/frontend/appTickThrottle.test.tsx (new)

GOAL
Resolve `REND-07` (`NEEDS-TEST`) by first measuring `<App>` render-count and frame impact under 60 Hz `simulation-tick` events when `hierarchies`/`rootSegments` are already 200 ms-throttled (`src/App.tsx:481-489`), then throttling/isolating `setEnvironmentalState` and `setHeadDirections` (`:456-475`) and memoizing `renderGrid()` (`:396-425`) / chart derivation (`:841-873`) if measured work exceeds target frame budget.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `src/App.tsx:481-489` already throttles `setHierarchies` and `setRootSegments` at 200 ms, while `:456-475` invokes `setEnvironmentalState` and `setHeadDirections` on every `simulation-tick` event, causing `<App>` re-renders including unmemoized `renderGrid()` (`:396-425`) and the inline SVG IIFE (`:841-873`).
- `NEEDS-TEST`: Static inspection confirms unthrottled `setState` calls at `:456-475`, whereas actual React commit cost / UI FPS drop under realistic tick rates is a hypothesis until measured by a test/profiler.

STEPS
1. Write `tests/frontend/appTickThrottle.test.tsx` mounting `<App>` (mocking Tauri IPC/events cleanly) and emitting a burst of N `simulation-tick` events (e.g., 30 ticks within 150 ms) while measuring `<App>` render count, `renderGrid` invocations, and total React commit time (`NEEDS-TEST` baseline). Assert bounded re-renders; observe failure/unbounded render count first.
2. If baseline confirms redundant full-`<App>` renders on unthrottled ticks, throttle/isolate `environmentalState` and `headDirections` updates cleanly (preserving final/latest tick consistency on pause/stop) and memoize `renderGrid()` / lineage SVG derivation (`useMemo`).
3. Do NOT drop pause/final-tick state updates or weaken IPC event assertions (`BLOCK 0` R2).

DONE WHEN
- npx vitest run --dir tests appTickThrottle exits 0
- npm run test:frontend exits 0
- npx tsc --noEmit exits 0
- Report pastes measured before/after render counts from the test run

VERIFY EXTRA
Verifier verifies final-tick flush behavior (when simulation stops or crosses a 200 ms boundary, the latest `environmentalState` and `headDirections` are never stuck stale).
```

### 20. `CARD F-DATA-01` — Align `get_lineage_graph` mock payload in `tests/e2e/tauri-mock.ts` with `LineageGraphPayload`

```text
CARD F-DATA-01 — Align get_lineage_graph mock payload with LineageGraphPayload
WAVE 2 · MAX FILES 2
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            check open PR #36 (`a5b7960d`, unmerged on 9dd4214c) before starting
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-data-01-lineage-mock-shape
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (test mock + frontend test file only)
ALLOWED PATHS  tests/e2e/tauri-mock.ts, tests/frontend/tauriMockLineage.test.ts (new)

GOAL
Fix the mismatched E2E mock payload for `get_lineage_graph` in `tests/e2e/tauri-mock.ts:92` and enforce `satisfies LineageGraphPayload` at compile time so frontend tests and E2E harnesses match the generated Rust IPC contract `src/types/generated/LineageGraphPayload.ts`.

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `tests/e2e/tauri-mock.ts:92` defines `get_lineage_graph: { nodes: [], edges: [] },` without a `satisfies` check, whereas `src/types/generated/LineageGraphPayload.ts:8-12` and `src/App.tsx:846` (`lineageGraph?.links`) expect `{ nodes, links, db_connected }` (sibling entries at `:80-107` already use `satisfies ...`).
- Open PR `#36` (`a5b7960d`, unmerged on `9dd4214c`) fixes line 92.

STEPS
1. Write `tests/frontend/tauriMockLineage.test.ts` validating `MOCK_RESPONSES.get_lineage_graph` satisfies `LineageGraphPayload` (`links` array, `db_connected` boolean, zero `edges` field) and add `satisfies LineageGraphPayload` to line 92 first so `npm run typecheck:tests` / Vitest fails loudly on `{ nodes: [], edges: [] }`.
2. Update `tests/e2e/tauri-mock.ts:92` to `{ nodes: [], links: [], db_connected: false } satisfies LineageGraphPayload`.
3. Do NOT alter `src/types/generated/LineageGraphPayload.ts` manually (ts-rs generated contract).

DONE WHEN
- npm run typecheck:tests exits 0
- npx vitest run --dir tests tauriMockLineage exits 0
- npm run test:frontend exits 0
- npm run test:e2e exits 0

VERIFY EXTRA
Verifier checks `git grep -n 'satisfies LineageGraphPayload' tests/e2e/tauri-mock.ts` and confirms `edges:` is absent from the `get_lineage_graph` mock.
```

### 21. `CARD F-DATA-02` — Declare emitted manifest fields in `map_manifest.schema.json` and tighten `additionalProperties`

```text
CARD F-DATA-02 — Declare emitted manifest fields in map_manifest.schema.json and tighten additionalProperties
WAVE 2 · MAX FILES 2
DEPENDS ON  T2 (`git merge-base --is-ancestor <T2_merge_sha> origin/feat/ecological-selection` exits 0);
            coordinate with `scripts/gen_world_manifest.ts` (generator of `map_manifest.json:5`)
BASE        origin/feat/ecological-selection; record its sha
BRANCH      swarm/f-data-02-map-manifest-schema
PR TARGET   feat/ecological-selection (draft)
DECISION    none
AUTHORIZATION  none (schema + new frontend test file only)
ALLOWED PATHS  map_manifest.schema.json, tests/frontend/mapManifestSchema.test.ts (new)

GOAL
Declare every legitimate generated field currently emitted in `map_manifest.json` (`_generated`, `worldArtifact.bytes`, `coordinateSystem.unitsPerCell`, `coordinateSystem.worldScale`, `view.bytes`, `view.checksum`) inside `map_manifest.schema.json` before tightening `"additionalProperties": false` on root and all five sub-objects, preventing silent schema typos (`BLOCK 0`).

CONTEXT
- Observed 2026-11-14 at `9dd4214c`: `map_manifest.schema.json:7,17,56,85,108,138` (audit cited `:45-120`) currently sets `"additionalProperties": true` on root and all 5 sub-objects (`worldArtifact`, `coordinateSystem`, `biomeTaxonomy`, `view`, `camera`).
- Critical verification finding: `map_manifest.json:4-20, 28-29, 52-53` (generated by `scripts/gen_world_manifest.ts:125`) contains real emitted fields NOT declared in `map_manifest.schema.json`: `_generated`, `worldArtifact.bytes`, `coordinateSystem.unitsPerCell`, `coordinateSystem.worldScale`, `view.bytes`, and `view.checksum`. Flipping `additionalProperties` to `false` without declaring those properties first breaks validation of the committed `map_manifest.json`.

STEPS
1. Write a failing Ajv test `tests/frontend/mapManifestSchema.test.ts` validating BOTH (a) the committed `map_manifest.json` passes and (b) an unexpected typo field inside root and each sub-object (e.g., `view.bogusField`) is rejected. Observe (b) fails today because `additionalProperties: true` lets typos pass silently.
2. Add exact schemas for `_generated`, `worldArtifact.bytes`, `coordinateSystem.unitsPerCell`, `coordinateSystem.worldScale`, `view.bytes`, and `view.checksum` to `map_manifest.schema.json` (and re-run against all existing `map_manifest` fixtures/tests), then switch `"additionalProperties"` to `false` consistently across root and all sub-objects.
3. Do NOT edit `map_manifest.json` hand-values (`scripts/gen_world_manifest.ts` owns generated values and `mapManifestEvidence.test.ts` hashes the committed bytes per `map_manifest.json:10`).

DONE WHEN
- npx vitest run --dir tests mapManifestSchema mapManifestEvidence exits 0
- npm run test:frontend exits 0
- npm run check:sbom-schema exits 0

VERIFY EXTRA
Verifier inserts a typo key inside each of root, `worldArtifact`, `coordinateSystem`, `biomeTaxonomy`, `view`, and `camera` in memory/scratch test copy and confirms all 6 fail schema validation while unmodified `map_manifest.json` passes.
```

---

## 5. Nhật ký lệnh kiểm chứng đã chạy trong quá trình triage `D1`

| Lệnh đã chạy (tại `E:\03_Development_Projects\project\anima-wt\d1` trên nền `9dd4214c`) | Kết quả quan sát |
| --- | --- |
| `git ls-files \| grep -i audit_report` | Trống (exit 1 từ grep — không có tệp trên thân cây `9dd4214c`) |
| `git diff --name-only 491ec61e..9dd4214c` | Chỉ `docs/ai/README.md` và `docs/ai/monitoring/pr-branch-audit-2026-11-14.md` (`D0` `#42`) |
| `git diff a43872c5:docs/AUDIT_REPORT.md a43872c5:AUDIT_REPORT.md` | Khác biệt duy nhất dòng 352–372 (`## Audit Track Inventory`) |
| `git grep -nE "^<<<<<<< \|^=======$\|^>>>>>>> " -- src/ src-tauri/src src-tauri/tests tests/ scripts/` | Trống (`0` conflict markers) |
| `npm ci` | Exit `0` |
| `node scripts/check_docs_links.mjs` | Exit `0` (`docs link check: 931 relative links in 149 files, 0 broken`) |
| `npm run check:docs` | Exit `0` (`931` links / `0 broken`; `103 files, 0 errors`; `29 commands, 7 events, 0 differences`) |
| `npm run check:text-hygiene` | Exit `0` (`no raw control bytes`) |
| `npm run check:sbom-schema` | Exit `0` (`valid CycloneDX 1.5 — 458 components, 459 dependency records, 458 unique purls, 1 documented licence gaps, schemas pinned at c320fc0f0b46`) |
| `cargo test` / `npm run check:notice` | **Chưa chạy trên `anima-dell`** (quy tắc phần cứng cấm build Rust/chạy generator gọi Cargo trên laptop) |
