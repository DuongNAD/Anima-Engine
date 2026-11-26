---
title: Nghiên cứu tiến hoá ngược, hồi sinh tổ tiên và dự báo ngược (hindcast) trong Anima
status: proposed
owner: architecture
last_reviewed: 2026-11-26
review_cycle: quarterly
---

# Nghiên cứu tiến hoá ngược, hồi sinh tổ tiên và dự báo ngược (hindcast) trong Anima

> **Phạm vi & tính chất tài liệu.** Đây là **tổng hợp bằng chứng bên ngoài, đối chiếu mã nguồn và đề xuất thí nghiệm** cho hai câu hỏi chủ dự án đặt ra đêm 2026-11-25. Tài liệu **không** phải hợp đồng, preregistration hay kế hoạch đã duyệt, và không thay đổi dòng code nào. Trạng thái đo được của dự án nằm duy nhất ở [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền). Mọi `file:line` dưới đây đọc từ trunk `feat/ecological-selection` tại `38ea4621` (2026-11-26); đường dẫn mã tính từ `src-tauri/`. Ghi chú gần nhất về phả hệ là [`OPEN_ENDEDNESS_LINEAGE_SPECIES_RESEARCH_2026-11-22.md`](OPEN_ENDEDNESS_LINEAGE_SPECIES_RESEARCH_2026-11-22.md); quy tắc quá khứ là [ADR-0008](../decisions/ADR-0008-world-past-and-time.md).

---

## 0. Câu hỏi, quy ước bằng chứng và TL;DR

### Câu hỏi của chủ dự án

1. Có thể bắt đầu từ một mốc thời gian đã biết, chạy xuôi cho tới khi khớp các mốc về sau, rồi **chạy ngược** để dựng lại quá khứ (hindcast) không?
2. "Sao chúng ta không nghiên cứu cách **ngược tiến hoá**, tức là tìm lại những gì đã đánh mất?"

### Quy ước phân loại bằng chứng

| Nhãn | Nghĩa trong tài liệu này |
|---|---|
| **Đã kiểm** | Đọc trực tiếp từ mã nguồn, test và ADR của Anima tại `38ea4621` ngày `2026-11-26`, có `file:line`. |
| **Nguồn ngoài — bài báo** | Công bố khoa học có DOI; metadata đối chiếu với Crossref/PubMed và số liệu lấy từ phần tóm tắt ngày `2026-11-26`. Số nào chỉ đọc qua bản tóm lược toàn văn thì ghi "chưa xác minh". |
| **Nguồn ngoài — tổ chức / tin tức** | Văn bản của IUCN và bài tin của tạp chí khoa học, dùng cho phần hồi sinh loài đã tuyệt chủng. |
| **Suy luận** | Tính toán và thiết kế rút ra cho Anima từ hai loại trên; phải đo hoặc chạy thử trước khi thành ADR hay preregistration. |

### TL;DR — sáu kết luận

1. **Không mô phỏng nào "chạy ngược vật lý" được, Anima cũng vậy (`Đã kiểm` + `Nguồn ngoài`).** Thế giới xoá thông tin ở mỗi tick: xác chết đổ năng lượng vào một con số chung `detritus`, phép đột biến cắt giá trị về biên, phả hệ nén bỏ genotype của tổ tiên. Quá khứ chỉ dựng lại được bằng cách **chạy xuôi lại** từ một trạng thái đã lưu. ADR-0008 đã chốt điều này: "quay lại quá khứ" chỉ có một nghĩa là **fork**.
2. **"Tìm lại cái đã mất" có thật trong tự nhiên nhưng hiếm và có điều kiện (`Nguồn ngoài — bài báo`).** Răng hàm dưới của ếch *Gastrotheca guentheri* quay lại sau khi mất ít nhất 230 triệu năm (Wiens 2011). Gen câm thì thường mục nát sau vài triệu năm (Marshall và cộng sự 1994). Còn "bánh cóc epistasis" chặn đường quay về ngay cả ở mức một protein (Bridgham và cộng sự 2009).
3. **Lợi thế riêng của Anima là biết trọn sự thật lịch sử.** Sinh học không có điều đó. Có bốn thí nghiệm chỉ Anima làm được: (a) Dollo in silico, (b) hồi sinh tổ tiên và thi đấu xuyên thời gian, (c) chấm điểm các phương pháp dựng lại tổ tiên bằng sự thật đã ghi, (d) kiểm định hindcast từ một mốc đã ghi.
4. **Hạ tầng đã có một nửa (`Đã kiểm`).** Có checkpoint fork bit-identical (`checkpoint_fork`), cây rẽ nhánh N-ary (`run_branch_tree`), phả hệ live opt-in không đổi quỹ đạo (`live.lineage_tracking`), xuất Newick và MRCA, cùng bản lưu mang cả brain (`SerializedAgent.brain`).
5. **Có bốn chỗ hổng chặn trực tiếp các thí nghiệm (`Đã kiểm`).**
   - `LiveExperimentAdapter::step` bỏ qua RNG của runner, nên chiến lược seed của cây rẽ nhánh không làm thế giới live khác đi.
   - Nén phả hệ xoá genotype tổ tiên, và nút phả hệ không giữ brain.
   - `GeneticInjection` là no-op (`F-071`).
   - Chưa có đường nào đưa một tổ tiên vào thế giới khác mà giữ được EU kín và giữ nguyên brain (D01).
6. **Khuyến nghị làm (c) trước, rồi tới (a) (`Suy luận`).** (c) chạy hoàn toàn ngoại tuyến trên một khoá opt-in đã có test "ghi không đổi quỹ đạo", không chạm lịch tick. Nó cũng dựng sẵn công cụ xuất tổ tiên mà (a) và (b) cần. Không thí nghiệm nào chạm manifest, khoá hay observable của `SEL2-P2`.

---

## 1. Khái niệm: vì sao không chạy ngược được, và làm gì thay thế

### 1.1 Bốn lý do quá khứ không suy ngược được từ hiện tại

| Lý do | Nói đơn giản | Ví dụ ngay trong Anima (`Đã kiểm`) |
|---|---|---|
| **Mất thông tin** | Nhiều trạng thái khác nhau cùng dẫn tới một trạng thái sau. Từ cái sau không biết được cái trước. | `ReclaimAndDespawnAgentCommand` cộng năng lượng còn lại của xác vào `pool.detritus`, một số thực chung (`core/agent_systems.rs:140-170`). Sau đó không còn biết EU ấy của ai. `mutate_genotype_reporting` dùng `.clamp(0.1, 5.0)` cho chiều dài đốt (`evolution/mutation.rs:112-117`), nên mọi giá trị vượt biên đều thành `5.0`. |
| **Tuyệt chủng** | Một nhánh chết hết thì không để lại hậu duệ nào để suy ra nó. | Nén phả hệ cắt mọi nút không có hậu duệ còn sống, và với nút bị nối tắt thì "genotype is gone" (`evolution/lineage.rs:293`; `core/live_lineage.rs:467-481`). |
| **Không định danh được** (non-identifiability) | Nhiều lịch sử khác nhau giải thích tốt như nhau cùng một hiện tại. | Một dòng dõi đột biến đi rồi đột biến về đúng giá trị cũ thì nhìn từ lá giống hệt một dòng dõi không đột biến. Chỉ phả hệ đã ghi mới phân biệt được, và cạnh `Clone` hay `Mutate` chỉ nói kết quả có khác cha mẹ hay không (`core/agent_systems.rs:1223-1225`). |
| **Hỗn loạn** | Sai khác cực nhỏ ở điều kiện đầu lớn dần theo hàm mũ (Lorenz 1963). | Bỏ vị trí luồng RNG khi khôi phục là quỹ đạo tách ra (`tests/snapshot_checkpoint_tests.rs:326`, `dropping_the_rng_stream_position_does_diverge`). Checksum còn phụ thuộc nền tảng: ADR-0008 ghi phát lại chỉ đúng từng bit "trên cùng build và cùng lớp nền tảng" ([ADR-0008](../decisions/ADR-0008-world-past-and-time.md), dòng 60). |

