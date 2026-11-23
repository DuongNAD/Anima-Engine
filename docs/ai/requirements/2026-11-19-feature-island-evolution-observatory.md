---
phase: requirements
feature: island-evolution-observatory
title: Requirements — Đài quan sát tiến hoá trên đảo (đề xuất chủ lực)
description: Đề xuất chọn một câu hỏi khoa học chủ lực cho Anima, là cơ chế của quy luật đảo (kích thước cơ thể khi bị cô lập), kiểm bằng quần đảo nhiều thế giới sống, đăng ký trước và chạy trên các máy sẵn có
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-19
contract: ../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md
---

# Đài quan sát tiến hoá trên đảo — đề xuất chủ lực

> **Chấp nhận ngày 2026-11-19.** Phiên lead soạn theo yêu cầu của chủ dự án ("làm cả hai, soạn đề xuất chủ
> lực trước"), sau câu hỏi "có điều gì có thể làm cho dự án này xịn hơn, hoàn thiện và đóng góp được cái gì
> to lớn". Chủ dự án chọn phương án đề xuất ở cả bốn câu của §11 cùng ngày. Tài liệu này không đổi luật
> hay thí nghiệm đã đăng ký; thẻ đầu tiên (M1) mở sau smoke SEL2-P.

## 1. Tóm tắt

Anima đã có ba thứ hiếm khi đi cùng nhau:

- một thế giới sống có sổ năng lượng khép kín;
- quy trình thí nghiệm đăng ký trước, có nhánh không đột biến, nhánh trôi dạt và đối chứng âm;
- một kiến trúc nhiều máy, gồm đảo theo QĐ-M7 và lưới tính toán OxideSwarm.

Đề xuất dùng cả ba cho **một** câu hỏi mà sinh học thật khó trả lời vì không tua lại được tiến hoá:

> **Khi một quần thể bị cô lập trên đảo, kích thước cơ thể thay đổi theo hướng nào, và vì cơ chế nào:
> thiếu tài nguyên hay thoát khỏi kẻ săn mồi?**

Quy luật đảo (*island rule*) nói loài lớn hoá lùn và loài nhỏ hoá khổng lồ trên đảo. Dữ liệu thật vẫn
đang tranh cãi: Meiri và cộng sự (2008) cho rằng quy luật này yếu và không nhất quán; Benítez-López và
cộng sự (2021) thấy nó đúng trên diện rộng ở động vật có xương sống trên cạn. Ở thiên nhiên, hai cơ chế
thường được nêu (giới hạn tài nguyên; thoát khỏi kẻ săn mồi và cạnh tranh) luôn đi cùng nhau. Trong
Anima có thể **bật tắt từng cơ chế** theo thiết kế giai thừa, lặp lại hàng chục seed, và đăng ký trước.

Đóng góp nhắm tới là một kết quả trong thế giới mô hình, công bố kèm code, manifest và dữ liệu. Nó trả
lời cơ chế nào đủ để sinh ra hướng thay đổi nào, và ở điều kiện nào. Nó **không** khẳng định điều gì về
loài thật (§12).

## 2. Vì sao câu hỏi này, mà không phải câu khác

| Ứng viên | Vì sao không chọn làm chủ lực lúc này |
|---|---|
| Tiến hoá mở (open-ended evolution) | Câu hỏi lớn nhất của ngành ALife (Packard và cộng sự, 2019), nhưng chưa có tiêu chí thành công được đồng thuận. 1.000 sinh vật là quá nhỏ. Rủi ro không ra kết quả nào trong một năm là cao |
| Thế giới khác luật ("mana", ADR-0002) | Độc đáo nhất, nhưng khó nối với một lý thuyết sinh học có thể bác bỏ. Hợp làm chương sau, khi đã có dụng cụ được tin |
| Nữ hoàng Đỏ (S43) | Đúng hướng nhưng cần một đại lượng "tốc độ đồng tiến hoá" chưa có trong mã. Giữ làm H3 (§3) |
| **Quy luật đảo** | Lý thuyết có tên, có tranh cãi thật, có dự đoán có hướng. Chỉ tiêu (khối lượng cơ thể theo kiểu gen) **đã có** và đang được dùng trong SEL2-P. Kiến trúc đảo của dự án chính là thiết kế thí nghiệm |

SEL2-P (thiếu thức ăn dẫn tới đổi khối lượng cơ thể, một quần thể) là **viên gạch đầu tiên** của hướng
này: nó kiểm cơ chế tài nguyên trên một đảo. Đề xuất này mở rộng nó, không thay nó.

## 3. Câu hỏi và giả thuyết (bản nháp, chưa đăng ký)

- **H1 (hướng phụ thuộc kích thước tổ tiên).** So với đất liền đối chứng, quần thể cô lập trên đảo
  sức chứa thấp có khối lượng cơ thể theo kiểu gen giảm khi tổ tiên lớn, và tăng khi tổ tiên nhỏ.
- **H1-R (cơ chế tài nguyên).** Chỉ hạ sức chứa (giữ nguyên kẻ săn mồi) đủ để sinh ra phần *giảm* của H1.
- **H1-P (cơ chế thoát săn mồi).** Chỉ bỏ kẻ săn mồi (giữ nguyên sức chứa) đủ để sinh ra phần *tăng* của H1.
- **H2 (đa dạng theo kích thước và cô lập, kiểu MacArthur–Wilson).** Ở trạng thái cân bằng, đa dạng
  kiểu gen trên đảo tăng theo sức chứa và tốc độ nhập cư. Đo bằng đa dạng dòng dõi và kiểu gen, **không**
  gọi là "loài" (cổng AE-S11/AE-S14).
- **H3 (để sau).** Dòng gen giữa các đảo làm chậm hay nhanh nhịp đồng tiến hoá vật săn và con mồi (S43).

Mỗi giả thuyết có một kết quả rỗng được viết trước. Ví dụ với H1: khác biệt nằm trong vùng trôi dạt, tức
là nhánh trôi dạt SEL2-D cho cùng độ lệch.

## 4. Thiết kế thí nghiệm

**Một quần đảo** là N thế giới sống có cùng `WorldLawSet`, chạy trong một tiến trình, với di cư theo
lịch tất định giữa chúng. **Đất liền** là một thế giới sức chứa cao, có kẻ săn mồi.

**Các yếu tố (giai thừa 2×2×2), cho H1, H1-R và H1-P:**

| Yếu tố | Mức | Khoá hiện có |
|---|---|---|
| Sức chứa đảo | thấp / cao | `live.prey_cap`, `live.resource_capacity_scale` (đã dùng ở S14, S15, SEL2-P) |
| Kẻ săn mồi | có / không | `live.predator_fraction` |
| Kích thước tổ tiên | nhỏ / lớn | **chưa có**: cần một khoá founder mới (§5) |

**Di cư:** `m ∈ {0, thấp}` (cô lập hoàn toàn so với cô lập một phần), cho H2.

**Đối chứng** (kế thừa nguyên mẫu SEL2-P và E-SEL1):
- nhánh không đột biến (`live.mutation_rate = 0`);
- nhánh trôi dạt (`live.drift_control`, SEL2-D);
- đối chứng âm cho mọi cổng (thẻ M2).

**Chỉ tiêu chính:** khối lượng cơ thể theo kiểu gen, tích phân theo thời gian sau giai đoạn khởi động,
cộng phân rã Robertson–Price. Đây là chỉ tiêu SEL2-P đã đăng ký, dùng lại để hai kết quả so được với nhau.

**"Kích thước đảo" là sức chứa, không phải diện tích.** Đổi diện tích thế giới là đổi luật, và ADR-0010
để ngỏ việc này. Đề xuất dùng sức chứa làm đại diện và nói rõ như vậy khi công bố. Chủ dự án quyết ở §11.

## 5. Đã có và còn thiếu

| Thành phần | Trạng thái | Ghi chú |
|---|---|---|
| Thế giới sống chạy headless, tất định theo seed | ✅ có | `core/live_experiment.rs` (`LiveExperimentAdapter`), lịch tick duy nhất ở `core::simulation_schedule::build_tick_schedule` |
| Bộ chạy thí nghiệm, manifest, đăng ký trước | ✅ có | `experiment_runner`; mẫu E-SEL1 và SEL2-P (fixture, runner, verifier, test manifest) |
| Nhánh trôi dạt | ✅ có | `run_live_drift_pair` (SEL2-D) |
| Chỉ tiêu khối lượng, đa dạng | ✅ có | `live.trait_mass_mean`, `live.trait_mass_var`, `live.shannon`, `live.simpson` |
| Tuần tự hoá một cá thể, lệnh di cư | ✅ có | `spawn_serialized_agent`; `SpawnMigrationCommand` (`core/world_systems.rs`), mang theo não được giao (D01) |
| **Bộ chạy quần đảo** | ❌ thiếu | N thế giới trong một tiến trình, trao đổi cá thể theo lịch tất định, một seed gốc sinh seed từng đảo |
| **Sổ năng lượng khi di cư** | ❌ thiếu | EU đi theo cá thể phải ghi xuất ở đảo đi và nhập ở đảo đến; cổng: tổng EU quần đảo bảo toàn |
| **Khoá kích thước tổ tiên** | ❌ thiếu | founder lớn hoặc nhỏ theo khai báo, không đọc môi trường trong `decode_genotype` (luật cứng ADR-0001) |
| **Chỉ tiêu theo đảo và quần đảo** | ❌ thiếu | trung bình theo đảo, khác biệt đảo với đất liền, đa dạng gộp |
| Kiểm chéo kết quả giữa nhiều máy | ⚠️ một phần | quỹ đạo thế giới sống lệch giữa các hệ điều hành từ khoảng tick 180, nên so khớp chính xác chỉ làm được trong cùng họ hệ điều hành (ADR-0010) |

*Gợi ý để xét khi làm "Chỉ tiêu theo đảo và quần đảo" (ghi chú 2026-11-22, từ
[đánh giá đài quan sát tiến hoá](../../research/EVOLUTION_OBSERVATORY_RESEARCH_2026-11-22.md) §2, chưa
quyết):*
- *Tiêu chí xâm lấn nguồn–bẫy (Pulliam 1988):* một dòng chỉ tính là đã định cư trên một đảo khi số
  sinh tại chỗ vượt số chết tại chỗ trong một cửa sổ khai trước, không tính cá thể nhập cư; nếu không,
  di cư vãng lai vào đảo bẫy sẽ đếm như xâm lấn (liên quan H2).
- *Muller plot theo đảo, ngoại tuyến:* xuất chuỗi tần số theo dòng cho từng đảo để vẽ hình cho bài báo
  bằng script phân tích, không thêm panel (khoá phạm vi §9).

## 6. Ràng buộc cứng (không thương lượng)

- `WorldLawSet` không đổi trong một lần chạy. Mọi đảo của một quần đảo có cùng luật; chỉ điều kiện ban
  đầu và các yếu tố đã khai báo là khác nhau.
- Không Lamarck. Di cư mang theo não và genotype được giao, không tạo não mới (ADR-0003, D01).
- Mọi khoản năng lượng mới đi qua `total_cost`. Di cư là chuyển EU, không tạo ra hay huỷ đi EU.
- Không khẳng định checksum tuyệt đối, và không khẳng định một quỹ đạo đạt tới một trạng thái. Luôn so
  các lần chạy với nhau.
- Không gọi một cụm hình thái là "loài" khi chưa qua cổng AE-S11/AE-S14.
- Không chạy seed xác nhận trước khi chủ dự án duyệt bản đăng ký và smoke đạt cổng.

## 7. Lực thống kê và ngân sách chạy (ước lượng, chưa đo)

Số đo gần nhất: một lượt 200.000 tick ở 16 hồ mất khoảng 1,96 phút trên Dell (S14). Win 1 nhanh hơn.

- **Một quần đảo 4 đảo + đất liền:** khoảng 10 phút trên Dell.
- **Giai thừa 8 ô × 20 seed:** 160 quần đảo, khoảng 27 giờ trên riêng Dell. Chia cho ba máy bằng
  OxideSwarm thì khoảng 8–10 giờ, chạy được trong một đêm.

Số seed cuối cùng tính từ khảo sát thăm dò (§8, M2), như SEL2-P đã làm. Không chọn số seed theo kết quả.

## 8. Lộ trình

| Mốc | Nội dung | Xong khi |
|---|---|---|
| **M0** (đang chạy) | SEL2-P: smoke, đóng băng, 20 seed xác nhận | Preprint đầu tiên (kết quả dương hay âm đều công bố) kèm code và dữ liệu |
| **M1** | Bộ chạy quần đảo, sổ EU khi di cư, khoá kích thước tổ tiên, chỉ tiêu theo đảo | Test: bảo toàn EU toàn quần đảo; di cư mang đúng não; cùng seed cho cùng kết quả trong ba tiến trình; quần đảo một đảo, không di cư, trùng thế giới đơn |
| **M2** | Khảo sát thăm dò (chỉ đo), như S13–S15 | Tìm được vùng tham số nơi quần thể sống đủ thế hệ ở cả bốn ô sức chứa × săn mồi |
| **M3** | Bản đăng ký trước cho H1, H1-R, H1-P (và H2 nếu đủ lực) | Chủ dự án duyệt; có kiểm độc lập |
| **M4** | Chạy xác nhận trên ba máy qua OxideSwarm | Kiểm khớp từng artifact; kiểm độc lập |
| **M5** | Bài báo thứ hai, bản phát hành mở (README tiếng Anh, tài liệu manifest) | Người ngoài chạy lại một ô và ra cùng con số trên cùng họ hệ điều hành |
| **M6** (sau) | Người tình nguyện góp máy chạy seed (`Contribute`, thiết kế 25/08 §3.1) | Cổng `contribute_matches_solo`; phần kiểm khớp chính xác dùng `ReferenceEvolutionWorld` |

Ước lượng thô: M1 khoảng 1–2 tuần cho một agent, M2 khoảng 1 tuần, M3–M4 khoảng 1–2 tuần. Kết quả quần
đảo đầu tiên có thể có trong khoảng 6–8 tuần nếu khoá phạm vi (§9).

## 9. Khoá phạm vi đề xuất (khoảng 3 tháng)

- **Tiếp tục:**
  - SEL2-P;
  - E3-P và M2 (cổng đối chứng âm);
  - LAND1;
  - bản dùng thử (V1, B1, W2-lite, W3-lite) tới mức chạy được và đẹp vừa đủ.
- **Tạm dừng:**
  - renderer native (QĐ-NATIVE đã để sau);
  - mở rộng năng lượng ngoại lai;
  - tải bản đồ theo vùng (W4);
  - artifact bản đồ chia ô đầy đủ (W1–W3 bản đầy đủ);
  - liên kết nhiều máy kiểu `Inhabit`.
- **Đồ hoạ:** sau bản dùng thử, chỉ sửa lỗi hiển thị, không thêm tính năng mới.

## 10. Công bố và mở

1. **Preprint SEL2-P (M0)** trên arXiv (q-bio.PE) hoặc bioRxiv, kèm manifest, artifact và verifier.
2. **Bài về quần đảo (M5),** nhắm hội nghị ALIFE hoặc tạp chí *Artificial Life*.
3. **Bài công cụ** (ví dụ JOSS), khi người ngoài chạy lại được.
4. **Bài phương pháp về đội agent AI** trên phần mềm khoa học. Dữ liệu đã có: các verdict trong hub và
   lỗi mà kiểm độc lập bắt được, ví dụ M4 ma trận rỗng, F047 khẳng định phụ thuộc hệ điều hành, verifier
   SEL2-P chấp nhận dấu H1 bị đảo, mắt B1 lệch thân.
5. **Ngôn ngữ:** tài liệu nội bộ giữ tiếng Việt; README, bài báo và tài liệu manifest công khai bằng
   tiếng Anh.
6. **Người ngoài:** mời một nhà nghiên cứu ALife hoặc sinh học tiến hoá phản biện bản đăng ký M3 trước
   khi chạy xác nhận.

## 11. Đã quyết (chủ dự án, trong chat, 2026-11-19)

| # | Câu hỏi | Trả lời |
|---|---|---|
| Q1 | Nhận "quy luật đảo" làm câu hỏi chủ lực? | **Nhận** |
| Q2 | Khoá phạm vi như §9 trong khoảng 3 tháng? | **Đồng ý**; rà lại sau kết quả SEL2-P |
| Q3 | "Kích thước đảo" đo bằng gì? | **Sức chứa** (`live.prey_cap`, `live.resource_capacity_scale`); diện tích để sau, cần ADR riêng |
| Q4 | README, bài báo và tài liệu công khai bằng tiếng Anh? Mời người phản biện bên ngoài trước M3? | **Cả hai**: tiếng Anh cho phần công khai; chủ dự án chọn và liên hệ người phản biện, lead soạn thư và gói tài liệu |

## 12. Không khẳng định

- Không khẳng định kết quả trong Anima nói lên điều gì trực tiếp về loài thật. Nó chỉ nói một cơ chế có
  **đủ** để sinh ra một hướng thay đổi trong một thế giới mô hình có luật khai báo.
- Không khẳng định "kích thước đảo" bằng sức chứa tương đương diện tích thật.
- Không khẳng định tiến hoá mở, trí tuệ hay hình thành loài.
- Số giờ chạy và số tuần ở §7–§8 là ước lượng, chưa đo.

## Tài liệu tham khảo

- Foster, J. B. (1964). Evolution of mammals on islands. *Nature* 202, 234–235.
- MacArthur, R. H. & Wilson, E. O. (1967). *The Theory of Island Biogeography*. Princeton University Press.
- Lomolino, M. V. (2005). Body size evolution in insular vertebrates: generality of the island rule.
  *Journal of Biogeography* 32, 1683–1699.
- Meiri, S., Cooper, N. & Purvis, A. (2008). The island rule: made to be broken? *Proc. R. Soc. B* 275, 141–148.
- Benítez-López, A. et al. (2021). The island rule explains consistent patterns of body size evolution in
  terrestrial vertebrates. *Nature Ecology & Evolution* 5, 768–786.
- Van Valen, L. (1973). A new evolutionary law. *Evolutionary Theory* 1, 1–30.
- Packard, N. et al. (2019). An overview of open-ended evolution. *Artificial Life* 25(2), 93–103.
- Ray, T. S. (1991). An approach to the synthesis of life. *Artificial Life II*, 371–408.

Các trích dẫn được ghi từ trí nhớ của phiên lead; cần đối chiếu số trang và năm trước khi đưa vào bài báo.
