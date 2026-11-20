---
phase: planning
feature: real-world-and-bodies
kind: thẻ
title: Thẻ F050 — đưa bước khởi tạo thế giới dùng chung vào beforeAll của Vitest
description: Chuyển chi phí khởi tạo thế giới 2048x2048 lần đầu trong 17 file test frontend sang hook beforeAll có ngân sách 60s để mọi test đầu tiên trên CI dưới 5s mà không nới testTimeout.
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-19
review_cycle: weekly
---

# Thẻ F050 — Trần thời gian render đầu tiên trong Vitest (`warmSharedWorld`)

## 1. Bối cảnh & Bằng chứng CI

- Ở các file dưới `tests/frontend/` render `<App />` hoặc `<WorldShowcase />`, lần render đầu tiên phải sinh thế giới dùng chung `2048x2048` (`SHARED_WORLD_SEED`, `SHARED_WORLD_OPTS`) đồng bộ trên luồng chính vì jsdom không có Web Worker ([`worldCache.ts`](../../../src/components/Landscape/utils/worldCache.ts)).
- Chi phí một lần này trước đây nằm trong thân test đầu tiên dùng `<App />`/`<WorldShowcase />`, chạm sát trần `15.000 ms` mặc định của Vitest trên runner CI chậm hơn (~`+38%`):
  - Trunk run `44507087207` (job `135501389851`): [`v1RealWorldEntities.test.tsx`](../../../tests/frontend/v1RealWorldEntities.test.tsx) mất `10.002 ms`.
  - PR `#161` run `44509366469` (job `135509371225`): cùng test đó vượt ngưỡng timeout ở `15.048 ms` (tổng file `15.165 ms`).

## 2. Thiết kế áp dụng (`tests/frontend/helpers/warmSharedWorld.ts`)

- Tạo helper [`tests/frontend/helpers/warmSharedWorld.ts`](../../../tests/frontend/helpers/warmSharedWorld.ts) gọi `loadOrGenerateWorld(SHARED_WORLD_SEED, SHARED_WORLD_OPTS)` từ module thật để nạp sẵn memo của `worldCache.ts` trước khi bất kỳ test nào chạy, xả microtask/macrotask cho `persistWorldArtifact`, và xuất hằng số `WARM_SHARED_WORLD_BUDGET_MS = 60_000`.
- Bổ sung `import { warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS } from './helpers/warmSharedWorld'` và `beforeAll(warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS)` trong `describe(...)` ở `17` file đủ điều kiện; **không** sửa `src/`, `vite.config.ts`, `tests/vitest.config.ts`, hoặc nâng `testTimeout`.

## 3. Bảng thời gian từng file (`17` file đã thêm hook)