Một điểm tinh tế (`Suy luận`): bản thân RNG của Anima **tua lại được**. `SimRng` là ChaCha12, một bộ sinh theo bộ đếm có thể đặt lại vị trí từ (`core/resources.rs:361-404`; "ChaCha seeks, it does not replay", `:383-389`). Thứ không đảo được là **động lực học**: phép kẹp, phép lấy max, cái chết, việc gộp vào một bể chung. Vì vậy chạy ngược thì không được, còn **chạy xuôi lại từ một điểm đã lưu** thì được, và chính xác từng bit trên cùng nền tảng.

### 1.2 Ba việc làm được thay cho "chạy ngược"

1. **Phát lại (replay).** Nạp keyframe gần nhất trước tick `t`, chạy xuôi cùng sổ đầu vào rồi so checksum. Đây là đường ADR-0008 đã chọn (dòng 174). Nó chỉ dựng lại quá khứ của một thế giới **đã được ghi**.
2. **Suy luận xác suất về quá khứ.** Khi chỉ có hiện tại, ta không tính ra quá khứ mà ước lượng một **phân phối** các quá khứ khả dĩ:
   - dựng lại trạng thái tổ tiên trên cây phát sinh (Pagel 1999);
   - tính toán Bayes xấp xỉ, ABC: chạy giả lập xuôi nhiều lần, giữ lại tham số nào cho thống kê tóm tắt gần dữ liệu thật (Pritchard và cộng sự 1999; Beaumont, Zhang & Balding 2002; tổng quan Csilléry và cộng sự 2010, Beaumont 2010);
   - đồng hoá số liệu (data assimilation) trong khí tượng. Mô hình chạy xuôi, được nắn liên tục theo quan trắc; tái phân tích ERA5 là sản phẩm của cách làm này (Kalnay 2003; Hersbach và cộng sự 2020).
3. **Hindcast.** Bắt đầu từ một mốc đã biết, chạy xuôi nhiều bản với ngẫu nhiên khác nhau, rồi đo **mốc về sau nào lặp lại được** và mốc nào chỉ là tình cờ. Đây đúng là thí nghiệm "tua lại cuộn băng sự sống" mà Gould (1989) đề xuất và Blount, Lenski & Losos (2018) tổng kết.

---

## 2. Tiến hoá ngược trong tự nhiên (`Nguồn ngoài — bài báo`)

### 2.1 Định luật Dollo và vị trí hiện nay của nó

Dollo (1893) phát biểu rằng một cơ thể không quay lại được trạng thái đã có ở tổ tiên. Gould (1970) phân tích rằng định luật này đúng ở mức **thống kê** cho cấu trúc phức tạp, chứ không phải một cấm đoán tuyệt đối. Collin & Miglietta (2008) tổng hợp các trường hợp ngón và cánh được "tìm lại" theo phát sinh chủng loại. Họ cũng chỉ ra cơ chế khiến điều đó khả dĩ: **con đường phát triển vẫn được giữ** sau khi đặc điểm biến mất, thường vì nó còn được dùng vào việc khác.

### 2.2 Những lần "tìm lại" được ghi nhận, kèm phản biện

- **Cánh của bọ que.** Whiting, Bradler & Maxwell (2003) suy ra từ cây phát sinh rằng bọ que đa dạng hoá khi không có cánh, và cánh xuất hiện lại thứ phát, có thể nhiều lần. Kết luận này bị phản biện (Trueman và cộng sự 2004; tác giả trả lời: Whiting & Whiting 2004). Goldberg & Igić (2008) cho thấy bằng mô phỏng rằng các phép thử bác bỏ Dollo **thường sai**. Hai nguyên nhân là đặt sai tần suất trạng thái ở gốc cây và bỏ qua việc trạng thái ảnh hưởng tốc độ hình thành và tuyệt chủng loài. Maddison (2006) chỉ ra cùng kiểu nhiễu đó.
- **Răng hàm dưới của ếch.** Wiens (2011) dựng cây có định tuổi cho 170 loài lưỡng cư. Răng hàm dưới mất ở tổ tiên ếch hiện đại ít nhất 230 triệu năm trước và quay lại ở *Gastrotheca guentheri* trong khoảng 5–17 triệu năm gần đây. Paluh và cộng sự (2021) xác nhận bằng micro-CT và mô học rằng đó là răng thật.
- **Lại tổ (atavism), "răng gà mái".** Kollar & Fisher (1980) ghép biểu mô gà với trung mô răng chuột và thu được thân răng có chất nền men. Marshall, Raff & Raff (1994) cho rằng kết quả đó "nhiều khả năng là sản phẩm phụ của thí nghiệm". Harris và cộng sự (2006) sau đó mô tả mầm răng thế hệ đầu kiểu bò sát ở gà đột biến *talpid2*: khả năng làm răng vẫn tiềm ẩn trong bộ gen dù chim mất răng từ lâu.

### 2.3 Vì sao thường **không** tìm lại được

- **Gen mục nát khi chọn lọc lỏng (relaxed selection).** Lahti và cộng sự (2009) tổng hợp: khi một đặc điểm không còn được chọn, các gen của nó tích đột biến tự do. Marshall, Raff & Raff (1994) ước lượng xác suất đáng kể để kích hoạt lại gen câm trong khoảng 0,5–6 triệu năm. Quá 10 triệu năm thì "không thể", trừ khi gen ấy vẫn được giữ bởi một ràng buộc chọn lọc khác. Cá hang *Astyanax* là ví dụ kinh điển của tiến hoá thoái hoá (Jeffery 2009), và thấu kính giữ vai trò trung tâm trong việc mắt thoái hoá (Yamamoto & Jeffery 2000). Ghi chú: Protas và cộng sự (2006) nói về **bạch tạng** (gen *Oca2*), không phải mất mắt.
- **Bánh cóc epistasis.** Bridgham, Ortlund & Thornton (2009) hồi sinh thụ thể glucocorticoid tổ tiên. Năm đột biến "hạn chế" làm mất ổn định cấu hình cũ. Đảo riêng các đột biến đổi chức năng thì ra protein không hoạt động, mà đảo các đột biến hạn chế trước cũng không lấy lại chức năng tổ tiên. Con đường đi tới không còn là con đường đi về.
- **Đảo ngược ở mức kiểu hình nhưng không ở mức gen.** Teotónio & Rose (2000) đưa 25 quần thể ruồi giấm đã phân hoá trở lại môi trường tổ tiên trong 50 thế hệ: sự đảo ngược "không phổ quát". Teotónio và cộng sự (2009) thấy độ thích nghi hội tụ hoàn toàn về mức tổ tiên, trong khi tần số allele chỉ quay lại một phần. Tổng quan khái niệm: Teotónio & Rose (2001).

### 2.4 Hồi sinh tổ tiên và tua lại cuộn băng

