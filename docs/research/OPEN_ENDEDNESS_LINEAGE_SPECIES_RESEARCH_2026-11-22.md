---
title: Phả hệ, tiến hoá mở, Red Queen và phân định loài — đánh giá báo cáo ngoài
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-22
review_cycle: quarterly
---

# Phả hệ, tiến hoá mở, Red Queen và phân định loài — đánh giá báo cáo ngoài

> **Phạm vi & tính chất tài liệu.** Đây là bản **đánh giá và chắt lọc** một báo cáo nghiên cứu ngoài
> (*"Định Lượng Tiến Hoá ALife"*, soạn bằng công cụ nghiên cứu tự động, nhận ngày 2026-11-22; dưới đây
> gọi là **báo cáo gốc**). Báo cáo gốc đề xuất một giao thức phả hệ cho OSS-073, bộ thước đo tiến hoá
> mở (MODES, Bedau–Packard, quy trình Tokyo Type 1 của Channon 2024), khung đánh giá Red Queen cho S43,
> một quy trình phân định loài tự động cho AE-S11/AE-S14, và một bộ quy chuẩn thống kê. Tài liệu này
> **không** phải hợp đồng, quyết định hay kế hoạch đã duyệt. Số đo của repo chỉ nằm ở
> [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
> Mọi thay đổi chạm luật thế giới, phả hệ hay chẩn đoán loài phải đọc trước phần "Required reading"
> trong `CLAUDE.md`.

## 0. Nhãn bằng chứng và kết luận ngắn

| Nhãn | Nghĩa |
|---|---|
| **Đã kiểm** | Đối chiếu trực tiếp với mã nguồn và tài liệu của repo ngày 2026-11-22. |
| **Nguồn ngoài** | Công trình có thật mà báo cáo gốc trích; nội dung tóm theo báo cáo gốc, chưa đọc lại từng bài trừ khi ghi rõ. |
| **Chưa xác minh** | Con số dẫn về nguồn không truy được (mục trích dẫn số 15 của báo cáo gốc là `unknown_url`) hoặc là tính toán riêng của báo cáo gốc. |
| **Suy luận** | Đánh giá của tài liệu này. |

**Kết luận (`Suy luận`):**

1. **Đáng lấy một phần.** Phần văn liệu phần lớn có thật và hợp với dự án, nhất là cảnh báo rằng nén
   đường đơn làm mất các bước trung gian trên line of descent (§1) và các phép thử Red Queen theo
   thời gian (§4).
2. **Báo cáo gốc hiểu sai nghĩa hai mã nội bộ.** AE-S14 không phải phân hệ cách ly sinh sản, và S43
   là một cổng chứ không phải một mô-đun (§6).
3. **Khoảng trống lớn nhất không nằm trong báo cáo gốc:** ca sinh liên tục của thế giới sống không
   được ghi vào lineage tracker (§1.2). Mọi line of descent, S43 hay phân định loài trên thế giới sống
   đều cần việc này trước.
4. **Phần thống kê (§5 của báo cáo gốc) đã bị thay thế** bởi
   [`VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md`](VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md) (§7).
5. **Mọi con số "đo" nổi bật đều `Chưa xác minh`** (§8) và không được dùng làm ngân sách hay ngưỡng cổng.

## 1. Line of descent cho OSS-073

OSS-073 trong repo là "giao thức đo line of descent kiểu Avida", với bằng chứng hoàn tất "bám được
dòng dõi của genotype thống trị cuối run"
([`OPEN_SOURCE_ADOPTION_PLAN.md`](../planning/OPEN_SOURCE_ADOPTION_PLAN.md), bảng OS7;
[`OPEN_SOURCE_LANDSCAPE.md`](OPEN_SOURCE_LANDSCAPE.md) dòng 93). Báo cáo gốc gọi nó là "giao thức
phả hệ kép"; đó là đề xuất của báo cáo, không phải định nghĩa của repo.

### 1.1 Nén đường đơn làm mất genotype trung gian (`Đã kiểm`)

- `LineageTracker::compact` gọi `simplify_with` với `compress_unary_paths: true`
  ([`lineage.rs`](../../src-tauri/src/evolution/lineage.rs) khoảng dòng 396–430;
  [`simplify.rs`](../../src-tauri/src/evolution/simplify.rs) dòng 157–175). Lời gọi định kỳ nằm ở
  [`simulation_loop.rs`](../../src-tauri/src/core/simulation_loop.rs) dòng 994–1001.
- Số **đột biến** vẫn đúng sau khi nén: `SimplifiedEdge` mang `events` và `mutations`, và
  `LineageNode.cumulative_mutations` được điền trước khi cắt (`lineage.rs` dòng 11–28).
- Nhưng các node trung gian bị xoá cùng genotype của chúng. Vì vậy không dựng lại được **từng bước**
  trên line of descent: không phân loại được bước có lợi / trung tính / có hại so với cha mẹ trực tiếp,
  không làm được phép đảo ngược đột biến kiểu Lenski et al. 2003.
- `Suy luận`: định nghĩa hoàn thành của OSS-073 cần nói rõ chọn cách nào. (a) Ghi nhật ký sai phân
  đột biến trên cạnh đã nén, đủ để phát lại genotype trung gian. (b) Ghim tổ tiên của genotype thống
  trị trước khi `compact` chạy. Báo cáo gốc gợi ý (a), mượn từ `keep_unary_in_individuals` của tskit
  (`Nguồn ngoài`).

### 1.2 Ca sinh của thế giới sống không vào lineage tracker (`Đã kiểm`)

- Lời gọi `add_reproduction` duy nhất ngoài chính tracker là ở `simulation_loop.rs` dòng 956, trong
  vòng lặp MAP-Elites theo epoch.
- Sinh sản liên tục (`autonomous_reproduction_system` → `execute_reproduction_birth` →
  `SpawnOffspringCommand`,
  [`agent_systems.rs`](../../src-tauri/src/core/agent_systems.rs) dòng 464–476 và 1129–1150) có
  `lineage_id`, `generation` và `parent_ids` nhưng không ghi vào tracker. Nó chỉ ghi vào bộ đếm nhân
  khẩu và `VitalStatistics`.
- Theo đọc mã ở `agent_systems.rs` dòng 1129, `lineage_id` của con là `"{cha}.{thế hệ cha + 1}"`, nên
  các con cùng một cha nhận **cùng** id. Trước khi ghi ca sinh sống vào tracker phải có id duy nhất cho
  từng cá thể. `add_reproduction` đã từ chối cạnh có cha không tồn tại; chưa kiểm nó xử lý id trùng
  ra sao.
- `Suy luận`: đây là việc chặn trước cho OSS-073 trên thế giới sống, cho S43 và cho AE-S11. Nó cũng
  đụng ngân sách bộ nhớ và zero-alloc của vòng tick, nên cần thiết kế riêng (ví dụ ghi sự kiện ra một
  buffer cấp sẵn rồi gom ngoài tick).

### 1.3 Cửa sổ cắt tỉa trễ và đoàn hệ đông lạnh

- `Đã kiểm`: tập `samples` của `compact` đã gồm mọi elite trong archive MAP-Elites
  (`simulation_loop.rs` dòng 994–1000). Đó là một dạng "ghim" sẵn có.
- `Nguồn ngoài` / `Suy luận`: MODES cần giữ nhánh đã tuyệt chủng trong một cửa sổ thời gian, còn S43
  cần kho đối thủ đông lạnh theo chu kỳ. Hiện chưa có cả hai.

## 2. Xuất phả hệ theo chuẩn dữ liệu ALife

- `Nguồn ngoài`: chuẩn của Lalejini, Dolson et al. (2019) có bốn trường `id`, `ancestor_list`,
  `origin_time`, `destruction_time`.
- `Đã kiểm`: repo đã xuất Newick (`evolution/newick.rs`, lệnh `export_phylogenetic_tree`) và
  [`RESEARCH_SYNTHESIS_2026-11-13.md` §4](RESEARCH_SYNTHESIS_2026-11-13.md) đã đòi xuất sự kiện sinh và
  chết. `LineageNode` không có thời điểm chết (`lineage.rs` dòng 7–29), nên `destruction_time` chưa có
  nguồn.
- `Suy luận`: xuất chuẩn này là một bước nhỏ sau khi §1.2 xong. Nó cho phép dùng công cụ phân tích
  phả hệ bên ngoài mà không nhập code.

## 3. Bedau–Packard, MODES và bóng trung tính SEL2-D

- `Nguồn ngoài`: quy trình năm bước của Channon (2024) chuẩn hoá hoạt động tiến hoá theo thành phần
  bằng một mô hình bóng trung tính chạy song song. MODES (Dolson et al. 2019) lọc nhiễu bằng knockout
  vị trí có nghĩa và bộ lọc bền vững trên phả hệ. Miconi (2008) cảnh báo rằng thống kê hoạt động một
  mình không chứng minh được chọn lọc Darwin.
- `Đã kiểm`: bóng trung tính **đã có**. [`drift_control.rs`](../../src-tauri/src/core/drift_control.rs)
  (thẻ SEL2-D, dòng 1–15) phát lại cùng số ca sinh và chết mỗi tick, nhưng chọn nạn nhân và cha mẹ ngẫu
  nhiên đều, có tuỳ chọn đặt lại theo checkpoint kiểu Channon. Bộ chạy ghép cặp `selected` / `drift`
  nằm ở [`live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs) dòng 3460–3480.
- Còn thiếu: thống kê hoạt động theo thành phần, và một định nghĩa "thành phần" (genotype, allele hay
  module não) phải chốt trước khi đo.
- `Suy luận`: [kế hoạch fork luật](../ai/planning/2026-11-18-research-law-fork-and-multi-machine.md)
  đã nêu hoạt động tiến hoá QNN (Droop–Hickinbotham) và MODES đo trên nhiều độ sâu lọc; nên đối chiếu
  hai hướng đó trước khi chọn.

## 4. Các phép thử cho S43 (Red Queen thú săn – con mồi)

- `Đã kiểm`: S43 chưa có. Sinh thái thú săn – con mồi sống được (P1) đã đạt nhưng "không phải S43 hay
  Red Queen" ([`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) dòng 643 và 801).
