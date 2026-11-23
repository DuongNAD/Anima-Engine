---
title: Chẩn đoán nhánh scarce_mut0 sụp đổ — quần thể sáng lập là một dòng vô tính kém thích nghi
status: active
owner: lead
last_reviewed: 2026-11-23
review_cycle: weekly
description: VIAB-1 thấy nhánh không đột biến tuyệt chủng cả khi không khan hiếm; chẩn đoán cho thấy đây không phải lỗi code mà là genotype sáng lập cố định kém thích nghi, và điều đó đổi cách thiết kế E3b và SEL2-P2
kind: đo
---

# Chẩn đoán: nhánh `scarce_mut0` sụp đổ

Đây là ghi chú chẩn đoán, chỉ đo, không sửa mã nguồn. Probe chạy trên macOS tại trunk `e576350`, với các seed
thăm dò 880001–880006. Mục FINDINGS chính thức (dự kiến F-067) sẽ được thêm sau khi PR #200 và #207 merge, để
khỏi xung đột ở đầu `FINDINGS.md`.

## Kết luận

**Đây là hiệu ứng tiến hoá thật, không phải lỗi code.** Độ tin cậy cao cho vế "không phải lỗi", trung bình-cao
cho cơ chế.

- Mọi cá thể sáng lập là **cùng một genotype 3 đốt**, khai cứng trong `live_experiment.rs::genesis`: khối
  lượng các đốt là 1,5, 1,0 và 0,8, tổng 3,3. Lead đã đọc lại đoạn code này.
- Với `live.mutation_rate = 0`, quần thể không có biến dị di truyền nào, nên mãi giữ kiểu hình đó. Kiểu hình
  này chỉ sống được ở mức biên: dự trữ khoảng 30–38 EU so với mục tiêu 100, và chết chủ yếu vì đói.
- Với rate 0,05, quần thể nhanh chóng tìm ra và cố định một đột biến: bỏ đốt lá "đuôi" (toán tử 4 của
  `mutate_genotype`). Thân 2 đốt nặng 2,5, dự trữ lên khoảng 75–80 EU, và số chết đói giảm gần về 0.

Nhánh mut0 chết theo hai kiểu:

- **Sớm** (1–2 "năm", mỗi năm 6.000 tick): quần thể sáng lập sụp đổ. Nhánh có đột biến vượt qua được nhờ
  cứu hộ tiến hoá.
- **Muộn** (sau 100k tick): một quần thể ở mức biên tuyệt chủng ngẫu nhiên sau một chuỗi năm xấu.

## Bằng chứng

**Từ dữ liệu VIAB-1 (PR #215).** 210 lượt mut0 trên 25 ô SEL2-P, trong đó 205 tuyệt chủng. Các ô dùng lại cùng
seed và cùng nhánh, nên các lượt này không độc lập.

- Tick tuyệt chủng: trung vị 21.400, phân vị 10 là 4.400, phân vị 90 là 124.800.
- Trong 147 cặp mà nhánh tham chiếu sống còn mut0 chết:
  - tỉ lệ chết đói của mut0 có trung vị 0,92, so với 0,70 ở tham chiếu;
  - tuổi thọ trung bình của mut0 thấp hơn ở 100 % số cặp (trung vị 2.665 tick so với 5.327).

**Đọc code** (`agent_systems.rs` 691–694 và 1105–1127, `mutation.rs::mutate_genotype`). Không có đường nào
khiến rate 0 làm gì khác ngoài "con giống hệt cha":

- `gen_bool(0)` vẫn rút từ RNG như khi rate > 0 mà không có đột biến nào xảy ra.
- Nhánh mutate não bị khoá vì SEL2-P đặt `live.evolved_brains = 0`.
- Năng lượng con nhận không phụ thuộc genotype.
- `NodeIdExhausted` bị bỏ qua, không bao giờ huỷ một ca sinh.
- EU khép kín chỉ trôi khoảng 1e-8.

Kiểm chứng thực nghiệm: rate 0,001 và rate 0 cho checksum trùng từng bit ở các seed mà chưa đột biến nào xảy ra
(880003, 880004, 880006).

**Probe A.** Chạy `sel2p-baseline-ref.json` với `strict_prey_cap = 1`, 200k tick. Seed sống sót: 5/6 với rate
0,05; 2/6 với 0,001; 1/6 với 0. Kết quả này tái hiện đúng mẫu của VIAB-1.

**Probe B.** Đổi rate giữa lượt chạy, tại tick 30k:

- Chuyển 0 → 0,05: 3/3 seed còn sống được cứu, cuối lượt thân 2 đốt cố định ở 78–80/80 cá thể.
- Chuyển 0,05 → 0: 5/5 seed sống tới 200k. Một khi đã có genotype thích nghi, quần thể không cần đột biến
  liên tục nữa, tức đột biến không phải là một khoản "trợ cấp" năng lượng.

## Hệ quả cho E3b và SEL2-P2

1. **SEL2-P2.** Theo thiết kế hiện tại, nhánh đối chứng `scarce_mut0` không bao giờ qua được C1. Một nhánh
   mut0 sống được cần các cá thể sáng lập đã thích nghi, ví dụ genotype 2 đốt, hoặc một "bể sáng lập đã thích
   nghi" khai báo trước. Đây là quyết định thiết kế cho chủ dự án và lead.
2. **E3b, và cả việc đọc lại E3.** Ngay ở độ phong phú (scale 1,0), nhánh có đột biến đã giảm khối lượng cơ thể
   từ 3,3 xuống khoảng 2,5, vì cá thể sáng lập kém thích nghi. Mọi nhánh của E3b đều xuất phát từ cùng dòng vô
   tính đó, nên:
   - tín hiệu "khan hiếm làm giảm khối lượng" sẽ chồng lên một xu hướng giảm khối lượng có sẵn ở mọi nhánh;
   - tương phản giữa các nhánh vẫn hợp lệ nhờ ghép cặp theo seed, nhưng diễn giải phải nói rõ điều này;
   - đường cơ sở cần được đo;
   - `trait_mass_var` bắt đầu từ 0 trong mọi nhánh, vì mọi cá thể giống hệt nhau.

   Bản nháp đăng ký E3b phải nêu điều này và đưa "bể sáng lập" thành một ô `OWNER DECISION`.
3. **Chưa làm.** Chưa so mut0 với cá thể sáng lập 2 đốt, vì phải đổi genotype ở genesis. Chưa có đường cong
   sống sót trên nhiều seed hơn cho vế "tuyệt chủng muộn là ngẫu nhiên". Hai việc này cần thành một thẻ có
   uỷ quyền nếu chủ dự án muốn.

## Ghi chú phụ

Các cạnh của genotype sáng lập có `joint_anchor.x = 1.0`, nằm ngoài bán kính 0,2 của đốt cha. Toán tử 1 và 2
kẹp anchor vào trong bán kính, nên genotype sáng lập vi phạm bất biến đó. Đây không phải nguyên nhân ở đây:
thân 2 đốt thắng thế vẫn giữ anchor nằm ngoài bán kính. Nên ghi thành một finding riêng.

Probe là code dùng một lần, không commit (`mut0_probe.rs`, `mut0_switch.rs`). Chẩn đoán này **đã** xem
`trait_mass_*`, nhưng chỉ trên seed thăm dò, không phải seed của bản đăng ký nào.
