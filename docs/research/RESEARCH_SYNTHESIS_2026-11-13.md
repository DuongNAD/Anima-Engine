---
title: Tổng hợp nghiên cứu ngày 2026-11-13 — khoa học, tất định, nhiều thiết bị, phát hành
status: proposed
owner: maintainers
last_reviewed: 2026-11-13
review_cycle: quarterly  # và khi một phép đo thay thế một mục "chưa kiểm"
---

# Tổng hợp nghiên cứu ngày 2026-11-13

> **Tài liệu này là gì.** Chủ dự án giao năm câu hỏi cho một công cụ nghiên cứu và nhận về năm
> báo cáo. Tài liệu này gom những gì các báo cáo đó tìm thấy, ghi rõ điều nào đã được kiểm, điều
> nào chưa, điều nào sai, và mỗi điều kéo theo việc gì cho Anima.
>
> **Nó không phải gì.** Không phải số đo: không lệnh nào của repo được chạy. Không phải kế
> hoạch: thứ tự việc nằm ở
> [kế hoạch định hướng và điều phối](../planning/DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md).

## 0. Nguồn và nhãn

| Báo cáo | Chủ đề |
|---|---|
| C | Thiết kế thí nghiệm, áp lực chọn lọc, thước đo tiến hoá mở, tất định, nơi công bố, MAP-Elites |
| D | Thế giới chia sẻ trên máy cá nhân, tin kết quả, lớp mạng, phát hành ứng dụng Tauri, người dùng đầu tiên |
| A, B | Viết cho lưới OxideSwarm; chỉ lấy phần liên quan tới một ứng dụng mô phỏng |
| E | Nghiên cứu khả thi về việc dùng OxideSwarm cho Anima |

Cả năm báo cáo đều được đọc trọn. Tác giả của chúng không đọc được nhánh thân cây của repo, nên
nhiều chỗ mô tả Anima theo bản `main` cũ hoặc theo suy đoán.

| Nhãn | Nghĩa |
|---|---|
| **Đã kiểm** | Mở nguồn gốc hoặc mã của repo ngày 2026-11-13 và thấy đúng |
| **Nguồn ngoài** | Báo cáo nói vậy, kèm nguồn; chưa mở nguồn để kiểm |
| **Suy luận** | Kết luận của báo cáo hoặc của tài liệu này, không có nguồn trực tiếp |
| **Sai / đáng ngờ** | Trái với mã, với hợp đồng của repo, hoặc nguồn không khớp điều được trích |

Tài liệu này **không chép số hiệu năng** từ các báo cáo. Chỗ nào cần số thì trỏ tới
[STATE_OF_THE_PROJECT.md §1](../planning/STATE_OF_THE_PROJECT.md) hoặc ghi "chưa đo".

## 1. Mười kết luận

1. **Tất định phải xong trước mọi thứ khác.** Thí nghiệm, kiểm kết quả từ máy khác và thế giới
   nhiều thiết bị đều dựa vào nó. (Suy luận, cả năm báo cáo cùng hướng.)
2. **Giống nhau giữa CPU và GPU là không đạt được.** Mọi lượt chạy cần tái lập phải chạy trên
   CPU. (Nguồn ngoài; khớp với hợp đồng tất định hiện có.)
3. **Giống nhau giữa các hệ điều hành là một lựa chọn có giá**, không phải mặc định. Phải quyết:
   chỉ cam kết trong cùng một lớp nền tảng, hay khoá toán học để bằng nhau tuyệt đối (§5).
4. **Thí nghiệm đầu tiên cần ba đối chứng âm và một bước hiệu chuẩn** để không lặp lại hiệu ứng
   sàn của E2 (§2).
5. **Lockstep theo từng nhịp qua mạng gia đình là ngõ cụt.** Thế giới nhiều thiết bị khả thi ở
   dạng các đảo chạy độc lập, trao đổi thưa (§8). Thiết kế 25/08 đang dùng lockstep theo nhịp.
6. **Bốn lớp kiểm bất biến không bắt được "tiến hoá chọn lọc"**: thiết bị chạy nhiều nhánh hợp
   lệ rồi nộp nhánh nó thích (§9).
