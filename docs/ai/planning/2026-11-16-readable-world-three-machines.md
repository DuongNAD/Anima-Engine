---
title: Thế giới nhìn rõ trên ba máy — chia lane, chia file, thứ tự
status: proposed
owner: maintainers
last_reviewed: 2026-11-16
---

# Thế giới nhìn rõ trên ba máy

Tài liệu này chia [kế hoạch Thế giới nhìn rõ](2026-11-16-feature-readable-world.md) cho ba agent
Antigravity đang chạy trên ba máy của chủ dự án: `anima-mac`, `anima-case` và `anima-dell`.

- **Kênh phối hợp:** hub Google Drive `Anima_Agent_Hub`, lập ngày 2026-11-14. Hub có `AGENT_RULES.md`,
  `WORK_PLAN.md` và wrapper `anima_bridge.py`. Hub nằm trên Drive riêng của chủ dự án, không ở trong repo.
- **Prompt và hàng đợi cho đợt này:** nằm trong hub, ở `PROMPTS_2026-11-16.md`.
- **Tài liệu này giữ phần thuộc về dự án:** ai sở hữu file nào, làm gì trước, ai kiểm bài của ai.

Mọi luật của hub vẫn áp dụng, gồm vòng kiểm bài, điều kiện tự merge và luật L2.

## 1. Lane và máy

| Lane | Agent | Máy | Vì sao máy này |
|---|---|---|---|
| **A — Sinh vật** | `anima-mac` | MacBook Air M5, 32 GB, không quạt | Việc chủ yếu là mã và thiết kế; chỉ cần lệnh Node. Mac không được build Rust (luật L9) |
| **B — Bản đồ và mức chất lượng** | `anima-case` | Máy bàn DESKTOP-3EPV830, i5-14600KF, RTX 5060 Ti | GPU rời: chạy được `npm run capture:views` và là máy tham chiếu cho mức Cao |
| **C — Hiệu năng và đo** | `anima-dell` | Dell Vostro 3530, máy yếu nhất | GPU tích hợp: đúng máy tham chiếu cho mức Thấp. Lane này chỉ có frontend, hợp với luật "không build Rust" của Dell |

## 2. Mỗi file chỉ một lane được sửa

