---
title: Buổi dùng thử đầu tiên với người ngoài (bản đồ, thân sinh vật, thế giới thí nghiệm)
status: active
owner: maintainers
last_reviewed: 2026-11-19
review_cycle: per-release
language: vi
translation_of: OWNER_TRIAL_RUN.md
---

# Buổi dùng thử đầu tiên với người ngoài

Viết ngày 2026-11-19 cho chủ dự án, theo quyết định QD-TRIAL (chạy thử trên Windows 1). Đây là bản dùng thử
(*prototype*): mục tiêu là cho một người bạn thấy dự án đang đi về đâu và lấy được ý kiến của họ, chưa phải
bản phát hành.

Thứ tự gồm ba bước, và mỗi bước có điều kiện để được sang bước sau:

1. **Chạy thử một mình trên Windows 1** khoảng 30 phút (§3), có lead đọc log cùng. Chưa ai chạy app desktop
   thật với bản đồ mới; mọi kiểm tra trước đó đều chạy trong trình duyệt với dữ liệu giả lập.
2. **Cho bạn xem trên Windows 1**, bạn tự cầm máy (§5).
3. **Cài lên máy của người bạn** (§6), chỉ khi bước 1 không có lỗi nặng.

## 1. Điều kiện trước khi bắt đầu

Các PR sau phải đã merge vào `feat/ecological-selection`. Kiểm bằng
`gh pr view <số> --json state -q .state`, phải in ra `MERGED`.