7. **Không tự xây TLS và xác thực** trên đường WebSocket hiện có (§10).
8. **Bỏ `ml-wgpu` khỏi feature `desktop` vẫn là lối ra đơn giản nhất** cho vướng mắc giấy phép:
   `hexf-parse` chỉ vào bản build qua `naga` (§11, đã kiểm).
9. **Các dự án cùng loại không cần nhiều máy lúc ra mắt.** Chúng có người dùng nhờ một bản chạy
   được ngay, thế giới mẫu và cách chia sẻ sinh vật (§12).
10. **Việc lưới giúp được ngay là build, test và loạt seed**, không phải chia thế giới thành
    mảnh (§13).

## 2. Thiết kế thí nghiệm

| Điều tìm thấy | Nhãn | Việc kéo theo |
|---|---|---|
| Kết quả của mô phỏng tiến hoá thường lệch hoặc hai đỉnh. Dùng kiểm định không tham số (Kruskal–Wallis, rồi Mann–Whitney hoặc Wilcoxon), hiệu chỉnh Holm–Bonferroni, báo cỡ hiệu ứng Cliff's delta. Không tính sức mạnh sau khi đã có kết quả | Nguồn ngoài | Ghi vào tiền đăng ký của E2R và E3 |
| Thiết kế ghép cặp theo seed: đối chứng và xử lý dùng cùng seed | Nguồn ngoài; khớp ER07 của hợp đồng thí nghiệm | Đã có trong hợp đồng; giữ |
| Ba đối chứng âm: (1) tắt đột biến, một cá thể không có thức ăn phải chết đói; (2) "bóng trung tính": cùng thế giới nhưng chọn lọc bị ngắt; (3) tổ tiên đấu với hậu duệ khi tắt đột biến | Nguồn ngoài | Thêm vào thẻ M2 (đối chứng âm) của kế hoạch swarm |
| Tránh hiệu ứng sàn: trước khi chạy thật, quét mật độ thức ăn với đột biến tắt cho tới khi có sinh, có chết đói rõ rệt, không tuyệt chủng sớm và không chạm trần quần thể | Suy luận | Một bước hiệu chuẩn trước E2R-P; seed hiệu chuẩn tách khỏi seed xác nhận |
| Số seed, sức mạnh, cỡ hiệu ứng nên dùng | **Không tìm thấy** | Xác định bằng lượt chạy thử, ghi vào tiền đăng ký |

**Đáng ngờ:** nguồn chính của mục này trong báo cáo C là một bản thảo có tiêu đề về chủ đề khác
hẳn. Các phương pháp thống kê nêu trên là chuẩn, nhưng phải tìm nguồn khác trước khi trích trong
bài báo.

## 3. Áp lực chọn lọc

Ba điều kiện để chọn lọc tự sinh ra mà không cần viết hàm fitness (nguồn ngoài, theo phần giới
thiệu của Avida): tài nguyên cạn được; chi phí liên tục dẫn tới chết; sinh sản trả bằng năng
lượng của cha mẹ. Thiếu thuế nền và thuế theo nơ-ron thì quần thể đứng yên.

**Báo cáo C không biết** thân cây đã có mortality và fitness sinh thái sau khoá manifest
(F-018). Nó đề xuất các thứ đó như thể chưa có. Phần còn dùng được là danh sách kiểm:

- Mọi lượng ăn vào phải ghi nợ một trường tài nguyên hoặc một con mồi.
- Thuế nền, thuế hình thái và thuế não tính theo từng nhịp.
- Một khoá manifest tắt cập nhật của mạng actor-critic dùng chung, để não chỉ thay đổi qua di
  truyền trong lượt đối chứng.

| Nền tảng | Đo gì | Mượn được gì |
|---|---|---|
| Tierra | Kích thước bộ gen, ký sinh | Khởi đầu bằng một tổ tiên tự duy trì |
| Avida | Phả hệ, dòng hậu duệ, loại bỏ từng lệnh | Kỷ luật phả hệ và đối chứng |
| Polyworld | Độ phức tạp thần kinh; lượt "có chọn lọc" so với "thụ động" | Đối chứng thụ động |
| The Bibites | Thời gian chết đói, thị phần dòng | Thử nghiệm trên một cá thể |
| Neural MMO | Hoàn thành nhiệm vụ | Phản ví dụ: bộ học dùng chung, không tiến hoá |
| Framsticks, ALIEN | Hình thái, cân bằng năng lượng | Báo cáo không nêu |

