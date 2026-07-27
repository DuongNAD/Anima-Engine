---
title: ADR-0003 — Não di truyền theo cá thể và mở rộng không gian hành động
status: accepted
owner: simulation-architecture
last_reviewed: 2026-07-27
decision_date: 2026-07-25
accepted_date: 2026-07-25
supersedes: none
superseded_by: none
---

# ADR-0003 — Não di truyền theo cá thể và mở rộng không gian hành động

## Bối cảnh

Anima là engine ALife có **cơ thể tiến hoá nhưng trí tuệ không tiến hoá**. Đo từ mã nguồn
ngày 2026-07-25:

- `BrainModel::new(15, 64, 4)` được `insert_resource` **một lần** trong
  `SimulationEngine::start` → **toàn bộ agent chia sẻ một mạng ~5.5k tham số**.
- `MorphologyGenotype` chỉ có `nodes`/`edges`. Không có gen não ở bất kỳ đâu trong
  `evolution/`.
- Gradient A2C (`simulation_loop.rs`, Adam + TD-error) cập nhật **chính mạng dùng chung đó**.

Hệ quả suy ra được mà không cần chạy:

1. MAP-Elites chỉ illuminate **không gian hình thái**; hai elite ở hai ô niche khác nhau
   vẫn hành xử bằng cùng một bộ não.
2. `niche_divergence` (E11) đo được phân kỳ khối lượng cơ thể nhưng **không thể** đo phân
   kỳ hành vi — hành vi không có biến thể để phân kỳ.
3. Một gradient dùng chung kéo mọi agent về **một chính sách trung bình**; nó *chống lại*
   đa dạng, ngược mục tiêu quality-diversity của chính dự án.

**Ràng buộc thứ hai, hẹp hơn và dễ bỏ sót.** 4 output của actor head đi thẳng vào
`InertiaComponent.cpg_parameters: [f32; 4]` (`core/components.rs`), rồi được đọc ở
`agent_systems.rs` để đặt tần số/biên độ CPG. Kiểm tra các hệ thống sinh thái cho thấy
**không có hành động nào khác do não điều khiển**:

| Hành vi | Cơ chế hiện tại | Não có kiểm soát? |
|---|---|---|
| Vận động | 4 tham số CPG từ actor head | Có |
| Phát pheromone | `agent_release_pheromone_system` cộng `PheromoneReleaser.strength` **mỗi tick**, vô điều kiện | Không |
| Săn/chiến đấu | `combat_system` kích hoạt **tự động theo khoảng cách** predator–prey | Không |
| Ăn | Va chạm với `Food` | Không |

Nghĩa là bộ não hiện tại là **bộ điều khiển dáng đi**, không phải bộ ra quyết định. Kể cả
khi mỗi agent có não riêng, hai agent vẫn **không thể** khác nhau ở "săn hay trốn", "phát
tín hiệu hay im lặng" — vì không tồn tại kênh để biểu đạt.

Bằng chứng bên ngoài nằm ở
[nghiên cứu nâng cấp map/ML](../research/MAP_AND_ML_UPGRADE_RESEARCH.md) §B. Tóm tắt: cả
ba hệ tham chiếu gần nhất — JaxLife (ALIFE 2024), The Bibites, và nghiên cứu môi trường
sinh thái quy mô lớn 2025 (60.000+ agent) — đều tiến hoá **trọng số riêng từng cá thể** và
**không dùng RL**. JaxLife tự nêu hạn chế của họ là *"mọi agent giống hệt nhau về vật lý"*,
tức chính thứ Anima đã có.

Trạng thái phụ thuộc, cần nói rõ vì nó chi phối phạm vi:

- [ADR-0001](ADR-0001-creature-development-lifecycle.md) đã `accepted` nhưng **chưa triển
  khai**: không tồn tại `ecomorph.rs`, `DevelopedPhenotype` hay `develop_at_birth` trong
  mã nguồn. ADR này **không được phép** phụ thuộc vào chúng.
- Determinism tầng live vừa được sửa (C2): `core::resources::SimRng` + `derived_rng`, 8
  điểm `thread_rng()` đã bị gỡ, có gate `tests/sim_determinism_tests.rs`. Đây là tiền đề
  bắt buộc — không có nó thì không thể so sánh đối chứng "não riêng" với "não chung".

## Động lực quyết định

- Hành vi phải **di truyền và biến dị** thì QD/E11/S43 mới có nghĩa.
- Phải quan sát được đa dạng hành vi, không chỉ đa dạng dáng đi.
- Không được chặn bởi ADR-0001 (chưa triển khai) và không được cản nó sau này.
- Giữ **luật zero-alloc** trên tick path (`SIMULATION_RULES.md`).
- Giữ **closed-EU**; mọi chi phí năng lượng mới phải có nguồn/sink.
- Giữ **D07 determinism** của
  [hợp đồng phát triển sinh vật](../reference/CREATURE_DEVELOPMENT_CONTRACT.md).
- Giữ **D09 tương thích dữ liệu cũ**: save cũ load được, default giữ hành vi cũ.
- Ngân sách máy phát triển yếu: không được biến mỗi tick thành N lần backward pass.
- Phải có **đường tắt/rollback** rõ ràng, theo tiền lệ `exotic_energy=None` của
  [ADR-0002](ADR-0002-world-laws-and-exotic-energy.md).

## Các phương án

### A. Giữ não dùng chung, chỉ tăng dung lượng mạng

Rẻ nhất. Không giải quyết gì: một mạng lớn hơn vẫn là **một** chính sách cho mọi cá thể,
nên đa dạng hành vi vẫn bằng không và QD vẫn chỉ đo hình thái. **Bị từ chối.**

### B. Não dùng chung + nhiễu ngẫu nhiên theo cá thể

Thêm một vector nhiễu per-agent vào input hoặc output. Tạo được *biến thiên* nhưng không
tạo được *di truyền*: nhiễu không truyền cho con, không chịu chọn lọc. Đây là đa dạng giả.
**Bị từ chối.**

### C. Vector trọng số phẳng, giao diện cố định, di truyền theo cá thể

`BrainGenotype` là vector `f32` mã hoá một topology cố định `I → H → A`. Vì giao diện cố
định, **não không phụ thuộc hình thái** ⇒ độc lập hoàn toàn với ADR-0001; mutation là nhiễu
Gauss; inference là matmul theo lô.
Nhược: gặp bức tường brain–body khi hình thái đa dạng cao (xem D). **Được chọn cho v1.**

### D. Mã hoá gián tiếp (CPPN/HyperNEAT) ngay từ đầu

Genome là một CPPN nhận **toạ độ hai điểm** và xuất **trọng số kết nối** giữa chúng, nên
một genome sinh được não cho **mọi** body plan. Đúng về lâu dài, và Anima đã có sẵn hệ toạ
độ substrate (`MorphologyEdge.joint_anchor`, vị trí segment).

Không chọn cho v1 vì hai lý do:

1. Bức tường brain–body **chỉ thực sự đau khi hình thái đa dạng cao**, mà hiện tại quần thể
   khởi tạo là 10 cá thể cùng một genotype 3-node. Trả giá phức tạp trước khi có vấn đề là
   tối ưu hoá sớm.
2. Tài liệu 2024–2025 cảnh báo co-optimization não–thân **thường xuyên thất bại trong việc
   chọn được hình thái có tiềm năng cao** và dễ **hội tụ sớm**. Cần dữ liệu từ v1 để biết
   Anima có gặp không, trước khi chọn cách chữa.

**Hoãn sang ADR riêng**, kích hoạt khi EB-S09 (xem gate) cho thấy fitness suy giảm theo độ
đa dạng hình thái.

**Đã đo (EB-S09, 2026-07-25) — tín hiệu YẾU nhưng CÓ.** Reciprocal transplant trên trục hình
thái (thân 2/3/5/8 đốt × 5 gait, đo quãng đường di chuyển — chính đại lượng
`check_epoch_completion_system` chấm điểm): gait tốt nhất là **#2 cho 2, 5 và 8 đốt** nhưng
**#1 cho 3 đốt** (0,490 so với 0,321 — hơn **53%**). Nghĩa là **điều khiển tối ưu ĐÃ phụ thuộc
cơ thể**, đúng cơ chế đã dự đoán: bốn tham số CPG được áp cho *mọi* khớp, dù cơ thể có bao nhiêu.

Chưa đủ để mở ADR phương án D — quần thể khởi tạo vẫn là 10 cá thể **cùng một genotype 3-node**,
nên bức tường chưa gây thiệt hại thật. Nhưng đây là bằng chứng nó **sẽ** cần khi hình thái thực sự
đa dạng, chứ không còn là lo xa. Điều kiện kích hoạt nên đọc lại khi quần thể có nhiều body plan.

*Một hệ quả phụ đáng lưu ý:* quãng đường tăng mạnh theo số đốt (0,066 → 1,94). So sánh fitness
**giữa** các hình thái vì thế bị nhiễu bởi kích thước cơ thể — thêm một lý do MAP-Elites phải bin
theo khối lượng cơ thể thay vì xếp hạng phẳng.

### E. Bỏ RL hoàn toàn, thuần tiến hoá

Giống JaxLife/Bibites/arXiv 2510.18221. Đơn giản hơn nhiều và **phụ thuộc `burn` gần như
biến mất**, giảm mạnh rủi ro migrate của việc nâng cấp dependency. Đánh đổi: mất khả năng
thích nghi trong đời cá thể. **Bị từ chối theo quyết định của chủ dự án ngày 2026-07-25**,
chọn hướng hybrid (F).

### F. Hybrid — tiến hoá quyết định khởi tạo, học trong đời tinh chỉnh

Tiến hoá đặt **trọng số khởi tạo**; A2C tinh chỉnh **trong đời từng cá thể**, không chia sẻ
giữa các cá thể; cái học được **không di truyền**. Đây là **hiệu ứng Baldwin**: cái học
được không truyền, nhưng *khả năng học nhanh* thì có. **Được chọn**, sau cờ, mặc định tắt.

### Giữ nguyên hiện trạng

Chi phí: mọi metric đa dạng đã xây (MAP-Elites, `niche_divergence`, archive coverage,
Red-Queen S43) tiếp tục đo một đại lượng không tồn tại. Rủi ro lớn nhất không phải hiệu
năng mà là **kết luận sai**: báo cáo "đồng tiến hoá" từ một hệ mà hành vi không thể tiến hoá.

## Quyết định

1. **`BrainGenotype` là anh em của `MorphologyGenotype`, không nằm trong `DevelopedPhenotype`.**
   Đặt tại `evolution/brain_genotype.rs`. Lý do: giữ ADR-0003 độc lập với ADR-0001 chưa
   triển khai, và để `develop_at_birth` sau này chỉ phải lo hình thái.

2. **Bốn lớp dữ liệu theo đúng mô hình của
   [CREATURE_MORPHOGENESIS.md](../explanation/CREATURE_MORPHOGENESIS.md):**

   | Lớp | Nội dung não | Di truyền? |
   |---|---|---|
   | Genotype | `BrainGenotype` — trọng số khởi tạo + `ArchSpec` | Có |
   | Birth environment | *(v1: không dùng)* | — |
   | Developed phenotype | *(v1: rỗng — giao diện cố định nên không có bước phát triển)* | — |
   | Runtime state | Trọng số **sau khi học trong đời** | **Không** |

   Không có Lamarck: trọng số đã học **MUST NOT** ghi ngược vào `BrainGenotype`.

3. **Giao diện cố định trong v1, và topology phải TRÙNG `ActorCriticModel` đang chạy.**
   `ArchSpec { inputs, hidden, outputs }` với topology `I → H → H → {A actor, 1 critic}` —
   **hai** lớp trunk cùng bề rộng, đúng như `ai/model.rs` (trunk1, trunk2, actor_head,
   critic_head), relu sau mỗi trunk, sigmoid ở actor, tuyến tính ở critic. Trùng topology là
   **điều kiện để EB-S02 và EB-S04 có nghĩa**: không trùng thì không có parity để so, và
   `brain_genotype=None` không còn là baseline đối chứng thật.
   Số chiều là dữ liệu, không phải hằng số biên dịch, để v2 đổi mà không phá format.

4. **Mở rộng không gian hành động — bắt buộc đi kèm, không phải tuỳ chọn.** Một quyết định
   riêng lẻ "não per-agent" mà vẫn chỉ xuất 4 tham số CPG là **không quan sát được**. Bốn
   output mới **chỉ mở van cho hành vi đã tồn tại**, không thêm cơ chế mô phỏng mới:

   | Output mới | Nối vào | Mặc định legacy |
   |---|---|---|
   | `pheromone_emit` | nhân vào `PheromoneReleaser.strength` trong `agent_release_pheromone_system` | `1.0` = phát liên tục như hiện nay |
   | `attack_intent` | ngưỡng kích hoạt trong `combat_system` | `1.0` = luôn tấn công khi đủ gần |
   | `feed_intent` | ngưỡng nhận năng lượng trong `detect_food_collisions_system` | `1.0` = luôn ăn |
   | `signal_channel` | *(dự trữ, chưa nối)* | không dùng |

   Vì mỗi van có mặc định "luôn mở" tái lập đúng hành vi hôm nay, đây là thay đổi
   **có thể hoàn tác từng phần**.

5. **Inference không đi qua `burn`.** Mỗi agent có trọng số riêng nên bài toán đổi từ
   "1 model × N input" thành "N model × 1 input". Với mạng cỡ này đó là matmul nhỏ; triển
   khai bằng buffer tiền cấp phát trong `BrainInferenceBuffer` + `rayon`, **giữ nguyên luật
   zero-alloc**. Hệ quả phụ có giá trị: `burn` chỉ còn nằm trên đường **training**, thu hẹp
   biên migrate của việc nâng cấp dependency.