- **"Hoá thạch đông lạnh" của LTEE.** Thí nghiệm tiến hoá dài hạn trên *E. coli* đông lạnh mẫu định kỳ, rồi hồi sinh tổ tiên để thi đấu với hậu duệ (Lenski và cộng sự 1991: độ thích nghi trung bình tăng khoảng 37% sau 2.000 thế hệ; Wiser, Ribeck & Lenski 2013: quỹ đạo theo luật luỹ thừa, chưa thấy trần). Blount, Borland & Lenski (2008) tìm thấy khả năng dùng citrate (Cit+) xuất hiện ở một quần thể sau hơn 30.000 thế hệ, muộn nhất ở thế hệ 31.500. Không có Cit+ nào trong 8,4×10¹² tế bào tổ tiên. Phát lại từ các mẫu đông lạnh cho thấy một đột biến "tiềm năng hoá" đã xuất hiện muộn nhất ở thế hệ 20.000. Số lần phát lại cụ thể (4/72, 5/340, 8/2.800) chỉ đọc qua bản tóm lược toàn văn: **chưa xác minh**.
- **Sinh vật số.** Lenski, Ofria, Pennock & Adami (2003) theo dõi trọn lịch sử trong Avida và thấy chức năng phức tạp được xây trên chức năng đơn giản hơn. Yedid, Ofria & Lenski (2008) cho chức năng `EQU` mất đi qua một đợt tuyệt chủng hàng loạt rồi tiến hoá lại. Khả năng tiến hoá lại phụ thuộc yếu tố ngẫu nhiên, lịch sử, bộ gen và sinh thái. Đây là tiền lệ trực tiếp gần nhất cho thí nghiệm (a).
- **Hồi sinh phân tử.** Thornton (2004) tổng quan phương pháp tổng hợp lại protein tổ tiên. Bridgham, Carroll & Thornton (2006) hồi sinh thụ thể **mineralocorticoid** tổ tiên (không phải glucocorticoid như thường bị nhớ nhầm).

### 2.5 Dựng lại trạng thái tổ tiên và các sai lệch đã biết

- Pagel (1999) đặt nền cho phương pháp hợp lý cực đại (ML) dựng lại trạng thái tổ tiên.
- Williams và cộng sự (2006) chỉ ra bằng mô phỏng rằng parsimony và ML **ước lượng quá cao độ bền nhiệt** của protein tổ tiên, còn cách lấy mẫu từ hậu nghiệm Bayes thì không mắc lỗi này.
- **Chuẩn có sự thật biết trước** là thứ sinh học rất hiếm. Hillis và cộng sự (1992) nuôi phage T7 với chất gây đột biến và tạo ra "phát sinh chủng loại đầu tiên biết trọn vẹn": cả năm phương pháp dựng đúng cấu trúc cây, và bản đồ giới hạn tổ tiên đúng trên 98%. Randall và cộng sự (2016) làm phả hệ protein huỳnh quang đỏ trong phòng thí nghiệm. Mọi thuật toán đều chính xác ở mức trình tự, nhưng vài vị trí suy sai đã đủ đổi kiểu hình. Anima có thể tạo chuẩn kiểu này với chi phí rất thấp (§4.3).

### 2.6 Hồi sinh loài đã tuyệt chủng (de-extinction): nói đúng mức

- IUCN SSC (2016) dùng từ **"proxy"** (vật thay thế) thay cho "hồi sinh". Văn bản thừa nhận hiện chưa thể tạo lại một phiên bản trung thành về di truyền của loài đã tuyệt chủng.
- Dê núi Pyrenees (bucardo) nhân bản năm 2003: một con sinh ra sống rồi chết sau vài phút do dị tật phổi (Folch và cộng sự 2009).
- Năm 2025, Colossal Biosciences công bố "sói dire". Bài tin của *Nature* (Callaway 2025) và bản đăng lại trên *Scientific American* cho biết đó là tế bào sói xám với 20 chỉnh sửa ở 14 vị trí gen. Nhóm chuyên gia họ Chó của IUCN SSC nói chúng không phải sói dire cũng không phải proxy. Câu trích "chỉ là sói xám có 20 chỉnh sửa" gán cho nhà khoa học của công ty chỉ thấy qua đoạn trích tìm kiếm: **chưa xác minh, không dùng**.

Bài học cho Anima (`Suy luận`): ngay cả với sinh vật thật, "tìm lại cái đã mất" thường là **dựng một bản gần đúng**, không phải hồi sinh. Mọi kết quả của Anima phải nói rõ là trong thế giới mô hình.

---

## 3. Anima đã có gì cho việc này (`Đã kiểm`, `38ea4621`)

