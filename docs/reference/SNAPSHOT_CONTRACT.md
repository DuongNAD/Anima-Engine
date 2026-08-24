---
title: Hợp đồng snapshot (checkpoint khoa học)
status: active
owner: maintainers
last_reviewed: 2026-07-25
---

# Hợp đồng snapshot

Định nghĩa **thế nào là một checkpoint** trong Anima Engine, khác với "một file save".
Code: `src-tauri/src/core/snapshot.rs`. Gate: `src-tauri/tests/snapshot_checkpoint_tests.rs`.

Đọc kèm: `docs/reference/ENERGY_LEDGER_CONTRACT.md` (G1.1),
`docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md` (**D02**, **D09**).

## 1. Định nghĩa

> Checkpoint không phải là "đủ trạng thái để vẽ lại thế giới". Nó là **đủ trạng thái để việc tiếp
> tục từ đó không phân biệt được với việc chưa từng dừng lại.**

Đây là tập hợp thực sự lớn hơn, và phần dễ quên nhất là **vị trí rút (draw position) của RNG**:
khôi phục seed thôi sẽ khởi động lại chuỗi, nên run được resume phân kỳ ngay ở lần rút ngẫu nhiên
kế tiếp. `SimRng` vì vậy dùng `ChaCha12Rng` (chứ không phải `StdRng`) để có
`get_word_pos`/`set_word_pos` — seek O(1), không phát lại.

## 2. Ba khuyết tật của cách lưu cũ

`serde_json::to_string_pretty` → `std::fs::write`:

1. **Không có version.** Thêm một trường vào `SavedSimulationState` là âm thầm đổi ý nghĩa của file,
   và không có gì trên đĩa nói file đó thuộc shape nào. `#[serde(default)]` khiến file cũ *load
   được* — không đồng nghĩa *load đúng*.
2. **Không kiểm tra.** File bị cắt cụt deserialize thành một thế giới trông hợp lý.
3. **Không nguyên tử.** `fs::write` cắt file đích rồi mới ghi. Crash / đầy đĩa / hai lần save đua
   nhau ⇒ mất chính file bạn đang cố bảo vệ.

## 3. Envelope

```
SnapshotEnvelope {
  schema_version:    u32
  build_provenance:  { engine_version, target, profile }
  checksum:          u32   // FNV-1a 32
  state:             RawValue  // JSON thô của SavedSimulationState
}
```

**`state` là raw JSON, không phải struct đã parse.** Lý do rất cụ thể:
**vòng round-trip `f64` của serde_json không chính xác từng bit.** Quan sát được:
`eco_animals = 990.5102615356445` đọc lại thành `990.5102615356444`. Nếu checksum được tính bằng
cách *serialize lại* state đã parse, nó sẽ lệch với checksum ghi cạnh nó, và một file hoàn toàn tốt
trượt chính bài kiểm tra toàn vẹn của mình.

Giữ raw ⇒ **byte được hash, byte trên đĩa và byte được verify là cùng một chuỗi byte**. Nó cũng làm
checksum miễn nhiễm với thứ tự lặp của `HashMap` (MAP-Elites grid): thứ tự nào được ghi thì thứ tự
đó được hash. State vẫn hiện ra như một object lồng bình thường trong file, không phải chuỗi escape.

## 4. Ghi nguyên tử

Temp file cùng thư mục → `write_all` → `flush` → **`sync_all`** → `rename`.

`sync_all` là bắt buộc: thiếu nó, rename có thể tới đích trước dữ liệu, và mất điện để lại một file
đúng tên nhưng rỗng. Temp file mang PID để hai engine cùng ghi một đường dẫn không trộn byte.

Trên Windows `rename` thất bại nếu đích tồn tại, nên phải xoá trước — vẫn mở ra một khe hẹp không có
file nào ở đích, nhưng nghiêm ngặt tốt hơn hành vi cũ là cắt cụt đích trước khi ghi byte đầu tiên.

## 5. Version và migration

