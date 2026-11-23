---
title: Đài quan sát tiến hoá, phát hiện sự kiện và chuẩn dữ liệu tái lập — đánh giá đề xuất ngoài
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-22
review_cycle: quarterly
---

# Đài quan sát tiến hoá, phát hiện sự kiện và chuẩn dữ liệu tái lập — đánh giá đề xuất ngoài

> **Phạm vi & tính chất tài liệu.** Đây là bản **đánh giá và chắt lọc** một báo cáo nghiên cứu ngoài
> (*"Quan Sát Tiến Hoá Anima"*, soạn bằng công cụ nghiên cứu tự động, nhận ngày 2026-11-22; dưới đây
> gọi là **báo cáo gốc**). Báo cáo gốc đề xuất ba khối: giao diện đài quan sát đa tỷ lệ (Muller plot,
> lưới thức ăn động, bản đồ nhiệt hốc sinh thái), bộ phát hiện sự kiện sinh thái khử nhiễu cho
> ChroniclePanel, và chuỗi tái lập "bit-exact" đa nền tảng kèm đóng gói Parquet / RO-Crate / ODD /
> Zenodo. Tài liệu này **không** phải hợp đồng hay kế hoạch đã duyệt. Số đo của repo chỉ nằm ở
> [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền);
> quyết định hiện hành về tất định và quá khứ thế giới nằm ở
> [ADR-0008](../decisions/ADR-0008-world-past-and-time.md),
> [ADR-0010](../decisions/ADR-0010-pinned-world-artifact.md) và
> [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md). Câu hỏi khoa học chủ lực là
> [quy luật đảo](../ai/requirements/2026-11-19-feature-island-evolution-observatory.md) (QD-ISLAND).

## 0. Nhãn bằng chứng và kết luận ngắn

| Nhãn | Nghĩa |
|---|---|
| **Đã kiểm** | Đối chiếu trực tiếp với mã nguồn và tài liệu của repo ngày 2026-11-22, hoặc mở trang nguồn ngày đó. |
| **Nguồn ngoài** | Công trình có thật mà báo cáo gốc trích; nội dung tóm theo báo cáo gốc, chưa đọc lại từng bài trừ khi ghi rõ. |
| **Chưa xác minh** | Con số dẫn về mục trích dẫn số 25 của báo cáo gốc (`unknown_url`), tức mô phỏng riêng của công cụ không có mã, hoặc không tìm thấy trong nguồn được dẫn. |
| **Suy luận** | Đánh giá của tài liệu này. |

**Kết luận (`Suy luận`):**

1. **Dùng được khoảng một phần ba.** Bộ lọc bền bỉ cho sự kiện, tiêu chí nguồn–bẫy cho xâm lấn, cách
   dựng Muller plot không tự cắt, libm tất định và lưu ý về siêu dữ liệu phát hành đều khớp với việc
   repo đang làm.
2. **Phần còn lại viết cho một dự án khác**: 50.000 tác tử, mô phỏng trên GPU hoặc wasm, và lời hứa
   giống từng bit giữa các hệ điều hành. Repo đã đo và chọn khác (§3).
3. **Mọi con số định lượng "đã kiểm chứng" trong báo cáo gốc là `Chưa xác minh`** (§4). Không dùng làm
   ngân sách, ngưỡng hay tiêu chí cổng.
4. Mọi phần giao diện mới vướng khoá phạm vi QD-ISLAND §9 ("Đồ hoạ: … không thêm tính năng mới"), nên
   phần đáng làm sớm là **phân tích ngoại tuyến** trên dữ liệu đã xuất, không phải panel mới.

## 1. Hiện trạng liên quan (`Đã kiểm`)

- **Phả hệ:** `InMemoryLineageTracker` có `compact` ([`lineage.rs:259`](../../src-tauri/src/evolution/lineage.rs));
  `simplify` kiểu tskit, giữ số `Mutate` qua cạnh đã nén ([`simplify.rs:188`](../../src-tauri/src/evolution/simplify.rs));
  MRCA ([`mrca.rs:177`](../../src-tauri/src/evolution/mrca.rs)); Newick
  ([`newick.rs:164`](../../src-tauri/src/evolution/newick.rs)). Còn thiếu *line of descent* (OSS-073).
- **Xuất dữ liệu:** CSV, JSON và Newick qua `ScientificExportRequest`
  ([`telemetry_export.rs:778-800`](../../src-tauri/src/core/telemetry_export.rs)). Arrow/Parquet đang
  *Defer* ([`OPEN_SOURCE_LANDSCAPE.md:66`](OPEN_SOURCE_LANDSCAPE.md)).
- **Lưới thức ăn:** "Nothing in the running app writes an `EnergyFluxMatrix`"
  ([`telemetry_export.rs:321`](../../src-tauri/src/core/telemetry_export.rs)); sổ năng lượng chỉ giữ
  một bể động vật, không tách guild.