| Năng lực | Ký hiệu và vị trí | Cho phép gì | Giới hạn |
|---|---|---|---|
| Fork từ checkpoint | `ExperimentModel` (`core/experiment_runner.rs:72`); `checkpoint_fork` (`core/experiment_runner/fork.rs:135`); `CheckpointForkReport` (`:86`); `genesis_fork` (`:50`) | Hai nhánh chung tiền tố, chung snapshot, **chung bản sao RNG**: nhánh đối chứng trùng từng bit với lần chạy không ngắt. | Không có thao tác "đổi seed tại điểm rẽ". |
| Fork của thế giới live | `LiveExperimentAdapter` (`core/live_experiment.rs:1935`); `LiveSnapshot` (`:1968`); `snapshot` / `from_snapshot` (`:2494`, `:2524`); test `a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards` (`tests/live_experiment_tests.rs:398`) và `run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run` (`:472`) | Chạy K tick, lưu xuống đĩa, nạp lại, chạy tiếp N−K tick: khớp với chạy liền N tick. | Chỉ headless; desktop chưa chạy dưới executor đa luồng. |
| Cây rẽ nhánh N-ary | `run_branch_tree` (`core/branch_tree.rs:653`); `BranchSeedStrategy::{InheritStream, DerivedStream, ExplicitSeed}` (`:42-50`); `compute_pairwise_divergence` (`:435`); `dynamic_time_warping` (`:413`) | Nhiều nhánh phản thực tế, có số đo phân kỳ giữa các nhánh. | `LiveExperimentAdapter::step` nhận `_rng: &mut StdRng` mà không dùng (`core/live_experiment.rs:2543-2548`). Thế giới live rút ngẫu nhiên từ `SimRng` riêng nằm trong snapshot, nên đổi seed ở cây **không** đổi thế giới live. `SimRng::reseed` có (`core/resources.rs:403`) nhưng runner không gọi. |
| Bản ghi thế giới | `WorldEvent::{Birth, Death, Kill, …}` (`core/world_record/types.rs:282-313`); `WorldKeyframe` (`:317-327`); `keyframe_snapshot_sha256` để dành cho H3 (`:337-339`) | Chuỗi hash, checksum từng đoạn, sự kiện sinh–tử. Phát lại tới tick `t` theo ADR-0008. | Sự kiện `Birth` không mang genotype; keyframe chỉ có số tổng. Theo ADR-0008, sổ sự kiện là đầu ra, "không bao giờ được đọc ngược vào mô phỏng" (dòng 158). |
| Phả hệ live | `LIVE_KEY_LINEAGE_TRACKING = "live.lineage_tracking"` (`core/live_experiment.rs:277`); `LiveLineage` (`core/live_lineage.rs:139`); `record_birth` (`:416`); `LineageNode { genotype, origin_tick, destruction_tick, … }` (`evolution/lineage.rs:7-47`) | Sinh vô tính, mỗi con một cha mẹ (cạnh `Mutate` hoặc `Clone`), nên phả hệ là **cây** thật. Test `recording_lineage_does_not_change_the_trajectory` (`tests/live_lineage_tests.rs:495`) chứng minh bật ghi không đổi quỹ đạo. | Nút chỉ giữ `MorphologyGenotype`, **không giữ brain**. Nén chạy từ `FIRST_COMPACTION_AT = 4_096` nút (`core/live_lineage.rs:86`) và xoá genotype các nút bị nối tắt. `set_compaction_floor` (`:353`) chỉ gọi được từ code, chưa có khoá manifest. |
| Xuất cây | `to_newick` (`evolution/newick.rs:164`); `mrca` (`evolution/mrca.rs:177`); `simplify` (`evolution/simplify.rs:188`) | Newick để `ape`, DendroPy và các công cụ khác đọc; độ dài cạnh tính bằng số thế hệ. | Newick là định dạng cây; một DAG có lai chéo phải bỏ bớt cạnh (có đếm). |
| Khôi phục cá thể | `SerializedAgent { genotype, brain, live_id, … }` (`core/simulation_state.rs:105-144`); `spawn_serialized_agent` (`:922`) chép brain y nguyên (`:967`, D01) và chép cả năng lượng (`:988`) | Snapshot giữ mọi cá thể sống cùng brain của nó. | Chép năng lượng mà **không trừ từ ngăn nào**. Thả một tổ tiên vào thế giới **khác** sẽ tạo EU từ hư không. |
| Ngủ đông và thức dậy | `HibernateAgentCommand` (`core/agent_systems.rs:222`); `WakeAgentCommand` (`:309`); `DormantAgents` (`:213`) | EU đi sang ngăn `dormant` rồi quay về, cùng một cá thể (D01): một "tủ đông" giữ EU kín. | Chỉ được gọi trong `tests/living_world_dormancy_tests.rs`; chưa hệ thống hay can thiệp nào dùng. |
| Tầng quần thể gộp | `RehydrateCommand` (`core/aggregate_population.rs:1164`) | Dựng lại cá thể từ cohort, có brain lưu trữ. | Vượt `ARCHIVE_CAP` thì brain có thể là của **cá thể khác** (`:226-232`). |
| Sinh từ genotype | `SpawnGenotypeCommand` (`core/agent_systems.rs:108`) | Lấy năng lượng từ `detritus`, giữ EU kín. | Gán **brain mới** (`:470-484`), không hồi sinh được brain tổ tiên. |
| Tiêm gen | `InterventionKind::GeneticInjection` (`crates/anima-domain/src/intervention.rs:123`) | Kiểm tra hợp lệ, vào hash manifest. | **No-op**: chỉ ghi ledger (`core/live_experiment.rs:1281-1296`; [`FINDINGS.md` F-071](../../FINDINGS.md)). |
| Bỏ thú săn | `InterventionKind::RemovePredators` (`core/live_experiment.rs:1251`); test `removing_predators_removes_predators_and_conserves_their_energy` (`tests/live_experiment_tests.rs:236`) | Cắt thú săn có giữ EU: kịch bản "đảo không thú săn". | Cắt một lần; muốn thú săn quay lại thì cần cơ chế khác. |
| Đối chứng trôi trung tính | `core/drift_control.rs` (SEL2-D), luồng `sim_stream::LIVE_DRIFT_SELECTION` | Phát lại cùng số sinh–tử, chọn nạn nhân và cha mẹ ngẫu nhiên. | Thiết kế cho SEL2; tái dùng thì phải giữ bit-identical. |

### 3.1 Một đặc điểm bị chọn lọc lỏng thì hiện ra sao trong code

- **Hình thái không có "gen câm".** Toán tử 4 xoá hẳn một đốt lá (`evolution/mutation.rs:210-228`). Toán tử 3 thêm đốt mới bằng cách chép một đốt bất kỳ rồi cộng nhiễu, tối đa 15 đốt (`:162-163`). Không còn vật liệu nào được giữ lại để mục nát, nên "tìm lại một đốt" chỉ tốn đúng một đột biến bất kể đã mất bao lâu. Đây là **đối chứng âm** tự nhiên cho thí nghiệm (a) (`Suy luận`).
- **Brain có đúng cơ chế "gen trôi khi không dùng".** Đầu vào thứ 12 (chỉ số 11) là `hit_is_predator` (`core/agent_systems.rs:1833-1848`). Với `EVOLVED_ARCH` gồm 15 đầu vào, 64 nơron ẩn và 8 đầu ra (`evolution/brain_genotype.rs:50-54`), 64 trọng số tầng một gắn với ô này nằm ở chỉ số `h·15 + 11`. Khi không còn thú săn, ô này luôn bằng 0. 64 trọng số ấy khi đó **hoàn toàn trung tính** và đi ngẫu nhiên dưới `mutate_brain(…, 0.05, 0.02, …)`. Tỉ lệ đột biến được viết cứng trong lệnh sinh con và chỉ chạy khi `mutation_rate > 0` (`core/agent_systems.rs:1180-1184`).
- **Ước lượng tốc độ "mục nát" (`Suy luận`, chưa đo).** Phương sai thêm vào mỗi thế hệ là 0,05 × 0,02² = 2×10⁻⁵. Trọng số khởi tạo theo He có độ lệch chuẩn √(2/15) ≈ 0,365, tức phương sai ≈ 0,133 (`evolution/brain_genotype.rs:267-285`). Để nhiễu trôi bằng cỡ tín hiệu ban đầu cần khoảng 6.700 thế hệ. Một lần chạy đạt bao nhiêu thế hệ thì **chưa đo**.
- **Có sẵn một "bánh cóc".** Tầng ẩn và tầng ra vẫn bị chọn lọc cho các việc khác: tìm cỏ, uống nước, CPG. Vì vậy cách "đọc" tín hiệu ở ô 11 thay đổi ngay cả khi 64 trọng số đầu vào được trả về giá trị cũ. Đây là bản mô hình của kết quả Bridgham và cộng sự 2009 (`Suy luận`).
- **Không Lamarck.** `AgentBrain.learned` không bao giờ chép vào genotype (ADR-0003). Cái học được trong đời không đi vào "tìm lại".

---

## 4. Bốn thí nghiệm chỉ Anima làm được (`Suy luận`, dựa trên §3)

Quy tắc chung cho cả bốn:

- Mỗi thí nghiệm dùng **khoá hoặc can thiệp opt-in mới**. Khi vắng khoá, checksum phải trùng từng bit với trunk.
- Không sửa `LIVE_OBSERVABLE_IDS` (bị đóng băng) và không thêm observable live mới. Số đo tính **ngoại tuyến** từ snapshot hoặc dữ liệu phả hệ xuất ra.
- Không chạm manifest, seed hay khoá của `SEL2-P2` (`tests/prereg_sel2p2_manifest_tests.rs`, [bản nháp](../ai/planning/2026-11-24-sel2-p2-preregistration-draft.md)).
- `WorldLawSet` bất biến trong một lần chạy. Mọi thay đổi theo thời gian là **can thiệp có `CauseId`**, không phải đổi luật.
- Kết quả ban đầu chỉ là **khám phá**. Muốn khẳng định điều gì thì phải preregister theo [mẫu thống kê](../ai/planning/2026-11-23-prereg-statistics-template.md).

### 4.1 (a) Dollo in silico: "đảo vắng thú săn rồi thú săn quay lại"

