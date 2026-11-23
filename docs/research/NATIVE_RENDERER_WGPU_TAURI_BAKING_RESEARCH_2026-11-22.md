---
title: Kiến trúc Renderer Native wgpu + Tauri v2, Đường ống 30 FPS Intel Iris Xe và Baking Offline OxideSwarm
status: proposed
owner: architecture
last_reviewed: 2026-11-22
review_cycle: quarterly
---

# Kiến trúc Renderer Native wgpu + Tauri v2, Đường ống 30 FPS Intel Iris Xe và Baking Offline OxideSwarm

> **Phạm vi & tính chất tài liệu.** Tài liệu này là **tổng hợp nghiên cứu ngoài kết hợp đối chiếu mã nguồn Anima Engine** làm đầu vào cho **spike renderer Map v2 Pha 2 (`NR-SPIKE`)**, tức bước đo trước khi viết `ADR-0011`, và cho thẻ nướng tài nguyên **`WV2-5`** (chạy trên grid worker **OxideSwarm**, `QĐ-OXS`). Đây **không** phải hợp đồng kiến trúc đã duyệt hay số đo hiệu năng chính thức trên phần cứng của dự án; mọi bằng chứng đo được hiện hành nằm duy nhất tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
>
> **`ADR-0011` chưa tồn tại** (thư mục [`docs/decisions/`](../decisions/README.md) hiện dừng ở `ADR-0010`). Theo [`2026-11-20-feature-map-v2.md`](../ai/requirements/2026-11-20-feature-map-v2.md) (bảng pha, dòng P2), spike phải đo **cả ba ứng viên** — (a) `wgpu` tự viết trong cửa sổ Tauri, (b) Bevy mới kèm ước lượng chi phí nâng mô phỏng, (c) Godot 4 làm viewer riêng — trên **cùng một cảnh bắt buộc** (địa hình World v2, nước, 10.000 cây, 1.000 sinh vật), và **mã spike không merge**, chỉ merge ghi chú kết quả. Tài liệu này chỉ đào sâu ứng viên (a); mọi chữ "khuyến nghị", "mặc định" hay "loại bỏ" bên dưới là **giả thuyết để spike kiểm**, không phải lựa chọn của `ADR-0011`.
>
> **Cảnh báo phân loại bằng chứng (`2026-11-22`).** Các ước lượng ngân sách khung hình trong tài liệu này (tổng 33,33 ms / mục tiêu bền nhiệt 26,7 ms, ~2,8 ms TAAU, ~4,2 ms bóng CSM, ~4,3 MB/frame địa hình) và một số ngưỡng đề xuất cho Cổng 2 đa kiến trúc trong báo cáo gốc được suy dẫn từ **mô hình phân tích phần cứng ngoài** mà nguồn gốc (nguồn nội bộ chưa công bố liên kết) đã bị loại khỏi mục tham khảo, nên **không truy được nguồn — (chưa xác minh)**. Ngân sách khởi điểm duy nhất của dự án là bảng "Ngân sách khung hình" trong [`2026-11-20-feature-map-v2.md`](../ai/requirements/2026-11-20-feature-map-v2.md) (§3b: Iris Xe mức Thấp 33,3 ms vẽ 720p rồi phóng lên, chia theo hệ thống; P2 hiệu chỉnh lại bằng số đo); khi hai nguồn lệch nhau, bảng đó thắng. Lưu ý thêm: yêu cầu ghi mức Thấp là "ánh sáng và bóng nướng sẵn, … không có shadow map", trong khi Mục 2.1 dưới đây giả định bóng CSM 2–3 tầng — đây là điểm lệch cần spike xác nhận. Do chưa có benchmark WGSL công khai đo trực tiếp trên Intel Iris Xe 96 EU trong `wgpu 30` cho cùng cảnh 3D, mọi số liệu ngân sách ở Mục 2 và ngưỡng drift ở Mục 3.4 mang nhãn **`Mô hình ngoài / chưa đo trên Anima`** và bắt buộc hiệu chỉnh trên phần cứng thật ở Mốc 2–3 của `NR-SPIKE`.
>
> **Chất lượng nguồn.** Một số nguồn chịu tải chính là **bằng chứng yếu**: repo rất ít người theo dõi (`mihazs/lazysh` [2] — 2 sao, GPL-3.0; `Azazel-Labs/mipple` [112] — 1 sao; `stevepryde/sp-fidelity` [97] — 1 sao), issue lẻ của dự án nhỏ (`rusty#1848` [95]), issue do AI soạn (`EffectCraft#613` [58] mang chân "Generated with Claude Code"), và trang "skill" cho agent thay vì tài liệu kỹ thuật gốc ([107], [110], [9], [135]). Kết luận dựa chủ yếu trên các nguồn này được gắn **(nguồn yếu)** tại chỗ.

---

## 0. Đối chiếu mã nguồn Anima Engine & ba điểm hợp nhất (2026-11-22)

