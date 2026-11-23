---
title: Mẫu thống kê cho bản đăng ký E3b và SEL2-P2
status: proposed
owner: lead
last_reviewed: 2026-11-23
review_cycle: weekly
description: Mười quy tắc thống kê mà bản đăng ký E3b và SEL2-P2 phải điền trước khi đóng băng, kèm bảng cỡ mẫu tính sẵn, rút ra từ việc E3 không thể ra quyết định
kind: thẻ
---

# Mẫu thống kê cho bản đăng ký E3b và SEL2-P2

Tài liệu này là **đề xuất của lead**, chưa phải quyết định. Nó không thay bản đăng ký nào, không đặt cổng
mới, và không chọn SESOI thay chủ dự án. Nó liệt kê những gì một bản đăng ký mới phải khai trước để không
lặp lại cách E3 kết thúc, và cho sẵn các con số cỡ mẫu để khỏi phải tính lại mỗi lần.

Nguồn: kết quả E3 ([ghi chú E3-X](2026-11-20-e3-x-confirmation-results.md)), ghi chú nghiên cứu
[khả năng sống sót và thống kê](../../research/VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md) §3.3–3.5,
và các con số tính lại ở §4 bằng script trong phụ lục. Các con số trong ghi chú nghiên cứu mà nó đánh dấu
"chưa xác minh" **không** được dùng ở đây.

## 1. Vì sao E3 không thể ra quyết định

E3 chạy 12 seed × 3 nhánh, loại mọi cặp có một bên tuyệt chủng, và đòi tối thiểu 10 cặp hoàn chỉnh
(`min_valid_pairs_for_decision = 10`). Nhánh khan hiếm tuyệt chủng ở 6/12 seed, nên còn 6 cặp và kết luận là
`insufficient_evidence`.

Kết quả đó gần như đã được định sẵn bởi thiết kế:

- Với 12 seed và xác suất cả hai nhánh cùng sống sót trên mỗi seed $\pi_{LL} \approx 0{,}5$, xác suất có
  từ 10 cặp hoàn chỉnh trở lên là **1,9 %**.
- Kể cả có đủ 10 cặp, sức mạnh của chính quy tắc E3 ($d_z \ge 0{,}8$, hai phía, $\alpha = 0{,}05$) vẫn dưới
  80 %: cần khoảng 15 cặp cho phép kiểm định t ghép cặp. Với 6 cặp thì sức mạnh chỉ là 0,36.
- Với 6 cặp, giá trị p nhỏ nhất mà phép đổi dấu hay Wilcoxon chính xác đạt được là $2/2^6 = 0{,}031$. Ngưỡng
  bước đầu của Holm với ba giả thuyết là $\alpha/3 = 0{,}0167$, nên **không dữ liệu nào** qua được.

Bài học: số seed phải được chọn từ tỉ lệ sống sót đo được, không được chọn trước rồi hy vọng đủ cặp.

## 2. Số seed không phải là chi phí

- E3-X: 36 lượt × 18.000 tick hết 153,55 s trên Windows 1 (ghi chú E3-X §1).
- VIAB-1 pha B, SEL2-P: một ô 48 lượt hết 560–722 s (báo cáo của `anima-case`, 2026-11-23).

Nhân đôi hay nhân ba số seed chỉ tốn thêm vài phút đến dưới một giờ máy. Một vòng thí nghiệm không quyết
định được thì tốn nhiều ngày. Vì vậy mẫu này luôn chọn phía nhiều seed.

## 3. Mười quy tắc bản đăng ký phải điền

Mỗi quy tắc là một ô bản đăng ký phải điền bằng giá trị cụ thể trước khi đóng băng. Ô để trống là lý do trả
bản đăng ký về.

