---
title: Chính sách sử dụng nguồn mở
status: proposed
owner: maintainers
last_reviewed: 2026-11-17
review_cycle: quarterly
---

# Chính sách sử dụng nguồn mở

## License của Anima Engine — đã quyết định

Anima Engine là **nguồn mở, cấp phép kép `MIT OR Apache-2.0`** (© 2026 Duong Nguyen Anh). Hai file
[`LICENSE-MIT`](../../LICENSE-MIT) và [`LICENSE-APACHE`](../../LICENSE-APACHE) ở thư mục gốc;
[`NOTICE`](../../NOTICE) giữ attribution cho thành phần bên thứ ba đi kèm bản build nhị phân;
[`CONTRIBUTING.md`](../../CONTRIBUTING.md) đặt điều khoản inbound = outbound (Apache-2.0 §5), không
CLA.

Phạm vi: code, tài liệu và asset đều theo cấp phép kép; định dạng `.anmw` (World Artifact) được
tuyên bố là **định dạng mở**, ai cũng triển khai lại được. Chi tiết ở mục Giấy phép trong
[`README.md`](../../README.md).

> **Sửa 2026-08-08 — relicense.** Từ 2026-07-26 tới 2026-08-07, mục này viết `LICENSE` là
> **proprietary, all rights reserved**, và cả chính sách dưới đây được suy ra từ đó. Chủ sở hữu đã
> đổi sang cấp phép kép; `LICENSE` proprietary bị xoá. Việc này hợp lệ với **toàn bộ lịch sử commit**
> chứ không chỉ code từ nay về sau, vì dự án chỉ có một tác giả duy nhất giữ toàn bộ copyright, không
> có contributor ngoài nào cần xin phép. Hai mục tiếp theo đã được viết lại theo license mới.
>
> Bản trước nữa của mục này còn viết *"Repository hiện chưa có `LICENSE` ở thư mục gốc"* — câu đó
> sai sự thật ngay khi viết và đã được sửa 2026-07-26.

Tài liệu này là quy trình kỹ thuật, không phải tư vấn pháp lý.

## Phân loại mặc định

| Nhóm | License ví dụ | Xử lý |
|---|---|---|
| Cho phép sau kiểm tra | MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib | Ghi inventory, attribution/NOTICE và lock version |
| Cần review rõ ràng | MPL-2.0, LGPL, EPL, CC cho data/asset, license tùy chỉnh | ADR + xác định linking/distribution/modification |
| Chặn mặc định cho code tích hợp | GPL, AGPL, proprietary, source-available, không license | Chỉ tiếp tục sau quyết định license/pháp lý rõ ràng |

### Hệ quả của việc Anima Engine là `MIT OR Apache-2.0`

Cấp phép kép permissive **không** nới hàng thứ ba của bảng trên. Nó đổi lý do, và nới hàng thứ hai:

> *Sửa 2026-11-17.* Danh sách dưới đây từng mở đầu bằng bốn gạch đầu dòng viết **trước** relicense,
> nói về "một sản phẩm proprietary", nằm song song với bản viết lại ngày 2026-08-08 sau một lần gộp
> nhánh. Bốn gạch đó đã được gỡ: nội dung của chúng có đủ trong bản viết lại, với lý do đúng cho một dự
> án permissive. Ghi chú bằng chứng ngày 2026-07-27 được giữ nguyên, nay nằm dưới gạch "Attribution".

- **GPL/AGPL vẫn chặn cứng cho code tích hợp.** Lý do nay không còn là "proprietary không phân phối
  được copyleft", mà là: dự án **cam kết** cho người dùng chọn MIT. Link một thư viện GPL vào thì tác
  phẩm phái sinh phải phát hành theo GPL, tức là **không thể** tiếp tục chào MIT nữa — lời hứa trong
  `LICENSE-MIT` sẽ thành lời hứa không giữ được. Muốn nhận code GPL thì phải relicense **cả dự án**
  sang GPL; đó là một quyết định ADR, không phải một pull request.
- Với các dự án đó, hạng `Reference` trong [khảo sát](../research/OPEN_SOURCE_LANDSCAPE.md) vẫn
  nghĩa là **đọc bài báo và tài liệu mô tả mô hình**, không phải đọc source rồi viết lại — đường thứ
  hai là vùng xám pháp lý mà chính sách này không cho phép đi vào mà không có ý kiến pháp lý. Áp cho
  ít nhất **SLiM, Avida, ALIEN và Thrive** trong danh sách ứng viên hiện tại.
- **Hàng thứ hai (MPL-2.0, LGPL) nay dễ hơn thật.** Copyleft ở mức file (MPL-2.0) hoặc ở mức thư
  viện liên kết động (LGPL) tương thích được với một dự án permissive theo cách chúng không tương
  thích được với proprietary. Vẫn cần ADR xác định linking/distribution/modification — nhưng câu trả
  lời không còn mặc định là "không".
- Hạng `Oracle` **không** bị chặn khi công cụ chạy tách biệt và chỉ sinh dữ liệu. Output không mặc
  nhiên thừa hưởng license của tool — xem §"Code, model, data và asset".
- **Neo4j là tiền lệ đúng của ranh giới tiến trình.** Neo4j Community Edition là GPLv3; engine nói
  chuyện với nó qua giao thức Bolt tới một tiến trình cài riêng, không link. Vì thế GPLv3 không lan
  sang mã dự án. Điều kiện để lập luận này đứng vững là **fallback in-memory phải luôn chạy được** —
  ngày Neo4j trở thành bắt buộc, nó thành một cuộc thảo luận khác.
