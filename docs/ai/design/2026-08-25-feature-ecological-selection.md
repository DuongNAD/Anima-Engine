---
phase: design
feature: ecological-selection
title: Design — Đóng vòng sinh thái
description: Năm nhánh, điểm chèn cụ thể, và lý do từng quyết định thiết kế
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
requirements: ../requirements/2026-08-25-feature-ecological-selection.md
planning: ../planning/2026-08-25-feature-ecological-selection.md
testing: ../testing/2026-08-25-feature-ecological-selection.md
contract: ../../reference/ENERGY_LEDGER_CONTRACT.md
---

# Design — Đóng vòng sinh thái

> Bối cảnh và bảng bằng chứng T1–T15:
> [Requirements](../requirements/2026-08-25-feature-ecological-selection.md).

Năm nhánh: **M** tử vong · **F** fitness · **Q** trục mô tả · **R** sinh sản · **O** quan sát ·
**H** hiệu năng. (Sáu chữ, năm nhánh khoa học cộng một nhánh kỹ thuật; H không đổi hành vi nào.)

---

## 0. Nguyên tắc chung của cả feature

Bốn quy tắc dưới đây áp cho mọi nhánh. Vi phạm một trong bốn là lý do đủ để từ chối một bản vá,
kể cả khi test xanh.

1. **Mọi hành vi mới nằm sau một cờ, và cờ tắt là hành vi hôm nay, trùng bit.** Không phải "gần
   giống", không phải "trong dung sai". Trùng bit. Đây là điều duy nhất giữ cho mọi artifact thí
   nghiệm đã ghi còn giá trị.
2. **Không thêm một đường chuyển năng lượng mới nào.** Mọi thứ đi qua các nguyên hàm đã có
   trong `energy_ledger` và qua `ReclaimAndDespawnAgentCommand`. Một phép trừ trực tiếp trên
   `homeo.energy` bên ngoài chúng sẽ rò EU trong khi trông hoàn toàn hợp lý.
3. **Đọc `AgentEvaluation`, đừng viết lại nó.** Đại lượng cần cho fitness sinh thái đã được tính
   đúng mỗi tick từ trước feature này. Viết một bộ đếm thứ hai là tạo ra hai nguồn sự thật.
4. **Nhánh H là refactor thuần.** Nếu checksum đổi sau một thay đổi thuộc H, đó là bug của H,
   không phải "hệ quả phụ chấp nhận được".

---

## 1. Nhánh M — Tử vong

### 1.1 Vấn đề thiết kế thật

Cái khó không phải "làm sao cho agent chết". Cái khó là **chết ở đâu trong lịch trình**.

`ReclaimAndDespawnAgentCommand` giải quyết được vấn đề nguyên tử vì nó chạy tại sync point:
reclaim và despawn xảy ra cùng một lúc, nên không có cửa sổ nào để một hệ thống khác quan sát một
dự trữ đã ghi sổ nhưng chưa bị huỷ. Doc comment của nó mô tả đúng chế độ hỏng cũ, và chế độ hỏng
đó sẽ quay lại nguyên vẹn nếu ai đó tự trừ năng lượng rồi gọi despawn riêng.

Vì vậy nhánh M **không** thêm cơ chế. Nó chỉ thêm **điều kiện gọi** cơ chế đã có.

### 1.2 Trạng thái cần thêm

```
HomeostaticState {
    …,
    /// Số tick liên tục ở đáy. Reset về 0 ngay khi ăn hoặc uống được.
    /// #[serde(default)] — snapshot cũ nạp lại thành 0, đúng nghĩa "chưa đói".
    starving_ticks: u32,
}
```

Một `u32` trên component đã tồn tại, đã serialize, đã round-trip. Không resource mới, không kho
song song, không HashMap phụ. Đây là lựa chọn có chủ ý: mọi thứ khác đều cần thêm một đường
save/restore riêng, và mỗi đường như vậy là một chỗ để trạng thái lệch nhau sau checkpoint.

### 1.3 Điểm chèn

