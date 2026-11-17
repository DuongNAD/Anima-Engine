---
phase: monitoring
title: Đánh giá dự án theo mười hai tiêu chí, ngày 2026-11-17
description: Chấm điểm dự án trên mười hai tiêu chí sau lượt rà soát toàn bộ tài liệu, kèm căn cứ, giới hạn của cách chấm và ba việc kéo điểm lên nhanh nhất
status: completed
owner: maintainers
last_reviewed: 2026-11-17
---

# Đánh giá dự án theo mười hai tiêu chí (2026-11-17)

Chủ dự án yêu cầu bản đánh giá này trong chat ngày 2026-11-17, ở cuối lượt rà soát toàn bộ tài liệu
([`TODO.md`](../../../TODO.md), mục cùng ngày). Đây là bản ghi một thời điểm; thân của nó không được sửa
cho khớp hiện tại. Muốn chấm lại thì viết một bản mới và so hai bản.

## Đọc trước: điểm ở đây là gì và không là gì

- **Điểm là nhận định, không phải phép đo.** Theo quy ước phân loại ở
  [STATE §1.1](../../planning/STATE_OF_THE_PROJECT.md#11-phân-loại-mọi-con-số-trong-tài-liệu), các con số
  từ 0 tới 10 dưới đây không thuộc loại nào trong năm loại số của dự án, và không được trích như bằng chứng.
- **Người chấm:** một phiên Claude Code (Claude Fable 5.2), sau khi đọc tài liệu, đọc mã và đối chiếu với
  `gh pr list` trên thân cây. Phiên đó **không build, không chạy test và không chạy app**.
- **Số đo mà các nhận định dựa vào** nằm ở
  [STATE §1.h](../../planning/STATE_OF_THE_PROJECT.md#1h-đo-2026-11-17-tại-6f14124--ci-của-lần-fast-forward-main-và-một-lượt-chạy-cục-bộ-macos)
  (lượt đo tại `6f14124`, do một phiên khác thực hiện) và ở
  [ma trận tái lập đa nền tảng](../../paper/data/determinism_matrix.md). Tài liệu này trỏ tới chúng và không
  chép lại con số nào.
- **Mười hai tiêu chí và trọng số bằng nhau là lựa chọn của người chấm.** Một bộ tiêu chí khác sẽ cho một
  điểm trung bình khác.

## Bảng điểm

Thang 0–10. Cột "Căn cứ" nói điều gì nâng điểm; cột "Trừ điểm vì" nói điều gì giữ nó lại.

| # | Tiêu chí | Điểm | Căn cứ | Trừ điểm vì |
|---|---|---|---|---|
| 1 | Kiểm thử và cổng CI | 9 | Mọi gate của `ci.yml` xanh ở STATE §1.h. Gate nhắm vào chế độ hỏng thật: allow-list hai chiều cho target rỗng và test `#[ignore]`, đối chứng âm, ratchet ESLint ở 0. Hai nhánh được bảo vệ, hai job CI là bắt buộc | Cổng `just` của luồng hardening chạy `cargo test` không có `--features desktop`, nên bỏ sót các test bị gate theo feature |
| 2 | Tái lập trên một máy | 8 | Cùng seed và manifest cho cùng thế giới qua nhiều tiến trình, qua save/load và qua fork. Lịch tick là một chuỗi khai báo (F-041) | Đường chạy mặc định của app chưa tất định: uuid thật, đồng hồ thật, AI ngoài. Chưa có lượt chạy app đầy đủ nào dưới executor đa luồng |
| 3 | Tái lập giữa các máy | 4 | Đã đo, không đoán: thẻ M1 có ma trận ba hệ điều hành và tìm ra nguyên nhân (hàm toán của nền tảng lệch 1 ULP). Engine tham chiếu trùng tuyệt đối | Thế giới sống lệch giữa Windows, Linux và macOS trước tick 180, kể cả khi ghim địa hình. Tầm nhìn nhiều máy phải sống với điều này |
| 4 | Kết quả khoa học | 2 | Kỷ luật tiền đăng ký có thật: E2 chạy đúng một lần, kết quả null được báo đúng là null; E-SEL1 được đóng tại smoke thay vì chạy cho có | Chưa có thí nghiệm nào có sức mạnh thống kê. E2 null vì hiệu ứng sàn. E-SEL1 không có thí nghiệm. Cổng ra của chặng 4 chưa qua |
| 5 | Cơ chế tiến hoá trên thế giới mặc định | 3 | Tử vong, fitness sinh thái, sinh sản liên tục, vận động và các khoá thú săn – con mồi đều đã có trong mã, có gate | Tất cả tắt mặc định và mới kiểm ở đường headless. Một thế giới mặc định không có cái chết và chấm fitness bằng quãng đường (F-018, F-019). Cho tới F-042, sinh vật còn không tự di chuyển được |
| 6 | Hiệu năng có bằng chứng | 3 | Có số Criterion theo từng hệ thống và một lượt đo trong app (STATE §1.d); dụng cụ đo đã ship và có gate "đo không làm đổi quỹ đạo" | Lượt đo trong app là bản debug với 10 sinh vật. Chưa có bản release, chưa có 1.000 sinh vật. Đường GPU rò bộ nhớ |
| 7 | Sẵn sàng phát hành | 2 | Hồ sơ giấy phép, SBOM và NOTICE sinh tự động và có gate; đã chọn lối gỡ `hexf-parse` (QĐ-C = C) | Chưa có tag, chưa có bản cài. Thẻ nâng `burn` chưa làm, nên giấy phép vẫn chặn. CI không build bản cài. Ký bản cài và tự cập nhật chưa quyết (QĐ-M8) |
| 8 | Trải nghiệm người dùng | 4 | Cảnh 3D và 2D đã tốt hơn sau mười thẻ "thế giới nhìn rõ" | Chưa có người dùng nào ngoài chủ dự án. App nặng. App giữ một thế giới, không có lịch sử. `docs/tutorials/` gần như rỗng |
| 9 | Kiến trúc và nợ kỹ thuật | 6 | Hợp đồng rõ và có test; `anima-domain` giữ luật dùng chung; ba file lớn nhất vừa được tách (#99) | Sáu cặp module phụ thuộc vòng. `burn` và `bevy_ecs` cũ nhiều bản. Hai engine (tham chiếu và sống) chưa hội tụ (G2 gate #1). `decode_genotype` có bảy chỗ gọi và các kiểu của vòng đời đích chưa tồn tại |
| 10 | Tài liệu | 7 | Cấu trúc Diátaxis, metadata và link có gate; sau lượt rà 2026-11-17 các hợp đồng reference khớp mã | Trước lượt rà đó tài liệu lệch nặng (`PROJECT.md` ghi xong những thứ không có trong mã). Mười tài liệu còn quá hạn review. STATE dài hơn một nghìn rưỡi dòng. Các how-to chưa được chạy trên macOS |
| 11 | Quy trình và quản trị | 7 | Thẻ có "xong khi", verifier ngữ cảnh sạch, sổ quyết định, uỷ quyền theo thẻ | Một người quyết mọi thứ. Kế hoạch chồng kế hoạch (QĐ-M1 chưa quyết). Ngày 2026-11-17 hai phiên agent sửa chồng lên cùng bốn file. Còn 43 nhánh `codex/*` trên remote |
| 12 | Bảo mật và giấy phép | 7 | `cargo audit`, `cargo deny`, `npm audit` là gate; CSP có kiểm; tên save theo allow-list | Một văn bản giấy phép còn thiếu (`hexf-parse`). Mã mạng chỉ nối `127.0.0.1`, không xác thực, không TLS: đủ cho hôm nay, chưa đủ cho nhiều máy |

Trung bình cộng của mười hai tiêu chí: **5,2**.

## Cách đọc

Dự án mạnh ở việc chứng minh mã chạy đúng (tiêu chí 1, 2: mức 8–9) và yếu ở việc cho ra thứ mà mã đó
được xây để làm (tiêu chí 4, 7: mức 2). Khoảng cách này không phải phát hiện mới:
[roadmap 31/10 §0](../../planning/COMPLETION_ROADMAP_2026-10-31.md#0-kết-luận-trong-năm-câu) và
[kế hoạch 13/11 §3](../../planning/DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md#3-chẩn-đoán-năm-nút-thắt)
đã nói cùng điều bằng lời khác. Bảng điểm chỉ đặt một con số cạnh từng phần của nó.

Điểm thấp ở tiêu chí 3 không phải lỗi của ai: đó là một sự thật về số học dấu phẩy động đã được đo. Nó là
điểm thấp vì nó giới hạn tầm nhìn nhiều máy, và sẽ đứng yên cho tới khi có QĐ-M6.

## Ba việc kéo điểm lên nhanh nhất

| Việc | Tiêu chí nó nâng | Ai làm |
|---|---|---|
| Thí nghiệm có sức mạnh đầu tiên: thẻ S8, S6, rồi SEL2 ([kế hoạch swarm](../planning/2026-11-05-swarm-master-plan.md#thẻ-thêm-ngày-2026-11-17-từ-quyết-định-của-chủ-dự-án)) | 4 và 5 | Agent, chủ dự án duyệt bản đăng ký |
| Một lượt chạy app bản release với 1.000 sinh vật, theo [BENCHMARKING.md](../../how-to/BENCHMARKING.md). Đặt `ANIMA_FRESH_START=1`, nếu không app nạp lại autosave và đo nhầm quần thể | 6 | Chỉ chủ dự án |
| Nâng `burn` (thẻ R1c), rồi sinh lại hồ sơ giấy phép trên Windows | 7, và mở đường cho bản 0.9 | Agent, rồi chủ dự án trên Windows |

## Điều bản đánh giá này không nói

- Không nói mã đúng hay sai ở chỗ nào cụ thể. Bug đã chứng minh nằm ở [`FINDINGS.md`](../../../FINDINGS.md).
- Không so dự án với dự án nào khác; không có mốc bên ngoài nào được dùng.
- Không dự đoán khi nào một điểm sẽ đổi. Không tài liệu nào của dự án ước lượng thời gian, và bản này cũng không.
- Tiêu chí 8 được chấm mà không có một người dùng nào để hỏi. Nó là nhận định yếu nhất trong bảng.
