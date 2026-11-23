---
title: Đồng tiến hoá não–thân, evo-devo và vận động tiếp đất — đánh giá đề xuất ngoài
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-22
review_cycle: quarterly
---

# Đồng tiến hoá não–thân, evo-devo và vận động tiếp đất — đánh giá đề xuất ngoài

> **Phạm vi & tính chất tài liệu.** Đây là bản **đánh giá và chắt lọc** một báo cáo nghiên cứu ngoài
> (Google Docs *"Đồng Tiến Hoá Não-Thân Evo-Devo"*, soạn bằng công cụ nghiên cứu tự động, nhận ngày
> 2026-11-22; dưới đây gọi là **báo cáo gốc**). Báo cáo gốc đề xuất ba thay đổi lớn: bộ não mô-đun
> chia sẻ trọng số theo đốt, hàm `develop_at_birth` có chuẩn phản ứng và dị hình giới tính kèm tăng
> trưởng con non → trưởng thành trên sổ năng lượng số nguyên, và một hạt nhân vật lý cây khớp phẳng
> 2.5D thay cho bộ lái trong `locomotion.rs`. Tài liệu này **không** phải hợp đồng hay kế hoạch đã
> duyệt. Số đo của repo chỉ nằm ở [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền);
> quyết định hiện hành nằm ở [ADR-0001](../decisions/ADR-0001-creature-development-lifecycle.md),
> [ADR-0003](../decisions/ADR-0003-evolved-per-agent-brains.md) và
> [ADR-0006](../decisions/ADR-0006-locomotion-actuation.md). Mọi thay đổi chạm genotype, phenotype,
> não hay vận động phải đọc trước phần "Required reading" trong `CLAUDE.md`.

## 0. Nhãn bằng chứng và kết luận ngắn

| Nhãn | Nghĩa |
|---|---|
| **Đã kiểm** | Đối chiếu trực tiếp với mã nguồn và tài liệu của repo ngày 2026-11-22. |
| **Nguồn ngoài** | Công trình có thật mà báo cáo gốc trích; nội dung được tóm theo báo cáo gốc, chưa đọc lại từng bài trừ khi ghi rõ. |
| **Chưa xác minh** | Con số hoặc khẳng định mà báo cáo gốc dẫn về một nguồn không truy được (mục trích dẫn số 3 của báo cáo gốc là `unknown_url`), hoặc là mô phỏng riêng không có mã trong repo. |
| **Suy luận** | Đánh giá của tài liệu này. |

**Kết luận (`Suy luận`):**

1. **Phần tổng quan văn liệu có giá trị và nên giữ.** Chuỗi kết quả của Mertan & Cheney về việc đồng
   tiến hoá não–thân hội tụ sớm, khái niệm *negative transfer* và *first-mover advantage*, cơ chế bảo
   vệ đổi mới hình thái (Cheney et al. 2018), chuẩn phản ứng (Scheiner 1993), công tắc điều hoà *cis*
   cho dị hình giới tính, và luật co giãn cơ sinh học khi lớn lên đều liên quan trực tiếp tới ADR-0001
   và kế hoạch morphogenesis.
2. **Tiền đề về ADR-0003 trong báo cáo gốc không khớp code.** Báo cáo gốc giả định não xuất lệnh theo
   từng khớp nên đổi số đốt sẽ làm lệch số chiều. Thực tế não hiện tại xuất **tham số CPG toàn cơ thể**,
   không phụ thuộc số đốt (§1).
3. **Đề xuất vật lý là một biến thể chi tiết của Phương án B trong ADR-0006**, vốn đã được nêu và
   hoãn lại. Thực hiện nó cần một ADR mới, đổi `LIVE_MODEL_VERSION`, và xử lý việc báo cáo gốc chuyển
   thế giới 3D (`Vec3`) sang mặt phẳng 2.5D (§3).
4. **Sổ năng lượng `u64` micro-EU và `WorldArena` SoA thay ECS là viết lại kiến trúc**, không phải
   bước nhỏ, và không có bằng chứng đo nào trong repo cho thấy cần (§4).
5. **Mọi con số hiệu năng và kết quả mô phỏng của báo cáo gốc là `Chưa xác minh`** (§5). Chúng không
   được dùng làm ngân sách hay tiêu chí cổng.

## 1. Bộ não và số đốt (ADR-0003)

### 1.1 Hiện trạng (`Đã kiểm`)