| Version | Shape |
|---|---|
| 1 | Trước G1.1: agents, food, lakes, trees, pheromone grid, epoch, lineage. Không có trạng thái năng lượng. |
| 2 | G1.1: thêm ba ngăn closed-EU và standing crop. |
| 3 | G1.2: thêm vị trí RNG, season clock, energy baseline; bọc trong envelope. |

`read` chấp nhận version hiện tại và **hai version trước** (N−2), đúng cửa sổ G1.2 yêu cầu. File cũ
hơn bị từ chối kèm thông báo nêu rõ version, thay vì bị ép vào một shape nó chưa từng được ghi.

File không có khoá `schema_version` là file tiền-envelope (v1 hoặc v2); `read` phát hiện và migrate
tiến. Mọi trường thêm từ v1 đều `#[serde(default)]`, và mỗi default chính là hành vi "save này có
trước tính năng đó" — nên save cũ vẫn load được (**D09**).

## 6. Những gì snapshot phải mang

| Nhóm | Trường |
|---|---|
| Quần thể | `agents` (kèm phenotype/brain), `foods`, `trees`, `lakes` |
| Năng lượng đóng (G1.1) | `eco_detritus`, `eco_plants`, `eco_animals`, `resource_field_r`, `energy_baseline` |
| Ngẫu nhiên | `sim_rng_seed`, **`sim_rng_pos`** |
| Nhịp thời gian | `tick_count`, `epoch_manager`, `season_phase`, `season_rate` |
| Định danh thế giới | `world_identity` (S08) |

`energy_baseline` được mang theo để run được resume vẫn đo bảo toàn so với **genesis gốc**, thay vì
chốt baseline mới khi load — điều đó sẽ tha thứ cho mọi trôi dạt xảy ra trước khi save.

## 7. Gate

```
checksum(run N) == checksum(run K → save → load → run N−K)
```

`snapshot::world_checksum` băm mọi thứ quyết định thế giới đi đâu tiếp: reserve và vị trí từng agent,
từng ô của resource field, ba ngăn năng lượng, seed **và vị trí** RNG, season clock, food trên mặt đất.

Agent và food được sắp xếp theo **nội dung**, không bao giờ theo entity id: Bevy lặp theo archetype,
và thế giới được khôi phục cấp phát id theo thứ tự khác thế giới đã lớn lên tới cùng trạng thái đó.
Băm theo id sẽ báo một khác biệt không tồn tại.

Kết quả: `N=4000, K=1500` → `reference=0x5d871e5c`, `resumed=0x5d871e5c`.

**Bài control là bắt buộc.** `dropping_the_rng_stream_position_does_diverge` đặt `sim_rng_pos = 0`
(đúng hành vi tiền-G1.2) và khẳng định checksum **phải** lệch. Không có nó, một gate xanh có thể chỉ
đang chứng minh rằng thế giới không nhạy với RNG.

## 8. Chưa đóng

- Schedule của gate là `.chain()` + `SingleThreaded` **do test tự khai báo**. Executor đa luồng của
  Bevy chọn thứ tự hệ thống theo từng lần chạy, nên một run không bị gián đoạn còn không khớp với
  *chính nó*. Khai báo thứ tự trong engine thật là việc của **G1.3**; cho tới lúc đó gate này tự
  khai báo thứ tự của mình và **không** chứng minh engine live là tất định.
- Gate đi qua `serialize_world_state` → envelope → đĩa → `snapshot::read` → `spawn_serialized_agent`
  + `restore_energy_state`. Nó **không** phủ phần `SimulationEngine::start` tự nối các mảnh đó lại —
  1600 dòng trong một hàm. **G2** là nơi phần đó trở nên test được.
- Chưa mang: dynamic fields (M3), exotic-energy field (AE2), causal ledger (M2), world laws /
  experiment manifest, tiến độ Meta-AI. Bốn cái đầu hiện chỉ tồn tại trong headless slice chứ không
  phải trong world Bevy live, nên chúng không nằm trong quỹ đạo mà gate này đo; khi AE4 đưa chúng
  vào world live thì phải thêm vào cả `SavedSimulationState` lẫn `world_checksum` **cùng lúc**.
