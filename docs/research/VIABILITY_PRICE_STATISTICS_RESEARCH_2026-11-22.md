---
title: Sinh thái học Khả năng Sống sót, Phương trình Price và Thống kê cho QD-ISLAND
status: proposed
owner: architecture
last_reviewed: 2026-11-24
review_cycle: quarterly
---

# Sinh thái học Khả năng Sống sót, Phương trình Price và Thống kê cho QD-ISLAND

> **Phạm vi & tính chất tài liệu.** Tài liệu này là **tổng hợp nghiên cứu ngoài kết hợp đối chiếu mã nguồn Anima Engine** cho ba nút thắt nối tiếp ở Chặng 4 (`SEL2-P` boom–bust, `E3-X` `insufficient_evidence` do tuyệt chủng khan hiếm, và chuẩn bị thiết kế `2×2×2` của [`2026-11-19-feature-island-evolution-observatory.md`](../ai/requirements/2026-11-19-feature-island-evolution-observatory.md)). Đây **không** phải hợp đồng hay số đo pass/fail chính thức của dự án; bằng chứng đo được trên repo nằm duy nhất tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
>
> **Cảnh báo nguồn số liệu kiểm chứng (`2026-11-22`).** Các kết quả mô phỏng Monte Carlo và ODE minh hoạ trong báo cáo gốc (như mức giảm CV 63%, sai số loại I 68,2% của phân tích sống sót, hay ngưỡng $N_{\text{seed}}$ cụ thể) được dẫn từ báo cáo tổng hợp bên ngoài với nguồn nội bộ chưa công bố liên kết (đã loại bỏ các mục placeholder không có URL khỏi phần tài liệu tham khảo). Nguồn đó đã bị loại khỏi tài liệu, nên mọi con số mô phỏng kiểm chứng ấy (kể cả tỷ lệ sống sót 24 seed, $N_{\min}$, ICC 0,82 / sai số loại I 82,9%, $N_{\text{seed}} = 32 / 48\text{--}64 / 96$, 68,2%) là **giả thuyết định lượng chưa xác minh, chưa đo trên Anima**. Chúng **không được dùng để đặt tiêu chí cổng** hay số seed của bất kỳ bản đăng ký nào; chỉ số đo trên Anima (khảo sát VIAB-1 và các thẻ trong [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) §3.00) mới làm được việc đó.
>
> **Liên quan (2026-11-22):** [`OPEN_ENDEDNESS_LINEAGE_SPECIES_RESEARCH_2026-11-22.md`](OPEN_ENDEDNESS_LINEAGE_SPECIES_RESEARCH_2026-11-22.md) §7 ghi rằng mục thống kê của một báo cáo ngoài khác (kiểm tra tính chuẩn trước, rồi Cliff's δ / Vargha–Delaney A) mâu thuẫn với khuyến nghị cho thiết kế ghép cặp theo seed ở §0 và §3.3 ở đây; tài liệu này được ưu tiên.

---

## 0. Đối chiếu mã nguồn Anima Engine & đính chính khuyến nghị cũ (`Đã kiểm`, 2026-11-22)

1. **Hiện trạng cơ chế sinh thái trong mã nguồn ([`ecology.rs`](../../src-tauri/src/core/ecology.rs), [`locomotion.rs`](../../src-tauri/src/core/locomotion.rs), [`environmental_systems.rs`](../../src-tauri/src/core/environmental_systems.rs), [`agent_systems.rs`](../../src-tauri/src/core/agent_systems.rs)):**
   - Hàm `herbivore_intake` trong [`ecology.rs`](../../src-tauri/src/core/ecology.rs) hiện đang dùng `holling_type2`. Hàm `holling_type3` trong cùng file **cố định số mũ 2** (`a·N² / (1 + a·h·N²)`), còn `holling_type3_with_refuge` thêm hằng số trú ẩn vào mẫu số; cả hai đang dùng cho bắt mồi (`holling_type3_with_refuge` trong phép tính năng lượng bắt được), không dùng cho gặm cỏ. Một biến thể Holling III với $q \approx 1{,}5$ là **mã mới**, không phải "đấu nối hàm sẵn có".
   - `GRASS_STAY_FRACTION = 0.25` tại [`locomotion.rs`](../../src-tauri/src/core/locomotion.rs) **không phải** neo dự trữ thực vật: đó là ngưỡng ở-lại-hay-rời-đi của chế độ opt-in `live.grass_steering` (con mồi đứng yên nếu cỏ gặm được ở ô hiện tại $\ge 0{,}25 \cdot r_{\max}$, nếu không thì lái tới ô giàu hơn trong cửa sổ `7×7`). Nói cách khác, một ngưỡng bỏ mảng kiểu GUD **đã có trong mã**. Thẻ S11 (lái tới cỏ, tiền thân của khoá này) đo ra kết quả **âm**: lái tới cỏ không tăng lượng ăn ([kết quả S11](../ai/planning/2026-11-18-s11-grass-steering.md)). Neo dự trữ thực vật thật sự là `live.plant_reserve` (S10) tại [`environmental_systems.rs`](../../src-tauri/src/core/environmental_systems.rs).
   - Bước tái sinh cỏ `step_regrowth_gated_strided` (`REGROWTH_STRIDE = 4` tại [`ecology.rs`](../../src-tauri/src/core/ecology.rs)) rút năng lượng trực tiếp từ mảnh vụn (*detritus*) theo tốc độ logistic, không có bể khoáng hoá trễ riêng.
   - `autonomous_reproduction_system` tại [`agent_systems.rs`](../../src-tauri/src/core/agent_systems.rs) **đã có** ngưỡng sinh sản theo dự trữ (con mồi: `0.65 · energy_target`; kẻ săn mồi: `0.95`) và phần dự trữ an toàn bố mẹ giữ lại (con mồi `0.30`, kẻ săn mồi `0.35`), cùng các trần quần thể (`live.prey_cap`, `live.strict_prey_cap`). Cái chưa có là khoang dự trữ sinh sản riêng kiểu DEB ($E_R$) và thời gian nghỉ hồi phục (*refractory cooldown*).
2. **Ghi chú về cỡ hiệu ứng trong [`RESEARCH_SYNTHESIS_2026-11-13.md` §2](RESEARCH_SYNTHESIS_2026-11-13.md) (`Đã kiểm` + `Suy luận toán học`):**
   - Bản tổng hợp cũ viết: kiểm định không tham số "Kruskal–Wallis, rồi Mann–Whitney hoặc Wilcoxon", hiệu chỉnh Holm–Bonferroni, báo cỡ hiệu ứng Cliff's delta; dòng kế tiếp ghi thiết kế ghép cặp theo seed. Nó **không** ghép cụ thể bộ ba "Paired Wilcoxon + Holm + Cliff's delta". Cliff's delta là cỡ hiệu ứng đúng cặp với Mann–Whitney (hai mẫu độc lập).
   - Căng thẳng chỉ xuất hiện khi dùng $\delta$ **cùng với** thiết kế ghép cặp theo seed: $\delta$ so sánh chéo mọi cặp không ghép, bỏ qua tương quan nội seed ($\rho$). Với dữ liệu ghép cặp, nên dùng **tương quan hạng–nhị phân ghép cặp ($r_c$)** đi kèm **giả trung vị Hodges–Lehmann ($\hat{\Delta}_{\text{HL}}$) + khoảng tin cậy BCa 95%** và **tỷ lệ đồng thuận theo seed**. Trên thực tế Cliff's delta chưa từng vào bản đăng ký nào: E3 dùng `paired_mean_delta` và `d_z`.
3. **Liên kết hợp đồng liên quan:** Yêu cầu `QD-ISLAND` nằm tại [`2026-11-19-feature-island-evolution-observatory.md`](../ai/requirements/2026-11-19-feature-island-evolution-observatory.md) và khung thực nghiệm tiến hoá nằm tại [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md).

---

## 1. Sinh thái học khả năng sống sót trong mô phỏng năng lượng đóng

### 1.1. Giải phẫu sụp đổ boom–bust: Nghịch lý làm giàu, phân nhánh Hopf và bẫy atto-fox

> **Điều đã đo trên Anima (nguồn: [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) §3.00, hàng "4. Vòng khoa học đầu tiên").** Sụp đổ của SEL2-P **không** khớp với bức tranh nghịch lý làm giàu (cỏ bị gặm cạn toàn cục rồi quần thể sụp theo):
>
> - Smoke SEL2-P (#170): quần thể sáng lập bùng nổ rồi **chết đói trong khi đồng cỏ còn đầy 92–98%** ([ghi chú smoke](../ai/planning/2026-11-19-sel2-p-smoke.md)).
> - Chẩn đoán DIAG-SEL2: sinh hàng loạt ở tick 1, gặm mù làm trụi cỏ **cục bộ** trong khi bản đồ vẫn đầy, và chi phí hoạt động CPG thay đổi theo seed.
> - S9: khát gây 71–80% số cái chết ở cả bốn ô hệ số 0,5 và 0,7 ([kết quả S9](../ai/planning/2026-11-18-s9-death-causes.md)); S12 (`live.lake_layout`, rải hồ trên lưới hai chiều) giảm tuyệt chủng ở hệ số 0,7 từ 4/6 xuống 1/6 seed ([kết quả S12](../ai/planning/2026-11-18-s12-lake-layout.md)).
> - Các khoá opt-in đã có nhắm đúng các cơ chế đó: `live.founder_layout` (#186, phân bố cá thể sáng lập), `live.strict_prey_cap` (#177, sửa F-052), `live.lake_layout` (S12, #135), `live.grass_steering` (#182) và `live.plant_reserve` (S10, #124).
>
> Phần còn lại của mục này (Rosenzweig–MacArthur, nghịch lý làm giàu, atto-fox) là **`Suy luận`** từ lý thuyết ngoài: một cơ chế có thể góp phần khi các nguyên nhân đã đo ở trên được gỡ, **không** phải lời giải thích đã kiểm cho sụp đổ của SEL2-P. Khảo sát VIAB-1 là nơi kiểm nó.

Trong các mô phỏng sự sống nhân tạo (ALife) bảo toàn chặt tổng năng lượng:

$$
E_{\text{agents}} + E_{\text{grass}} + E_{\text{detritus}} = E_{\text{tot}} = \text{hằng số}
$$

một nhân quả thường được nêu trong lý thuyết (`Suy luận`, chưa kiểm trên Anima) cho tuyệt chủng hàng loạt không phải do tổng năng lượng trung bình quá thấp, mà do bất ổn định động lực tạo biên độ dao động rơi xuống dưới ngưỡng 1 cá thể rời rạc. Cần phân biệt rõ về mặt nhiệt động học (`Nguồn ngoài`): hệ sinh thái tự nhiên gần như đóng về vật chất (nguyên tố dinh dưỡng tuần hoàn qua bể mảnh vụn hữu cơ) nhưng mở về dòng năng lượng entropy thấp [19]; khi một thế giới ALife như `SEL2-P`, *Echo* hay *Flow-Lenia* (Plantec et al. 2023, chưa có trong danh mục tham khảo) áp đặt định luật bảo toàn tuyệt đối "token năng lượng", đại lượng bảo toàn đó vận hành về mặt toán học tương đương một nguyên tố dinh dưỡng giới hạn tuần hoàn khép kín trong lý thuyết của DeAngelis & Loreau [20].

Ở cấp độ tương tác tiêu thụ–tài nguyên cục bộ, mô hình Rosenzweig–MacArthur với tăng trưởng logistic của đồng cỏ và đáp ứng chức năng bão hoà Holling loại II mô tả trực tiếp cơ chế sinh dao động:

$$
\frac{dR}{dt} = rR\left(1 - \frac{R}{K}\right) - C\frac{aR}{1 + ahR}, \qquad \frac{dC}{dt} = C\left(e\frac{aR}{1 + ahR} - m\right)
$$

trong đó $R$ là sinh khối tài nguyên (đồng cỏ), $C$ là mật độ cá thể tiêu thụ, $a$ là tốc độ tìm kiếm, $h$ là thời gian xử lý, $b = 1/(ah)$ là hằng số bán bão hoà, $e$ là hiệu suất đồng hoá và $m$ là tốc độ tiêu hao chuyển hoá cộng tử vong. Điểm cân bằng đồng tồn tại nằm tại $R^* = \frac{m}{a(e - mh)}$; ngay khi sức chứa $K$ vượt qua ngưỡng phân nhánh Hopf giải tích $K_{\text{crit}} = b + 2R^*$, cân bằng mất ổn định và chuyển sang chu trình giới hạn (*limit cycle*) — hiện tượng gọi là **nghịch lý làm giàu** (*paradox of enrichment*) [23, 24].

Hệ quả chí tử khi chuyển từ hệ vi phân liên tục sang mô phỏng dựa trên cá thể rời rạc ($N \in \mathbb{Z}_{\ge 0}$) là **vấn đề "atto-fox"** của Mollison [27]: hệ ODE liên tục vẫn phục hồi được từ đáy mật độ cỡ $10^{-12}$, nhưng trong mô phỏng rời rạc, bất kỳ đáy chu kỳ nào chạm dưới 1 cá thể nguyên vẹn đều rơi vào trạng thái hấp thụ $N = 0$ (tuyệt chủng vĩnh viễn). Trong phân tích số minh hoạ từ báo cáo ngoài (`Mô hình ngoài / chưa đo trên Anima`, với $r = 1{,}0, a = 0{,}2, h = 0{,}2, e = 0{,}5, m = 0{,}2 \Rightarrow b = 25, R^* = 2{,}174, K_{\text{crit}} = 29{,}348$), khi $K/K_{\text{crit}}$ tăng từ $0{,}68$ ($K=20$) lên $2{,}73$ ($K=80$), đáy $R_{\min}$ giảm từ $2{,}14$ xuống $2{,}77 \times 10^{-12}$ và đáy $C_{\min}$ tụt xuống $9{,}46 \times 10^{-3}$.

Theo lý thuyết (`Suy luận`), trong hệ năng lượng đóng, **tái chế tức thời quá nhanh có thể khuếch đại nghịch lý làm giàu** [20]: nếu năng lượng từ xác chết và chuyển hoá lập tức biến thành cỏ ở tick kế tiếp, mỗi đợt chết hàng loạt lập tức "bón phân" kích hoạt một cơn bùng nổ cỏ mới và đồng bộ hoá chu kỳ boom–bust ngày càng dữ dội. Ngược lại, đưa bể mảnh vụn (*detritus*) có tốc độ khoáng hoá hữu hạn làm bộ đệm trễ pha giữa cái chết và tái sinh cỏ giúp giảm sốc phản hồi mà không làm rò rỉ một đơn vị năng lượng nào.

### 1.2. Năm cơ chế tự điều tiết không gian lận sinh học (`Nguồn ngoài` + `Mô hình ngoài`)

Để ổn định quần thể dưới chế độ khan hiếm trong vòng tuần hoàn bảo toàn tổng năng lượng (Đồng cỏ ↔ Dự trữ cá thể ↔ Mảnh vụn khoáng hoá), năm "van điều tiết sinh thái" cần đặt đúng các điểm chuyển tiếp của dòng năng lượng:

1. **Cơ chế 1 — Đáp ứng chức năng Holling loại III yếu ($q \in [1{,}4;\, 1{,}6]$) hoặc nổi lên không gian [5, 34]:**
   Dạng tổng quát $f(R) = \frac{v_{\max} R^q}{H^q + R^q}$ (hoặc $\frac{a R^q}{1 + ah R^q}$) làm tỷ lệ gặm riêng phần $f(R)/R$ giảm khi $R \to 0$, tự tạo vùng trú ẩn động cho cỏ phục hồi. Uszko et al. (2015) [5] gọi số mũ này là $b$ ($b = 1$: loại II; $b > 1$: loại III), rút ra một điều kiện giải tích nối $b$ với ổn định cục bộ, và thấy loại III tham số hoá từ thực nghiệm ổn định hoá được một số cặp loài nhưng không ổn định hoá được các cặp khác: ngưỡng $b$ cần thiết phụ thuộc các tham số còn lại, không phải một hằng số. Nhận định trong báo cáo ngoài rằng $q \in [1{,}4;\, 1{,}6]$ là tối ưu và $q \ge 2$ làm khoảng bất ổn phình lại là **(chưa xác minh)**. Trong Anima, `holling_type3` hiện cố định $q = 2$ và chỉ dùng cho bắt mồi (§0); một biến thể gặm cỏ với $q$ khai báo là mã mới, phải opt-in qua một khoá `live.*` mà khi vắng thì giữ nguyên từng bit đường cũ, kèm nâng `LIVE_MODEL_VERSION` theo [ADR-0008](../decisions/ADR-0008-world-past-and-time.md) nếu quỹ đạo mặc định đổi. **Không** đổi chế độ gặm mặc định. Cách thay thế chân thực hơn là để Holling III nổi lên vĩ mô từ đáp ứng cục bộ Holling II kết hợp dị thể không gian và di chuyển phụ thuộc mật độ (Morozov 2010, 2013 [34, 35]).
2. **Cơ chế 2 — Ngưỡng bỏ mảng (Giving-Up Density, GUD) theo Định lý Giá trị Biên [29, 37]:**
   Theo Charnov (1976) và Brown (1988), cá thể nên rời mảng ngay khi suất thu hoạch biên tụt xuống bằng tổng chi phí kiếm ăn:

   $$
   H(R_{\text{GUD}}) = C + P + \text{MOC}
   $$

   với $C$ là chi phí chuyển hoá khi khai thác, $P$ là chi phí rủi ro săn mồi (nếu có) và $\text{MOC}$ là chi phí cơ hội kỳ vọng ở mảng khác sau khi tính phí di chuyển. Trong hệ năng lượng đóng, khi $H(R) \le C + \text{MOC}$, ở lại gây lỗ năng lượng ròng nên cá thể chủ động rời đi, để lại sinh khối cỏ nền tự xúc tác. (Theo phân tích ODE minh hoạ ngoài tại $K=80$, chỉ một ngưỡng GUD tương đương 1,2% sức chứa $K$ đã khoá chu trình Holling II vào cân bằng ổn định — **chưa xác minh, chưa đo trên Anima**.) **Trong Anima, cơ chế này không mới:** `live.grass_steering` đã cài một ngưỡng ở-lại-hay-rời-đi (`GRASS_STAY_FRACTION = 0.25`, §0), và thẻ S11 đo lái tới cỏ ra kết quả âm. Việc còn lại, nếu có, là so ngưỡng cố định đó với một ngưỡng theo chi phí ròng $C + \text{MOC}$, chứ không phải thêm GUD. Theo quy luật Kleiber, cá thể thân lớn có GUD cao hơn cá thể nhỏ, mở đường cho phân chia ổ sinh thái tự nhiên [31].
3. **Cơ chế 3 — Điều tiết sinh sản phụ thuộc mật độ qua ngân sách năng lượng động (DEB) và *capital breeding* [6, 40]:**
   Chỉ riêng GUD không cứu được mô phỏng rời rạc nếu cá thể vẫn vận hành kiểu *income breeding* cực đoan (chạm ngưỡng thấp là phân đôi ngay): việc bỏ mảng sớm lại giúp quần thể săn hết các mảng giàu nhanh hơn và chết đói đồng bộ hơn. Khung DEB của Kooijman và triển khai DEB-IBM của Martin et al. (2012) [6] tách nội trạng cá thể thành dự trữ $E$, cấu trúc $V$ và bộ đệm trưởng thành/sinh sản $E_R$ phân bổ theo quy tắc $\kappa$ ($\kappa$ cho duy trì/tăng trưởng, $1 - \kappa$ cho trưởng thành/sinh sản). Áp dụng **chiến lược *capital breeding*** — đòi hỏi ngưỡng dự trữ cao (bố mẹ giữ lại đủ vốn sống sau sinh), thời gian nghỉ/thai nghén (*refractory cooldown*) và cấp vốn khởi đầu đủ dày cho con non — tạo phanh sinh sản phụ thuộc mật độ hoàn toàn nội sinh (`Suy luận`). Anima đã có một phần của phanh này: ngưỡng sinh sản `0.65 · energy_target` và dự trữ an toàn `0.30` cho con mồi trong `autonomous_reproduction_system` (§0); thứ còn thiếu là khoang $E_R$ riêng và thời gian nghỉ. Mọi khoang năng lượng mới phải nằm trong sổ EU khép kín, và mọi khoản chi mới đi qua `total_cost` trong `metabolic_decay_system`.
4. **Cơ chế 4 — Cạnh tranh tranh giành (*contest*) thay cho chia đều hỗn loạn (*scramble*) [4, 46]:**
   Dưới *ideal scramble*, toàn bộ tài nguyên mảng $V$ chia đều cho $N$ cá thể ($V/N$); hễ $N > V/m$, toàn bộ $N$ cá thể cùng nhận dưới ngưỡng tối thiểu $m$ và sụp đổ đồng loạt theo đường cong quá bù kiểu Ricker/Hassell. Dưới *contest competition* (phát sinh từ ưu thế kích thước/năng lượng, thứ tự tiếp cận hay lãnh thổ cục bộ), nhóm cá thể chiếm ưu thế vẫn nhận đủ khẩu phần tối đa, chuyển động lực sang đường cong có trần đơn điệu kiểu Beverton–Holt [4]. Trong mã hoá ALife, thay chia đồng đều đồng thời trên một ô bằng gặm tuần tự theo thứ bậc ưu thế (hoặc thêm số hạng cản trở $-cP^2$ / đáp ứng Beddington–DeAngelis).
5. **Cơ chế 5 — Quản trị hiệu ứng Allee và cứu hộ liên đảo có kiểm soát [49, 53, 54]:**
   Hiệu ứng Allee mạnh tạo ngưỡng tới hạn $A > 0$ dưới đó quần thể rơi tất định về 0:

   $$
   \frac{dN}{dt} = -rN\left(1 - \frac{N}{A}\right)\left(1 - \frac{N}{K}\right)
   $$

   Ngay cả khi sinh sản vô tính, ngẫu nhiên nhân khẩu học khi $N < 10$ vẫn có thể đẩy đảo về 0 [51]. Hai biện pháp chân thực cho `QD-ISLAND` là: (i) cho phép tụ cụm không gian để mật độ tiếp xúc cục bộ cao hơn mật độ trung bình đảo [54], và (ii) hiệu chỉnh suất di cư liên đảo ở mức trung gian để kích hoạt hiệu ứng cứu hộ (*rescue effect*) mà không đồng bộ hoá dao động toàn quần đảo [55, 58].

| Cơ chế / Quy tắc thiết kế | Phân loại | Tác động lên động lực khả năng sống sót | Rủi ro hoặc giới hạn cần lưu ý |
|---|---|---|---|
| **GUD bỏ mảng** ($H(R) \le C + \text{MOC}$) [29] | Chân thực sinh học | Tạo nơi trú ẩn đồng cỏ tự nhiên nhờ chi phí kiếm ăn ròng | **Đã có** dạng ngưỡng cố định (`live.grass_steering`); S11 đo âm. Phản tác dụng trong IBM rời rạc nếu đi kèm sinh sản tức thời không có phanh (*Mô hình ngoài*) |
| **Holling loại III yếu** ($q$ khai báo) [5, 34] | Chân thực sinh học | Giảm tỷ lệ gặm ở mật độ thấp, ổn định cân bằng đồng tồn tại | Ngưỡng $q$ ổn định phụ thuộc tham số [5]; dải $q \in [1{,}4;\, 1{,}6]$ **(chưa xác minh)**. `holling_type3` hiện cố định $q = 2$; biến thể mới phải opt-in |
| **Dự trữ DEB + ngưỡng sinh sản + thời gian nghỉ** [6] | Chân thực sinh học | Chuyển sang *capital breeding*; tạo phanh sinh sản phụ thuộc mật độ | Ngưỡng quá cao so với $E_{\text{tot}}$ làm chậm luân chuyển thế hệ dưới mục tiêu 50–100 thế hệ |
| **Cạnh tranh tranh giành (*Contest*) trên ô** [4, 46] | Chân thực sinh học | Ngăn chết đói đồng loạt; ổn định biên độ chu kỳ ở mật độ vừa và khan hiếm | Ưu thế mờ dần khi $N_{\min} \approx 3$ do nhiễu nhân khẩu học thuần tuý lấn át (*Mô hình ngoài*) |
| **Bể mảnh vụn (*Detritus*) khoáng hoá chậm** [20] | Chân thực sinh học | Lệch pha giữa tử vong và tái sinh cỏ, làm dịu vòng phản hồi nội sinh | Khoáng hoá quá chậm có thể khoá chặt năng lượng chết ở đầu mô phỏng |
| **Sàn bất tử / Bơm năng lượng / Thay thế cá thể chết** [59] | **Gian lận sinh học** | Cưỡng bức $N_{\min} \ge 1$ giả tạo | Phá vỡ định luật bảo toàn năng lượng, làm lệch chọn lọc sống sót và vô hiệu tiêu chí MC |

> **Con số từ nguồn đã bị loại (`chưa xác minh`, không được dùng làm tiêu chí cổng):** Trong mô phỏng đối chứng năng lượng đóng 24 seed của báo cáo ngoài ($E \in \{900, 450, 280\}$), phối hợp đủ 5 van nâng tỷ lệ sống sót 46–50 thế hệ từ 0% lên 100% ở cả 3 mức với sai số bảo toàn năng lượng $\le 7{,}2 \times 10^{-11}$; tuy nhiên tại mức cực khan hiếm ($E=280$, $N_{\min} \approx 3{,}0\text{--}3{,}3$), nhiễu nhân khẩu học vẫn cao. Các mức $N_{\text{mean}} \ge 18\text{--}25$, $N_{\min} \ge 8\text{--}10$ cá thể/đảo và tuyệt chủng $\le 5\%\text{--}10\%$ mà báo cáo ngoài đề xuất chỉ là giả thuyết để VIAB-1 kiểm; mức thật phải lấy từ số đo trên Anima.

---

## 2. Phương trình Price cho thế hệ chồng lấp, di cư liên đảo và chọn lọc đa cấp (`Nguồn ngoài` + `Suy luận`)

### 2.1. Đồng nhất thức Price chính xác 5 thành phần cho đảo mở có thế hệ chồng lấp

Phương trình Price cổ điển (Price 1970) $\bar{w}\Delta\bar{z} = \text{Cov}(w,z) + \mathbb{E}(w\Delta z)$ đòi hỏi quần thể đóng và thế hệ rời rạc không chồng lấp [62, 63]. Trong `QD-ISLAND`, cả ba điều kiện bị vi phạm cùng lúc: cá thể sống sót song song với con cháu (thế hệ chồng lấp), kiểu hình biến đổi theo thời gian (độ dẻo / lão hoá), và các đảo trao đổi cá thể qua di cư.

Hợp nhất khung kế toán sinh–tử thế hệ chồng lấp [7], cấu trúc tuổi của Coulson & Tuljapurkar (2008) [61], và phương trình Price cho quần thể mở của Kerr & Godfrey-Smith (2009) / Brown & Field (tiền ấn phẩm bioRxiv 2021) [9, 64], xét hai lát cắt kiểm kê $t$ (quần thể tổ tiên $A$, quy mô $n_A$) và $t+\Delta t$ (quần thể hậu duệ $D$, quy mô $n_D$):

- Với mỗi cư dân $i \in A$ có tính trạng $z_i$, đặt $s_i \in \{0, 1\}$ là biến chỉ thị $i$ **vừa sống sót vừa còn lưu trú trên đảo** tại $t+\Delta t$ (tách rạch ròi $1 - s_i = d_i + e_{i,\text{self}}$: chết vs di cư đi); nếu $s_i = 1$, biến đổi nội cá thể là $\Delta z_{i,\text{onto}} = z'_i - z_i$.
- Đặt $b_i$ là số con bản địa sống sót và còn trên đảo của $i$ tại $t+\Delta t$ (sinh sản hữu tính lưỡng bội gán $1/2$ mỗi con cho mỗi cha/mẹ để bảo toàn tổng liên kết tổ tiên–hậu duệ [10]), với thiên lệch truyền thụ $\Delta z_{i,\text{birth}} = \bar{y}_i - z_i$.
- Đặt thể lực bản địa cục bộ là $w_i = s_i + b_i$ với trung bình $\bar{w}$.
- Trên đảo hậu duệ $j \in D$, gán giá trị tổ tiên cục bộ (*ancestry value*) $a_j = 1$ nếu $j$ là bản địa (sống sót hoặc sinh trên đảo) và $a_j = 0$ nếu $j$ là nhập cư ($m_{\text{in}}$ cá thể, chiếm tỷ lệ $p_{\text{imm}} = m_{\text{in}}/n_D = 1 - \bar{a}_D$).

Đồng nhất thức đại số chính xác tuyệt đối (không dư số) tách biến thiên $\Delta\bar{z} = \bar{z}_D - \bar{z}_A$ thành đúng **5 thành phần**:

$$
\Delta\bar{z} = \underbrace{\frac{\text{Cov}_A(s, z)}{\bar{w}}}_{\text{1. Chọn lọc sống sót + lưu trú}} + \underbrace{\frac{\text{Cov}_A(b, z)}{\bar{w}}}_{\text{2. Chọn lọc sinh sản}} + \underbrace{\frac{\mathbb{E}_A(s\,\Delta z_{\text{onto}})}{\bar{w}}}_{\text{3. Biến đổi cá thể (độ dẻo / lão hoá)}} + \underbrace{\frac{\mathbb{E}_A(b\,\Delta z_{\text{birth}})}{\bar{w}}}_{\text{4. Thiên lệch truyền thụ (đột biến / tái tổ hợp)}} \underbrace{- \frac{\text{Cov}_D(a, z)}{\bar{a}_D}}_{\text{5. Dòng nhập cư}}
$$

Trong đó số hạng dòng nhập cư rút gọn trực quan thành:

$$
-\frac{\text{Cov}_D(a, z)}{\bar{a}_D} = p_{\text{imm}}(\bar{z}_{\text{imm}} - \bar{z}_{\text{nat}})
$$

với $\bar{z}_{\text{imm}}$ là trung bình nhóm nhập cư và $\bar{z}_{\text{nat}}$ là trung bình nhóm bản địa tại $t+\Delta t$. Nếu nhân hai vế với tỷ số tăng trưởng tổng của đảo $\lambda_D = n_D / n_A = \bar{w}/\bar{a}_D$ (dạng dòng chảy không chia $\bar{w}$, tránh kỳ dị tuyệt chủng), đồng nhất thức tương đương viết là:

$$
\lambda_D\Delta\bar{z} = \text{Cov}_A(s, z) + \text{Cov}_A(b, z) + \mathbb{E}_A(s\,\Delta z_{\text{onto}}) + \mathbb{E}_A(b\,\Delta z_{\text{birth}}) + \frac{m_{\text{in}}}{n_A}(\bar{z}_{\text{imm}} - \bar{z}_A)
$$

> **Cảnh báo đại số:** Ở dạng chuẩn hoá theo $\bar{w}$, dòng nhập cư dùng hiệu $(\bar{z}_{\text{imm}} - \bar{z}_{\text{nat}})$; ở dạng nhân $\lambda_D$, dòng nhập cư dùng hiệu $(\bar{z}_{\text{imm}} - \bar{z}_A)$. Ghép nhầm mẫu số sẽ phá vỡ đồng nhất thức.
>
> **Khử nhiễu cấu trúc tuổi / giai đoạn năng lượng [67, 68]:** Ngay cả khi đồng nhất thức trên cân bằng tới sai số máy, $\Delta\bar{z}$ không trọng số vẫn dao động mạnh sau sốc mật độ do biến đổi cơ cấu tuổi $\sum_a [\Delta c(a, t)]\bar{z}(a, t)$. Khi cần phân tích chọn lọc thuần tuý, áp dụng khung giá trị sinh sản của Lion (2018) / Grafen (2015) ($\frac{d\bar{z}}{dt} = \mathbf{v}^\top \mathbf{C}\mathbf{f} + O(\epsilon^3)$) bằng cách gán trọng số giá trị sinh sản cơ bản $v_k$ của từng lớp tuổi/trạng thái.

### 2.2. Dạng thời gian liên tục và bất đối xứng nhiễu nhân khẩu học khi $N$ nhỏ

Trên lưới tick mịn, phương trình Price thời gian liên tục của Day & Gandon (2006) và mở rộng cho nhiễu nhân khẩu học hữu hạn $N$ của Day, Parsons, Lambert & Gandon (2020) [72, 73] viết là (dạng chính xác và số hiệu phương trình **chưa xác minh** với bài gốc):

$$
\frac{d\bar{z}}{dt} = \text{Cov}(z, b - d) + \mathbb{E}[b\,\Delta z_{\text{mut}}] + \mathbb{E}\left[\frac{dz^s}{dt}\right]
$$

$$
\mathbb{E}\left[\frac{d\bar{z}}{dt}\right] = \text{Cov}(z, b)\left(1 - \frac{1}{N}\right) - \text{Cov}(z, d)\left(1 + \frac{1}{N}\right) + \mathbb{E}[b\,\Delta z_{\text{mut}}]\left(1 - \frac{1}{N}\right) + \mathbb{E}\left[\frac{dz^s}{dt}\right]
$$

Trên đảo khan hiếm có $N=10$ cá thể, hệ số $(1 - 1/N)$ làm suy giảm 10% chọn lọc sinh sản và thiên lệch đột biến, trong khi $(1 + 1/N)$ khuếch đại thêm 10% chọn lọc tử vong — giải thích về mặt toán học vì sao gen tiết kiệm chuyển hoá thường thắng thế mạnh hơn dự đoán tất định khi mật độ thấp [72].

### 2.3. Phân rã MLS1 toàn quần đảo, nghịch lý Simpson và phân tích ngữ cảnh [8, 76, 78]

Nhờ tính đệ quy của phương trình Price [10], trên toàn quần đảo gồm $K$ đảo (đóng về nhân khẩu ở cấp toàn cục, cá thể chết trên đường di cư được hạch toán vào $1 - s_i$ của đảo xuất phát), tổng biến thiên chọn lọc tách chính xác thành chọn lọc nội đảo và chọn lọc liên đảo theo khuôn khổ **MLS1** (Okasha 2004, 2006):

$$
\bar{w}\,\Delta_s\bar{z} = \underbrace{\sum_{k=1}^K q_k\,\text{Cov}_k(w_{kj}, z_{kj})}_{\text{Chọn lọc nội đảo } \mathbb{E}_k[\text{Cov}_k(w,z)]} + \underbrace{\sum_{k=1}^K q_k(W_k - \bar{w})(Z_k - \bar{z})}_{\text{Chọn lọc liên đảo } \text{Cov}_k(W_k, Z_k)}
$$

với $q_k = n_k / \sum_{\ell=1}^K n_\ell$. Hai nguyên tắc bắt buộc cho `QD-ISLAND`:

1. **Bắt buộc trọng số hoá theo quy mô đảo $q_k$:** Bình quân không trọng số khi $n_k$ lệch nhau làm sai lệch đồng nhất thức Price [10].
2. **Tách nghịch lý Simpson khỏi sản phẩm phụ xuyên cấp (*cross-level by-product*) bằng phân tích ngữ cảnh:** Trong MLS1, thành phần liên đảo $\text{Cov}_k(W_k, Z_k) \neq 0$ vẫn xuất hiện ngay cả với tính trạng phi xã hội ($w_{kj} = c_1 z_{kj}$) nếu các đảo khác nhau về $Z_k$ [8]. Do đó, để xác nhận chọn lọc cấp đảo thực thụ trong `QD-ISLAND`, phải chạy song hành hồi quy **phân tích ngữ cảnh** (*contextual analysis* [78]):

   $$
   w_{kj} = \beta_1 z_{kj} + \beta_2 Z_k + \varepsilon_{kj}
   $$

   và kiểm định $\beta_2 \neq 0$ (tác động của đặc trưng đảo $Z_k$ lên thể lực cá thể sau khi kiểm soát $z_{kj}$).

### 2.4. Năm bẫy toán học chí tử khi áp công thức Price vào ALife

| Bẫy toán học | Nguyên nhân gốc rễ | Hệ quả / minh hoạ định lượng | Quy tắc cài đặt chuẩn trong `QD-ISLAND` |
|---|---|---|---|
| **1. Bỏ cá thể sống sót khỏi thể lực** [7] | Mượn công thức thế hệ rời rạc chỉ dùng $w_i = b_i$ | Xoá sổ $\text{Cov}(s,z)/\bar{w}$ và trộn $\Delta z_{\text{onto}}$ vào sai lệch truyền thụ | Đặt $w_i = s_i + b_i$; tách 4 số hạng bản địa ($s$, $b$, $\Delta z_{\text{onto}}$, $\Delta z_{\text{birth}}$); dùng trọng số $v_k$ khi cơ cấu tuổi dao động mạnh [67] |
| **2. Cửa sổ kiểm kê $\Delta t$ quá dài** | Áp công thức rời rạc trên $\Delta t$ lớn so với thời gian thế hệ $T_{\text{gen}}$ | Trong kiểm chứng hàm mũ ngoài ($r \in \{0{,}05; 0{,}35; 0{,}70\}$), $\Delta t=2{,}0$ lệch $-5{,}1\%$ và $\Delta t=5{,}0$ lệch $-26{,}8\%$ so với $\text{Cov}(r, z)$ tức thời | Đặt $\Delta t \le 0{,}1\,T_{\text{gen}}$, hoặc ghi nhật ký sự kiện sinh/tử/di cư và cộng dồn chính xác theo tick ($\Delta t = 1$) |
| **3. Nhiễu đồng biến trạng thái năng lượng** [80, 81] | Tính $\text{Cov}(w,z)$ hoặc $h^2 S$ trên kiểu hình biểu hiện $z = g + e$ phụ thuộc năng lượng/ô giàu | Hiện tượng chim *Malurus cyaneus* [81]: trong kiểm chứng ngoài với $h^2 = 0{,}239$ và chọn lọc thực trên gen âm ($-0{,}2038$), $\text{Cov}(w,z)/\bar{w} = +1{,}4979$ và $h^2 S = +0{,}3583$ đảo ngược dấu | Tính **Robertson–Price trực tiếp trên giá trị di truyền $g_i$**: $\Delta\bar{g}_{\text{sel}} = \text{Cov}(w, g)/\bar{w}$, chạy song song với $z_i$ để tách minh bạch $\text{Cov}(w, e)/\bar{w}$ [70] |
| **4. Bỏ sót nhập cư, sai mẫu số `ddof=1` và thiếu trọng số $q_k$** | Dùng `np.cov` (chia $N-1$), bỏ nhóm $a_j=0$, hoặc bình quân không trọng số đảo | Bỏ nhập cư $-\text{Cov}_D(a,z)/\bar{a}_D$ đảo dấu tiến hoá; `ddof=1` gây thiên lệch $N/(N-1)$ (+11,1% khi $N=10$) (*Mô hình ngoài*) | Luôn dùng hiệp phương sai tổng thể (`ddof=0`); hạch toán đủ số hạng thứ 5; trọng số $q_k = n_k/N$; unit test đóng đồng nhất thức sai số $< 10^{-12}$ |
| **5. Điểm kỳ dị tuyệt chủng ($\bar{w} \to 0$) & nhầm hồi cứu với tiên nghiệm** [85] | Chia cho $\bar{w}=0$ khi đảo diệt vong; đồng nhất hiệp phương sai thực nghiệm 1 seed với chọn lọc tất định | Không xác định khi $\bar{w}=0$; trôi dạt phi tuyến của thể lực tương tác với chọn lọc (Rice 2008 [85]) | Phân tích dòng chưa chuẩn hoá $\bar{w}\Delta\bar{z}$ hoặc đặt điều kiện tường minh $n_D > 0$ trong khung hai phần; lấy trung bình đa seed để ước lượng chọn lọc hệ thống |

---

## 3. Thiết kế thống kê, phân tích lực mô phỏng và chống giả lặp cho `QD-ISLAND`

### 3.1. Xác định đơn vị thực nghiệm và triệt tiêu giả lặp (*pseudoreplication*) [11, 88, 90]

Theo Hurlbert (1984) và Lazic (2022) [88], phân tách ba thực thể bằng phép thử: *"Hai thực thể có thể được gán độc lập vào hai điều kiện xử lý khác nhau hay không?"*

- Cá thể trên cùng một đảo và từng đảo bên trong cùng một lượt chạy quần đảo **chỉ là đơn vị quan sát** vì di cư liên đảo gắn kết chặt chẽ mật độ và phả hệ dưới cùng một cấu hình xử lý [89]. Mức phình phương sai do hiệu ứng thiết kế là $\text{DE} = 1 + (m - 1)\,\text{ICC}$.
- **Đơn vị thực nghiệm (Experimental Unit)** là **một lượt chạy quần đảo (*archipelago-run*) ứng với một cặp `(seed, cell)`**, và **Đơn vị phân khối (Blocking Unit)** là **Master Seed** lặp qua cả 8 ô của thiết kế `2×2×2`.
- *Kiểm chứng mô phỏng ngoài — nguồn đã bị loại, **chưa xác minh** (16 seed, 8 đảo/lượt chạy, 25 cá thể/đảo, $\text{ICC}_{\text{đảo}} = 0{,}820$, $\text{ICC}_{\text{cá thể}} = 0{,}667$):* phân tích ngây thơ cấp cá thể đẩy sai số loại I lên 82,9% ($n=3.200$/nhánh), cấp đảo lên 36,4% ($n=128$), còn t-test hai mẫu bỏ qua ghép cặp lại quá bảo thủ (1,7%); chỉ ghép cặp theo seed ở cấp lượt chạy quần đảo ($n=16$ cặp) mới giữ đúng ~5,8%.
- **Quy tắc bắt buộc:** Mọi chỉ số cấp quần đảo (Robertson–Price $\Delta\bar{g}$, tỷ trọng MLS1, RMST) phải tóm gọn thành **đúng 1 giá trị vô hướng cho mỗi `(seed, cell)`** trước khi suy luận. (Chỉ khi kiểm tra chênh lệch đảo giàu vs đảo nghèo bên trong cùng lượt chạy mới dùng mô hình hỗn hợp lồng nhau `(1 | seed / run_id / island_id)` với hiệu chỉnh bậc tự do Kenward–Roger neo theo số lượt chạy và seed).

### 3.2. Đối chiếu các khung suy luận cho `2×2×2` ghép cặp theo seed (`Nguồn ngoài` + `Mô hình ngoài`)

Với ma trận dữ liệu $N_{\text{seed}} \times 8$ cân đối theo seed, biến đổi trực giao sạch nhất là **7 tương phản Walsh trực giao nội seed** ($a, b, c \in \{-1, +1\}$):

$$
d_{i,A} = \frac{1}{4}\sum_{b,c}(Y_{i,+1,b,c} - Y_{i,-1,b,c}), \qquad
d_{i,AB} = \frac{1}{2}\sum_c\big[(Y_{i,+1,+1,c} - Y_{i,-1,+1,c}) - (Y_{i,+1,-1,c} - Y_{i,-1,-1,c})\big], \qquad
d_{i,ABC} = \sum_{a,b,c}(a\cdot b\cdot c)\,Y_{i,a,b,c}
$$

(chính xác cho cả yếu tố chính $A, B, C$, tương tác bậc hai $AB, AC, BC$, và bậc ba $ABC$). Kiểm định $t$ một mẫu trên từng vector $d_{i,c}$ tương đương toán học với LMM cấu trúc tối đa có đầy đủ độ dốc ngẫu nhiên cho từng tương phản (mà không vấp lỗi hội tụ kỳ dị của `lme4`), đồng thời cho phép áp dụng trực tiếp Paired Wilcoxon và hoán vị đổi dấu nội seed.

| Phương pháp suy luận (`2×2×2` ghép cặp theo seed) | Kiểm soát Type I dưới lệch / boom–bust | Kiểm soát Type I dưới dị phương sai / random slope | Xử lý khuyết ô ngẫu nhiên (MAR) | Khuyến nghị vai trò trong `QD-ISLAND` |
|---|---|---|---|---|
| **LMM chỉ Random Intercept `(1 \| seed)`** [12] | Tốt (4,6%–5,0%) | **THẤT BẠI** (21,0%–41,0% với $\sigma_{\text{slope}, A} > 0$; 1,3% quá bảo thủ khi dị phương sai $A\times B$) | Tốt | **Loại bỏ** làm mô hình chính (bẫy thiếu random slope của Barr et al. 2013) |
| **Tương phản Walsh + Paired $t$ (= Maximal LMM)** [95] | Bền (4,7%–5,6%, khớp Schielzeth et al. 2020) | Chính xác (4,5%–5,4%) | Cần khối đủ (hoặc LMM phương sai riêng theo ô) | **Chính (song hành)** cho biến liên tục không ngoại lai cực đoan |
| **Tương phản Walsh + Paired Wilcoxon** | Chính xác (4,5%–5,3%), kháng ngoại lai mạnh | Vững (4,0%–5,3%) | Hỏng nếu nhiều ô tuyệt chủng (Mục 3.4) | **Chính (phi tham số)** khi tỷ lệ tuyệt chủng $\le 10\%$ |
| **Hoán vị đổi dấu nội seed / `permuco` + Westfall–Young `maxT`** [97, 99] | Chính xác (4,8%–5,7%), phi tham số | Chính xác (5,2%) | Hỗ trợ mất cân bằng qua `permuco` | **Chuẩn vàng khẳng định** và tối ưu lực khi các chỉ số tương quan |
| **Aligned Rank Transform (ART)** [13, 94, 100] | Hỏng với dữ liệu rời rạc / lệch mạnh | **THẤT BẠI** (7,3% ở `2×2×2`; tiệm cận ~100% khi hiệu ứng chính lớn) | Thất bại | **Cấm dùng** |

### 3.3. Kiểm toán đa kiểm định ẩn và bộ cỡ hiệu ứng ghép cặp thay Cliff's delta

1. **Đa kiểm định ẩn (Hidden Multiplicity, Cramer et al. 2016 [15, 101]):**
   Một ANOVA `2×2×2` thực hiện 7 kiểm định đồng thời ($A, B, C, AB, AC, BC, ABC$), đẩy FWER không hiệu chỉnh lên $1 - 0{,}95^7 = 30{,}2\%$. Nếu gộp 7 tương phản × nhiều đầu ra ($m=21$), ngưỡng Bonferroni/Holm bước đầu sụp về quá thấp. Cần khoá trước hai kỷ luật trong preregistration:
   - Tách **Họ Khẳng định Chính (*Confirmatory Primary Family*)** gồm đúng $m = 2\text{--}3$ tương phản cốt lõi gắn giả thuyết (giữ ngưỡng bước đầu Holm ở $\alpha/3 = 0{,}0167$), tách khỏi **Họ Khám phá (*Exploratory Family*)** kiểm soát theo Benjamini–Hochberg FDR;
   - Khi kiểm định đồng thời nhiều biến tương quan trên cùng tập seed, ưu tiên **Westfall–Young step-down `maxT/minP`** trên cùng ma trận hoán vị đổi dấu nội seed để tận dụng tương quan giữa các thống kê kiểm định [99].
2. **Cỡ hiệu ứng cho dữ liệu ghép cặp theo seed — bộ ba thay cho Cliff's delta [14, 111]:**
   Cliff's delta $\delta = \frac{2U}{n_x n_y} - 1$ là cỡ hiệu ứng cho **hai mẫu độc lập** tương ứng Mann–Whitney U, nên đúng khi so hai nhóm độc lập (như bản tổng hợp 2026-11-13 viết), nhưng bỏ qua ghép cặp khi thiết kế ghép theo seed. Trong minh hoạ số từ báo cáo ngoài (chưa xác minh) ($N=32$ seed, dịch chuyển +0,600, tương quan $\rho = 0{,}926$), Paired Wilcoxon cho $p = 0{,}000035$, trong khi $\delta$ chỉ báo $+0{,}199$ (khớp Mann–Whitney không nghĩa $p = 0{,}1729$). Thay $\delta$ bằng bộ ba:
   - **Tương quan hạng–nhị phân ghép cặp (*matched-pairs rank-biserial correlation*, Kerby 2014 [111]):**
     $$
     r_c = \frac{R_+ - R_-}{R_+ + R_-}
     $$
     với $R_+$, $R_-$ là tổng hạng dương và âm của $|d_i|$. Báo cáo ngoài kèm ngưỡng $0{,}11 / 0{,}28 / 0{,}43$ (nhỏ/vừa/lớn); các mức này trùng với ngưỡng Vargha–Delaney cho **hai mẫu độc lập** đổi sang thang $\delta$, nên **không dùng** cho $r_c$ ghép cặp. Ngưỡng vật chất của $r_c$ phải khai trước trong bản đăng ký, không mượn từ quy ước hai mẫu.
   - **Giả trung vị Hodges–Lehmann ghép cặp ($\hat{\Delta}_{\text{HL}}$):** trung vị của mọi trung bình cặp Walsh $(d_i + d_j)/2$ ($i \le j$), báo cáo kèm **khoảng tin cậy bootstrap BCa 95%** trên đúng đơn vị khoa học gốc.
   - **Tỷ lệ đồng thuận theo seed:** phần trăm seed có $d_i > 0$.

### 3.4. Xử lý tuyệt chủng chế độ khan hiếm (*Truncation-by-Death*) cho `E3-X` và `QD-ISLAND` [2, 3, 16, 18]

Khi một lượt chạy diệt vong trước chân trời đánh giá $\tau$, các chỉ số tiến hoá tại $\tau$ **không tồn tại về mặt định nghĩa** (không phải khuyết ngẫu nhiên MAR). So sánh riêng nhóm sống sót tạo bẫy biến va chạm $A \to S \leftarrow U \to Y$ (mô phỏng ngoài, chưa xác minh, báo dương tính giả tới 68,2% khi $p_{\text{ext}}=0{,}5$), gán 0 làm méo phương sai, còn xoá nguyên block theo seed làm sụp đổ tỷ lệ khối hoàn chỉnh (chỉ còn 6,2%–20,0% khối đủ 4 ô khan hiếm nếu $p_{\text{ext}} = 0{,}5$, và vẫn mất 44%–59% ở $p_{\text{ext}} = 0{,}2$).

Do đó, sau khi VIAB-1 đã hạ được tuyệt chủng xuống mức đo được và chấp nhận được, có thể áp dụng **Kiến trúc Hai Phần (Two-Part) + SACE + Worst-Rank**:

1. **Phần 1 — Khả năng sống sót trên 100% seed bằng RMST (Restricted Mean Survival Time) [16, 115, 118]:**
   $$\mu(\tau) = \int_0^\tau S(t)\,dt$$
   tính trên thời gian sống cụt $T_{i,c} = \min(T_{\text{ext}}, \tau)$ với $\tau$ khoá trước (ví dụ $\tau = 100$ thế hệ). Không cần giả định nguy cơ tỷ lệ Cox và cho phép chạy thẳng tương phản Walsh trên $\mu(\tau)$.
2. **Phần 2a — Tiến hoá có điều kiện qua SACE ghép cặp CRN trên tầng luôn sống sót ($G = LL$) [2, 3]:**
   Nhờ chạy ghép cặp cùng seed (CRN), quan sát trực tiếp được tập seed thuộc tầng $LL$ (sống ở cả hai nhánh so sánh). Hiệu số nội seed trên tầng $LL$ triệt tiêu biến ẩn $U$. Bắt buộc báo cáo tỷ lệ tầng $\hat{\pi}_{LL}$ và không ngoại suy nếu $\hat{\pi}_{LL} < 0{,}85$.
3. **Phần 2b — Phân tích độ nhạy tổng hợp bằng điểm Hạng Xấu nhất (*Worst-Rank Score*, Lachin 1999) [18, 114]:**
   Xếp hạng mọi lượt chạy tuyệt chủng thấp hơn nghiêm ngặt mọi lượt chạy sống sót, phá hoà giữa các lượt chạy chết bằng thời điểm tuyệt chủng $T_{\text{ext}}$ (chết sớm hạng thấp hơn), rồi chạy Paired Wilcoxon hoặc hoán vị.

### 3.5. Phân tích lực mô phỏng theo ADEMP-PreReg, SESOI và TOST [17, 122, 130, 132]

- **Cô lập luồng RNG để giữ tương quan CRN ($\rho$) [123, 125]:** Trong hệ ALife hỗn loạn, một RNG dùng chung toàn cục sẽ mất đồng bộ ngay khi một điều kiện xử lý làm dịch chuyển thứ tự sự kiện sinh–tử, kéo $\rho$ về $0$ và làm mất lợi thế giảm phương sai $\text{Var}(X-Y) = \text{Var}(X) + \text{Var}(Y) - 2\text{Cov}(X,Y)$. Bắt buộc **tách luồng RNG độc lập theo thành phần** (khởi tạo địa hình/gen, nhiễu môi trường từng đảo, di cư, đột biến) và chạy độ nhạy lực trên $\rho \in \{0{,}0;\, 0{,}4;\, 0{,}7\}$.
- **Phương sai không phải là lực kiểm định [128, 129] (`Suy luận toán học`):** Với thang tương phản ở §3.2, $\|\mathbf{c}_A\|^2 = 0{,}5$, $\|\mathbf{c}_{AB}\|^2 = 2{,}0$, $\|\mathbf{c}_{ABC}\|^2 = 8{,}0$; tỷ số phương sai 4× và 16× chỉ là hệ quả của cách chọn thang, vì tín hiệu trên mỗi thang cũng khác. Ví dụ tương tác suy giảm hoàn toàn (tín hiệu $d$ ở $B=+1$, $0$ ở $B=-1$): ước lượng $A$ là $d/2$ với phương sai $0{,}5\sigma^2$, ước lượng $AB$ là $d$ với phương sai $2\sigma^2$, nên hai thống kê kiểm định **bằng nhau** ($d/(\sqrt{2}\sigma)$) — không có hình phạt 4× về lực trong kịch bản này. Quy tắc "16×" của Gelman giả định tương tác chỉ bằng **một nửa** hiệu ứng chính. Lực cho từng tương phản phải tính theo cỡ hiệu ứng giả thuyết trên chính thang của nó. Vẫn nên để $ABC$ trong họ khám phá trừ khi lý thuyết dự đoán nó mạnh.
- **Mức $N_{\text{seed}}$ từ mô phỏng ngoài — nguồn đã bị loại, chưa xác minh, không được dùng để chốt số seed** ($N_{\text{rep}}=3.000$ với Holm $m=3$, $\text{SESOI } d=0{,}50$). Để so: [`QD-ISLAND` §7](../ai/requirements/2026-11-19-feature-island-evolution-observatory.md) dự trù 8 ô × 20 seed và lượt 200.000 tick (khoảng 33 thế hệ ở S13), khoảng 27 giờ trên Dell; số seed cuối cùng tính từ khảo sát thăm dò. Các mức dưới đây gấp 1,6–5 lần số seed đó:
  - Chỉ kiểm định hiệu ứng chính ($d=0{,}50$): $N_{\text{seed}} = 24$ đạt ~80,2% ($\rho=0$) đến ~91,9% ($\rho=0{,}4$); báo cáo ngoài đề xuất $N_{\text{seed}} = 32$.
  - Kiểm định cả tương tác bậc hai suy giảm ($d=0{,}50$): ở $\rho = 0{,}4$, báo cáo ngoài đề xuất $N_{\text{seed}} = 48\text{--}64$ ($N=64$ đạt ~81,4% Paired $t$ / ~77,4% Paired Wilcoxon), và $96$ khi $\rho = 0$. Cách định nghĩa $d$ cho tương tác trong mô phỏng đó không rõ (xem mục phương sai và lực ở trên).
- **Cấm lấy cỡ hiệu ứng từ pilot nhỏ + tích hợp TOST [130, 132, 134]:** Tuyệt đối không cắm ước lượng điểm $d$ từ pilot ($n=8\text{--}12$) vào công thức cỡ mẫu (Albers & Lakens 2018 [130]). Thay vào đó, neo **$N_{\text{seed}}$ theo SESOI khoá trước** (ví dụ $|d_{\text{SESOI}}| = 0{,}50$ hoặc $|r_c| = 0{,}30$ hoặc ngưỡng thô Robertson–Price), chỉ lấy tham số nhiễu ($\hat{\sigma}_{\text{slope}}, \hat{\rho}, \widehat{\text{ICC}}, p_{\text{ext}}$) từ **tập seed hiệu chuẩn tách biệt hoàn toàn** (cấm tái sử dụng vào tập khẳng định [119]), và chạy **TOST** với biên $[-\text{SESOI}, +\text{SESOI}]$ (CI 90%) để phân biệt dứt khoát *"tương đương thực tế với 0"* với *"chưa đủ bằng chứng"* [134].
- **Năm trụ cột ADEMP-PreReg [17, 136]:** Khai báo rõ *Aims*, *Data-Generating Mechanism* ($\rho \in \{0{,}0; 0{,}4\}$, $p_{\text{ext}} \in \{0{,}05; 0{,}10\}$), *Estimands* (RMST, Walsh di truyền Robertson–Price, SACE), *Methods* (Walsh + Wilcoxon / sign-flip + Holm $m \le 3$ hoặc Westfall–Young `maxT`), và *Performance Measures* ($\text{MCSE} = \sqrt{\hat{p}(1-\hat{p})/N_{\text{rep}}}$ với $N_{\text{rep}} = 3.000\text{--}5.000$), kèm **cây quyết định dự phòng** khoá trước (ví dụ $p_{\text{ext}} > 15\%$ tự động nâng RMST + SACE + Worst-Rank làm báo cáo chính).

---

## 4. Gợi ý cho các thẻ đã có (VIAB-1, E3b, SEL2-P2, `QD-ISLAND`)

> **Cập nhật 2026-11-24 — các gợi ý dưới đây đã đi tới đâu.** Phần này chỉ ghi trạng thái; bốn gợi ý
> giữ nguyên làm lịch sử.
> - **VIAB-1 đã chạy** ([kết quả](../ai/planning/2026-11-23-viab1-results.md), #215; kiểm lại ở
>   [VIAB-1b](../ai/planning/2026-11-23-viab1b-results.md), #228). Phát hiện chính không nằm trong
>   các cơ chế ở §1: nhánh `mut0` sụp cả khi **không** khan hiếm, vì mọi founder là cùng một genotype
>   3 đốt khai cứng, chỉ sống được ở mức biên; quần thể có đột biến tự thoát bằng cách bỏ đốt đuôi
>   ([chẩn đoán mut0](../ai/planning/2026-11-23-mut0-collapse-diagnosis.md)). Hệ quả là founder 2 đốt
>   opt-in (`live.founder_genotype = 1`), sống `12/12` seed ở VIAB-1b.
> - **E3b** (QĐ-E3B D1–D5): D3 lấy phân tích cặp đầy đủ làm chính, thêm Lee bounds và RMST làm đầu ra
>   riêng cho các tương phản có `U`; D4 khoá SESOI $\pm\ln(1{,}5)$ với $N = 64$.
> - **SEL2-P2** (QĐ-SEL2P2 D1–D7): họ khẳng định $m = 3$ với Holm, $\alpha = 0{,}05$ (D3, khớp gợi ý
>   $m \le 3$ của mục 4); SESOI $\pm\ln(1{,}10)$ (D4); xử lý tuyệt chủng theo mẫu #214 R3 với Lee bounds,
>   RMST(200k) và Lachin worst-rank (D6, khớp §3.4). Một lượt kiểm mù VIAB-2 12 seed × 200.000 tick chạy
>   trước khi đóng băng (QĐ-VIAB2-R).
> - **Chưa dùng:** cặp $r_c$ + $\hat{\Delta}_{\text{HL}}$ + BCa của §3.3, bộ đo Robertson–Price/MLS1
>   (gợi ý 2) và việc đo $\hat{\rho}_{\text{CRN}}$ (gợi ý 3).
> - **Cấu trúc tuổi (§2.1, §2.4):** ADR-0013 (proposed, chờ D1–D5) đề xuất tuổi theo lịch và giai đoạn
>   sống opt-in. Nếu được chấp nhận, đó là nguồn cho phân rã theo tuổi $\sum_a [\Delta c(a,t)]\bar z(a,t)$
>   mà hiện code chưa có (`AgentEvaluation::survival_ticks` không phải tuổi).

Quyết định đầy đủ: các mục `QD-E3B`, `QD-SEL2P2-D1..D7` và `QD-VIAB2-R` trong
[`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md).

> Đây **không** phải cổng mới. Lộ trình chính thức là các thẻ trong [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) §3.00: khảo sát khả năng sống sót VIAB-1 (QĐ-VIAB), rồi đăng ký mới E3b và SEL2-P2, rồi các mốc của `QD-ISLAND`. Bốn mục dưới đây là gợi ý đầu vào cho các thẻ đó; mọi ngưỡng trong đó là giả thuyết, tiêu chí thật do bản đăng ký và chủ dự án khoá.

1. **Gợi ý cho VIAB-1 — cơ chế khả năng sống sót trên tập seed hiệu chuẩn tách biệt:** Trước hết đo các khoá opt-in đã có (`live.founder_layout`, `live.strict_prey_cap`, `live.lake_layout`, `live.grass_steering`, `live.plant_reserve`), vì chúng nhắm đúng các nguyên nhân đã đo ở §1.1. Chỉ sau đó mới cân nhắc mã mới, mỗi thứ là một khoá `live.*` opt-in giữ nguyên từng bit đường cũ khi vắng: Holling III với $q$ khai báo, khoang $E_R$ + *refractory cooldown*, cạnh tranh *contest* trên ô, bể mảnh vụn khoáng hoá chậm. Mọi khoang năng lượng mới nằm trong sổ EU khép kín và mọi khoản chi mới đi qua `total_cost`. Các ngưỡng báo cáo ngoài đề xuất (sống sót $\ge 90\%\text{--}95\%$, 50–100 thế hệ, $N_{\min} \ge 8\text{--}10$) là **chưa xác minh**; 50–100 thế hệ cũng dài gấp 1,5–3 lần lượt 200.000 tick (~33 thế hệ) mà `QD-ISLAND` dự trù.
2. **Gợi ý cho bộ đo Robertson–Price / MLS1 (`QD-ISLAND` M1):** Triển khai đồng nhất thức 5 thành phần (`ddof=0`), `MLS1` trọng số $q_k$ + hồi quy ngữ cảnh $\beta_2 Z_k$, chạy song song trên giá trị di truyền $g_i$ và kiểu hình $z_i$. *Tiêu chí kiểm định đơn vị:* sai số đóng đồng nhất thức $< 10^{-12}$ ở mọi cửa sổ $\Delta t$.
3. **Gợi ý cho phân tích lực của E3b / SEL2-P2 / `QD-ISLAND`:** Đo $\hat{\rho}_{\text{CRN}}$ thực tế trên simulator Anima bằng seed hiệu chuẩn, đo riêng các thành phần nhiễu ($\hat{\sigma}_{\text{slope}}, \hat{\rho}, \widehat{\text{ICC}}, p_{\text{ext}}$), rồi chạy Monte Carlo theo SESOI để chốt $N_{\text{seed}}$. Không lấy các mức 32 / 48–64 của báo cáo ngoài làm sàn. Tách luồng RNG theo thành phần làm đổi quỹ đạo, nên cũng phải là opt-in và đi kèm nâng `LIVE_MODEL_VERSION` theo [ADR-0008](../decisions/ADR-0008-world-past-and-time.md) nếu đường mặc định đổi; `live.founder_layout = 2` đã dùng một luồng RNG riêng.
4. **Gợi ý cho bản đăng ký (đóng băng và tập seed khẳng định mới):** Khoá mã nguồn simulator, mở dải seed hoàn toàn mới (ví dụ `20001..20064`), họ giả thuyết chính $m \le 3$, bộ cỡ hiệu ứng ghép cặp ($r_c$, $\hat{\Delta}_{\text{HL}}$ + 95% BCa CI, đồng thuận seed), biên TOST $\pm\text{SESOI}$, và nhánh Two-Part RMST + CRN SACE + Lachin Worst-Rank.

---

## 5. Tài liệu tham khảo chọn lọc từ nguồn gốc

> **Ghi chú biên tập:** Nguồn nội bộ chưa công bố liên kết (mục `1` trong báo cáo ngoài gốc) đã được loại bỏ. Các liên kết năm 2026 (`[19]`, `[118]`) và bản tiền ấn phẩm (`[9]`, `[13]`) cần xác minh lại trạng thái bình duyệt/chính thức trước khi trích dẫn vào báo cáo công bố của dự án.

- **[2]** Causal Inference with Truncation-by-Death and Unmeasured Confounding (arXiv): https://arxiv.org/pdf/2109.13623
- **[3]** Identification and estimation of survivor average causal effects (PMC): https://pmc.ncbi.nlm.nih.gov/articles/PMC4131726/
- **[4]** Anazawa — *Scramble and contest competition, unequal resource allocation...*: https://www.researchgate.net/publication/228992380_Scramble_and_contest_competition_unequal_resource_allocation_and_resource_monopolization_as_determinants_of_population_dynamics
- **[5]** Uszko et al. — *When is a type III functional response stabilizing?*: https://www.researchgate.net/publication/277903095_When_is_a_type_III_functional_response_stabilizing_Theory_and_practice_of_predicting_plankton_dynamics_under_enrichment
- **[6]** Martin, Zimmer, Grimm & Jager — *Linking Individual-based Models and Dynamic Energy Budget Theory*: http://www.bio.vu.nl/thb/research/bib/Mart2013.pdf
- **[7]** Coulson & Tuljapurkar (2008) — *The Dynamics of a Quantitative Trait in an Age-Structured Population* (PMC): https://pmc.ncbi.nlm.nih.gov/articles/PMC3345271/
- **[8]** Okasha (2004) — *Multilevel Selection and the Partitioning of Evolutionary Change*: https://www.zoology.ubc.ca/let/pdfs/Okasha_2004.pdf
- **[9]** Brown & Field (2021, bioRxiv preprint) — Price equation with immigration/open populations: https://www.biorxiv.org/content/10.1101/2021.09.28.462138.full
- **[10]** Price (1972) — *Extension of covariance selection mathematics*: https://dynamics.org/Altenberg/LIBRARY/REPRINTS/Price_extension_AnnHumGenetLond.1972.pdf
- **[11]** Lazic (2010, PMC) — *The problem of pseudoreplication*: https://pmc.ncbi.nlm.nih.gov/articles/PMC2817684/
- **[12]** Importance of Random Slopes in Mixed Models (Barr et al. lineage): https://www.researchgate.net/publication/359639913_The_Importance_of_Random_Slopes_in_Mixed_Models_for_Bayesian_Hypothesis_Testing
- **[13]** Tsandilas & Casiez (under review / pre-pub) — *The illusory promise of the Aligned Rank Transform*: https://statransform.github.io/jovi/
- **[14]** Effect sizes for nonparametric tests (PMC): https://pmc.ncbi.nlm.nih.gov/articles/PMC12701665/
- **[15]** Cramer et al. (2016) — *Hidden multiplicity in exploratory multiway ANOVA*: https://www.researchgate.net/publication/269417778_Hidden_multiplicity_in_exploratory_multiway_ANOVA_Prevalence_and_remedies
- **[16]** Restricted Mean Survival Time Explained (MetricGate): https://metricgate.com/blogs/restricted-mean-survival-time/
- **[17]** Siepe et al. (2024, ADEMP-PreReg): https://www.researchgate.net/publication/375251208_Simulation_Studies_for_Methodological_Research_in_Psychology_A_Standardized_Template_for_Planning_Preregistration_and_Reporting
- **[18]** *Worst-Rank Score Methods—A Nonparametric Approach to Informatively Missing Data* — tác giả và năm **chưa xác minh** (tiêu đề khác bài Lachin 1999 ở [114]; trước đây ghi nhầm là Lachin 1999): https://www.researchgate.net/publication/344423884_Worst-Rank_Score_Methods-A_Nonparametric_Approach_to_Informatively_Missing_Data
- **[19]** *Earth as a Closed Life-Support System* (arXiv `2608.21505` — **cần xác minh**): https://arxiv.org/pdf/2608.21505
- **[20]** Theis et al. / Loreau (2021, HAL) — *Nutrient cycling and self-regulation determine food web stability*: https://hal.science/hal-03410819/document
- **[22]** Polyworld / Evolutionary ALife (Frontiers in Ecology and Evolution) — **không** phải nguồn của Flow-Lenia (Flow-Lenia là Plantec et al. 2023); nội dung bài này chưa xác minh: https://www.frontiersin.org/journals/ecology-and-evolution/articles/10.3389/fevo.2021.750779/full
- **[23]** Rosenzweig–MacArthur Jacobi Stability (MDPI): https://www.mdpi.com/2073-8994/14/9/1815
- **[24]** *Paradox of enrichment* (Wikipedia overview): https://en.wikipedia.org/wiki/Paradox_of_enrichment
- **[27]** Mollison (1991, Oxford ORA) — *Dependence of epidemic and population velocities on basic parameters* (Atto-fox): https://ora.ox.ac.uk/objects/uuid:3fcc3ac2-c5e7-4313-ba89-15b446fc5f24
- **[29]** Brown (1988) — *Patch use as an indicator of habitat preference, predation risk, and competition*: https://www.scribd.com/document/742326236/Brown1988
- **[31]** Allometric Scaling of Resource Intake (Kleiber / foraging): https://arxiv.org/html/1002.3208v1
- **[34]** Morozov (2010) — *Emergence of Holling type III zooplankton functional response*: https://pubmed.ncbi.nlm.nih.gov/20406647/
- **[35]** Morozov & Petrovskii (2013) — *Scaling up the predator functional response in heterogeneous environment*: https://pubmed.ncbi.nlm.nih.gov/23892150/
- **[37]** Brown / Kotler — *Strategy maps: Generalised giving-up densities for optimal foraging*: https://www.researchgate.net/publication/367614082_Strategy_maps_Generalised_giving-up_densities_for_optimal_foraging
- **[40]** Stephens, Boyd, McNamara & Houston (2009) — *Capital breeding and income breeding: their meaning, measurement, and worth*, Ecology 90(8):2057–2067 (đường dẫn trỏ tới bài này; Jönsson 1997, *Capital and income breeding as alternative tactics of resource use in reproduction*, là một bài khác, chưa có đường dẫn): https://www.researchgate.net/publication/26798158_Capital_breeding_and_income_breeding_Their_meaning_measurement_and_worth
- **[46]** Anazawa (2019, PMC) — *Inequality in resource allocation and population dynamics models*: https://pmc.ncbi.nlm.nih.gov/articles/PMC6689590/
- **[49]** Courchamp, Berec & Gascoigne — *Allee Effects in Ecology and Conservation*: https://books.google.com/books/about/Allee_Effects_in_Ecology_and_Conservatio.html?id=IS4TDAAAQBAJ
- **[51]** Stephens, Sutherland & Freckleton (1999) — *What Is the Allee Effect?*: https://conservationbytes.com/wp-content/uploads/2015/09/stephens-et-al-1999-oikos.pdf
- **[53]** Schreiber (2003) — *Allee effects, extinctions, and chaotic transients in simple population models*: https://schreiber.faculty.ucdavis.edu/wp-content/uploads/sites/568/2019/08/schreiber2003.pdf
- **[54]** Demographic effects of aggregation in presence of a component Allee effect: https://arxiv.org/html/2305.13414v2
- **[55]** Stabilizing effect of small prey immigration (Taylor & Francis 2024): https://www.tandfonline.com/doi/full/10.1080/13873954.2024.2366337
- **[58]** Nutrient flows between ecosystems can destabilize simple food chains: https://www.researchgate.net/publication/45091254_Nutrient_flows_between_ecosystems_can_destabilize_simple_food_chains
- **[59]** Soros & Stanley — *Necessary Conditions for Open-Ended Evolution Empirically Tested*: http://workshops.alife.org/oee4/papers/soros-oee4-camera-ready.pdf
- **[61]** Coulson & Tuljapurkar (2008 PDF copy): https://www.zoology.ubc.ca/let/pdfs/Coulson-Tuljapurkar_2008.pdf
- **[62]** Price Equation philosophical analysis: https://www.researchgate.net/publication/308261381_One_equation_to_rule_them_all_A_philosophical_analysis_of_the_Price_Equation
- **[63]** Frank (2012) — *Natural selection. IV. The Price equation*: https://arxiv.org/html/1204.1515v1
- **[64]** Kerr & Godfrey-Smith (2009) — *Generalization of the Price equation for evolutionary change*: https://pubmed.ncbi.nlm.nih.gov/19215293/
- **[67]** Barfield, Holt & Gomulkiewicz / Grafen context (PMC7133508): https://pmc.ncbi.nlm.nih.gov/articles/PMC7133508/
- **[68]** Lion (2018) — *Theoretical Approaches in Evolutionary Ecology*: https://evolepid.cefe.cnrs.fr/pub/Lion2018c.pdf
- **[70]** Queller (2017) — *Fundamental Theorems of Evolution*: https://www.zoology.ubc.ca/let/pdfs/Queller2017.pdf
- **[72]** Day, Parsons, Lambert & Gandon (2020), Phil. Trans. R. Soc. B 375:20190357 — *The Price equation and evolutionary epidemiology*: https://royalsocietypublishing.org/rstb/article/375/1797/20190357/23734/The-Price-equation-and-evolutionary
- **[73]** Day & Gandon (2006) — *Insights From Price's Equation into Evolutionary Epidemiology*: https://www.queensu.ca/academia/troy-day/sites/acaddaywww/files/uploaded_files/Uploads/PDF/DayGandon.pdf
- **[76]** Chuang, Rivoire & Leibler (2009) — *Simpson's paradox in a synthetic microbial system*: https://arxiv.org/html/1203.5863v1
- **[78]** Okasha / Heisler & Damuth lineage (Frontiers 2021) — *Contextual Analysis*: https://www.frontiersin.org/journals/ecology-and-evolution/articles/10.3389/fevo.2021.780508/full
- **[80]** Morrissey, Kruuk & Wilson (2010) — *The danger of applying the breeder's equation in observational studies of natural populations*: https://pubmed.ncbi.nlm.nih.gov/20831731/
- **[81]** Hajduk, Walling, Cockburn & Kruuk (2020, PMC7133512) — *The 'algebra of evolution': the Robertson–Price identity and viability selection for body mass in a wild bird population* (dữ liệu chim *Malurus cyaneus*): https://pmc.ncbi.nlm.nih.gov/articles/PMC7133512/
- **[85]** Rice (2008) — *A stochastic version of the Price equation*: https://www.researchgate.net/publication/23281501_A_stochastic_version_of_the_Price_equation_reveals_the_interplay_of_deterministic_and_stochastic_processes_in_evolution
- **[88]** Lazic (2022, Nature Primers) — *Genuine replication and pseudoreplication*: https://stanlazic.github.io/PDF/2022_Lazic_Nature_Primers.pdf
- **[89]** Brief Guide to Statistical Analysis of Grouped Data (PMC): https://pmc.ncbi.nlm.nih.gov/articles/PMC12893352/
- **[90]** Pseudoreplication & Design Effect overview: https://manuelgarcia.info/guides/research/guide/pseudoreplication-research
- **[94]** Lüpsen (2017) — *The aligned rank transform and discrete variables: A warning*: https://www.researchgate.net/publication/298411816_The_aligned_rank_transform_and_discrete_variables_A_warning
- **[95]** Schielzeth et al. (2020) — *Robustness of linear mixed-effects models to violations of distributional assumptions*: https://hal.science/hal-03055227/document
- **[97]** Kherad-Pajouh & Renaud (2015) — *A general permutation approach for analyzing repeated measures (`permuco`)*: https://www.researchgate.net/publication/269334513_A_general_permutation_approach_for_analyzing_repeated_measures_ANOVA_and_mixed-model_designs
- **[99]** Meinshausen et al. — *Asymptotic optimality of the Westfall–Young permutation procedure*: https://www.researchgate.net/publication/51910847_Asymptotic_optimality_of_the_Westfall-Young_permutation_procedure_for_multiple_testing_under_dependence
- **[100]** Wobbrock et al. (2011) — *The aligned rank transform for nonparametric factorial analyses*: https://faculty.washington.edu/wobbrock/pubs/chi-11.06.pdf
- **[101]** Cramer et al. (2016, PubMed) — *Hidden multiplicity in exploratory multiway ANOVA*: https://pubmed.ncbi.nlm.nih.gov/26374437/
- **[111]** Kerby (2014) — *The Simple Difference Formula (Rank-biserial)*: https://www.researchgate.net/publication/304355151_The_Simple_Difference_Formula_An_Approach_to_Teaching_Nonparametric_Correlation
- **[114]** Lachin (1999) — *Worst-Rank Score Analysis with Informatively Missing Observations*: https://www.rama.mahidol.ac.th/ceb/sites/default/files/public/pdf/journal_club/2021/s0197-2456_2899_2900022-7.pdf
- **[115]** Restricted Mean Survival Time (PMC): https://pmc.ncbi.nlm.nih.gov/articles/PMC9081686/
- **[118]** Munko et al. (GFDrmst, arXiv `2605.30517` — **cần xác minh**): https://arxiv.org/pdf/2605.30517
- **[119]** Kumle, Võ & Draschkow (2021) — *Estimating power in (generalized) linear mixed models*: https://www.researchgate.net/publication/351360812_Estimating_power_in_generalized_linear_mixed_models_An_open_introduction_and_tutorial_in_R
- **[122]** Morris, White & Crowther (ADEMP Framework): https://r-statistics.co/ADEMP-Framework-in-R.html
- **[123]** Law & Kelton — Variance Reduction / Common Random Numbers: https://www.scribd.com/document/378628562/BUDT758K-Sim-L24-OutputAnalysis-IV
- **[125]** Common Random Numbers for Simulation Models (PMC): https://pmc.ncbi.nlm.nih.gov/articles/PMC2761656/
- **[128]** Sample size for interactions — Unil (`16×` rule): https://api.unil.ch/iris/server/api/core/bitstreams/8e00a751-a83a-4d49-8280-3f237d003e13/content
- **[129]** Interaction power & variance scaling (`arXiv:2302.12878`): https://arxiv.org/pdf/2302.12878
- **[130]** Albers & Lakens (2018) — *When power analyses based on pilot data are biased*: https://www.casperalbers.nl/files/powerJESP2018.pdf
- **[132]** Lakens (2022) — *Equivalence Testing and Interval Hypotheses*: https://lakens.github.io/statistical_inferences/09-equivalencetest.html
- **[134]** Lakens, Scheel & Isager (2018) — *Equivalence Testing for Psychological Research: A Tutorial*: https://pure.tue.nl/ws/portalfiles/portal/91049519/PREPRINT_Lakens_etal_EquivalenceTestingTutorial_20171117.pdf
- **[136]** Lakens (2022) — *Preregistration and Transparency*: https://lakens.github.io/statistical_inferences/13-prereg.html
