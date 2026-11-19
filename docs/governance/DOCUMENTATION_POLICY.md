---
title: Chính sách tài liệu
status: accepted
owner: maintainers
last_reviewed: 2026-11-14
review_cycle: quarterly
---

# Chính sách tài liệu

## Mục tiêu

Mỗi khái niệm có một nguồn sự thật, người đọc biết tài liệu nào để học/làm/tra cứu,
và thay đổi code kéo theo đúng tài liệu/test. Cấu trúc dựa trên
[Diátaxis](https://diataxis.fr/), bổ sung planning, research, decisions và governance.

## Cấu trúc

| Vị trí | Câu hỏi trả lời | Nội dung |
|---|---|---|
| `docs/tutorials/` | Tôi học hệ thống theo lộ trình thế nào? | Bài đi từ trạng thái rỗng đến kết quả |
| `docs/how-to/` | Làm một việc cụ thể thế nào? | Build, test, benchmark, tạo artifact |
| `docs/reference/` | Điều gì đang đúng? | Index tới contract/schema/API/version |
| `docs/explanation/` | Vì sao thiết kế như vậy? | Kiến trúc, mô hình, trade-off |
| `docs/planning/` | Sẽ làm gì và theo thứ tự nào? | Roadmap, task dependency, gate |
| `docs/research/` | Bằng chứng bên ngoài là gì? | Khảo sát, benchmark, nguồn khoa học |
| `docs/decisions/` | Đã quyết định gì và vì sao? | ADR bất biến, supersede thay vì viết lại |
| `docs/governance/` | Duy trì hệ thống thế nào? | Chính sách docs, dependency, release |
| `docs/ai/` | Feature đang được hiểu/thiết kế/kiểm thử/triển khai ra sao? | Lifecycle working set cho AI agent; link về contract/ADR chuẩn |

## Metadata tối thiểu

Tài liệu trong `docs/` dùng front matter:

```yaml
---
title: Tên rõ nghĩa
status: proposed | active | accepted | completed | deprecated | superseded
owner: team-or-role
last_reviewed: YYYY-MM-DD
review_cycle: weekly | monthly | quarterly | per-release
---
```

`last_reviewed` là ngày nội dung được xác minh, không phải ngày chỉnh chính tả. Thêm hay sửa
front matter mà không xác minh lại nội dung thì **không** đổi `last_reviewed`.

`review_cycle` nhận thêm một chú thích YAML khi chu kỳ còn kèm một điều kiện kích hoạt, ví dụ
`review_cycle: monthly  # và sau mỗi chặng`. Giá trị trước dấu `#` phải nằm trong bốn giá trị trên.

### Cưỡng chế bằng gate (2026-11-14)

Hai mục ở trên và quy tắc "mỗi thư mục có `README.md`" ở [Liên kết và đặt tên](#liên-kết-và-đặt-tên)
đã nằm trong chính sách từ tháng 7 nhưng chưa từng được kiểm, nên đã trôi. Đến 2026-11-14, 7 tài
liệu không có front matter, 65 thiếu `review_cycle` (phần lớn ở `docs/ai/` và ADR — nay được
miễn, lý do ở bảng dưới), 2 dùng trạng thái ngoài danh sách, 4 hợp đồng
`reference/` không có trong chỉ mục, và `docs/ai/` không có chỉ mục. Nay
`npm run check:docs` (CI: bước *Docs metadata check*) cưỡng chế:

| Nhóm | Bắt buộc |
|---|---|
| Mọi tài liệu trong `docs/` | `title`, `status`, `owner`, `last_reviewed` (ngày thật, không ở tương lai); `status` trong danh sách trên; `completed` chỉ trong `docs/ai/` |
| Ngoài `docs/ai/`, `docs/decisions/`, `docs/archive/` | Thêm `review_cycle`. `docs/ai/` là working set và ADR là bản ghi bất biến: chúng được thay bằng supersede, không được review định kỳ |
| `kind: agent-goal` và `kind: review-findings` | Bản ghi phiên: `created` thay cho `last_reviewed`; `status` là `proposed`, `active` hoặc `completed`; agent-goal `completed` cần ngày `completed` |
| `docs/archive/` | `status` là `superseded` hoặc `deprecated`, và `superseded_by` trỏ tới một file có thật |
| Mọi thư mục trong `docs/` có tài liệu | Có `README.md` làm chỉ mục |
| Mọi tài liệu | Được link từ `docs/README.md` hoặc một `docs/**/README.md` (điều kiện "Definition of done" bên dưới), hoặc là tài liệu *chờ* của một [chỉ mục sinh](#chỉ-mục-sinh-2026-11-19) |
| Mọi khối chỉ mục sinh | Đọc được (hàng đúng dạng), và sinh lại trong bộ nhớ thì không còn tài liệu chờ nào |

Được miễn: template (`docs/ai/*/README.md`, `ADR-0000-template.md`) và bốn file do agent sở hữu ở
gốc `docs/ai/`. Danh sách miễn nằm trong `NO_FRONT_MATTER` của
[`scripts/check_docs_metadata.mjs`](../../scripts/check_docs_metadata.mjs), mỗi mục kèm lý do; một
mục không còn đúng (file đã mất, hoặc đã có front matter) làm gate đỏ, nên danh sách không phình
lặng lẽ. Tài liệu quá hạn review chỉ được **báo** (`node scripts/check_docs_metadata.mjs --report`),
không làm gate đỏ — đúng với mục [Nhịp bảo trì](#nhịp-bảo-trì). Đổi quy tắc ở đây thì đổi hằng số
trong script trong cùng commit.

### Chỉ mục sinh (2026-11-19)

Trước ngày này, mỗi PR thêm một ghi chú dưới `docs/ai/` cũng thêm hàng của nó vào cùng một bảng
trong `docs/ai/README.md`, nên hai PR mở cùng lúc chèn một dòng vào cùng một chỗ và GitHub báo xung
đột ở PR thứ hai. `.gitattributes merge=union` không giúp được: GitHub không dùng merge driver của
repo khi merge PR. Thiết kế và các phương án đã loại:
[`2026-11-19-feature-generated-docs-index.md`](../ai/design/2026-11-19-feature-generated-docs-index.md).

Một README chỉ mục có thể giữ một **khối chỉ mục sinh**: một bảng
`| Ngày | Tài liệu | Loại | Trạng thái |` nằm giữa hai dòng `<!-- docs-index:begin … -->` và
`<!-- docs-index:end … -->`. Hôm nay chỉ `docs/ai/README.md` có khối. Khối phụ trách mọi tài liệu
mà README đó là chỉ mục gần nhất khi đi ngược lên cây (template không tính là chỉ mục).

| Quy tắc | Nội dung |
|---|---|
| Tài liệu *chờ* | Tài liệu trong phạm vi khối mà chưa chỉ mục nào link tới được chấp nhận khi front matter có `title`, `status`, `description`; `title` và `description` không chứa link markdown; và thư mục của nó được link từ một chỉ mục (để danh sách thư mục trên GitHub tới được nó). Thiếu một điều kiện thì đỏ |
| Sinh lại | `npm run gen:docs-index` giữ nguyên chữ và thứ tự mọi hàng đã có, chỉ cập nhật ô trạng thái theo `status`, bỏ hàng có file đã mất, và thêm một hàng cho mỗi tài liệu chờ, theo (ngày, đường dẫn). Ngày lấy từ tên file, rồi `created`, rồi `last_reviewed`; cột Loại là `kind` nếu có, không thì tên thư mục |
| Cổng | `npm run check:docs` chạy generator trong bộ nhớ và đỏ nếu chỉ mục sinh ra còn tài liệu chờ hay thêm lỗi; mỗi lần chạy nó in từng tài liệu chờ và từng ô trạng thái lệch, nhưng **không** đỏ vì chúng |
| Ai sinh lại | Lead, trong một PR tài liệu của mình (thường là PR cập nhật trạng thái sau một đợt merge). PR của agent không sửa README có khối. `node scripts/gen_docs_index.mjs --check` in những gì sẽ đổi mà không ghi |
| Sửa tay | Được sửa chữ một hàng — lần sinh sau giữ nguyên — miễn hàng giữ dạng bốn ô: ngày, link rồi tóm tắt, loại, trạng thái trong backtick. Hàng sai dạng làm cổng đỏ |

Quy tắc ở đây nằm trong `scripts/lib/docs_index.mjs`, mà cả generator lẫn
[`scripts/check_docs_metadata.mjs`](../../scripts/check_docs_metadata.mjs) đều dùng; đổi một bên thì
đổi bên kia và mục này trong cùng commit.

### `completed` và schema mở rộng cho `docs/ai/` (chuẩn hoá 2026-07-26)

Ba khác biệt dưới đây đã tồn tại trong cây tài liệu trước khi được viết ra. Ghi lại thành quy tắc
để phiên sau không "sửa" đi rồi phiên sau nữa lại sửa về.

**1. `completed` là trạng thái hợp lệ, chỉ cho `docs/ai/`.** Một bản ghi mục tiêu đã hoàn thành
không phải `accepted` (nó không phải quyết định), cũng không phải `superseded` (không có gì thay thế
nó). Ép nó vào một trong hai là mô tả sai. `completed` chỉ dùng cho tài liệu working-set trong
`docs/ai/`; tài liệu ở `reference/`, `decisions/`, `explanation/` không dùng trạng thái này.

**2. Tài liệu `kind: agent-goal` trong `docs/ai/planning/` dùng schema mở rộng:**

```yaml
---
kind: agent-goal
agent: <tên agent chạy>
feature: <slug feature>
title: ...
status: proposed | active | completed
created: YYYY-MM-DD
completed: YYYY-MM-DD      # khi status: completed
owner: team-or-role
predecessor: <tên file>    # tuỳ chọn, tạo chuỗi thời gian
---
```

`kind: review-findings` (bản ghi một đợt review độc lập) dùng cùng schema này.

Đây là **bản ghi phiên làm việc**, không phải nguồn sự thật. Khi thân tài liệu và
[`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) mâu thuẫn, tài liệu trạng thái
thắng — thân agent-goal được giữ nguyên làm bản ghi của kế hoạch **tại thời điểm đó**.

**3. `docs/ai/*/README.md` là template thượng nguồn, được miễn.** Chúng dùng schema
`phase`/`title`/`description` và thân là placeholder (`[Description]`). Chúng là khung của quy trình
lifecycle, không phải tài liệu dự án, nên không áp metadata tối thiểu và **không** cần điền. Đừng
coi chúng là tài liệu chưa hoàn thành.

## Quy tắc nguồn sự thật

1. Reference/schema/code nói **hiện tại**; planning nói **tương lai**.
2. ADR ghi **quyết định**; research giữ **bằng chứng** dẫn đến quyết định.
3. `TODO.md` chỉ giữ trạng thái tác vụ, không định nghĩa lại hợp đồng.
4. README là bản đồ và tóm tắt, không sao chép toàn bộ nội dung nguồn chuẩn.
5. Luật định lượng phải có đơn vị, invariant, test và owner.
6. Nếu code và tài liệu xung đột, không tự coi code là đúng; mở finding và xác định
   nguồn chuẩn trước khi sửa một trong hai.
7. `docs/ai/` là working set theo feature. Khi một quyết định được accepted, nguồn chuẩn vẫn là
   `docs/reference/`/ADR; lifecycle docs phải link tới nguồn đó thay vì định nghĩa contract cạnh tranh.
8. **Một con số đo chỉ có một nhà.** Trạng thái đo được của toàn dự án nằm ở
   [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)
   và **chỉ ở đó**. Tài liệu khác trỏ về, không chép lại — một con số được chép sẽ ôi mà không ai
   biết, và đó chính là chế độ hỏng đã để `841 pass` đứng cạnh một lần chạy `843` và một ledger ghi
   ESLint `open` sau khi baseline đã về 0.
9. **Phân loại trước khi viết một con số.** Năm loại, và trộn chúng là lỗi tài liệu:

   | Loại | Yêu cầu |
   |---|---|
   | **Đo hiện tại** | Phải kèm **lệnh + ngày chạy** (và cấu hình nếu có), hoặc trỏ về §1 |
   | **Đo lịch sử** | Phải nói rõ là lịch sử, kèm ngày/commit sinh ra nó. Không cập nhật số cũ trong ADR hay ledger — bản ghi lịch sử là mục đích của chúng |
   | **Tham số thiết kế / hợp đồng** | Hằng số, ngưỡng, schema version, số hiệu yêu cầu. **Không** gắn lệnh: nó không phải phép đo |
   | **Mục tiêu / ngân sách** | Ghi là *mục tiêu*, không phải kết quả đã đạt |
   | **Chính sách** | Khoản miễn trừ đã duyệt; phải có lý do ghi kèm và một gate cưỡng chế |

10. **Đừng nâng cấp một câu khi đi qua ranh giới bằng chứng.** "Instrumentation đã ship" không phải
    "đã có số đo"; "hạ tầng thí nghiệm tồn tại" không phải "có kết quả khoa học"; "headless adapter
    verified" không phải "experiment-ready"; "gate CSP xanh trên artifact" không phải "app đã khởi
    động dưới CSP đó". Giới hạn trung thực phải được giữ nguyên văn, không làm mượt.

## Ma trận thay đổi

| Khi thay đổi… | Bắt buộc xem/cập nhật |
|---|---|
| Luật tick, đơn vị, bảo toàn | `SIMULATION_RULES.md`, unit/property tests, benchmark |
| Biome hoặc điều kiện chuyển biome | `BIOME_TAXONOMY.md`, fixtures, renderer mapping |
| Tọa độ/chunk/transform | `COORDINATE_CONTRACT.md`, Rust/TS parity tests |
| Artifact/schema/save | Schema, fixture, migration, Rust/TS tests, changelog |
| Canonical map view | `MAP_MANIFEST.md`, manifest JSON, ảnh trước/sau |
| Dependency/engine | ADR, open-source inventory, benchmark, rollback |
| Milestone/phạm vi | `WORLD_SIMULATION_PLAN.md`, `TODO.md`, docs index nếu cần |
| World law, exotic source hoặc experiment schema | `EVOLUTION_EXPERIMENT_CONTRACT.md`, ADR, `SIMULATION_RULES.md` nếu đổi unit/conservation, feature lifecycle docs |

## ADR

Tạo từ [`ADR-0000-template.md`](../decisions/ADR-0000-template.md), đánh số tăng dần:
`ADR-0001-short-title.md`. ADR accepted không bị sửa để đổi quyết định; tạo ADR mới
với `supersedes` và đánh dấu ADR cũ là `superseded`.

ADR bắt buộc khi:

- thêm/thay engine, database, schema format hoặc dependency runtime lớn;
- đổi nguồn sự thật, tick order, determinism hoặc hệ tọa độ;
- chấp nhận license cần review;
- một quyết định làm thay đổi từ hai subsystem trở lên;
- bỏ tương thích artifact/save.

## Liên kết và đặt tên

- **Mỗi thư mục con của `docs/` phải có `README.md` làm index.** Đây là điều kiện để OSS-001 (mọi
  tài liệu chuẩn cách điểm vào ≤ 2 lần nhấp) còn đúng khi thêm tài liệu mới: không có index, tài
  liệu thứ hai trong một thư mục chỉ tới được nếu ai đó nhớ thêm nó vào `docs/README.md`. Bổ sung
  2026-07-26 sau khi phát hiện `research/` và `governance/` thiếu index dù mỗi thư mục đã có hai
  tài liệu.
- Dùng link tương đối trong Markdown.
- Link tới nguồn chuẩn, không tạo chuỗi alias dài.
- Tên file viết hoa kiểu hiện tại cho contract gốc; file trong `docs/` dùng tên mô tả
  nhất quán. Không đổi tên hàng loạt chỉ vì thẩm mỹ.
- Mọi index phải phân biệt `proposed` với `accepted`.
- Link web trong research phải là upstream/official; ghi ngày review.

## Di chuyển tài liệu hiện có

Các file gốc đang là nguồn chuẩn và có thay đổi đang diễn ra, nên không di chuyển
trong đợt quy hoạch này. Migration về sau làm từng nhóm:

1. Tìm incoming links và owner.
2. Di chuyển một nguồn chuẩn.
3. Để stub ở đường cũ trong ít nhất một release, trỏ tới đường mới.
4. Cập nhật toàn bộ link, script và AI context.
5. Chạy link checker và test liên quan.
6. Chỉ xóa stub khi không còn consumer được hỗ trợ.

## Definition of done cho tài liệu

- Có owner, trạng thái và ngày review.
- Có link vào từ `docs/README.md` hoặc một index con — hoặc, trong phạm vi một
  [chỉ mục sinh](#chỉ-mục-sinh-2026-11-19), có front matter đủ để sinh hàng của nó.
- Không trùng nguồn sự thật.
- Code/schema/test được liên kết hai chiều khi cần.
- Link nội bộ hợp lệ; ví dụ/lệnh đã chạy trên phiên bản hiện tại.
- Tuyên bố map quality có ảnh, region và kết quả đủ các gate bắt buộc.

## Nhịp bảo trì

- Mỗi PR: `npm run check:docs` (CI chạy sẵn) và kiểm file được ảnh hưởng theo ma trận.
- Mỗi PR tài liệu của lead: `npm run gen:docs-index`, để các hàng chờ vào bảng.
- Mỗi release: kiểm tra tutorial/how-to và artifact migration.
- Hàng quý: review research, dependency, license, ADR proposed lâu ngày, và danh sách
  `node scripts/check_docs_metadata.mjs --report`.
- Tài liệu quá hạn không tự sai, nhưng phải hiện owner và được đưa vào backlog review.