- **Câu hỏi.** Xác suất tìm lại phản ứng né thú săn phụ thuộc thế nào vào **thời gian vắng** Δ, và vào **mức trôi** của 64 trọng số ô 11?
- **Thiết kế.**
  1. Chạy với `live.evolved_brains = 1` và có thú săn tới `T_loss`.
  2. Tại `T_loss`, đưa toàn bộ thú săn vào ngăn `dormant` qua `HibernateAgentCommand`. EU kín, và thú săn bị "đông lạnh" chứ không tiến hoá tiếp.
  3. Từ cùng lịch sử con mồi, fork tại `T_loss + Δᵢ` cho nhiều Δ. Ở nhánh xử lý, đánh thức thú săn (`WakeAgentCommand`); nhánh đối chứng không bao giờ đánh thức.
- **Số đo (ngoại tuyến).**
  - *Thử nghiệm đầu dò*: chạy `BrainGenotype::forward` của từng con mồi trên một bộ đầu vào chuẩn, lần lượt với ô 11 bằng 1 và bằng 0, rồi lấy độ lệch đầu ra CPG. Đây là hàm thuần của genotype, không chạm mô phỏng.
  - Khoảng cách L2 từ 64 trọng số tới trung bình tại `T_loss`.
  - Tỉ lệ con mồi chết do bị săn sau khi thú săn quay lại.
  - "Tìm lại" nghĩa là phản ứng đạt ≥ x% mức trước khi mất trong vòng G thế hệ. x và G sẽ chốt khi preregister.
- **Phép thử bánh cóc.** Ghép 64 trọng số tổ tiên vào hậu duệ (chỉ ngoại tuyến). Nếu phản ứng không quay lại, phần hạ nguồn đã đổi.
- **Đối chứng null.** Nhánh không bao giờ đánh thức đo độ trôi tự phát. Nhánh thú săn chưa từng đi cho mức nền. Một bản phát lại trôi trung tính kiểu SEL2-D tách phần chọn lọc khỏi phần trôi.
- **Đối chứng âm.**
  - Số đốt (hình thái) **không được** cho thấy phụ thuộc vào Δ, vì không có vật liệu ẩn (§3.1).
  - Hoán vị nhãn Δ thì hệ số dốc phải về 0.
  - Với `live.mutation_rate = 0`, brain không đột biến (`agent_systems.rs:1180`), nên phản ứng giữ nguyên với mọi Δ. Đây cũng là đối chứng dương cho bộ máy đo.
- **An toàn khoa học.**
  - EU đi qua ngăn `dormant`: ngủ đông cộng vào `pool.dormant`, thức dậy trừ ra (`core/agent_systems.rs:257`, `:337-338`), và `live.closed_eu_total` đã cộng ngăn này (`core/live_experiment.rs:1753-1756`, `:1861-1864`).
  - Tắt học trong đời (`ANIMA_LIFETIME_LEARNING` không đặt) để phản ứng phản ánh genotype.
  - "Tìm lại một phản ứng" không phải hình thành loài.
- **Biến thể sau.** "Hang tối": một can thiệp opt-in xoá các ô thị giác 9–12. Giao với [QD-REALMS](QD_REALMS_HABITATS_RESEARCH_2026-11-22.md); để sau.
- **Code cần.** Một can thiệp opt-in "cửa sổ ngủ đông theo lớp" nối hai lệnh có sẵn. Test: vắng can thiệp thì bit-identical, có thì EU bảo toàn qua cửa sổ. Thêm công cụ đầu dò ngoại tuyến đọc `SavedSimulationState`.

### 4.2 (b) Hồi sinh tổ tiên và thi đấu xuyên thời gian kiểu LTEE

- **Code hiện cho phép gì.**
  - Snapshot tại `t₁` giữ mọi cá thể sống kèm brain (`SerializedAgent.brain`), và khôi phục **cả thế giới** đúng D01.
  - **Chưa** có cách trộn tổ tiên từ `t₁` vào thế giới tại `t₂`:
    - `spawn_serialized_agent` tạo EU không nguồn (`simulation_state.rs:988`);
    - `SpawnGenotypeCommand` gán brain mới (`agent_systems.rs:470-484`), vi phạm ý nghĩa "hồi sinh";
    - `GeneticInjection` không làm gì (F-071).
  - Keyframe ADR-0008 nặng khoảng 0,4–0,6 MB (ADR-0008, dòng 84) và chính là "tủ đông" của một lần chạy.
- **Thiết kế: hoán đổi một đổi một.**
  1. Tại `t₂`, chọn ngẫu nhiên k cư dân từ một luồng RNG phái sinh riêng, như cách `drift_control` dùng `sim_stream::LIVE_DRIFT_SELECTION`.
  2. Đặt mỗi tổ tiên vào đúng vị trí của một cư dân. **Chuyển nguyên reserve** của cư dân sang tổ tiên, rồi gỡ cư dân. EU giữ đúng, mật độ giữ nguyên.
  3. Đánh dấu tổ tiên trong một bảng phụ, như `LiveLineage` làm, để không đổi archetype của cư dân.
  4. Chiều ngược lại: đưa hậu duệ từ `t₂` vào snapshot `t₁`.
- **Số đo.** Độ thích nghi tương đối theo kiểu Lenski (1991): tỉ số các tham số Malthus ln(N_cuối/N_đầu) của nhóm tổ tiên so với nhóm cư dân, tính trên số hậu duệ còn sống từ phả hệ.
- **Đối chứng null.** Hoán đổi cư dân với bản sao của chính họ cùng tick (`t₁ = t₂`), và hoán đổi chỉ gắn nhãn: tỉ số kỳ vọng bằng 1.
- **Đối chứng âm.** Tổ tiên lấy từ một lần chạy `mutation_rate = 0` có genome trùng cư dân, nên tỉ số phải bằng 1.
- **An toàn khoa học.**
  - D01: tổ tiên mang đúng brain và phenotype đã lưu, không phát triển lại.
  - Đây là thao tác thí nghiệm, không phải sinh sản, và không được ghi thành cạnh `Mutate` hay `Clone`.
- **Code cần.** Một lệnh hoán đổi mới, giữ EU chính xác (cỡ M–L), cùng test bảo toàn EU và test vắng lệnh thì bit-identical.

### 4.3 (c) Chấm điểm các phương pháp dựng lại tổ tiên bằng sự thật đã ghi

- **Thiết kế.**
  1. Chạy với `live.lineage_tracking = 1` và đặt sàn nén cao hơn tổng số lần sinh, để giữ mọi nút cùng genotype.
  2. Xuất cây thật (`to_newick`) và bảng đặc điểm của mọi nút: `total_mass` (`evolution/genotype.rs:64`), chiều dài và bán kính từng đốt, số đốt.
  3. Giấu toàn bộ nút trong. Dựng lại từ các lá còn sống bằng phương pháp chuẩn:
     - ML theo chuyển động Brown cho đặc điểm liên tục;
     - mô hình Mk và parsimony cho số đốt;
     - lấy mẫu hậu nghiệm Bayes, để kiểm lại lỗi Williams và cộng sự (2006) đã thấy.
  4. So với sự thật.