- **ChroniclePanel** ([`ChroniclePanel.tsx`](../../src/components/ChroniclePanel.tsx)) hiển thị
  `chronicle_history`, do Meta-AI ghi với `Uuid::new_v4()` và `SystemTime::now()`
  ([`meta_ai.rs:242-258`](../../src-tauri/src/evolution/meta_ai.rs)). Đây là nhật ký can thiệp môi
  trường, **không** phải luồng sự kiện sinh thái.
- **Sổ sự kiện thế giới** của ADR-0008 (đã chấp nhận) khai `Birth`, `Death { cause }`, `Kill`,
  `Migration`, `Chronicle`, `EpochReplacement`; là đầu ra, không đọc ngược vào mô phỏng
  ([ADR-0008:150](../decisions/ADR-0008-world-past-and-time.md)), kèm `world_checksum` mỗi khoảng 600 tick
  (dòng 162).
- **RNG:** `SimRng` là `ChaCha12Rng` có vị trí luồng lưu và khôi phục được
  ([`resources.rs:361-390`](../../src-tauri/src/core/resources.rs)); World v2 tách miền seed bằng SHA-256
  ([`anima-worldgen/src/rng.rs`](../../src-tauri/crates/anima-worldgen/src/rng.rs)).
- **Gọi tên loài:** ER09 chỉ cho gọi "candidate species" khi qua bằng chứng tối thiểu
  ([`EVOLUTION_EXPERIMENT_CONTRACT.md:164-175`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)).

## 2. Ý tưởng: nhận, để sau, hay bỏ (`Suy luận`)

| Ý tưởng của báo cáo gốc | Quyết định đề xuất | Nơi sẽ đặt | Đã có gì |
|---|---|---|---|
| Muller plot: cắt dòng hiếm bằng cách gộp về tổ tiên gần nhất được giữ, xếp lồng DFS hai lần, PCHIP trên **độ dày từng lớp** rồi chuẩn hoá tổng | **Nhận, ngoại tuyến** (hình cho bài báo) | Script phân tích trên dữ liệu xuất; QD-ISLAND §5 | Phả hệ và MRCA có; chưa có chuỗi tần số theo dòng, chưa có view Muller |
| Bộ lọc bền bỉ (MODES, Dolson et al. 2019): sự kiện là "ứng viên" tới khi dòng còn hậu duệ sau N thế hệ | **Nhận** | Sự kiện dẫn xuất đọc từ sổ sự kiện ADR-0008, tính sau, ngoài vòng tick | Trùng ý ER09 "lineage bền qua số thế hệ tối thiểu" |
| Tiêu chí xâm lấn nguồn–bẫy (Pulliam 1988): sinh tại chỗ > chết tại chỗ, không tính nhập cư | **Nhận** | QD-ISLAND §5 (chỉ tiêu theo đảo, H2) | `Migration` đã có trong sổ sự kiện; chưa có chỉ tiêu theo đảo |
| Hệ số lưỡng cực + Hartigan dip + khoảng cách 2-GMM cho "phân nhánh" | **Ứng viên** cho ngưỡng có version | ER09 | Chưa có `SpeciesClusterRecord`; chỉ được gọi "candidate species" |
| STARS (Rodionov) trên đường bao đã khử chu kỳ săn mồi–con mồi; BOCPD | **Để sau**; nếu dùng thì khai trước trong bản đăng ký | Phân tích thí nghiệm, không phải UI | SEL2-E và smoke SEL2-P thấy bùng nổ–sụp đổ, nên cần khử chu kỳ trước khi dò điểm gãy |
| Bảng phả hệ theo ALife Data Standards | **Nhận định dạng** | Cạnh Newick trong `telemetry_export.rs` | Newick đã ship (OSS-070) |
| Libm thuần phần mềm, ghim phiên bản | **Đã là một phương án mở** của QĐ-M6 | [`determinism_matrix.md:110`](../paper/data/determinism_matrix.md) | Phạm vi đã đo: `sin`, `cos`, `exp`, `powf`, `atan2`, `tanh` |
| Một nguồn siêu dữ liệu duy nhất cho `CITATION.cff` / `.zenodo.json`; ODD cho bài phương pháp | **Nhận cho Chặng 5** | Danh sách phát hành v0.9 | `CITATION.cff` có ở gốc repo; chưa có `.zenodo.json` |
| Bậc dinh dưỡng MacKay et al. 2020, bố cục ghim Frishman & Tal 2008 | **Để sau** | — | Không có ma trận dòng được đo để vẽ |
| Bản đồ nhiệt hốc PCA-env (Broennimann), căn trục Procrustes | **Để sau** | — | Không có chỉ tiêu hốc; vướng khoá phạm vi đồ hoạ |
| Philox4x32-10, tắt FMA, hstrat trên GPU, tskit cho sinh sản hữu tính, Parquet nhiều tầng, RO-Crate/BagIt tự đẩy lên Zenodo | **Bỏ lúc này** | — | Xem §3 |

