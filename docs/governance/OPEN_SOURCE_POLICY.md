---
title: Chính sách sử dụng nguồn mở
status: proposed
owner: maintainers
last_reviewed: 2026-07-26
review_cycle: quarterly
---

# Chính sách sử dụng nguồn mở

## License của Anima Engine — đã quyết định

[`LICENSE`](../../LICENSE) ở thư mục gốc là **proprietary, all rights reserved**
(© 2026 Duong Nguyen Anh). Việc công khai source không tự cấp quyền sử dụng, sửa đổi hay phân phối,
kể cả với chính repository này.

> **Sửa 2026-07-26.** Bản trước của mục này viết *"Repository hiện chưa có `LICENSE` ở thư mục
> gốc"* và coi đó là blocker quản trị duy nhất. Câu đó **đã sai sự thật**: file tồn tại. Blocker
> (OSS-003) vì thế **đã được gỡ** — nhưng nó gỡ theo hướng thắt chặt, không phải nới ra. Xem hệ quả
> ở mục kế tiếp.

Phạm vi riêng cho code, model, dataset, texture, ảnh và âm thanh vẫn **chưa được tách bạch** trong
`LICENSE`; đó là phần còn nợ của OSS-003, không phải toàn bộ nó.

Tài liệu này là quy trình kỹ thuật, không phải tư vấn pháp lý.

## Phân loại mặc định

| Nhóm | License ví dụ | Xử lý |
|---|---|---|
| Cho phép sau kiểm tra | MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib | Ghi inventory, attribution/NOTICE và lock version |
| Cần review rõ ràng | MPL-2.0, LGPL, EPL, CC cho data/asset, license tùy chỉnh | ADR + xác định linking/distribution/modification |
| Chặn mặc định cho code tích hợp | GPL, AGPL, proprietary, source-available, không license | Chỉ tiếp tục sau quyết định license/pháp lý rõ ràng |

### Hệ quả của việc Anima Engine là proprietary

Hàng thứ ba của bảng trên nay có hiệu lực thật, không còn là mặc định thận trọng:

- Một sản phẩm **proprietary không phân phối được** code copyleft (GPL/AGPL) đã link vào. Với các dự
  án đó, hạng `Reference` trong [khảo sát](../research/OPEN_SOURCE_LANDSCAPE.md) nghĩa là **đọc bài
  báo và tài liệu mô tả mô hình**, không phải đọc source rồi viết lại — đường thứ hai là vùng xám
  pháp lý mà chính sách này không cho phép đi vào mà không có ý kiến pháp lý.
- Điều này áp cho ít nhất **SLiM, Avida, ALIEN và Thrive** trong danh sách ứng viên hiện tại.
- Ngược lại, hạng `Oracle` **không** bị chặn khi công cụ chạy tách biệt và chỉ sinh dữ liệu. Nhưng
  output không mặc nhiên thừa hưởng license của tool — xem §"Code, model, data và asset".
- Dự án proprietary vẫn phải giữ **attribution/NOTICE** cho mọi thành phần permissive được phân
  phối. Hiện repository **chưa có file `NOTICE`**; cần tạo trước lần phát hành đầu tiên.

Một tool AGPL chạy tách biệt trong research có rủi ro khác với thư viện link vào ứng
dụng, nhưng vẫn phải review cách triển khai, phân phối và đầu ra. Không suy luận từ
tên dự án hoặc mô tả website; kiểm tra file license của đúng phiên bản.

## Hồ sơ bắt buộc cho dependency

- Tên, upstream URL, package registry.
- Version/tag/commit và ngày kiểm tra.
- SPDX expression và đường dẫn license.
- Loại: runtime, build, dev-only, offline tool, copied code, model, data, asset.
- Điểm tích hợp và owner.
- Code đã sửa/copy, attribution và NOTICE cần thiết.
- Transitive dependencies đáng chú ý.
- CVE/advisory và nguồn phát hành.
- Test, benchmark, feature flag và cách gỡ.
- Chính sách update: cadence, version range và migration.

Hồ sơ có thể bắt đầu trong ADR; khi tooling ổn định, sinh inventory tự động từ
`Cargo.lock` và `package-lock.json`, bổ sung inventory thủ công cho data/asset/model.

## Quy trình nhận thành phần

1. **Discover:** xác nhận upstream chính thức, hoạt động duy trì và license đúng tag.
2. **Classify:** xác định integration type; asset/data tách khỏi code.
3. **Minimize:** ưu tiên adapter nhỏ hoặc dev/offline tool hơn dependency runtime.
4. **Baseline:** tạo fixture và đo đường hiện tại trước pilot.
5. **Pilot:** lock version, feature flag, không để type vendor lan qua domain.
6. **Verify:** correctness, determinism, performance, memory, binary size, security.
7. **Record:** ADR, inventory, NOTICE/credits và tài liệu cập nhật.
8. **Maintain:** audit hàng quý/release; có owner cho migration.
9. **Remove:** giữ khả năng đọc dữ liệu cũ hoặc cung cấp migration trước khi gỡ.

## Code, model, data và asset

- License của **code** không tự bao phủ checkpoint/model weight.
- Dataset có thể có điều khoản attribution, non-commercial hoặc privacy riêng.
- Texture, mesh, âm thanh và screenshot phải lưu nguồn, tác giả, license, thay đổi và
  phạm vi phân phối.
- Output của một tool không mặc nhiên có cùng license với tool; kiểm tra điều khoản và
  input data.
- Không commit package cache, virtual environment hoặc dữ liệu không có provenance.

## Kiểm soát tự động đề xuất

- Rust: `cargo-deny` cho advisory, bans, licenses và sources.
- JavaScript: lockfile review và công cụ audit hiện có; không auto-fix major version
  trong cùng PR tính năng.
- Tài liệu: lychee kiểm tra link.
- Release: sinh SBOM khi pipeline phát hành được thiết lập.
- Renovation/update bot chỉ mở PR; benchmark và compatibility test vẫn là gate.

## Sự cố license hoặc bảo mật

Khi phát hiện license sai, package bị chiếm quyền, CVE nghiêm trọng hoặc upstream biến
mất:

1. Đóng băng release chứa thành phần nếu còn phân phối.
2. Xác định version, đường phụ thuộc và artifact bị ảnh hưởng.
3. Tắt feature/adapter nếu có đường hoàn tác an toàn.
4. Nâng cấp, thay thế hoặc loại bỏ trong PR độc lập.
5. Cập nhật SBOM/inventory/NOTICE và ghi ADR hoặc incident note.
6. Kiểm chứng rằng save/artifact cũ còn đọc được hoặc có migration.

Các ứng viên đang đánh giá được ghi tại
[`OPEN_SOURCE_LANDSCAPE.md`](../research/OPEN_SOURCE_LANDSCAPE.md).