6. **Học trong đời sau cờ, mặc định TẮT.** N cá thể = N backward pass. Khi bật, chỉ chạy
   cho agent trong active-radius của Simulation-LOD. Cờ tắt = thuần tiến hoá (phương án E),
   nên E vẫn là đường lùi chạy được chứ không phải giả thuyết.

7. **Đường tương thích và rollback là `brain_genotype: Option<BrainGenotype>`.**
   `None` ⇒ dùng `BrainModel` chung như hôm nay, **bit-identical với baseline**. Theo đúng
   tiền lệ `exotic_energy=None` của ADR-0002.

8. **Save/migration.** `SerializedAgent` và `AgentMigrationData` mang **cả** `BrainGenotype`
   **và** trọng số đã học, đều `#[serde(default)]`. Restore/migration **MUST NOT** khởi tạo
   lại não: một cá thể di cư không được quên những gì nó đã học. Đây là mở rộng của D02
   sang trạng thái nhận thức.

9. **Determinism, và một đính chính.** `BrainGenotype` khởi tạo/đột biến nhận `&mut impl Rng`
   từ `SimRng`/`derived_rng`, không bao giờ `thread_rng()`. Đồng thời: D07 quy định seed lấy
   từ `WorldIdentity.seed`, nhưng `SimRng::from_env()` (C2) đang đọc `ANIMA_SIM_SEED`. Phải
   hoà giải — `WorldIdentity.seed` là nguồn khi có world artifact, `ANIMA_SIM_SEED` chỉ là
   override cho chạy headless. Ghi nhận là nợ của C2, sửa trong bước 1 của kế hoạch dưới.

10. **Chi phí năng lượng của mô não: mặc định `0.0`.** Sinh học ủng hộ việc tính phí (mô
    thần kinh đắt, và phí này tạo áp lực chọn lọc chống phình não), nhưng nó đụng closed-EU.
    Đặt `brain_metabolic_cost` là tham số có ngưỡng, mặc định `0.0` để baseline không đổi,
    và **chỉ bật sau khi EB-S06 chứng minh EU vẫn đóng**.

11. **Không được tuyên bố quá.** Một cụm hành vi mới **không phải** một loài, và một ô
    MAP-Elites mới **không phải** bằng chứng thích nghi. Áp gate chứng cứ như CM-S11/AE-S11
    trước mọi tuyên bố về "phân loài hành vi" hay "đồng tiến hoá".

