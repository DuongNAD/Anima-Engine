---
title: ADR-0004 — Người quan sát nhập vai là một can thiệp được khai báo
status: proposed
owner: simulation-architecture
last_reviewed: 2026-07-26
decision_date: pending
supersedes: none
superseded_by: none
---

# ADR-0004 — Người quan sát nhập vai là một can thiệp được khai báo

## Bối cảnh

Anima Engine theo đuổi hai mục tiêu cùng lúc: một **dụng cụ nghiên cứu tiến hóa** (manifest →
run → checkpoint fork → đối chứng, có provenance nhân quả) và một **thế giới để trải nghiệm**
ở ngôi thứ nhất. Hai mục tiêu này không mâu thuẫn về đối tượng người dùng. Chúng mâu thuẫn ở
đúng một điểm kỹ thuật:

> Sự hiện diện của người quan sát **đã** là một lực tác động lên thế giới, và lực đó hiện
> không được khai báo ở đâu cả.

Điều này không phải suy đoán. Nó đã nằm trong code:

- [`LodFocus`](../../src-tauri/src/core/simulation_lod.rs) mang vị trí camera của người quan sát.
  [`SharedLodFocus`](../../src-tauri/src/core/simulation_lod.rs) để luồng lệnh Tauri ghi, và
  [`sync_lod_focus_system`](../../src-tauri/src/core/simulation_lod.rs) chép nó vào world mỗi tick.
- [`tier_at`](../../src-tauri/src/core/simulation_lod.rs) phân agent thành `Hot` / `Warm` / `Cold`
  theo khoảng cách tới focus đó, và
  [`should_infer`](../../src-tauri/src/core/simulation_lod.rs) quyết định agent có được suy nghĩ
  trong tick này không.
- Test [`cold_agents_stop_asking_entirely`](../../src-tauri/tests/simulation_lod_tests.rs) chốt
  rằng agent `Cold` **thật sự không suy nghĩ** — đây là chủ đích, đó chính là khoản tiết kiệm.

Ghép lại: **chỗ người chơi nhìn quyết định con nào được suy nghĩ.** Đây là hiệu ứng quan sát viên,
đã hiện diện, và nó nằm ngoài mọi cơ chế provenance hiện có.

Cần phân biệt hai tính chất thường bị gộp làm một:

| | Định nghĩa | LOD hiện tại |
|---|---|---|
| **Tái lập được** | Cùng đầu vào ⇒ cùng quỹ đạo | **Có** — [`tiering_is_reproducible`](../../src-tauri/tests/simulation_lod_tests.rs); tiering là hàm thuần của `(focus, entity_index, tick)`, không clock, không RNG |
| **Không nhiễu** | Bật/tắt không đổi quỹ đạo | **Không** — và không thể có, vì `Cold` không nghĩ là chủ đích |

Hôm nay run nghiên cứu vẫn sạch, nhưng chỉ nhờ một chi tiết: `LodFocus::default()` là
`enabled: false`, headless không có camera nào ghi vào nó. Sự sạch sẽ đó là **hệ quả phụ của việc
không có UI**, không phải một hợp đồng. Ngay khi có người quan sát nhập vai — và đó là mục tiêu sản
phẩm — nó mất.

