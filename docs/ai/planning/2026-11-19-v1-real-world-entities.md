---
phase: planning
feature: real-world-and-bodies
title: Thẻ V1 — Bản đồ 3D vẽ thực thể mô phỏng (hồ, cây, thức ăn) và gắn nhãn lớp trang trí
description: Prototype (to be replaced) cho thẻ V1 — dựng instanced hồ, cây, thức ăn từ environmental_state / get_environmental_elements qua hợp đồng toạ độ chung, bỏ qua lớp phủ cỏ vì chưa có IPC ResourceField, gắn nhãn và nút bật/tắt cho lớp trang trí, nâng ngân sách WorldShowcase 130 -> 145 KiB
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# Thẻ V1 — Bản đồ 3D vẽ thực thể của chính mô phỏng

> **Ghi chú prototype (`MSG_20261119_040227`, 2026-11-19):** đây là bản **`prototype, to be replaced`**,
> ưu tiên để chủ dự án mở thử và nhìn thấy thực thể của mô phỏng trên bản đồ 3D trước khi ANMW v3 / ADR-0010 thay thế.

## 1. Phạm vi và thay đổi mã (chỉ frontend)

- **`src/components/Landscape/utils/simEnvironment.ts`** (*mới*, `prototype, to be replaced`):
  - Chuẩn hoá `type` (`'lake' | 'tree' | 'food'`) và ánh xạ `EnvironmentalElement` sang toạ độ render thông qua `isInsideSimBounds`, `simToRender`, `simScale` (`utils/liveAgentTransform.ts`) và kiểm tra ranh giới lưới `256×256` qua `worldXzToCell` / `DEFAULT_XZ_BOUNDS` (`utils/coordinate.ts`).
  - Bảo toàn quy ước trục của backend (`src-tauri/src/core/components.rs:149-156`, `src/PixiViewport.tsx:609-614`): `EnvironmentalElement.x` → `renderX`, `EnvironmentalElement.y` → `renderZ` (tương ứng trục Z của Bevy).
  - Độ cao mặt đất lấy từ `surfaceHeight(...)` (`utils/worldSample.ts`) — cùng hàm mà `buildAgentPoses` (`utils/agentPose.ts`) dùng để đặt chân sinh vật, nên sinh vật và hồ/cây/thức ăn đứng trên cùng một mặt đất; riêng mặt hồ nâng nhẹ `LAKE_SURFACE_OFFSET = 0.18` để tránh z-fighting.
  - Màu biến thiên tất định theo mức `resources` qua `computeEntityColor`, **không** tô lại màu theo quần xã (D08).
- **`src/components/Landscape/LiveEnvironment.tsx`** (*mới*, `prototype, to be replaced`):
  - Tối đa **3 lệnh vẽ instanced** (`sim-env-lakes` gắn thẻ `water`, `sim-env-trees` và `sim-env-food` gắn thẻ `flora`), 1 instance cho mỗi thực thể hợp lệ do mô phỏng phát.
  - Nguồn dữ liệu: lấy trạng thái ban đầu qua `get_environmental_elements` (nếu có Tauri runtime, giống `src/PixiViewport.tsx:1156`) cho tới khi gói `simulation-tick` đầu tiên đến; cập nhật theo `environmental_state` qua kênh chia sẻ từ `LiveAgents` hoặc `tickSource`.
  - Ở chế độ `?capture=1` (nếu không truyền nguồn test/fetcher tường minh), trả về `null` và không gọi IPC để 8 ảnh chụp chuẩn và `rw_budget.spec.ts` không thay đổi.
  - Không dùng `useFrame`; cập nhật buffer instance trong `useLayoutEffect` khi có dữ liệu/cấu hình địa hình thay đổi.
- **`src/components/Landscape/LiveAgents.tsx`**:
  - Bổ sung prop tuỳ chọn `onEnvironmentalState` để chuyển tiếp `payload.environmental_state` sang kênh của `LiveEnvironment`, tránh mở listener `simulation-tick` thứ hai. Không thay đổi hành vi vẽ sinh vật hay quy tắc `surfaceHeight`.
- **`src/components/Landscape/WorldShowcase.tsx`**:
  - Gắn `<LiveEnvironment />` bên cạnh `<LiveAgents />`.
  - Bao `WorldCaves`, `WorldBirds`, `WorldFish`, `WorldWildlife` dưới cờ hiển thị `showDecoration` (mặc định `true`; luôn vẽ khi `capture !== null`), và thêm vào `WorldHud` vùng chú thích lớp (`world-layers-legend`) hiển thị:
    - Số lượng thực thể mô phỏng (`Simulated: X lakes · Y trees · Z food`), và
    - Nhãn thấy rõ + tooltip **`"Decoration, not simulated: fish, birds, wildlife, caves"`** kèm nút ẩn/hiện lớp trang trí (`toggle-decoration-btn`), không xoá các component trang trí khỏi cảnh.