Toàn bảng là nguồn ngoài.

## 4. Thước đo tiến hoá mở

| Họ thước đo | Tính được từ | Nhãn |
|---|---|---|
| Thống kê hoạt động của Bedau và Packard | Cần lượt "bóng trung tính" chạy song song | Nguồn ngoài |
| MODES: thay đổi, mới lạ, sinh thái, phức tạp | Phả hệ cộng nhật ký thụ động | Nguồn ngoài |
| Phylometrics: số lá, độ sâu, độ cân bằng của cây | Phả hệ; chỉ đúng nghĩa với dòng vô tính | Nguồn ngoài |

Việc kéo theo: xuất sự kiện sinh và chết (định danh, tổ tiên, thời điểm sinh, thời điểm chết) ở
dạng mà công cụ phả hệ bên ngoài đọc được. Việc này gần với OSS-072 và OSS-073. Báo cáo gọi "tiến
hoá mở không giới hạn" là ngõ cụt cho bài báo đầu tiên.

## 5. Tất định

### 5.1 Vì sao hai máy cho kết quả khác nhau

| Nguyên nhân | Nhãn |
|---|---|
| Hàm siêu việt (`sin`, `exp`...) của thư viện toán mỗi hệ điều hành cho kết quả khác nhau ở bit cuối. Tài liệu của Rust ghi độ chính xác của chúng là không xác định | Nguồn ngoài; khớp với hiểu biết chung |
| Phép nhân-cộng gộp (FMA): có sẵn trên ARM64, tuỳ cờ trên x86_64 | Nguồn ngoài |
| Phép cộng không kết hợp: thứ tự cộng đổi khi dùng SIMD hoặc chạy song song | Nguồn ngoài |
| Giá trị NaN và số không âm khi băm | Nguồn ngoài |
| CPU so với GPU; `burn-ndarray` mặc định đa luồng | Nguồn ngoài; một issue được trích cho ý này nói về chuyện khác |

### 5.2 Hồ sơ CPU tất định

Báo cáo C đề xuất, tất cả là suy luận và chưa thử trên Anima:

- Một tiến trình cho mỗi seed; lịch một luồng.
- Hàm toán lấy từ `libm`; `glam` ở chế độ scalar; tắt FMA và ghim toolchain.
- Chuẩn hoá NaN và số không âm trước khi băm.
- Burn chạy CPU, một luồng.
- Một ảnh Linux x86_64 làm nền tảng tham chiếu.

Giá của hồ sơ này về tốc độ: **chưa đo**, và các báo cáo cũng không có số.

Hai thứ bị loại: bằng nhau giữa CPU và GPU, và số cố định (thư viện số cố định thiếu hàm siêu
việt, không dùng được với Burn và `glam`).

### 5.3 Bevy đa luồng

- Bật phát hiện mơ hồ thứ tự hệ thống ở mức lỗi, để hai hệ thống xung đột mà chưa sắp thứ tự làm
  hỏng build thay vì chạy sai âm thầm.
- RNG theo từng thực thể, dẫn xuất từ seed của manifest.
- Tránh `HashMap` chuẩn trên đường tất định; tránh lệnh song song trong vòng lặp song song.

**Đáng ngờ:** bằng chứng lấy từ tài liệu của các bản Bevy mới hơn nhiều so với bản 0.13 mà Anima
đang dùng. Phải kiểm từng thiết lập trên đúng bản đang dùng.

### 5.4 Hai ví dụ từ bên ngoài

- Một engine vật lý 2D mã nguồn mở công bố năm 2024 rằng nó đạt cùng trạng thái trên Apple
  silicon và x86, với ba trình biên dịch. (Nguồn ngoài.) Tức là đạt được, với kỷ luật ở 5.2.