12. **[Bổ sung 2026-07-27 — DEC-1] Mốc so sánh của EB-S04 là bản dựng CÓ SEED.**
    Mốc cũ — "trùng bit với bản dựng *trước* ADR-0003" — **bị bỏ**. Xem mục
    [Cập nhật 2026-07-27](#cập-nhật-2026-07-27--dec-1-re-baseline-eb-s04) để biết vì sao mốc cũ
    không tồn tại chứ không phải chỉ khó đo. Mốc mới, phát biểu tường minh:

    > **Baseline EB-S04 = bản dựng có seed, `BrainPolicy::evolved = false`** (tương đương
    > `brain_genotype = None`). Hai mệnh đề phải đúng và cả hai đều **trùng bit**:
    > **(a)** cùng seed ⇒ cùng quỹ đạo, giữa hai lần chạy bất kỳ của cùng một bản dựng;
    > **(b)** cài `ActionGates` mà để mở ⇒ quỹ đạo trùng với bố cục component *trước* bước 4.

    Quyết định **mặc định** (`evolved: true` hay giữ opt-in) **không** nằm trong mục này và vẫn để
    mở. Nó cần một thí nghiệm đối chứng được tiền đăng ký trước khi chạy; mốc EB-S04 mới chỉ là điều
    kiện cần để thí nghiệm đó có một baseline thật để so.

## Hệ quả

### Tích cực

- Hành vi trở thành đối tượng của chọn lọc; MAP-Elites, `niche_divergence` và S43 lần đầu
  đo được thứ chúng vốn định đo.
- Anima vào một vị trí hiếm trong tài liệu: **cơ thể tiến hoá và não tiến hoá**, trên nền
  sinh thái năng lượng đóng đã kiểm chứng. Ba hệ tham chiếu gần nhất chỉ có vế sau.
- Phụ thuộc `burn` co lại còn đường training ⇒ giảm rủi ro cho việc nâng `burn 0.13 → 0.21`.
- `brain_genotype=None` cho một baseline đối chứng **chạy được**, không phải mô tả trên giấy.
- Bốn van hành động biến các hệ thống thụ động sẵn có thành kênh biểu đạt mà không thêm
  cơ chế mô phỏng nào.

### Tiêu cực / rủi ro

- Bộ nhớ theo quần thể: trọng số × N agent. Với `I=15, H=64, A=8` là **5.769 `f32` ≈ 22,5 KiB**
  mỗi cá thể ⇒ 1 triệu agent ≈ **21 GiB** chỉ riêng trọng số — **không tương thích với mục tiêu
  "triệu agent"** nếu mọi cá thể đều giữ đủ trọng số.

  Điều này biến Simulation-LOD (M3, còn nợ ở backend) từ "tối ưu hoá" thành **điều kiện tiên
  quyết của quy mô**: chỉ agent trong active-radius mới cần trọng số thường trú, ngoài đó là
  cập nhật quần thể thống kê. Trần bộ nhớ khi đó là **số agent thường trú**, không phải tổng
  quần thể — cùng một lập luận mà terrain streaming đã dùng.

  **Đã đo (EB-S12, 2026-07-25):** 22,5 KiB mỗi agent; **~46.500 agent thường trú mỗi GiB** trọng
  số. Bật học-trong-đời thì agent mang **hai** mạng ⇒ **45 KiB**, tức **một nửa** số agent thường
  trú trong cùng ngân sách — chi phí thật của nửa Baldwin, không phải ước lượng. Trong ba hướng
  giảm mà ADR liệt kê (bề rộng ẩn, lượng tử hoá, chia sẻ theo dòng dõi) mới chỉ hướng đầu dùng
  được: giảm `H` từ 64 xuống 32 cho **~3,1×** ít tham số, vì ma trận trunk→trunk chi phối tổng.
- Inference thủ công là code số học mới; sai sót ở đây âm thầm hơn lỗi biên dịch. Bắt buộc
  có test parity với đường `burn` hiện tại (EB-S02).
- Bốn output mới đổi kích thước tensor hành động. ~~⇒ đụng IPC payload và type phía TypeScript.~~
  **Đo lại 2026-07-25: dự đoán này SAI.** Vector hành động nằm trọn trong backend
  (`AgentInferenceResponse` → `InertiaComponent`/`ActionGates`); `src/commands/` không lộ
  `cpg_parameters`/`actions` và frontend không có tham chiếu nào. **Toàn bộ ADR-0003 không chạm
  frontend** — phạm vi thực tế hẹp hơn hẳn dự đoán ban đầu.
- Học trong đời khi bật sẽ **đắt** và có thể làm sai lệch tín hiệu chọn lọc (agent học giỏi
  che mất genome kém). Đây là lý do nó mặc định tắt và cần EB-S08 để so sánh.
- Rủi ro hội tụ sớm của co-optimization não–thân (tài liệu 2024–2025) sẽ xuất hiện dần khi
  hình thái đa dạng lên; QD là cơ chế giảm nhẹ, không phải cách chữa khỏi.

## Kế hoạch triển khai và hoàn tác

1. ✅ **[XONG 2026-07-25] Hoà giải seed** (nợ C2): `resolve_run_seed(world_seed)` — `WorldIdentity.seed`
   là nguồn, `ANIMA_SIM_SEED` chỉ là override; `init_world` là **nơi duy nhất** quyết định và chèn
   `SimRng`. Luồng tiến hoá sinh ra *trước* world nên dùng `world_seed_from_disk()` +
   `WorldArtifact::peek_seed` (đọc mỗi header, không giải mã payload); hai đường phải trùng và được
   khoá bởi `evolution_thread_and_world_agree_on_seed`. `derived_rng` nay nhận run seed tường minh.
2. ✅ **[XONG 2026-07-25] Type + hàm thuần, chưa đụng runtime**: `evolution/brain_genotype.rs` —
   `ArchSpec` (`I → H → H → {A, 1}`, khớp `ActorCriticModel`), `BrainGenotype`, khởi tạo He/Xavier,
   `mutate_brain`, `crossover_brains`, `forward_into` (zero-alloc, buffer của caller). 17 unit test.
   **Không system nào đọc chúng.**

   *Ghi chú cho bước 3:* layout trọng số ở đây là `w[out * fan_in + in]`, tức **chuyển vị** so với
   `burn 0.13` (`Linear::weight` có shape `[d_input, d_output]`, tính `input.matmul(weight)`). Chép
   phẳng giữa hai biểu diễn mà không chuyển vị cho ra mạng chạy được, hữu hạn, và **sai âm thầm**.
   Burn cũng khởi tạo **cả bias** từ `U(-k, k)` chứ không phải 0 như module này.
3. ✅ **[XONG 2026-07-25] Parity gate (EB-S02)**: `ActorCriticModel::from_flat_weights` (lần đầu chạm
   runtime, thuần additive — một constructor, không đổi hành vi model đang chạy) + `transpose_to_burn`
   tách riêng và unit-test. `tests/brain_parity_tests.rs` 8/8: parity trên arch đang chạy (15×64×4),
   arch mở rộng (15×64×8), **bốn arch mọi chiều khác nhau** (arch vuông sẽ để lọt lỗi chuyển vị),
   input bão hoà, độc lập theo hàng trong batch, và genome đã qua mutation+crossover.
   **Hai test chứng minh gate có lực**: một trọng số sai, hoặc một lớp bị chuyển vị, đều phải phá
   parity — nếu không thì mọi assert còn lại chỉ là trang trí.
   Sai số thực đo: actor `1.8e-7` (đúng bằng `f32::EPSILON`, một ULP), critic `8.0e-7`.
4. ✅ **[XONG 2026-07-25] Mở rộng action space, mọi van mặc định "luôn mở"** ⇒ hành vi **không đổi**.
   `core::components::ActionGates` (3 trường, `Default` = mở hết; `ActionGates::of(None)` cũng đọc là
   **mở**, vì save cũ không được nạp thành agent từ chối ăn — D09). Nối vào
   `agent_release_pheromone_system` (nhân, có clamp `[0,1]`), `detect_food_collisions_system`, và
   **cả hai nhánh** của `combat_system`. `decode_genotype` gắn `ActionGates::default()`.
   **Chưa có gì ghi vào component này** — nó chỉ là chỗ để bước 5 ghi vào.

   *Quyết định phạm vi đã tinh chỉnh:* **không** nới `BrainModel::new(15, 64, 4)` thành `(15, 64, 8)` ở
   bước này. Đổi số tham số của model dùng chung sẽ đổi lượng RNG tiêu thụ lúc khởi tạo ⇒ đổi trọng số
   ⇒ **đổi quỹ đạo hôm nay**, tức phá chính EB-S04. Tensor hành động chỉ nới khi `brain_genotype =
   Some(..)`, nơi kiến trúc rộng là **của riêng từng cá thể** và model dùng chung không bị đụng tới.
5. 🟡 **[MỘT PHẦN 2026-07-25] Bật `Some(...)` sau cờ cho genesis + evolutionary replacement.**
   Đã xong: `core::components::AgentBrain{genotype, learned}` · save (`SerializedAgent.brain`) và
   migration (`AgentMigrationData.brain`), cả hai `#[serde(default)]`, restore/migration **mang theo**
   chứ không sinh mới (D01) và **từ chối** brain hỏng thay vì chạy nhiễu · `core::resources::BrainPolicy`
   (resource, mặc định **tắt**, `ANIMA_EVOLVED_BRAINS` bật) + `EVOLVED_ARCH` 15×64×**8** và
   `action_index` chốt ý nghĩa từng output · genesis và `SpawnGenotypeCommand` tạo brain khi cờ bật,
   rút từ `SimRng`. Gate EB-S07/EB-S10 xanh.

5b. ✅ **[XONG 2026-07-25] Nối suy luận — brain riêng điều khiển hành vi thật.**

   Phát hiện khi làm: `ai::model::brain_inference_system` **không nằm trong schedule**; đường sống là
   `sensory_system` → `InferenceRequestBatch` **qua channel** → worker thread → `action_resolution_system`.

   - `AgentBrain.genotype` chuyển sang `Arc<BrainGenotype>` (bật feature `rc` của `serde`, **không thêm
     crate**), nên `AgentInferenceRequest.brain: Option<Arc<..>>` chỉ là **tăng refcount** — không chép
     ~23 KiB trọng số mỗi agent mỗi tick.
   - Worker **lọc** request có brain ra khỏi lô Burn thay vì chạy hết rồi ghi đè: giữ đường legacy
     **bit-identical** khi không agent nào có brain (nền của EB-S04) và không trả tiền cho forward pass
     bị vứt đi. Lô rỗng thì bỏ qua Burn hẳn.
   - `AgentInferenceResponse.actions` nới `[f32;4]` → `[f32;8]`; `action_resolution_system` ghi
     `0..CPG_LEN` vào CPG và 3 slot sau vào `ActionGates`. `LastTransitionState.action` **giữ 4 slot** —
     nó nuôi A2C của model dùng chung, vốn không biết gì về van.
   - Fallback khi brain lỗi là **van mở + không đổi vận động**, không phải vector 0 — vector 0 sẽ đọc
     thành "đóng mọi van", tức agent lặng lẽ ngừng ăn vì lý do không liên quan chọn lọc.

   **Đính chính dự đoán phạm vi:** ADR trước đó nói bước này chạm IPC và TypeScript. **Sai** — vector
   hành động hoàn toàn nội bộ backend; `src/commands/` không lộ `cpg_parameters`/`actions` và frontend
   không có tham chiếu nào. Toàn bộ ADR-0003 **không chạm frontend**.
6. ✅ **[XONG 2026-07-25] Chạy đối chứng có seed.** `tests/brain_controlled_comparison_tests.rs`
   (11/11) chạy vòng lặp ECS headless và bơm channel suy luận bằng **chính** `run_inference_batch`
   mà worker gọi — một bản dựng lại sẽ chỉ đang tự đo chính nó. Để làm được, logic worker được tách
   khỏi closure của thread ra thành hàm trong `ai/model.rs`; trước đó nó **không test được**.

   **Bước này tìm ra một lỗi mà C2 chưa bắt: model dùng chung khởi tạo KHÔNG tất định.**
   Vị trí và năng lượng khớp giữa hai lần chạy cùng seed, nhưng `cpg_parameters` thì không.
   Nguyên nhân: `LinearConfig::init` trả `Param::uninitialized` — trọng số được materialize **lười**
   từ một RNG tĩnh toàn tiến trình **tự tiến lên** mỗi lần rút, nên `Backend::seed` trước lúc dựng
   **không sửa được gì**: model thứ hai rút từ generator đã bị model thứ nhất đẩy đi.
   Sửa bằng `BrainModel::new_seeded`: tự rút trọng số từ stream có seed rồi nạp qua
   `from_flat_weights`, giữ nguyên phân phối `U(-k, k)`, `k = sqrt(1/fan_in)` của Burn. Vẫn **lười**
   materialize (materialize ngay đẩy `SimulationEngine::start` chậm quá ngưỡng stress test).

   **Hệ quả cần nói thẳng:** quỹ đạo baseline **không** còn trùng bản dựng trước ADR — trước đó
   *không tồn tại* một quỹ đạo baseline ổn định để mà trùng. Đây là sửa lỗi có chủ ý, không phải hồi quy.
7. ✅ **[XONG 2026-07-25] Học trong đời sau cờ riêng, mặc định tắt, chỉ trong active-radius.**
   `brain_genotype::learn_step` — backprop viết tay cho chính topology này, cộng
   `ai::model::lifetime_learning_system` và `resources::LifetimeLearning{enabled, learning_rate,
   discount, interval, active_radius}` (`ANIMA_LIFETIME_LEARNING`, **chỉ có hiệu lực khi `evolved`**).
   Reward là chính drive-reduction homeostatic mà model dùng chung đang dùng.

   *Hai khác biệt có chủ ý, đều về chi phí:* **SGD thay vì Adam** (Adam cần 2 buffer moment mỗi tham
   số ⇒ gấp ba bộ nhớ per-agent vốn đã là rủi ro quy mô của ADR này); và **chỉ huấn luyện khối CPG**
   vì `LastTransitionState.action` chỉ ghi 4 tham số vận động, không có target cho van sinh thái.
   v1 vì thế phân vai rõ: **tiến hoá đặt chính sách sinh thái, học-trong-đời tinh chỉnh dáng đi.**

   **🔴 Bước này tìm ra lỗi dấu trong A2C của model dùng chung.** `run_training_loop` dùng
   `(a − â)²·(−td)`: với advantage **dương**, hệ số âm ⇒ giảm loss = **tăng** `(a − â)²` = đẩy chính
   sách **ra xa** hành động vừa tốt hơn kỳ vọng, và **về phía** hành động tệ hơn. `learn_step` viết
   **đúng dấu** (`+td`) thay vì sao chép lỗi. Lỗi này **có trước** ADR-0003; sửa nó đổi quỹ đạo
   legacy nên được tách thành việc riêng.

   *Vì sao gradient check một mình không đủ:* `the_learning_gradient_matches_finite_differences` pass
   với **cả hai** dấu — nó chỉ kiểm đạo hàm có khớp hàm loss, không kiểm hàm loss có đúng ý đồ.
   `learning_moves_the_policy_toward_a_rewarded_action` mới là test phát hiện ra, và nó fail với dấu cũ.
8. **Rollback**: đặt `brain_genotype = None` và đóng các van về `1.0`. Reader của cả hai
   trường vẫn giữ, nên save sinh ra ở chế độ bật vẫn đọc được sau khi tắt.

## Bằng chứng xác minh

| Gate | Bằng chứng | Ngưỡng | Trạng thái |
|---|---|---|---|
| **EB-S01** | Cùng seed ⇒ cùng `BrainGenotype` khởi tạo/đột biến/crossover | exact | ✅ pass — `evolution::brain_genotype::tests` 17/17 |
| **EB-S02** | Forward pass thủ công vs `burn` `ActorCriticModel`, cùng trọng số | ≤ `1e-5` (đo được: actor `1.8e-7`, critic `8.0e-7`) | ✅ pass — `tests/brain_parity_tests.rs` 8/8 |
| **EB-S03** | Tick path với não per-agent | `allocs == 0` | ✅ pass — `tests/brain_budget_tests.rs`: suy luận per-agent **0 alloc/tick**, bước gradient **0 alloc**; cài mạng đã học tốn **đúng 1** alloc (ngoại lệ có chủ ý, throttle bằng `interval`). Đường Burn dùng chung **không** zero-alloc và chưa bao giờ — đo và ghi lại thay vì giấu |
| **EB-S04** | `brain_genotype = None` vs baseline **bản dựng có seed** (re-baseline 2026-07-27, quyết định 12) | quỹ đạo **bit-identical** ở cả (a) cùng seed hai lần chạy và (b) van mở vs bố cục trước bước 4 | ✅ pass — đo 2026-07-27 tại `2b61d07`: `tests/brain_controlled_comparison_tests.rs` 11/11, trong đó `the_run_is_reproducible_under_one_seed` là (a) và `installing_the_gates_changed_nothing_with_them_open` là (b). Mốc **cũ** ("trùng bản dựng trước ADR") đã bị bỏ vì bản dựng đó không có quỹ đạo tất định nào để mà trùng — xem [Cập nhật 2026-07-27](#cập-nhật-2026-07-27--dec-1-re-baseline-eb-s04) |
| **EB-S05** | Mọi van hành động mở hoàn toàn | hành vi **không đổi** so với trước bước 4 | ✅ pass — `tests/action_gates_tests.rs` 13/13 |
| **EB-S06** | Closed-EU với `brain_metabolic_cost > 0` | delta trong dung sai S01 (`1e-9`) | ✅ pass — `tests/brain_cost_and_coupling_tests.rs`: chi phí não gộp vào `total_cost` ⇒ chảy qua `respired` vào detritus, EU đóng; chi phí có thật và **tăng theo kích thước não**; mặc định `0.0` giữ baseline y hệt |
| **EB-S07** | Restore + migration | `BrainGenotype` **và** trọng số đã học giữ nguyên | ✅ pass — `tests/brain_persistence_tests.rs` 14/14 |
| **EB-S08** | Học trong đời bật vs tắt, cùng seed | có báo cáo, không có ngưỡng pass/fail | ✅ pass — `tests/brain_lifetime_learning_tests.rs` 9/9 + gradient check bằng sai phân hữu hạn trong `evolution::brain_genotype` |
| **EB-S09** | Fitness theo độ đa dạng hình thái | phát hiện bức tường brain–body | ✅ đã đo — **tín hiệu YẾU nhưng CÓ**: gait tốt nhất là #2 cho thân 2/5/8 đốt nhưng **#1 cho thân 3 đốt** (0,490 vs 0,321 — hơn 53%). Điều khiển tối ưu ĐÃ phụ thuộc cơ thể. Chưa đủ để mở ADR phương án D (quần thể khởi tạo vẫn là 10 cá thể giống hệt), nhưng là bằng chứng sẽ cần khi hình thái thực sự đa dạng |
| **EB-S10** | Save cũ (không có trường não) load | default `None`, hành vi cũ | ✅ pass — `brain_persistence_tests` (save + migration payload) |
| **EB-S11** | Phân kỳ hành vi, cùng seed, `Some` vs `None` | phương sai > 0 ở `Some` | ✅ pass phần hành vi — cùng quan sát: `None` cho **1** chính sách, `Some` cho **8/8** chính sách khác nhau, và khác cả ở kênh sinh thái chứ không chỉ dáng đi. Độ phủ archive **chưa đo được** headless (cần luồng tiến hoá) |
| **EB-S12** | Bộ nhớ mỗi agent và tổng theo quần thể | có ngân sách công bố, không hồi quy im lặng | ✅ pass — **22,5 KiB/agent** (5.769 f32); agent có học mang **hai** mạng ⇒ **45 KiB**. Trần công bố `BRAIN_BUDGET_BYTES` 24 KiB / 48 KiB. 1 triệu agent ≈ **21,5 GiB** ⇒ ~**46.500 agent thường trú mỗi GiB**. Giảm nửa bề rộng ẩn ⇒ **~3,1×** ít tham số |

## Trạng thái tại thời điểm accepted (2026-07-25)

Ghi lại đúng những gì đúng khi quyết định được khoá, để người đọc sau không phải suy ra từ lịch sử.

> 📜 **Đo lịch sử.** Mọi con số trong mục này là kết quả **ngày 2026-07-25**, lúc ADR được accept.
> Chúng cố ý **không** được cập nhật: một ADR ghi lại căn cứ của quyết định tại thời điểm đó. Số
> hiện tại ở [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).

**Đã triển khai và đo:** cả 7 bước của kế hoạch. **11/12 gate pass.**
`cargo test --no-fail-fast -j 2` → 470 passed *(đo 2026-07-25)*. Cờ `ANIMA_EVOLVED_BRAINS` và
`ANIMA_LIFETIME_LEARNING` đều **mặc định tắt**, `brain_metabolic_cost` mặc định `0.0` — một run
không khai báo gì chạy đúng đường legacy.

**EB-S04 chỉ đạt một phần, và phần thiếu là KHÔNG THỂ đo.** Yêu cầu gốc là "trùng bit bản dựng
trước ADR". Bước 6 phát hiện bản dựng đó **không có quỹ đạo ổn định nào** để mà trùng: model dùng
chung khởi tạo từ RNG toàn tiến trình chưa bao giờ được gieo hạt. Phần đo được — cài van mà để mở
thì quỹ đạo trùng bit — đã pass. Ghi nhận là **giới hạn của gate**, không phải nợ kỹ thuật.

**Ba lỗi có sẵn tìm được trong quá trình triển khai**, cả ba cùng một hình dạng "chạy được, số hữu
hạn, nhìn hợp lý":

1. `MapElitesArchive.grid` là `HashMap` ⇒ `RandomState` gieo hạt theo tiến trình ⇒ chọn cha mẹ
   không tái lập **dù đã có RNG seed**. Sửa: `BTreeMap`.
2. `LinearConfig::init` trả `Param::uninitialized` ⇒ trọng số materialize **lười** từ RNG tĩnh
   **tự tiến lên** ⇒ `Backend::seed` trước lúc dựng vô dụng. Sửa: `BrainModel::new_seeded` tự cấp
   trọng số.
3. A2C actor loss dùng `(a−â)²·(−td)` — **sai dấu**, model dùng chung học ngược.
   **Chưa sửa** (đổi quỹ đạo legacy), đã tách thành việc riêng.

**Hai quyết định vận hành còn để mở — có chủ ý, không phải bỏ sót:**

- **Có bật `brain_metabolic_cost` mặc định không?** EB-S06 đã chứng minh bật được mà không rò năng
  lượng, nhưng bật nó đổi áp lực chọn lọc, nên là quyết định của chủ dự án chứ không phải hệ quả kỹ thuật.
- **Ngưỡng nào ở EB-S09 thì mở ADR cho phương án D?** Đọc hiện tại là "yếu nhưng có". Ngưỡng nên
  được chốt khi quần thể thực sự có nhiều body plan, chứ không phải đoán bây giờ.

**Việc kế tiếp mà ADR này không bao trùm:** độ phủ MAP-Elites archive (cần luồng tiến hoá chạy
headless), Simulation-LOD thật cho `active_radius`, và phương án D khi EB-S09 vượt ngưỡng.

## Cập nhật 2026-07-27 — DEC-1: re-baseline EB-S04

> Mục này **không** thuộc khối lịch sử phía trên. Nó ghi một quyết định lấy ngày 2026-07-27, sau khi
> ADR được accept, theo đúng cơ chế DEC-1 trong
> [`docs/ai/planning/2026-07-27-feature-anima-completion.md`](../ai/planning/2026-07-27-feature-anima-completion.md) §3.

### Vì sao mốc cũ bị bỏ — nó không tồn tại, chứ không phải khó đo

Yêu cầu gốc của EB-S04 là "quỹ đạo trùng bit với bản dựng **trước** ADR-0003". Bước 6 của kế hoạch
đo được điều khiến yêu cầu đó vô nghĩa: bản dựng ấy **không có một quỹ đạo tất định nào**.
`LinearConfig::init` trả `Param::uninitialized`, nên trọng số của model dùng chung được materialize
**lười** từ một RNG tĩnh toàn tiến trình **tự tiến lên** mỗi lần rút — hai lần chạy cùng seed đã
khác nhau, và đã khác nhau *trước* mọi thay đổi của ADR này.

Ba hệ quả, và điều thứ ba mới là lý do phải xử lý chứ không phải để im:

1. Mốc cũ không phải một mốc. Không có tập bit nào để so, nên "trùng bit với nó" là một mệnh đề
   không có giá trị chân lý.
2. Việc gieo hạt (`BrainModel::new_seeded`) là **sửa lỗi có chủ ý**, không phải hồi quy. Nó là thứ
   *tạo ra* một baseline có thể so, chứ không phải thứ phá baseline.
3. Một gate **không thể pass bằng cách viết code đúng** thì sớm muộn cũng bị ngừng đọc, và một gate
   bị ngừng đọc còn tệ hơn không có gate — nó vẫn chiếm một dòng "🟡" mà không ai còn tin.

Ba phương án đã cân nhắc và ghi ở DEC-1: (1) re-baseline tường minh; (2) để nguyên 🟡 vĩnh viễn;
(3) dựng lại một baseline "tương đương-có-seed" trùng bit. Chủ dự án chọn **(1)**.

### Mốc mới, và bằng chứng của nó

Baseline là **bản dựng có seed với `BrainPolicy::evolved = false`**, và gate đòi hai mệnh đề trùng
bit — xem quyết định 12 ở trên cho phát biểu chuẩn. Đo ngày **2026-07-27** tại commit `2b61d07`,
`cargo test --features desktop --test brain_controlled_comparison_tests` → **11 passed; 0 failed**:

| Mệnh đề | Test | Kết quả |
|---|---|---|
| (a) cùng seed ⇒ cùng quỹ đạo | `the_run_is_reproducible_under_one_seed` | ✅ |
| (b) van mở ⇒ trùng bố cục trước bước 4 | `installing_the_gates_changed_nothing_with_them_open` | ✅ |
| Đường legacy thật sự là legacy | `with_brains_off_no_agent_ever_gains_one`, `with_brains_off_no_gate_ever_closes`, `with_brains_off_every_agent_shares_one_policy` | ✅ |

So sánh bằng `to_bits()` trên vị trí, năng lượng, tham số CPG và ba van — "không đổi" nghĩa là không
đổi, không phải gần không đổi.

### Cái gì bị mất khi bỏ mốc cũ — nói thẳng

Mốc cũ, ngoài việc canh ADR-0003, còn **ngẫu nhiên** canh một thứ rộng hơn: "không có gì trong toàn
bộ engine làm đổi quỹ đạo legacy". Mốc mới không canh được điều đó theo cùng cách: nó so hai lần
chạy của **cùng một bản dựng**, nên một thay đổi nằm ngay trong chính commit gieo hạt sẽ không bị
nó phát hiện. Đây là chi phí thật của phương án (1), đã biết trước khi chọn, và cách bù là các gate
khác: `tests/sim_determinism_tests.rs` cho tầng live, `tests/energy_conservation_tests.rs` cho
closed-EU, và checksum quỹ đạo của `LiveExperimentAdapter`.

### Cái gì **không** được quyết ở đây

Quyết định **bật `evolved` mặc định** vẫn để mở. Re-baseline chỉ trả lại một baseline có thật để so;
nó không nói gì về việc treatment tốt hơn hay tệ hơn. Việc đó cần một chạy đối chứng nhiều seed
được **tiền đăng ký trước khi chạy**, và kết quả của nó sẽ được ghi vào ADR này khi đã có — không
sớm hơn.

Bộ tiền đăng ký đó (E2) đã có. **Đã chạy một lần ngày 2026-07-27** — kết quả và quyết định ở
[Cập nhật 2026-07-27 — E2](#cập-nhật-2026-07-27--e2-kết-quả-đo-được-và-quyết-định-mặc-định);
câu dưới đây mô tả trạng thái *tại thời điểm DEC-1*, giữ nguyên làm chứng cứ rằng bản đăng ký có
trước lần chạy:
[requirements](../ai/requirements/2026-07-27-experiment-e2-evolved-brain-default.md) ·
[design](../ai/design/2026-07-27-experiment-e2-evolved-brain-default.md) ·
[planning](../ai/planning/2026-07-27-experiment-e2-evolved-brain-default.md) ·
[testing](../ai/testing/2026-07-27-experiment-e2-evolved-brain-default.md), manifest máy đọc được ở
`src-tauri/tests/fixtures/experiments_e2/`. Quy tắc quyết định — bao gồm việc **kết quả âm hoặc null
vẫn là kết quả**, và một lỗi thật của đường não tiến hoá phải được ghi là **lỗi** chứ không được
giấu thành "chưa đủ bằng chứng" — nằm ở planning §8.

Phiên tiền đăng ký cũng tìm ra hai điều kiện chặn: `LiveExperimentAdapter` **chưa có đường nào** để
manifest yêu cầu `evolved = true` (`build_live_world` chèn cứng `BrainPolicy::default()`), và
`live_experiment::genesis` **không** tạo `AgentBrain` kể cả khi cờ bật — khác genesis của app trong
`simulation_loop.rs`. Nhánh treatment vì thế **chưa tồn tại**; đặc tả seam đã được chốt trước ở
design §3 để nó không thể được chỉnh sau khi nhìn thấy số.

## Cập nhật 2026-07-27 — E2: kết quả đo được và quyết định mặc định

> Mục này ghi kết quả của thí nghiệm đối chứng đã **tiền đăng ký trước khi chạy** (`ce761d1`), chạy
> **một lần** ngày 2026-07-27 tại commit `9c57184`. Quy tắc quyết định nằm ở
> [planning §8](../ai/planning/2026-07-27-experiment-e2-evolved-brain-default.md) và được viết
> **trước** khi có số. Kết quả đầy đủ:
> [`artifacts/experiments/e2-evolved-brain-default/RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md).

### Quyết định 13 — `evolved` **giữ nguyên opt-in**

Theo đúng quy tắc đã đăng ký: **H1 null ⇒ giữ opt-in**, ghi là "không đo được hiệu ứng vật chất nào
trong headless adapter ở N = 12, T = 18.000". Có **12/12 cặp hoàn chỉnh** (tối thiểu là 10) và seam
được dựng đúng đặc tả design §3, nên "chưa đủ bằng chứng" — vốn chỉ dành cho dưới 10 cặp hoặc một
tiền điều kiện được thoả mãn khác đặc tả — **không** áp dụng.

Nói thẳng: mặc định giữ opt-in **không phải** vì đã chứng minh não riêng vô ích hay có hại, mà vì
quy tắc chỉ cho phép lật mặc định khi có hiệu ứng dương **vật chất**, và không có gì vật chất được đo.

### Số đo, và vì sao nó gần như không mang bằng chứng về não

| đại lượng | giá trị |
|---|---|
| cặp hoàn chỉnh | 12/12 |
| `control_mean` / `treatment_mean` | `0.000000` / `0.000000` EU |
| `paired_mean_delta` | `0.000000` EU |
| SD ghép / phương sai giữa các seed | `0.000000` / `0.000000` |
| KTC 95 % | `[0.000000, 0.000000]` |
| `d_z` | **không xác định** (SD ghép bằng 0) |
| vật chất? | **không** — 1/3 điều kiện |

Delta không "nhỏ"; nó **đúng bằng 0 trên cả 12 seed**.

Lý do là **điểm đo cuối không có sức phân giải**. Ở T = 18.000, toàn bộ quần thể sáng lập — ở **cả
hai** nhánh, trên **cả 12** seed — đang ở đúng 0 năng lượng và đã như vậy hàng nghìn tick. Hai sự
kiện cộng lại:

1. **Engine không có cái chết vì đói.** `update_agent_evaluation_system` gặp `homeo.energy <= 0.0`
   thì `continue` — ngừng cộng fitness, **không** giết. Ba nơi gọi
   `ReclaimAndDespawnAgentCommand` là thay thế tiến hoá, săn mồi, và can thiệp `RemovePredators`.
   Trong **app** đây là lựa chọn thiết kế chứ không phải lỗi: luân chuyển quần thể đến từ thay thế
   theo epoch.
2. **Adapter không chạy thay thế tiến hoá** — chính là finding **E2-F3**, đã ghi trong bản tiền đăng
   ký *trước* khi chạy.

Cộng lại: không gì giết agent đã đói và không gì thay thế nó, nên `live.agent_count` = **10 ở mọi
nhánh, mọi seed, mọi mẫu**, và quần thể đóng băng thành mười cơ thể bất động giữ 0 EU. E2-F3 được
đăng ký như một giới hạn phạm vi về **chọn lọc**; lần chạy này đo được hệ quả thứ hai chưa từng được
đăng ký của nó: nó cũng **phá huỷ mọi đại lượng năng lượng đo ở cuối một run dài**.

**Đây không phải lỗi của đường não tiến hoá.** Cả hai nhánh làm y hệt nhau, và nguyên nhân có mặt cả
khi `evolved = false`.

### Điều DUY NHẤT mang thông tin thật: EB-S06 giữ vững dưới treatment

`live.closed_eu_total` chênh lệch ghép **−2,6 × 10⁻⁹ EU trên tồn kho 1,5 × 10⁵ EU** (tương đối
~1,7 × 10⁻¹⁴) — nhiễu dấu phẩy động, không phải rò rỉ. **Não riêng theo cá thể không làm dịch chuyển
năng lượng nằm ngoài sổ cái.** Hợp đồng closed-EU đứng vững dưới treatment, đo trên 12 cặp.

Dấu vết đo được duy nhất khác của treatment: **+0,505 EU vào detritus và −0,505 EU khỏi standing
crop**, đối xứng chính xác, bảo toàn tới 1e−9, `d_z` = 0,11, dịch chuyển tương đối 0,05 % — **không
vật chất** theo mọi ngưỡng đã đăng ký.

### Quan sát CHƯA đăng ký — sinh giả thuyết, KHÔNG phải kết luận

Bản tiền đăng ký cấm đổi metric sau khi thấy số. Hai quan sát dưới đây **không được trích dẫn như
kết quả E2**:

- **(a)** Treatment chạm sàn năng lượng **muộn hơn** control ở **10/12 seed** (tick trung bình 8.650
  so với 5.250). Ngược chiều dự đoán của H1. "Thời gian tới sàn" không phải observable trong registry
  và chưa bao giờ được đăng ký.
- **(b)** Delta năng lượng ở tick 600: trung bình +7,32, SD 19,12, `d_z` 0,383, **âm ở chỉ 6/12
  seed**. Nghĩa là bức tranh quỹ đạo sớm nhìn từ riêng seed 700001 **không** khái quát được.

Cộng lại, (a) và (b) nói điều trung thực: **thí nghiệm này không đo được chiều của hiệu ứng não,
theo bất kỳ chiều nào.**

### Hai lỗi lịch trình do smoke tìm ra, sửa trước khi khoá T

Cả hai đều **không** thuộc đường não, và cả hai đều đã làm hỏng kết quả nếu để nguyên:

- `993a587` — `ecosystem_census_system` chụp ảnh `pool.animals` mà không khai báo thứ tự với các
  system dịch chuyển dự trữ của agent, nên nó báo cáo hoặc metabolism của tick này hoặc của tick
  trước, lệch 0,186 EU, do sort topo chọn **theo từng tiến trình**. Chạm `live.animals_eu` (H3) và
  `live.closed_eu_total` (H5).
- `ec94933` — **bảy** cặp system dịch chuyển EU không có thứ tự khai báo, nên checksum thế giới **và
  `live.mean_agent_energy`** dịch chuyển theo hash seed của tiến trình. Gate lẽ ra bắt được điều này
  thì **rỗng**: `ScheduleGraph::systems` trả về rỗng sau khi schedule đã khởi tạo.

Chấp nhận tái lập xuyên tiến trình được ghi tường minh trước khi chạy: **24 tiến trình độc lập** ở
T = 18.000 cho **một** kết quả duy nhất — checksum, cả 11 observable và cả 30 điểm chuỗi mẫu đều
trùng bit.

### Thí nghiệm tiếp theo cần gì (thay cho §8.1 chung chung)

1. **Một điểm đo trước sàn.** T ≈ 3.000–6.000 với genesis này, hoặc một metric tích phân trên cửa sổ
   còn thông tin thay vì lấy mẫu ở cuối. Chọn từ bằng chứng đã đo, và **đăng ký trước khi chạy**.
2. **Một quần thể có luân chuyển.** Hoặc cái chết vì đói, hoặc luồng tiến hoá nằm trong hợp đồng thí
   nghiệm, để `live.agent_count` có thể thay đổi và chọn lọc có cái để tác động.
3. **"Thời gian tới sàn" như một observable được đăng ký**, vì đó là tín hiệu chưa đăng ký mạnh nhất
   mà lần chạy này tạo ra.

### Ngôn ngữ

Trạng thái thế giới sống **không đổi**: **headless adapter verified**. Không có run desktop app nào,
không Tauri handle, không cửa sổ, không GPU, không renderer, không luồng nền. Không dòng nào ở đây
được trích dẫn thành "live world experiment-ready".

## Tài liệu bị ảnh hưởng

- Contract: [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md)
  — D02 mở rộng sang trạng thái nhận thức; D07 cần hoà giải nguồn seed.
- Decision: [`ADR-0001`](ADR-0001-creature-development-lifecycle.md) — không xung đột;
  `BrainGenotype` cố ý nằm ngoài `DevelopedPhenotype`.
- Research: [`MAP_AND_ML_UPGRADE_RESEARCH.md`](../research/MAP_AND_ML_UPGRADE_RESEARCH.md) §B1/B2/B4.
- Rules: [`SIMULATION_RULES.md`](../../SIMULATION_RULES.md) — luật zero-alloc, closed-EU.
- IPC: `PROJECT.md` "Interface Contracts" — payload hành động đổi kích thước.
- Planning/TODO: [`TODO.md`](../../TODO.md).