- [`brain_genotype.rs`](../../src-tauri/src/evolution/brain_genotype.rs): `BrainGenotype { version,
  arch: ArchSpec, weights }`; `ArchSpec { inputs, hidden, outputs }`. Kiến trúc live là
  `BrainModel::new(15, 64, 4)`: 15 đầu vào, 64 nơ-ron ẩn, 4 đầu ra là **tham số CPG** (`CPG_LEN = 4`),
  ADR-0003 mở rộng đầu ra tới các cổng hành động (`COUNT = 8`: pheromone, tấn công, ăn, tín hiệu).
- Đốt cơ thể do CPG điều khiển qua `update_cpg_system` → `resolve_joints_system` →
  `integrate_physics_system` ([`physics/dynamics.rs`](../../src-tauri/src/physics/dynamics.rs)).
  Não không xuất mô-men từng khớp.
- Đột biến hình thái có thêm nút ([`mutation.rs`](../../src-tauri/src/evolution/mutation.rs),
  `mutate_genotype` gọi `add_node`), nhưng vì đầu ra não là tham số CPG chung, **thêm/bớt đốt không
  làm lệch số chiều của não**.

### 1.2 Điều báo cáo gốc nói đúng (`Nguồn ngoài` + `Suy luận`)

Vấn đề thật không phải số chiều mà là **chất lượng điều phối khi hình thái đổi**:

- **Mertan & Cheney** (EuroGP 2024, arXiv:2402.09231; ALIFE 2025, arXiv:2508.17464): đồng tối ưu
  não–thân hội tụ sớm; bộ điều khiển kế thừa tối ưu cho cơ thể cha mẹ gây *negative transfer* trên
  cơ thể con; hình thái xuất hiện sớm tích luỹ lợi thế tinh chỉnh (*first-mover advantage*). Báo cáo
  gốc dẫn con số 1.305.840 hình thái EvoGym 3×3; chưa đọc lại bài để xác nhận.
- **Cheney, Bongard, SunSpiral & Lipson (2018)**, *J. R. Soc. Interface* 15:20170937: bảo vệ đổi mới
  hình thái (*morphological innovation protection*) giảm áp lực đào thải ngay sau đột biến tô-pô.
- Các họ bộ điều khiển bất biến theo số đốt: CPPN/HyperNEAT, GNN truyền thông điệp (NerveNet, SMP của
  Huang, Mordatch & Pathak 2020), Transformer hình thái (Amorpheus, MetaMorph, BoT), NCA cục bộ (Hartl,
  Levin & Zöttl 2024). Chúng quan trọng **nếu** Anima chuyển sang điều khiển từng khớp.

### 1.3 Đánh giá đề xuất não mô-đun 858 tham số (`Suy luận`)

- Kiến trúc lai *Morphology-Conditioned CPG + Tree-SMP Elman RNN* của báo cáo gốc là một hướng hợp
  lý **cho tương lai có điều khiển từng khớp hoặc CPG sóng lan truyền** (§3). Nó không sửa một lỗi
  hiện có.
- Nếu làm, phải giữ mọi luật cứng của ADR-0003: không Lamarck (`AgentBrain.learned` không ghi về
  genotype); restore/migration mang theo não được giao; đường legacy là mặc định và vẫn tới được;
  layout trọng số và cổng parity EB-S02; chỉ một mục tiêu A2C (`simulation_loop::a2c_loss`).
- "Bảo vệ đổi mới hình thái" (`morph_innovation_age`) **can thiệp vào chọn lọc**. Trong Anima chọn
  lọc phải đến từ sinh thái hoặc `EvolutionaryReplacement` đã khai báo; một cơ chế bảo vệ chỉ được
  phép là khoá opt-in, khai báo như một can thiệp, và được tính vào thiết kế thí nghiệm.

## 2. Evo-devo, dị hình giới tính và tăng trưởng (ADR-0001)

### 2.1 Khớp với mục tiêu đã duyệt (`Đã kiểm`)

- Hàm `develop_at_birth` thuần, chạy một lần, đọc genotype bất biến và mẫu môi trường lúc sinh là
  **mục tiêu đã chấp nhận** của ADR-0001 (task CM-201/CM-202 trong
  [`CREATURE_MORPHOGENESIS_PLAN.md`](../planning/CREATURE_MORPHOGENESIS_PLAN.md)). Kiểu đầu ra tên là
  `DevelopedPhenotype`, **không** phải `PhenotypeTarget` như báo cáo gốc đặt. Các kiểu này **chưa tồn
  tại trong code**; xem bảng "Trạng thái triển khai" của
  [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md).