[`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md) §2 liệt kê bốn nguồn rò rỉ thế giới
bên ngoài: `Uuid::new_v4()`, `SystemTime::now()`, Gemini, và thứ tự hệ thống của Bevy. ADR này gọi
tên **nguồn thứ năm: camera.**

Đọc kèm: [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md),
[`ADR-0002`](ADR-0002-world-laws-and-exotic-energy.md) (nguồn gốc của `InterventionCommand` và causal
ledger), [`SNAPSHOT_CONTRACT.md`](../reference/SNAPSHOT_CONTRACT.md).

## Động lực quyết định

- **Không được phá đường headless.** Run không có người quan sát phải bit-identical với hôm nay.
  Đây là điều kiện tiên quyết, không phải mục tiêu phấn đấu.
- **Nhiễu được phép, nhiễu giấu thì không.** Người chơi tác động lên thế giới là tính năng. Tác động
  đó không xuất hiện trong manifest, ledger hay fingerprint mới là lỗi.
- **Trải nghiệm nhập vai là thiết bị đo, không phải chi phí.** Open-endedness không đo được đáng tin
  bằng metric — mọi chỉ số đều bị nghiệm suy biến lách qua. Thứ phát hiện sự kiện thú vị trong lịch
  sử ngành đều là một con người nhìn thấy điều bất ngờ. Lớp nhập vai còn bắt được lỗi nguy hiểm nhất
  của mô phỏng có test xanh: đúng kỹ thuật, sai hiển nhiên.
- **Hot loop cấm cấp phát heap.** Ghi lại dấu vết người quan sát không được cấp phát trong tick path
  (test khẳng định `allocs == 0`).
- **Tương thích ngược schema.** Manifest cũ và JSON thiếu key phải nạp được nguyên vẹn — đúng cách
  [`exotic_interventions`](../../src-tauri/src/core/experiment.rs) đã làm với `#[serde(default)]`.
- **Không sinh ra thế giới thứ ba.** Dự án đã có vết nứt headless/live; giải pháp không được thêm một
  nhánh nữa.

## Các phương án

### Phương án A — Người quan sát là bóng ma (ghost-only)

Người quan sát không bao giờ ghi vào thế giới. `LodFocus` bị vô hiệu vĩnh viễn trong mọi run được
quan sát; camera chỉ đọc.

- **Ưu**: rẻ nhất, rủi ro bằng không, quỹ đạo luôn bằng headless.
- **Nhược**: giết nửa "trải nghiệm". Không nhập vai, không tương tác, và mất luôn khoản tiết kiệm LOD
  — nghĩa là thế giới xem được sẽ nhỏ hơn nhiều trên máy yếu.
- **Rollback**: không cần.

### Phương án B — Hai build tách biệt (game build / research build)

Một binary cho trải nghiệm, một cho thí nghiệm.

- **Ưu**: mỗi bên tự do tối ưu.
- **Nhược**: đây chính là vết nứt headless/live được **thể chế hóa**. Hai simulator sẽ trôi xa nhau,
  và điều người chơi thấy sẽ không còn là điều nhà nghiên cứu đo. Vi phạm trực tiếp động lực cuối.
- **Rollback**: rất đắt, vì lúc đó đã có hai codebase.

### Phương án C — Người quan sát là một can thiệp được khai báo *(chọn)*

Người quan sát vẫn nhập vai đầy đủ, nhưng mọi tác động của họ — kể cả **ánh nhìn** — đi qua đúng
những đường ống mà can thiệp khí hậu đã đi: khai báo trong manifest, cấp `CauseId`, ghi vào
[`CausalLedger`](../../src-tauri/crates/anima-domain/src/causal.rs), và tái lập được từ bản ghi.

- **Ưu**: giữ được cả hai mục tiêu; tận dụng hạ tầng đã có; biến "người chơi làm nhiễu" từ ô nhiễm
  thành **dữ liệu**.
- **Nhược**: thêm một trục schema; run có người quan sát không so trực tiếp được với run không có
  (chúng là hai treatment khác nhau) và UI phải nói rõ điều đó; phần replay phụ thuộc vào việc đường
  live trở nên tất định (xem §Kế hoạch).
- **Rollback**: `ObserverPolicy::Absent` là mặc định và là đường lui — bỏ key khỏi manifest thì mọi
  thứ trở về hôm nay.

### Giữ nguyên hiện trạng

`LodFocus` tiếp tục là một forcing không khai báo. Chừng nào chưa có UI nhập vai thì chưa lộ. Ngay
khi có, mọi phiên có người xem đều là một thí nghiệm bị nhiễm **trông y hệt** một thí nghiệm sạch —
cùng manifest, cùng fingerprint, khác quỹ đạo. Đây là dạng sai nguy hiểm nhất của dự án này: sai mà
mọi gate vẫn xanh.

## Quyết định

Chọn **Phương án C**, gồm bốn phần.

### C1 — Ba chính sách quan sát, khai báo trong manifest

Thêm vào [`ExperimentManifest`](../../src-tauri/src/core/experiment.rs), với `#[serde(default)]`
để manifest cũ nạp nguyên vẹn:

```rust
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ObserverPolicy {
    /// Không có người quan sát. Đường headless hôm nay, bit-identical. Mặc định và rollback.
    #[default]
    Absent,
    /// Có camera, chỉ đọc, và **tiering bị tắt**. Quỹ đạo phải bằng `Absent`.
    Spectate,
    /// Camera lái LOD focus và/hoặc người chơi hành động. Một treatment khác — không phải
    /// `Absent` bị hỏng.
    Inhabit { cause_id: CauseId, trace: TraceRef },
}
```

Ranh giới then chốt nằm giữa `Spectate` và `Inhabit`, không nằm giữa "xem" và "không xem":

- **`Spectate` phải chứng minh được là bằng `Absent`.** Người dùng xem toàn bộ thế giới ở ngôi thứ
  nhất, và run vẫn là chính cái run mà cluster đã chạy. Giá phải trả là mất tiết kiệm LOD, và đó là
  một đánh đổi được **khai báo** chứ không phải một bất ngờ.
- **`Inhabit` được phép làm mọi thứ**, với điều kiện mọi thứ đó nằm trong bản ghi.

`ObserverPolicy` đi vào
[`ExperimentManifest::fingerprint()`](../../src-tauri/src/core/experiment.rs) — nó là đầu vào
**có nghĩa**, nên nó phải lật fingerprint đúng như AE-S03 lật khi một luật đổi. Hệ quả cố ý: một run
có người nhập vai mang **danh tính khác**, không phải phiên bản nhiễm của cùng một danh tính.

Nhưng nó **không** được chạm vào
[`WorldLawSet::fingerprint()`](../../src-tauri/src/core/experiment.rs). Đây là cái bẫy gần nhất
của ADR này: người quan sát trông giống một điều kiện của thế giới, và nhét họ vào law set thì code
vẫn chạy, số vẫn hữu hạn. Nhưng [`ADR-0002`](ADR-0002-world-laws-and-exotic-energy.md) (ER01) buộc
`WorldLawSet` bất biến trong một run và một nhánh checkpoint **không bao giờ** đổi law fingerprint —
mà toàn bộ giá trị của C2 nằm ở chỗ fork được một checkpoint để bỏ người quan sát ra. Người quan sát
là **trạng thái/forcing**, cùng hạng với `exotic_interventions`, không phải luật.

### C2 — Dấu vết người quan sát là đầu vào hạng nhất

`focus` là **đầu vào duy nhất không tất định** của tiering: `tier_at` thuần theo
`(position, focus, bands)`, `should_infer` thuần theo `(tier, entity_index, tick, warm_interval)`.
Ghi lại `focus` thì tiering tất định trở lại. Đây là điểm tinh tế của cả ADR — không cần cấm camera,
chỉ cần **ghi nó xuống**.

```rust
/// Một mẫu quan sát tại một tick. Ghi khi đổi, không ghi mỗi tick.
pub struct ObserverSample {
    pub tick: u64,
    pub focus: LodFocus,
    pub actions: Vec<ObserverAction>,
}
```

Bản ghi (`ObserverTrace`) là artifact có checksum riêng, được manifest tham chiếu qua `TraceRef`
(id + checksum), không nhúng thẳng — một phiên một giờ ở 60 Hz là ~216k tick và manifest không phải
chỗ chứa nó.

Quy tắc nội suy: focus được ghi **khi đổi** kèm tick, và giá trị giữa hai mẫu được suy ra bằng một
phép nội suy **khai báo trong trace header**, không phải "đủ gần thì thôi". Một phép nội suy ngầm là
một nguồn phân kỳ ngầm.

Đổi lại, ba việc trở nên khả thi:

1. **Replay không cần người.** Đọc trace thay vì đọc camera sống ⇒ cùng quỹ đạo.
2. **Phản thực.** Fork checkpoint, bỏ trace ⇒ "thế giới sẽ ra sao nếu không ai đi qua".
3. **So cặp.** Hai nhánh cùng tổ tiên, khác đúng một biến: sự hiện diện của con người.

### C3 — Mọi hành động nhập vai mang một `CauseId`

> **Người quan sát không được ghi thẳng vào world state.**

Hành động nhập vai tạo ra một lệnh đi qua đúng cái seam mà
[`InterventionQueue`](../../src-tauri/crates/anima-domain/src/intervention.rs) đã dùng, và thay
đổi kết quả được ghi bằng
[`CausalLedger::record`](../../src-tauri/crates/anima-domain/src/causal.rs) với `cause_id` của
người quan sát. Vì `record` đã có quy tắc "cause của parent luôn thắng", cả chuỗi hệ quả phía sau tự
động thừa kế gốc — và
[`trace_to_root`](../../src-tauri/crates/anima-domain/src/causal.rs) trả lời được:

> Đàn thú này tuyệt chủng vì một con người đã đi qua ở tick 40 231.

`ObserverAction` là **enum riêng, không nhồi vào**
[`InterventionKind`](../../src-tauri/crates/anima-domain/src/intervention.rs).
`InterventionKind` có hình dạng *vùng + cường độ + đường cong*, hợp với forcing khí hậu và hoàn toàn
không hợp với "tôi vừa ăn một quả". Nơi một hành động **thật sự** mang hình dạng vùng (đốt một khu
rừng), nó hạ xuống thành một `InterventionCommand` bình thường và dùng lại
[`validate_intervention`](../../src-tauri/src/core/experiment.rs). Hai kênh, một `CauseId`
namespace, một ledger.

`CAUSE_BACKGROUND = 0` giữ nguyên nghĩa; người quan sát nhận id từ dải riêng để một hiệu ứng do người
gây ra không bao giờ bị đọc nhầm thành động lực nền.

### C4 — Người quan sát đổi nhịp *xem*, không đổi nhịp *tính*

Tiến hóa cần hàng vạn thế hệ; một phiên chơi có 40 phút. Quy tắc:

> **Tua nhanh là render ít khung hình hơn trên cùng số tick, không phải bỏ tick.**

Bỏ tick không phải tua nhanh — đó là một thế giới khác.
[`SimClock::fires(tick, period)`](../../src-tauri/crates/anima-domain/src/sim_clock.rs) đã thuần
theo tick, nên nhịp trình chiếu tách được khỏi nhịp mô phỏng mà không đụng vào quỹ đạo. Hệ số tua đi
vào trace header (nó đổi *khi nào* người chơi có mặt để tác động), không đi vào lịch trình tick.

## Hệ quả

### Tích cực

- Trải nghiệm nhập vai trở thành **lớp observability cho đúng bài toán mà metric bó tay**, thay vì
  một khoản chi phí bên lề.
- Nhiễu do người chơi trở thành dữ liệu có thể fork và phản thực. Một phiên chơi **sinh ra** một thí
  nghiệm.
- Nguồn rò rỉ thứ năm được gọi tên và đóng lại; `DETERMINISM_CONTRACT` §2 đầy đủ hơn.
- Bản sắc sản phẩm rõ ràng và (theo hiểu biết hiện tại) chưa có tiền lệ: **một thế giới nơi sự hiện
  diện của người quan sát là một can thiệp được ghi lại, phát lại và rẽ nhánh được.**
- Không sinh nhánh mới: một world, một bộ luật, ba chính sách quan sát.

### Tiêu cực / rủi ro

- **Rủi ro lớn nhất là so sánh sai.** Run `Inhabit` và run `Absent` là hai treatment khác nhau. Nếu
  UI xếp chúng cạnh nhau như hai lần chạy của cùng một thí nghiệm, ADR này tạo ra đúng loại tự tin
  sai mà nó định ngăn. Fingerprint khác nhau là hàng rào kỹ thuật; nhãn trong UI là hàng rào còn lại.
- **`Inhabit` replay bị chặn bởi tính tất định của đường live.**
  [`DETERMINISM_CONTRACT`](../reference/DETERMINISM_CONTRACT.md) §5 nói rõ: physics/CPG chạy song song
  trong schedule live và **không** tái lập được; gate hiện tại không đi qua `SimulationEngine::start`.
  Nên phần replay của C2 chỉ có nghĩa sau khi việc đó xong. Việc đó mang **hai tên** trong tài liệu
  hiện hành — **G2** (task 1 / gate #1, theo `DETERMINISM_CONTRACT` §5 và kế hoạch G0–G4) và
  **§3.3 + §3.6** (theo [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md)). Cùng một
  việc; giữ cả hai tên khi tra cứu.
- Kích thước trace: cần đo thật. Ghi-khi-đổi giúp nhiều vì focus trơn từng khúc, nhưng một người chơi
  quay camera liên tục là trường hợp xấu nhất và phải benchmark, không phải phỏng đoán.
- `Spectate` mất tiết kiệm LOD ⇒ số agent xem được cùng lúc giảm. Trên máy yếu đây là ràng buộc thật.
  Có thể có `Spectate` dùng tiering **cố định theo lưới** (không theo camera) như bước tiếp, nhưng
  không thuộc ADR này.
- Thêm bề mặt schema cần bảo trì và cần bump `MANIFEST_SCHEMA_VERSION`.

## Kế hoạch triển khai và hoàn tác

Thứ tự có chủ ý: mọi thứ không phụ thuộc G2 làm trước, để giá trị đến sớm mà không chờ.

1. **Baseline.** Chốt checksum hiện tại từ `determinism_gate_tests.rs`
   (`0xe4c7f5e9`, control âm `0xe66c09d2`) làm mốc "không đổi gì".
2. **O1 — `ObserverPolicy` + `Absent`/`Spectate`.** Không cần trace, không cần G2. Gate quyết định là
   `spectate_matches_absent`. Đây là bước làm cho "vừa nghiên cứu vừa trải nghiệm" thành **lời hứa
   kiểm chứng được** thay vì khẩu hiệu, và nó khả thi ngay hôm nay.
3. **O2 — Ghi trace + `CauseId` cho hành động.** Ledger và provenance chạy được kể cả khi replay chưa
   bit-exact: "vì sao đàn thú chết" là câu hỏi trả lời được trước khi "chạy lại y hệt" trả lời được.
4. **O3 — `Inhabit` replay.** *Phụ thuộc G2 = §3.3 + §3.6* (đường live tất định). Không tuyên bố
   replay trước mốc này.
5. **O4 — Fork phản thực + so cặp trong UI**, dùng lại đường checkpoint của AE-209.
6. **Hoàn tác.** Bỏ key `observer` khỏi manifest ⇒ `ObserverPolicy::Absent` ⇒ đường hôm nay. Trace cũ
   là artifact rời, xóa không ảnh hưởng run cũ.

## Bằng chứng xác minh

| Gate | Lệnh / artifact | Ngưỡng | Kết quả |
|---|---|---|---|
| Correctness | `observer_writes_go_through_the_intervention_seam` | Không đường ghi world state nào khác từ observer | pending |
| Correctness | Manifest cũ (không có key `observer`) nạp và validate | Bằng hành vi hôm nay | pending |
| Determinism | `spectate_matches_absent` — hai tiến trình độc lập, cùng manifest, một `Spectate` có camera path | Cùng checksum | pending |
| Determinism | **Control âm**: cùng camera path, `Inhabit` | Checksum **phải khác** — nếu bằng thì trace không tác dụng gì và gate trên xanh vì lý do sai | pending |
| Determinism | `an_inhabited_run_replays_from_its_trace_without_a_human` *(sau G2)* | Bằng checksum phiên sống | pending |
| Provenance | `trace_to_root` trên hệ quả sau một hành động observer | Trả về `CauseId` của observer, không phải `CAUSE_BACKGROUND` | pending |
| Performance | Bench tick path khi đang ghi trace | `allocs == 0`; regression tick time trong ngưỡng `BENCHMARK_BASELINE.md` | pending |
| Performance | Kích thước trace, phiên 1 giờ, trường hợp camera quay liên tục | Đo thật, đặt ngưỡng sau lần đo đầu | pending |
| License/security | Không dependency mới | n/a | pending |

Control âm là bắt buộc theo `DETERMINISM_CONTRACT` §4. Ở đây nó đặc biệt quan trọng: không có nó,
một `Spectate` cài đặt sai thành "y hệt `Absent` vì trace bị bỏ qua hoàn toàn" sẽ làm gate xanh.

## Tài liệu bị ảnh hưởng

- **Contract/reference**: [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md) (§2 thêm
  nguồn rò rỉ thứ năm: camera; §5 thêm phụ thuộc O3 → G2);
  [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) (chính sách
  quan sát là một phần của danh tính run);
  [`SNAPSHOT_CONTRACT.md`](../reference/SNAPSHOT_CONTRACT.md) (`TraceRef` trong saved state).
- **Kiến trúc**: [`PROJECT.md`](../../PROJECT.md) — `set_lod_focus` nhận thêm chính sách; bảng IPC cần
  mô tả rằng focus giờ là một kênh được ghi.
- **How-to/tutorial**: hướng dẫn chạy thí nghiệm cần nói rõ khi nào dùng `Spectate` và vì sao
  `Inhabit` không so trực tiếp được với `Absent`.
- **Planning/TODO**: O1–O4 vào backlog của
  [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) (tài liệu sống, đã ghi ADR này ở
  §3.8) và một dòng nhật ký ở [`TODO.md`](../../TODO.md); O3 gắn phụ thuộc §3.3 + §3.6.
- **Inventory/NOTICE**: không đổi (không dependency mới).