| # | Quy tắc | Bản đăng ký phải khai |
|---|---|---|
| R1 | Đơn vị thực nghiệm là seed. Các nhánh ghép cặp theo seed (CRN). Seed xác nhận là một dải mới, tách hẳn khỏi seed hiệu chuẩn của VIAB-1 và của E3 | Dải seed xác nhận; danh sách seed hiệu chuẩn bị cấm |
| R2 | Họ khẳng định có nhiều nhất 3 giả thuyết, kiểm soát bằng Holm. Mọi so sánh khác thuộc họ khám phá (Benjamini–Hochberg) | Danh sách giả thuyết chính, thứ tự, $\alpha$ |
| R3 | Chỉ số chính tính cả tuyệt chủng, chọn theo $p_{\text{ext}}$ đo ở VIAB-1 (xem dưới bảng) | $p_{\text{ext}}$ từng nhánh, cận trên 80 %; chỉ số chính đã chọn |
| R4 | SESOI khai trước, trên thang của chỉ số chính. Không lấy cỡ hiệu ứng từ pilot hay từ E3 | SESOI và lý do khoa học |
| R5 | Tham số nhiễu ($\sigma$, $\rho_{\text{CRN}}$, $p_{\text{ext}}$, $\pi_{LL}$) chỉ lấy từ seed hiệu chuẩn, và chỉ từ biến sống sót và biến cổng. Không từ khối lượng cơ thể (QD-VIAB) | Nguồn của từng tham số, file và commit |
| R6 | Số seed chọn theo §4: số cặp cần cho sức mạnh ≥ 0,8 ở ngưỡng Holm bước đầu, rồi đổi sang số seed theo $\pi_{LL}$ | Số cặp, số seed, phép tính |
| R7 | Kiểm tra trước rằng p nhỏ nhất đạt được với số cặp tối thiểu nhỏ hơn ngưỡng Holm bước đầu ($n \ge 7$ với $\alpha/3$) | Số cặp tối thiểu và p nhỏ nhất của nó |
| R8 | Cỡ hiệu ứng ghép cặp báo cáo đủ bộ: $d_z$, $r_c$ (rank-biserial ghép cặp), giả trung vị Hodges–Lehmann với CI bootstrap BCa 95 %, và tỉ lệ seed có hiệu dương | Có / không |
| R9 | TOST với biên ±SESOI (CI 90 %) để tách "tương đương 0" khỏi "chưa đủ bằng chứng". Nhờ vậy một kết quả âm có sức mạnh vẫn là kết quả, và vẫn qua cổng Chặng 4 | Biên TOST |
| R10 | Bảng quyết định đóng: mỗi tổ hợp kết quả của R2, R3 và R9 ánh xạ tới đúng một giá trị trong `{supports, equivalent_to_zero, insufficient_evidence}` | Bảng ánh xạ |

**R3, chọn chỉ số chính theo tỉ lệ tuyệt chủng.** Ngưỡng 5 % và 0,85 dưới đây là đề xuất của lead, chưa được ai duyệt:

- $p_{\text{ext}} \le 5\,\%$ ở mọi nhánh: phân tích trên các cặp hoàn chỉnh được làm chính. Chỉ số worst-rank của Lachin đi kèm làm độ nhạy.
- $p_{\text{ext}} > 5\,\%$ ở bất kỳ nhánh nào: chỉ số worst-rank trên **mọi** seed làm chính, không loại seed nào. Kèm theo đó:
  - RMST (thời gian sống trung bình bị chặn tại $\tau$) cho phần sống sót;
  - SACE trên tầng sống ở cả hai nhánh, chỉ khi $\hat{\pi}_{LL} \ge 0{,}85$.

  Các thí nghiệm hiện đã ghi `first_extinct_tick`, đủ để tính cả hai.

## 4. Bảng cỡ mẫu tính sẵn

**Số cặp cần cho phép kiểm định ghép cặp hai phía.** Cột Wilcoxon là số của phép t chia cho hiệu suất
tương đối tiệm cận 0,955.

