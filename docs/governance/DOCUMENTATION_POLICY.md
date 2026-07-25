---
title: Chính sách tài liệu
status: accepted
owner: maintainers
last_reviewed: 2026-07-24
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
status: proposed | active | accepted | deprecated | superseded
owner: team-or-role
last_reviewed: YYYY-MM-DD
review_cycle: monthly | quarterly | per-release
---
```

`last_reviewed` là ngày nội dung được xác minh, không phải ngày chỉnh chính tả.

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
- Có link vào từ `docs/README.md` hoặc một index con.
- Không trùng nguồn sự thật.
- Code/schema/test được liên kết hai chiều khi cần.
- Link nội bộ hợp lệ; ví dụ/lệnh đã chạy trên phiên bản hiện tại.
- Tuyên bố map quality có ảnh, region và kết quả đủ các gate bắt buộc.

## Nhịp bảo trì

- Mỗi PR: kiểm tra link nội bộ và file được ảnh hưởng theo ma trận.
- Mỗi release: kiểm tra tutorial/how-to và artifact migration.
- Hàng quý: review research, dependency, license, ADR proposed lâu ngày.
- Tài liệu quá hạn không tự sai, nhưng phải hiện owner và được đưa vào backlog review.