| Việc | Nơi | Ghi chú |
|---|---|---|
| Tăng/reset `starving_ticks` | cuối `metabolic_decay_system`, [`world_systems.rs:121-178`](../../../src-tauri/src/core/world_systems.rs) | đúng chỗ đã biết `homeo.energy` sau khi trừ |
| Quyết định chết | hệ thống mới `mortality_system`, chạy **sau** `metabolic_decay_system` và **sau** mọi hệ thống cho ăn | thứ tự này là bắt buộc, xem 1.4 |
| Thực thi | `commands.add(ReclaimAndDespawnAgentCommand { root })` | không đường nào khác |
| Ghi lịch trình | [`simulation_schedule.rs`](../../../src-tauri/src/core/simulation_schedule.rs) `build_tick_schedule` | **một chỗ duy nhất**; test khai `.chain()` riêng là test một lịch trình app không chạy |

### 1.4 Thứ tự trong tick, và tại sao nó không tuỳ chọn

Lịch trình hiện tại đã khai một chuỗi năng lượng tường minh: `herbivore_grazing →
resource_field_regrowth → detect_food_collisions → combat → metabolic_decay → ecosystem_census`.
Comment tại chỗ ghi lý do: các thứ tự này **không giao hoán** — ăn trước hay đốt trước quyết định
một con có chết đói hay không.

`mortality_system` phải nằm **sau `metabolic_decay_system`** và **trước `ecosystem_census_system`**:

- sau trao đổi chất, vì một con vừa ăn xong không được chết vì số đo của tick trước;
- trước tổng kiểm kê, vì census phải đếm thế giới sau khi xác đã trở thành detritus, không phải
  một thế giới nửa vời.

Đặt nó ở chỗ khác thì residual vẫn có thể bằng 0 — và kết quả vẫn sai, vì nó sai về *thời điểm*
chứ không sai về *tổng*. Đây chính là loại lỗi mà chuỗi khai báo kia sinh ra để chặn.

### 1.5 Tham số

Đặt trong resource `MortalityPolicy`, không phải hằng số rải rác:

```
MortalityPolicy {
    enabled: bool,           // ANIMA_MORTALITY, mặc định false
    starve_ticks: u32,       // số tick ở đáy trước khi chết
    dehydration_ticks: u32,
}
```

**Thiếu resource ≠ tắt.** Theo đúng bẫy đã ghi ở [ADR-0004](../../decisions/ADR-0004-observer-as-declared-intervention.md)
về `ObserverPolicy`: thiếu nghĩa là *chưa ai khai báo*, và phải giữ hành vi cũ. Ở đây hành vi cũ
là không chết, nên thiếu resource và `enabled: false` tình cờ trùng nhau — nhưng hãy viết
`Option<Res<MortalityPolicy>>` và xử lý `None` tường minh, đừng dựa vào sự trùng hợp đó.

### 1.6 Điều nhánh M cố ý KHÔNG làm

- **Không** cho năng lượng ảnh hưởng tới khả năng vận động. Đó là một thay đổi hành vi riêng, có
  rủi ro riêng, và trộn nó vào đây sẽ làm không ai biết hiệu ứng đo được đến từ đâu.
- **Không** đụng `aggregate_population`. Invariant "starvation must not kill" ở đó nói về **quần
  thể tổng hợp** của AE3, một mô hình khác, có hợp đồng khác. Sửa nó là sửa nhầm mô hình.

---

## 2. Nhánh F — Fitness sinh thái

### 2.1 Hình dạng: một trait, hai bản cài, một cờ

```
trait FitnessRule {
    fn score(&self, eval: &AgentEvaluation, tracker: &FeatureTracker,
             homeo: &HomeostaticState, genotype: &MorphologyGenotype) -> f32;
    fn id(&self) -> &'static str;   // đi vào provenance
}
```

- `LocomotionFitness` — trả **đúng** `tracker.cumulative_distance + tracker.tick_count as f32`.
  Không "tương đương", không viết lại cho gọn. Đây là dòng chứng minh N3.
- `EcologicalFitness` — đọc `AgentEvaluation`.

Chọn bằng `ANIMA_FITNESS=locomotion|ecological`, mặc định `locomotion`.

### 2.2 `EcologicalFitness` chấm bằng gì

Đề xuất, và lý do từng số hạng:

```
score = survival_ticks                          // sống bao lâu mà KHÔNG ở đáy (T6)
      + w_surplus * max(0, energy_gained - energy_spent)   // thặng dư ròng
```