- **Số đo.** Sai số theo độ sâu nút; độ phủ thực tế của khoảng tin cậy 95%; độ lệch có hướng; độ chính xác khi kiểm định Dollo cho số đốt, vì Anima biết **từng lần mất và lần tìm lại thật**, đúng thứ Goldberg & Igić (2008) chỉ mô phỏng được.
- **Đối chứng null.** Một lần chạy trôi trung tính, nơi giả định Brown gần đúng, phải cho độ phủ gần mức danh nghĩa. So với lần chạy có chọn lọc thì thấy phương pháp lệch tới đâu.
- **Đối chứng âm.** Hoán vị đặc điểm giữa các lá, hoặc hoán vị cây, thì độ chính xác phải tụt về mức chỉ dùng tiên nghiệm. Với `mutation_rate = 0`, mọi nút trùng nhau và bài toán tầm thường.
- **Một bẫy đã thấy trước.** Phép kẹp trong `mutation.rs` tạo **biên phản xạ**, nên chuyển động Brown sai ở gần biên. Phải báo cáo riêng các nút sát biên.
- **An toàn khoa học.**
  - Hoàn toàn ngoại tuyến; khoá ghi đã có test không đổi quỹ đạo (`live_lineage_tests.rs:495`).
  - Sàn nén đã được lưu trong `SavedLiveLineage.compaction_floor` (`core/live_lineage.rs:96-107`).
  - Công cụ ngoài như `ape` hay `phytools` (GPL) chỉ dùng qua định dạng và chạy ở tiến trình riêng, theo [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md).
- **Chi phí bộ nhớ.** Mỗi nút giữ một bản sao `MorphologyGenotype` đầy đủ (`evolution/lineage.rs:229-230`; `evolution/simplify.rs:5-8`), nên phải đo trước. Brain (khoảng 5.769 số f32, `core/resources.rs:933`) không vào nút; đặc điểm brain để giai đoạn sau, lấy từ snapshot.
- **Code cần.** Một cách đặt sàn nén cho lần chạy chuẩn: khoá opt-in hoặc binary riêng của harness. Một bộ xuất CSV + Newick. Một script so sánh. Cỡ S–M.

### 4.4 (d) Kiểm định hindcast từ một mốc đã ghi

- **Thiết kế.**
  1. Từ trạng thái đã ghi S(t₀), chạy N bản sao, mỗi bản **đổi seed của `SimRng` tại t₀**. Luật, can thiệp và trạng thái giữ nguyên.
  2. Với mỗi mốc về sau M, đo tỉ lệ bản sao tái hiện được nó: chỉ số lặp lại p_M. Ví dụ mốc: thú săn tuyệt chủng trước t₁, dấu của dịch chuyển `trait_mass`, thời điểm MRCA của quần thể sống, số đốt tối đa vượt k.
  3. Đặt lịch sử gốc vào phân phối ấy để xem nó điển hình hay hiếm.
  4. Bước hai, kiểu ABC: giấu một tham số của quá khứ, chẳng hạn tỉ lệ thú săn ban đầu, rồi suy ra nó từ các mốc. Chấm hậu nghiệm so với giá trị thật để đo **mức định danh được**.
- **Đối chứng.**
  - Đối chứng dương: các bản sao `InheritStream` phải trùng nhau tuyệt đối, p_M = 1 với mọi M.
  - Đối chứng âm: mốc định nghĩa trên chuỗi thời gian đã hoán vị phải cho p_M ở mức may rủi.
  - Chỉ so các lần chạy trên **cùng một nền tảng** và không ghim checksum tuyệt đối (ADR-0008, dòng 60).
- **An toàn khoa học.** Không sửa `checkpoint_fork`, vì AE-S09 cần nó bit-identical. Đổi seed là một tham số được khai báo và ghi vào provenance của nhánh. Không đổi luật.
- **Code cần.** Một móc `reseed` mới trên `ExperimentModel`, mặc định báo lỗi như `reconfigure_forcings` (`core/experiment_runner.rs:113-126`); `LiveExperimentAdapter` cài nó bằng `SimRng::reseed`. Một hàm runner cho tập hợp "fork đổi seed". Một bộ đánh giá mốc chạy ngoại tuyến. Cỡ M.

---

## 5. Giới hạn trung thực

- **Bộ gen trừu tượng không phải bộ gen thật.** `MorphologyGenotype` chỉ có các đốt (chiều dài, bán kính, khối lượng; tối đa 15 đốt) và khớp nối. Brain là một vector trọng số với kiến trúc cố định. Không có mạng điều hoà, giả gen hay con đường phát triển: `DevelopedPhenotype` của ADR-0001 **chưa tồn tại**, và genotype vẫn đi thẳng qua `decode_genotype`. "Gen mục nát" ở đây chỉ là **phép tương tự**.
- **Kết quả là mẫu hình trong thế giới mô hình.** Chúng **không** dự báo gì cho loài thật hay cho con người, và không ủng hộ hay bác bỏ một dự án hồi sinh loài cụ thể nào.
- **Quần thể nhỏ.** Theo QD-SCALE-TRIAL, đảo thử khoảng 100 cá thể, nên trôi gen rất mạnh. Mọi số thế hệ trong §3.1 chưa đo.
- **Hình thái ≠ loài.** "Tìm lại một đặc điểm" trong một dòng dõi không phải sự kiện hình thành loài (AE-S11/AE-S14).
- **Thú săn "đông lạnh" ở (a) khác tự nhiên**, nơi thú săn cũng tiến hoá. Muốn có đồng tiến hoá thì cần hai đảo theo [ADR-0014](../decisions/ADR-0014-shared-island-host-and-guests.md); để sau.
- **Tính tất định chỉ đúng trên cùng build và cùng lớp nền tảng.** Bit-exact đa nền tảng chưa được quyết.

---

## 6. Thẻ đề xuất và quyết định của chủ dự án

### 6.1 Thẻ (cỡ ước lượng: S ≤ 1 ngày agent, M 2–3 ngày, L > 3 ngày; `Suy luận`)

| Thẻ | Nội dung | Cỡ | Điều kiện |
|---|---|---|---|
| `RV-1` | Xuất phả hệ đầy đủ: cách đặt sàn nén cho lần chạy chuẩn, CSV đặc điểm + Newick, đo bộ nhớ; test bật/tắt ghi vẫn trùng checksum | S | không |
| `RV-2` | Chấm điểm dựng lại tổ tiên ngoại tuyến (ML-Brown, Mk, parsimony, Bayes) trên một lần chạy trôi trung tính và một lần có chọn lọc; báo cáo khám phá | S–M | `RV-1` |
| `RV-3` | Can thiệp opt-in "cửa sổ ngủ đông theo lớp" nối `HibernateAgentCommand` / `WakeAgentCommand`; test EU kín và bit-identical khi vắng | M | sau khi `SEL2-P2` freeze |
| `RV-4` | Đầu dò phản ứng brain ngoại tuyến và phép ghép trọng số tổ tiên (bánh cóc) | S | không |
| `RV-5` | Quét khám phá Dollo theo Δ (thí nghiệm a), chưa preregister | M | `RV-3`, `RV-4` |
| `RV-6` | Móc `reseed` trên `ExperimentModel` + tập hợp fork đổi seed; test `InheritStream` vẫn bit-identical | M | sau khi `SEL2-P2` freeze |
| `RV-7` | Bộ đánh giá mốc và chỉ số lặp lại p_M (thí nghiệm d) | S | `RV-6` |
| `RV-8` | Lệnh hoán đổi tổ tiên một đổi một giữ EU chính xác, kèm thử thi đấu (thí nghiệm b) | M–L | `RV-1`, sau `RV-5` |

### 6.2 Quyết định cần chủ dự án chốt

- **RV-D1 — Làm thí nghiệm nào trước.**
  - Khuyến nghị: **(c) trước (`RV-1`, `RV-2`), rồi (a)**.
  - Lý do: (c) rẻ nhất, ngoại tuyến, không chạm lịch tick, và dựng sẵn công cụ xuất tổ tiên mà (a) và (b) đều cần. Newick còn là phép kiểm độc lập cho hạ tầng phả hệ.
  - (a) là câu trả lời trực tiếp nhất cho câu hỏi "tìm lại cái đã mất".