- **Attribution vẫn bắt buộc, và hồ sơ của nó sinh tự động.** [`NOTICE`](../../NOTICE),
  [`licensing/THIRD_PARTY_LICENSES.txt`](../../licensing/THIRD_PARTY_LICENSES.txt) và
  [`sbom.cdx.json`](../../sbom.cdx.json) đều do script sinh ra và có gate CI
  (`npm run check:compliance`); không file nào được sửa tay. Chúng mô tả bản build Windows, nên phải
  sinh lại trên Windows ([`FINDINGS.md`](../../FINDINGS.md) F-025). Số thành phần hiện hành nằm ở
  [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).

  *Sửa 2026-11-17:* gạch này từng viết rằng `NOTICE` "được duy trì thủ công" và chỉ liệt kê dependency
  trực tiếp. Câu đó sai từ `766609e`, và mâu thuẫn với chính ghi chú 2026-07-27 dưới đây. Phần còn nợ
  thật của OSS-004 là inventory cho **model / dataset / asset**.

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

  *Cập nhật 2026-11-19:* dòng còn nợ cuối cùng — lúc này là `utf8-chars` 3.0.7, vì `hexf-parse` đã
  rời đồ thị phụ thuộc ở R1c — được đóng bằng cách **chọn Apache-2.0** trong biểu thức
  `MIT OR Apache-2.0` mà chính crate khai. Crate không có file license, NOTICE hay dòng copyright
  nào, ở bản đã publish lẫn ở commit phát hành, nên không có gì để vendor. MIT đòi tái hiện thông báo
  copyright của chủ sở hữu, mà thông báo đó không tồn tại; Apache-2.0 (§4) chỉ đòi kèm văn bản chuẩn
  của License cùng các thông báo và file NOTICE mà tác phẩm có, ở đây không có gì, nên chọn
  Apache-2.0 thoả mọi điều kiện mà không bịa ra chữ nào. Văn bản đóng gói là bản Apache-2.0 mà các
  crate khác trong inventory đã ship nhiều nhất, trùng từng byte, phần phụ lục để trống nên không ghi
  tên chủ sở hữu nào. Cơ chế hẹp và fail-closed (mục `elections` trong
  [`upstream/sources.json`](../../licensing/upstream/sources.json), kiểm tra ở
  `scripts/lib/upstream_licenses.mjs`, chi tiết ở [`licensing/README.md`](../../licensing/README.md)):
  biểu thức phải là phép `OR` thuần; chỉ được chọn Apache-2.0; không trùng với mapping hay bản ghi
  `blocked`; artifact đã cài không được có file license nào, vì văn bản đã cài luôn thắng; hash văn
  bản được ghim. [`licensing/UNRESOLVED.md`](../../licensing/UNRESOLVED.md) còn **0** dòng. Chủ dự án
  giao việc này ngày 2026-11-19 (QD-C2 trong
  [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md)). Đây là bản ghi một lựa chọn kỹ thuật mà
  license đã khai cho phép, **không phải tư vấn pháp lý**.

- Apache-2.0 chỉ tương thích **một chiều** với GPLv3: hạ nguồn GPLv3 dùng được code Apache-2.0, chiều
  ngược lại thì không. Nhánh MIT là thứ giữ cho hạ nguồn **GPLv2** cũng dùng được.

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

## Kiểm soát tự động

Mục này từng là danh sách **đề xuất**. Đối chiếu `.github/workflows/ci.yml` và `package.json` ngày
2026-11-17, phần lớn đã thành gate thật; cột cuối nói cái nào còn là đề xuất.

| Phạm vi | Kiểm soát | Trạng thái |
|---|---|---|
| Rust — advisory | `cargo audit`, cấu hình ở `src-tauri/.cargo/audit.toml` | Gate CI |
| Rust — license, bans, sources | `cargo deny check`, allow-list ở `src-tauri/deny.toml` | Gate CI từ 2026-08-08 |
| JavaScript — advisory | `npm run check:audit` (`scripts/check_npm_audit.mjs`): `npm audit` ở cả hai gói npm, ngưỡng `high` | Gate CI. Một advisory mức high hoặc critical chỉ được miễn khi `scripts/npm_audit_policy.mjs` nêu tên nó kèm lý do, và gate kiểm lại ở mỗi lượt rằng nó vắng khỏi `npm audit --omit=dev` (phần được ship). Không auto-fix major version trong cùng PR tính năng |
| Hồ sơ bên thứ ba | `NOTICE`, văn bản license, SBOM và schema của nó, ranh giới bundle; cục bộ gom trong `npm run check:compliance` | Gate CI, mỗi kiểm tra là một bước riêng. Hồ sơ mô tả bản build Windows và phải sinh lại trên Windows |
| Văn bản license vendor từ upstream | `npm run verify:upstream-licenses` | Chạy tay, cần mạng; **không** nằm trong CI |
| Link tài liệu | `scripts/check_docs_links.mjs`, trong `npm run check:docs` | Gate CI. Thay cho lychee mà bản trước của mục này đề xuất |
| Bot cập nhật dependency | Chỉ mở PR; benchmark và compatibility test vẫn là gate | **Vẫn là đề xuất**; repo chưa cấu hình bot nào |

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