- `survival_ticks` là đại lượng duy nhất trong crate đã có điều kiện sống gắn sẵn. Nó tăng chỉ khi
  `energy > 0 && hydration > 0` — đúng nghĩa "số tick sống được", và nó đã được tính từ trước.
- Thặng dư ròng thưởng cho việc **kiếm được nhiều hơn tiêu**, tức là thưởng cho hiệu quả kiếm ăn
  chứ không thưởng cho việc chạy vòng quanh.
- **Không có số hạng quãng đường.** Đây là điểm mấu chốt: nếu quãng đường quay lại trong công thức,
  P-04 (trục QD trùng fitness) tự động quay lại cùng nó.

Trọng số `w_surplus` là tham số thế giới, không phải hằng số biên dịch.

### 2.3 Điểm chèn, và cái bẫy ở đó

[`agent_systems.rs:523`](../../../src-tauri/src/core/agent_systems.rs) hiện bind
`_eval, _homeo` — hai giá trị bị vứt. Bỏ dấu gạch dưới và truyền chúng vào `FitnessRule`.

> ⚠️ **Bẫy.** Query hiện tại đã lấy sẵn `&AgentEvaluation` và `&HomeostaticState`. Đừng thêm một
> query thứ hai; đừng đổi `iter_mut` thành `iter`. Query này viết vào `FeatureTracker` để reset
> nó, nên nó phải giữ `mut`. Thêm một query trùng lặp lên cùng component sẽ tạo xung đột truy cập
> mà Bevy phát hiện lúc chạy, không phải lúc biên dịch.

### 2.4 Provenance

`id()` của luật đang dùng phải xuất hiện ở ba chỗ: `ExperimentManifest`, export của
`tick_capture`, và bản ghi provenance của ensemble. Một kết quả không nói được nó được chấm bằng
luật nào là một kết quả không dùng được — và đây là chỗ rẻ nhất để bảo đảm điều đó.

---

## 3. Nhánh Q — Trục mô tả độc lập với fitness

### 3.1 Vì sao trục hiện tại hỏng

MAP-Elites hoạt động được nhờ mô tả hành vi **độc lập** với fitness; đó là cách nó soi sáng một
không gian thay vì leo một đỉnh. Ở đây trục thứ hai `foraging_range` bằng đúng
`cumulative_distance`, còn fitness bằng `cumulative_distance + tick_count` — trục là fitness trừ
một hằng số. Kho không hỏng, nó chỉ khám phá **một** chiều thay vì hai, và doc comment tại
[`ecology.rs:163-168`](../../../src-tauri/src/core/ecology.rs) mô tả một điều chưa xảy ra.

### 3.2 Trục thay thế

Giữ trục 1 (**khối lượng cơ thể**) — nó là tính trạng chủ đạo của Metabolic Theory, độc lập với
mọi fitness đề xuất, và không có gì sai với nó.

Đổi trục 2 sang một mô tả **cách sống**. Hai ứng viên, chọn một:

| Ứng viên | Định nghĩa | Ưu | Nhược |
|---|---|---|---|
| **Q-a · Bậc dinh dưỡng thực tế** | tỉ lệ EU nạp vào đến từ săn mồi so với tổng EU nạp vào | trực tiếp là *ổ sinh thái*; số liệu đã đi qua ledger | cần một bộ đếm nạp-theo-nguồn trên agent |
| **Q-b · Độ chuyên hoá biome** | phân tán của thời gian ở các biome, chuẩn hoá | không cần đếm gì mới ngoài một histogram nhỏ | ít trực tiếp hơn về mặt sinh thái |

**Khuyến nghị Q-a.** Nó là trục duy nhất trong hai cái mà một người đọc kết quả sẽ gọi tên được
bằng ngôn ngữ sinh thái: "kho này chứa từ con ăn cỏ thuần tới con ăn thịt thuần, ở mọi cỡ cơ thể".
Đó chính là điều doc comment cũ hứa.

### 3.3 Tương thích

Đổi trục là **đổi hợp đồng** của kho lưu trữ. Một archive cũ nạp vào hệ trục mới sẽ đặt cá thể vào
ô sai và không có gì báo lỗi. Vì vậy:

- thêm `descriptor_set_id` vào dữ liệu archive đã lưu;
- `MapElitesArchive::from_saved` **từ chối** id không khớp, có thông báo, thay vì nạp im lặng;
- test: nạp archive `v1` vào bản dựng `v2` phải là lỗi, không phải một kho lệch.

---

## 4. Nhánh R — Sinh sản theo ngưỡng năng lượng

> Đây là nhánh rủi ro nhất. Nó chỉ nên bắt đầu sau khi M và F đã xanh, vì nó **giả định** cả hai.

### 4.1 Nguyên tắc: sinh sản là một giao dịch năng lượng

Cá thể có `energy >= repro_threshold` thì sinh. Chi phí là năng lượng **thật**:

```
parent.energy  -= offspring_endowment      // qua debit_reserve
child.energy   += offspring_endowment      // qua credit_reserve, tại cùng sync point
```

Vì cả hai vế đi qua nguyên hàm của ledger tại **một** sync point, EU đóng theo cấu trúc —
đúng khuôn của `ReclaimAndDespawnAgentCommand`, chỉ ngược chiều. Một `SpawnOffspringCommand` mới
phải theo đúng khuôn đó: **một** `Command`, không phải hai bước qua `Commands`.

### 4.2 Trần quần thể

`max_population` cứng, khai báo được. Chạm trần thì **không** sinh, và ghi một counter
`births_suppressed_by_cap`. Một trần im lặng là một trần mà không ai biết đã chạm phải, và nó sẽ
làm mọi kết luận về động lực học quần thể sai một cách không thể phát hiện được.

### 4.3 Không chạy chồng với thay thế theo epoch

`ANIMA_REPRODUCTION=threshold` và thay thế theo epoch là **loại trừ nhau**. Bật cả hai phải là lỗi
cấu hình ở preflight, không phải một thế giới mà cả hai cùng chạy — vì cái sau thay toàn bộ quần
thể bất kể cái trước vừa làm gì.

### 4.4 Lineage

`add_reproduction` **từ chối** ghi cạnh có cha không tồn tại, và đó là hành vi đúng: một cạnh mồ
côi phá toàn bộ đồ thị phả hệ, vì cả `to_newick` lẫn `simplify` từ chối xử lý đồ thị chứa nó
(bẫy đã ghi ở [STATE_OF_THE_PROJECT §3.0](../../planning/STATE_OF_THE_PROJECT.md)). Với sinh sản
theo ngưỡng, cha **luôn** tồn tại tại thời điểm sinh, nên hãy ghi cạnh trong **cùng** command,
không phải ở một hệ thống sau đó.

---

## 5. Nhánh O — Quan sát được tiến hoá

### 5.1 Vấn đề: trung bình không phát hiện được tiến hoá

Mười một observable hiện tại đều là tổng hoặc trung bình. **Trung bình quần thể có thể đứng yên
trong khi phân bố tách đôi** — đó là chính xác hình dạng của một sự kiện phân hoá, và engine hôm
nay mù với nó.

### 5.2 Observable thêm vào

| Khoá | Nội dung | Nguồn |
|---|---|---|
| `live.trait_mass_mean` · `_var` · `_p10` · `_p90` | phân bố khối lượng cơ thể | duyệt agent, đã có |
| `live.births` · `live.deaths` | luân chuyển trong cửa sổ | nhánh M và R |
| `live.mean_lifespan` | tuổi thọ trung bình của các ca tử trong cửa sổ | nhánh M |
| `live.shannon` · `live.simpson` | đa dạng | **đã tính** tại [`simulation_loop.rs:2192`](../../../src-tauri/src/core/simulation_loop.rs), chỉ cần nối |
| `live.archive_coverage` | tỉ lệ ô MAP-Elites đã lấp | archive, đã có |

### 5.3 Hai quy tắc bắt buộc

1. **`evolution.births` là tích luỹ nên mang `Aggregation::Instant`, không phải `Sum`** — quy tắc
   đã ghi ở [EVOLUTION_EXPERIMENT_CONTRACT](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md).
   `live.births` ở đây là **đếm theo cửa sổ**, khác hẳn; đừng chép nhầm quy tắc tổng hợp.