- `Nguồn ngoài`, theo thứ tự ưu tiên `Suy luận`:
  1. **Time-shift assay** (Gandon et al. 2008): hiệu năng trung bình theo độ lệch thời gian giữa thú
     săn và con mồi. Chạy đua vũ trang cho đường tăng đơn điệu; chọn lọc dao động cho đường tuần hoàn.
     Phải khảo sát độ lệch dài, vì quanh 0 hai cơ chế trông giống nhau.
  2. **CIAO / Master Tournament** (Cliff & Miller 1995; Floreano & Nolfi): cá thể hiện tại đấu với đối
     thủ tổ tiên đông lạnh. Cần kho đối thủ đông lạnh (§1.3) và một sân đấu có điều kiện biên cố định.
  3. Dominance tournament, phân rã HodgeRank, đồ thị đấu thưa: hoãn đến khi có dữ liệu từ (1) và (2).
- Ràng buộc của repo: S43 là tín hiệu trên **ensemble**, không kết luận từ một run
  ([`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md) dòng 981).

## 5. Phân định loài hai giai đoạn cho AE-S11

- Ràng buộc của repo: không gọi một hình thái trực quan hay một ô MAP-Elites là loài. Báo cáo gốc
  đồng ý và nêu ba lý do: ranh giới ô áp đặt từ ngoài, chuỗi chủ ô đa phát sinh, không có cấu trúc hợp
  dòng trong ô (`Nguồn ngoài`). Kết quả MAP-Elites nên báo bằng coverage và QD-score.
- `Suy luận`, phần đáng lấy:
  - **Giai đoạn 1, giả thuyết loài sơ cấp:** phân cụm khoảng cách genotype không cần ngưỡng cứng
    (HDBSCAN, ASAP), hoặc phân hoạch trên cây chính xác (quy tắc 4θ của Birky, GMYC, mPTP). Sukumaran &
    Knowles (2017) nhắc rằng mô hình hợp dòng phân định *cấu trúc*, không phải *loài*.
  - **Giai đoạn 2, kiểm chứng nhân quả:** phép thử xâm lấn tương hỗ (Geritz et al.), chạy trên một
    checkpoint fork (AE-S09) với đột biến tắt. Hai nhánh là đa hình được bảo vệ khi cả hai chiều xâm
    lấn đều dương. Repo chưa có khoá manifest để tắt đột biến; cần kiểm trước khi hứa.
  - Phép thử này **không** thay CM-S11: thích nghi địa phương vẫn dùng reciprocal transplant.
  - Chính sách loài và ngưỡng phải được version, theo cổng chấp nhận của
    [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md).
- **Không áp dụng lúc này:** chỉ số cách ly tình dục, độ mô-đun mạng giao phối không gian, chỉ số cách
  ly tổng hợp tiền/hậu hợp tử. Sinh sản sống là sinh thái học, không giao phối, không giới tính, không
  crossover ([ADR-0005](../decisions/ADR-0005-live-population-turnover.md) dòng 116).

## 6. Nghĩa đúng của các mã nội bộ (`Đã kiểm`)

| Mã | Báo cáo gốc hiểu là | Nghĩa trong repo | Neo |
|---|---|---|---|
| OSS-073 | Giao thức phả hệ kép | Line of descent kiểu Avida | `OPEN_SOURCE_LANDSCAPE.md:93`, `OPEN_SOURCE_ADOPTION_PLAN.md` bảng OS7 |
| S43 | Một mô-đun Red Queen | Cổng: Red Queen thú săn – con mồi, tín hiệu trên ensemble | `WORLD_SIMULATION_PLAN.md:854`, `:981` |
| AE-S11 | Phân định loài cho tiến hoá vô tính | Bộ dò loài không gắn nhãn chỉ vì hình thái/màu khác | `EVOLUTION_EXPERIMENT_CONTRACT.md:302` |
| AE-S14 | Cách ly sinh sản hữu tính | Ensemble nhiều seed báo effect size, khoảng và cả run thất bại | `EVOLUTION_EXPERIMENT_CONTRACT.md:305` |
| CM-S11 | (không nêu) | Reciprocal transplant chứng minh thích nghi địa phương; không thay S43 | `CREATURE_DEVELOPMENT_CONTRACT.md:253-256` |

Phần của báo cáo gốc thực sự khớp với AE-S14 là mục thống kê (§7), không phải mục cách ly sinh sản.

## 7. Thống kê: đã bị thay thế

Mục 5 của báo cáo gốc khuyên kiểm tra tính chuẩn (Shapiro–Wilk, Levene) trước khi chọn kiểm định, và
báo Cliff's δ hoặc Vargha–Delaney A. Với thiết kế ghép cặp theo seed của repo, khuyến nghị đó bị
thay bởi [`VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md`](VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md)
§0 và §3.3 (tương quan hạng–nhị phân ghép cặp, giả trung vị Hodges–Lehmann, khoảng BCa). E3 dùng
`paired_mean_delta` và `d_z`; mã ở
[`statistics.rs`](../../src-tauri/src/core/experiment_runner/statistics.rs) (`paired_effect` dòng 73,
`EffectSize` dòng 156). Ghép cặp cùng seed (CRN) đã có. Phần còn dùng được: ba đối chứng âm, đặc biệt
"bóng trung tính" (đã có, §3).

## 8. Độ tin cậy của số liệu và trích dẫn

**Đã kiểm với nguồn gốc (2026-11-22):**

- Lenski, Ofria, Pennock & Adami (2003), *Nature* 423: EQU xuất hiện ở bước 111 (update 27.450); 103
  đột biến đơn, 6 kép, 2 ba; 45 bước có lợi, 48 trung tính, 18 có hại; sau EQU thêm 233 bước (62 / 132
  / 39). Đảo ngược đột biến bước 110 làm mất EQU. Khớp với báo cáo gốc.
- Dolson, Rodriguez-Papa & Moreno (2024), Phylotrack, arXiv 2405.09389: 296 MiB đỉnh cho 100.000 cá
  thể qua khoảng 40 thế hệ; 70,6 MiB và 71,0 MiB ở quần thể 10 và 1.000. Khớp, nhưng đó là phép đo
  60 giây với phylotrackpy.
- Channon (2024), *A Procedure for Testing for Tokyo Type 1 Open-Ended Evolution*, *Artificial Life*
  30(3): có thật, quy trình năm phương pháp dựa trên thống kê hoạt động tiến hoá.

**Chưa xác minh (mọi số dẫn về mục 15 = `unknown_url`, là tính toán riêng của báo cáo gốc):**

- Nén lưu trữ từ 3.814,7 GiB xuống 132,9 MiB (hệ số 29.401). Số học tự nhất quán, nhưng việc báo cáo
  gốc gọi nó là "tương thích" với 296 MiB của Phylotrack là so sai: hai bên có quy mô khác nhau hàng
  nghìn lần.
- Giảm số trận hậu kiểm 1.250 lần ở mốc 50.000 thế hệ; tỷ phần gradient 99,5% và curl 100,0% trên ma
  trận mẫu; tỷ phần curl 94,5% dưới nhiễu trắng.
- Thời gian hợp dòng Monte Carlo 400,1 thế hệ.
- Độ chệch +0,1667 của chỉ số thô so với 0,0000 của chỉ số cách ly tình dục.

**Trích dẫn rác:** mục 10, 11 và 12 của báo cáo gốc (một văn bản của Isidore of Seville, một hướng dẫn
chơi Final Fantasy IV, một sổ đăng ký bảo tàng) được dùng làm nguồn cho câu "các mã OSS-073, S43,
AE-S11, AE-S14 là nội bộ". Câu đó đúng, nhưng nguồn thì không liên quan.

**Nguồn ngoài chưa đọc lại** (có thật theo hiểu biết chung, nội dung tóm theo báo cáo gốc): MODES
(Dolson et al. 2019), Gandon et al. 2008, Czarnecki et al. 2020, HodgeRank (Jiang et al. 2011), Birky
et al. 2010, Sukumaran & Knowles 2017, Rolán-Álvarez & Caballero 2000, Fletcher et al. 2013.