- Một nghiên cứu năm 2005 trên tính toán tình nguyện cho thấy cùng một mô phỏng phân tử cho kết
  quả khác nhau giữa hai nền tảng, và so sánh có dung sai không cứu được vì sai số bị khuếch
  đại. (Nguồn ngoài.) Đây là cơ sở của §9.

### 5.5 Quyết định cần có

Hợp đồng tất định hiện chỉ hứa "cùng manifest, cùng build thì cùng quỹ đạo". Có hai mức để chọn:

| Mức | Cam kết | Giá | Đủ cho |
|---|---|---|---|
| Theo lớp | Bằng nhau trong cùng hệ điều hành, kiến trúc và build | Không sửa mã | Thí nghiệm ghép cặp trên cùng một lớp; kiểm chéo giữa các máy cùng lớp |
| Tuyệt đối | Bằng nhau giữa Windows x86_64 và macOS ARM64 | Hồ sơ 5.2, chưa đo tốc độ | Kiểm chéo giữa mọi máy; thế giới nhiều thiết bị không phân lớp |

Đây là QĐ-M6 trong kế hoạch định hướng. Phép đo NM-2 của kế hoạch đó cho biết hôm nay hai nền
tảng lệch nhau tới đâu.

## 6. MAP-Elites

Báo cáo độ phủ và QD-Score, kèm một kho giả cho lượt đối chứng. Bốn bẫy: nhiễu khi đánh giá; mô
tả đo lúc sinh thay vì cả đời; số ô bùng nổ; mỗi ô một tinh hoa dẫn tới trì trệ. Đề xuất: biến kho
hiện có thành kho thụ động, nhận cá thể trưởng thành của dòng còn sống, mô tả tính cả đời, lấy
trung vị theo ô. Toàn mục là nguồn ngoài và suy luận. Nó khớp với việc còn mở "độ phủ kho chưa
đo được khi chạy không cửa sổ" (EB-S11).

## 7. Nơi công bố

| Nơi | Đòi gì | Nhãn |
|---|---|---|
| Hội nghị ALIFE | Bài đầy đủ, phản biện kín hai chiều; trên thực tế cần DOI cho mã và một đường dẫn ẩn danh | Nguồn ngoài; hạn nộp của kỳ tới: không tìm thấy |
| Tạp chí Artificial Life | Bài hoặc thư; tuyên bố về dữ liệu. Chính sách của nhà xuất bản: AI không là tác giả; dùng AI để sinh hoặc sửa đáng kể mã là "sử dụng trọng yếu", phải khai công cụ, phiên bản, mục đích, mức độ | Nguồn ngoài |
| JOSS | Giấy phép OSI, test trên CI, bản phát hành có DOI, sáu tháng lịch sử công khai, khai báo dùng AI. Một tác giả, chưa có cộng đồng hay người dùng nghiên cứu: không được nhận | Nguồn ngoài |

Việc kéo theo, rẻ và làm được ngay: giữ một nhật ký dùng AI (công cụ, phiên bản, việc đã làm) từ
bây giờ, vì cả ba nơi đều đòi khai. JOSS là hướng xa: báo cáo gọi "nộp JOSS bây giờ" là ngõ cụt.

## 8. Thế giới nhiều thiết bị

### 8.1 Bốn kiến trúc bị loại

| Kiến trúc | Vì sao | Nhãn |
|---|---|---|
| Lockstep theo từng nhịp | Máy chậm nhất và mạng tệ nhất định nhịp cho tất cả | Suy luận, từ kinh nghiệm của các trò chơi chiến thuật |
| Rollback | Phải mô phỏng lại nhiều nhịp mỗi lần lệch; quá đắt cho một thế giới sinh thái | Suy luận |
| Chia không gian liên tục, sinh vật qua biên từng nhịp | Các hệ lớn làm việc này đã dừng hoặc lỗi ở biên | Nguồn ngoài |
| Một máy chủ có thẩm quyền, giãn thời gian | Chỉ chia được tải CPU, không phải mô hình "mỗi máy một phần thế giới" | Suy luận |

Không báo cáo nào đưa ra được nhịp tối đa cho lockstep trên Wi-Fi hay mạng dây kèm phần cứng và
cách đo. Một báo cáo nêu số độ trễ của một bài viết nhà cung cấp trên một tuyến; không dùng ở đây.