| Việc | Ai làm | Vì sao cần cho buổi thử |
|---|---|---|
| B1, thân sinh vật (PR #153) | Windows 1 | Sinh vật là thân liền, có mắt ở phía trước |
| LAND1, mô phỏng phân biệt đất và nước | Dell | Không còn sinh vật đi trên biển |
| W2-lite, nút mở thế giới thí nghiệm | Mac | Mở một thế giới từ manifest đã đăng ký |
| W3-lite đợt 1 (sửa 5 lỗi xấu nhất) | Windows 1 | Bỏ quầng xanh quanh mép, biển lấm tấm, thác nước thành ô trắng, dải màu cam đỏ, tuyết phẳng |
| W3-lite đợt 2 (texture và cây CC0) | Windows 1 | Không bắt buộc: đẹp hơn rõ, nhưng có thể chưa kịp |

Lead báo khi đủ điều kiện, rồi tạm dừng agent trên Windows 1 trong lúc thử. Không chạy cùng lúc với agent,
vì app nặng và agent build Rust cùng lúc sẽ làm máy chậm.

## 2. Dựng bản chạy trên Windows 1

Dùng một worktree riêng, không dùng thư mục làm việc của agent. Chạy trong PowerShell:

```powershell
cd D:\teamwork_projects\Anima-Engine
git fetch origin
git worktree add D:\teamwork_projects\anima-trial origin/feat/ecological-selection
cd D:\teamwork_projects\anima-trial
npm ci
npm run tauri:build
```

- Dùng `npm run tauri:build`, **không** dùng `tauri build` trần. Lệnh trần thiếu `--features desktop`, nên
  build ra một bản thiếu tính năng mà không báo lỗi.
- Lần build đầu là build lạnh và có thể mất 20–40 phút. Tauri có thể tự tải công cụ đóng gói (NSIS, WiX)
  trong lần đầu.
- File chạy: `.\src-tauri\target\release\anima-engine.exe`.
- Bộ cài: trong `.\src-tauri\target\release\bundle\` (thư mục `nsis\` và `msi\`).

Chạy từ PowerShell để thấy log trên cửa sổ đó, và lưu log ra file để gửi lead:

```powershell
$env:ANIMA_FOUNDING_POPULATION = "200"
.\src-tauri\target\release\anima-engine.exe 2>&1 | Tee-Object -FilePath trial-log.txt
```

`ANIMA_FOUNDING_POPULATION` là số cá thể sáng lập. Không đặt thì chỉ có 10 con và bản đồ trông trống.
200 là mức vừa cho buổi thử; giới hạn trên là 10.000.

## 3. Chạy thử một mình (khoảng 30 phút, có lead đọc log)

Đánh dấu từng dòng. Dòng nào hỏng thì chụp màn hình và gửi `trial-log.txt` cho lead.

**Khởi động**

- [ ] App mở, không có cửa sổ lỗi.
- [ ] Nút "Bắt đầu mô phỏng" sáng lên sau vài giây. Nó cố ý mờ cho tới khi thế giới dùng chung đã nạp vào
      backend; lần đầu trên một máy mới, việc sinh thế giới 2048² có thể mất một lúc.
- [ ] Bấm "Bắt đầu mô phỏng": số sinh vật và số tick tăng.

**Bản đồ 3D** (nút "🏞️ Landscape Showcase")

- [ ] Bản đồ hiện ra; ô thông tin ghi **"Simulated: … lakes · … trees · … food"**. Nếu thấy "Synthetic
      fixture", app đang chạy dữ liệu giả lập: dừng lại và báo lead.
- [ ] Sinh vật đứng trên đất, không đi trên biển (cần LAND1).
- [ ] Lại gần một con: thân liền, mắt ở phía trước. Đỏ là thú săn mồi, xanh là con mồi.
- [ ] Có chú thích "Decoration, not simulated" cho cá, chim, thú nền và hang; nút ẩn trang trí hoạt động.
- [ ] FPS trên HUD từ 30 trở lên ở mức "Cao". Ghi lại con số.
- [ ] Các chế độ camera (Quay, Bay, Đi bộ, Trên cao, Cine) và tốc độ 0.5×–4× hoạt động.

**Chạy lâu**

- [ ] 20 phút ở tốc độ 4×: không treo, không văng.
- [ ] Bộ nhớ của `anima-engine.exe` trong Task Manager không tăng mãi (ghi số ở phút 1 và phút 20).
- [ ] Sinh vật vẫn còn: quần thể không chết hết trong 20 phút.

**Thế giới thí nghiệm** (cần W2-lite)

- [ ] Nút mở thế giới thí nghiệm hiện ra cạnh nút chạy mô phỏng và mở được một manifest có sẵn.
- [ ] Thế giới mở ra chạy được, bản đồ hiện đúng thế giới đó.

**Lưu và mở lại**

- [ ] Đóng app rồi mở lại: app hỏi hoặc tự tiếp tục từ bản tự lưu, không lỗi.

Nếu có lỗi nặng (văng, treo, sinh vật biến mất, bản đồ đen), **dừng ở đây**. Lead sửa trước, rồi mới
cho người ngoài xem.

## 4. Nói gì với người bạn (trung thực)

Nên nói:

- Đây là bản dùng thử của một mô phỏng sự sống nhân tạo. Sinh vật tiến hoá qua nhiều thế hệ.
- Hình dạng mỗi con được dựng từ các đốt, khớp và kích thước trong "bộ gen" của chính nó. Màu theo loại
  (săn mồi hay con mồi), không theo vùng.
- Hồ, cây và thức ăn trên bản đồ là của mô phỏng. Cá, chim, thú nền và hang chỉ để trang trí, và app có
  ghi rõ như vậy.
- Mắt và nhịp thở chỉ để trang trí. Thân được vẽ nhỏ hơn tỷ lệ thật so với khoảng cách, để nhìn cho rõ.
- Phần khoa học chạy riêng, ngoài app. Đang có một thí nghiệm đăng ký trước (SEL2-P: thiếu thức ăn có làm
  đổi kích thước cơ thể không), và câu hỏi chủ lực tiếp theo là quy luật đảo.

Không nên nói:

- "Trí tuệ nhân tạo tự sống", "loài mới", "tiến hoá như thật". Dự án chưa đo và chưa khẳng định những điều
  đó.
- Rằng mọi thứ trên bản đồ đều là mô phỏng: phần trang trí không phải.

## 5. Buổi xem (khoảng 20 phút, bạn cầm máy)

1. **Toàn cảnh** (camera Trên cao): một thế giới có núi tuyết, rừng, sông, hồ, biển.
2. **Lại gần một sinh vật**: hình dạng lấy từ bộ gen; mỗi con một khác.
3. **Ăn, uống, săn**: để tốc độ 1×, theo một con săn mồi.
4. **Tiến hoá**: chuyển 4× vài phút; chỉ số thế hệ và số lượng thay đổi.
5. **Thế giới thí nghiệm**: mở một manifest, giải thích "cùng luật, cùng hạt giống, chạy lại ra cùng kết
   quả trên cùng loại máy".
6. **Hỏi ý kiến** (§7).

## 6. Cài lên máy người bạn — được, nhưng có điều kiện

Cài được. Lý do chưa nên làm ngay:

- **Bộ cài chưa ký số** (quyết định QĐ-M8 về ký bộ cài vẫn để sau). Windows SmartScreen sẽ hiện "Windows
  protected your PC". Người bạn phải bấm "More info", rồi "Run anyway". Một số phần mềm diệt virus có thể
  chặn file chưa ký.
- **Chưa đo trên máy yếu.** Mục tiêu là 30 FPS ở mức Thấp trên Iris Xe với 1.000 sinh vật, nhưng chưa đo.
  Máy không có card rời có thể giật; khi đó chọn mức "Thấp" và giảm `ANIMA_FOUNDING_POPULATION`.
- **Chỉ có bản Windows.** CI chỉ build trên Windows. macOS cần ký và notarize, nếu không Gatekeeper sẽ chặn.
- **Không có tự cập nhật hay gửi lỗi tự động.** Nếu app văng trên máy bạn ấy, chỉ có ảnh chụp màn hình và
  file log mà bạn ấy tự gửi lại.

Khi bước 3 đã qua, đưa bạn ấy file trong `src-tauri\target\release\bundle\nsis\`, kèm ba dòng hướng dẫn:

1. Mở file cài. Nếu thấy SmartScreen, bấm "More info", rồi "Run anyway".
2. Mở Anima-Engine, đợi nút "Bắt đầu mô phỏng" sáng, bấm nó, rồi bấm "🏞️ Landscape Showcase".
3. Nếu chậm, chọn mức đồ hoạ "Thấp". Nếu lỗi, chụp màn hình và gửi lại.

Bản cài trên máy bạn ấy chạy với 10 sinh vật sáng lập, trừ khi đặt biến môi trường. Muốn nhiều hơn thì
hướng dẫn bạn ấy chạy từ PowerShell như §2, hoặc chờ một thẻ thêm ô nhập số sinh vật trong app.

## 7. Ý kiến cần thu lại

- Điều gì làm bạn thấy thú vị nhất? Điều gì khó hiểu nhất?
- Bạn có muốn tự đặt điều kiện cho một thế giới (thiếu thức ăn, nhiều kẻ săn mồi, đảo nhỏ) rồi xem chuyện
  gì xảy ra không?
- Bản đồ và sinh vật trông thế nào: đẹp, xấu, khó nhìn ở chỗ nào?
- Máy của bạn có chạy mượt không (FPS, mức đồ hoạ)?

Ghi lại câu trả lời và gửi cho lead; lead đưa chúng vào hàng việc.

## 8. Sau buổi thử

- Báo lead để mở lại agent trên Windows 1.
- Gửi lead `trial-log.txt`, ảnh chụp và ý kiến của người bạn.
- Có thể xoá worktree thử: `git worktree remove D:\teamwork_projects\anima-trial`.
