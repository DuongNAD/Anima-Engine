---
phase: planning
feature: real-world-and-bodies
kind: thẻ
title: Thẻ F050 — đưa bước khởi tạo thế giới dùng chung vào beforeAll của Vitest
description: Chuyển chi phí khởi tạo thế giới 2048x2048 lần đầu trong 17 file test frontend sang hook beforeAll có ngân sách 60s để mọi test đầu tiên trên CI dưới 5s mà không nới testTimeout.
status: active
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

> **Ghi chú đo CI:** Cột **Trước (CI trunk `44507087207`)** ghi nhận thời lượng test chậm đầu tiên chứa chi phí khởi tạo thế giới (và tổng thời lượng file). Cột **Sau** và **`beforeAll`** đang chờ lượt chạy CI của Draft PR.

| File (`tests/frontend/...`) | Trước — CI trunk `44507087207` (test / file) | Sau — CI PR (test đầu tiên / test render đầu tiên) | `beforeAll` (`warmSharedWorld`) |
|---|---|---|---|
| `b1HudSyntheticLabel.test.tsx` | `9787 ms` / `9789 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `dashboard_grid.test.tsx` | `6694 ms` / `7173 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `leak_proof.test.tsx` | `6643 ms` / `6827 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `phase3_ui.test.tsx` | `6683 ms` / `7111 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `phase3_adversarial.test.tsx` | `6988 ms` / `7372 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `landscape_showcase.test.tsx` | `8559 ms` (`F8.2`) / `19704 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `phase5_gemini_timeline.test.tsx` | `6466 ms` / `6668 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `phase4_ui.test.tsx` | `6807 ms` / `7746 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `phase4_adversarial_crashes.test.tsx` | `6770 ms` / `7612 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `phase6_challenger_stress.test.tsx` | `6434 ms` / `10601 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `phase4_adversarial.test.tsx` | `6759 ms` / `8363 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `phase6_adversarial.test.tsx` | `6759 ms` / `8291 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `phase6_ui.test.tsx` | `6255 ms` / `6724 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `rwMapQualityTiers.test.tsx` | `8649 ms` / `10299 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `rwMapQuickWins.test.tsx` | `8632 ms` / `10477 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `rwMapAutoTier.test.tsx` | `6517 ms` / `6550 ms` | _chờ CI PR_ | _chờ CI PR_ |
| `v1RealWorldEntities.test.tsx` | `10002 ms` / `10096 ms` | _chờ CI PR_ | _chờ CI PR_ |

## 4. Danh sách file được bỏ qua (`2 / 19`) và lý do

1. [`tests/frontend/experimentWorldControl.test.tsx`](../../../tests/frontend/experimentWorldControl.test.tsx) (`479 ms` trên trunk `44507087207`, `229 ms` local): mock toàn bộ `@/components/Landscape/utils/worldCache` (`vi.mock(...)` trả về `mockLoadOrGenerateWorld` / `getMemoizedWorld: null`) và kiểm thử trực tiếp trạng thái đang tải (`worldReady === false` làm nút mở bị vô hiệu hoá trước khi `resolveWorld` giải quyết). Chạy `warmSharedWorld` sẽ không đi qua mock hoặc phá vỡ bài kiểm tra đường dẫn tải nếu bỏ mock.
2. [`tests/frontend/b1ContinuousBodies.test.tsx`](../../../tests/frontend/b1ContinuousBodies.test.tsx) (`391 ms` trên trunk `44507087207`, `45 ms` local): không render `<App />` hay `<WorldShowcase />` (chỉ đọc `WorldShowcase.tsx` bằng `readFileSync` để kiểm tra thuộc tính tĩnh và dùng thế giới test nhỏ `32x32` `'b1-test-world'`), nên không tốn chi phí sinh thế giới `2048x2048`.