### 8.2 Kiến trúc khả thi

- **`Contribute` thành lô.** Host cắt thế giới thành các kỷ nguyên có seed cố định. Máy góp sức
  nhận trạng thái đầu, chạy trọn kỷ nguyên, trả trạng thái cuối kèm băm. Không có trao đổi nào
  trong lúc chạy.
- **`Inhabit` thành đảo.** Mỗi thiết bị giữ một đảo kín, cỡ theo sức máy, tự chạy nhịp của mình.
  Sinh vật di cư nguyên con, kèm năng lượng, qua host. Không có va chạm xuyên máy.

Cả hai là suy luận của báo cáo D.

### 8.3 Chỗ thiết kế 25/08 cần sửa

[Thiết kế tính toán phân tán](../ai/design/2026-08-25-feature-federated-compute.md) §4 dùng
lockstep theo rào nhịp và §5.2 chia dải không gian liên tục. Đó là hai trong bốn kiến trúc ở 8.1.
Thiết kế tự ghi cái giá của lựa chọn đó: máy chậm nhất định nhịp. Báo cáo D cho rằng cái giá ấy
không chịu được trên mạng gia đình, nhưng không có số đo; kết luận này là suy luận.
Ba phần của thiết kế vẫn đứng vững: ba chính sách liên bang, ngăn ngủ đông, và năm lớp tin cậy
(xem §9). Việc sửa là QĐ-M7 của kế hoạch định hướng.

### 8.4 Thiết bị rời mạng

Hợp đồng thuê chỉ theo thời gian là ngõ cụt: một máy bị treo rồi tỉnh lại sẽ ghi muộn. Cần thêm
một **mã rào tăng dần**: khi hết hạn, host tăng mã, đưa ảnh chụp cuối cùng đã kiểm năng lượng vào
ngăn ngủ đông, và từ chối mọi ghi mang mã cũ. Tiến độ sau ảnh chụp đó mất. (Nguồn ngoài cho
nguyên lý; suy luận cho cách áp vào Anima.)

Câu hỏi chưa ai trả lời: người dùng rút điện ngay trước một ảnh chụp để "chơi lại" kết quả xấu.

### 8.5 Mô hình đảo

Các đảo bắt đầu ở vùng khác nhau, giữ đa dạng lâu hơn, và thắng một quần thể lớn khi bài toán
tách được. Một quần thể lớn thắng khi đảo quá nhỏ. Di cư mỗi thế hệ làm các đảo giống hệt nhau.
(Nguồn ngoài, từ nghiên cứu về thuật toán di truyền.) Đề xuất: vòng tròn, di cư thưa, ít cá thể,
chọn cá thể khác biệt thay vì tốt nhất, cá thể rời đảo thì bị xoá khỏi đảo cũ.

Khoảng di cư và số cá thể: **không tìm thấy**.

## 9. Tin kết quả từ thiết bị lạ

| Cách | Giá | Bỏ sót gì | Nhãn |
|---|---|---|---|
| Bốn lớp bất biến (T1 đến T4 của thiết kế 25/08) | Rất rẻ | "Tiến hoá chọn lọc": thiết bị chạy nhiều nhánh đột biến hợp lệ rồi nộp nhánh nó chọn | Suy luận |
| Host chạy lại toàn bộ | Bằng cả phép tính | Không gì; nhưng khi đó không cần máy góp | Suy luận |
| Chạy tất định từng bit, kiểm ngẫu nhiên, tỉ lệ kiểm giảm dần theo chuỗi kết quả đúng của từng thiết bị | Cao lúc đầu, tiến về thấp | Một kẻ tạo nhiều danh tính mới | Nguồn ngoài cho nguyên lý |

Ba hệ quả:

- **Cam kết seed trước.** Host giao seed; thiết bị không tự chọn. Kiểm cả ảnh chụp giữa kỷ
  nguyên, vì chỉ kiểm nhịp cuối thì bỏ sót sửa đổi ở giữa.
- **So sánh có dung sai không dùng được** cho một hệ khuếch đại sai số (§5.4). Chỉ so bằng nhau
  tuyệt đối, và chỉ giữa các máy cùng lớp nền tảng cho tới khi QĐ-M6 chọn mức tuyệt đối.