- **RV-D2 — Thời điểm.**
  - Khuyến nghị: `RV-1`, `RV-2` và `RV-4` chạy song song ngay vì chúng ngoại tuyến hoặc opt-in.
  - Thẻ nào chạm lịch tick (`RV-3`, `RV-6`, `RV-8`) chờ `SEL2-P2` freeze để tránh mọi rủi ro với tính bit-identical của nó.
- **RV-D3 — Cơ chế "mất" của (a).**
  - Khuyến nghị: **ngủ đông thú săn** (EU kín, giữ nguyên cá thể theo D01).
  - Không khuyến nghị cắt rồi sinh lại thú săn bằng `SpawnGenotypeCommand`, vì cách đó gán brain mới, tức là thú săn khác.
- **RV-D4 — Phần "đã học" của brain khi hồi sinh ở (b).**
  - Khuyến nghị: giữ đúng D01 (mang nguyên brain) và chạy với **học trong đời tắt**, để phần học được không làm nhiễu phép so di truyền.
  - Không sửa D01.
- **RV-D5 — Vị thế của kết quả.**
  - Khuyến nghị: mọi kết quả của `RV-2`, `RV-5` và `RV-7` chỉ là **khám phá**.
  - Muốn có khẳng định thì phải preregister riêng: seed mới, khối seed tách biệt, không dùng lại dữ liệu khám phá.
- **RV-D6 — Cách đổi seed cho hindcast.**
  - Khuyến nghị: thêm móc `reseed` mới có mặc định báo lỗi.
  - **Không** sửa `checkpoint_fork`, và **không** để `BranchSeedStrategy` âm thầm chạm vào `SimRng` của thế giới live.
- **RV-D7 — Công cụ dựng lại tổ tiên bên ngoài.**
  - Khuyến nghị: chỉ dùng qua định dạng (Newick, CSV) và chạy ở tiến trình riêng.
  - Không liên kết thư viện GPL vào Anima.

---

## 7. Tài liệu tham khảo (metadata đối chiếu Crossref/PubMed ngày `2026-11-26`)

> **Đã bỏ hoặc sửa so với trí nhớ ban đầu:**
> - Bridgham, Carroll & Thornton 2006 nói về thụ thể **mineralocorticoid**.
> - Williams và cộng sự 2006 đăng ở **PLoS Comput Biol**, không phải PNAS.
> - Protas và cộng sự 2006 nói về **bạch tạng**.
> - Chưa định vị được trang chính xác của cụm "replaying life's tape" trong Gould 1989.
> - Câu trích về "sói dire" bị bỏ vì chưa xác minh.
> - Kalnay ghi năm 2003 theo bản in (Crossref ghi 2002).

### 7.1 Định luật Dollo, mất và tìm lại đặc điểm