## 3. Chỗ báo cáo gốc sai về repo hoặc va với quy tắc (`Đã kiểm` + `Suy luận`)

- **"Bit-exact" giữa x86_64, aarch64 và wasm32.** Repo hứa *cùng manifest và cùng build thì cùng quỹ
  đạo*, không hứa giống từng bit giữa target ([`DETERMINISM_CONTRACT.md:18`](../reference/DETERMINISM_CONTRACT.md),
  [`determinism.rs:39-43`](../../src-tauri/src/core/determinism.rs)). Thẻ M1 đo được thế giới live lệch
  giữa Windows, Linux và macOS vì libm lệch 1 ULP, còn hai máy macOS thì trùng
  ([ADR-0008:55-60](../decisions/ADR-0008-world-past-and-time.md)). Chuyển sang giống từng bit là
  **QĐ-M6**, một quyết định có giá vì đổi checksum của mọi thế giới live. Test không bao giờ ghim
  checksum tuyệt đối, chỉ so các lượt chạy.
- **Philox.** Không cần: ChaCha12 vốn là bộ sinh dựa trên bộ đếm và tua được vị trí. Điều báo cáo gốc
  muốn (luồng theo từng thực thể, không phụ thuộc số luồng) đã được giải bằng lịch tick đơn luồng dưới
  `DeterministicMode` và luồng tách miền.
- **FMA.** rustc không tự gộp `a*b + c` thành FMA (chỉ khi gọi `mul_add`); nguyên nhân lệch đã đo là
  libm, không phải FMA.
- **wasm32, trình duyệt, GPU.** Anima là app Tauri desktop; mô phỏng chạy trên CPU (Bevy ECS). GPU chỉ
  dùng cho learner `burn-wgpu` sau `ANIMA_USE_GPU`, còn CPU `burn-flex` là chuẩn tất định. Đề xuất
  hstrat trên GPU vì thế không áp dụng.
- **Quy mô.** Báo cáo gốc giả định 50.000 tác tử × 5.000 thế hệ. M3 đo 1.000 agent 5,6 ms/tick
  (Windows), 3,6 ms (Mac), và 4.096 agent vượt ngân sách 60 Hz ([`scaling.md`](../paper/data/scaling.md));
  S13 đạt khoảng 32,7 thế hệ trong 200.000 tick (STATE §3.00). Mọi bài toán bộ nhớ phả hệ ở cỡ "hàng
  trăm triệu lượt sinh" không phải bài toán hiện tại.
- **tskit.** Crate đã bị loại có lý do (binding C, mô hình khoảng genomic); thuật toán `simplify` đã viết
  bằng Rust ([`OPEN_SOURCE_LANDSCAPE.md:90`](OPEN_SOURCE_LANDSCAPE.md)).
- **ChroniclePanel.** Báo cáo gốc coi nó là luồng sự kiện sinh thái; thực tế là nhật ký Meta-AI dùng
  uuid và đồng hồ thật (§1). Một bộ dò sự kiện phải đọc sổ sự kiện ADR-0008, giữ nguyên "không phản hồi"
  (bật hay tắt ghi cho cùng checksum) và không cấp phát trên vòng tick.
- **Lưới thức ăn theo guild.** Không có dữ liệu để vẽ: app không ghi `EnergyFluxMatrix` và sổ năng lượng
  giữ một bể động vật (§1).
- **Gọi "phân nhánh tiến hoá" hay "loài".** Phải qua ER09 và cổng AE-S11/AE-S14; trước đó chỉ là
  "morph", "ecotype" hoặc "candidate species".

## 4. Con số chưa xác minh trong báo cáo gốc

Các con số sau dẫn về mục trích dẫn số 25 (`unknown_url`), tức các "mô phỏng kiểm chứng" của chính
công cụ, không có mã hay dữ liệu đi kèm. Chúng là **giả thuyết**, không phải số đo:

- 11,18 GB RAM cho cây phả hệ không cắt tỉa (50.000 tác tử × 5.000 thế hệ, 48 byte/nút).
- 25,2 MB so với 4,58 MB khi thu gọn nút đơn nhánh; 3,05 MB cho hstrat 64 byte × 50.000 tác tử.
- Độ dày âm −0,2396 (spline tự nhiên) và −0,0215 (PCHIP trên biên tích luỹ) so với +0,0097 (PCHIP trên
  độ dày từng lớp), sai số tổng 2,2e-16.