- `AgentSex` đã có trong [`components.rs`](../../src-tauri/src/core/components.rs). Đưa giới tính làm
  một đầu vào của `develop_at_birth` là mở rộng tự nhiên.

### 2.2 Nội dung đáng đưa vào kế hoạch morphogenesis (`Nguồn ngoài` + `Suy luận`)

- **Chuẩn phản ứng** (Scheiner 1993, *Annu. Rev. Ecol. Syst.* 24): genotype mã hoá hệ số của hàm
  môi trường → kiểu hình; đây không phải Lamarck. Dạng tham số
  `trait = f(β₀ + β_env·e_birth + β_sex·s)` khớp với hàng rào Weismann mà ADR-0001 yêu cầu.
- **Công tắc điều hoà *cis*** (Williams & Carroll 2009, *Nat. Rev. Genet.*): hai giới dùng chung gen
  cấu trúc, khác nhau ở mức biểu hiện. Cho phép dị hình giới tính tiến hoá dần từ đơn hình (β_sex = 0).
- **Kênh hoá vi sai** (Kriegman, Cheney & Bongard 2018, *Sci. Rep.*) và **giàn giáo hình thái**
  (Bongard 2011, *PNAS*) là cơ sở cho việc để mềm dẻo phát triển tồn tại trước khi đồng hoá di truyền.

### 2.3 Điểm cần chủ dự án quyết (`Suy luận`)

- **Tăng trưởng con non → trưởng thành là thay đổi kích thước trong đời.** ADR-0001 nói development
  xảy ra một lần lúc sinh và ECS tiêu thụ một phenotype. Tăng trưởng liên tục có thể mô hình bằng
  "trần trưởng thành khoá lúc sinh + hệ số tỉ lệ `s(t)` là `RuntimeState`", nhưng cách đọc đó chưa có
  trong hợp đồng. Cần một dòng quyết định trước khi viết code.
- Nếu có tăng trưởng, bảng co giãn của báo cáo gốc là điểm khởi đầu tốt (`Nguồn ngoài`: Hill 1950;
  Alexander 2003; Biewener 2003): chiều dài ∝ s, khối lượng ∝ s³, quán tính ∝ s⁵, mô-men cực đại ∝ s³,
  độ cứng khớp ∝ s⁵, giảm chấn ∝ s⁴, tần số CPG ∝ s^(−1/2) theo số Froude. Mục đích là giữ tỉ lệ gia
  tốc góc ổn định ở bước 1/60 s. **Chưa đo trong Anima.**
- Mọi chi phí năng lượng của tăng trưởng phải đi vào `total_cost` trong `metabolic_decay_system`
  (luật cứng của ADR-0003) để chảy qua `respired` vào detritus.

## 3. Vận động tiếp đất (ADR-0006)

### 3.1 Hiện trạng (`Đã kiểm`)

- [ADR-0006](../decisions/ADR-0006-locomotion-actuation.md) (accepted 2026-11-16) chọn **Phương án A**:
  `steering_drive_system` ([`locomotion.rs`](../../src-tauri/src/core/locomotion.rs)) ghi
  `target_velocity`, và lực ngoài duy nhất là `(target_velocity − velocity) · mass · 2.0` trên gốc.
  Hệ này opt-in; khi tắt, quỹ đạo cũ giữ nguyên từng bit.
- Bằng chứng thăm dò trong ADR-0006: cản dị hướng **không** tạo lực đẩy vì CPG 3 đốt hiện là **dao
  động đứng** quanh một trục cố định; lực đẩy từ ma sát cần **sóng lan truyền lệch pha**. Đó là
  **Phương án B**, đã được ghi là tuỳ chọn sau khi CPG sóng lan truyền được thiết kế riêng.
- Vật lý hiện là 3D: `velocity`, `force` là `glam::Vec3` ([`dynamics.rs`](../../src-tauri/src/physics/dynamics.rs)).

### 3.2 Điều báo cáo gốc bổ sung (`Nguồn ngoài`)

- **Ma sát vảy rắn bất đẳng hướng** (Hu, Nirody, Scott & Shelley 2009, *PNAS* 106:10081): ma sát
  trượt tiến thấp, trượt lùi trung bình, trượt ngang cao, và nhấc thân có chọn lọc tăng hiệu suất.
  Đây là cơ chế cụ thể cho Phương án B.