| $\alpha$ | $d_z$ | Sức mạnh 0,8: t / Wilcoxon | Sức mạnh 0,9: t / Wilcoxon |
|---|---|---|---|
| 0,05 | 0,5 | 34 / 36 | 44 / 47 |
| 0,05 | 0,8 | 15 / 16 | 19 / 20 |
| 0,05 | 1,0 | 10 / 11 | 13 / 14 |
| 0,0167 (Holm, $m = 3$) | 0,5 | 45 / 48 | 57 / 60 |
| 0,0167 | 0,8 | 20 / 21 | 25 / 27 |
| 0,0167 | 1,0 | 14 / 15 | 17 / 18 |

**Số seed để có từ $K$ cặp hoàn chỉnh trở lên với xác suất ≥ 0,9** (phân phối nhị thức theo $\pi_{LL}$).
Bảng này chỉ dùng khi chỉ số chính là phân tích trên các cặp hoàn chỉnh.

| $K$ | $\pi_{LL} = 0{,}5$ | 0,6 | 0,7 | 0,85 | 0,95 |
|---|---|---|---|---|---|
| 10 | 26 | 21 | 18 | 14 | 12 |
| 12 | 31 | 25 | 21 | 16 | 14 |
| 16 | 39 | 32 | 27 | 21 | 18 |
| 20 | 48 | 40 | 33 | 26 | 22 |

**Ví dụ.** SESOI $d_z = 0{,}8$, ba giả thuyết chính, chỉ số chính là phân tích trên cặp hoàn chỉnh:

1. Cần 21 cặp (Wilcoxon ở $\alpha/3$).
2. Nếu VIAB-1 đo được $\pi_{LL} = 0{,}85$, tra bảng thứ hai ở dòng $K = 20$: cần khoảng 26 seed. Làm tròn lên thành 30.
3. Nếu chỉ số chính là worst-rank thì không loại seed nào. Khi đó 21 seed là đủ cho sức mạnh, và vẫn nên chọn từ 24 trở lên để dư.

Chi phí của 30 seed × 3 nhánh × 18.000 tick là khoảng 6–7 phút trên Windows 1, ngoại suy từ E3-X.

## 5. Điều mẫu này không làm

- Không chọn SESOI và không chọn chỉ số chính. Hai việc đó thuộc bản đăng ký và chủ dự án.
- Không thay quy tắc `dz_min` hay `relative_delta_min` của E3. Bản E3 đã đóng băng giữ nguyên.
- Bảng sức mạnh giả định phân phối hiệu số gần chuẩn. Với worst-rank hay RMST, sức mạnh phải tính bằng
  mô phỏng Monte Carlo trên tham số nhiễu từ seed hiệu chuẩn, trước khi đóng băng.
- Không tách luồng RNG theo thành phần. Việc đó đổi quỹ đạo, nên là một khoá opt-in riêng theo ADR-0008.

## Phụ lục: script tính lại

Chạy bằng `python3 -I` với SciPy. Kết quả phải khớp §1 và §4.

```python
import numpy as np
from scipy import stats

# Giá trị p hai phía nhỏ nhất của phép đổi dấu / Wilcoxon chính xác với n cặp
for n in [5, 6, 7, 8, 10, 12]:
    print(n, 2 / 2**n)

# Số seed N để P(số cặp hoàn chỉnh >= K) >= 0,9
for K in [10, 12, 16, 20]:
    for pi in [0.5, 0.6, 0.7, 0.85, 0.95]:
        N = K
        while stats.binom.sf(K - 1, N, pi) < 0.9:
            N += 1
        print(K, pi, N)

# Số cặp cho phép t ghép cặp hai phía (t không trung tâm)
def n_needed(dz, alpha, power):
    for n in range(4, 500):
        df = n - 1
        nc = dz * np.sqrt(n)
        t = stats.t.ppf(1 - alpha / 2, df)
        p = 1 - stats.nct.cdf(t, df, nc) + stats.nct.cdf(-t, df, nc)
        if p >= power:
            return n

for alpha in [0.05, 0.05 / 3]:
    for dz in [0.5, 0.8, 1.0]:
        print(alpha, dz, n_needed(dz, alpha, 0.8), n_needed(dz, alpha, 0.9))

# Kiểm tra E3
print(stats.binom.sf(9, 12, 0.5))  # 0,0193
```