| Lane | Sở hữu |
|---|---|
| A | `src/components/Landscape/LiveAgents.tsx`, `src/components/Landscape/agentInterpolation.ts`, `src/components/Landscape/utils/liveAgentTransform.ts`, `src/PixiViewport.tsx`; file mới `utils/agentPose.ts`, `utils/syntheticTicks.ts`, `utils/tickRecording.ts`; test `tests/frontend/liveAgentTransform.test.ts`, `livingWorldInterpolation.test.ts`, `phase5_pixi_rendering.test.tsx`, và test mới tên `rwCreatures*` |
| B | `src/components/Landscape/WorldShowcase.tsx`, `WorldSky.tsx`, `utils/skyParams.ts`, `utils/sceneClock.ts`, `utils/captureMode.ts`; file mới `utils/qualityTiers.ts`; test `tests/frontend/landscape_showcase*.test.tsx`, `skyWeather.test.tsx`, và test mới tên `rwMap*`; ảnh chuẩn `map-views/*.png` và `map_manifest.json`, chỉ qua `npm run capture:views` |
| C | `src/components/Landscape/WorldVegetation.tsx`, `utils/floraGeometry.ts`, `utils/chunkLod.ts`, `utils/diagnosticsWindow.ts`, `src/window-globals.d.ts`; test `tests/frontend/vegetation.test.tsx`, test mới tên `rwPerf*`, spec mới `tests/e2e/rw_budget.spec.ts` cùng **đúng một** mục khai báo nó trong `scripts/e2e_spec_policy.mjs` (sau khi PR #47 merge); §4 của tài liệu kế hoạch; tài liệu đo mới trong `docs/ai/testing/` kèm một dòng chỉ mục |

Mọi file khác, cả ba lane chỉ được đọc. Hai chỗ chắc chắn phải phối hợp:

- **`WorldShowcase.tsx`** gắn `LiveAgents` (A) và `WorldVegetation` (C). Mọi prop mới hay đổi cách gắn đều
  do B làm, theo `CHANGE REQUEST` (§5).
- **`utils/qualityTiers.ts`** do B viết. A và C chỉ import. B báo kiểu dữ liệu bằng `INTERFACE` (§5) ngay
  khi đã push.

## 3. Thẻ

Mỗi thẻ là một draft PR vào `feat/ecological-selection`. Mã `RWx-y` trỏ tới §5 của kế hoạch.

| Thẻ | Lane | Gói | Bắt đầu khi | Ai kiểm |
|---|---|---|---|---|
| RW-A1 | A | RW0-1, RW0-2, RW0-3 | kế hoạch đã ở thân cây | `anima-case` |
| RW-A2 | A | RW1-4, RW2-1 … RW2-8 | RW-A1 đã merge | `anima-case` |
| RW-A3 | A | RW3-1 … RW3-3, rồi xin **G-A** | RW-A2 đã merge | `anima-case` |
| RW-A4 | A | RW4-7, RW5-1, RW5-2 (và RW3-4 nếu G-A yêu cầu) | G-A đã quyết, RW-B2 đã merge | `anima-case` |
| RW-B1 | B | RW1-1, RW1-2, RW1-3 | kế hoạch đã ở thân cây | `anima-mac` |
| RW-B2 | B | RW4-1, rồi gửi `INTERFACE` | RW-B1 đã merge | `anima-mac` |
| RW-B3 | B | RW4-4, RW4-5 | RW-B2 đã merge | `anima-mac` |
| RW-B4 | B | RW6-2: chụp lại 8 ảnh chuẩn trên RTX 5060 Ti nếu RW-B1 hoặc RW-C3 đổi thứ chúng nhìn thấy | RW-B3 và RW-C3 đã merge | `anima-mac` |
| RW-C1 | C | RW0-4 | kế hoạch đã ở thân cây | `anima-case` |
| RW-C2 | C | RW0-5, rồi xin **G-B** | RW-C1 đã merge | `anima-case` |
| RW-C3 | C | RW4-2, RW4-3 | G-B đã quyết, RW-B2 đã merge | `anima-case` |
| RW-C4 | C | RW4-6 | RW-C3 đã merge và PR #47 đã merge (ở thân cây `test:e2e` còn chỉ chạy `e2e/tiers/`) | `anima-case` |
| RW-C5 | C | RW6-1: gom bảng FPS của ba máy vào một tài liệu `docs/ai/testing/` | RW-A4, RW-B3, RW-C4 đã merge | `anima-case` |

- RW6-3, tức map gate `animal-map-vision`, là việc của chủ dự án.
- Phân công kiểm bài ở đây là **đề xuất**: `anima-case` kiểm A và C vì là máy mạnh nhất; `anima-mac` kiểm
  B và chỉ chạy lệnh Node. Chủ dự án đổi được trong `WORK_PLAN.md`.

## 4. Điều kiện để bắt đầu

1. **Kế hoạch và tài liệu này đã ở thân cây.** Agent đọc chúng bằng `git show origin/feat/ecological-selection:...`.
2. **Luật L2: tối đa ba PR mã mở cùng lúc, tính cho mọi agent.** Ngày 2026-11-16 đang mở năm PR (#46–#50),
   nên ba lane chưa mở được PR nào. Phải merge hoặc đóng bớt trước, hoặc chủ dự án nới L2 cho đợt này.
3. **D-4, bản ghi chuyển động thật cho G-A.** Luật hub cấm mọi agent chạy app desktop. Vì vậy bản ghi
   phải do **chủ dự án** làm một lần, trên máy case: chạy `npm run tauri:dev` rồi mở cảnh với `?record=1`
   (có sau RW-A1). Cách còn lại là mục (b) của D-4, cần dòng uỷ quyền cho `src-tauri/src`.
4. **Hàng đợi được kích hoạt:** chủ dự án dán khối hàng đợi từ `PROMPTS_2026-11-16.md` vào `WORK_PLAN.md`
   của hub. Cho tới lúc đó, các agent không làm gì với đợt này.

## 5. Hai loại tin nhắn thêm cho đợt này

Luật §9 của hub chỉ cho agent làm theo một số loại tin nhất định. Đợt này cần thêm hai loại, ghi trong
khối bổ sung của `PROMPTS_2026-11-16.md`:

| Tiêu đề | Gửi khi | Người nhận làm gì |
|---|---|---|
| `CHANGE REQUEST <thẻ>: <file>` | Cần đổi một file thuộc lane khác | Lane sở hữu làm thay đổi trong thẻ hiện tại hoặc thẻ kế tiếp của mình, rồi trả `DONE <thẻ>` kèm commit. Việc ngoài phạm vi kế hoạch thì chuyển `owner` |
| `INTERFACE <thẻ>: <file>` | Một kiểu dữ liệu hay hàm mà lane khác import vừa được push hoặc đổi | Đọc rồi làm tiếp. Không trả lời |

## 6. Điểm dừng cần chủ dự án

| Điểm dừng | Ai xin | Câu hỏi |
|---|---|---|
| G-A | `anima-mac`, sau RW-A3 | Chuyển động thật có đủ mượt và dễ đọc không? Nếu không, làm RW3-4 trong RW-A4 |
| G-B | `anima-dell`, sau RW-C2 | Trần mức Thấp đo trên Dell có đạt được mà cảnh vẫn nhìn ra không? |
| D-1 … D-5 | bất kỳ lane nào gặp | Các quyết định ở §8 của kế hoạch. Mặc định của D-1 nay cụ thể: Dell Vostro 3530 là máy sàn |

## 7. Điều chưa kiểm

- Chưa có agent nào chạy theo tài liệu này.
- Chưa đo FPS trên Case hay Dell; số ở §2 của kế hoạch chỉ lấy trên MacBook Air M5.
- Dell Vostro 3530 có nhiều cấu hình GPU. Tài liệu này giả định GPU tích hợp, chưa kiểm trên máy thật.
