---
title: Chính sách sử dụng nguồn mở
status: proposed
owner: maintainers
last_reviewed: 2026-08-08
review_cycle: quarterly
---

# Chính sách sử dụng nguồn mở

## License của Anima Engine — đã quyết định

Anima Engine là **nguồn mở, cấp phép kép `MIT OR Apache-2.0`** (© 2026 Duong Nguyen Anh). Hai file
[`LICENSE-MIT`](../../LICENSE-MIT) và [`LICENSE-APACHE`](../../LICENSE-APACHE) ở thư mục gốc;
[`NOTICE`](../../NOTICE) giữ attribution cho thành phần bên thứ ba được phân phối;
[`CONTRIBUTING.md`](../../CONTRIBUTING.md) đặt điều khoản inbound = outbound (Apache-2.0 §5), không
CLA.

Phạm vi: code, tài liệu và asset trong repository theo cấp phép kép, trừ khi một file ghi rõ điều
khác; định dạng `.anmw` (World Artifact) là **định dạng mở**, ai cũng có thể triển khai lại. Chi
tiết ở mục Giấy phép trong [`README.md`](../../README.md).

> **Sửa 2026-08-08 — relicense.** Từ 2026-07-26 tới 2026-08-07, mục này ghi `LICENSE` là
> proprietary, all rights reserved. Chủ sở hữu đã thay quyết định đó bằng cấp phép kép cho toàn bộ
> phần mình sở hữu trong lịch sử repository.

Tài liệu này là quy trình kỹ thuật, không phải tư vấn pháp lý.

## Phân loại mặc định

| Nhóm | License ví dụ | Xử lý |
|---|---|---|
| Cho phép sau kiểm tra | MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib | Ghi inventory, attribution/NOTICE và lock version |
| Cần review rõ ràng | MPL-2.0, LGPL, EPL, CC cho data/asset, license tùy chỉnh | ADR + xác định linking/distribution/modification |
| Chặn mặc định cho code tích hợp | GPL, AGPL, proprietary, source-available, không license | Chỉ tiếp tục sau quyết định license/pháp lý rõ ràng |

### Hệ quả của việc Anima Engine là `MIT OR Apache-2.0`

Cấp phép kép permissive không nới hàng thứ ba của bảng trên. Nó đổi lý do và nới hàng thứ hai:

- **GPL/AGPL vẫn chặn cứng cho code tích hợp.** Nhận code GPL sẽ làm tác phẩm phái sinh phải phát
  hành theo GPL và dự án không thể tiếp tục cung cấp nhánh MIT. Muốn thay đổi điều này phải có một
  quyết định relicense riêng, không phải một pull request tích hợp thông thường.
- Với các dự án đó, hạng `Reference` trong [khảo sát](../research/OPEN_SOURCE_LANDSCAPE.md) vẫn
  nghĩa là đọc bài báo và tài liệu mô tả mô hình, không phải đọc source rồi viết lại. Điều này áp
  cho ít nhất **SLiM, Avida, ALIEN và Thrive** trong danh sách ứng viên hiện tại.
- **MPL-2.0 và LGPL cần review theo từng trường hợp**, không còn mặc định bị loại chỉ vì dự án là
  proprietary. ADR phải xác định rõ linking, distribution và modification.
- Hạng `Oracle` không bị chặn khi công cụ chạy tách biệt và chỉ sinh dữ liệu. Output không mặc nhiên
  thừa hưởng license của tool — xem §"Code, model, data và asset".
- Neo4j Community Edition là GPLv3 nhưng chạy như tiến trình cài riêng qua Bolt, không được link hay
  phân phối cùng engine; fallback in-memory phải luôn hoạt động để giữ ranh giới này.
- Apache-2.0 chỉ tương thích một chiều với GPLv3; nhánh MIT giúp hạ nguồn GPLv2 vẫn có thể dùng code
  của Anima Engine.
- **Attribution vẫn bắt buộc.** [`NOTICE`](../../NOTICE) là inventory sinh tự động từ dependency
  graph được phân phối, còn văn bản license nằm trong
  [`licensing/THIRD_PARTY_LICENSES.txt`](../../licensing/THIRD_PARTY_LICENSES.txt).

  > **Sửa 2026-07-27.** Bản trước viết *"Hiện repository **chưa có file `NOTICE`**"*. Câu đó **đã
  > sai sự thật** kể từ `766609e`: [`NOTICE`](../../NOTICE) tồn tại và được sinh tự động. Trạng thái
  > đo được hôm nay:
  >
  > - [`NOTICE`](../../NOTICE) — inventory 458 thành phần (419 crate Rust · 21 gói npm có byte
  >   trong `dist/` · 18 gói cài nhưng **không** phân phối), sinh bởi `npm run gen:notice`.
  > - [`licensing/THIRD_PARTY_LICENSES.txt`](../../licensing/THIRD_PARTY_LICENSES.txt) — **văn bản
  >   license** của 439/440 thành phần được phân phối, kèm SHA-256 từng file trong
  >   [`third-party-index.json`](../../licensing/third-party-index.json).
  > - [`sbom.cdx.json`](../../sbom.cdx.json) — CycloneDX 1.5, đã **validate** với schema chính thức
  >   được vendor và ghim theo commit.
  >
  > **Cập nhật 2026-07-27 (đợt hai).** Bản trước ghi *"còn nợ 32 thành phần"*. Con số đo được hôm
  > nay là **1**. 31 thành phần còn lại đã đóng bằng cách **vendor văn bản license từ đúng commit
  > bất biến** mà bản phát hành đó được publish: 39 file, 24 commit, 19 repository, lưu nguyên byte
  > ở [`licensing/upstream/`](../../licensing/upstream/) kèm manifest chứng cứ
  > [`upstream/sources.json`](../../licensing/upstream/sources.json). Bằng chứng commit↔version lấy
  > từ `.cargo_vcs_info.json` trong `.crate` đã publish (do chính `cargo publish` ghi) và `gitHead`
  > của npm registry, không phải từ nhánh `main`. Generator đọc kho này **fail-closed**: sai hash,
  > sai commit, sai purl, symlink thoát thư mục, file không được git track, hoặc mapping thừa đều
  > làm dừng chạy. `npm run verify:upstream-licenses` tải lại từ URL đã ghim để đối chiếu byte —
  > chạy tay, không nằm trong CI.
  >
  > **Còn nợ và vẫn chặn phát hành:** **1** thành phần — `hexf-parse` 0.2.1 (CC0-1.0) — không có
  > văn bản license ở artifact **lẫn** repository tại commit phát hành, và file `LICENSE` duy nhất
  > mà dự án từng commit xuất hiện muộn hơn 3,5 năm với **license khác**. Chứng cứ tìm kiếm ghi đầy
  > đủ ở [`licensing/UNRESOLVED.md`](../../licensing/UNRESOLVED.md). Không sinh văn bản thay thế từ
  > danh sách SPDX: đóng dòng này là **quyết định pháp lý**, không phải việc của generator.
  >
  > Hai điểm cần pháp lý đọc kỹ, đã ghi rõ trong
  > [`licensing/README.md`](../../licensing/README.md): `neo4rs`/`neo4rs-macros` chỉ có **tuyên bố
  > license trong README** (dự án chưa từng publish file license), và `zune-inflate` khai
  > `MIT OR Apache-2.0 OR Zlib` nhưng upstream chỉ publish văn bản Zlib — đóng gói **mọi** file có
  > tại bản phát hành, không tự chọn hộ chủ sở hữu.

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
