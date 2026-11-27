---
title: Kế hoạch định hướng và điều phối — từ một cây xanh tới thế giới nhiều máy
status: proposed
owner: maintainers
last_reviewed: 2026-11-26
review_cycle: monthly  # và sau mỗi chặng
---

# Kế hoạch định hướng và điều phối Anima Engine

> **Vai trò của tài liệu này.** Nó **không** thay `EXECUTION_PLAN_2026-10-31.md`, brief 31/10 hay
> kế hoạch tổng cho swarm ngày 05/11. Ba tài liệu đó nói *làm việc gì và làm thế nào*. Tài liệu
> này nói *đi tới đâu, theo thứ tự nào, ai quyết gì, và cái gì phải dừng lại*. Nó cũng thêm ba
> phần mà chưa kế hoạch nào có: làm việc trên nhiều máy, người dùng đầu tiên, và cách giữ cho
> chính các kế hoạch khỏi chồng lên nhau.
>
> **Cách lập.** Đọc tài liệu và mã, chỉ đọc, ngày 2026-11-13, trên `feat/ecological-selection` @
> `bd89451`, `origin/main` @ `a43872c` và `origin/claude/funny-thompson-jj42fe`. Không lệnh build
> hay test nào được chạy. Tài liệu này **không mang số đo nào của riêng nó**: mọi số đo nằm ở
> [STATE_OF_THE_PROJECT.md §1](STATE_OF_THE_PROJECT.md).
>
> **Bản 2, cùng ngày.** Sửa theo
> [bản tổng hợp nghiên cứu 13/11](../research/RESEARCH_SYNTHESIS_2026-11-13.md): chặng 4, 5 và 7,
> ba luồng ở §6, và thêm QĐ-M6 đến QĐ-M8. Chặng 0 đến 3 không đổi; bảng hai tuần đầu chỉ thêm một
> ghi chú. Bản tổng hợp là tài liệu nghiên cứu, không phải một kế hoạch mới (L6, L7).
>
> **Ghi chú 2026-11-17 — tiến độ không nằm ở đây.** Tài liệu này giữ thứ tự chặng, cổng ra và luật; nội
> dung của nó không đổi. Mỗi chặng đang ở đâu được ghi ở
> [STATE §3.00, bảng tiến độ theo chặng](STATE_OF_THE_PROJECT.md#300-việc-kế-tiếp--cập-nhật-2026-11-17);
> trạng thái từng thẻ ở [`AGENT_WORKFLOW.md` §3](../how-to/AGENT_WORKFLOW.md#3-hàng-đợi-thẻ); quyết định đã
> ra và còn trống ở [roadmap 31/10 §4](COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được).
> §2 (hiện trạng) và §11 (hai tuần đầu) là ảnh chụp ngày 2026-11-13. Ba chỗ của tài liệu đã hết đúng, đều
> do hai cây hợp nhất ngày 2026-11-16: ở §12, mâu thuẫn "roadmap 16/10 không có trên thân cây"; ở Phụ lục
> A, dòng "chỉ trên `main`" của roadmap 16/10 và dòng "nhánh chưa gộp" của kế hoạch swarm. Cả hai tài liệu
> đó nay có trên thân cây.

## 0. Tóm tắt một trang

**Đích đến có ba tầng**, đều đã được viết ra trong repo:

1. Một **thế giới sống giải thích được** trên một máy, kèm phòng thí nghiệm tiến hoá
   ([WORLD_SIMULATION_PLAN.md](../../WORLD_SIMULATION_PLAN.md) §0, ADR-0004).
2. Một **kết quả khoa học đầu tiên có sức mạnh**, tiền đăng ký, chạy đúng một lần.
3. Một **thế giới nhiều thiết bị cùng nuôi**: máy nào kết nối thì nhận một đến vài sinh vật
   ([thiết kế tính toán phân tán](../ai/design/2026-08-25-feature-federated-compute.md)).

**Vấn đề hôm nay không phải thiếu kế hoạch.** Repo có ít nhất sáu tài liệu kế hoạch ở trạng thái
`proposed`. Vấn đề là năm nút thắt ở §3: chưa có một cây xanh, quá nhiều việc dở, quyết định
đang treo, việc chỉ con người làm được bị dồn lại, và kế hoạch chồng kế hoạch.

**Thứ tự không đổi:** xanh → gộp → đo lại → khoa học → phát hành → nhiều máy.

**Tám chặng** (§5), mỗi chặng có một cổng ra đo được. Chặng 0 và 1 không cần viết tính năng nào.

**Hai tuần đầu** (§11): chủ dự án ra sáu quyết định, đóng hoặc ghi lý do cho mười PR đang mở,
và swarm chỉ làm đúng hai việc: `B3b`, rồi `HT-C`.

**Nghiên cứu 13/11 đổi ba điều**, đều ở nửa sau của lộ trình:

- Thế giới nhiều máy đi theo **lô** (`Contribute`) và **đảo** (`Inhabit`), không theo lockstep
  từng nhịp (chặng 7, QĐ-M7).
- Giống nhau giữa các hệ điều hành là **một quyết định có giá**, không phải mặc định (QĐ-M6).
- Bản đầu tiên ra mắt **trên một máy**, kèm thế giới mẫu; nhiều máy không phải điều kiện để có
  người dùng (chặng 5, luồng C).

## 1. Đích đến, bằng lời của chính dự án

| Tầng | Nội dung | Tài liệu gốc | Người tham gia là ai |
|---|---|---|---|
| 1. Thế giới giải thích được | Lát cắt lưu vực → đồng cỏ → thỏ → sói. Tác động vào một tầng thì hiệu ứng phải lan qua cơ chế, bảo toàn, ghi nhân quả, phát lại được từ seed, rẽ nhánh được | `WORLD_SIMULATION_PLAN.md` §0, `SIMULATION_RULES.md` | Người làm thí nghiệm (World Lab) |
| 2. Công cụ nghiên cứu | Thí nghiệm là một manifest. Hai kiểu rẽ nhánh: từ khởi nguyên và từ checkpoint. Kết quả tự mô tả | `EVOLUTION_EXPERIMENT_CONTRACT.md`, ADR-0002 (`proposed`) | Nhà nghiên cứu |
| 3. Thế giới trải nghiệm | Người xem là một can thiệp có khai báo: `Absent`, `Spectate`, `Inhabit` | ADR-0004 (`accepted`) | Người quan sát |
| 4. Thế giới nhiều thiết bị | `FederationPolicy`: `Solo`, `Contribute` (chỉ góp CPU, phải cho kết quả bằng `Solo`), `Inhabit` (sinh vật đến và đi cùng thiết bị) | Thiết kế tính toán phân tán 25/08 (`proposed`), nhánh F của kế hoạch thế giới sống | Người góp máy |

Ý "mỗi người đóng vai một sinh vật" chính là chính sách `Inhabit` với
`min_agents_per_device = 1`. Thiết kế 25/08 đã nói rõ: nó vui hơn, và nó là một **treatment
riêng**, không được đem so với đường tham chiếu.

Tài liệu yêu cầu của thế giới sống ghi: đây là dự án nghiên cứu, không phải sản phẩm thương mại.
Kế hoạch này giữ nguyên điều đó.

## 2. Hiện trạng

Bảng này chỉ trỏ nguồn. Muốn con số thì mở nguồn.

| Mặt | Tình trạng ngày 2026-11-13 | Nguồn |
|---|---|---|
| Thân cây | `feat/ecological-selection` là nền, quyết ngày 2026-11-01 (QĐ-A). Nó đi trước `origin/main` 108 commit và đi sau 33 commit | `COMPLETION_ROADMAP_2026-10-31.md` §4; `git rev-list --left-right --count origin/main...HEAD`, 2026-11-13 |
| CI | Lượt CI gần nhất của `main`, ngày 2026-10-21, đỏ. Chồng PR #38 → #41 đỏ cả bốn. Không nhánh nào được bảo vệ, không gate nào là bắt buộc | Kế hoạch swarm 05/11 §1; `.github/workflows/ci.yml` |
| Pull request | Mười PR đang mở. Từ cuối tháng 7 tới nay chỉ một PR được gộp | `gh pr list`, 2026-11-13 |
| Việc ngoài kế hoạch | Một bản gộp HT-C đã làm sẵn trên `claude/funny-thompson-jj42fe`, thiếu phép đo checksum mà brief yêu cầu. Nhánh `claude/busy-hamilton-mn42mw` xoá tính năng thay vì sửa. Khoảng 40 nhánh `codex/*`. Mười file test chưa commit trong cây làm việc | `git branch -r`, `git status`, 2026-11-13 |
| Khoa học | Chỉ có E2, kết quả null do hiệu ứng sàn. Sinh thái chưa nối tới chọn lọc trên `main`; đã sửa trên thân cây sau khoá manifest | `STATE_OF_THE_PROJECT.md` §3.1, §3.19; F-018 |
| Phát hành | Chưa có tag, chưa có bản cài. `hexf-parse` thiếu văn bản giấy phép và đang chặn | `licensing/UNRESOLVED.md` |
| Hiệu năng | Mới có số debug với 10 agent và số Criterion theo từng hệ. Chưa có lượt release nào, chưa có 1.000 agent | `STATE_OF_THE_PROJECT.md` §1.d |
| Tất định | Đạt giữa các tiến trình ở chế độ tất định và trên adapter không cửa sổ. Đường đa luồng mặc định chưa phát lại được. Giữa các máy: hợp đồng **không hứa** | `DETERMINISM_CONTRACT.md` §1, §5 |
| Nhiều máy | Mã mạng hiện tại chỉ nối `127.0.0.1`, không xác thực, không TLS. Của nhánh F mới có lát F1 (ngăn `Dormant`) | `core/networking_systems.rs`; kế hoạch thế giới sống §5 |
| Nền tảng | Windows là đích. Thân cây build và test được trên macOS arm64. Linux mới có CI phần giao diện | `COMPLETION_ROADMAP_2026-10-31.md`; F-028 |
| Tài liệu | Bản `STATE` trên thân cây cũ hơn bản trên `main`. `SNAPSHOT_CONTRACT.md` dừng ở schema 3 trong khi mã ở schema 7. `TODO.md` là nhật ký, không phải backlog | So sánh hai nhánh; `TODO.md` dòng 3–13 |

## 3. Chẩn đoán: năm nút thắt

1. **Chưa có một cây xanh duy nhất.** Mọi phép đo, mọi thí nghiệm và mọi bản phát hành đều cần
   một cây mà mọi gate đều xanh. Hiện có hai cây lệch nhau và cả hai đều đỏ.
2. **Việc dở không có trần.** Mười PR mở, khoảng 40 nhánh, một bản gộp làm ngoài kế hoạch, một
   lô test chưa commit. Mỗi thứ là một món nợ phải đọc lại trước khi tin.
3. **Quyết định của chủ dự án đang treo.** QĐ-A′, QĐ-B, QĐ-C và QĐ-1 chặn gần như mọi thẻ Rust
   và toàn bộ việc phát hành.
4. **Việc chỉ con người làm được bị dồn lại.** Chạy app, đo bản release, tạo lại hồ sơ giấy
   phép trên Windows, duyệt tiền đăng ký, gộp PR.
5. **Kế hoạch chồng kế hoạch.** Phần lớn bốn mươi commit gần nhất trên thân cây là tài liệu kế
   hoạch và ghi nhận. Các tài liệu mâu thuẫn nhau ở vài chỗ nhỏ, và chưa tài liệu nào ở trạng
   thái `accepted`.

Nút thắt 1 chặn kỹ thuật. Bốn nút còn lại là về điều phối, và §4 xử lý chúng.

## 4. Luật điều phối

Mục này chỉ thêm những luật chưa có. Các luật đã có vẫn giữ nguyên: bất biến của
`SIMULATION_RULES.md`, quy tắc bằng chứng của `EXECUTION_PLAN`, luật song song ở §5 của kế hoạch
swarm.

| # | Luật | Vì sao |
|---|---|---|
| L1 | **Một thân, một hàng đợi.** PR mới chỉ nhắm vào thân cây. Không chồng PR quá hai tầng | Chồng bốn tầng #38 → #41 đỏ dây chuyền |
| L2 | **Trần việc dở.** Tối đa ba PR mã mở cùng lúc. PR mở quá 14 ngày phải có một dòng quyết định | Nút thắt 2 |
| L3 | **Đỏ thì dừng.** Khi thân cây đỏ, không thẻ tính năng nào chạy. Chỉ chạy thẻ làm xanh | Nút thắt 1 |
| L4 | **Ô quyết định trống thì không chạy.** Agent không chọn thay chủ dự án | Đã có tiền lệ: B1 tự chọn giả thuyết sai |
| L5 | **Việc ngoài kế hoạch là đề xuất.** Nhánh do agent tự làm không được gộp thẳng. Nó phải qua đúng các gate của thẻ tương ứng, do một verifier ngữ cảnh sạch chạy lại | Bản gộp trên `claude/funny-thompson-jj42fe` |
| L6 | **Mỗi vai một tài liệu.** `STATE` giữ số đo. Tài liệu này giữ thứ tự chặng. Kế hoạch swarm giữ danh sách thẻ. Brief giữ prompt. ADR giữ quyết định. `docs/research/` giữ điều tìm thấy từ bên ngoài, kèm nhãn đã kiểm hay chưa | Nút thắt 5 |
| L7 | **Một PR mã cho mỗi tài liệu kế hoạch mới.** Trước khi thêm một tài liệu kế hoạch, phải có ít nhất một PR mã được gộp kể từ tài liệu trước | Nút thắt 5 |
| L8 | **Việc của con người có hàng đợi riêng** (§8), và mỗi chặng ghi rõ nó chờ việc nào | Nút thắt 4 |
| L9 | **Build nặng không chạy trên máy phát triển** khi đã có máy thứ hai (luồng NM, §6) | Giới hạn máy ghi trong `README.md` |

## 5. Lộ trình theo chặng

Mỗi chặng có một cổng ra. Chưa qua cổng thì không sang chặng sau. Định danh trong cột "Việc" là
định danh gốc của các tài liệu đã có; tài liệu này không đặt lại tên cho chúng.

### Chặng 0 — Dọn bàn và quyết định

Không có agent nào viết mã ở chặng này. Nó tương ứng Đợt 0 của kế hoạch swarm.

| ID | Việc | Ai | Xong khi |
|---|---|---|---|
| MP-0.1 | Ra các quyết định chặn chặng 1 và 2: QĐ-A′, QĐ-B, QĐ-F028 | 🧑 | Mỗi quyết định là một dòng có ngày trong `COMPLETION_ROADMAP_2026-10-31.md` §4 |
| MP-0.2 | Ra các quyết định chặn phát hành: QĐ-C, QĐ-1. Ra QĐ-EN để hết nhập nhằng tên E3 | 🧑 | Như trên |
| MP-0.3 | Xử lý mười PR mở: đóng #38 và #39 nếu đã nằm trong `fix/ci-gates-green`; đóng hoặc ghi lý do cho #30, #31, #34 đến #37 (thẻ D0, H1 và H2 của kế hoạch swarm) | 🧑, 🤖 chỉ đọc | Không PR nào mở mà thiếu một dòng quyết định |
| MP-0.4 | Kiểm kê nhánh: bảng một dòng cho mỗi nhánh `codex/*` và `claude/*`: giữ, đóng, hay lấy lại ý | 🤖 chỉ đọc, 🧑 duyệt | Bảng nằm trong `docs/ai/planning/` |
| MP-0.5 | Cây làm việc: mười file test chưa commit và phần thêm vào `FINDINGS.md` hoặc thành một nhánh có PR, hoặc bị bỏ | 🧑 | `git status` sạch |
| MP-0.6 | Chốt chồng kế hoạch: đưa kế hoạch swarm 05/11 vào thân cây bằng một PR tài liệu; sửa các mâu thuẫn đã biết (§12); cập nhật `docs/planning/README.md` | 🤖, 🧑 gộp | README liệt kê đúng một tài liệu cho mỗi vai của L6 |

**Cổng ra:** không còn ô quyết định trống cho chặng 1 và 2; mọi PR mở đều có trạng thái.

### Chặng 1 — Một cây xanh

| Việc | Nguồn | Ghi chú |
|---|---|---|
| `B3b`: năm gate mà PR #40 làm đỏ | Brief 31/10; thẻ T1 | Việc mã duy nhất của chặng |
| `A7`: tạo lại `NOTICE` và hồ sơ giấy phép (việc con người H1 của `EXECUTION_PLAN`) | `EXECUTION_PLAN` HT-A | Phải làm trên Windows (F-025) |
| Icon thật thay file `placeholder` | QĐ-F028; F-028 | Điều kiện để Linux và macOS biên dịch |

**Cổng ra:** CI xanh cả hai job trên thân cây. Ngay lúc đó bật bảo vệ nhánh và đặt hai job làm
gate bắt buộc. Đây là thay đổi cấu hình GitHub, do chủ dự án làm.

### Chặng 2 — Gộp hai thân

Đây là `HT-C` (C1 đến C5), thẻ T2. Ba điều kiện lấy từ phép đo bề mặt gộp ngày 01/11:

- Một PR chỉ để gộp, sau khi `B3b` đã vào.
- Checksum 180 tick trên ít nhất sáu tiến trình, ở ba nhánh, dán nguyên output vào PR.
- Một test "ghi có đúng một lần" cho nhánh chết đói, vì lịch sau gộp đăng ký hai hệ mortality
  (QĐ-A′ quyết giữ cái nào).

Bản gộp đã có trên `claude/funny-thompson-jj42fe` được dùng làm **tham khảo** theo L5. Ba file
của chủ dự án (`PROJECT.md`, `AGENTS.md`, `STATE`) do chủ dự án giải quyết xung đột.

**Cổng ra:** `C5` — đo lại toàn bộ và ghi vào `STATE` §1.g. Từ đây chỉ còn một nhánh dài hạn.

### Chặng 3 — Trả nợ sau gộp

Đợt 2 của kế hoạch swarm: `B2` (cấp phát trên đường tick), `B5` (khôi phục `test:e2e` đầy đủ),
`B6a` đến `B6c` (giá trị bịa, lệnh IPC chết, đường vòng qua sổ năng lượng), `G6` đến `G9`.

**Cổng ra:** thẻ `R0` — đo lại. Không còn giá trị bịa, không còn test bị nới, `test:e2e` chạy đủ.

### Chặng 4 — Vòng khoa học đầu tiên

| Việc | Nguồn | Ghi chú |
|---|---|---|
| Quần thể luân chuyển, biến quan sát | `HT2.1`; thẻ S1, S2 | Cần QĐ-4, QĐ-5 |
| Gate G2 số 1: đường sống phát lại được | `HT2.3`; thẻ S3 | `TODO.md` gọi đây là "thực chất là P0" |
| Ma trận đa nền tảng, đối chứng âm, quy mô, residual EU | Thẻ M1 đến M4 | M1 dùng chung kết quả với NM-2 (§6) |
| Một lượt app release với 1.000 agent | `HT4.1`; thẻ H6, H7 | 🧑, theo `docs/how-to/BENCHMARKING.md` |
| E2R và E3: tiền đăng ký, duyệt, chạy một lần | `HT2.2`; thẻ E2R-P/X, E3-P/X, H3, H4 | Tên theo QĐ-EN |

Bổ sung từ bản tổng hợp nghiên cứu §2 đến §6. Chúng đi vào các thẻ đã có, không thêm thẻ:

| Thẻ | Bổ sung | Nhãn của nguồn |
|---|---|---|
| M2 | Ba đối chứng âm: tắt đột biến và một cá thể không có thức ăn phải chết đói; "bóng trung tính" với chọn lọc bị ngắt; tổ tiên đấu với hậu duệ khi tắt đột biến | Nguồn ngoài |
| E2R-P, E3-P | Một bước hiệu chuẩn mật độ thức ăn trước lượt thật, dùng seed tách khỏi seed xác nhận, để không lặp lại hiệu ứng sàn của E2 | Suy luận |
| E2R-P, E3-P | Tiền đăng ký ghi sẵn kiểm định không tham số, cách hiệu chỉnh và cỡ hiệu ứng. Số seed lấy từ lượt chạy thử; nghiên cứu không tìm được con số nào | Nguồn ngoài |
| S2 | Xuất sự kiện sinh và chết ở dạng công cụ phả hệ đọc được (gần OSS-072 và OSS-073) | Nguồn ngoài |
| S3, M1 | Mọi lượt chạy cần tái lập chạy trên CPU. Một thí nghiệm chạy trọn trên một lớp nền tảng cho tới khi có QĐ-M6 | Khớp hợp đồng tất định |

**Cổng ra:** ít nhất một thí nghiệm tiền đăng ký đã chạy đúng một lần và có `RESULT.md`. Kết quả
null vẫn qua cổng, miễn là thí nghiệm có sức mạnh thống kê đã khai trước.

### Chặng 5 — Bản 0.9 và bài phương pháp

Đợt 5 của kế hoạch swarm và `HT5` của roadmap 16/10: bản cài Windows, tag, DOI trên Zenodo,
README hai thứ tiếng, bản nháp bài phương pháp (P1 đến P9, theo QĐ-P1 đến QĐ-P3).

Chặng này thêm sáu việc chưa kế hoạch nào có. Hai việc cuối đến từ nghiên cứu 13/11:

| ID | Việc | Xong khi |
|---|---|---|
| MP-5.1 | Ba đến năm người ngoài cài bản 0.9 trong lúc chủ dự án ngồi xem | Ghi chép của từng buổi: họ dừng ở đâu, hỏi gì |
| MP-5.2 | Một kênh nhận phản hồi, một `SECURITY.md`, một mẫu PR | Ba file có trong repo |
| MP-5.3 | Một lượt `npm run tauri:build` được ghi lại, và bản cài được cài thử trên một máy sạch | Bản ghi trong `docs/ai/deployment/` |
| MP-5.4 | Quyết định về nền tảng thứ hai (QĐ-M4) | Một dòng quyết định |
| MP-5.5 | Quyết định về ký và tự cập nhật (QĐ-M8). Repo hôm nay chưa bật tự cập nhật | Một dòng quyết định, trước tag đầu tiên |
| MP-5.6 | Phạm vi cho người dùng đầu tiên: thế giới mẫu đi kèm bản cài; xuất và nhập sinh vật thành file. Đây là đề xuất; chủ dự án quyết cái nào vào 0.9, cái nào để 1.0 | Một dòng quyết định cho mỗi mục |

Bản 0.9 là bản **một máy**. Bốn dự án cùng loại mà nghiên cứu xem xét đều có người dùng trước khi
có bất kỳ tính năng nhiều máy nào (bản tổng hợp §12).

**Cổng ra:** tag `v0.9.0`, bản cài tải được, ít nhất ba buổi MP-5.1 có ghi chép.

**Cập nhật 2026-11-26 ([QD-FOCUS](../how-to/AGENT_WORKFLOW.md)):**

- Đường tới `v0.9.0` đi theo đúng thứ tự FOCUS-2.
- Phạm vi thế giới sống đóng băng tới tag này (FOCUS-1).
- Thí nghiệm kế tiếp là SEL2-P2, chạy đúng một lần, xen giữa các thẻ phát hành (FOCUS-3).

### Chặng 6 — Bản 1.0: lát cắt dọc hoàn chỉnh

Đợt 6 của kế hoạch swarm (V0 đến V11, H11). Tài liệu này không thêm gì ngoài một điều kiện: mọi
thẻ V chạy sau `R0`, và vẫn theo L2.

**Cổng ra:** `H11` — tag `v1.0.0`.

### Chặng 7 — Thế giới nhiều máy

Nhánh F của kế hoạch thế giới sống (F2 đến F4), sau `v1.0.0`. Xem NM-4 ở §6: phần chuẩn bị
không đụng mã Anima làm được sớm hơn nhiều.

**Trước dòng mã đầu tiên: sửa thiết kế 25/08 (QĐ-M7).** Thiết kế đó dùng lockstep theo rào nhịp
(§4) và giao mỗi thiết bị một dải không gian liên tục (§5.2). Chính nó ghi cái giá: máy chậm
nhất định nhịp. Bản tổng hợp nghiên cứu §8 cho rằng cái giá đó không chịu được trên mạng gia
đình, và đề xuất hình dạng khác cho hai chính sách:

| Chính sách | Hình dạng đề xuất | Kiểm bằng |
|---|---|---|
| `Contribute` | **Lô.** Host cắt thế giới thành các kỷ nguyên có seed cố định. Máy góp chạy trọn một kỷ nguyên rồi trả trạng thái cuối kèm băm. Không trao đổi gì trong lúc chạy | So bằng nhau tuyệt đối với host, giữa các máy cùng lớp nền tảng (QĐ-M6) |
| `Inhabit` | **Đảo.** Mỗi thiết bị giữ một đảo kín, tự chạy nhịp của mình. Sinh vật di cư nguyên con, kèm năng lượng, qua host, thưa | Bất biến T1 đến T4 lúc di cư. Không so với `Solo`: đây vẫn là treatment riêng |

Ba phần của thiết kế 25/08 giữ nguyên: ba chính sách liên bang, ngăn `Dormant`, năm lớp tin cậy.
Bản sửa cần thêm ba điều:

- Hợp đồng thuê có **mã rào tăng dần**, để một máy treo rồi tỉnh lại không ghi muộn được.
- Host giao seed trước; thiết bị không tự chọn. Việc này chặn kiểu gian lận mà T1 đến T4 bỏ sót:
  chạy nhiều nhánh hợp lệ rồi nộp nhánh mình thích.
- Tỉ lệ kiểm lại của T5 bắt đầu cao với thiết bị mới và giảm dần theo chuỗi kết quả đúng, thay
  cho một tỉ lệ cố định.

Cả mục này là suy luận từ báo cáo bên ngoài. Chưa có phép đo nào trên Anima, và nghiên cứu cũng
không đưa ra được nhịp tối đa của lockstep trên mạng gia đình.

**Cổng ra:** gate `contribute_matches_solo` xanh với hai máy thật cùng lớp nền tảng. Nếu QĐ-M6
chọn mức tuyệt đối thì thêm một cặp khác nền tảng.

## 6. Ba luồng chạy song song với các chặng

Ba luồng này không chạm `src-tauri/src`, nên không chặn và không bị chặn bởi thân cây, trừ chỗ
ghi rõ.

### Luồng NM — Nhiều máy

| ID | Việc | Sớm nhất khi | Cần gì | Xong khi |
|---|---|---|---|---|
| NM-1 | Đẩy `cargo test` và build nặng sang máy thứ hai qua một lưới tác vụ. Agent trên máy phát triển gửi lệnh, máy kia chạy trong sandbox và trả output | Ngay | Một lưới đã cài trên hai máy. Dự án OxideSwarm của chủ dự án có thử nghiệm APP-2 đúng cho việc này | Một PR của Anima dán output test chạy trên máy thứ hai |
| NM-2 | Ma trận checksum giữa các máy: cùng manifest, cùng commit, chạy `anima-lab --model live` trên Windows, macOS và Linux, so `final_checksum` | Sau chặng 2 | Icon thật (QĐ-F028); thẻ M1 | Một bảng trong `STATE` §1: cặp nền tảng nào khớp, cặp nào lệch. Bảng đó là đầu vào của QĐ-M6 |
| NM-3 | Quét seed trên nhiều máy: mỗi seed là một tác vụ, mỗi tác vụ trả một mục của `EnsembleSummary`, máy gửi gộp lại | Sau chặng 4 | NM-2; một cách chia manifest theo seed, vì `anima-lab` chỉ nhận `--model` và một manifest; các máy cùng lớp nền tảng cho tới khi có QĐ-M6 | Một thí nghiệm có N seed chạy trên hai máy và cho cùng báo cáo với chạy một máy |
| NM-4 | Lớp vận chuyển và danh tính cho nhánh F | Thiết kế: sau chặng 5. Mã: chặng 7 | QĐ-M3, QĐ-M7; một ADR | Xem dưới |

**Về NM-1 và NM-3.** Một nghiên cứu khả thi bên ngoài xếp đúng hai việc này lên đầu: build và test
từ xa hợp nhất, loạt seed hợp thứ hai, còn chia một thế giới thời gian thực thành mảnh thì không
hợp (bản tổng hợp §13). Các lệnh mẫu trong nghiên cứu đó dùng cờ mà `anima-lab` không có; không
chép chúng.

**Về NM-2.** `DETERMINISM_CONTRACT.md` chỉ hứa "cùng manifest, cùng build thì cùng quỹ đạo". Nó
không hứa giống nhau giữa các target hay mức tối ưu. NM-2 vì thế là một phép **đo**, không phải
một gate. Kết quả của nó quyết định lớp kiểm tra chéo T5 của thiết kế phân tán: nếu hai nền tảng
lệch nhau thì host chỉ tính lại được batch của thiết bị cùng nền tảng với nó.

NM-2 cũng là đầu vào của QĐ-M6. Nếu hai nền tảng lệch, có hai lối: nhận mức "theo lớp", hoặc trả
giá cho hồ sơ CPU tất định (lịch một luồng, hàm toán từ `libm`, tắt FMA, chuẩn hoá NaN; bản tổng
hợp §5.2). Giá về tốc độ của hồ sơ đó chưa ai đo. So sánh có dung sai không phải lối thứ ba:
một hệ khuếch đại sai số làm nó vô nghĩa.

**Về NM-4.** Thiết kế 25/08 ghi thẳng: không mở host ra Internet cho tới khi có xác thực người góp
máy và TLS, và đó là việc của lát F4. Có ba đường:

- **Tự xây** xác thực, TLS và đi qua NAT lên đường WebSocket hiện có. Nghiên cứu gọi đây là ngõ
  cụt.
- **Dùng một lưới có sẵn làm lớp vận chuyển cho mọi thứ.** Shard chạy như một dịch vụ trên máy
  của người góp, lưu lượng `anima-migration-v1` đi trong kênh của lưới. OxideSwarm đã có kênh mã
  hoá, đi qua NAT và sandbox; phần dịch vụ chạy lâu và đường hầm cho ứng dụng của nó mới ở mức kế
  hoạch. Chờ phần đó là chờ một dự án khác.
- **Tách theo chính sách**, là đề xuất của nghiên cứu. `Contribute` ở dạng lô dùng chế độ tác vụ
  mà lưới đã có hôm nay, không cần gì mới. `Inhabit` nhúng một thư viện QUIC ngang hàng vào
  Anima, sau một trait vận chuyển mỏng. OxideSwarm đang dùng chính loại thư viện đó, nên hai dự
  án chia sẻ được kinh nghiệm mà không cái nào chờ cái nào.

Đường thứ ba chỉ có nghĩa khi QĐ-M7 nhận hình dạng lô và đảo. Rủi ro của nó: thư viện còn trẻ,
API còn đổi. Chọn đường nào là QĐ-M3, và nó cần một ADR trước khi có dòng mã nào.

Thứ tự bên trong nhánh F không đổi: `Contribute` trước, vì nó phải cho kết quả bằng `Solo` và do
đó kiểm được; `Inhabit` sau.

### Luồng D — Tài liệu

Luồng tài liệu của kế hoạch swarm (DOC1 đến DOC8), cộng ba chỗ lệch đã thấy:

- `SNAPSHOT_CONTRACT.md` mô tả schema 3; mã ở schema 7.
- Bản `STATE` trên thân cây thiếu cập nhật ngày 16/10 của bản trên `main`. Chặng 2 (`C5`) sửa.
- `docs/planning/README.md` gọi `TODO.md` là backlog; `TODO.md` tự nhận là nhật ký. *(đã xử lý 2026-11-17 ở #97 tại [`docs/planning/README.md`](README.md))*

Thêm một việc từ nghiên cứu 13/11, làm được ngay và không cần build: **mở một nhật ký dùng AI**
(công cụ, phiên bản, đã dùng vào việc gì). Ba nơi công bố mà nghiên cứu xem xét đều đòi khai
việc này, và nhật ký không dựng lại được về sau. Nó phục vụ QĐ-P3.

Bản tổng hợp nghiên cứu, nếu chủ dự án nhận, nằm ở `docs/research/` như các tài liệu nghiên cứu
đã có.

### Luồng C — Người dùng và người đóng góp

Bắt đầu ở chặng 5 (MP-5.1, MP-5.2). Trước đó chỉ có một việc: không hứa gì trong README mà
`STATE` §1 chưa có bằng chứng.

Nghiên cứu 13/11 thêm một quan sát: các dự án cùng loại có người dùng nhờ một bản chạy được
ngay, thế giới mẫu và cách chia sẻ sinh vật, không nhờ nhiều máy (bản tổng hợp §12). MP-5.6 đưa
quan sát đó thành một quyết định về phạm vi. Hướng nộp một tạp chí phần mềm mã nguồn mở là hướng
xa: nơi đó đòi lịch sử công khai, người dùng và cộng đồng mà dự án chưa có.

## 7. Hàng đợi quyết định của chủ dự án

Cột "Đề xuất" chép từ tài liệu đã có, trừ các dòng QĐ-M là của tài liệu này.

| ID | Câu hỏi | Đề xuất | Chặn | Cần trước chặng |
|---|---|---|---|---|
| QĐ-A′ | Sau gộp giữ hệ mortality nào | Giữ `mortality_system` của thân cây | HT-C | 2 |
| QĐ-B | `AGENTS.md` khai `src-tauri/src` là chỉ đọc | Một dòng uỷ quyền chuẩn để dán vào thẻ | Mọi thẻ Rust | 1 |
| QĐ-F028 | Commit icon thật | Có | Biên dịch ngoài Windows, NM-2 | 1 |
| QĐ-C | Giấy phép `hexf-parse` | (A) bỏ `ml-wgpu` khỏi feature `desktop`, kèm ADR. Đã kiểm ngày 13/11: `hexf-parse` chỉ vào bản build qua `naga`, nên (A) gỡ nó hẳn. (A) cũng khớp với việc mọi lượt chạy cần tái lập đều chạy CPU | Phát hành | 5 |
| QĐ-1 | Dừng ở mức hoàn thiện nào | Mức 2, bản 0.9, trước | Phát hành | 5 |
| QĐ-EN | Tên thí nghiệm | E2R = làm lại câu hỏi não; E3 = gradient năng suất | Chặng 4 | 4 |
| QĐ-4, QĐ-5 | Quần thể luân chuyển; sinh sản sinh học | Theo kế hoạch swarm §2; QĐ-5 cần một ADR | S1 | 4 |
| QĐ-2/H, QĐ-6 | Map 2 và Research Dashboard; chỗ đặt hợp đồng IPC | Theo kế hoạch swarm §2 | G4, G6, G8 | 3 |
| QĐ-P1 đến P3 | Loại bài báo, chỗ để bản nháp, khai báo dùng AI | Theo kế hoạch swarm §2 | P1 đến P9 | 5 |
| **QĐ-M1** | Tài liệu nào là `accepted` cho mỗi vai của L6 | `EXECUTION_PLAN` và kế hoạch swarm cho việc; tài liệu này cho thứ tự chặng | MP-0.6 | 1 |
| **QĐ-M2** | Tách crate trước hay sau `v1.0.0` | Sau, như thẻ X1 của kế hoạch swarm. Không bao giờ song song với HT-C | `CRATE_SPLIT_PLAN` | 3 |
| **QĐ-M3** | Lớp vận chuyển của nhánh F: tự xây, dùng một lưới có sẵn cho mọi thứ, hay tách theo chính sách | Không tự xây. Nghiên cứu 13/11 đề xuất tách: lưới tác vụ cho `Contribute`, một thư viện QUIC ngang hàng nhúng trong Anima cho `Inhabit`. Vẫn quyết sau NM-1 và NM-2, khi đã dùng thử lưới cho việc thật | NM-4, F4 | 7 |
| **QĐ-M4** | Nền tảng phát hành thứ hai | macOS không ký, nếu NM-2 cho thấy nó khớp checksum với Windows | MP-5.4 | 5 |
| **QĐ-M5** | `Inhabit` có phải mục tiêu của dự án, hay chỉ `Contribute` | Cả hai, theo thiết kế 25/08, với `Inhabit` luôn dán nhãn là treatment riêng | F3 | 7 |
| **QĐ-M6** | Mức tất định giữa các nền tảng: "theo lớp" (bằng nhau trong cùng hệ điều hành, kiến trúc và build) hay "tuyệt đối" (bằng nhau giữa Windows x86_64 và macOS ARM64) | "Theo lớp" cho tới khi NM-2 có bảng. Chỉ xét "tuyệt đối" sau khi đo được giá tốc độ của hồ sơ CPU tất định | T5, NM-3 trên hai nền tảng, cổng ra chặng 7 | 4 nếu một thí nghiệm chạy trên hai nền tảng; nếu không thì 7 |
| **QĐ-M7** | Sửa thiết kế 25/08: `Contribute` thành lô, `Inhabit` thành đảo, bỏ lockstep theo nhịp và dải không gian liên tục | Có, bằng một bản sửa thiết kế trước khi F2 có mã. Đây là suy luận từ nghiên cứu bên ngoài, chưa có phép đo | F2 đến F4, NM-4 | 7 |
| **QĐ-M8** | Ký bản cài và tự cập nhật: có bật tự cập nhật không; nếu có, khoá ký để ở đâu và ai giữ bản dự phòng | Chưa đề xuất. Mất khoá thì người dùng hết nhận được bản cập nhật, nên phải quyết trước tag đầu tiên | MP-5.5 | 5 |

> **Ghi chú (2026-11-24):** Các hàng của bảng §7 trên đã được chốt lần lượt từ `2026-11-14` tới `2026-11-24` (văn bản gốc giữ nguyên để tra cứu lịch sử):
> - **`QĐ-A′`, `QĐ-B`, `QĐ-F028`, `QĐ-C`/`QD-C2`, `QĐ-1`, `QĐ-EN`, `QĐ-4`/`QĐ-5`, `QĐ-6`, `QĐ-P1..P3`:** đã quyết (xem [`COMPLETION_ROADMAP_2026-10-31.md` §4](COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được)); riêng phần đuôi `QĐ-2/H` chốt theo `QD-OPENC` (ẩn Map 2 cho tới khi dán nhãn trang trí ở thẻ `C14`, giữ Research Dashboard chỉ-đọc) và `QĐ-P2` chốt hình vẽ coi là dữ liệu ở repo công khai còn bản nháp văn bản ở repo riêng tư.
> - **`QĐ-M1`, `QĐ-M2`, `QĐ-M6`:** đã quyết theo `QD-OPENQ` (`EXECUTION_PLAN` + swarm plan cho việc và `DIRECTION` cho thứ tự chặng; tách crate sau `v1.0.0`; tất định theo lớp nền tảng).
> - **`QĐ-M4` & `QĐ-M8`:** đã quyết theo `QD-OPENQ` + `QD-OPENC` + `QD-REL09` (`0.9` không ký; quyết ký & tự cập nhật trước `v1.0.0`; macOS không ký là nền tảng thứ hai nếu `NM-2` khớp lớp Windows).
> - **`QĐ-M5` & `QĐ-M7`:** đã quyết theo `QD-SHAREDISLAND` + `QD-OPENQ` + `QD-ADR0014` ([ADR-0014](../decisions/ADR-0014-shared-island-host-and-guests.md), `#256`): đảo chung do host mô phỏng là mục tiêu chính, thay thế định nghĩa `Inhabit` cũ của `QĐ-M7`; `Contribute` theo lô giữ nguyên. `QĐ-M3` (vận chuyển Internet) để mở tới khi mốc LAN 1 chạy xong.
> - Nguồn chi tiết mọi quyết định ngày `2026-11-24` (`PR #255`): [`AGENT_WORKFLOW.md` §4](../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0).

## 8. Việc chỉ con người làm được

| Việc | Chặng | Vì sao agent không làm được |
|---|---|---|
| Ra quyết định ở §7 | 0, rồi rải theo chặng | L4 |
| Gộp PR; giải quyết xung đột ở `PROJECT.md`, `AGENTS.md`, `STATE` | 1 đến 3 | Chỉ chủ dự án gộp; agent không sửa ba file đó |
| Bật bảo vệ nhánh | 1 | Cấu hình tài khoản GitHub |
| Tạo lại hồ sơ giấy phép trên Windows | 1, 5 | Hồ sơ mô tả một bản build Windows (F-025) |
| Chạy app, đo bản release với 1.000 agent | 4 | Chạy app từng làm treo máy; agent bị cấm chạy |
| Duyệt tiền đăng ký thí nghiệm | 4 | Thẻ H3, H4 |
| Mở từng nguồn trích dẫn của bài báo | 5 | Rủi ro trích dẫn bịa |
| Ngồi xem người khác cài | 5 | MP-5.1 |

Việc của con người là tài nguyên khan hiếm nhất. Mỗi tuần nên có một buổi dành riêng cho hàng đợi
này, trước khi bật swarm.

## 9. Thước đo tiến độ

Đây là **mục tiêu**, không phải số đo. Số đo, khi có, vào `STATE` §1.

| Thước đo | Mục tiêu | Lấy từ đâu |
|---|---|---|
| Số ngày liên tiếp thân cây xanh | Tăng dần sau chặng 1; về 0 là tín hiệu dừng theo L3 | Lịch sử chạy CI |
| Số PR mã đang mở | Không quá ba (L2) | `gh pr list` |
| Tuổi PR mở lâu nhất | Dưới 14 ngày | `gh pr list` |
| Số ô quyết định trống ở §7 cho chặng hiện tại | Không | §7 |
| Tỉ lệ PR mã trên PR tài liệu kế hoạch | Ít nhất một trên một (L7) | `git log` |
| Số thí nghiệm tiền đăng ký đã chạy | Một ở cuối chặng 4 | `artifacts/experiments/` |
| Số người ngoài đã cài được bản 0.9 | Ba ở cuối chặng 5 | Ghi chép MP-5.1 |
| Số cặp nền tảng đã đo checksum | Ba ở NM-2 | `STATE` §1 |

## 10. Rủi ro

| Rủi ro | Dấu hiệu | Khống chế |
|---|---|---|
| Lập kế hoạch thay cho làm | Nhiều commit tài liệu liên tiếp, không PR mã nào được gộp | L7; tài liệu này là tài liệu kế hoạch cuối cùng trước khi `B3b` vào |
| Agent tối ưu cho màu xanh: nới test, bịa giá trị, thu hẹp lệnh | Đã xảy ra trên `main` ngày 22/10 | Verifier ngữ cảnh sạch; luật của kế hoạch swarm §6 |
| Bản gộp làm sẵn bị gộp thẳng vì "đã xong" | PR từ `claude/funny-thompson-jj42fe` không kèm output checksum | L5 |
| Một người, bốn dự án | Chặng 0 kéo dài quá hai tuần | Trong lúc Anima ở chặng 0 đến 2, không dự án nào khác được thêm việc mới cần chủ dự án quyết |
| Máy phát triển treo | Build đầy đủ chạy trên máy phát triển | L9; NM-1 |
| Checksum lệch giữa các nền tảng | NM-2 cho thấy lệch | Chấp nhận như một kết quả; T5 chỉ so trong cùng nền tảng; ghi vào hợp đồng tất định |
| Tầm nhìn nhiều máy kéo phạm vi | Thẻ F2 đến F4 xuất hiện trước `v1.0.0` | Chặng 7 đứng sau chặng 6; trước đó chỉ NM-1 đến NM-3 |
| `hexf-parse` không ai quyết | QĐ-C còn trống khi vào chặng 5 | Đưa QĐ-C vào buổi quyết định đầu tiên, dù nó chưa chặn chặng 1 |
| Phụ thuộc vào một lưới chưa xong | NM-4 chờ tính năng mà lưới chưa có | QĐ-M3 chỉ quyết sau khi NM-1 và NM-2 đã dùng lưới cho việc thật. Đường "tách theo chính sách" chỉ dùng thứ lưới đã có |
| Nhánh F xây trên một kiến trúc khó sống ngoài LAN | Thẻ F2 bắt đầu khi thiết kế 25/08 còn nguyên lockstep theo nhịp | QĐ-M7 đứng trước F2 |
| Kết luận của nghiên cứu bên ngoài bị chép vào như sự thật | Một tài liệu của repo trích một con số hay một lệnh từ báo cáo mà không qua bản tổng hợp | Bản tổng hợp gắn nhãn từng điều; chỉ điều "đã kiểm" mới được dùng làm căn cứ. Số đo vẫn chỉ vào `STATE` §1 |
| Khoá toán học để bằng nhau giữa các nền tảng rồi mới biết nó quá chậm | Mã của hồ sơ CPU tất định xuất hiện trước khi có phép đo | QĐ-M6 đòi phép đo trước |

## 11. Hai tuần đầu

| # | Việc | Ai | Kết quả nhìn thấy được |
|---|---|---|---|
| 1 | Một buổi quyết định: QĐ-A′, QĐ-B, QĐ-F028, QĐ-C, QĐ-1, QĐ-EN | 🧑 | Sáu dòng có ngày trong roadmap 31/10 §4 |
| 2 | Thẻ D0: bảng trạng thái mười PR và các nhánh | 🤖 chỉ đọc | Một file trong `docs/ai/planning/` |
| 3 | Đóng hoặc ghi lý do cho từng PR theo bảng đó | 🧑 | Không PR nào thiếu quyết định |
| 4 | Dọn cây làm việc (MP-0.5) | 🧑 | `git status` sạch |
| 5 | Đưa kế hoạch swarm vào thân cây, sửa mâu thuẫn ở §12 | 🤖, 🧑 gộp | Một PR tài liệu |
| 6 | Thẻ T1: `B3b` | 🤖, tối đa hai thẻ Rust | Một PR, output gate dán nguyên |
| 7 | Tạo lại hồ sơ giấy phép trên Windows | 🧑 | `check:notice` và `check:licenses` xanh |
| 8 | Bật bảo vệ nhánh khi CI xanh lần đầu | 🧑 | Hai job là gate bắt buộc |
| 9 | NM-1: chạy một lượt `cargo test` của Anima trên máy thứ hai qua lưới | 🧑 cài, 🤖 gửi | Output dán vào một PR |
| 10 | Thẻ T2: `HT-C`, với ba điều kiện của chặng 2 | 🤖, verifier riêng | Một PR chỉ gộp |

Việc 1 đến 5 không cần build gì. Việc 6 chỉ bắt đầu khi việc 1 xong.

Ghi chú của bản 2: nghiên cứu 13/11 không thêm việc nào vào bảng này. Việc duy nhất nên bắt đầu
cùng lúc là nhật ký dùng AI ở luồng D, vì nó không cần build và không dựng lại được về sau.

## 12. Mâu thuẫn giữa các tài liệu cần sửa ở MP-0.6

- `COMPLETION_ROADMAP_2026-10-31.md` mục HT0′.1 và `EXECUTION_PLAN` mục C1 đến C3 vẫn khuyên lấy
  `main` làm nền; §4 của chính roadmap đó quyết ngược lại.
- `EXECUTION_PLAN` mục H4 ghi QĐ-A đến QĐ-D còn chờ; A và D đã quyết.
- `EXECUTION_PLAN` nhắc `agent_death_system`; tên thật là `agent_mortality_system`.
- "E3" đang chỉ hai thí nghiệm khác nhau (QĐ-EN sửa).
- QĐ-B là "còn mở" trong roadmap và là "đã cho phép" trong brief.
- Số commit thân cây đi trước `main` được ghi bằng bốn con số khác nhau ở bốn tài liệu. Chỉ nên
  ghi lệnh, không ghi số.
- Roadmap 16/10, mà roadmap 31/10 nói vẫn còn hiệu lực cho HT2 đến HT5, không có trên thân cây.
- `H1` đến `H4` chỉ những việc khác nhau trong `EXECUTION_PLAN` và trong kế hoạch swarm. Khi
  trích, phải ghi kèm tên tài liệu.

## 13. Tài liệu này không tuyên bố

- Không có số đo nào ở đây là số đo mới. Mọi con số về trạng thái git đều kèm lệnh và ngày.
- Không có ước lượng thời gian. Các tài liệu gốc cũng không có.
- Không nói rằng bản gộp trên `claude/funny-thompson-jj42fe` đúng hay sai; nó chưa được kiểm.
- Không nói rằng hai nền tảng cho cùng checksum; NM-2 mới đo điều đó.
- Không nói rằng một lưới bên ngoài phù hợp cho nhánh F; đề xuất ở QĐ-M3 là của một nghiên cứu bên
  ngoài và chưa được thử.
- Không nói rằng hình dạng lô và đảo chạy được trên Anima, hay rằng lockstep theo nhịp đã được đo
  là quá chậm. Cả hai là suy luận; nghiên cứu không có số đo cho mạng gia đình.
- Không nói rằng hồ sơ CPU tất định cho kết quả bằng nhau giữa các nền tảng trên Anima; chưa ai
  thử.
- Phần hiện trạng dựa trên việc đọc tài liệu và mã. Không lệnh nào của repo được chạy để kiểm lại.

## Phụ lục A — Các tài liệu kế hoạch hiện có và vai của chúng

| Tài liệu | Ở đâu | Trạng thái | Vai đề nghị (L6) |
|---|---|---|---|
| `STATE_OF_THE_PROJECT.md` | `docs/planning/` | Không phải kế hoạch | Số đo, nguồn duy nhất |
| `COMPLETION_ROADMAP_2026-10-16.md` | Chỉ trên `main` | Một phần bị thay | HT2 đến HT5 còn hiệu lực; cần có mặt trên thân cây |
| `COMPLETION_ROADMAP_2026-10-31.md` | `docs/planning/` | `proposed` | Quyết định QĐ-A đến QĐ-E |
| `EXECUTION_PLAN_2026-10-31.md` | `docs/planning/` | `proposed` | Việc HT-A đến HT-D |
| Brief 31/10 | `docs/ai/planning/` | Bộ làm việc | Prompt của từng thẻ |
| Phép đo bề mặt gộp 01/11 | `docs/ai/planning/` | Bộ làm việc | Điều kiện của HT-C |
| Kế hoạch tổng cho swarm 05/11 | Nhánh chưa gộp | `proposed` | Danh sách thẻ và đợt |
| `CRATE_SPLIT_PLAN_2026-10-31.md` | `docs/planning/` | `proposed` | Sau `v1.0.0` (QĐ-M2) |
| Kế hoạch thế giới sống 25/08, thiết kế phân tán 25/08 | `docs/ai/` | `proposed` | Chặng 7 |
| `RD_TECH_UPGRADE_PLAN_2026-08-07.md` | `docs/planning/` | Chưa bắt đầu | Ngoài 1.0 |
| Tài liệu này | `docs/planning/` (đề nghị) | `proposed` | Thứ tự chặng, luật điều phối |
| Bản tổng hợp nghiên cứu 13/11 | `docs/research/` (đề nghị) | `proposed` | Điều tìm thấy từ bên ngoài, kèm nhãn; không phải kế hoạch |

## Phụ lục B — Đối chiếu chặng với các tài liệu đã có

| Chặng | `EXECUTION_PLAN` | Roadmap 31/10 và 16/10 | Kế hoạch swarm |
|---|---|---|---|
| 0 | H4 | §4 | Đợt 0 (H0 đến H2), D0 |
| 1 | HT-A, HT-B tới B3b | HT0′ | T1 |
| 2 | HT-C | HT1′ | T2 |
| 3 | B2, B5, B6 | HT0′.4, HT0′.5 | Đợt 2, R0 |
| 4 | HT-D | HT2, HT4.1 | Đợt 3, Đợt 4 |
| 5 | — | HT5 | Đợt 5 |
| 6 | — | Mức 3 | Đợt 6 |
| 7 | — | — | Đợt 7; nhánh F |