2. **Observable chỉ phát ra khi cấu trúc sinh ra nó tồn tại.** Một manifest xin `live.deaths` trên
   một run tắt tử vong phải **fail ở preflight**. Trả về 0 là tệ hơn fail: nó trông như một phép đo.

---

## 6. Nhánh H — Hiệu năng, refactor thuần

### 6.1 Mẫu đã có trong repo

`combat_system` tại [`world_systems.rs:300-340`](../../../src-tauri/src/core/world_systems.rs) làm
đúng như sau, và nó là bản mẫu để chép:

1. dựng `centroid_index: Local<EntityHashMap<(bool, usize)>>`, `clear()` chứ không `new()`;
2. một lượt qua danh sách agent để lấp chỉ mục;
3. **một** lượt qua toàn bộ segment, tra chỉ mục để biết segment thuộc về ai.

Tổng: O(A + S) thay vì O(A · S), và không cấp phát nào trong tick path vì `Local` giữ bộ đệm qua
các tick.

### 6.2 Bảy chỗ áp mẫu

| Hệ thống | Vị trí | Hình dạng hiện tại |
|---|---|---|
| `metabolic_decay_system` | [`world_systems.rs:121`](../../../src-tauri/src/core/world_systems.rs) | O(A·S) |
| `update_agent_evaluation_system` | [`agent_systems.rs:468`](../../../src-tauri/src/core/agent_systems.rs) | O(A·S) |
| `detect_food_collisions_system` | [`world_systems.rs:236`](../../../src-tauri/src/core/world_systems.rs) và `:248` | O(A·S) rồi O(A·F) |
| `spawn_food_system` lân cận | [`world_systems.rs:337`](../../../src-tauri/src/core/world_systems.rs) | O(A·S) |
| hai chỗ ở migration | [`world_systems.rs:436`](../../../src-tauri/src/core/world_systems.rs), `:454` | O(A·S) |
| `detect_environmental_collisions_system` | [`environmental_systems.rs:282`](../../../src-tauri/src/core/environmental_systems.rs) | O(A·S) |

### 6.3 `sensory_system` — dùng lưới đã dựng sẵn

[`agent_systems.rs:877`](../../../src-tauri/src/core/agent_systems.rs) và `:890` quét tuyến tính
toàn bộ mồi / thức ăn cho **từng** agent, trong khi `SpatialHashGrid` đã được dựng lại mỗi tick và
đang được dùng cho raycast **ngay bên dưới trong cùng hàm**. Đây là chỗ dễ nhất trong cả nhánh H:
hạ tầng đã có, chỉ là chưa gọi.

### 6.4 Điều kiện nghiệm thu của H, và nó nghiêm hơn các nhánh khác

Vì H là refactor thuần, nó có một gate mà các nhánh khác không có: **cùng seed ⇒ cùng checksum,
trước và sau**. Nếu checksum đổi, thay đổi đã đổi ngữ nghĩa ở đâu đó — thường là thứ tự duyệt,
vì `EntityHashMap` không duyệt theo thứ tự chèn. Chỗ nào thứ tự duyệt ảnh hưởng tới kết quả (ví dụ
ai giành được miếng ăn cuối), phải sắp xếp tường minh, không dựa vào thứ tự của map.

---

## 7. Rủi ro đã biết

| Rủi ro | Chỗ nó sẽ xuất hiện | Cách chặn |
|---|---|---|
| Rò EU khi thêm cái chết | trừ năng lượng ngoài `ReclaimAndDespawn` | gate residual chạy trên **mọi** cấu hình cờ |
| Quần thể tuyệt chủng ngay | ngưỡng chết quá gắt so với NPP | tham số hoá, và một test "quần thể sống sót ở NPP mặc định" |
| Quần thể bùng nổ | nhánh R không trần | trần cứng + counter, xem 4.2 |
| Checksum lệch ở nhánh H | thứ tự duyệt `EntityHashMap` | gate checksum trước/sau, sắp xếp tường minh chỗ có tranh chấp |
| Kết luận sai vì không biết luật fitness nào đang chạy | thiếu provenance | `id()` vào manifest + capture + provenance |
| Sửa nhầm `anima-core` | có hai `MorphologyGenotype` trong cây | xem bẫy 1 ở [bản giao việc](../planning/2026-08-25-agent-goal-ecological-selection.md) |