- **Bộ điều khiển PD ổn định** (Tan, Liu & Turk 2011, *IEEE CG&A*): mô-men khớp tính ẩn sang bước kế
  tiếp, cho phép độ cứng lớn ở bước thời gian cố định.
- **Thuật toán ABA** (Featherstone): động lực học cây khớp trong thời gian tuyến tính.
- **Rủi ro khai thác vật lý** (Lehman et al., *The Surprising Creativity of Digital Evolution*,
  *Artificial Life* 2020; Sims 1994): tiến hoá khai thác sai số tích phân, ví dụ rung khớp tần số cao
  để lấy "lực đẩy ma". Đề xuất kiểm "quẫy trong chân không, khối tâm không trôi" là một test tốt.

### 3.3 Đánh giá (`Suy luận`)

- Báo cáo gốc giả định "góc nhìn từ trên xuống, mặt phẳng 2.5D". Anima có địa hình với cao độ, hồ,
  sông và mô phỏng 3D; chuyển sang mặt phẳng là đổi ngữ nghĩa thế giới, ảnh hưởng terrain navigation,
  World v2 và mọi checksum. Phương án ít xâm lấn hơn là giữ 3D và thêm tiếp xúc nền + ma sát dị hướng
  dọc trục đốt theo pháp tuyến địa hình.
- Thứ tự hợp lý: (1) thiết kế **CPG sóng lan truyền** (điều kiện tiên quyết ADR-0006 đã nêu); (2) đo
  lực đẩy từ ma sát dị hướng trên vật lý hiện có; (3) chỉ khi (2) không đủ ổn định mới cân nhắc thay
  bộ tích phân bằng ABA + SPD.
- Thay `locomotion.rs` mặc định cần ADR mới (thay thế hoặc mở rộng ADR-0006), bump
  `LIVE_MODEL_VERSION` theo ADR-0008, cập nhật `build_tick_schedule` và các test thứ tự lịch, và giữ
  đường cũ tới được bằng khoá opt-in.
- CPG sóng lan truyền cũng là điều kiện tiên quyết cho **lực đẩy dưới nước** (cản dị hướng theo
  từng đốt, khối lượng kèm theo) mà đề xuất môi trường thuỷ sinh cần; xem
  [`QD_REALMS_HABITATS_RESEARCH_2026-11-22.md`](QD_REALMS_HABITATS_RESEARCH_2026-11-22.md).
- Đối chiếu Rapier2D/Avian2D của báo cáo gốc dựa trên benchmark của một engine nhỏ (Balaur) và
  changelog; **yếu**, không dùng để loại trừ phương án.

## 4. Sổ năng lượng và bố cục bộ nhớ (`Suy luận`)

- Báo cáo gốc đề xuất mọi ngăn năng lượng là `u64` micro-EU để "tránh rò rỉ do làm tròn". Hiện trạng
  (`Đã kiểm`): năng lượng cá thể là `f32` (`HomeostaticState.energy`), dòng `respired` cộng dồn bằng
  `f64` vào detritus. Thẻ M4 đo **dư EU lớn nhất 1,03e-8 EU** ([`eu_residual.md`](../paper/data/eu_residual.md)).
  Không có bằng chứng rò rỉ cần sửa; đổi sang số nguyên là viết lại hợp đồng EU đóng.
- Ba ngăn `E_struct / E_reserve / E_repro` theo DEB (Kooijman; Van der Meer 2006) là mô hình đẹp cho
  tăng trưởng, nhưng chạm `autonomous_reproduction_system` (ngưỡng 0,65, dự trữ an toàn 0,30) và các
  khoá đã hiệu chuẩn của Chặng 4. Chỉ nên làm sau khi đóng VIAB-1 → E3b / SEL2-P2, dưới khoá opt-in.
- `WorldArena` SoA tĩnh `#[repr(align(64))]` thay Bevy ECS mâu thuẫn với kiến trúc hiện tại
  (`bevy_ecs` 0.13, lịch dựng ở một chỗ duy nhất `core::simulation_schedule::build_tick_schedule`).
  Ràng buộc zero-alloc trong vòng tick **đã** là luật của repo; không cần đổi kiến trúc để có nó.

## 5. Con số chưa xác minh trong báo cáo gốc

Các con số sau dẫn về mục trích dẫn `unknown_url` hoặc mô phỏng riêng không có mã trong repo. Chúng
là **giả thuyết**, không phải số đo:

- 1,878 ms/tick cho 1.000 cá thể × 8 đốt (11,3 % khung 60 Hz); ~3,5 ms ở 16 đốt; 14,19 MB RAM.
  So sánh (`Đã kiểm`): thẻ M3 đo **1.000 agent 5,6 ms/tick trên Windows, 3,6 ms trên Mac** với hệ
  hiện tại ([`scaling.md`](../paper/data/scaling.md)).
- Độ trôi khối tâm "0,000000 m" khi quẫy 20 Hz trong chân không; kiểm toán 1.200 bước dư 0 µEU.
- Con non 6,4 g đi 2,465 m/10 s; con trưởng thành 3,166 → 4,115 m khi nhấc bụng; dáng trot 4,981 m/10 s.
- 13,31 MFLOPs/tick cho não 858 tham số; 120,83 MFLOPs cho Body Transformer.
- Các nguồn yếu khác: hồ sơ alphaXiv, bản PDF tải lại trên `originalwisdom.com`, trang benchmark
  Balaur. Các tiền ấn phẩm 2026 (BraiNCA arXiv:2604.01932, AdaControl arXiv:2608.23100) chưa đọc lại.

## 6. Việc nên làm tiếp (`Suy luận`, chờ chủ dự án)

1. **Không** mở thẻ code nào từ báo cáo gốc trước khi đóng Chặng 4.
2. Nếu muốn sinh vật tự đi bằng cơ thể: mở một thẻ thiết kế **CPG sóng lan truyền + ma sát dị hướng
   trên vật lý 3D hiện có** (Phương án B của ADR-0006), có test "quẫy trong chân không không trôi" và
   test tịnh tiến, tất cả opt-in.
3. Khi làm CM-201/CM-202: cân nhắc thêm giới tính và dạng chuẩn phản ứng vào `develop_at_birth`, và
   ghi một dòng quyết định về tăng trưởng trong đời (§2.3).
4. Ghi "bảo vệ đổi mới hình thái" vào danh sách can thiệp chọn lọc có thể khai báo, chưa bật.

## 7. Tài liệu tham khảo chính (theo báo cáo gốc; chưa kiểm lại từng DOI)

- Cheney, N., Bongard, J., SunSpiral, V., & Lipson, H. (2018). Scalable co-optimization of morphology
  and control in embodied machines. *J. R. Soc. Interface*, 15, 20170937.
- Mertan, A., & Cheney, N. (2024). Investigating premature convergence in co-optimization of
  morphology and control in evolved virtual soft robots. EuroGP 2024. arXiv:2402.09231.
- Mertan, A., & Cheney, N. (2025). Evolutionary brain-body co-optimization consistently fails to
  select for morphological potential. ALIFE 2025. arXiv:2508.17464.
- Gupta, A., Savarese, S., Ganguli, S., & Fei-Fei, L. (2021). Embodied intelligence via learning and
  evolution. *Nature Communications*, 12, 5721.
- Huang, W., Mordatch, I., & Pathak, D. (2020). One policy to control them all: Shared modular
  policies for agent-agnostic control. ICML 2020.
- Scheiner, S. M. (1993). Genetics and evolution of phenotypic plasticity. *Annu. Rev. Ecol. Syst.*, 24, 35–68.
- Williams, T. M., & Carroll, S. B. (2009). Genetic and molecular insights into the development and
  evolution of sexual dimorphism. *Nature Reviews Genetics*, 10, 797–804.
- Kriegman, S., Cheney, N., & Bongard, J. (2018). How morphological development can guide evolution.
  *Scientific Reports*, 8, 13934.
- Bongard, J. (2011). Morphological change in machines accelerates the evolution of robust behavior.
  *PNAS*, 108(4), 1234–1239.
- Hu, D. L., Nirody, J., Scott, T., & Shelley, M. J. (2009). The mechanics of slithering locomotion.
  *PNAS*, 106(25), 10081–10085.
- Tan, J., Liu, K., & Turk, G. (2011). Stable proportional-derivative controllers. *IEEE Computer
  Graphics and Applications*, 31(4), 34–44.
- Lehman, J., et al. (2020). The surprising creativity of digital evolution. *Artificial Life*, 26(2), 274–306.
- Sims, K. (1994). Evolving virtual creatures. SIGGRAPH 1994.
- Featherstone, R. (2008). *Rigid Body Dynamics Algorithms*. Springer.
