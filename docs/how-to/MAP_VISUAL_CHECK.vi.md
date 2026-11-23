---
title: Chụp và so sánh bản đồ 3D trước/sau một thay đổi hình ảnh (capture:map, prototype)
status: active
owner: maintainers
last_reviewed: 2026-11-19
review_cycle: per-release
language: vi
translation_of: MAP_VISUAL_CHECK.md
---

# Chụp và so sánh bản đồ 3D trước/sau một thay đổi hình ảnh

**Prototype.** Công cụ để xem lại một thay đổi trong `src/components/Landscape/` (địa hình, nước,
thác, thảm thực vật, bầu trời, sinh vật, vật liệu, LOD) mà không cần người chụp màn hình bằng tay.
Nó **không** phải gate: không có gì đậu hay rớt theo việc bức ảnh trông ra sao.

```powershell
npm run capture:map -- --label before
```

```powershell
npm run capture:map -- --label after --baseline artifacts/map-captures/<thư-mục-lượt-before>
```

Lệnh giống nhau trên PowerShell và zsh. Script (`scripts/capture_map.mjs`):

1. dùng lại Vite dev server trên `127.0.0.1:5173` nếu nó phục vụ **đúng checkout này**, ngược lại tự
   chạy Vite rồi tắt khi xong (cổng bị một worktree khác giữ thì script từ chối và gợi ý `--port`);
2. mở `landscape.html?replay=synthetic&agents=200&segments=5&quality=high` bằng Google Chrome đã cài
   (Playwright `channel: 'chrome'`, không tải gì), chờ thế giới sinh xong (`window.__world` và lưới
   địa hình có trong scene);
3. tính khoảng tám góc nhìn **từ dữ liệu thế giới**: toàn cảnh, đường chân trời, bờ biển gần tâm bản
   đồ, đỉnh cao nhất, thung lũng sông lớn nhất, ô rừng dày nhất, thác cao nhất, cận cảnh một sinh vật
   (bám theo nó). Góc nào không có đối tượng thì ghi `skipped` kèm lý do;
4. mỗi góc: ảnh PNG, FPS trung vị và p5, draw call và tam giác (`window.__animaFrameCounters`); cả
   lượt: lỗi console và lỗi trang, phân loại riêng thông báo thiếu backend Tauri;
5. ghi vào `artifacts/map-captures/<thời-điểm>/` (đã gitignore): ảnh, `report.json`, và `index.html`
   đặt lượt baseline bên cạnh, kèm chênh lệch FPS.

Camera được đặt từ phía script qua hook `window.__worldScene.onBeforeRender` mà trang đã công bố cho
công cụ, nên không có mã nào trong `src/` được thêm cho công cụ này và một baseline chụp được cả trên
commit cũ.

## Đọc kết quả

- FPS mặc định **không khóa vsync** (`--disable-gpu-vsync`), để máy nhanh vẫn thấy được khoảng dư.
  Hai lượt cùng mã trên Mac M5 lệch tới ~25% ở một góc (đo 2026-11-19): chênh lệch FPS nhỏ hơn mức
  đó là nhiễu. Draw call và tam giác là số ổn định, trừ các góc có sinh vật.
- `environment.softwareGl: true` nghĩa là trình duyệt đang vẽ bằng phần mềm; khi đó FPS vô nghĩa.
- Sinh vật là fixture `?replay=synthetic`: dữ liệu thử sinh theo thủ tục, **không phải** đầu ra mô
  phỏng.

## Công cụ này không cho thấy

- App desktop thật (Tauri webview, có mô phỏng chạy phía sau).
- Hiệu năng trên máy đích Iris Xe: FPS ở đây là GPU của máy đang chạy.
- Ảnh chuẩn trùng từng byte: đó là `npm run capture:views`, công cụ khác.

Hướng dẫn cho phiên Claude Code nằm ở skill
[`.claude/skills/map-visual-check/SKILL.md`](../../.claude/skills/map-visual-check/SKILL.md).