> **Ghi chú đo CI:**
> - **Trước (CI trunk `44507087207`, job `135501389851`):** thời lượng test chậm đầu tiên chứa chi phí khởi tạo thế giới (và tổng thời lượng file).
> - **Sau (CI PR `#169` commit `33c6d223`, run [`44523362754`](https://github.com/DuongNAD/Anima-Engine/actions/runs/44523362754), job `135557441154`):** `npm run test:frontend` đạt `73 passed (73)` / `732 passed (732)` trong `110.37s`. Ở chế độ reporter mặc định trên CI, Vitest chỉ in chi tiết từng test `> 300 ms`; các test không có dòng riêng đạt `<= 300 ms`. Toàn bộ `17/17` file có test đầu tiên (và test render đầu tiên) **dưới `5.000 ms`** (cao nhất là `v1RealWorldEntities.test.tsx` ở `3.443 ms`, so với `10.002 ms` trên trunk và timeout `15.048 ms` ở PR `#161`).

| File (`tests/frontend/...`) | Trước — CI trunk `44507087207` (test / file) | Sau — CI PR `#169` `44523362754` (test đầu tiên / file) | `beforeAll` (`warmSharedWorld`) |
|---|---|---|---|
| `b1HudSyntheticLabel.test.tsx` | `9787 ms` / `9789 ms` | `3428 ms` / `8400 ms` | `4916 ms` |
| `dashboard_grid.test.tsx` | `6694 ms` / `7173 ms` | `<= 300 ms` / `5867 ms` | `5176 ms` |
| `leak_proof.test.tsx` | `6643 ms` / `6827 ms` | `<= 300 ms` / `5438 ms` | `5015 ms` |
| `phase3_ui.test.tsx` | `6683 ms` / `7111 ms` | `<= 300 ms` / `5937 ms` | `5229 ms` |
| `phase3_adversarial.test.tsx` | `6988 ms` / `7372 ms` | `457 ms` / `5819 ms` | `4955 ms` |
| `landscape_showcase.test.tsx` | `8559 ms` (`F8.2`) / `19704 ms` | `544 ms` (`F8.2`: `2643 ms`) / `17114 ms` | `5319 ms` |
| `phase5_gemini_timeline.test.tsx` | `6466 ms` / `6668 ms` | `<= 300 ms` / `5541 ms` | `5089 ms` |
| `phase4_ui.test.tsx` | `6807 ms` / `7746 ms` | `<= 300 ms` / `6424 ms` | `5244 ms` |
| `phase4_adversarial_crashes.test.tsx` | `6770 ms` / `7612 ms` | `<= 300 ms` / `6275 ms` | `5299 ms` |
| `phase6_challenger_stress.test.tsx` | `6434 ms` / `10601 ms` | `421 ms` (max `2848 ms`) / `8663 ms` | `4884 ms` |
| `phase4_adversarial.test.tsx` | `6759 ms` / `8363 ms` | `351 ms` (max `1219 ms`) / `7500 ms` | `5493 ms` |
| `phase6_adversarial.test.tsx` | `6759 ms` / `8291 ms` | `<= 300 ms` / `6832 ms` | `5183 ms` |
| `phase6_ui.test.tsx` | `6255 ms` / `6724 ms` | `<= 300 ms` / `5625 ms` | `4895 ms` |
| `rwMapQualityTiers.test.tsx` | `8649 ms` / `10299 ms` | `<= 300 ms` (render đầu: `2342 ms`) / `8828 ms` | `4931 ms` |
| `rwMapQuickWins.test.tsx` | `8632 ms` / `10477 ms` | `2440 ms` / `9239 ms` | `4952 ms` |
| `rwMapAutoTier.test.tsx` | `6517 ms` / `6550 ms` | `<= 300 ms` / `5458 ms` | `5157 ms` |
| `v1RealWorldEntities.test.tsx` | `10002 ms` / `10096 ms` | `3443 ms` / `8569 ms` | `4976 ms` |

## 4. Danh sách file được bỏ qua (`2 / 19`) và lý do

1. [`tests/frontend/experimentWorldControl.test.tsx`](../../../tests/frontend/experimentWorldControl.test.tsx) (`479 ms` trên trunk `44507087207`, `229 ms` local): mock toàn bộ `@/components/Landscape/utils/worldCache` (`vi.mock(...)` trả về `mockLoadOrGenerateWorld` / `getMemoizedWorld: null`) và kiểm thử trực tiếp trạng thái đang tải (`worldReady === false` làm nút mở bị vô hiệu hoá trước khi `resolveWorld` giải quyết). Chạy `warmSharedWorld` sẽ không đi qua mock hoặc phá vỡ bài kiểm tra đường dẫn tải nếu bỏ mock.
2. [`tests/frontend/b1ContinuousBodies.test.tsx`](../../../tests/frontend/b1ContinuousBodies.test.tsx) (`391 ms` trên trunk `44507087207`, `45 ms` local): không render `<App />` hay `<WorldShowcase />` (chỉ đọc `WorldShowcase.tsx` bằng `readFileSync` để kiểm tra thuộc tính tĩnh và dùng thế giới test nhỏ `32x32` `'b1-test-world'`), nên không tốn chi phí sinh thế giới `2048x2048`.