- **Tỉ lệ 1% của lớp T5** chưa có báo cáo nào đánh giá.

## 10. Lớp mạng

| Phương án | Kết luận của báo cáo D | Nhãn |
|---|---|---|
| Tự thêm TLS, xác thực thiết bị và đi qua NAT lên đường WebSocket hiện có | Ngõ cụt | Suy luận |
| WireGuard hoặc Tailscale trong một ứng dụng công khai | Không | Suy luận |
| Nhúng một thư viện QUIC ngang hàng (iroh) vào Anima cho `Inhabit`, sau một trait vận chuyển mỏng | Nên | Suy luận; rủi ro: thư viện còn trẻ, API đổi, relay công cộng bị giới hạn tốc độ |
| Dùng chế độ lô có sandbox của OxideSwarm cho `Contribute` | Nên, không cần lưới thay đổi gì | Suy luận |
| Chờ lưới có dịch vụ chạy lâu và đường hầm | "Ngõ cụt về lịch" | Suy luận |

Hai ghi chú mà báo cáo không có:

- OxideSwarm đã dùng chính thư viện iroh đó. Hai dự án dùng chung một lớp vận chuyển là khả
  thi mà không cần cái này chờ cái kia.
- Các thử nghiệm của báo cáo giả định đường WebSocket chạy được giữa hai máy. Mã hiện tại chỉ
  nối `127.0.0.1` (đã kiểm).

Các hệ quả này đi vào QĐ-M3 của kế hoạch định hướng.

## 11. Phát hành

| Điều tìm thấy | Nhãn |
|---|---|
| `hexf-parse` chỉ được kéo vào bởi `naga`, tức là qua đường GPU | **Đã kiểm** trong `src-tauri/Cargo.lock` |
| CC0 không buộc phải kèm văn bản giấy phép. Thứ thực sự chặn là công cụ kiểm giấy phép và dịch vụ ký miễn phí cho mã nguồn mở, vốn đòi giấy phép được OSI công nhận; CC0 không nằm trong số đó | Nguồn ngoài |
| Windows: bản cài không ký bị cảnh báo. Dịch vụ ký miễn phí cho mã nguồn mở đòi đã có một bản phát hành công khai, xác thực nhiều lớp và một trang chính sách ký | Nguồn ngoài |
| macOS: cần tài khoản nhà phát triển trả phí, chứng chỉ và công chứng | Nguồn ngoài |
| Tự cập nhật: chữ ký là bắt buộc. Mất khoá riêng thì mọi người dùng hết nhận được bản cập nhật | Nguồn ngoài |

Hệ quả: phương án (A) của QĐ-C, bỏ `ml-wgpu` khỏi feature `desktop`, gỡ luôn `naga` và
`hexf-parse` khỏi bản phát hành. Báo cáo D không xét phương án này vì nó không biết tên thư viện.

Repo hôm nay chưa bật tự cập nhật: không có plugin cập nhật trong `src-tauri/Cargo.toml` hay
`src-tauri/tauri.conf.json` (đã kiểm). Việc mới cho chặng phát hành là một quyết định: có bật
không, và nếu có thì khoá ký để ở đâu, **trước** bản phát hành đầu tiên (QĐ-M8).

## 12. Người dùng đầu tiên

| Dự án | Điều đã làm | Nhãn |
|---|---|---|
| Lenia | Bản chạy trên trình duyệt kèm danh mục loài, ra trước bài báo | Nguồn ngoài |
| Avida-ED | Làm thí nghiệm không cần viết mã | Nguồn ngoài |
| The Bibites | Miễn phí, video theo giả thuyết, chia sẻ file lưu | Nguồn ngoài |
| ALIEN | Có máy chủ MCP dựng sẵn để agent điều khiển mô phỏng | **Đã kiểm** trong README của nó |

Không dự án nào trong bốn cái cần nhiều máy lúc ra mắt. Đề xuất của báo cáo D cho Anima: bản một
máy cài được, thế giới mẫu, xuất và nhập sinh vật thành file, và công bố thí nghiệm.

