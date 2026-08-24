---
title: Hợp đồng tất định (deterministic mode)
status: active
owner: maintainers
last_reviewed: 2026-07-25
---

# Hợp đồng tất định

Code: `src-tauri/src/core/determinism.rs`. Gate: `src-tauri/tests/determinism_gate_tests.rs`.
Đọc kèm: `docs/reference/SNAPSHOT_CONTRACT.md` (G1.2), `CREATURE_DEVELOPMENT_CONTRACT.md` (**D07**).

## 1. Lời hứa, và giới hạn của nó

> **Cùng manifest + cùng build ⇒ cùng quỹ đạo.**

Đây **không** phải lời hứa bit-identical giữa các target hay mức tối ưu hoá: float reassociation
biến đó thành một lời hứa khác và lớn hơn nhiều. `snapshot::BuildProvenance` ghi target và profile
chính xác để một sai lệch giữa hai máy có thể **quy trách nhiệm** thay vì gây bối rối.

## 2. Năm nguồn rò rỉ thế giới bên ngoài

| Nguồn | Rò rỉ |
|---|---|
| `Uuid::new_v4()` — lineage id, chronicle id, offspring id | entropy của OS |
| `SystemTime::now()` — chronicle timestamp | đồng hồ treo tường |
| Gemini (`evolution::meta_ai`) | mạng, một secret, và output của một model từ xa |
| **Thứ tự chạy hệ thống của Bevy** | lịch trình của executor đa luồng |
| **Camera của người quan sát** (`LodFocus`) | chỗ một con người đang nhìn |

Nguồn thứ tư dễ bị bỏ sót nhất vì trông như chi tiết triển khai chứ không phải *đầu vào*. Bevy đảm
bảo hai hệ thống tranh chấp không chạy cùng lúc, nhưng **không** đảm bảo cái nào chạy trước. Đây
không phải lo lắng lý thuyết: G1.1 phát hiện một residual năng lượng **đổi dấu giữa các lần chạy** vì
nó, và gate của G1.2 phải tự khai báo thứ tự mới có được checksum ổn định.

### 2.1 Nguồn thứ năm: camera (ADR-0004)

Thêm vào 2026-07-26. Nó bị bỏ sót lâu nhất vì không trông giống một nguồn ngẫu nhiên nào cả — nhưng
`tier_at` phân agent theo khoảng cách tới `LodFocus`, và một agent `Cold` **không suy nghĩ**
(`cold_agents_stop_asking_entirely`). Nên **chỗ người dùng nhìn quyết định con nào được suy nghĩ**.

Khác bốn nguồn trên ở một điểm quan trọng: nó **tái lập được nhưng có nhiễu**. Tiering là hàm thuần
của `(focus, entity_index, tick)` — không clock, không RNG — nên cùng một đường camera luôn cho cùng
kết quả. Cái nó không làm được là *không đổi gì*, và không thể làm được: `Cold` bỏ suy nghĩ chính là
khoản tiết kiệm. Gộp "tái lập được" với "không nhiễu" là cái bẫy ở đây.

Vì thế nó không bị tắt như ba nguồn đầu, mà bị **khai báo**
([`ObserverPolicy`](../../src-tauri/src/core/observer.rs)):

| Policy | Camera có tier không | Quỹ đạo |
|---|---|---|
| `Absent` | không | đường headless |
| `Spectate` | không | **bắt buộc bằng** `Absent` — gate `spectate_matches_absent` |
| `Inhabit` | có | một **treatment khác**, không phải bản nhiễm của cùng một run |

Hai điều dễ sai, cả hai đã có test giữ:

- **Thiếu resource `ObserverPolicy` ≠ `Absent`.** Thiếu = chưa ai khai báo ⇒ tuân theo camera, giữ
  nguyên hành vi tiền-ADR. `Absent` là khai báo ngược lại ⇒ cấm camera.
- **Từ chối focus là thay trọn `LodFocus::default()`, không chỉ tắt `enabled`.** Giữ lại `center` sẽ
  để nguyên một đường camera sống bên trong world cho bất kỳ system nào sau này đọc phải.

Một run `Inhabit` được ghi lại qua [`ObserverTrace`](../../src-tauri/src/core/observer.rs) và cắm rễ
ở `CAUSE_OBSERVER`. Bản ghi đó **chưa** đủ để replay: replay cần đường live tất định, tức mục §5 dưới
đây. Provenance thì không phải chờ.

## 3. Công tắc

`DeterministicMode`, **mặc định tắt**, bật bằng `ANIMA_DETERMINISTIC` (khác `0`/`false`/rỗng).

Mặc định tắt là có chủ ý: một phiên tương tác muốn uuid thật và timestamp thật — chronicle là log
hướng người dùng, đóng dấu nó bằng thời gian suy ra từ tick là nói dối trong ngữ cảnh đó. Thí nghiệm
muốn điều ngược lại. "Chưa đặt" luôn nghĩa là "hành xử như trước".