- 22 báo động giả của CUSUM giảm còn 0 với STARS; bước ngoặt ở bước 250, RSI = 27,58.
- Bậc dinh dưỡng thú săn đỉnh 3,028 so với 1,559 khi trộn cạnh mùn bã; giảm 4,7 lần độ dịch chuyển ngang
  (0,1711 so với 0,8081), khoảng cách tách nút 0,758.
- CSV 6,75 GB, nhị phân 2,98 GB, Parquet 0,93 GB; gói kiểm chứng 43,5 MB.

**Kiểm ngẫu nhiên trích dẫn (`Đã kiểm`, mở trang ngày 2026-11-22):**

| Trích dẫn | Kết quả |
|---|---|
| Phylotrack, arXiv:2405.09389: 67.000 lượt sinh/giây, 296 MiB | **Không có trong tóm tắt** (tóm tắt chỉ nói định tính "hàng chục nghìn"). Chưa đọc thân bài: `Chưa xác minh` |
| arXiv:2608.17159, khảo sát 117 dự án: đồng thuận 0,61–0,64, ORCID 62 % → 0 % | Bài có thật (Shan, 17/08/2026, 117 dự án). Các con số **không có trong tóm tắt**: `Chưa xác minh` |
| PhyloFrame, arXiv:2605.28545: cây "từ 300.000 tới 30 triệu lá" | Bài có thật (Moreno, Sukumaran, Zaman, Dolson, 27/05/2026). Tóm tắt nói "≥ 300.000 taxa"; **không có 30 triệu** |
| Zenodo: có cả `.zenodo.json` và `CITATION.cff` thì chỉ dùng `.zenodo.json` | **Đúng** theo trang trợ giúp Zenodo "Describe software" |

Chưa kiểm: báo cáo "DOI deadlock" của Weiss (05/2026), roc-validator 0.13 (10/2026), "Dynamic Tree
Colors" (2026), ngày preview của mã hoá ALP trong Parquet, và ngày phát hành RO-Crate 1.3.

## 5. Việc nên làm tiếp (`Suy luận`, chờ chủ dự án)

1. **Không** mở thẻ giao diện nào từ báo cáo gốc khi khoá phạm vi QD-ISLAND §9 còn hiệu lực.
2. Khi làm chỉ tiêu theo đảo (QD-ISLAND M1): thêm tiêu chí xâm lấn nguồn–bẫy và chuỗi tần số theo dòng
   cho từng đảo, để vẽ Muller plot ngoại tuyến cho bài báo.
3. Khi sổ sự kiện ADR-0008 có trên thân cây: thử một bộ dò sự kiện dẫn xuất ngoài tiến trình, có bộ lọc
   bền bỉ, kèm đối chứng âm (chuỗi chỉ có dao động săn mồi–con mồi không được sinh sự kiện).
4. Khi định nghĩa `SpeciesClusterRecord`: cân nhắc hệ số lưỡng cực + dip test làm ngưỡng có version.
5. Xuất thêm bảng phả hệ theo ALife Data Standards cạnh Newick (OSS-070, OSS-073).
6. Chặng 5: một nguồn siêu dữ liệu duy nhất cho `CITATION.cff` và `.zenodo.json` (nếu thêm), và mô tả
   mô hình theo ODD cho bài phương pháp.
7. QĐ-M6 vẫn là chỗ quyết libm tất định; báo cáo gốc không thêm bằng chứng mới cho quyết định đó.

## 6. Tài liệu tham khảo chính (theo báo cáo gốc; chưa kiểm lại từng DOI trừ §4)

- Dolson, E., Vostinar, A., Wiser, M., & Ofria, C. (2019). The MODES toolbox: Measurements of open-ended
  dynamics in evolving systems. *Artificial Life*, 25(1), 50–73.
- Pulliam, H. R. (1988). Sources, sinks, and population regulation. *American Naturalist*, 132(5), 652–661.
- Rodionov, S. N. (2004). A sequential algorithm for testing climate regime shifts. *Geophysical Research
  Letters*, 31, L09204.
- Hartigan, J. A., & Hartigan, P. M. (1985). The dip test of unimodality. *Annals of Statistics*, 13(1), 70–84.
- Fritsch, F. N., & Carlson, R. E. (1980). Monotone piecewise cubic interpolation. *SIAM J. Numer. Anal.*, 17(2), 238–246.
- MacKay, R. S., Johnson, S., & Sansom, B. (2020). How directed is a directed network? *R. Soc. Open Sci.*, 7, 201138.
- Lalejini, A., Dolson, E., Bohm, C., et al. (2019). Data standards for artificial life software. ALIFE 2019.
- Dolson, E., et al. (2024). Phylotrack: C++ and Python libraries for in silico phylogenetic
  tracking. arXiv:2405.09389.
- Grimm, V., et al. (2020). The ODD protocol for describing agent-based and other simulation models: A
  second update. *JASSS*, 23(2), 7.
- Noble, R. ggmuller (R package, CRAN).