1. Dollo, L. (1893). *Les lois de l'évolution.* Bull. Soc. belge Géol. Pal. Hydr., 7, 164–166.
2. Gould, S. J. (1970). *Dollo on Dollo's law: irreversibility and the status of evolutionary laws.* J. Hist. Biol., 3(2), 189–212. DOI: [10.1007/BF00137351](https://doi.org/10.1007/BF00137351)
3. Collin, R., & Miglietta, M. P. (2008). *Reversing opinions on Dollo's Law.* Trends Ecol. Evol., 23(11), 602–609. DOI: [10.1016/j.tree.2008.06.013](https://doi.org/10.1016/j.tree.2008.06.013)
4. Whiting, M. F., Bradler, S., & Maxwell, T. (2003). *Loss and recovery of wings in stick insects.* Nature, 421, 264–267. DOI: [10.1038/nature01313](https://doi.org/10.1038/nature01313)
5. Trueman, J. W. H., Pfeil, B. E., Kelchner, S. A., & Yeates, D. K. (2004). *Did stick insects really regain their wings?* Syst. Entomol., 29, 138–139. DOI: [10.1111/j.0307-6970.2004.00251.x](https://doi.org/10.1111/j.0307-6970.2004.00251.x)
6. Whiting, M. F., & Whiting, A. S. (2004). Trả lời Trueman và cộng sự. Syst. Entomol., 29, 140–141. DOI: [10.1111/j.1365-3113.2004.0255.x](https://doi.org/10.1111/j.1365-3113.2004.0255.x)
7. Goldberg, E. E., & Igić, B. (2008). *On phylogenetic tests of irreversible evolution.* Evolution, 62(11), 2727–2741. DOI: [10.1111/j.1558-5646.2008.00505.x](https://doi.org/10.1111/j.1558-5646.2008.00505.x)
8. Wiens, J. J. (2011). *Re-evolution of lost mandibular teeth in frogs after more than 200 million years, and re-evaluating Dollo's law.* Evolution, 65(5), 1283–1296. DOI: [10.1111/j.1558-5646.2011.01221.x](https://doi.org/10.1111/j.1558-5646.2011.01221.x)
9. Paluh, D. J., Dillard, W. A., Stanley, E. L., Fraser, G. J., & Blackburn, D. C. (2021). Evolution, 75, 3203–3213. DOI: [10.1111/evo.14379](https://doi.org/10.1111/evo.14379)
10. Kollar, E. J., & Fisher, C. (1980). *Tooth induction in chick epithelium: expression of quiescent genes for enamel synthesis.* Science, 207, 993–995. DOI: [10.1126/science.7352302](https://doi.org/10.1126/science.7352302)
11. Harris, M. P., Hasso, S. M., Ferguson, M. W. J., & Fallon, J. F. (2006). *The development of archosaurian first-generation teeth in a chicken mutant.* Curr. Biol., 16(4), 371–377. DOI: [10.1016/j.cub.2005.12.047](https://doi.org/10.1016/j.cub.2005.12.047)
12. Marshall, C. R., Raff, E. C., & Raff, R. A. (1994). *Dollo's law and the death and resurrection of genes.* PNAS, 91(25), 12283–12287. DOI: [10.1073/pnas.91.25.12283](https://doi.org/10.1073/pnas.91.25.12283)
13. Lahti, D. C., Johnson, N. A., Ajie, B. C., Otto, S. P., Hendry, A. P., et al. (2009). *Relaxed selection in the wild.* Trends Ecol. Evol., 24(9), 487–496. DOI: [10.1016/j.tree.2009.03.010](https://doi.org/10.1016/j.tree.2009.03.010)
14. Jeffery, W. R. (2009). *Regressive evolution in Astyanax cavefish.* Annu. Rev. Genet., 43, 25–47. DOI: [10.1146/annurev-genet-102108-134216](https://doi.org/10.1146/annurev-genet-102108-134216)
15. Yamamoto, Y., & Jeffery, W. R. (2000). *Central role for the lens in cave fish eye degeneration.* Science, 289, 631–633. DOI: [10.1126/science.289.5479.631](https://doi.org/10.1126/science.289.5479.631)
16. Protas, M. E., Hersey, C., Kochanek, D., et al. (2006). Nat. Genet., 38, 107–111 (bạch tạng, *Oca2*). DOI: [10.1038/ng1700](https://doi.org/10.1038/ng1700)
17. Teotónio, H., & Rose, M. R. (2000). *Variation in the reversibility of evolution.* Nature, 408, 463–466. DOI: [10.1038/35044070](https://doi.org/10.1038/35044070)
18. Teotónio, H., & Rose, M. R. (2001). *Perspective: reverse evolution.* Evolution, 55(4), 653–660. DOI: [10.1554/0014-3820(2001)055[0653:PRE]2.0.CO;2](https://doi.org/10.1554/0014-3820(2001)055[0653:PRE]2.0.CO;2)
19. Teotónio, H., Chelo, I. M., Bradić, M., Rose, M. R., & Long, A. D. (2009). Nat. Genet., 41, 251–257. DOI: [10.1038/ng.289](https://doi.org/10.1038/ng.289)

### 7.2 Hồi sinh phân tử, dựng lại tổ tiên, chuẩn có sự thật

20. Thornton, J. W. (2004). *Resurrecting ancient genes: experimental analysis of extinct molecules.* Nat. Rev. Genet., 5, 366–375. DOI: [10.1038/nrg1324](https://doi.org/10.1038/nrg1324)
21. Bridgham, J. T., Carroll, S. M., & Thornton, J. W. (2006). *Evolution of hormone-receptor complexity by molecular exploitation.* Science, 312, 97–101. DOI: [10.1126/science.1123348](https://doi.org/10.1126/science.1123348)
22. Bridgham, J. T., Ortlund, E. A., & Thornton, J. W. (2009). *An epistatic ratchet constrains the direction of glucocorticoid receptor evolution.* Nature, 461, 515–519. DOI: [10.1038/nature08249](https://doi.org/10.1038/nature08249)
23. Pagel, M. (1999). Syst. Biol., 48(3), 612–622. DOI: [10.1080/106351599260184](https://doi.org/10.1080/106351599260184)
24. Maddison, W. P. (2006). *Confounding asymmetries in evolutionary diversification and character change.* Evolution, 60(8), 1743–1746. DOI: [10.1111/j.0014-3820.2006.tb00517.x](https://doi.org/10.1111/j.0014-3820.2006.tb00517.x)
25. Williams, P. D., Pollock, D. D., Blackburne, B. P., & Goldstein, R. A. (2006). *Assessing the accuracy of ancestral protein reconstruction methods.* PLoS Comput. Biol., 2(6), e69. DOI: [10.1371/journal.pcbi.0020069](https://doi.org/10.1371/journal.pcbi.0020069)
26. Hillis, D. M., Bull, J. J., White, M. E., Badgett, M. R., & Molineux, I. J. (1992). *Experimental phylogenetics: generation of a known phylogeny.* Science, 255, 589–592. DOI: [10.1126/science.1736360](https://doi.org/10.1126/science.1736360)
27. Randall, R. N., Radford, C. E., Roof, K. A., Natarajan, D. K., & Gaucher, E. A. (2016). *An experimental phylogeny to benchmark ancestral sequence reconstruction.* Nat. Commun., 7, 12847. DOI: [10.1038/ncomms12847](https://doi.org/10.1038/ncomms12847)

### 7.3 Tua lại cuộn băng, LTEE và sinh vật số

28. Gould, S. J. (1989). *Wonderful Life: The Burgess Shale and the Nature of History.* W. W. Norton, New York.
29. Lenski, R. E., Rose, M. R., Simpson, S. C., & Tadler, S. C. (1991). Am. Nat., 138, 1315–1341. DOI: [10.1086/285289](https://doi.org/10.1086/285289)
30. Blount, Z. D., Borland, C. Z., & Lenski, R. E. (2008). *Historical contingency and the evolution of a key innovation in an experimental population of Escherichia coli.* PNAS, 105(23), 7899–7906. DOI: [10.1073/pnas.0803151105](https://doi.org/10.1073/pnas.0803151105)
31. Wiser, M. J., Ribeck, N., & Lenski, R. E. (2013). *Long-term dynamics of adaptation in asexual populations.* Science, 342, 1364–1367. DOI: [10.1126/science.1243357](https://doi.org/10.1126/science.1243357)
32. Blount, Z. D., Lenski, R. E., & Losos, J. B. (2018). *Contingency and determinism in evolution: Replaying life's tape.* Science, 362(6415), eaam5979. DOI: [10.1126/science.aam5979](https://doi.org/10.1126/science.aam5979)
33. Lenski, R. E., Ofria, C., Pennock, R. T., & Adami, C. (2003). *The evolutionary origin of complex features.* Nature, 423, 139–144. DOI: [10.1038/nature01568](https://doi.org/10.1038/nature01568)
34. Yedid, G., Ofria, C. A., & Lenski, R. E. (2008). *Historical and contingent factors affect re-evolution of a complex feature lost during mass extinction in communities of digital organisms.* J. Evol. Biol., 21, 1335–1357. DOI: [10.1111/j.1420-9101.2008.01564.x](https://doi.org/10.1111/j.1420-9101.2008.01564.x)

### 7.4 Hindcast, đồng hoá số liệu, hỗn loạn và ABC

35. Lorenz, E. N. (1963). *Deterministic nonperiodic flow.* J. Atmos. Sci., 20(2), 130–141. DOI: [10.1175/1520-0469(1963)020<0130:DNF>2.0.CO;2](https://doi.org/10.1175/1520-0469(1963)020%3C0130:DNF%3E2.0.CO;2)
36. Kalnay, E. (2003). *Atmospheric Modeling, Data Assimilation and Predictability.* Cambridge University Press. DOI: [10.1017/CBO9780511802270](https://doi.org/10.1017/CBO9780511802270)
37. Hersbach, H., Bell, B., Berrisford, P., et al. (2020). *The ERA5 global reanalysis.* Q. J. R. Meteorol. Soc., 146(730), 1999–2049. DOI: [10.1002/qj.3803](https://doi.org/10.1002/qj.3803)
38. Pritchard, J. K., Seielstad, M. T., Perez-Lezaun, A., & Feldman, M. W. (1999). Mol. Biol. Evol., 16(12), 1791–1798. DOI: [10.1093/oxfordjournals.molbev.a026091](https://doi.org/10.1093/oxfordjournals.molbev.a026091)
39. Beaumont, M. A., Zhang, W., & Balding, D. J. (2002). *Approximate Bayesian computation in population genetics.* Genetics, 162(4), 2025–2035. DOI: [10.1093/genetics/162.4.2025](https://doi.org/10.1093/genetics/162.4.2025)
40. Csilléry, K., Blum, M. G. B., Gaggiotti, O. E., & François, O. (2010). Trends Ecol. Evol., 25, 410–418. DOI: [10.1016/j.tree.2010.04.001](https://doi.org/10.1016/j.tree.2010.04.001)
41. Beaumont, M. A. (2010). Annu. Rev. Ecol. Evol. Syst., 41, 379–406. DOI: [10.1146/annurev-ecolsys-102209-144621](https://doi.org/10.1146/annurev-ecolsys-102209-144621)

### 7.5 Hồi sinh loài đã tuyệt chủng (`Nguồn ngoài — tổ chức / tin tức`)

42. IUCN SSC (2016). *Guiding Principles on Creating Proxies of Extinct Species for Conservation Benefit.* Version 1.0. Gland: IUCN SSC. https://portals.iucn.org/library/efiles/documents/Rep-2016-009.pdf
43. Folch, J., Cocero, M. J., Chesné, P., et al. (2009). *First birth of an animal from an extinct subspecies (Capra pyrenaica pyrenaica) by cloning.* Theriogenology, 71(6), 1026–1034. DOI: [10.1016/j.theriogenology.2008.11.005](https://doi.org/10.1016/j.theriogenology.2008.11.005)
44. Callaway, E. (2025). *This company claimed to 'de-extinct' dire wolves. Then the fighting started.* Nature, 644, 21–23 (tin). DOI: [10.1038/d41586-025-02456-3](https://doi.org/10.1038/d41586-025-02456-3)