Khi bật:

- **Id** lấy từ `RunIdentity::next_id` → `"<prefix>-<run_id:016x>-<counter:08x>"`. Hex zero-pad nên
  id sắp xếp theo thứ tự cấp phát, làm lineage graph đọc được. Mỗi luồng có **namespace riêng** nên
  hai nguồn id đồng thời không đụng nhau mà không cần khoá trên hot path.
- **Timestamp** lấy từ `tick_timestamp_ms(tick)` — hàm thuần của tick và `sim_rules::TICK_HZ`,
  gốc `DETERMINISTIC_EPOCH_MS`.
- **AI ngoài** không được gọi: `allows_external_ai()` trả `false`, cả `GeminiMetaAiClient` lẫn
  `GeminiWebSessionClient` rơi về `MockMetaAiClient` — một hàm thuần của epoch và history. Hợp đồng
  là AI ngoài chỉ được **đề xuất** intervention, được đóng băng vào manifest và replay từ đó; tới lúc
  replay thì không còn gì để hỏi.
- **Schedule** chạy `ExecutorKind::SingleThreaded`. Executor đơn luồng đi theo thứ tự topo của
  schedule — hàm của các ràng buộc đã khai báo và thứ tự chèn, nên cùng binary + cùng manifest cho
  cùng thứ tự mọi lần. Đánh đổi song song, đúng giá phải trả cho một run mà mục đích là tái lập được.

## 4. Gate

**Hai tiến trình độc lập**, không phải hai world trong một tiến trình. Điều này quan trọng:

- Thứ tự lặp `HashMap`/`HashSet` đến từ `RandomState`, tự gieo **một lần mỗi tiến trình** từ OS.
  Hai world trong cùng tiến trình dùng chung seed đó nên **đồng ý với nhau** trong khi cả hai bất
  đồng với lần chạy ngày mai.
- Hành vi phụ thuộc địa chỉ (pointer hashing, tái dùng allocator) cũng ổn định trong một tiến trình.

Nên tiến trình con chính là dụng cụ đo: mỗi test tự chạy lại binary test này với một env var yêu cầu
dựng world, chạy, và in ra một dòng checksum; tiến trình cha so sánh.

| Test | Chứng minh |
|---|---|
| `two_independent_processes_replaying_the_same_manifest_agree` | Hai tiến trình, cùng manifest ⇒ cùng checksum |
| `a_checkpoint_continuation_in_another_process_agrees_with_an_uninterrupted_run` | Checkpoint tiếp tục ở tiến trình khác khớp run liền mạch |
| `the_checksum_is_actually_a_function_of_the_trajectory` | **Control âm**: cùng manifest chạy nửa số tick ⇒ checksum **phải khác** |
| `the_deterministic_switch_actually_reaches_the_child_process` | Công tắc thật sự tới được tiến trình con |

Kết quả: `process A = 0xe4c7f5e9`, `process B = 0xe4c7f5e9`, `checkpoint-resumed = 0xe4c7f5e9`,
và control âm `half = 0xe66c09d2` ≠ `0xe4c7f5e9`.

**Control âm là bắt buộc.** Không có nó, một checksum hằng số sẽ làm cả hai gate trên xanh vì lý do
sai.

## 5. Chưa đóng

- **Thứ tự hệ thống là tất định, nhưng chưa được *viết ra*.** Executor đơn luồng cho một thứ tự tổng
  ổn định, nhưng nó vẫn được suy ra từ thứ tự chèn + các ràng buộc `.after(...)` rời rạc, chứ không
  phải một danh sách khai báo tường minh mà người đọc có thể kiểm. Chuyển schedule live sang
  `.chain()` đầy đủ (hoặc `SystemSet` có thứ tự) vẫn là việc còn lại.
- Gate dựng world và chạy schedule năng lượng trực tiếp; nó **không** đi qua
  `SimulationEngine::start` (1600 dòng trong một hàm). Vì vậy nó chứng minh *nhân* tất định, chưa
  chứng minh toàn bộ đường khởi động live. **G2** là nơi phần đó test được.
- `meta_ai::add_chronicle_event` vẫn dùng `Uuid::new_v4()` + `SystemTime::now()`: nó không nhận
  `DeterministicMode` hay tick. Chronicle từ đường đó là log UI chứ không nằm trong quỹ đạo mà gate
  đo, nhưng nó **có** vào saved state, nên cần nối nốt.
- `networking_systems.rs` còn nhiều `SystemTime::now()`; nằm ngoài danh sách file cho phép của G1 và
  ngoài quỹ đạo single-node.
- Physics/CPG chạy song song trong schedule live khi tắt determinism — đó là đường mặc định, và nó
  **không** tái lập được. Đây là chủ ý: chỉ run cần tái lập mới trả giá.