## 13. Dùng lưới cho Anima

Báo cáo E xếp hạng năm cách dùng:

| Cách dùng | Phán quyết | Ghi chú |
|---|---|---|
| Build và test từ xa trên một máy mạnh, giữ thư mục build giữa các lượt | Hợp, làm trước | Giải đúng nút thắt máy phát triển |
| Loạt thí nghiệm: mỗi seed một lượt chạy trọn thế giới | Hợp | Cần một bộ chia và gộp manifest; rủi ro lệch dấu phẩy động giữa các kiến trúc |
| Địa hình và World Artifact gửi dưới dạng file | Một phần, chỉ kèm với loạt thí nghiệm | |
| Đánh giá não độc lập trên GPU | Một phần | GPU phá tất định từng bit |
| Thế giới thời gian thực chia mảnh | Không hợp | Một máy bàn còn rất xa mức đầy tải, nên chia mảnh không giải nút thắt nào |

Dòng cuối dựa trên số đo của chính repo ở `BENCHMARK_BASELINE.md`; số đó là cận dưới, chưa tính
suy luận của não, nên kết luận là suy luận.

**Sai:** bốn công thức thí nghiệm của báo cáo E dùng lệnh con và cờ mà `anima-lab` không có.
Chương trình chỉ nhận `--model` và một đường dẫn manifest (đã kiểm trong
`src-tauri/src/bin/anima-lab.rs`). Không chạy các công thức đó nguyên văn.

## 14. Sai hoặc đáng ngờ, gom lại

- Báo cáo C đề xuất mortality và chi phí như thể chưa có; thân cây đã có sau khoá manifest.
- Báo cáo C lấy bằng chứng về Bevy và Burn từ các bản mới hơn bản Anima đang dùng.
- Nguồn chính cho mục thống kê của báo cáo C có tiêu đề không liên quan.
- Hai issue được trích cho tính tất định (một của Bevy, một của Burn) nói về chuyện khác.
- Báo cáo D coi vướng mắc CC0 là chặn cả dự án; nó chỉ chặn đường GPU.
- Báo cáo D và E đặt "bằng nhau giữa các nền tảng" làm điều kiện tiên quyết; hợp đồng hiện tại
  không hứa điều đó, và §5.5 coi nó là một lựa chọn.
- Báo cáo E dùng một số đo debug với 10 agent làm giá của một nhịp trong ước lượng thời gian.
- Báo cáo E dùng lệnh không tồn tại (§13).
- Một phép tính phần trăm trong báo cáo D sai ở chữ số cuối.

## 15. Tác động tới kế hoạch

| Mục của tài liệu này | Thay đổi trong kế hoạch định hướng |
|---|---|
| §2, §3, §4, §6 | Chặng 4: bổ sung vào các thẻ đã có (M2, S1, S2, E2R-P, E3-P), không thêm thẻ mới |
| §5 | Thêm QĐ-M6; NM-2 trở thành phép đo đầu vào cho quyết định đó |
| §7 | Luồng D: nhật ký dùng AI, bắt đầu ngay |
| §8, §9 | Chặng 7 và QĐ-M7: sửa thiết kế 25/08 trước khi có mã; `Contribute` thành lô, `Inhabit` thành đảo |
| §10 | QĐ-M3 có thêm một đề xuất |
| §11 | Chặng 5: QĐ-C phương án (A); QĐ-M8 về ký và tự cập nhật (MP-5.5) |
| §12 | Chặng 5: thế giới mẫu, xuất và nhập sinh vật (MP-5.6) |
| §13 | NM-1 đứng đầu luồng nhiều máy, như đã ghi |

## 16. Những điều chưa trả lời được

- Giá về tốc độ của hồ sơ CPU tất định.
- Burn trên CPU có cho kết quả bằng nhau giữa x86_64 và ARM64 không.
- Số seed và cỡ hiệu ứng cho E2R và E3.
- Khoảng di cư và cỡ đảo tối thiểu.
- Tỉ lệ kiểm ngẫu nhiên đủ dùng.
- Hạn nộp của hội nghị ALIFE kỳ tới.
- Cách chặn việc rút điện trước ảnh chụp.
