---
phase: design
feature: generated-docs-index
title: Chỉ mục sinh cho docs/ai — thêm ghi chú mà không sửa file chỉ mục chung
description: So sánh năm cách bỏ xung đột lặp lại trên docs/ai/README.md và chọn chỉ mục sinh từ front matter giữ nguyên hàng cũ; checker chấp nhận ghi chú chờ khi front matter đủ để sinh hàng và thư mục của nó được link, rồi tự chạy generator trong bộ nhớ để chứng minh không còn gì chờ
status: active
owner: maintainers
last_reviewed: 2026-11-19
---

# Chỉ mục sinh cho `docs/ai/`

Chủ dự án duyệt thẻ này ngày 2026-11-19 ("Làm"). Ghi chú này là thiết kế; mã nằm trong cùng PR:
[`scripts/lib/docs_index.mjs`](../../../scripts/lib/docs_index.mjs) (hợp đồng chung),
[`scripts/gen_docs_index.mjs`](../../../scripts/gen_docs_index.mjs) (`npm run gen:docs-index`) và
[`scripts/check_docs_metadata.mjs`](../../../scripts/check_docs_metadata.mjs) (cổng). Quy tắc chính
thức nằm ở mục "Chỉ mục sinh" của
[chính sách tài liệu](../../governance/DOCUMENTATION_POLICY.md#chỉ-mục-sinh-2026-11-19).

Bản thân ghi chú này được thêm **không có hàng** trong [`docs/ai/README.md`](../README.md): nó là
tài liệu chờ đầu tiên, và CI xanh trên PR này là phép thử chấp nhận của thiết kế.

## 1. Vấn đề

Gần như mọi PR thêm một ghi chú kết quả dưới `docs/ai/` và, theo dòng uỷ quyền của thẻ, thêm hàng
của nó vào bảng "Bản giao việc, bản ghi phiên và đo lường" trong `docs/ai/README.md`, vì
`npm run check:docs` đòi mọi tài liệu được link từ một chỉ mục. Hàng mới luôn nằm ở cuối bảng, nên
hai PR mở cùng lúc chèn một dòng vào **cùng một chỗ**: GitHub đánh dấu PR thứ hai là xung đột, và
lead merge thân cây vào nhánh đó rồi xếp lại hàng bằng tay.

Đo trên `origin/feat/ecological-selection` tại `2a0f45a` (2026-11-19):

- Kể từ 2026-11-17, 17 commit merge đồng bộ nhánh (`Merge ... into ...`, không tính merge PR) mang
  thay đổi vào file này — phần lớn là `lead/*-merge` hoặc merge thân cây vào nhánh của agent.
- Ba PR đang mở (#151, #152, #153) cùng chèn một hàng ngay sau hàng S2 (dòng 85). Bất kỳ hai PR nào
  trong ba cũng xung đột với nhau.

## 2. Tiêu chí

1. Không PR nào phải sửa một file dùng chung để thêm một ghi chú.
2. Duyệt `docs/ai/` trên GitHub vẫn thấy mọi ghi chú.
3. Bảo đảm "mọi tài liệu tới được từ một chỉ mục" được giữ, hoặc thay bằng một bảo đảm tương đương
   mà CI cưỡng chế.
4. Chạy được bằng `node` thuần trên Windows và macOS (agent ở cả hai).
5. PR đang mở đã thêm hàng vào README vẫn xanh, hoặc chỉ cần rebase tầm thường.

Ràng buộc: cả hai nhánh được bảo vệ, mọi thay đổi vào bằng PR có `Rust (src-tauri)` và `Frontend`
xanh, nên không bot nào đẩy được chỉ mục đã sinh lại sau khi merge.

## 3. Các phương án

**(a) Sinh lúc kiểm; lead commit định kỳ; checker chấp nhận ghi chú chưa có hàng.** PR không đụng
README. Checker coi một ghi chú chưa có hàng là hợp lệ nếu front matter đủ để sinh hàng. Lead chạy
generator trong PR tài liệu của mình. Đạt tiêu chí 1. Ở dạng thô thì yếu ở tiêu chí 2–3: giữa hai
lần sinh, README không link tới ghi chú mới, và một generator viết lại cả bảng từ front matter sẽ xoá
chữ tóm tắt mà lead đã viết cho từng hàng (chữ đó khác `description` của file ở gần như mọi hàng).

**(b) Mỗi mục một file chỉ mục nhỏ (hoặc chỉ dùng danh sách thư mục), generator ráp lại.** Nếu bảng
ráp xong được commit trong mỗi PR thì hai PR lại chèn vào cùng chỗ trong file ráp — xung đột y như cũ,
chỉ dễ gỡ hơn. Nếu không commit bảng ráp thì GitHub chỉ cho thấy một thư mục file mảnh, mỗi mảnh
chép lại front matter của ghi chú và có thể trôi khỏi nó. Biến thể "chỉ link tới thư mục, bỏ bảng"
đạt tiêu chí 1–2 nhưng mất cột trạng thái và tóm tắt mà lead và agent đọc hằng ngày.

**(c) `.gitattributes` với `merge=union`.** Tài liệu GitHub về xung đột merge
([About merge conflicts](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/addressing-merge-conflicts/about-merge-conflicts),
đọc ngày 2026-11-19) không nhắc tới `.gitattributes` hay merge driver; nó chỉ nói nút merge bị khoá
cho tới khi xung đột được gỡ. Câu trả lời duy nhất từ phía GitHub mà tôi tìm được là thư của GitHub
Support (khoảng 2017) được trích trong
[community discussion #9288](https://github.com/orgs/community/discussions/9288): GitHub không xét
`.gitattributes` do người dùng đặt khi merge PR. Yêu cầu tính năng đó vẫn mở, bình luận mới nhất
ngày 2026-10-02. Vậy không thể dựa vào nó trên nhánh được bảo vệ, nơi mọi merge đi qua GitHub. Kể cả
nếu được tôn trọng, `union` sai với một bảng: hai PR cùng đổi một hàng (ví dụ ô trạng thái) sẽ để lại
**cả hai** dòng, một bảng có hàng trùng mà không ai bị báo.

**(d) Sinh và commit lại bảng trong mỗi PR, CI chạy `--check`.** Bảo đảm mạnh nhất (README luôn đủ),
nhưng hai PR sinh lại cùng chèn hàng vào cùng chỗ: vẫn xung đột, vi phạm tiêu chí 1.

**(e) Bot sinh lại sau khi merge.** Nhánh được bảo vệ từ chối push trực tiếp. Một PR do
`GITHUB_TOKEN` mở không kích hoạt workflow, nên check bắt buộc không bao giờ chạy và PR không merge
được nếu không có PAT hay GitHub App — thêm bí mật và thêm một PR cho mỗi lần merge.

**(f) — chọn — (a) giữ hàng cũ, thêm đường tới thư mục, và tự kiểm khép kín.**

| Tiêu chí | (a) thô | (b) | (c) | (d) | (e) | **(f)** |
|---|---|---|---|---|---|---|
| 1. Không sửa file chung | có | không (nếu commit bảng) | không (GitHub bỏ qua) | không | có | **có** |
| 2. GitHub thấy mọi ghi chú | danh sách thư mục, không link | có | có | có | có | **có, qua link thư mục trong README** |
| 3. Bảo đảm chỉ mục, CI cưỡng chế | yếu đi | có | có | mạnh nhất | có | **thay bằng bảo đảm tương đương, xem §5** |
| 4. `node` thuần, Win/macOS | có | có | không cần mã | có | Actions | **có, giữ CRLF/LF của file** |
| 5. PR đang mở | xanh | rebase | — | phải sinh lại | xanh | **xanh, merge sạch (đo ở §6)** |
| Giữ chữ tóm tắt lead viết | không | — | có | không | không | **có** |

## 4. Hợp đồng

Toàn bộ nằm trong `scripts/lib/docs_index.mjs`, mà cả generator và checker đều import, nên hai bên
không thể hiểu khác nhau.

- **Khối.** Một README chỉ mục tham gia bằng đúng một khối giữa một dòng bắt đầu bằng
  `<!-- docs-index:begin` và một dòng bắt đầu bằng `<!-- docs-index:end`. Trong khối là một bảng,
  đầu bảng `| Ngày | Tài liệu | Loại | Trạng thái |`, mỗi hàng có bốn ô: ngày `YYYY-MM-DD`, ô tài liệu
  mở đầu bằng một link markdown rồi tới tóm tắt, loại, và trạng thái trong dấu backtick. Hàng sai
  dạng làm `check:docs` đỏ, chỉ rõ số dòng. Hôm nay chỉ `docs/ai/README.md` có khối.
- **Phạm vi.** Một tài liệu thuộc khối của README chỉ mục gần nhất khi đi ngược lên cây, bỏ qua
  template. Với `docs/ai/<pha>/x.md` đó là `docs/ai/README.md`, vì `docs/ai/<pha>/README.md` là
  template thượng nguồn.
- **Tài liệu chờ.** Một tài liệu trong phạm vi mà không chỉ mục nào link tới được chấp nhận khi
  (1) front matter có `title`, `status`, `description`, (2) `title` và `description` không chứa link
  markdown (link tương đối sẽ sai khi chép vào README), và (3) thư mục của nó được link từ một chỉ
  mục — `docs/ai/README.md` link cả bảy thư mục pha. Thiếu một điều kiện thì đỏ như trước, với thông
  báo nói thiếu gì.
- **Sinh lại** (`npm run gen:docs-index`). Giữ nguyên chữ và thứ tự mọi hàng đã có; chỉ thay ô
  trạng thái cho khớp `status` của file; bỏ hàng có file đã mất; thêm một hàng cho mỗi tài liệu chờ,
  xếp theo (ngày, đường dẫn), ở cuối bảng. Ngày là `YYYY-MM-DD` đầu tiên trong tên file, không có thì
  `created`, rồi `last_reviewed`. Loại là `kind` nếu có (ví dụ `thẻ`, `đo`, `agent-goal`), không có
  thì tên thư mục pha. Ký tự `|` trong chữ được thoát. Kiểu xuống dòng của file được giữ, nên một
  checkout Windows với `core.autocrlf=true` không bị bẩn. `--check` in ra những gì sẽ đổi mà không
  ghi, và **không** phải cổng CI: có hàng chờ là trạng thái bình thường giữa hai PR của lead.

## 5. Bảo đảm thay thế

Trước: mọi tài liệu được link từ một chỉ mục **đã commit**.

Sau, CI cưỡng chế từng lần chạy `check:docs`:

1. Mọi tài liệu ngoài phạm vi khối: y như trước.
2. Mọi tài liệu chờ tới được từ `docs/README.md` qua README của nó và link tới thư mục của nó —
   GitHub hiển thị danh sách thư mục xếp theo tên, tức theo ngày.
3. Checker **chạy generator trong bộ nhớ** trên cây đang kiểm rồi kiểm lại: chỉ mục sinh ra phải
   không còn tài liệu chờ nào và không thêm lỗi nào. Tức là "mọi tài liệu được link từ một chỉ mục"
   đúng cho chỉ mục mà `npm run gen:docs-index` sẽ commit.

Cái mất đi: giữa hai lần sinh, hàng của ghi chú mới chưa có trong bảng. Mỗi lần chạy, kể cả khi xanh,
checker in từng tài liệu chờ và tổng số, để độ trễ đó không vô hình. Cái được thêm: ô trạng thái
trong bảng trước đây chỉ được đối chiếu bằng tay (lần cuối 2026-11-17); nay checker báo từng ô lệch
với front matter và generator sửa chúng.

Đối chứng âm mới trong `selfTest()` của checker: thiếu `description`; có link trong `description`;
thư mục không được link; khối thiếu dòng kết thúc; hàng có ô trạng thái không nằm trong backtick. Một
đối chứng sinh lại kiểm rằng hàng đã sửa chữ được giữ, chỉ ô trạng thái đổi, hàng chờ được thêm với
`|` đã thoát, chạy lần hai không đổi gì, và sau đó không còn gì chờ. Bảy đột biến của mã (bỏ từng điều
kiện ở trên, bỏ cập nhật trạng thái, không thêm hàng, nới dạng ô trạng thái) đều làm self-test thoát 2.

## 6. Tương thích

- Sinh lại trên cây hiện tại: 40 hàng giữ nguyên, 0 thêm, 0 trạng thái đổi, 0 bỏ — bảng giống hệt
  từng byte. Tập link trong bảng trước và sau PR này được so trong mô tả PR.
- Hai dòng đánh dấu khối được đặt cách dòng 85 (chỗ #151–#153 chèn hàng) bằng ít nhất một dòng không
  đổi, nên git merge chúng sạch; hàng của các PR đó nằm trong khối và đúng dạng. Kết quả merge thử
  từng PR với nhánh này nằm trong mô tả PR.
- `linkTargets` nay bỏ dấu `/` cuối của link thư mục; với link tới file thì không đổi gì.

## 7. Agent làm khác đi thế nào

- Viết ghi chú với `description:` một dòng (không link) và, nếu muốn chữ riêng ở cột Loại, `kind:`.
- **Không** thêm hàng vào `docs/ai/README.md`. Dòng uỷ quyền của thẻ chỉ cần cấp một đường dẫn: ghi
  chú kết quả.
- Vẫn chạy `npm run check:docs`; dòng "pending index row" cho ghi chú của mình là bình thường.
- Lead chạy `npm run gen:docs-index` trong PR tài liệu kế tiếp, có thể sửa lại chữ tóm tắt của hàng
  mới; generator giữ chữ đó về sau.

## 8. Giới hạn và việc chưa làm

- Bảng Feature ở đầu `docs/ai/README.md` vẫn sửa tay. Một tài liệu vòng đời mới không có ô ở đó sẽ
  là tài liệu chờ và được generator thêm vào bảng thứ hai; lead chuyển nó lên nếu muốn.
- Không có hạn cho tài liệu chờ: một hạn như vậy sẽ làm đỏ PR của agent vì việc của lead. Con số
  chờ được in ở mỗi lần chạy thay vào đó.
- Thư mục khác dưới `docs/` có thể tham gia bằng cách thêm khối vào README của nó; PR này không đổi
  README nào khác.