- **`tests/frontend/v1RealWorldEntities.test.tsx`** (*mới*):
  - 4 ca test kiểm tra: (1) ánh xạ toạ độ & đồng nhất `surfaceHeight` với sinh vật qua `buildAgentPoses`; (2) tick tổng hợp với 2 hồ, 3 cây, 2 thức ăn dựng đúng số instance tại vị trí đã ánh xạ và co lại còn 1 thức ăn ở tick kế tiếp; (3) lấy dữ liệu khởi đầu qua `get_environmental_elements` trong Tauri và giữ im lặng ở `?capture=1`; (4) nhãn `"Decoration, not simulated"` + tooltip + nút bật/tắt `WorldFish`/`WorldBirds`/`WorldWildlife`/`WorldCaves`.

## 2. Lớp phủ mật độ cỏ (`ResourceField`) — bỏ qua có chứng cứ

Mục này **được bỏ qua trong thẻ V1** (không sửa `src-tauri/`):
- `SimulationTickPayload` (`src-tauri/src/core/simulation_state.rs:52-56`, `src/types/generated/SimulationTickPayload.ts:5`) chỉ mang `{ segments, environmental_state, head_directions }`, không có `ResourceField`.
- Lệnh `get_spatial_density_matrix` (`src-tauri/src/commands/environment.rs:182-218`) truyền `None` vào tham số `field: Option<&ResourceField>` của `generate_spatial_density_matrix` (`src-tauri/src/core/telemetry_export.rs:425-500`), khiến `vegetation_density` luôn mang hằng số `0.5f64` mỗi ô (`telemetry_export.rs:439`, đã ghi tại `docs/reference/IPC_CONTRACT.md:51`).
- Không có lệnh IPC hay event Tauri nào khác xuất lưới `ResourceField`.

## 3. Ngân sách bundle & số đo sanity (`prototype`)

- **`scripts/check_bundle_budget.mjs`**: nâng trần chunk `WorldShowcase` từ `130` lên `145` KiB với lý do **`prototype`** theo chỉ thị `PROTOTYPE-FIRST` (`MSG_20261119_040227`, 2026-11-19). Chunk `WorldShowcase` trên thân cây đã ở mức `~127,5 KiB`; sau khi thêm `LiveEnvironment` + `simEnvironment` + chú thích trang trí, kích thước đo được là **`132,9 KiB`** (`136.11 kB`, `92%` của `145 KiB`), tổng toàn bộ JS đạt **`1897,9 / 2000 KiB`**.
- Kiểm tra WebGL sanity (headless Chromium `1280×800`, `vite preview` + mock Tauri với 2 hồ, 3 cây, 2 thức ăn): hiển thị `Simulated: 2 lakes · 3 trees · 2 food` và `Decoration, not simulated: fish, birds, wildlife, caves`, `sim-environment` có `2 / 3 / 2` instance, `window.__animaFrameCounters` ghi nhận `26` draw calls / `687.356` tam giác (`water`: `6` calls / `38.878` tris; `flora`: `5` calls / `319.742` tris), `0` `console.error` / `pageerror`. Ảnh chụp kiểm tra lưu ở hub: `test_runs/anima-case_V1_screenshot.png`.

## 4. Không khẳng định

- Không khẳng định cảnh 3D là toàn bộ bản đồ thống nhất: địa hình nền vẫn đến từ bộ sinh JS (`WorldShowcase`) cho tới khi W1/W2 (ANMW v3 / ADR-0010) hợp nhất, và lớp phủ mật độ cỏ `ResourceField` chưa tồn tại ở frontend vì chưa có IPC tương ứng.
- Số tam giác/draw call trên không phải benchmark trên phần cứng mục tiêu hay ma trận `10/200/1000 × tier`; thẻ B1 và cổng hiệu năng sẽ đo riêng khi có thân liền.
- Cá, chim, thú nền và hang vẫn là lớp trang trí giả lập và được gắn nhãn rõ ràng trên HUD.

## 5. Kết quả gate

- `npm run lint` & `node scripts/eslint_ratchet.mjs`: `0 errors, 0 warnings (baseline 0)`.
- `npm run typecheck:tests`: thoát `0`.
- `npm run test`: `14` file / `109` test pass (`src/**`).
- `npm run test:frontend`: `69` file / `704` test pass (bao gồm toàn bộ `rw*`, `map2*`, `landscape_showcase`, và `v1RealWorldEntities.test.tsx`).
- `npm run build`: thoát `0` (`840` module).
- `npm run check:bundle`: `WorldShowcase 132.9 KiB` (`<= 145 KiB`), tổng `1897.9 / 2000 KiB` — OK.
- `npm run check:ipc`, `npm run check:csp`, `npm run check:docs`: thoát `0`.