1. **Đối chiếu các crate trong [`Cargo.lock`](../../src-tauri/Cargo.lock) (`Đã kiểm`):**
   - `tauri 2.11.5`, `tao 0.35.3`, **`wry 0.55.1`**, **`wgpu 30.0.2`**, **`libm 0.2.16`**, `raw-window-handle 0.6.2`.
   - **`wgpu 30.0.2` chỉ có mặt như phụ thuộc bắc cầu của burn:** `burn 0.22.1` → `cubecl-wgpu 0.11.4` → `wgpu`, và chỉ được biên dịch dưới feature `ml-wgpu` (thuộc `desktop`). Cổng G2 của CI ([`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) dòng 140 và 143) **từ chối** `wgpu`, `zstd-sys`, `meshopt`, `bc7enc`, `ktx2` (cùng `naga`, `cubecl-wgpu`, …) trong bản build mặc định của `anima-engine`. Vì vậy "đã có `wgpu 30` trong lockfile" **không** có nghĩa là renderer dùng được ngay: một renderer native cần **feature gate riêng** (không lọt vào build mặc định) và phải **giữ cùng phiên bản `wgpu` với burn** để resolver không dựng hai cây `wgpu` (thiết kế World v2 đã đặt đúng ràng buộc này cho `gpu-bake`). Mỗi lần nâng burn sẽ kéo theo renderer và ngược lại.
   - **`zstd`:** `zstd 0.11.2+zstd.1.5.2` chỉ nằm trong lockfile, được kéo qua `zip` bởi `tch`/`torch-sys` (phụ thuộc tuỳ chọn của burn mà không feature nào bật); `cargo tree --features desktop -e normal -i zstd` không in gì, tức **không có zstd nào được biên dịch** trong app. Bản thư viện C trong lockfile lại là `zstd-sys 2.0.16+zstd.1.5.7`. Anima **chưa ghim** phiên bản zstd nào; chọn và ghim một phiên bản cho `WV2-5` là quyết định tương lai.
   - **Về [15] (`Suy luận`):** Anima dùng đúng `wry 0.55.1`, phiên bản được nêu trong báo cáo lỗi hồi quy WebView2 Evergreen 149.0.4022.98 (ô compositor 256 px bị cũ sau thao tác zoom khi nội dung `<img>`/`<canvas>` đổi mà kích thước layout không đổi). **Chưa ai chứng minh Anima gặp lỗi này**; nó chỉ là một lý do cần đo, không phải căn cứ cho quyết định tách lớp 3D xuống bề mặt native.
   - API `wgpu 30.0.2` liên quan có thật trong mã nguồn crate: enum 7 trạng thái `CurrentSurfaceTexture`, `SurfaceTargetUnsafe::CompositionVisual` / `CoreAnimationLayer`, `Dx12SwapchainKind::{DxgiFromHwnd, DxgiFromVisual}`. Issue `wgpu#9003` [45] có tiêu đề "Crash on nvidia cards since wgpu 28"; bản sửa liên kết với nó ("Remove hold of snatch write lock during present") đụng tới `present`, nhưng việc nó đã có trong `wgpu 29` là **(chưa xác minh)**.
2. **Hợp nhất 1 — Đóng gói KTX2 giữa [`2026-11-20-world-v2-design.md` §3.2](../ai/design/2026-11-20-world-v2-design.md) và `WV2-5` (`Đã kiểm` + `Suy luận`):**
   - **Kế hoạch hiện hành (thiết kế `WV2-D`, đã chấp nhận):** đầu ra của `WV2-5` phục vụ viewer Three.js WebGL2 của Pha 1 (`WV2-6`), nên §3.2 giữ **Basis/UASTC trong vỏ KTX2** vì macOS WKWebView chưa xác minh hỗ trợ `EXT_texture_compression_bptc` / `EXT_texture_compression_rgtc`; bộ mã hoá BC đã chọn là `bc7enc_rdo`; cổng DoD của `WV2-5` là `Block PSNR >= 42 dB, SSIM >= 0.996` (thiết kế, thẻ `WV2-5`, dòng 553); và mọi job grid chạy trên **một worker duy nhất** (`--worker desktop-3epv830`, thiết kế §6 dòng 421, `QĐ-OXS`).
   - **Đề xuất cho sau Pha 2 (`Suy luận`, cần sửa thiết kế):** nếu renderer native (`wgpu` trên DX12 & Metal) được `ADR-0011` chọn, cả hai backend giải mã phần cứng **BC7 và BC5**; khi đó một gói **Native BC7/BC5 trong KTX2 + Zstd** cho bản desktop sẽ bỏ được bước transcode trên CPU. Đây **không** phải kế hoạch hiện tại của `WV2-5`; áp dụng nó đòi hỏi một vòng sửa thiết kế `WV2-D`.
3. **Hợp nhất 2 — Tất định toán học: `libm 0.2.16` hiện có vs `alice-det-math` (`Đã kiểm` + `Suy luận`):**
   - [`anima-worldgen/src/lib.rs`](../../src-tauri/crates/anima-worldgen/src/lib.rs) đã sử dụng `libm 0.2.16` kèm cấu hình `clippy` cấm các hàm toán học `std` phi tất định.
   - Do đó, **không tự động thay bằng `alice-det-math`** khi chưa đi qua cổng đánh giá thư viện [`OPEN_SOURCE_POLICY.md`](../governance/OPEN_SOURCE_POLICY.md) và chưa kiểm chứng xem `libm 0.2.16` trên toàn cụm worker `arm64` ↔ `x86_64` có đạt 100% SHA-256 bit-exact cho heightfield / LUT hay chưa.
4. **Liên kết yêu cầu:** Các chỉ tiêu mục tiêu của Map v2 nằm tại [`2026-11-20-feature-map-v2.md`](../ai/requirements/2026-11-20-feature-map-v2.md) và thiết kế phân tầng hiện hành tại [`2026-11-20-world-v2-design.md`](../ai/design/2026-11-20-world-v2-design.md).

---

## 1. Tích hợp `wgpu 30` + `raw-window-handle` trong Tauri v2 (Windows DX12 & macOS Metal)

### 1.1. Kiến trúc lớp bề mặt, xử lý trong suốt và chống nhấp nháy compositor (`Nguồn ngoài`)

Phương án dựng `wgpu` không màn hình (*headless*) rồi đọc ngược buffer đẩy qua IPC vào `<canvas>` gây độ trễ ~300 ms trên máy yếu và gãy luồng sự kiện chuột [13], đồng thời WebView2 149 có báo cáo lỗi ô compositor 256 px bị cũ trên `Tauri 2 / wry 0.55.1` [15] (chưa kiểm Anima có gặp hay không). Kiến trúc được các nguồn dưới đây mô tả là đặt bề mặt dựng hình `wgpu` **nằm bên dưới** lớp WebView trong suốt làm HUD. Tuy nhiên, gắn swapchain `wgpu` trực tiếp lên cùng `HWND` của WebView2 sẽ gây tranh chấp `Present` và nhấp nháy nặng (`tauri#9220`) [13]. Phải tách bề mặt theo compositor bản địa từng hệ điều hành:

- **Windows (DX12 + WebView2) — hai đường tích hợp:**
  1. *Đường thực dụng tương thích `wry` (0.55–0.57) — (nguồn yếu: chủ yếu dựa trên [2], một repo 2 sao):* Giữ cửa sổ cấp cao ở trạng thái không trong suốt (*opaque*), tạo **một `child HWND` riêng** cho `wgpu` nằm dưới `WRY_WEBVIEW` theo thứ tự Z và **bỏ cờ `WS_CLIPSIBLINGS`**, đồng thời cấu hình `DefaultBackgroundColor` của WebView2 với alpha đúng bằng `0` kết hợp CSS `background: transparent` [2]. Cảnh giác bẫy `WS_CLIPCHILDREN` trên cửa sổ cha làm cắt nền trong suốt thành màu đen đặc (`tauri#12450`) [21]. Trình chiếu qua `Dx12SwapchainKind::DxgiFromHwnd` giữ tương thích đầy đủ với **RenderDoc** (ngược với `DxgiFromVisual`) [23]. (Cần kiểm chứng trên phần cứng thật: *per-monitor DPI*, *TSF IME* và GPU reset vì bằng chứng `lazysh` mới kiểm qua CI `windows-latest` [2]).
  2. *Đường triệt để (DirectComposition Visual Tree tự quản lý):* Bật `no_redirection_bitmap` (`WS_EX_NOREDIRECTIONBITMAP`, bổ sung qua PR `#15410` thay thế thủ thuật cũ `DwmEnableBlurBehindWindow(0,0,-1,-1)` hay lỗi trên Windows 10), tự tạo `IDCompositionDevice`, `IDCompositionTarget` và hai visual xếp chồng: visual đáy truyền qua `wgpu::SurfaceTargetUnsafe::CompositionVisual(content)`, visual đỉnh gắn WebView2 qua `ICoreWebView2CompositionController::put_RootVisualTarget` [23, 27]. Đổi lại phải tự duy trì lớp bọc Win32/COM cho `ICoreWebView2CompositionController` (vì `wry` chưa hỗ trợ sẵn) và lưu ý phản hồi Microsoft Q&A rằng `WS_EX_NOREDIRECTIONBITMAP` có thể làm viền cửa sổ tiêu chuẩn Windows 11 dày thêm 1 px [29].
- **macOS (Metal + WKWebView):**
  `raw-window-handle` 0.6 dùng `NSView` trong `AppKitWindowHandle` [30], cho phép chèn sibling `NSView` gắn `CAMetalLayer` xuống bên dưới `WKWebView` bằng `addSubview:positioned:relativeTo:` [2] (hoặc truyền thẳng `SurfaceTargetUnsafe::CoreAnimationLayer`). Bắt buộc đặt khoá KVC riêng tư `drawsBackground = NO` trên `WKWebView` để lộ lớp Metal phía sau [34]. **Giới hạn phát hành (`Nguồn ngoài`):** Do `drawsBackground` là API riêng tư của WebKit, giải pháp này dùng được cho bản ký **Developer ID** phân phối trực tiếp nhưng **không thể nộp lên Mac App Store** [2, 8]. Cần chọn đường dựng GPU ngay lúc tạo cửa sổ, tránh chuyển nóng giữa `drawRect` phần mềm và `CAMetalLayer` [35].

### 1.2. Hit-testing sự kiện chuột xuyên HUD không gây lỗi ghép hình

Không thể subclass `WM_NCHITTEST` từ tiến trình chính trên WebView2 Windows vì `Chrome_RenderWidgetHostHWND` chạy trong tiến trình riêng `msedgewebview2.exe` [7]. Đồng thời, bật/tắt `set_ignore_cursor_events` liên tục ép `SetWindowLongPtr` đổi `WS_EX_TRANSPARENT | WS_EX_LAYERED`, khiến DWM dựng lại cây ghép hình và là nguyên nhân nghi ngờ gây race đen màn hình trên Windows 10 (`tauri#15947`) [1].

*Cột cuối là giả thuyết để `NR-SPIKE` kiểm cho ứng viên (a), không phải lựa chọn của `ADR-0011` (chưa tồn tại).*

| Phương án định tuyến chuột | Cơ chế Windows | Cơ chế macOS | Độ trễ điều khiển 3D | Tác động DWM / IME / Bảo trì | Giả thuyết cho spike (`Suy luận`) |
|---|---|---|---|---|---|
| **A. DOM-First Event Forwarding** | `pointer-events: none` nền DOM, `auto` ở HUD; gom lô sự kiện vùng trống sang Rust qua *web message* [2, 40] | Dùng chung mã JS với Windows trên `WKWebView` [2] | Đi qua JS + IPC; cần gom lô theo khung hình | **An toàn nhất:** giữ nguyên 100% TSF/IME tiếng Việt, không đụng `WS_EX_LAYERED` | **Phương án thử trước trong `NR-SPIKE`** (giả thuyết) — ít mã nền tảng |
| **B. Native Host Pointer Router** | `ICoreWebView2CompositionController`: `WndProc` tra bảng rect HUD từ DOM, trúng HUD mới gọi `SendMouseInput`/`SendPointerInput` [27] | Override `NSView::hitTest(_:)` trả về `nil` ngoài bảng rect HUD để rơi thẳng xuống Metal `NSView` [39, 42] | **Thấp nhất:** kéo/cuộn/picking 3D đi trực tiếp bằng Rust không chờ JS | Trung bình: phải tự chuyển tiếp con trỏ, bánh xe, phím tắt và test TSF/IME | **Nâng cấp cho Pha 2B** khi cần độ nhạy camera tối đa |
| **C. Toggling `set_ignore_cursor_events` / `SetWindowRgn`** | Bật/tắt `WS_EX_TRANSPARENT \| WS_EX_LAYERED` hoặc `SetWindowRgn` [1] | Bật/tắt `ignoresMouseEvents` [44] | Thấp khi click-through | Rủi ro cao: nghi ngờ gây đen màn hình Win10 (`#15947`), lệch đồng bộ DOM | **Ứng viên yếu nhất** — giả thuyết là loại; spike chỉ cần ghi lý do |

### 1.3. Vòng đời Surface không panic, Resize và Đa màn hình DPI (`wgpu 30`)

`Surface::configure` trong `wgpu 30` **panic** ngay khi `(width == 0 || height == 0)`, còn `SurfaceTexture` cũ chưa drop, sai định dạng hoặc sai không gian màu [32]. Luôn xử lý đầy đủ 7 nhánh của enum `CurrentSurfaceTexture` (có từ `wgpu 29–30`) [47]:

| Trạng thái `CurrentSurfaceTexture` | Nguyên nhân điển hình trong Tauri v2 | Quy trình phục hồi an toàn không panic |
|---|---|---|
| `Success(tex)` | Lấy texture khung hình bình thường | Vẽ khung hình và gọi `tex.present()` |
| `Suboptimal(tex)` | Đổi DPI, đang kéo giãn cửa sổ | Trình chiếu hoặc drop `tex` ngay; đánh dấu cấu hình lại ở đầu frame kế (chỉ gọi `surface.configure` khi không còn `SurfaceTexture` sống) [32] |
| `Timeout` | Compositor/GPU bận nhất thời | Bỏ qua frame hiện tại, thử lại chu kỳ sau |
| `Occluded` | Thu nhỏ cửa sổ hoặc bị che kín | Tạm dừng dựng hình để tiết kiệm điện & nhiệt cho iGPU |
| `Outdated` | Kích thước vật lý hoặc thuộc tính bề mặt đổi | Kiểm tra `w > 0 && h > 0`, gọi `surface.configure` rồi thử lại |
| `Lost` | Mất bề mặt hoặc TDR/driver reset (`DXGI_ERROR_DEVICE_REMOVED`) | Kiểm tra `set_device_lost_callback`: mất device → dựng lại `Device` & GPU resources; mất surface → `create_surface()` + `configure()` |
| `Validation` | Lỗi kiểm hợp lệ trong error scope / `on_uncaptured_error` | Ghi log đầy đủ và kích hoạt nhánh hồi phục an toàn |

**Bốn quy tắc vận hành resize/DPI & khởi tạo:**
1. *Chống deadlock & lỗi handle WebView2:* Không tạo cửa sổ / `Window::add_child` bên trong command đồng bộ trên Windows [18]; tránh khởi tạo controller WebView2 đầu tiên trên cửa sổ chưa từng hiện (`.visible(false)` gặp lỗi `ERROR_INVALID_WINDOW_HANDLE 0x80070578` trên WebView2 155.0 [20]) — tạo hiển thị rồi ẩn ngay qua API của `tao` để giữ cờ nội bộ đồng bộ.
2. *Chống giật live-resize Windows (DX12):* Áp dụng kinh nghiệm `rune#47` [26]: vẽ đồng bộ ngay trong nhánh `Resized` ở frame-latency `1` với `PresentMode::Immediate`, giữ swapchain cỡ màn hình trong lúc kéo và chỉ đổi viewport. Nếu dùng `CompositionVisual`, dịch ngược offset trong `WM_SIZING` (`SetOffsetX(+dx)` / `SetOffsetY(+dy)` + `Commit()`) rồi reset về `0` khi frame mới trình chiếu.
3. *Đồng bộ DPI Windows (`PerMonitorV2`):* Tauri đặt `SetProcessDpiAwarenessContext(-4)`; khi nhận `WM_DPICHANGED` / `ScaleFactorChanged`, cập nhật kích thước vật lý `wgpu`, giữ toạ độ logic cho DOM, đồng bộ `SetBounds` WebView2 & `SetWindowPos` child HWND kèm hậu kiểm sau khi khôi phục từ minimize [51].
4. *Chống rung/giãn live-resize macOS (Zed GPUI + `metal-live-resize`):* Đặt `contentsGravity = kCAGravityTopLeft` và `contentsScale = window.backingScaleFactor` trên `CAMetalLayer`, luôn đọc kích thước thực từ drawable texture mỗi frame [54]; **chỉ bật động `presentsWithTransaction = true`** lúc khởi tạo và khi đang kéo giãn (Zed chứng minh bật thường trực khoá ProMotion 120 Hz xuống 60 FPS), trả về bất đồng bộ với triple-buffering khi tương tác bình thường [55, 57].

### 1.4. Phòng thủ biên dịch Shader (DXC vs FXC) & Watchdog TDR trên DX12 / iGPU

Kinh nghiệm từ các dự án Rust + `wgpu` (`EffectCraft#613`, `LightCraft#2936/#471/#78`; phiên bản `wgpu` của từng dự án chưa kiểm) gợi ý ba kỷ luật [58, 59]. *(Nguồn yếu: đây là issue của dự án nhỏ, `EffectCraft#613` do AI soạn; các con số 12/224 kernel, 2,3 s / 7,2 s lấy từ phần bình luận của issue, chưa đo trên Anima.)* Phần có kiểm trong mã nguồn `wgpu-types 30.0.2`: DXC tối thiểu `v1.8.2502`; `static-dxc` không bật được trên `aarch64` Windows.
- **Bắt buộc dùng DXC thay cho FXC:** FXC (`/T cs_5_1 /Ges`) hỏng 12/224 compute kernel của `EffectCraft` (lỗi `X3500`, `X3507`, `X3511`), biên dịch tuần tự chậm (trung vị 2,3 giây, lên tới 7,2 giây với `cache: None` ở lần chạy đầu trên Core Ultra 7 165U), và cả `Features::SHADER_F16` (SM 6.2+) lẫn `Features::SUBGROUP` trên DX12 đều **bắt buộc dùng DXC** [46, 58].
- **Không dùng `Dx12Compiler::Auto` thả nổi ngoài `PATH`:** Có thể nạp nhầm `dxcompiler.dll` của app khác thiếu `dxil.dll` ký mã, gây sập `Device(Lost)` khi khởi động (`LightCraft#471`). Hãy đóng gói tường minh cặp `dxcompiler.dll` + `dxil.dll` (`>= v1.8.2502`) cạnh binary (hoặc bật `static-dxc` trên `x86_64-pc-windows`; lưu ý `static-dxc` chưa hỗ trợ `aarch64-pc-windows`), truyền qua `Dx12BackendOptions`, biên dịch pipeline song song nhiều luồng và lưu `PipelineCache` [58, 61].
- **Chia nhỏ Compute Submit để tránh WDDM TDR / Linux i915 Hang Check:** Không gộp tác vụ compute nặng nhiều giây vào một submit duy nhất trên iGPU dùng chung với desktop; chia nhiều gói ngắn và bọc trong `error_scope` + `set_device_lost_callback` [59].

---

## 2. Đường ống đồ hoạ nhẹ đạt 30 FPS trên Intel Iris Xe

### 2.1. Hồ sơ phần cứng Iris Xe & nguyên tắc ngân sách (`Mô hình ngoài / chưa đo trên Anima`)

Intel Iris Xe G7 (`96 EU`, Xe-LP Gen 12.1/12.2) có **768 ALU**, 48 TMU, 24 ROP, `1 MB L2` + `3,75 MB GPU L3`, xung `950–1550 MHz` (~1,69 TFLOPS FP32 ở 15 W / tới ~2,4 TFLOPS boost), bản rút gọn `80 EU` có 640 ALU / 40 TMU / 20 ROP [5, 62, 64]. Lợi thế lớn nhất là **thông lượng FP16 gấp đôi FP32 (2:1, ~3,38–4,0 TFLOPS)**. Điểm nghẽn thực sự là **băng thông RAM chia sẻ với CPU/OS/WebView2** (chênh ~3,2× giữa `DDR4-3200 single-channel 25,6 GB/s`, `DDR4 dual 51,2 GB/s`, `LPDDR4x-4267 68,3 GB/s`, `LPDDR5-5200 83,2 GB/s`) và **giảm xung nhiệt ở trần 15 W TDP** [5].

*Các số ms trong mục này không truy được nguồn (chưa xác minh); ngân sách khởi điểm của dự án là bảng §3b trong [`2026-11-20-feature-map-v2.md`](../ai/requirements/2026-11-20-feature-map-v2.md), nơi mức Thấp của Iris Xe không có shadow map.* Do đổ bóng (CSM 2–3 tầng, mô hình ước tính ~4,2 ms — chỉ áp dụng nếu mức Thấp có shadow map, trái với yêu cầu hiện hành) và ghép nền DWM + WebView2 1080p (ước tính ~2,0 ms) gần như bất biến theo độ phân giải 3D nội bộ, hai đòn bẩy chính để đạt mục tiêu bền nhiệt **$\le 26{,}7$ ms** (dự phòng 20% dưới trần `33,33 ms` cho giảm xung nhiệt) là:
1. Hạ độ phân giải dựng hình nội bộ xuống **720p** (`0,67×` = 44,4% số pixel của 1080p) kèm **Dynamic Resolution Scaling (DRS)** tự hạ xuống `0,59×` (635p) hoặc `0,50×` (540p) trên máy OEM RAM đơn kênh (~454,4 MB/frame giới hạn trong mô hình phân tích).
2. Bật có điều kiện `Features::SHADER_F16` trong WGSL (chạy trên cả Metal và DX12 qua DXC SM 6.2+) cho pha trộn vật liệu, sương mù và hậu kỳ để khai thác thông lượng 2:1 và giảm áp lực thanh ghi EU [60].

### 2.2. Bầu trời & khí quyển vật lý theo Hillaire (EGSR 2020) [71, 72]

Thay vì raymarch tán xạ từng điểm ảnh (~1,6 ms ở 1M px [70]), dùng 4 LUT nhỏ của Hillaire (2020) tổng dung lượng `< 0,55 MB` (`~0,27 MB` nếu nén `Rg11b10Ufloat` cho 3 bảng 2D và `32×32×16` froxel; ước tính mô hình ngoài ~0,5 ms tổng cộng trên Iris Xe):

| Bảng tra cứu (LUT) | Kích thước tham chiếu | Định dạng `wgpu` | Dung lượng VRAM | Triển khai & tần suất cập nhật |
|---|---|---|---|---|
| **Transmittance LUT** | `256 × 64` | `Rg11b10Ufloat` / `Rgba16Float` | `64 KB / 128 KB` | Fragment RTT (~40 bước, tham số hoá Bruneton); chỉ cập nhật khi đổi thông số khí quyển |
| **Multi-Scattering LUT** | `32 × 32` | `Rg11b10Ufloat` / `Rgba16Float` | `4 KB / 8 KB` | Fragment RTT hoặc Compute (64 hướng × 20 bước, hệ số $f_{\text{ms}} = 1/(1 - F_{\text{ms}})$); chỉ khi đổi khí quyển |
| **Sky-View LUT** | `192 × 108` | `Rg11b10Ufloat` / `Rgba16Float` | `81 KB / 162 KB` | Fragment RTT (~30 bước phi tuyến dồn gần chân trời); cập nhật khi góc mặt trời / cao độ camera đổi |
| **Aerial Perspective Froxel** | `32 × 32 × 16` (nhẹ cho Iris Xe) / `32³` | **Bắt buộc `Rgba16Float`** | `128 KB` / `256 KB` | Compute pass ghi 3D StorageTexture **mỗi frame**, tích luỹ $L_{\text{out}} = L_{\text{in}}\cdot T + L_{\text{scat}}$ |

*Lưu ý `wgpu`:* Render target `Rg11b10Ufloat` cần `Features::RG11B10UFLOAT_RENDERABLE` [60] (nên Fragment RTT cho 3 bảng 2D tối đa tương thích). Tắt mặc định mây thể tích raymarch 64 bước trên Iris Xe [72].

### 2.3. Thảm thực vật hướng GPU: Two-Phase Hi-Z, Fixed-Bucket MDI & Hemi-Octahedral Impostors

1. **Two-Phase Hi-Z Occlusion Culling (tránh sót hình 0 frame) [75, 77]:**
   - *Pha 1:* Compute cull frustum + Hi-Z frame trước → Depth Prepass 1.
   - *Early Hi-Z:* Dựng chuỗi mip bằng **WGSL Single Pass Downsampler (SPD)** trong workgroup shared memory (1–2 dispatch thay vì chuỗi barrier từng mip).
   - *Pha 2:* Chỉ re-test nhóm bị Pha 1 loại với Early Hi-Z mới dựng → vẽ bổ sung ở Depth Prepass 2 (0 frame lag) → dựng Final Hi-Z.
2. **Xử lý phân kỳ Metal vs DX12 cho Multi-Draw Indirect [46, 76, 80]:**
   Trong `wgpu 30`, multi-draw-indirect (MDI, `multi_draw_indexed_indirect`) là hàm lõi, không còn cờ feature riêng; `wgpu-types 30.0.2` chỉ có `Features::MULTI_DRAW_INDIRECT_COUNT` (MDIC), và DX12/Vulkan hỗ trợ MDIC. Trên **Metal** trong `wgpu-hal 30.0.2` (bản đang ghim, `Đã kiểm` trong mã nguồn crate): MDIC chưa được triển khai (`draw_indirect_count` / `draw_indexed_indirect_count` là stub `//TODO`), còn MDI được thực hiện bằng **vòng lặp phía CPU**, mỗi lần một lệnh indirect đơn. Việc chuyển MDI sang **GPU Indirect Command Buffers (ICBs)** (khi số lệnh `>= 512`, tắt nếu bật `STRICT_WEBGPU_COMPLIANCE`) đến từ `gfx-rs/wgpu#9640`, merge upstream ngày 2026-11-05 nhưng **chưa có trong bản phát hành `30.0.2`** [46]. Cách thống nhất sạch cho cả hai OS là gộp tất cả mesh thực vật vào **Global Vertex/Index Megabuffer**, chia sẵn một mảng **cố định các xô `(Species, LOD)`** (ví dụ 16–32 loài × 3 LOD = 48–96 xô); compute shader `atomicAdd` vào `instance_count` của đúng xô và phía CPU chỉ phát đúng số lượng xô cố định bằng `multi_draw_indexed_indirect` (xô trống `instance_count = 0` GPU tự bỏ qua).
3. **Hemi-Octahedral Impostors & Hashed Alpha Testing [84, 85, 88]:**
   - Chiếu bán cầu trên xuống `[-1, 1]²` (xoay 45°, chuẩn $L_1$, phản xạ nửa dưới, xoay ngược), nướng lưới `8×8` (`1024²`) hoặc `12×12` cắt gọn vùng trống, tạo **coverage-weighted mipmaps** và loại texel `coverage = 0` khỏi phép khớp endpoint BC (kỹ thuật `SmartImposter` [85]). Với **BC7 Albedo+Alpha + BC5 Octahedral Normal + BC4 Depth/Coverage** (`2,5 B/texel` × `1,333×` mip chain), mỗi loài `1024²` chỉ tốn **`3,33 MB`** (`53,3 MB` cho 16 loài vs `682,6 MB` nếu dùng `2048²` RGBA8 không nén).
   - Thay thế 4× MSAA + Alpha-to-Coverage (quá đắt băng thông và xung đột temporal [86, 87]) bằng: lõi tán cây đặc tận dụng Early-Z + viền lá dùng **Hashed Alpha Testing** trong không gian vật thể 3D (Wyman & McGuire [88, 89]), và **Stochastic Single-View Selection** cho impostor khi đi kèm TAAU (giảm 3× texture fetch so với nội suy barycentric 3 góc nhìn [85]).

### 2.4. Địa hình: Weight-Pruned Splatting + Hex-Tiling (Pha 2A) vs RVT (Pha 2B) [91, 92, 93, 94]

- **Pha 2A (giả thuyết thử trước trong `NR-SPIKE`, `Suy luận`):** Dùng **Splatting $\le 4$ lớp cắt tỉa theo trọng số** (`w < 0.01` bỏ qua trong fragment shader, trộn theo *height-based blending*) trên `splatmap RGBA8`, ước tính ~5,4 taps/px (`~4,3 MB/frame` ở 720p). Áp dụng **Practical Real-Time Hex-Tiling (Mikkelsen JCGT 2022)** nhưng **chỉ cho 1–2 lớp chủ đạo**: trong WGSL dùng `textureSampleGrad` với đạo hàm màn hình tính trước dịch chuyển lục giác, đặt `r = 0.75` cho Albedo/Roughness, giữ `r = 0.5` cho Normal map và xoay ngược vector pháp tuyến theo góc ô trước khi pha trộn [92].
- **Pha 2B (khi cần $> 4$ lớp + mạng đường + decal rộng):** Nâng cấp sang **Runtime Virtual Texturing (RVT)** để nén chi phí tô bóng địa hình xuống cố định **3 taps/px** (~2,4 MB/frame ở 720p: 1 Page Table + 2 Physical Cache Albedo & World-Space Normal [93]) theo vòng khép kín **Need → Supply → Query** [94]: Feedback RT thu gọn qua `AsyncGPUReadback`, cache LRU có `tileBorder` tường minh tránh rỉ màu lọc song tuyến tính/dị hướng, và giới hạn cứng số tile nướng mỗi frame (kiểu `MaxUploadsPerFrame = 8`). RVT chỉ có lãi băng thông ròng trên Iris Xe khi số lớp vật liệu + decal vượt quá 4.

### 2.5. So sánh 5 bộ Upscale WGSL / `wgpu` trên đường ống 720p → 1080p [9, 95, 96, 98, 100, 106]

| Bộ Upscale | Bản quyền | Pass & đầu vào | Trạng thái WGSL / `wgpu` | Ước lượng mô hình ngoài (720p→1080p), (chưa xác minh) | Giả thuyết để spike kiểm (`Suy luận`) |
|---|---|---|---|---|---|
| **Lightweight TAAU + RCAS** *(Tự chủ WGSL)* | Nội bộ (+ RCAS `MIT`) | **2–3 compute pass** (Reconstruct/Dilate + Variance-clipped Accumulate + RCAS); cần Jitter, Depth, Velocity, Reactive `R16Float` | **100% WGSL thuần**; chạy đồng nhất DX12 & Metal; không cần `globallycoherent`/`wave64` [9, 95] | ~`2,8 ms` / `50,1 MB` BW | **Ứng viên thử đầu tiên** — kỳ vọng khử rung tán lá tốt ở `0,67×` (khớp lựa chọn trong `rusty#1848`, nguồn yếu) |
| **AMD FSR 1.0 (EASU + RCAS)** | `MIT` (`AMD-FSR1-wgpu-shader`) [96] | **2 compute pass**; chỉ cần ảnh màu | **100% WGSL thuần**, tích hợp tức thì | ~`1,4 ms` / `30,8 MB` BW | **Ứng viên fallback siêu nhẹ** cho laptop RAM đơn kênh / tiết kiệm pin |
| **Qualcomm SGSR v1** *(Spatial)* | `BSD 3-Clause` [98] | **1 pass duy nhất**; chỉ cần ảnh màu | GLSL gốc, dễ port sang WGSL | Chưa đo trên Iris Xe/WGSL; nhẹ hơn FSR 1 | Phương án thay thế FSR 1 nếu muốn rút xuống 1 pass |
| **AMD FSR 2.2.1 / `sp-fidelity` (FSR 2 / 3.1.4)** | FSR 2.2.1 & `sp-fidelity`: `MIT` *(lưu ý gói FSR 3.1 SDK mới có điều khoản binary-only / HLSL)* | **~6 compute pass** (Luminance pyramid, Dilate, Depth Clip, Lock, Accumulate, RCAS) | Thiếu `globallycoherent` WGSL phải tách pass; **`sp-fidelity` chưa kiểm chuẩn trên DX12/Vulkan** (mới test Metal) [95, 97] | ~`4,8 ms` / `69,4 MB` BW | **Kỳ vọng không hợp làm mặc định** — tải lịch sử 1080p nặng trên Iris Xe (cần đo) |
| **Arm Accuracy Super Resolution (ASR)** | `MIT` (`arm/accuracy-super-resolution`) [100] | Biến thể temporal rút gọn từ FSR 2 cho GPU công suất thấp [106] | Chủ yếu C/Vulkan/UE, chưa có port WGSL | Chưa đo trên Iris Xe | Tham khảo thuật toán để tinh giản thêm TAAU nội bộ |

**Năm kỷ thuật cốt lõi cho Lightweight TAAU + RCAS (theo `@pmndrs/upscaler` và `rusty#1848` [9, 95] — nguồn yếu: một tệp `CLAUDE.md` của repo và một issue lẻ; các con số µs / % là đo của họ, chưa đo trên Anima):**
1. *Halton jitter + đồng bộ mip bias:* $18$ pha ở Quality (`0,67×`) / $32$ pha ở Performance (`0,50×`) [69] kèm $\text{mipBias} = -\log_2(\text{renderHeight}/\text{displayHeight}) \approx -0{,}585$ ở `720p → 1080p`.
2. *Same-Frame Reconstructed Depth Clip bằng `atomicMin`:* Dùng thứ tự bit IEEE 754 dương để phát tán `atomicMin` chiều sâu tuyến tính *khung hình hiện tại* sang `uv - motion`, khử ~97% báo giả lộ nền khi lùi camera dọc trục nhìn (tốn thêm +6 đến +13 µs trong đo kiểm `@pmndrs/upscaler`) [9].
3. *Reactive Mask `R16Float` + dải chết phơi sáng:* Đánh dấu nước, hạt, tán lá rung mạnh vào reactive mask; nếu auto-exposure, đặt deadband `1/16 stop` và `EXPOSURE_MAX = 8` tránh lượng tử hoá f16.
4. *RCAS trong không gian tonemap điều kiện hoá:* Rút ~34% thời gian compute RCAS trong đo kiểm `@pmndrs/upscaler` [9].
5. *Vị trí trong frame graph:* TAAU thay hoàn toàn TAA, chạy ngay **sau** pass phụ thuộc Depth (froxel, SSAO, ánh sáng) và **trước** hậu kỳ/HUD ở độ phân giải đích [10].

---

## 3. Chiến lược nén & kiểm chuẩn baking offline cho `WV2-5` (chạy trên grid OxideSwarm)

> **Kế hoạch hiện hành vs đề xuất.** `WV2-5` là **thẻ nướng** trong thiết kế [`2026-11-20-world-v2-design.md`](../ai/design/2026-11-20-world-v2-design.md); **OxideSwarm** là **grid worker** chạy các job của thẻ đó (`QĐ-OXS`). Thiết kế đã chấp nhận chốt: Basis/UASTC trong KTX2 cho viewer Three.js Pha 1 (`WV2-6`), bộ mã hoá `bc7enc_rdo`, cổng `Block PSNR >= 42 dB, SSIM >= 0.996` (dòng 553), và mọi job chạy trên **một worker duy nhất** `desktop-3epv830` (dòng 421), với job mã hoá texture xếp loại *pinned-run* vì chưa chứng minh được byte-identity xuyên `arm64` ↔ `x86_64`. Mọi thứ trong Mục 3 khác với các điểm đó — Native BC7/BC5, `bc7e`/`mipple`, ngưỡng `SSIM >= 0.98` lỏng hơn, kiến trúc xuất bản chuẩn Linux `x86_64` cùng runner `arm64` đối soát — là **đề xuất** (`Suy luận`) cần một vòng sửa thiết kế `WV2-D` (và, với thư viện mới, cổng [`OPEN_SOURCE_POLICY.md`](../governance/OPEN_SOURCE_POLICY.md)), **không** phải kế hoạch hiện tại.

### 3.1. Đóng gói KTX2: Native BC7/BC5 vs Basis Universal UASTC vs XUBC7

| Chiến lược KTX2 | Định dạng bên trong & siêu nén | Tốc độ nạp runtime | Chất lượng & tỷ số nén | Khi nào chọn |
|---|---|---|---|---|
| **1. Native BC7 / BC5 trong KTX2 + Zstd** *(Desktop-first)* | Albedo/Emissive: `BC7_SRGB`; Metal/Rough/AO: `BC7_UNORM`; Normal: `BC5_UNORM`; + `Zstd` [107] | **0 ms transcode CPU** — giải nén Zstd và upload thẳng GPU [107] | **Cao nhất:** `bc7e` > 45 dB @ 10–20 Mpix/s [108]; benchmark tự công bố của `mipple` (nguồn yếu: repo 1 sao, chưa ai tái lập) trên 24 ảnh Kodak báo BD-rate tốt hơn `bc7enc_rdo` **−31,5% (BC7)** và **−26,1% (BC5)** dưới `Zstd-19` và nhanh hơn 2–3× [112] | **Đề xuất** cho bản desktop nếu `ADR-0011` chọn renderer native — cần sửa thiết kế `WV2-D` |
| **2. KTX2 + UASTC LDR 4×4 + Zstd** *(Cross Web/Mobile)* | UASTC (`-uastc_level 2–3`, RDO `0.5–1.5`) + `Zstd` [113] | Cần CPU transcode (~160 ms cho bộ texture trên Ryzen 5 3600; chậm ở debug) [115] | Cao (60 phân hoạch chung BC7/ASTC) nhưng vẫn kém nhẹ BC7 trực tiếp [117] | **Kế hoạch hiện hành của `WV2-5`** cho viewer Pha 1 WebGL2 (`WV2-D §3.2`); cũng hợp khi xuất 1 gói chung Desktop + Mobile |
| **3. Basis Universal v2.5 XUBC7 / XUASTC LDR** | Siêu nén không mất dữ liệu khối BC7 (~3,5–5,6 bpp) hoặc mất dữ liệu ~1,15–2,5 bpp kèm Zstd fixed-point [118, 119] | Fixed-point + bảng seek từng mip (API transcode đa luồng chưa mở hết) [118] | Giữ trọn trần `bc7e_scalar_level 4+` khi lossless [119] | Đáng thử nếu cần tải map qua mạng `< 2,5 bpp` mà giữ đích BC7 |

**Bốn quy tắc tự động khoá lỗi texture pipeline (`Nguồn ngoài` [6, 107, 110]):**
1. *Normal map:* Tuyệt đối dùng **BC5** (nén BC7 có thể làm sai số tụt xuống < 30 dB [108]); nếu đi qua UASTC bắt buộc truyền đồng thời `-normal_map` và `-separate_rg_to_color_alpha` (quên cờ này làm kênh Y kẹt cứng tại 255 [110]).
2. *Tách sRGB vs Linear + Toksvig:* Chỉ Albedo/Emissive dùng `BC7_SRGB`; mọi bản đồ dữ liệu dùng `UNORM` tuyến tính, lọc mip trong không gian tuyến tính, tái chuẩn hoá pháp tuyến và nướng bù **Toksvig roughness** ở từng mip [107].
3. *Căn lề staging DX12:* D3D12 bắt buộc **row pitch là bội số của 256 byte** và **mip offset là bội số của 512 byte** — các mip nhỏ `<= 32×32` phải đệm hàng đúng `bytes_per_row` [110].
4. *Tính dung lượng trọn chuỗi mip:* Mọi ngân sách VRAM nhân hệ số $\sum_{k=0}^\infty (1/4)^k = 4/3 \approx 1{,}333\times$ [107].

### 3.2. Đường ống tối ưu lưới 7 bước bắt buộc qua `meshopt` [107, 111, 123, 125]

Chạy đúng thứ tự vì mỗi bước phụ thuộc trực tiếp bố cục bước trước:
1. **Indexing:** `generate_vertex_remap` → `remap_index_buffer` → `remap_vertex_buffer`.
2. **LOD Simplification:** Chế độ `permissive` + heuristic **foliage area-preservation** (`meshoptimizer v1.3+`) tránh tán cây LOD xa bị rỗng [125].
3. **`optimize_vertex_cache`:** Tối đa hoá FIFO vertex cache 16–32 đỉnh [123].
4. **`optimize_overdraw`:** Ngưỡng `threshold = 1.05` (hy sinh tối đa 5% cache để kéo overdraw về ~1,0) [123].
5. **`optimize_vertex_fetch`:** Chạy **sau cùng** trên index buffer đã chốt để tối ưu L1/L2 Iris Xe [123].
6. **Quantization (`unorm` / `snorm` / `half`):** **Giữ nguyên bố cục lượng tử hoá trong VRAM GPU** và giải mã trong vertex shader (giải nén về `f32` lúc upload sẽ xoá sạch lợi ích tiết kiệm băng thông) [107].
7. **Encode + Zstd:** Nén luồng `encode_vertex_buffer` (tỷ số `2–4×`) và `encode_index_buffer` (`5–6×` cho index 16-bit), tốc độ giải nén CPU đạt **5–10 GB/s** (theo số liệu GIC 2026 [111]), rồi bọc Zstd ngoài cùng.

### 3.3. Giải phẫu 5 nguồn bất định đa kiến trúc `arm64 ↔ x86_64` (nếu grid được mở ra nhiều kiến trúc)

*Thiết kế hiện hành chạy mọi job trên một worker; bảng này chỉ cần tới nếu sau này đề xuất chạy nướng trên nhiều kiến trúc được chấp nhận.*

| Nguồn sai lệch bit | Cơ chế kỹ thuật | Tác động `arm64 ↔ x86_64` | Kỷ luật kiểm soát đề xuất (`Suy luận`) |
|---|---|---|---|
| **1. Co gọn FMA** | NEON có FMA làm tròn 1 lần; C/C++ hay bật `-ffp-contract=fast`, còn SSE2 làm tròn 2 lần [126] | Lệch 1 ULP tích luỹ qua vòng lặp nướng | Rust tắt FMA contraction mặc định; biên dịch mọi FFI C/C++ bằng **`-ffp-contract=off`** |
| **2. Hàm siêu việt `libm` hệ thống** | `std` gọi xuống `glibc` / Apple `libSystem` / UCRT (`tan64` từng ghi nhận lệch hàng chục nghìn ULP ở dải Payne–Hanek) [128] | Lệch toạ độ chiếu, mật độ khí quyển, lưới | Tiếp tục khoá qua `clippy.toml` + `libm 0.2.16` hiện hành trên Anima (hoặc đánh giá `alice-det-math` [129] nếu cần bổ sung hàm ngoài `libm`) |
| **3. Độ rộng SIMD ISPC (`bc7e`)** | 8-wide `AVX2` (~1,5–2× nhanh hơn M1 [108]) vs 4-wide `NEON` | Có thể chọn khác mode BC7 ở biên quyết định (Unity `6000.7.0b7` từng gặp [130]) | Không ép bit-exact cho nén khối BC; kiểm bằng Cổng 2 chất lượng/drift |
| **4. Basis Universal & `zlib`** | Khronos `KTX-Software` xác nhận BasisU non-deterministic xuyên arch (`#60`) & RDO đa luồng (`#151`); `zlib` GCC lệch x86/arm [3] | Bitstream khác nhau dù ảnh hợp lệ | Bỏ `zlib`; nếu dùng UASTC RDO ép `--uastc_rdo_m` hoặc `--threads 1` |
| **5. Phiên bản Zstd** | Từ `1.3.4+` output tất định bất kể số luồng (`-T0..-Tn`), **nhưng khác giữa các phiên bản Zstd** [131, 132] | Nâng cấp `zstd` ở một nhóm node làm vỡ hash cache | Ghim cứng chính xác một phiên bản `zstd` trên toàn cụm. Anima **chưa ghim** phiên bản nào: `zstd 0.11.2+zstd.1.5.2` chỉ nằm trong lockfile (qua `tch` → `zip`, không được biên dịch), `zstd-sys` trong lockfile là `1.5.7`; chọn phiên bản là quyết định của `WV2-5` |

**Kỷ luật xuất bản (đề xuất, `Suy luận` — trái với thiết kế hiện hành chạy trên một worker duy nhất, cần sửa `WV2-D`):** Tách đôi hai chuẩn — ép **SHA-256 bit-exact 100%** cho tầng hình học (`meshopt`), heightfield và LUT; đồng thời chỉ định một kiến trúc xuất bản chuẩn (**Canonical Publishing Arch** — khuyến nghị Linux `x86_64 AVX2` để tận dụng tốc độ nén ISPC ~2×) ký và publish artifact cuối, còn runner `arm64` chạy tự động **kiểm chuẩn tương đương đa kiến trúc (Cổng 2)**.

### 3.4. Bộ cổng kiểm chuẩn hai tầng: SHA-256, PSNR, LDR/HDR-FLIP & Sai số góc BC5

Do normal map BC5 tái tạo $z = \sqrt{\max(0, 1 - x^2 - y^2)}$ [4], khi $z \to 0$ ở biên dốc, chỉ 1 bước lượng tử hoá 8-bit ($2/255$) làm góc pháp tuyến xoay nhiều độ nên **PSNR là thước đo mù với normal map**. Đồng thời cần phân biệt rõ giữa **mốc tham chiếu có nguồn công bố** (`BC7 >= 42 dB` [108], sàn khối Binomial `35 dB RGB / 38 dB RGBA` [120], `Mean LDR-FLIP <= 0.05` [134]) với **ngưỡng Cổng 2 đề xuất** neo theo $\text{PSNR} = 20\log_{10}(255/\text{RMSE})$ (`50 dB` $\leftrightarrow \text{RMSE}=0{,}806/255$, `55 dB` $\leftrightarrow 0{,}453/255$ — `Mô hình ngoài / chưa hiệu chỉnh trên map Anima`):

| Nhóm tài nguyên | Độ đo | Cổng 1: Gốc → Đã nướng | Cổng 2: `arm64 ↔ x86_64` *(đề xuất CI)* | Kỷ luật thực thi CI |
|---|---|---|---|---|
| **Albedo / Emissive** (`BC7_SRGB`) | PSNR, SSIM, **LDR-FLIP** | Mốc chuẩn: `PSNR >= 42 dB` (45–46 dB không RDO) [108]; sàn khối RDO `>= 35/38 dB` [120]; `Mean FLIP <= 0.05` [134]; đề xuất `SSIM >= 0.98` (**lỏng hơn** cổng `SSIM >= 0.996` đã chốt cho `WV2-5` ở thiết kế dòng 553; cổng thiết kế vẫn có hiệu lực) | `PSNR >= 50–55 dB`; `Mean FLIP <= 0.015–0.02`; lệch kích thước Zstd `<= 2%` | Giải mã sRGB sang display-linear trước khi chạy FLIP [135]; đo trên **ảnh sau transcode đích cuối** [107] |
| **Normal Map** (`BC5_UNORM`) | **Angular Error** (Mean, P95, Max độ) | Đề xuất: `Mean <= 0.8°–1.0°`; `P95 <= 2.5°–3.0°`; `Max <= 6.0°–8.0°` | `Mean Drift <= 0.25°`; `P95 Drift <= 1.0°` | **Cấm duyệt normal map chỉ bằng PSNR** [4]; kiểm quy ước kênh Y |
| **Metal / Rough / AO / Splat** (`UNORM`) | PSNR/RMSE từng kênh, biên alpha mip | Đề xuất: `PSNR >= 40 dB`/kênh; kiểm tra bảo toàn phủ alpha mọi mip [85] | `PSNR >= 52 dB`; `Max Diff <= 2–3` mức 8-bit | Không ghép kênh nhiễu cao với kênh mịn trong cùng khối BC7 [107] |
| **HDR / IBL / Lightmap** (`BC6H` / UASTC HDR) | **HDR-FLIP** [136] | Đề xuất: `Mean HDR-FLIP <= 0.05–0.08`; cấm cắt HDR về LDR | `Mean HDR-FLIP <= 0.02` | Xuất kèm bản đồ phơi sáng sai số cực đại để bắt *fireflies* [136] |
| **Hình học `meshopt`, Heightfield, LUT** | **SHA-256**, topo, `meshopt_decode*` | 0 lỗi topo/winding; kiểm sai số lượng tử hoá theo texel [107] | **100% Bit-exact (SHA-256)** (`libm`/all-det + `-ffp-contract=off`) | Giải mã ngược qua `meshopt_decode*` trong CI |

---

## 4. Lộ trình thực nghiệm đề xuất cho ứng viên (a) của `NR-SPIKE` & cho `WV2-5`

*Mốc 1–3 chỉ bao ứng viên (a) — `wgpu` tự viết trong cửa sổ Tauri. `NR-SPIKE` vẫn phải đo cả (b) Bevy mới và (c) Godot 4 trên cùng cảnh bắt buộc trước khi viết `ADR-0011`; mã spike không merge, chỉ merge ghi chú kết quả. Mã spike của (a) cần feature gate riêng để không vi phạm cổng G2 của CI (Mục 0).*

1. **Mốc 1 — Vỏ cửa sổ Tauri v2 + `wgpu 30` không nhấp nháy & không panic (Tuần 1, ứng viên a):** Thử nghiệm song song trên Windows 10/11 (DX12 kèm cặp `dxcompiler.dll` + `dxil.dll`, đối chứng Child HWND `DxgiFromHwnd` vs `WS_EX_NOREDIRECTIONBITMAP` + `CompositionVisual` dưới DOM-First) và macOS Retina 2x (sibling Metal `NSView` + `drawsBackground = NO` + `presentsWithTransaction` động khi resize). *Tiêu chí:* kéo giãn 4 cạnh, minimize/restore 50 chu kỳ, kéo qua lại DPI 100% ↔ 150%/200% với 0 panic, 0 nền đen DWM, gõ tiếng Việt TSF/IME bình thường.
2. **Mốc 2 — Hiệu chỉnh ngân sách 30 FPS trên phần cứng thật Intel Iris Xe (Tuần 2, ứng viên a):** Đo **cảnh bắt buộc** của `feature-map-v2` (địa hình World v2, nước, **10.000 cây**, **1.000 sinh vật**), dựng bằng các kỹ thuật đề xuất (địa hình 4 lớp cắt tỉa + Hex-Tiling Top-2, bầu trời Hillaire + froxel `32×32×16`, cây hai pha Hi-Z SPD + xô indirect cố định + hemi-octahedral impostor `1024²` BC7/BC5/BC4) ở 720p. Một biến thể tải nặng hơn (>100k cây) là phép đo **bổ sung** do tài liệu này đề xuất, không thay cho cảnh bắt buộc, trong **15 phút tải liên tục** trên laptop 15 W sau thermal throttling. Dùng timestamp query ở ranh giới pass (vì Apple Silicon không hỗ trợ timestamp bên trong pass/encoder [60]) để thay mô hình ngoài bằng số đo thật và chốt ngưỡng DRS / fallback FSR 1 cho RAM đơn kênh.
3. **Mốc 3 — Đo bộ upscale Lightweight TAAU + RCAS thuần WGSL (Tuần 3, ứng viên a):** Triển khai TAAU 2–3 pass với Halton 18/32 pha, same-frame `atomicMin` depth clip, `R16Float` reactive mask và RCAS điều kiện hoá; đối chứng chất lượng (`LDR-FLIP` vs tham chiếu supersampled + kiểm thị giác khi xoay/dolly-out) với FSR 1.
4. **Mốc 4 — Đề xuất mở rộng `WV2-5` (cần sửa thiết kế `WV2-D` trước, `Suy luận`):** Khép pipeline `meshopt` 7 bước (giữ vertex lượng tử hoá trên GPU); chọn và ghim **một** phiên bản zstd (hiện chưa có — Mục 0); nếu renderer native được chọn, thử Native BC7/BC5 trong KTX2 (so tốc độ/BD-rate `bc7enc_rdo` — bộ mã hoá thiết kế đã chọn — với `bc7e` / `mipple` trên bộ texture Anima) mà vẫn giữ cổng `PSNR >= 42 dB, SSIM >= 0.996` của thiết kế; chỉ khi grid được mở ra nhiều kiến trúc mới cần runner xuất bản chuẩn và cổng đối soát tự động (SHA-256 + PSNR/FLIP/BC5 Angular Error) với runner `arm64`.

---

## 5. Tài liệu tham khảo chọn lọc từ nguồn gốc

> **Ghi chú biên tập:** Nguồn nội bộ chưa công bố liên kết (mục `12` trong báo cáo ngoài gốc) đã được loại bỏ. Một số nguồn gần đây / năm 2026 (`[15]`, `[110]`, `[111]`, `[130]`) và nhật ký PoC/issue cộng đồng cần rà lại ở thời điểm kiểm thử `NR-SPIKE`.

- **[1]** `tauri#15947` — Windows 10 transparent window black screen race (`set_ignore_cursor_events`): https://github.com/tauri-apps/tauri/issues/15947
- **[2]** `mihazs/lazysh` — *windows-macos-ios-surface-strategies.md*: https://github.com/mihazs/lazysh/blob/main/docs/architecture/windows-macos-ios-surface-strategies.md
- **[3]** KhronosGroup `KTX-Software` Releases (BasisU non-determinism `#60`, `#151`): https://github.com/KhronosGroup/KTX-Software/releases
- **[4]** `m31labs.dev/gosx/assetpipe/texture/bcn` — BC5 normal angular error curve: https://pkg.go.dev/m31labs.dev/gosx/assetpipe/texture/bcn
- **[5]** Technical City — *Iris Xe Graphics 80EU vs G7 96EU*: https://technical.city/en/gpu/Iris-Xe-Graphics-G7-96EU-vs-Iris-Xe-Graphics-80EU
- **[6]** Aras Pranckevičius — *Blogs (Texture compression series)*: https://aras-p.info/blog/page/11/
- **[7]** *Intercepting Windows Messages from Webview2 (Edge/Chromium)*: https://stackoverflow.com/questions/65110069/intercepting-windows-messages-from-webview2-edge-chromium
- **[8]** Wails Guide — *Private macOS APIs* (`drawsBackground` & Mac App Store policy): https://wails.io/guides/build/private-macos-apis/
- **[9]** `@pmndrs/upscaler` (`CLAUDE.md`) — WGSL TAAU benchmarks & same-frame atomicMin depth clip: https://github.com/pmndrs/upscaler/blob/main/CLAUDE.md
- **[10]** HWCooling — *AMD reveals details of FSR 2.0*: https://www.hwcooling.net/en/amd-reveals-details-of-fsr-2-0-its-temporal-dlss-2-0-competitor/
- **[13]** `tauri#11944` — *Render wgpu frames as webview overlay*: https://github.com/orgs/tauri-apps/discussions/11944
- **[15]** `MicrosoftEdge/WebView2Feedback#5642` — WebView2 Evergreen 149 + `wry 0.55.1`, ô compositor 256 px bị cũ sau zoom (chưa kiểm trên Anima): https://github.com/MicrosoftEdge/WebView2Feedback/issues/5642
- **[18]** `tauri::window::Window` Docs (deadlock warning on synchronous add_child): https://docs.rs/tauri/latest/tauri/window/struct.Window.html
- **[20]** `Handy#2320` — WebView2 155.0 `ERROR_INVALID_WINDOW_HANDLE (0x80070578)` on invisible host: https://github.com/cjpais/Handy/issues/2320
- **[21]** `tauri#12450` — Transparent attribute on child window (`WS_CLIPCHILDREN`): https://github.com/tauri-apps/tauri/issues/12450
- **[23]** `wgpu_types::Dx12SwapchainKind`: https://docs.rs/wgpu-types/latest/wgpu_types/enum.Dx12SwapchainKind.html
- **[26]** `anomalyco/rune#47` — Live resize jitter on DX12 + `wgpu 30`: https://github.com/anomalyco/rune/issues/47
- **[27]** Microsoft Learn — `ICoreWebView2CompositionController`: https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2compositioncontroller?view=webview2-1.0.4314.35
- **[29]** Microsoft Q&A — `WS_EX_NOREDIRECTIONBITMAP` 1px border on Win11: https://learn.microsoft.com/en-us/answers/questions/2244571/ws-ex-noredirectionbitmap-unexpectedly-makes-windo
- **[30]** `raw-window-handle` CHANGELOG (0.6 `NSView` change): https://github.com/rust-windowing/raw-window-handle/blob/master/CHANGELOG.md
- **[32]** `wgpu` `surface.rs` source (`Surface::configure` panic preconditions): https://docs.rs/wgpu/latest/src/wgpu/api/surface.rs.html
- **[34]** Transparent background `WKWebView` (`drawsBackground = NO`): https://stackoverflow.com/questions/27211561/transparent-background-wkwebview-nsview
- **[35]** `wind-ui-rust` — Cross-platform GPU / AppKit layer design: https://github.com/huanfeng/wind-ui-rust/blob/main/docs/gpu-cross-platform-design.md
- **[39]** UniWebView — *Clicking Through*: https://docs.uniwebview.com/guide/transparency-through.html
- **[40]** Microsoft Learn — WebView2 performance best practices (web messages vs host objects): https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance
- **[42]** Apple Developer — `NSView::hitTest(_:)`: https://developer.apple.com/documentation/appkit/nsview/hittest(_:)
- **[44]** `CodeAll` — `30-implementation-stack.md` (`ignoresMouseEvents`): https://github.com/RichardDou0607/CodeAll/blob/main/docs/30-implementation-stack.md
- **[45]** `gfx-rs/wgpu#9003` — "Crash on nvidia cards since wgpu 28" (bản sửa liên kết đụng tới `present`; phiên bản sửa chưa xác minh): https://github.com/gfx-rs/wgpu/issues/9003
- **[46]** `gfx-rs/wgpu` CHANGELOG (Metal GPU ICB `>=512` draws, DXC requirements): https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md
- **[47]** `wgpu::CurrentSurfaceTexture` Docs: https://docs.rs/wgpu/latest/wgpu/enum.CurrentSurfaceTexture.html
- **[51]** Wails Changelog (WebView2 post-DPI resize bounds sync): https://wails.io/changelog/
- **[54]** `metal-live-resize` Crate Docs: https://docs.rs/metal-live-resize
- **[55]** Apple Developer — `CAMetalLayer::presentsWithTransaction`: https://developer.apple.com/documentation/quartzcore/cametallayer/presentswithtransaction
- **[57]** Zed Blog — *120 FPS*: https://zed.dev/blog/120fps
- **[58]** `EffectCraft#613` — FXC vs DXC compute shader failures: https://github.com/storytold/effectcraft/issues/613
- **[59]** `LightCraft#2936` — FXC compile latency, `Dx12Compiler::Auto` DLL mismatch & i915/TDR hang: https://github.com/storytold/lightcraft/issues/2936
- **[60]** `wgpu::Features` Docs (`SHADER_F16`, `RG11B10UFLOAT_RENDERABLE`, timestamp caveats): https://docs.rs/wgpu/latest/wgpu/struct.Features.html
- **[61]** `wgpu::Dx12Compiler` Docs: https://docs.rs/wgpu/latest/wgpu/enum.Dx12Compiler.html
- **[62]** NanoReview — Intel Iris Xe G7 (96EU): https://nanoreview.net/en/gpu/intel-iris-xe-graphics-g7-96eu
- **[64]** Sysrqmts — Intel Iris Xe Graphics G7 96EU specs: https://sysrqmts.com/gpus/compare/intel-iris-xe-graphics-g7-96eu-vs-intel-uhd-graphics-630
- **[69]** GPUOpen `FidelityFX-FSR2` (Halton jitter & mip bias): https://github.com/GPUOpen-Effects/FidelityFX-FSR2
- **[70]** `skycope` — Sky & atmosphere shader performance log: https://github.com/skycope
- **[71]** `@pmndrs/sky` `PLAN.md` (Hillaire 2020 LUT specs): https://github.com/pmndrs/sky/blob/main/PLAN.md
- **[72]** Merova Studios — *Why the Sky Is Blue* (LUT & volumetric cloud cost): https://www.merova.net/docs/atmosphere
- **[75]** WebGPU GPU-Driven Occlusion Culling & Hi-Z Guide: https://www.serdarozden.com/webgpu-gpu-driven-occlusion-culling-guide
- **[76]** Tiger's Place — *Indirect draws in WebGPU*: https://tigerabrodi.blog/indirect-draws-in-webgpu-and-why-they-re-so-powerful
- **[77]** `SpartanEngine/docs/rendering.md` (Two-Phase Hi-Z culling): https://github.com/PanosK92/SpartanEngine/blob/master/docs/rendering.md
- **[80]** `lakan` `0020-platform-minimums.md` (`MDI` / `MDIC` backend support): https://gitlab.com/darkanrs/lakan/-/blob/v0.3.0/docs/plan/adr/0020-platform-minimums.md
- **[84]** `yummers.dev` — *Hemi-Octahedral Impostors*: https://yummers.dev/hemi-octahedral-impostors.html
- **[85]** `SamG-Coder/SmartImposter` (Coverage-weighted mipmaps & stochastic single-view): https://github.com/SamG-Coder/SmartImposter
- **[86]** *Transparency: The Rasterization Nightmare* (Medium): https://medium.com/design-bootcamp/transparency-the-rasterization-nightmare-71ceb98a8e3c
- **[87]** Ben Golus — *Anti-aliased Alpha Test: The Esoteric Alpha To Coverage*: https://bgolus.medium.com/anti-aliased-alpha-test-the-esoteric-alpha-to-coverage-8b177335ae4f
- **[88]** Wyman & McGuire (I3D 2017) — *Hashed Alpha Testing*: https://cwyman.org/papers/i3d17_hashedAlpha.pdf
- **[89]** Wyman & McGuire (TVCG 2019) — *Improved Alpha Testing Using Hashed Sampling*: https://www.computer.org/csdl/journal/tg/2019/02/08010336/17D45VTRoxw
- **[91]** 80 Level — *Deep Dive: Terrain Shader Generation Systems*: https://80.lv/articles/using-next-gen-terrain-engines-for-games-production-009snw
- **[92]** Mikkelsen (JCGT 2022) — *Practical Real-Time Hex-Tiling*: https://jcgt.org/published/0011/03/05/paper.pdf
- **[93]** Epic Games — *Runtime Virtual Texturing*: https://dev.epicgames.com/documentation/unreal-engine/runtime-virtual-texturing-in-unreal-engine
- **[94]** CGBull — *Infinity RP Runtime Virtual Texturing* (Need → Supply → Query): https://www.cgbull.net/blog/infinity-rp-runtime-virtual-texturing/
- **[95]** `rusty#1848` — FSR2/3.1 vs TAAU in WGSL/`wgpu`: https://github.com/MatthewLacerda2/rusty/issues/1848
- **[96]** `firdawolf/AMD-FSR1-wgpu-shader` (MIT): https://github.com/firdawolf/AMD-FSR1-wgpu-shader
- **[97]** `stevepryde/sp-fidelity` (MIT): https://github.com/stevepryde/sp-fidelity
- **[98]** Snap Inc. — *Lens Studio Third-Party Software Notices* (SGSR BSD 3-Clause): https://www.snap.com/terms/lensstudionotices?lang=es-ES
- **[100]** HyperQuant Media — *Compendium* (SGSR / Arm ASR): https://hyperquantmedia.com/compendium/
- **[106]** Arm ASR announcement (derived from FSR2, MIT): https://videocardz.com/newz/arm-introduces-accuracy-super-resolution-derived-from-amd-fsr2-and-open-source
- **[107]** `vercel-labs/vgpu` — *asset-delivery.md* (KTX2/BCn, Toksvig, meshopt quantization, mip chains): https://github.com/vercel-labs/vgpu/blob/canary/skills/vgpu/blender/references/asset-delivery.md
- **[108]** Aras Pranckevičius — *Texture Compression in 2020* (`bc7e` PSNR benchmarks): https://aras-p.info/blog/2020/12/08/Texture-Compression-in-2020/
- **[110]** `forge-pipeline-texture-compression` (DX12 256B row pitch & `-separate_rg_to_color_alpha`): https://agentarea.ai/skills/code-review-benchmark-coderabbit-prs2-forge-gpu-coderabbit-pr323-20260317-forge-pipeline-texture-compression/
- **[111]** GIC 2026 PDF — *ISPC: The Next Chapter* (`meshoptimizer` 5–10 GB/s decode): https://call.gic.gd/media/ISPC_The_Next_Chapter_GIC2026_IuUBBwQ.pdf
- **[112]** `Azazel-Labs/mipple` `BENCHMARKS.md` (−31,5% BC7 / −26,1% BC5 BD-rate under Zstd-19): https://github.com/Azazel-Labs/mipple/blob/main/docs/BENCHMARKS.md
- **[113]** Basis Universal `basisu_tool.cpp`: https://skia.googlesource.com/external/github.com/BinomialLLC/basis_universal/+/bd2756a61741cbc7fcab5cc0a86c2099a3b0a3f7/basisu_tool.cpp
- **[115]** Vulkan Samples — *Texture Compression with KTX and Basis Universal*: https://docs.vulkan.org/samples/latest/samples/performance/texture_compression_basisu/README.html
- **[117]** Binomial LLC — *UASTC LDR 4×4 Specification*: https://github.com/BinomialLLC/basis_universal/wiki/UASTC-LDR-4x4-Texture-Specification
- **[118]** Binomial LLC `basis_universal` (XUBC7 / XUASTC): https://github.com/binomialLLC/basis_universal
- **[119]** Binomial LLC — *XUBC7 Usage Guide*: https://github.com/BinomialLLC/basis_universal/wiki/XUBC7-Usage-Guide
- **[120]** Binomial LLC — *ASTC and XUASTC LDR Usage Guide* (35/38 dB block quality floor): https://github.com/BinomialLLC/basis_universal/wiki/ASTC-and-XUASTC-LDR-Usage-Guide
- **[123]** `meshopt 0.6.0` on crates.io: https://crates.io/crates/meshopt/0.6.0
- **[125]** Graphics Programming Weekly Database (`meshoptimizer v1.3` foliage simplification): https://www.jendrikillner.com/article_database/
- **[126]** `tpt-deterministic` Docs (FMA contraction): https://docs.rs/tpt-deterministic
- **[128]** Rust Users Forum — *Subtle floating-point differences between C and Rust* (`libm` Payne–Hanek): https://users.rust-lang.org/t/subtle-floating-point-differences-between-c-library-and-its-rust-re-write/82355?page=2
- **[129]** `alice-det-math` Docs: https://docs.rs/alice-det-math
- **[130]** Unity `6000.7.0b7` Release Notes (BC7E cross-arch divergence fix): https://unity.com/releases/editor/beta/6000.7.0b7
- **[131]** `facebook/zstd#999` — Cross-version vs multi-thread determinism: https://github.com/facebook/zstd/issues/999
- **[132]** `facebook/zstd#2949` — Deterministic compression parameters: https://github.com/facebook/zstd/issues/2949
- **[134]** `tksuoran/erhe` — `graphics_test_coverage.md` (`LDR-FLIP <= 0.05` CI threshold): https://github.com/tksuoran/erhe/blob/main/doc/erhe/graphics_test_coverage.md
- **[135]** `@pmndrs/pathtracer` `CLAUDE.md` (display-linear FLIP error inflation if sRGB not decoded): https://github.com/pmndrs/pathtracer/blob/main/CLAUDE.md
- **[136]** Andersson et al. — *Visualizing and Communicating Errors in Rendered Images* (HDR-FLIP): https://www.researchgate.net/publication/354064991_Visualizing_and_Communicating_Errors_in_Rendered_Images
