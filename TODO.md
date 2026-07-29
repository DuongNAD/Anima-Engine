# Danh sách công việc & Kế hoạch tiếp theo (TODO)

> ⬇️ **Backlog ưu tiên và trạng thái đo được nay nằm ở
> [`docs/planning/STATE_OF_THE_PROJECT.md`](docs/planning/STATE_OF_THE_PROJECT.md)** — đó là tài liệu
> một phiên mới đọc đầu tiên. File này giữ **nhật ký công việc** theo thứ tự thời gian ngược, để tra
> cứu "việc đó đã làm khi nào và vì sao". Phần "Mô hình Thỏ Papercraft" cũ được giữ ở cuối file như lưu trữ.
>
> ## 📜 Mọi con số trong file này là **đo lịch sử**
>
> Mỗi mục ghi kết quả *tại commit của mục đó*, kể cả mục trên cùng. Một mục mới hơn ở nhánh khác,
> hoặc một lần chạy lại sau đó, sẽ cho số khác — và điều đó **không** làm mục cũ sai, nó chỉ làm mục
> cũ không còn là hiện tại. **Trạng thái hôm nay chỉ ở một chỗ:**
> [`STATE_OF_THE_PROJECT.md` §1](docs/planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).

---

# ⏪ [MỚI NHẤT] OSS-072 MRCA + nối phả hệ vào IPC (2026-07-29)

Việc #3 của bàn giao 2026-07-27. `§3.15.1` trong `STATE_OF_THE_PROJECT.md`; số đo ở
[§1.f](docs/planning/STATE_OF_THE_PROJECT.md#1f-oss-072--ipc-phả-hệ--đo-2026-07-29-tại-a6d06ac).

## MRCA của một DAG không phải một node, và đó là toàn bộ nội dung của việc này

`evolution/mrca.rs`. Trong một cây có gốc, MRCA của hai cá thể là **một** node, và mọi thư viện
phylogenetics trả về đúng một. Phả hệ này **không phải cây**: `RelationType::Crossover` cho một cá
thể hai cha, nên `LineageRelation` mô tả một **DAG**, và một DAG có thể có nhiều tổ tiên chung
**không so sánh được với nhau** — không cái nào là tổ tiên của cái kia, nên không cái nào "gần hơn".

Trường hợp nhỏ nhất là thứ engine này sinh ra có chủ đích — hai anh em, mỗi đứa là crossover của
cùng một cặp:

```text
      r
     / \
    a   b
    |\ /|
    | X |
    |/ \|
    x   y
```

`a` và `b` đều là tổ tiên chung của `x` và `y`; `r` cũng là, nhưng **kém gần hơn cả hai**. Đáp án
trung thực cho `mrca(x, y)` là **tập** `{a, b}`.

Trả về một cái là đúng chế độ hỏng mà cả hệ con này liên tục gặp: một đáp án hữu hạn, hợp lý, cho
một câu hỏi không có đáp án duy nhất. Và nó sẽ không bị bắt — **một cài đặt trả về một node sẽ pass
mọi test cây trong file gate.** `LineageMrcaPayload.ambiguous` là chỗ nói điều đó ra cho phía tiêu
thụ, vì một `ancestors[0]` trông vẫn hoàn toàn đúng.

## Ba quy ước được khai báo, vì cái nào cũng có một lựa chọn ngược nghe hợp lý

- **Tổ tiên có tính phản xạ:** `mrca(x, x) = {x}`. Quy ước "tổ tiên thực sự" trả về **cha** của `x`,
  đọc như một lỗi ở mọi call site.
- **Không có tổ tiên chung = tập rỗng, không phải `Err`.** Genesis gọi `add_root` mỗi founder một
  lần, nên rừng nhiều gốc là hình dạng **bình thường**; trả `Err` biến trường hợp thường thành đường
  lỗi và ép mọi caller coi "không họ hàng" là thất bại.
- **Truy vấn rỗng = `Err`.** Mọi node đều là tổ tiên chung của tập rỗng, nên đáp án đúng về mặt toán
  là *cả đồ thị* — thứ trông như kết quả mà không ai hỏi.

Thứ tự trả về là generation giảm dần rồi id, nhưng **mọi phần tử đều không so sánh được với nhau**,
nên thứ tự là cách trình bày còn **tập** mới là đáp án. `mrca` cố ý **không** kiểm generation: bất
đồng giữa generation và cạnh không làm tập tổ tiên sai, chỉ làm thứ tự sai — và `to_newick` mới là
chỗ từ chối đồ thị đó.

## Gate mạnh nhất không phải cây biết trước đáp án

Nó là một **cài đặt thứ hai, cố tình ngây thơ**: giao tập hợp + reachability từng cặp, `O(|C|²·E)`,
không dùng chung một hàm nào với bản chính. Chạy trên một DAG sinh từ LCG cố định (không `rand` —
`sim_determinism_tests` quét mã nguồn chặn `thread_rng()`, và một fixture đổi mỗi lượt sẽ báo một lỗi
khác nhau mỗi lần nó fail), so **36 cặp** cộng cả hàng lá cùng lúc — đúng truy vấn OSS-073 sẽ làm.

Kèm hai thứ giữ cho nó không xanh rỗng: `the_oracle_can_actually_disagree` (control âm), và một
assertion bắt fixture **phải thực sự** sinh ra ít nhất một MRCA nhiều đáp án — nếu không, lượt chạy
đó không kiểm trường hợp DAG chút nào, và test **nói ra** điều đó.

## Bất biến liên hệ thống: nén không làm xê dịch MRCA

`compaction_leaves_the_mrca_where_it_was`. MRCA của một tập sample có **ít nhất hai con** được giữ
lại — nếu chỉ một, con đó đã là tổ tiên chung gần hơn — nên nén đường unary không bao giờ với tới nó.
Lý do cấu trúc, không phải may mắn. Nếu nó thôi đúng thì phần khoa học dựng trên phả hệ **âm thầm**
đổi đáp án sau epoch 50.

## Phả hệ đã có mặt trên IPC, và một gate mới cho chỗ không ai canh

`get_lineage_mrca(individuals)` · `export_lineage_newick` · `get_simplified_lineage(samples)`, đều
**chỉ đọc**; `compact` vẫn là đường ghi duy nhất. Đóng đúng khoảng trống §3.15.1 ghi: `to_newick` và
`simplify` không lệnh Tauri nào gọi tới. **"Nối vào IPC" không phải "UI đã dùng"** — chưa component
nào gọi.

`tests/ipc_registration_tests.rs` chốt rằng **mọi** `#[tauri::command]` đều có trong
`generate_handler!`. Một lệnh thiếu ở đó vẫn biên dịch (nó là API công khai, không phải dead code),
vẫn sinh binding `ts-rs`, vẫn được ghi trong `PROJECT.md`, và trả `Unknown command` khi app gọi.
Clippy không thấy, `check_ipc_arg_case` không thấy (nó kiểm **cách viết** ở call site đã tồn tại), và
test frontend không thấy — chúng mock `invoke`, mà mock thì trả lời mọi cái tên.

**Gate đó bắt được một lỗi trong parser của chính nó ngay lần chạy đầu**, và đó là phần đáng nhớ: nó
tách danh sách theo dấu phẩy nên mục **đầu tiên** còn dính `generate_handler![` ở trước và trượt
`strip_prefix`, khiến nó báo `get_simulation_status` chưa đăng ký — một lệnh chưa bao giờ thiếu.
Control âm khi đó **đang pass rỗng**: nó chỉ khẳng định "hiệu hai tập khác rỗng", mà hiệu với một tập
rỗng thì **luôn** khác rỗng, nên nó đồng ý với một parser không parse được gì. Nay parser quét mọi
`commands::` thay vì tách dấu phẩy, và control ghim **giá trị** parser trả về.

(Số 32 lệnh trong một lượt đếm tay hoá ra là 31: cái thứ 32 là một **comment** trích
`#[tauri::command]` khi giải thích chính quy tắc camelCase. Scanner vì thế đòi attribute **mở đầu**
dòng, và có control âm cho đúng ca đó.)

## 🔴 Finding mở ra từ chính lượt chạy gate: `test:frontend` đỏ trên **checkout Windows**, vì CRLF

Không phải do nhánh này, và không phải do mã. `frontend/thirdPartyLicenses.test.ts` **không nạp
được** vì `scripts/check_text_hygiene.mjs` mở đầu bằng shebang `#!/usr/bin/env node` **và** export
symbol mà test import — Vite dồn shim CJS lên đầu dòng 1 và đẩy shebang ra giữa dòng, chỗ `#!` là lỗi
cú pháp.

**Cái quyết định là dòng kết thúc.** Cùng một commit, chỉ đổi dòng kết thúc của **một** file:

| `scripts/check_text_hygiene.mjs` | Kết quả |
|---|---|
| **CRLF** (git checkout ra như vậy — `core.autocrlf=true`, `scripts/**` chưa ghim) | 38 file passed · **1 FAILED** · exit 1 |
| **LF**, không đổi gì khác | ✅ **39 file passed · 440 test · exit 0** |

Nên **hàng CI trên runner GitHub vẫn đúng**: job `Frontend` chạy `ubuntu-latest`, checkout LF, xanh
thật — và cũng vì thế CI **không** thấy được lỗi này. Cái không tái lập được là hai hàng **chạy tay
trên Windows** ở §1.b/§1.e ghi `0 fail`; trên checkout CRLF chúng không thể xanh.

Bản sửa là **một dòng ghim trong `.gitattributes`**, đúng họ với các dòng đã có ở đó — việc riêng,
không gộp vào nhánh phả hệ. Điều đáng ghi lại: file `.gitattributes` ấy được viết với luận chứng
rằng autocrlf phá các artifact **so sánh byte**; ca này CRLF không làm sai một phép so byte mà làm
**một parser không parse được**, nên phạm vi cần ghim rộng hơn cái file đó tự mô tả.

---

# ⏪ Adapter thí nghiệm cho thế giới sống + đo tick trong tiến trình (2026-07-27)

`§3.3` và nửa còn lại của `§3.2` trong `STATE_OF_THE_PROJECT.md`. Ba mảnh khoá vào nhau:

1. **Lịch trình sống có đúng một định nghĩa.** `core/simulation_schedule.rs::build_tick_schedule` là
   khối `add_systems` đã nằm giữa closure 900 dòng của `SimulationEngine::start`, tách ra nguyên
   trạng. Trước đó mọi test headless tự khai một `.chain()` khác — tức là ghim một lịch trình app
   **không** chạy.
2. **`LiveExperimentAdapter: ExperimentModel`** (`core/live_experiment.rs`) chạy lịch trình đó qua
   **cùng** `experiment_runner` với `ReferenceEvolutionWorld`. `RefCell<World>` chứ không `unsafe`:
   hai method của trait nhận `&self` còn mọi query Bevy cần `&mut World`, nên cách trung thực là
   mượn động, không phải cache một giá trị sẽ ôi.
3. **`core/tick_capture.rs`** đo một tick của lịch trình đó: ba pha kẹp **chính xác** bằng `Instant`
   trong vòng lặp sim, bốn pha còn lại giới hạn bằng checkpoint và **nói ra** rằng chúng chỉ là
   checkpoint (`PhaseSummary.exact`). Ring cấp phát sẵn, không kênh, không thread.

## Gate checkpoint lộ ra hai lỗ persistence thật

- **Pha stride của regrowth sống trong `Local<usize>`** — trạng thái quỹ đạo nằm ở chỗ không snapshot
  nào với tới. `REGROWTH_STRIDE = 4` nên một lần resume mọc lại **một phần tư khác** của thế giới.
  Gate cũ không bắt vì `K = 1500` chia hết cho 4. Nay là `ResourceField::regrowth_phase`, có trong
  save (schema 5) **và** trong `world_checksum`.
- **Một tick để lại suy luận đang bay.** App trả lời request ở tick *sau*, nên ở ranh giới checkpoint
  hai batch khoá theo `Entity` đang nằm trong kênh — mà id entity không ổn định qua restore. Adapter
  trả lời **trong chính tick đã hỏi** (`live_inference_pump_system`), nên tick là nguyên tử.

## `MIN_SUPPORTED_SCHEMA` không chặn bump như tài liệu cũ nói

ADR-0004 và mục "Hoãn có lý do" bên dưới đều viết rằng bump `SCHEMA_VERSION` 4→5 sẽ **mất khả năng
đọc save v2**. Sai, và đã sửa cả hai chỗ: hằng số đó chỉ áp cho file **có** `schema_version` (v3+);
v1/v2 được ghi không có envelope nên đi nhánh pre-envelope. Schema nay là 5 và test save v1/v2 vẫn xanh.

**Chưa làm, nói thẳng:** chưa có lần chạy **app desktop** nào — số hiệu năng thật vẫn cần một con
người bấm chạy với `ANIMA_TICK_CAPTURE`, thủ tục ở `docs/how-to/BENCHMARKING.md`.

**Đo lịch sử tại `bb8248e` (2026-07-27)** — `cargo test --features desktop --no-fail-fast`
**843 pass · 0 fail · 4 ignored** (79 target, 0 rỗng) và `cargo test --no-default-features
--no-fail-fast` **825 pass · 0 fail · 4 ignored**; `cargo fmt --check` + clippy hai cấu hình exit 0;
`npm run test` 109 pass, `npm run test:frontend` 432 pass, lint/ratchet 0/0, `npm run build` pass.

> Bản ghi đầu tiên của mục này viết `841`/`823`, lấy từ một lần chạy **trước** lần cuối cùng của
> chính commit đó — đúng loại "chậm một lần chạy" mà quy ước ở đầu file tồn tại để chặn. Số trên là
> số cuối cùng đã ghi trong commit message của `bb8248e`. Số **hiện tại** (khác, vì `2285a92` đã
> thêm test và đổi allow-list) ở
> [`STATE_OF_THE_PROJECT.md` §1](docs/planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).

---

# ⏪ OSS-071b — nối `simplify` vào tracker sống (2026-07-26)

`LineageTracker::compact(samples)` thay thế **thật** bộ nhớ của `InMemoryLineageTracker`, gọi ở
thread tiến hoá mỗi 50 epoch. `746 pass · 0 fail`, 75 target 0 rỗng, fmt + clippy (cả hai cấu hình
feature) sạch, docs link 416/0 gãy.

## Tập sample KHÔNG phải "ai đang sống"

Mọi `lineage_id` trong archive MAP-Elites đều có thể được chọn làm cha ở epoch sau, và một elite
**không nhất thiết là tổ tiên của ai đang sống**. Prune theo liveness sẽ xoá đúng node mà lần sinh
sản kế tiếp gọi tên.

Hazard đó **từng là hỏng âm thầm**: `add_reproduction` ghi cạnh vô điều kiện, nên một sample bị sót
tạo ra **cạnh mồ côi** — và cạnh mồ côi làm hỏng **toàn bộ** đồ thị, vì cả `to_newick` lẫn `simplify`
đều từ chối xử lý đồ thị chứa nó. Một lần ghi sai đầu độc export và mọi lần compaction sau.

Nay `add_reproduction` **từ chối ghi** cạnh có cha không tồn tại, báo lỗi nêu tên cha đó, và giữ lại
những cạnh hợp lệ khác trong cùng lần gọi. Mất một liên kết tổ tiên thay vì mất cả đồ thị. Đó chính
là thứ khiến compaction an toàn để bật — và nó là cải thiện độc lập, vì trước đây không gì chặn một
cạnh mồ côi cả.

## Chạy với nén TẮT, có chủ ý

Nén là bước đạt cận O(alive), nhưng nó xoá node mà consumer **vẫn đang đọc**: đồ thị UI vẽ lấy thẳng
từ tracker, và `get_mutations_count` đi qua `RelationType` **từng cạnh** — thứ một cạnh đã nén không
mang được. Nên compaction sống chỉ bỏ nhánh tuyệt chủng, **đúng chỗ genotype nằm**, và giữ nguyên
thân cây.

**Việc kế tiếp để mở khoá phần còn lại:** lưu số đếm đột biến tích luỹ theo node, kiểu `Option<u32>`
chứ không phải `u32` — mặc định `0` sẽ đọc thành "không có đột biến" cho mọi save cũ.

## Neo4j không bị đụng

Chỉ bộ nhớ trong co lại. Hệ quả cần biết: khi Neo4j online, `get_lineage_graph` đọc từ database nên
vẫn trả **đồ thị đầy đủ**. Xoá khỏi database là thao tác phá huỷ từ xa, cần quyết định riêng.

## Control âm

`compacting_against_every_node_removes_nothing` — nếu `compact` xoá bất kể tập sample thì mọi khẳng
định khác vẫn xanh trong khi nó âm thầm phá dữ liệu. Và
`compaction_refuses_a_malformed_graph_rather_than_rewriting_it`: đồ thị có chu trình thì bỏ lần
compaction, không im lặng viết lại — viết lại sẽ **xoá bằng chứng** về cách nó hỏng.

---

# ⏪ OSS-071 — `simplify()`, và chỗ nó suýt nói dối (2026-07-26)

`src-tauri/src/evolution/simplify.rs` + `tests/lineage_simplify_tests.rs` (13 test) + 6 unit test.
`726 pass · 0 fail`, 73 target 0 rỗng, fmt + clippy sạch, docs link 412/0 gãy.

## Bước hai mới là bước có tác dụng

1. **Prune** — bỏ node không có hậu duệ nào trong tập sample.
2. **Nén đường đơn** — node không phải sample, đúng 1 cha 1 con thì nối tắt.

**Bước 1 một mình KHÔNG chặn được bộ nhớ**, và điều đó được biến thành phép đo chứ không phải lời
khẳng định: tổ tiên của quần thể sống vẫn kéo về genesis, nên prune chỉ bỏ nhánh tuyệt chủng và giữ
nguyên mọi thân cây. `pruning_without_compression_would_not_have_been_enough` đo cả hai và đòi chênh
> 3×. Cây nhị phân sâu 10 (2.047 node, 16 cá thể sống) → còn **31 node** = 16 sample + 15 điểm rẽ
nhánh, đúng cận `2·samples`.

## Chỗ suýt sai âm thầm

`get_mutations_count` trong `commands/evolution.rs` **đếm cạnh `Mutate` dọc đường tổ tiên** để ra con
số đột biến trên UI. Gộp 5 cạnh `Mutate` thành 1 cạnh `Mutate` là **giữ đúng kiểu mà làm số đếm
thành 1 thay vì 5** — hữu hạn, hợp lý, sai gấp năm.

Nên `SimplifiedEdge` mang `events`/`mutations`/`crossovers` thay vì `relation_type`, và là **kiểu
riêng** chứ không mở rộng `LineageRelation` — cái đó đã persist vào save state và Neo4j; không nhét
khái niệm chỉ dùng để phân tích vào một định dạng lưu trữ.

Hai chỗ **từ chối nén** cùng lý do: node crossover (2 cha — nối tắt phải chọn một, vứt cái kia), và
hình thoi (nén sẽ gộp hai đường tổ tiên khác nhau thành một cạnh rồi cộng số đếm như thể nối tiếp).

## Kiểm chứng bằng OSS-070

Tách thêm `to_newick_from` nhận cặp (cha, con), để lineage đã simplify xuất được Newick **mà không
ai phải bịa `relation_type`**; `to_newick` nay là wrapper mỏng nên hai đường không lệch được. Test
`the_simplified_lineage_is_still_a_tree_a_newick_parser_would_accept` chốt kết quả vẫn không chu
trình, không cạnh mồ côi, không đảo generation.

Control âm: `the_ancestry_check_can_actually_fail` cắt một cạnh khỏi kết quả và đòi phép so tổ tiên
phải đỏ — nếu không, `ancestry_among_retained_nodes_is_unchanged` có thể đang xanh vì phép so rỗng
chứ không vì bất biến đúng.

## Chưa xong, có lý do

**Chưa nối vào tracker sống**, nên **bộ nhớ thực tế chưa giảm**. `simplify` là hàm thuần trả về một
*giá trị*. Bước thay thế bộ nhớ của `InMemoryLineageTracker` cần chính sách về khi nào chạy, ai cung
cấp danh sách cá thể sống, và tương tác với `load_state`/Neo4j.

## Một ghi chú vận hành

Lần chạy full suite đầu tiên đỏ với `rustc.exe` tự crash (`STATUS_STACK_BUFFER_OVERRUN`) và báo
thiếu rlib của chính `anima_engine_lib`. **Không phải lỗi code**: `Get-Process cargo` cho thấy hai
tiến trình của phiên khác đang ghi cùng `target/`. Chờ chúng xong rồi chạy lại: 726 pass, 0 fail.
Trước khi kết luận một lỗi build kỳ lạ là hồi quy, hãy hỏi máy lúc đó đang chạy gì.

---

# ⏪ OSS-070 — xuất Newick, và cái test bắt được (2026-07-26)

`src-tauri/src/evolution/newick.rs` + `tests/newick_export_tests.rs` (14 test) + 8 unit test.
`706 pass · 0 fail · 4 ignored`, 72 target 0 rỗng, fmt + clippy sạch. **0 dependency mới** — Newick
là *định dạng*, nên license của `ape`/`ggtree` không liên quan gì.

## Đánh đổi phải nói rõ, vì nó không phải chi tiết

`Crossover` cho một cá thể **hai** cha mẹ ⇒ lineage là **DAG**; Newick chỉ biểu diễn **cây**. Export
giữ **một** cha mẹ và **đếm** số cạnh không biểu diễn được (`dropped_parent_edges`).

Đếm thay vì bỏ im lặng là toàn bộ vấn đề: một export lặng lẽ cắt một nửa phả hệ có crossover **vẫn
parse được, vẫn vẽ ra được, và vẫn sai**. Cha mẹ sống sót chọn theo **id nhỏ nhất**, không theo thứ
tự cạnh — vì thứ tự khác nhau giữa tracker in-memory (push order) và Neo4j (query order), và một
export đổi hình dạng theo nơi dữ liệu đến từ đâu thì không tái lập được.

## Defect thật do test bắt được

Kiểm tra generation ban đầu chạy **trước** kiểm tra chu trình. Nhưng generation không thể tăng đơn
điệu vòng quanh một chu trình, nên **mọi chu trình đều kéo theo đảo generation** — code báo
`GenerationInversion` cho một đồ thị mà vấn đề thật là vòng lặp, tức **báo triệu chứng thay vì
nguyên nhân**, và dẫn người đọc tới một node có bản ghi hoàn toàn bình thường.

Ba test đỏ vì đúng lý do đó (`a_cycle_is_refused`, `a_cycle_beside_a_healthy_tree_is_still_refused`,
`the_reported_cycle_node_is_on_the_loop_not_merely_hanging_off_it`). Thứ tự nay là **chu trình
trước, generation sau**. Lỗi thứ tư là test tôi viết sai: gạch ngang không phải ký tự dành riêng của
Newick, nên `a-kid` không cần quote — và nếu nó cần thì mọi UUID sẽ bị quote vô ích.

Hai control âm đáng giữ: `a_cycle_beside_a_healthy_tree_is_still_refused` chứng minh "không có gốc"
**không** phải phép thử tương đương với "có chu trình"; `a_deep_chain_does_not_overflow_the_stack`
(50.000 node) chốt rằng emitter là vòng lặp chứ không đệ quy — bản đệ quy sẽ qua mọi test vừa màn
hình và chết đúng lúc run thật đủ dài.

## Chân "parser bên thứ ba" đã chạy — và đã được chứng minh là đỏ được

DendroPy 5.0.10 (`pip install dendropy`, **dev-only**) đọc
`src-tauri/tests/fixtures/newick/lineage_forest.nwk`. Gate là **hai nửa trên cùng một file**:
`cargo test` ghim output vào fixture, `python scripts/verify_newick.py` bắt parser ngoài đọc chính
file đó. Round-trip thuần Rust chỉ chứng minh serializer nhất quán với chính nó.

Đã phá fixture hai cách để chắc gate không xanh vô nghĩa:

- **Lồng ngoặc ngược chiều** — vẫn là Newick *hợp lệ* nên parse trót lọt, nhưng khẳng định topology
  bắt được. Đây là loại lỗi so chuỗi phía Rust không thấy.
- **Bỏ quote quanh nhãn có dấu hai chấm** — DendroPy từ chối. Đúng thứ quy tắc quote tồn tại để
  chặn: `child:two` không quote bị đọc thành branch length và **cắt cụt tên** chứ không báo lỗi,
  trừ khi phần còn lại không phải số.

## Chưa xong, có lý do

**Chưa nối vào IPC.** `to_newick` là hàm thư viện, chưa lệnh Tauri nào gọi. Việc riêng, cần cập nhật
hợp đồng IPC ở `PROJECT.md`.

Mục kế tiếp mở khoá: **OSS-071 `simplify()`** — và OSS-070 vừa cho nó cách kiểm chứng, vì bất biến
"quan hệ tổ tiên của phần giữ lại không đổi" so được bằng cách xuất Newick trước và sau prune.

---

# ⏪ OSS-010 Criterion — đo thật thay cho ước lượng, và hoà giải trạng thái (2026-07-26)

Hai PR trong cùng ngày, ghi chung vì mục thứ hai tồn tại chỉ để sửa cái mục thứ nhất làm sai đi.

## OSS-010 (PR #11, merge `80dabdd`)

Criterion vào làm `dev-dependency`, cộng `src-tauri/benches/tick_systems.rs`. Nó hợp với ràng buộc
nặng nhất của dự án — **không chạy full backend** — vì bench từng system headless chứ không boot
Tauri. Bảng số: [`docs/how-to/BENCHMARKING.md`](docs/how-to/BENCHMARKING.md).

Ba thứ số đo nói ra, đáng nhớ hơn bản thân các con số:

1. **Ngân sách khung hình là cận dưới.** Tổng system chạy mỗi tick ở 1.000 agent ≈ **493 µs ≈ 3,0 %**
   của 16,67 ms. Chưa gồm não, lập lịch ECS, emit, va chạm, trao đổi chất. **Không** rút ra tuyên bố
   "60 FPS" từ đây.
2. **Chi phí không ở chỗ hay đoán.** `integrate_physics_system` rẻ và tuyến tính (4,92 ns/agent);
   `rebuild_spatial_grid_system` đắt **gấp ~15 lần** ở cùng số agent, và phần lớn chi phí quy mô nhỏ
   là quét ô lưới cấp phát sẵn chứ không phải xử lý agent. Tối ưu ở đó, không phải solver.
3. **Con số 4,2 ms trong `ecology.rs` không tái lập.** Release build cho ~0,36 ms — thấp hơn ~12 lần.
   Việc stride **vẫn đúng** (đo được 3,97×); chỉ con số biện minh là chưa đối chứng được. Finding
   mở, không phải lỗi đã xác định.

**Một chỗ tôi sai và đã tự sửa trong cùng PR:** số báo cáo lần đầu là **slope estimate**, không phải
trung vị — dòng `time: [a b c]` của `cargo bench` in ra slope khi lấy mẫu tuyến tính. Chênh thật:
`step_water` 297,6 µs (slope) so với 271,5 µs (trung vị). Mọi con số đã chuyển sang trung vị; mọi kết
luận giữ nguyên.

**Phần cứng mục tiêu đổi sang i5-14600KF** (khai báo *Dell Vostro 3530* vô hiệu, người duy trì xác
nhận). `benchmark_report.json` vốn đã ghi đúng CPU — chỉ văn xuôi là còn nói máy cũ.

**Guard chống hỏng im lặng:** `target/` bị gitignore, nên chạy `bench_baseline.mjs` trên clone mới sẽ
thay số thật bằng proxy — mà kết quả **vẫn validate, vẫn trông như baseline**. Script nay từ chối và
exit ≠ 0 trừ khi `ANIMA_BENCH_ALLOW_PROXY_ONLY=1`. Đã test cả hai chiều.

**§3.2 vẫn chưa đóng, nhưng không còn vì phần cứng:** cái đo được là cận dưới của một tick. Ba hàng
đắt nhất đòi in-app tick capture, và suy luận não per-agent đang tắt mặc định (§3.1).

## Hoà giải trạng thái (PR này)

PR #9 (review nguồn mở) merge **sau** #11 nhưng được viết **trước**, nên nó vào `main` mang theo ba
khẳng định đã hết đúng: OSS-010 "chưa thực thi", "chưa có mục nào được thực thi", và §3.2 nói công cụ
"chưa ai thêm vào". Không phải xung đột git — hai PR không đụng file chung — nên nó merge sạch và để
lại tài liệu nói sai. Mục này sửa đúng ba chỗ đó.

Bài học giữ lại trong `OPEN_SOURCE_ADOPTION_PLAN.md`: **một tài liệu trạng thái viết bằng câu tuyệt
đối hết hạn nhanh hơn tài liệu viết theo từng mục có ngày.**

---

# ⏪ Review nguồn mở đợt 1 + chuẩn hoá tài liệu (2026-07-26)

Không đụng tới code. Đây là đợt review định kỳ đầu tiên của
[`docs/research/OPEN_SOURCE_LANDSCAPE.md`](docs/research/OPEN_SOURCE_LANDSCAPE.md), khởi từ một câu
hỏi khảo sát 19 dự án bên ngoài.

## Phát hiện chính — tài liệu sai, không chỉ thiếu

Ba thứ đã sai sự thật chứ không phải chưa cập nhật:

1. **`OPEN_SOURCE_POLICY.md` nói "repository hiện chưa có `LICENSE`".** File tồn tại, và là
   **proprietary, all rights reserved**. Blocker quản trị OSS-003 vì thế đã gỡ — nhưng theo hướng
   **thắt chặt**: thành phần copyleft (GPL/AGPL) nay là chặn cứng cho mọi đường tiếp xúc với code,
   áp cho ít nhất SLiM, Avida, ALIEN và Thrive. Hệ quả phụ **đã xử lý 2026-07-27:** `NOTICE` và
   `licensing/THIRD_PARTY_LICENSES.txt` đều đã sinh tự động và có gate CI; sau đợt vendor
   `licensing/upstream/` (2026-07-27) chỉ còn **1** thành phần mà upstream chưa từng publish văn bản
   license, liệt kê kèm chứng cứ tìm kiếm ở `licensing/UNRESOLVED.md` (mục 3.16).
2. **`three-mesh-bvh` được xếp "Pilot ưu tiên cao" trên một tiền đề code đã bác bỏ.** Không có
   `THREE.Raycaster` nào trong `src/`; cao độ địa hình lấy giải tích qua `sampleElevation`; LOD theo
   khoảng cách đã có ở `chunkLod.ts`. Còn `raycasts` trong `PixiViewport.tsx` là telemetry cảm biến
   backend vẽ thành đường 2D — trùng tên, khác việc. Hạ xuống Defer, kèm trigger mở lại.
3. **Criterion đã được chốt "Adopt" từ 2026-07-24 (OSS-010) và chưa từng được thêm.** Việc này
   không còn là vệ sinh: nó là công cụ đã duyệt cho đúng mục **P0 §3.2** (tuyên bố "60 FPS
   real-time" chưa từng được đo). Và nó hợp ràng buộc nặng nhất của dự án — không chạy full backend
   trên máy dev — vì bench từng system headless chứ không boot Tauri.

## Đã ghi thêm

- 9 ứng viên thiếu khỏi ma trận, trong đó `burn`/`burn-wgpu` là **runtime dep đang chạy mà không có
  trong inventory** (mục 3.17 mới).
- Miền mới **§5 Phả hệ**: `lineage.rs` lưu mọi lần sinh sản kèm bản sao genotype đầy đủ và không bao
  giờ prune → bộ nhớ tăng theo *tổng số từng sống*. Lấy **thuật toán** `simplify()` của tskit chứ
  không lấy crate, và **định dạng** Newick chứ không lấy code R. Thành §3.15 (P1) + OS7.
- **§3.1 Landlab:** vì sao là Oracle chứ không phải dependency — bốn lý do gắn với code
  (`step_water` định tuyến đồng bộ có chủ ý; ngân sách nước phải ~0 ở gate S16; thang thời gian địa
  chất vs 60 Hz; Python trong tick loop). Kèm khoảng trống thật mà nó chỉ ra: không có flow
  accumulation, và `step_erosion` không vận chuyển trầm tích.
- F2 Rapier: giữ Pilot nhưng thêm **tiền điều kiện cứng** — không mở pilot trước khi physics/CPG
  hết chạy song song, vì OSS-040 đòi một đường cơ sở lặp lại được, mà hiện một run liền mạch còn
  không khớp chính nó.

## Chuẩn hoá tài liệu

- Tạo `docs/research/README.md` và `docs/governance/README.md` — hai thư mục duy nhất thiếu index
  dù mỗi cái đã có hai tài liệu. Thêm quy tắc "mỗi thư mục con của `docs/` phải có index" vào chính
  sách, vì đó là điều kiện để OSS-001 (≤ 2 lần nhấp) còn đúng khi thêm tài liệu mới.
- Thêm frontmatter cho 2 tài liệu `docs/ai/planning/` không có; ghi nhận `**Status:** proposed` của
  bản G0–G4 đã lỗi thời (G1.1–G1.4 nay là *Live integrated*).
- Hợp thức hoá ba khác biệt đã tồn tại sẵn thay vì churn: `completed` vào enum trạng thái (chỉ cho
  `docs/ai/`), schema `kind: agent-goal`, và **miễn** cho `docs/ai/*/README.md` — chúng là template
  thượng nguồn, không phải tài liệu chưa hoàn thành.

## Ba việc tiếp theo, theo thứ tự

1. OSS-010 Criterion (P0, chặn §3.2) · 2. OSS-070 xuất Newick (~40 dòng, 0 dependency) ·
3. Phần còn nợ của OSS-003: tách phạm vi license. (`NOTICE` xong 2026-07-27; xem
   `licensing/UNRESOLVED.md` cho 1 mục còn chặn phát hành.)

---

# ⏪ ADR-0004 O3 — phát lại người quan sát (cơ chế xong, tuyên bố phiên sống vẫn chặn) (2026-07-26)

`668 pass · 0 fail · 71 target` (O2 là 662/70, tức **+6 test / +1 target**), clippy sạch, 318 link
docs 0 gãy.

## Đã làm

`ObserverReplay` phát lại một trace thay cho camera sống. Hai thuộc tính đáng nói:

- **Loại trừ, không phải ưu tiên.** Khi replay có mặt, `SharedLodFocus` bị bỏ qua hoàn toàn. Nếu chỉ
  "ưu tiên trace", một `set_lod_focus` lạc từ UI ai đó quên đóng sẽ lái run trong khi trace vẫn được
  ghi công — cách duy nhất một replay nói dối về thứ nó tái tạo. `a_live_camera_cannot_steer_a_replay`
  chạy camera thù địch ngược chiều suốt run và kết quả không đổi.
- **Nội suy được khai báo** đúng thứ C2 đòi: focus **giữ nguyên** giữa hai mẫu. Không phải xấp xỉ cho
  tiện — vì `record` chỉ lưu khi giá trị đổi, giữ-nguyên tái dựng **đúng** tín hiệu gốc.

Gate: `tests/observer_replay_tests.rs` (6), gồm control âm
`replaying_a_different_trace_produces_a_different_session`.

## Cái KHÔNG tuyên bố, và vì sao

Gate `an_inhabited_run_replays_from_its_trace_without_a_human` vẫn **pending**. Nó đo quỹ đạo *thế
giới sống*, mà physics/CPG chạy song song nên một run liền mạch còn không khớp chính nó
(`DETERMINISM_CONTRACT` §5). Gate vừa viết tự khai báo thứ tự schedule và ghim **hệ con** — đúng phạm
vi `SNAPSHOT_CONTRACT` §8 tự nhận. ADR-0004 tự dặn "không tuyên bố replay trước G2".

## Hoãn có lý do, không phải quên

Lưu trace vào save state cần bump `SCHEMA_VERSION`. Trả cái giá đó cho dữ liệu chưa mode nào tiêu thụ
là sai thứ tự. Nó đi cùng lúc replay thành mode sống, và khi đó phải vào **cả** `SavedSimulationState`
lẫn `world_checksum` một lượt (§8).

> **Sửa 2026-07-27.** Câu cũ ở đây nói bump 4→5 sẽ "mất khả năng đọc save v2" vì
> `MIN_SUPPORTED_SCHEMA = SCHEMA_VERSION - 2`. Sai: hằng số đó chỉ áp cho file **có** `schema_version`
> (v3 trở lên); v1/v2 không có envelope nên đi nhánh pre-envelope của `snapshot::from_bytes` bất kể
> nó. Schema nay là **5** (gói live-adapter) và test save v1/v2 vẫn xanh. Cái đúng còn lại là lý do
> **thứ tự**, không phải lý do tương thích.

Ghi lại một phân biệt đáng giá khi tới lúc đó: **khi ghi, trace là đầu ra** và không lái thế giới nên
không thuộc checksum; **khi phát lại, phần trace còn lại là đầu vào** và thuộc. Cùng một dữ liệu, hai
vai trò tuỳ mode.

---

# 🧾 ADR-0004 O2 — ghi lại người quan sát, và cắm rễ hệ quả vào họ (2026-07-26)

`662 pass · 0 fail · 70 target` (O1 là 647/68, tức **+15 test / +2 target**), clippy sạch, 318 link
docs 0 gãy.

## Đã làm

- `ObserverTrace` ghi **focus hiệu lực** (sau policy, không phải cái UI xin), buffer cấp phát sẵn,
  ghi-khi-đổi, và **đếm** mẫu tràn thay vì bỏ im lặng — một trace ngừng ghi trong im lặng đọc y hệt
  một camera ngừng di chuyển.
- `CAUSE_OBSERVER` ở **đỉnh** dải `CauseId` (scenario cấp tay từ dưới lên, không có bộ cấp phát nào),
  cộng `is_reserved_cause`, cộng luật cấm manifest giành id đó.
- App sống nay khai báo `Inhabit` thật thay vì "chưa khai báo". Hành vi không đổi — `Inhabit` là
  policy duy nhất cho focus đi qua — nhưng hệ quả đã có gốc thay vì đọc như động lực nền.
- `DETERMINISM_CONTRACT` §2 nay là **năm** nguồn rò rỉ, thêm §2.1 cho camera kèm bảng ba policy.
- Gate: `observer_trace_tests` (6, có control âm) + `observer_trace_zero_alloc_tests` (1 test, 3 pha)
  + 8 unit test mới.

## Lỗ hổng thiết kế mà chính test bắt được

O1 enforce policy bằng `enabled = false` nhưng **giữ nguyên `center`**. Nên dưới `Spectate`, toạ độ
camera vẫn trôi vào world mỗi tick: trace đầy chuyển động thế giới chưa từng cảm nhận, và tệ hơn —
một đường camera sống nằm sẵn trong world cho system sau này đọc phải, tái lập đúng cái nhiễu vừa
cấm. Gate O1 không thấy vì nó chỉ đo *ai được suy nghĩ*, mà `tier_at` trả `Hot` khi `!enabled` bất kể
`center`. **Sửa: từ chối focus là thay trọn `LodFocus::default()`.**

## Ba điều chỉnh so với C2/C3 khi va vào code thật

1. **`ObserverSample` không mang `actions`.** Engine chưa có hành động nhập vai nào. Một
   `Vec<ObserverAction>` rỗng vĩnh viễn đúng là thứ "chạy được và sai âm thầm".
2. **`CausalLedger` chưa có trong world Bevy sống** (headless tới khi G2 hội tụ), nên provenance được
   chứng minh ở chỗ ledger thật sự sống. Gate `observer_writes_go_through_the_intervention_seam` là
   **n/a**, không phải pending — chưa có hành động nào để đi qua seam.
3. **Một mâu thuẫn tự tạo, đã gỡ:** validate cấm intervention giành `CAUSE_OBSERVER`, nhưng C3 nói
   hành động observer *hạ xuống thành* intervention mang đúng id đó. Phân biệt đúng: cấm manifest
   **khai báo sẵn** (viết trước khi run bắt đầu ⇒ không thể do người gây ra lúc chạy), không cấm
   intervention sinh **lúc chạy**.

## Bẫy quy trình, tự vấp hai lần trong lượt này

Phóng thẳng `clippy` + cả suite mà không `cargo check --all-targets` trước. Hai lỗi — thiếu import
`InterventionKind`/`CauseId`, và fixture dùng `start_tick: 0` nên trượt khỏi cửa sổ `1..=run_ticks`
(`experiment.rs:999` từ chối factor không bao giờ kích hoạt) — đều bắt được trong 20 giây bằng
`cargo check`, thay vì mất hai vòng nhiều phút.

Đáng chú ý: chính `ordinary_hand_written_cause_ids_are_still_accepted` cứu lượt này. Nếu chỉ có test
"cấm CAUSE_OBSERVER", tôi đã tưởng luật hoạt động trong khi thực ra nó từ chối **mọi** manifest.

## Còn lại

**O3** (replay `Inhabit`, phụ thuộc §3.3/§3.6) và cùng với nó `TraceRef` vào `SNAPSHOT_CONTRACT`
(cần bump schema + migration).

---

# 👁️ ADR-0004 accepted — O1: người quan sát thành chính sách được khai báo (2026-07-26)

ADR-0004 được chấp nhận. **O1 đã ship**, đo được `647 pass · 0 fail · 68 target` (baseline 629/67,
tức **+18 test / +1 target**, không hồi quy).

## Đã làm

- [`core/observer.rs`](src-tauri/src/core/observer.rs) — `ObserverPolicy` = `Absent` / `Spectate` /
  `Inhabit { cause_id }`.
- Enforcement ở [`sync_lod_focus_system`](src-tauri/src/core/simulation_lod.rs) — **một chỗ duy nhất**,
  vì đó là một chỗ duy nhất camera chạm được vào world.
- `ExperimentManifest.observer` với `#[serde(default)]`, vào fingerprint qua tag `0xF7`, và `validate`
  từ chối `Inhabit` cắm rễ ở `CAUSE_BACKGROUND`.
- Gate: [`tests/observer_policy_tests.rs`](src-tauri/tests/observer_policy_tests.rs) 7 pass +
  11 unit test cho kiểu và manifest.

## Hai chỗ phải lệch khỏi ADR khi va vào code thật — đã ghi vào ADR, không lặng lẽ

1. **Enum ship cả ba biến thể, không phải hai như C1 viết.** LOD-bật là đường đang chạy thật
   (`PixiViewport.tsx` gọi `set_lod_focus`), nên thiếu `Inhabit` sẽ khiến app sống không có chính sách
   hợp lệ nào và LOD bị tắt câm. Ở O1 `Inhabit` **khai báo** nhiễu; **ghi lại** là O2.
2. **Gate không phải checksum hai tiến trình như bảng bằng chứng ban đầu hứa.** Khuôn đó đo quỹ đạo
   thế giới sống, mà đường live chưa tất định (`DETERMINISM_CONTRACT` §5) — dùng nó ở O1 sẽ tạo một
   gate đỏ vì lý do không liên quan tới người quan sát. Thay bằng so **timeline "agent nào xin nghĩ ở
   tick nào"** qua 40 tick với camera đi hết mọi band; so cả chuỗi chứ không so tổng.

## Bẫy mới, đáng nhớ nhất của lượt này

**Thiếu resource `ObserverPolicy` ≠ `Absent`.** Thiếu nghĩa là chưa ai khai báo ⇒ **tuân theo camera**
(giữ nguyên hành vi cũ). `Absent` là khai báo ngược lại ⇒ **cấm** camera. Lẫn hai cái này sẽ âm thầm
tắt LOD của app đang chạy và trông như một cải tiến an toàn.

## Chưa làm, có chủ đích

App sống vẫn ở trạng thái "chưa khai báo". Cho nó `Inhabit` cần một `CauseId` thật, mà cấp phát cause
id thuộc về **O2** (nơi có ledger) — bịa một hằng số bây giờ có thể trùng id do scenario cấp.
Hệ quả của ADR lên `DETERMINISM_CONTRACT` §2 (nguồn rò rỉ thứ năm: camera) và `SNAPSHOT_CONTRACT`
cũng thuộc O2.

---

# 🧭 ADR-0004 + hoà giải tài liệu giữa các phiên (2026-07-26)

Người dùng yêu cầu **kiểm tra các phiên khác xem có lệch hướng không**, rồi **cập nhật tài liệu**.

## Kết luận kiểm tra: không có lệch hướng kỹ thuật

Đối chiếu 4 nhánh, 6 PR và các phiên gần nhất với luật cứng trong CLAUDE.md và kế hoạch G0–G4:
không phiên nào vi phạm hợp đồng nào. Lượt kiểm toán lúc 04:07 (`STATE_OF_THE_PROJECT.md`) là công
việc tốt và các số của nó tự chạy lại được. Vấn đề tìm thấy là **vệ sinh quy trình và mâu thuẫn
tài liệu**, không phải hướng đi.

## Đã sửa trong lượt này

- **Mới:** [`docs/decisions/ADR-0004`](docs/decisions/ADR-0004-observer-as-declared-intervention.md)
  — người quan sát nhập vai là can thiệp được khai báo (`proposed`). Xuất phát từ một phát hiện:
  `LodFocus` do camera lái **đã** là forcing lên thế giới (`cold_agents_stop_asking_entirely`) và
  đang nằm ngoài mọi provenance. Đây là **nguồn rò rỉ thứ năm** bổ sung cho bốn nguồn ở
  `DETERMINISM_CONTRACT` §2.
- **`STATE_OF_THE_PROJECT.md` §3.3 và §3.6 là cùng một việc** (G2 task 1 / hội tụ AE4) nhưng bị xếp
  ở hai bậc P0 và P1. Một phiên nhận §3.3 sẽ chạm tường ngay. Đã buộc hai mục vào nhau và ghi rõ
  §3.6 thực chất là P0.
- **Một việc, hai tên.** `DETERMINISM_CONTRACT` §5 và kế hoạch G0–G4 gọi đường khởi động live là
  **G2**; tài liệu sống gọi là **§3.3**. Đã ghi chú chéo ở cả hai phía thay vì chọn một tên.
- **`STATE_OF_THE_PROJECT.md` §3.8 (mới)** — hai ADR `proposed` giờ đều đang đỡ tải: ADR-0004 dựa
  vào ER01 của ADR-0002, nên việc hoà giải ADR-0002 (§3.10) đã lên giá. Bảng P2 dời số 3.8→3.9…3.13→3.14.
- **§4 thêm một bẫy đã xảy ra thật:** công việc chưa commit của phiên khác có thể đang nằm trên
  nhánh của bạn — chạy `git status` + `gh pr list` trước khi commit.

## Còn mở — cần người dùng quyết

- **PR #6 đang lẫn phạm vi.** Toàn bộ tài liệu kiểm toán + ADR-0004 chưa commit, nằm trên
  `fix/temp-path-collisions` — nhánh có PR #6 mở với tiêu đề chỉ nói về temp path. Cần tách sang
  nhánh docs riêng trước khi push. Chưa tự commit vì đụng công việc chưa commit của phiên khác.
- **ADR-0004 chờ quyết định.** Chưa đem hệ quả của nó vào contract nào, đúng kỷ luật `proposed`.

---

# 🔍 Kiểm toán toàn dự án + tài liệu bàn giao (2026-07-26)

Người dùng yêu cầu **đánh giá dự án và chấm điểm**, rồi **cập nhật tài liệu + đề xuất việc cho phiên sau**.

## Kiểm toán: chạy lại toàn bộ gate, không trích dẫn tài liệu

Trên `main` tại `c0a3cff`, cây làm việc sạch. Mọi số dưới đây là **số đo trong ngày**:

- `cargo test --features desktop --no-fail-fast` → **629 pass · 0 fail · 4 ignored**, 67 test binary,
  **0 warning biên dịch**.
- `check_test_targets.mjs` → 65 target, **0 target chạy rỗng**.
- `cargo fmt --check` và `cargo clippy --all-targets --features desktop -- -D warnings` → **sạch cả hai**.
- `npm run test` → 13 file · **90 pass**. `npm run test:frontend` → 26 file · **243 pass**, 1 skip.
- `npm run lint` → **0 error**, 491 warning (ratchet baseline 491, giữ nguyên). `npm run build` → pass.
- `check_docs_links.mjs` → 245 link, **0 gãy**.

**Điểm: 8,0/10.** Kỹ thuật loại giỏi, sản phẩm loại khá.

## Ba con số nói nhiều nhất về chất lượng

- **5** `.unwrap()/.expect()` trong toàn bộ Rust *production* (con số thô 275 gần như nằm hết trong
  `#[cfg(test)]`). Với 47,7k dòng thì đây là kỷ luật hiếm.
- **2** khối `unsafe` trong cả backend — nhưng **cả hai đều thiếu `// SAFETY:`** (`ai/model.rs:360`,
  `unsafe impl Send/Sync for BrainModel`, type này ôm `WgpuDevice`). Đây là 2/2, không phải 2 trên nhiều.
- **3** marker `TODO/FIXME` trong mã nguồn. Không có vùng code bị bỏ hoang.

## Khoảng cách thật của dự án — KHÔNG phải chất lượng, mà là bằng chứng trên đường mặc định

1. **Não tiến hoá per-agent đang TẮT mặc định.** `BrainPolicy::default()` có `evolved: false` ⇒ một run
   mặc định vẫn là **mọi agent dùng chung một `BrainModel`** — đúng gap mà `MAP_AND_ML_UPGRADE_RESEARCH.md`
   gọi là lớn nhất. Máy móc đã xong và đã test (**11/12 gate EB pass**); nó chỉ đang tắt.
   **Phát hiện đáng giá nhất của lượt kiểm toán:** gate còn lại **EB-S04** fail vì khởi tạo model dùng chung
   đã đổi từ ngẫu nhiên sang **có seed** — tức fail vì một **cải tiến có chủ ý**, không phải hồi quy. Một gate
   không thể pass bằng cách viết code đúng thì phải được **re-baseline tường minh**, và đó mới là việc cần
   làm trước, không phải lật cờ.
2. **Số hiệu năng là proxy.** `BENCHMARK_BASELINE.md` tự khai điều này (không chạy full backend vì đã crash
   máy dev) ⇒ tuyên bố "60 FPS real-time" **chưa từng được đo**, và mọi quyết định về scale đang dựa trên
   ước lượng.
3. **Thế giới Bevy sống chưa experiment-ready.** Phần khoa học AE1–AE3 nằm ở `ReferenceEvolutionWorld`
   **headless**. Lệnh cấm tuyên bố experiment-ready trong CLAUDE.md vẫn đang đúng.

## Nợ nền tảng đã biết (không phải nợ bảo mật — advisory đã sạch và đã có gate)

- `burn`/`wgpu` chưa gate được sau feature. **Blocker là hình dạng, không phải khối lượng:** `learn_handle`
  (`simulation_loop.rs:182`) gán từ `if has_wgpu {…} else {…}` mà hai nhánh không `cfg` riêng được.
- `tokio = { features = ["full"] }` vẫn vô điều kiện (`Cargo.toml:56`), dù hai hệ con cần nó đã nằm sau feature.
- G2 gate #1 (một thay đổi luật đổi cả hai engine) còn dở — `anima-domain` đã tách, cần thêm workspace member.
- Vòng đời thread: cần supervisor + cancellation token. **Đính chính bản ghi G2 cũ:** `inference_handle`
  từng bị drop thì **nay đã được join** (`simulation_loop.rs:660` có ghi chú) — phần còn thiếu không phải cái handle đó.
- Nợ framework: `burn` 0.13.2 (ghim, có lý do), `bevy_ecs` 0.13, React 18→19, `@react-three/fiber` 8→9.
- 491 warning ESLint bị **đóng băng** ở baseline: ratchet chặn tăng nhưng không ép giảm.
- ADR-0002 vẫn `proposed` dù AE1–AE3 đã ship ⇒ theo quy tắc 6 của chính sách tài liệu, phải **mở finding**,
  không tự coi code là đúng.

## Tài liệu đã cập nhật trong lượt này

- **Mới:** [`docs/planning/STATE_OF_THE_PROJECT.md`](docs/planning/STATE_OF_THE_PROJECT.md) — tài liệu sống,
  tên ổn định: trạng thái đo được + bậc đạt được từng hệ con + backlog P0/P1/P2 kèm **điểm neo file:symbol**
  và **định nghĩa hoàn thành** cho từng mục + bẫy đã biết + bộ lệnh xác minh đầy đủ.
- `CLAUDE.md`: thêm mục **Start here** trỏ tới tài liệu trên. Ranh giới: CLAUDE.md giữ **luật không đổi giữa
  các phiên**, STATE_OF_THE_PROJECT giữ **trạng thái đang đổi**.
- `docs/planning/README.md`: thêm hàng "Bắt đầu ở đây".
- `handoff.md`, `plan.md`: gắn nhãn **lịch sử** — chúng mô tả công việc Phase 1 / Phase 6 đã xong từ lâu và
  đang trông như tài liệu hiện hành.

---

# 🎲 [MỚI NHẤT] C2 — RNG CÓ SEED cho sim sống + nghiên cứu nâng cấp map/ML (2026-07-25)

Người dùng yêu cầu nghiên cứu sâu **map** và **mô hình machine**, tham khảo nguồn mở/paper/Reddit/X.
Kết quả khảo sát: **[`docs/research/MAP_AND_ML_UPGRADE_RESEARCH.md`](docs/research/MAP_AND_ML_UPGRADE_RESEARCH.md)** (status `proposed`).
Chốt với người dùng: **hybrid tiến hoá + học-trong-đời**; làm **C2 trước**.

- **Phát hiện lớn nhất (chưa làm, là B1):** `BrainModel::new(15,64,4)` là Bevy Resource → **toàn bộ agent
  dùng CHUNG một bộ não**; genome chỉ có hình thái. Kèm ràng buộc hẹp thứ hai: 4 output actor đi thẳng vào
  `InertiaComponent.cpg_parameters` → não là **bộ điều khiển dáng đi**, không phải bộ ra quyết định. Hai
  thứ này phải sửa CÙNG nhau mới quan sát được đa dạng hành vi.
- **✅ C2 XONG — RNG có seed.** `core/resources.rs`: `SimRng` (resource; `StdRng` + seed; đọc `ANIMA_SIM_SEED`,
  mặc định `DEFAULT_SIM_SEED=1337`; có `reseed`/`seed()`), `sim_seed_from_env()`, `derived_rng(stream)` +
  `sim_stream::{WORLD_INIT, EVOLUTION}`. **Cả 8 điểm `thread_rng()` biến mất khỏi `src/`**: 4 system nhận
  `ResMut<SimRng>` (`spawn_food_system`, `seed_dropping_system`, `check_epoch_completion_system`,
  `manual_migration_system`); 3 hàm tiến hoá nhận `&mut impl Rng` (`mutate_genotype`, `crossover_genotypes`,
  `MapElitesArchive::select_parent`); world-init + luồng tiến hoá lấy sub-stream riêng.
- **Lỗi thứ hai phát hiện khi sửa — `HashMap` phá tái lập.** `MapElitesArchive.grid` là `HashMap`, mà
  `RandomState` **gieo hạt theo tiến trình** → thứ tự duyệt đổi mỗi lần chạy. `select_parent` duyệt chính
  collection này ⇒ **có RNG seed vẫn không tái lập**. Đã đổi `HashMap` → **`BTreeMap`** (khoá `(i32,i32)`
  đã `Ord`; `len`/`get`/`insert`/`iter`/`clear` giữ nguyên nên không call-site nào phải sửa).
- **Vì sao nhiều stream chứ không một:** world setup / Bevy schedule / luồng tiến hoá chạy **đồng thời**;
  một stream chung sẽ khiến kết quả phụ thuộc **thứ tự lập lịch thread** — đúng thứ đang loại bỏ.
- **Gate mới:** `tests/sim_determinism_tests.rs` **11/11** — cùng seed ⇒ cùng chuỗi; seed khác ⇒ phân kỳ;
  `reseed` tua lại; sub-stream độc lập + tái lập; **chọn cha mẹ / mutation / crossover replay**; archive duyệt
  theo **thứ tự khoá niche** (chốt lỗi `HashMap`); và một test **quét mã nguồn** fail nếu `thread_rng()` quay lại.
- **6 test world phải khai báo seed.** `ResMut<SimRng>` panic nếu resource vắng — **đó là hành vi đúng**: một
  world không khai báo seed thì không có câu chuyện tái lập nào. 7 test (challenger_meta_ai, environmental_elements
  ×2, hrrl, lineage_stress ×2 — cái thứ 2 do poisoned mutex lan ra, meta_ai_stress) đã thêm
  `world.insert_resource(SimRng::from_seed(0x5EED))`. Các test này đều là **zero-alloc**, và chúng xanh trở lại ⇒
  `StdRng` không cấp phát trên hot path (đúng như `ThreadRng` trước đó).
- **Verify (số thật, chạy đầy đủ):** `cargo test --no-fail-fast` toàn backend → **358 passed · 1 failed · 1 ignored**,
  **0 lỗi build**. Riêng `terrain_challenger_tests::test_terrain_zero_heap_allocations_erosion_hotpath` là **FLAKY,
  KHÔNG liên quan**: chênh đúng **1 allocation (11 vs 12)**, **xanh khi chạy riêng 3/3 lần** và xanh khi chạy lại cả
  file — đếm allocation bằng global allocator bị nhiễu khi 4 test trong file chạy song song. Thay đổi này không đụng
  `terrain.rs` hay bất kỳ đường nào nó gọi.
- **Clippy:** `cargo clippy --lib --tests` → **0 chẩn đoán ở TOÀN BỘ file thuộc thay đổi này** (40 chẩn đoán còn lại
  đều ở file khác, có sẵn từ trước).
- **Lưu ý phối hợp:** một phiên song song đang sửa backend (`core/evolution_pathway.rs`, hằng `ae3::*`,
  `ReferenceEvolutionWorld`) trong CÙNG working tree; vài lần `cargo check` giữa chừng gãy vì bản ghi dở của họ.
  Số ở trên lấy lúc lib compile sạch.
- **Còn nợ của C2:** save/load **chưa mang seed lẫn vị trí draw** ⇒ nạp lại một run đã lưu chưa tiếp tục đúng
  chuỗi ngẫu nhiên. `SimRng::reseed` đã sẵn; wiring vào `SavedSimulationState` là bước riêng.
- **Tinh chỉnh:** seed run → env `ANIMA_SIM_SEED`; thêm stream mới → hằng số trong `resources::sim_stream`.
- **✅ ADR-0003 ĐÃ VIẾT (chưa code):** [`docs/decisions/ADR-0003-evolved-per-agent-brains.md`](docs/decisions/ADR-0003-evolved-per-agent-brains.md)
  (`proposed`) — não di truyền theo cá thể + mở rộng action space. Chốt: `BrainGenotype` là **anh em** của
  `MorphologyGenotype` (KHÔNG nằm trong `DevelopedPhenotype`) vì **ADR-0001 accepted nhưng CHƯA triển khai**
  (không có `ecomorph.rs`/`develop_at_birth` trong code) — ADR-0003 không được phụ thuộc vào nó · giao diện
  **cố định** ở v1 (CPPN/HyperNEAT hoãn sang ADR riêng, kích hoạt bởi gate EB-S09) · **4 van hành động mới chỉ
  mở van cho hành vi ĐÃ tồn tại** (pheromone hiện phát vô điều kiện mỗi tick, combat/ăn kích hoạt tự động theo
  khoảng cách) nên mặc định "luôn mở" tái lập đúng hành vi hôm nay · inference **không qua `burn`** (matmul thủ
  công, zero-alloc) ⇒ `burn` co lại còn đường training, giảm rủi ro nâng 0.13→0.21 · rollback
  `brain_genotype=None` theo tiền lệ `exotic_energy=None` · học-trong-đời sau cờ, **mặc định TẮT** · 12 gate
  **EB-S01…EB-S12**.
- **Rủi ro mở đã ghi thẳng trong ADR:** trọng số 5.769 `f32` ≈ **22,5 KiB/agent** ⇒ 1 triệu agent ≈ **21 GiB**
  ⇒ **Simulation-LOD (M3 backend) trở thành điều kiện tiên quyết của quy mô**, không còn là tối ưu hoá.
- **✅ ADR-0003 BƯỚC 1 XONG — hoà giải seed (trả nợ C2).** D07 quy định seed lấy từ `WorldIdentity.seed`;
  `SimRng::from_env()` cũ đọc `ANIMA_SIM_SEED` là sai thứ tự thẩm quyền. Nay: `resolve_run_seed(world_seed)`
  (world là nguồn, env chỉ **override** cho sweep headless) · **`init_world` là nơi DUY NHẤT** chèn `SimRng`
  ⇒ mọi test dùng `init_world()` tự có (đã gỡ 2 dòng insert thừa) · luồng tiến hoá sinh ra **trước** world nên
  dùng `world_seed_from_disk()` + **`WorldArtifact::peek_seed`** (đọc mỗi header 36B, không giải mã payload
  2048²) · `derived_rng(run_seed, stream)` nhận seed tường minh thay vì tự đọc env.
- **✅ ADR-0003 BƯỚC 2 XONG — `evolution/brain_genotype.rs` (hàm thuần, KHÔNG system nào đọc).** `ArchSpec`
  `I → H → H → {A actor, 1 critic}` **khớp `ActorCriticModel`** (2 lớp trunk — ADR viết nhầm "1 lớp ẩn", đã sửa)
  · `BrainGenotype{version, arch, weights}` + validate · khởi tạo **He cho trunk / Xavier cho head** (một sigma
  chung sẽ bão hoà sigmoid ở bề rộng 64 ⇒ quần thể khởi đầu trông giống hệt nhau vì lý do không liên quan chọn lọc)
  · `mutate_brain(rate, sigma)` không bao giờ đổi kiến trúc · `crossover_brains` uniform per-weight, arch lệch thì
  fallback parent A · **`forward_into` zero-alloc** (buffer của caller) + wrapper cấp phát cho test. **17/17 unit test.**
- **⚠️ Bẫy đã ghi cho bước 3 (parity EB-S02):** layout ở đây là `w[out*fan_in + in]`, **CHUYỂN VỊ** so với
  `burn 0.13` (`Linear::weight` shape `[d_input, d_output]`, `input.matmul(weight)`). Chép phẳng không chuyển vị
  ⇒ mạng chạy được, số hữu hạn, **sai âm thầm**. Burn cũng init **cả bias** từ `U(-k,k)`, không phải 0.
- **Lỗi của chính tôi do full-suite bắt được:** 3 test đọc/ghi `ANIMA_SIM_SEED` chạy song song trong cùng tiến trình
  ⇒ đua nhau; xanh khi chạy riêng file, đỏ khi chạy cả suite (đúng loại lỗi với test terrain flaky). Đã thêm
  `ENV_LOCK` mutex (recover poison để lỗi thật không bị che). **5/5 lần chạy liên tiếp xanh.**
- **Verify bước 1+2:** `cargo test --no-fail-fast` toàn backend → **383 passed · 1 failed · 0 lỗi build**. Failure là
  `core::evolution_pathway::tests::ae302_...` của **phiên song song** — file đó **không tham chiếu bất kỳ thứ gì
  tôi sửa** (grep 0 kết quả) và **xanh khi chạy riêng**. `sim_determinism_tests` **15/15**, `brain_genotype` **17/17**.
  `cargo clippy --lib --tests` **0 chẩn đoán ở mọi file tôi sửa**.
- **✅ ADR-0003 BƯỚC 3 XONG — parity gate EB-S02.** `ActorCriticModel::from_flat_weights(...)` trong
  `ai/model.rs` (**lần đầu chạm runtime, thuần additive**: một constructor + 2 hàm private, không đổi hành vi
  model đang chạy). Nhận `usize` chứ không `ArchSpec` để **`ai` không phụ thuộc `evolution`** và
  `brain_genotype` không phải kéo `burn` vào (quyết định 5 của ADR). `transpose_to_burn` tách riêng + unit-test
  vì lỗi chuyển vị là **vô hình**: cùng độ dài, và với lớp vuông thì vẫn chạy.
- **`tests/brain_parity_tests.rs` 8/8:** parity trên arch đang chạy (15×64×4), arch mở rộng (15×64×8),
  **4 arch mọi chiều khác nhau** (arch vuông sẽ để lọt lỗi chuyển vị), input bão hoà (ReLU tắt/mở hẳn + sigmoid
  vào đuôi phẳng), **độc lập theo hàng trong batch** (burn chạy batch, đường thủ công chạy từng agent — chỉ hoán
  đổi được nếu output của một hàng không phụ thuộc hàng khác), và genome đã qua mutation+crossover (phân phối mà
  He/Xavier không bao giờ sinh ra).
- **Hai test chứng minh gate CÓ LỰC:** một trọng số lệch, hoặc một lớp bị chuyển vị, **phải** phá parity — nếu
  tolerance đủ lỏng để nuốt trôi thì mọi assert còn lại chỉ là trang trí.
- **Tolerance có căn cứ, không nói suông:** hạ tạm xuống `1e-12` để đọc sai số thật → actor **`1.8e-7`**
  (đúng bằng `f32::EPSILON`, tức **một ULP** — hai bản cài đặt khớp tới giới hạn của kiểu), critic **`8.0e-7`**.
  Chốt `TOLERANCE = 1e-5`: dư 1–2 bậc so với nhiễu float, vẫn thấp xa mọi lỗi thật.
- **Verify bước 3:** `cargo test --no-fail-fast` toàn backend → **395 passed · 0 failed · 0 ignored-fail ·
  0 lỗi build** (lần chạy này cả file của phiên song song cũng xanh). `cargo clippy --lib --tests` **0 chẩn đoán
  ở mọi file tôi sửa**. Gate **EB-S01, EB-S02 pass**; 10 gate còn lại pending.
- **✅ ADR-0003 BƯỚC 4 XONG — mở rộng action space, van mặc định MỞ ⇒ hành vi không đổi (EB-S05).**
  `core::components::ActionGates{pheromone_emit, attack_intent, feed_intent}` + `ACTION_GATE_THRESHOLD=0.5`
  (**ngưỡng tất định** — van xác suất sẽ phải dùng RNG, tức nối kết quả sinh thái vào thứ tự draw, đúng thứ
  `SimRng` sinh ra để tránh). `ActionGates::of(None)` đọc là **MỞ** — save cũ không được nạp thành agent từ chối ăn (D09).
  Nối: `agent_release_pheromone_system` (nhân + clamp `[0,1]` chống output não loạn), `detect_food_collisions_system`,
  và **CẢ HAI nhánh** của `combat_system` (nhánh có/không `CombatEvents` — predator không được đánh ở đường này mà
  nhịn ở đường kia). `decode_genotype` gắn `ActionGates::default()`. **Chưa có gì ghi vào component** — chỉ là chỗ cho bước 5.
- **Tinh chỉnh phạm vi (phát hiện khi làm):** **KHÔNG** nới `BrainModel::new(15,64,4)`→`(15,64,8)` ở bước này.
  Đổi số tham số model dùng chung ⇒ đổi lượng RNG tiêu thụ lúc init ⇒ đổi trọng số ⇒ **đổi quỹ đạo hôm nay**, tức
  tự phá EB-S04. Tensor hành động chỉ nới khi `brain_genotype=Some(..)`, nơi arch rộng là **của riêng cá thể**.
  ⇒ Bước 4 **không đụng IPC/TypeScript** như dự đoán ban đầu trong ADR.
- **`tests/action_gates_tests.rs` 13/13:** mỗi van có **cặp** test — *đồng nhất* (`default()` == không có component)
  **và** *nhạy* (van đóng thì chặn thật). Chỉ có test đồng nhất thì một van bị bỏ quên vẫn pass.
- **Một giả thiết của tôi SAI, đã sửa:** định dò rủi ro thứ-tự-archetype bằng tranh giành **thức ăn** → 4/4 agent đều
  ăn được (`[40,40,40,40]`), vì `despawn` là **deferred Commands** ⇒ trong một tick không hề có tranh giành. Chỗ thật
  sự nhạy thứ tự là **combat**: nó sửa energy **trực tiếp** và `predation_capture` phụ thuộc energy **hiện tại** của
  con mồi ⇒ ai đánh trước ăn miếng đậm nhất. Đã đổi sang 3 predator / 1 prey, assert các phần **không bằng nhau**
  (nếu bằng nhau thì test không phát hiện được đảo thứ tự) rồi so có-van vs không-van.
- **Vì sao phải dò:** thêm component **đổi archetype**, Bevy duyệt query **theo archetype**, và cả hai system đều
  `break` ở kết quả đầu — van mở thì *số học* không đổi nhưng **ai** ăn/**ai** bị ăn vẫn có thể đổi. Các test đồng
  nhất per-system không thấy được vì mỗi cái chỉ có một actor.
- **Verify bước 4:** `cargo test --no-fail-fast` → **407 passed · 1 failed · 0 lỗi build**. Failure là
  `terrain_challenger_tests::test_terrain_zero_heap_allocations_erosion_hotpath` — **flaky đã biết, không liên quan**
  (đã tạo task riêng). `cargo clippy --lib --tests` **0 chẩn đoán ở mọi file tôi sửa**.
  Gate **EB-S01, EB-S02, EB-S05 pass**; 9 gate còn lại pending.
- **🟡 ADR-0003 BƯỚC 5 — MỘT PHẦN.** Xong phần dữ liệu + vòng đời; **chưa** nối vào suy luận.
  - **`core::components::AgentBrain{genotype, learned}`**: `genotype` là cái sinh sản chép, `learned` là
    runtime state chết theo cá thể. `live_weights()` ưu tiên `learned`. **Không Lamarck** — có test khoá.
  - **Save + migration (D02)**: `SerializedAgent.brain` và `AgentMigrationData.brain`, cả hai `#[serde(default)]`.
    Restore (`spawn_serialized_agent`) và migration (`SpawnMigrationCommand`) **mang theo** brain chứ **không sinh
    mới** (D01 — cấp brain mới cho cá thể restore là tạo sinh vật khác đội cùng lineage id). Brain hỏng (độ dài
    `learned` lệch arch) bị **từ chối + log**, agent rơi về model dùng chung thay vì chạy nhiễu.
  - **`core::resources::BrainPolicy`** (resource, mặc định **TẮT**, bật bằng `ANIMA_EVOLVED_BRAINS`; resource chứ
    không phải đọc env rải rác ⇒ test set trực tiếp được). `EVOLVED_ARCH` = 15×64×**8** và **`action_index`** chốt
    ý nghĩa từng output (0..4 CPG, 4 pheromone, 5 attack, 6 feed, 7 signal dự trữ) — để off-by-one không biến
    "ăn" thành "đánh" một cách im lặng.
  - **Genesis + `SpawnGenotypeCommand`** tạo brain khi cờ bật, rút từ `SimRng` ⇒ cùng seed, cùng quần thể sáng lập.
  - **`tests/brain_persistence_tests.rs` 14/14** (EB-S07, EB-S10): restore giữ đúng brain · **restore 2 lần từ
    cùng payload cho kết quả giống hệt** (nếu bị roll mới thì agent vẫn "có brain, vẫn hợp lý, vẫn đúng lineage id"
    — chỉ so hai lần restore mới lộ) · `None` không bị âm thầm nâng cấp · save cũ không có trường `brain` → `None` ·
    `learned` round-trip và `live_weights()` ưu tiên nó · payload migration qua wire · policy mặc định tắt + tái lập.
- **✅ ADR-0003 BƯỚC 5b XONG — brain riêng ĐIỀU KHIỂN HÀNH VI THẬT.** Phát hiện khi làm:
  **`ai::model::brain_inference_system` KHÔNG nằm trong schedule**; đường sống là `sensory_system` →
  `InferenceRequestBatch` **qua channel** → worker thread → `action_resolution_system`.
  - `AgentBrain.genotype` → **`Arc<BrainGenotype>`** (bật feature `rc` của `serde`, **không thêm crate**), nên
    `AgentInferenceRequest.brain` chỉ **tăng refcount** thay vì chép ~23 KiB trọng số mỗi agent mỗi tick.
    Test dùng **`Arc::ptr_eq`** để chứng minh không chép — so bằng giá trị sẽ pass kể cả khi đã chép.
  - Worker **LỌC** request có brain ra khỏi lô Burn thay vì chạy hết rồi ghi đè ⇒ đường legacy giữ
    **bit-identical** khi không agent nào có brain (nền EB-S04), và không trả tiền cho forward pass bị vứt.
    Lô rỗng thì bỏ Burn hẳn (tensor 0 hàng). Buffer worker (`shared_slots`/`shared_actions`/`brain_scratch`)
    cấp phát 1 lần rồi tái dùng, cùng kiểu với `inputs` sẵn có.
  - `AgentInferenceResponse.actions` `[f32;4]`→`[f32;8]`; `action_resolution_system` ghi `0..CPG_LEN` vào CPG,
    3 slot sau vào `ActionGates`. **`LastTransitionState.action` GIỮ 4 slot** — nó nuôi A2C của model dùng chung,
    vốn không biết gì về van, nên không có lý do phình save format.
  - **Fallback khi brain lỗi = van MỞ + không đổi vận động**, không phải vector 0: vector 0 đọc thành "đóng mọi
    van", tức agent lặng lẽ ngừng ăn vì lý do không liên quan chọn lọc.
- **⚠️ Đính chính dự đoán của chính tôi trong ADR:** đã ghi bước này "chạm IPC + TypeScript". **SAI.** Vector hành
  động nằm trọn backend — `grep` `src/commands/` và toàn bộ `src/**.ts(x)` cho **0 tham chiếu** tới
  `cpg_parameters`/`actions`. **Toàn bộ ADR-0003 không chạm frontend.** Đã sửa cả phần "Hệ quả tiêu cực" của ADR.
- **`tests/brain_action_routing_tests.rs` 9/9:** agent có brain gửi đúng genome (và **chia sẻ** chứ không chép) ·
  agent legacy không gửi gì · output van tới đúng `ActionGates` (brain quyết định "săn nhưng không ăn" — điều
  **bất khả** trước bước này) · CPG vẫn lấy 4 slot đầu · response của model dùng chung để van **mở nguyên** ·
  agent không có component van vẫn resolve được · **hai genome khác nhau ra quyết định khác nhau trên cùng input**.
- **Verify bước 5b:** `cargo test --no-fail-fast` → **430 passed · 1 failed · 0 lỗi build**; failure là
  `terrain_challenger_tests::...erosion_hotpath` (**flaky đã biết, không liên quan**). `cargo clippy --lib --tests`
  **0 chẩn đoán ở mọi file tôi sửa**. Gate **EB-S01, EB-S02, EB-S05, EB-S07, EB-S10 pass**; 7 còn lại pending.
- **✅ ADR-0003 BƯỚC 6 XONG — ĐỐI CHỨNG CÓ SEED, và nó tìm ra một lỗi.**
  - **Harness:** `tests/brain_controlled_comparison_tests.rs` **11/11** — chạy vòng lặp ECS headless (không cần
    Tauri) và bơm channel suy luận bằng **CHÍNH** `run_inference_batch` mà worker gọi. Để làm được, logic worker
    được **tách khỏi closure của thread** ra `ai::model::run_inference_batch` + `InferenceScratch`; trước đó nó
    **không test được** — mà đó lại là hàm quyết định mọi hành động của mọi agent mỗi tick.
  - **🔴 LỖI TÌM ĐƯỢC (C2 chưa bắt): model dùng chung khởi tạo KHÔNG tất định.** Hai lần chạy cùng seed cho
    **vị trí và năng lượng khớp** nhưng `cpg_parameters` **khác**. Nguyên nhân: `LinearConfig::init` trả
    `Param::uninitialized` — trọng số materialize **LƯỜI** từ RNG tĩnh toàn tiến trình **tự tiến lên** mỗi lần
    rút. Nên gọi `Backend::seed` trước lúc dựng **KHÔNG sửa được gì**: model thứ hai rút từ generator đã bị model
    thứ nhất đẩy đi. (Tôi đã thử `Backend::seed` trước — vẫn fail cả khi chạy đơn luồng riêng lẻ, đó là cách loại
    trừ giả thuyết "race".)
  - **Sửa:** `BrainModel::new_seeded` tự rút trọng số từ stream có seed rồi nạp qua `from_flat_weights`, giữ
    **nguyên phân phối** `U(-k,k)`, `k=sqrt(1/fan_in)` của Burn. Cả world lẫn worker dùng cùng run seed — trước
    đây hai model đó **chưa bao giờ giống nhau** dù lẽ ra phải giống.
  - **🔴 Regression của chính tôi, đã sửa:** bản đầu materialize tensor **NGAY** ⇒ đẩy `SimulationEngine::start`
    chậm quá ngưỡng, `environmental_elements_stress_tests` (2 test) fail vì engine chưa kịp tick trong 150ms.
    Sửa bằng `Param::uninitialized` — giữ **lười** materialize như Burn, nhưng **giá trị đã được quyết định**.
  - **Hệ quả phải nói thẳng:** quỹ đạo baseline **KHÔNG** còn trùng bản dựng trước ADR — vì trước đó *không tồn
    tại* một quỹ đạo baseline ổn định để mà trùng. Đây là sửa lỗi có chủ ý, không phải hồi quy.
- **KẾT QUẢ ĐO ĐƯỢC (EB-S11):** cùng một quan sát đưa cho cả quần thể → `None` cho **1** chính sách duy nhất
  (đúng bản chất model dùng chung), `Some` cho **8/8 chính sách khác nhau**, và khác **cả ở kênh sinh thái** chứ
  không chỉ dáng đi; ít nhất một agent kết thúc run với van **lệch khỏi mặc định mở**. Cùng seed thì tái lập,
  seed khác thì quần thể khác, bật cờ thì run đổi. **Đa dạng hành vi là có thật và đo được.**
- **Còn nợ:** **độ phủ MAP-Elites archive** chưa đo được headless (cần luồng tiến hoá) ⇒ nửa còn lại của EB-S11
  vẫn pending.
- **Verify bước 6:** `cargo test --no-fail-fast` → **441 passed · 1 failed · 0 lỗi build**; failure là
  `terrain_challenger_tests::...erosion_hotpath` (**flaky đã biết**). `cargo clippy --lib --tests` **0 chẩn đoán
  ở mọi file tôi sửa**. Gate **EB-S01, EB-S02, EB-S05, EB-S07, EB-S10, EB-S11(hành vi) pass**; **EB-S04 một phần**;
  còn EB-S03/S06/S08/S09/S12 pending.
- **✅ ADR-0003 BƯỚC 7 XONG — HỌC-TRONG-ĐỜI, và nó tìm ra lỗi thứ hai.**
  - **`brain_genotype::learn_step`**: backprop **viết tay** cho đúng topology này (relu×2 → sigmoid actor +
    critic tuyến tính), cùng `LearnScratch` để một tick học **không cấp phát**. `ai::model::lifetime_learning_system`
    + `resources::LifetimeLearning{enabled, learning_rate, discount, interval, active_radius}`
    (`ANIMA_LIFETIME_LEARNING`, **chỉ có hiệu lực khi `evolved`** — học mà không có não riêng thì không có gì để đổi).
    Reward = chính drive-reduction homeostatic mà model dùng chung đang dùng.
  - **`AgentBrain.learned` → `Option<Arc<BrainGenotype>>`** + `live()`. Học **thay** cả mạng thay vì sửa tại chỗ:
    mạng cũ có thể đang được một request suy luận cầm, giật trọng số ra dưới chân nó sẽ khiến hành động của agent
    phụ thuộc thời điểm thread. Đổi lại, mỗi lần học cấp phát 1 lần ⇒ **`interval` throttle** là để trả giá đó.
  - **🔴 LỖI THỨ HAI TÌM ĐƯỢC: A2C của model dùng chung SAI DẤU.** `run_training_loop` dùng `(a−â)²·(−td)`:
    advantage **dương** ⇒ hệ số âm ⇒ giảm loss = **tăng** `(a−â)²` = đẩy chính sách **RA XA** hành động vừa tốt
    hơn kỳ vọng, và **VỀ PHÍA** hành động tệ hơn. Model dùng chung đã và đang học **ngược**. `learn_step` viết
    **đúng dấu** (`+td`) thay vì sao chép lỗi. Lỗi **có trước** ADR-0003; sửa đổi quỹ đạo legacy ⇒ **đã tạo task riêng**.
  - **Vì sao gradient check một mình KHÔNG đủ:** `the_learning_gradient_matches_finite_differences` pass với
    **CẢ HAI** dấu — nó chỉ kiểm đạo hàm có khớp hàm loss, **không** kiểm hàm loss có đúng ý đồ.
    `learning_moves_the_policy_toward_a_rewarded_action` mới là test phát hiện ra (fail với dấu cũ). Cần **cả hai**.
  - **Hai khác biệt có chủ ý, đều về chi phí:** **SGD thay vì Adam** (Adam cần 2 buffer moment/tham số ⇒ gấp ba
    bộ nhớ per-agent vốn đã là rủi ro quy mô); **chỉ huấn luyện khối CPG** vì `LastTransitionState.action` chỉ ghi
    4 tham số vận động, không có target cho van sinh thái ⇒ v1 phân vai: **tiến hoá đặt chính sách sinh thái,
    học-trong-đời tinh chỉnh dáng đi**. Huấn luyện van cần ghi thêm giá trị van đã dùng = đổi save format, hoãn.
  - **`active_radius` hiện đo từ gốc toạ độ** — chỗ giữ chỗ cho Simulation-LOD (M3 backend còn nợ). Ràng buộc của
    ADR **tồn tại và test được** thay vì chỉ là lời hứa; nối vào tâm LOD thật là phần việc của M3.
  - **`tests/brain_lifetime_learning_tests.rs` 9/9** + 21/21 unit test `brain_genotype`: mặc định không học · học
    cần `evolved` · bật thì đổi thật · **genome không bị ghi ngược** (kiểm qua system đang chạy, không chỉ qua type) ·
    tái lập theo seed · **ngoài active-radius không học** · `interval` chặn thật · agent chưa hành động thì bỏ qua ·
    world không có policy resource **không panic**.
- **Verify bước 7:** `cargo test --no-fail-fast` → **454 passed · 1 failed · 0 lỗi build**; failure là
  `terrain_challenger_tests::...erosion_hotpath` (**flaky đã biết**). `cargo clippy --lib --tests` **0 chẩn đoán ở
  mọi file tôi sửa**. Gate **EB-S01, S02, S05, S07, S08, S10, S11(hành vi) pass**; **EB-S04 một phần**;
  còn **EB-S03, S06, S09, S12** pending (zero-alloc tick với não riêng, closed-EU khi bật chi phí não, bức tường
  brain–body, ngân sách bộ nhớ).
- **✅ EB-S03 + EB-S12 XONG — `tests/brain_budget_tests.rs` 7/7** (ổn định 5/5 lần chạy).
  - **EB-S03 (zero-alloc):** suy luận per-agent **0 alloc/tick** · bước gradient **0 alloc** · cài mạng đã học tốn
    **ĐÚNG 1** alloc — ngoại lệ có chủ ý (thay cả mạng để request đang bay không bị giật trọng số), pin ở **1** để
    nếu thành 1/tick hay 1/trọng số thì test đỏ. **Đường Burn dùng chung KHÔNG zero-alloc và chưa bao giờ**
    (`inputs.clone()` + dựng tensor) — tôi **đo và ghi lại** thay vì assert 0 rồi giấu, vì nó làm rõ "per-agent
    0 alloc" là cải thiện thật chứ không phải cách viết test.
  - **EB-S12 (bộ nhớ):** **22,5 KiB/agent** (5.769 f32); agent có học mang **HAI** mạng ⇒ **45 KiB**, tức **một
    nửa** số agent thường trú trong cùng ngân sách — chi phí thật của nửa Baldwin. Trần công bố
    `BRAIN_BUDGET_BYTES` 24 KiB / 48 KiB ⇒ đổi kiến trúc làm phình bộ nhớ phải là **sửa hằng số có chủ ý**, không
    phải phát hiện ở quy mô lớn. 1 triệu agent ≈ **21,5 GiB** ⇒ ~**46.500 agent thường trú mỗi GiB**. Trong ba
    hướng giảm ADR liệt kê, mới dùng được hướng đầu: `H` 64→32 cho **~3,1×** ít tham số (ma trận trunk→trunk chi phối).
  - **Bẫy lặp lại lần thứ ba:** allocator là **toàn tiến trình**, nên 3 test đo bộ nhớ (không giữ lock) cấp phát
    song song và làm hỏng số của test zero-alloc → **13 alloc** cho vòng lặp đáng lẽ 0. **Mọi** test trong file có
    tracking allocator phải giữ lock, kể cả test không đo alloc. (Lần đầu sửa bằng perl bị lỗi cú pháp nên file
    không đổi mà test vẫn xanh — tức lần xanh đó là **may mắn**; đã sửa lại bằng Edit và chạy 5/5.)
  - **Lưu ý máy:** `cargo test` mặc định làm cạn paging file (`os error 1455` khi mmap rlib). Dùng **`-j 2`**.
- **Verify:** `cargo test --no-fail-fast -j 2` → **462 passed · 0 failed · 0 lỗi build** (lần này test terrain
  flaky cũng xanh). `cargo clippy --lib --tests` **0 chẩn đoán ở mọi file tôi sửa**.
  **ADR-0003: 9/12 gate pass** (EB-S01, S02, S03, S05, S07, S08, S10, S11-hành-vi, S12) · **EB-S04 một phần** ·
  còn **EB-S06** (closed-EU khi bật `brain_metabolic_cost`) và **EB-S09** (bức tường brain–body) — cả hai cần
  tính năng chưa bật, không phải nợ đo đạc.
- **✅ EB-S06 + EB-S09 XONG — `tests/brain_cost_and_coupling_tests.rs` 9/9.**
  - **EB-S06 (chi phí não, closed-EU):** `BrainPolicy.brain_metabolic_cost` (đơn vị: energy/giây trên mỗi 1.000
    tham số, **mặc định `0.0`**) + `AgentBrain::metabolic_cost()`. **Điểm mấu chốt của thiết kế:** chi phí được
    **gộp vào `total_cost`** trong `metabolic_decay_system` chứ **KHÔNG** trừ riêng — vì mọi thứ trong `total_cost`
    chảy qua `decay` → `respired` → `detritus`, nên năng lượng bị **DI CHUYỂN** chứ không bị **HUỶ**, và EU đóng
    *by construction*. Trừ riêng là cách viết tự nhiên hơn và **sẽ rò**. Delta < **1e-9** (đúng ngưỡng S01).
  - Chi phí **có thật và tăng theo kích thước não** (não 15×16×8 tốn ít hơn 15×64×8) ⇒ có áp lực chọn lọc chống
    phình não, đúng lý do ADR đặt ra nó. Tính theo **genome**, không theo mạng đã học — học không làm não to ra
    nên không được làm tăng hoá đơn. `NaN`/âm/0 đều bị bỏ qua thay vì làm hỏng sổ năng lượng.
  - **🔬 EB-S09 (bức tường brain–body) — ĐÃ ĐO, TÍN HIỆU YẾU NHƯNG CÓ.** Reciprocal transplant trên trục hình thái
    (thân **2/3/5/8 đốt × 5 gait**, đo **quãng đường** — chính đại lượng `check_epoch_completion_system` chấm điểm,
    không phải proxy bịa ra). Kết quả:
    - 2 đốt → gait **2** (0,066) · 3 đốt → gait **1** (0,490) · 5 đốt → gait **2** (1,422) · 8 đốt → gait **2** (1,943)
    - Ở thân 3 đốt, gait 1 hơn gait 2 **~53%** (0,490 vs 0,321) — **không phải nhiễu**.
    - ⇒ **Điều khiển tối ưu ĐÃ phụ thuộc cơ thể**, đúng cơ chế dự đoán: 4 tham số CPG áp cho *mọi* khớp bất kể
      cơ thể có bao nhiêu.
  - **Kết luận:** **CHƯA mở ADR phương án D (CPPN/HyperNEAT)** — quần thể khởi tạo vẫn là 10 cá thể **cùng một
    genotype 3-node**, nên bức tường chưa gây thiệt hại thật. Nhưng đây là bằng chứng nó **sẽ** cần khi hình thái
    đa dạng thật, không còn là lo xa. Đọc lại điều kiện kích hoạt khi quần thể có nhiều body plan.
  - **Hệ quả phụ đáng lưu ý:** quãng đường tăng mạnh theo số đốt (0,066 → 1,94) ⇒ so fitness **giữa** các hình thái
    bị nhiễu bởi kích thước cơ thể — thêm một lý do MAP-Elites phải **bin theo khối lượng** thay vì xếp hạng phẳng.
- **Verify:** `cargo test --no-fail-fast -j 2` → **470 passed · 1 failed · 0 lỗi build**; failure là
  `terrain_challenger_tests::...erosion_hotpath` (**flaky đã biết**, xanh ở lần chạy trước). `cargo clippy` **0 chẩn
  đoán ở mọi file tôi sửa**.
  🎉 **ADR-0003: 11/12 gate pass**; chỉ **EB-S04 còn một phần** (và phần thiếu là *không thể đo* — không có bản dựng
  trước ADR để so, xem bước 6). **Toàn bộ ADR-0003 đã triển khai và đo xong.**
- **✅ ADR-0003 CHUYỂN SANG `accepted` (2026-07-25).** Frontmatter `status: accepted` + `accepted_date`;
  [`docs/decisions/README.md`](docs/decisions/README.md) và [`docs/README.md`](docs/README.md) đã cập nhật.
  Thêm mục **"Trạng thái tại thời điểm accepted"** ghi lại đúng những gì đúng khi khoá quyết định, để người đọc
  sau không phải suy ra từ lịch sử: 11/12 gate · EB-S04 thiếu phần **không thể đo** · **3 lỗi có sẵn** đã tìm ra
  (2 đã sửa, 1 tách task) · và **2 quyết định vận hành CÒN ĐỂ MỞ CÓ CHỦ Ý**:
  - **Có bật `brain_metabolic_cost` mặc định không?** EB-S06 chứng minh bật được mà không rò năng lượng, nhưng
    bật nó **đổi áp lực chọn lọc** ⇒ là quyết định của chủ dự án, không phải hệ quả kỹ thuật.
  - **Ngưỡng nào ở EB-S09 thì mở ADR phương án D?** Đọc hiện tại "yếu nhưng có"; ngưỡng nên chốt khi quần thể
    thực sự có nhiều body plan, không phải đoán bây giờ.
- **ADR-0003 giờ là quyết định BẤT BIẾN.** Theo quy ước `docs/decisions/README.md`: **không viết lại để đổi kết
  luận** — muốn đổi thì tạo ADR mới và dùng `supersedes`/`superseded_by`. Ba việc ADR này **không** bao trùm:
  độ phủ MAP-Elites archive (cần luồng tiến hoá headless), Simulation-LOD thật cho `active_radius` (M3 backend),
  và phương án D (CPPN/HyperNEAT) khi EB-S09 vượt ngưỡng.
- **Verify bước 5:** `cargo test --no-fail-fast` → **421 passed · 1 failed · 0 lỗi build**; failure là
  `terrain_challenger_tests::test_terrain_zero_heap_allocations_erosion_hotpath` (**flaky đã biết, không liên quan**,
  đã có task riêng). `cargo clippy --lib --tests` **0 chẩn đoán ở mọi file tôi sửa**.
  Gate **EB-S01, EB-S02, EB-S05, EB-S07, EB-S10 pass**; 7 gate còn lại pending.

---

# 🧹 Housekeeping (chờ backend) — test hồi quy M1 hydrology + đính chính §6 (2026-07-24)

Trong lúc chờ phiên backend xong (để làm Simulation-LOD), làm việc frontend an toàn (không đụng `src-tauri`):
- **Test hồi quy M1** mới (`src/__tests__/worldHydrology.test.ts`, 3 test @512²): world sane (0 NaN, đất ~38%, có Ocean/River/Lake/Beach) · hồ hình thành + **endorheic salt lake** (saline>0 nhưng không phải tất cả bồn) · `riverAmt` ∈ [0,255]. → khoá hành vi spillway/delta/endorheic (v19/v20) vốn trước chỉ có smoke tạm.
- **Đính chính WORLD_DESIGN §6**: stack `LandscapeShowcase/terrainGenerator/terrainCache/Terrain/Water/Vegetation/Sky/Weather/Minimap` **KHÔNG chết** — `src/App.tsx` import + là nền ~½ bộ 237 test; `worldGen.ts` còn dùng `ImprovedNoise2D`. → **KHÔNG gỡ** (đề xuất "dọn" trước đó là SAI).
- **Verify:** src Vitest **57/57** (thêm 3; các test frontend của phiên song song cũng đang xanh) · tsc 0 · lint 0.

---

# 🧩 M3 — Chunk + LOD động + STREAMING cho terrain (đo trần tài nguyên) (2026-07-24)

Mở đường tối ưu cho nhiều agent: chia terrain thành lưới chunk để **frustum-cull** vùng ngoài màn hình + hạ chi tiết chunk xa (**LOD**). **Thuần frontend, file mới, KHÔNG đụng backend** (phiên song song đang ở đó); **KHÔNG bump `WORLD_GEN_VERSION`** (render-side).

- **`utils/chunkLod.ts` mới** (thuần, test được): `makeChunkGrid` (lưới C×C phủ kín [0,1]²), `lodForDistance`/`resForLod` (LOD theo khoảng cách, chia đôi res mỗi cấp, clamp `minRes`), `estimateCost` (đếm tam giác + cull = "đo trần tài nguyên"), `buildChunkGeometry` (mesh 1 chunk **khớp CHÍNH XÁC** quy ước WorldTerrain: X=(u−0.5)·rs, Y=elev·hu, Z=(v−0.5)·rs, uv=(u,v); + skirt che khe giữa chunk khác LOD).
- **`WorldTerrainLod.tsx` mới** (opt-in): terrain chunk-hoá, dùng lại texture builder + material + river-shimmer **y hệt** WorldTerrain (1 material chung, nhiều chunk mesh). **LOD ĐỘNG theo camera**: mỗi frame chunk dưới camera giữ chi tiết cao, chunk xa hạ res; **cache geometry per (chunk,lod)** dựng-1-lần rồi **swap imperatively** (không rebuild/re-render), throttle theo quãng camera di chuyển (`RECOMPUTE_MOVE=30u`), skirt che khe. Mặc định uniform (chỉ culling) = **hình học trùng khít WorldTerrain**.
- **STREAMING** (`updateActiveChunks` + WorldTerrainLod): với `loadRadius>0`, chỉ chunk gần camera **resident** (mount + dựng geometry); chunk xa **unmount + dispose geometry** (thả sau commit qua useEffect) → **trần bộ nhớ cố định** bất kể world to cỡ nào = đường mở world lớn hơn 1 mesh. Hysteresis (`unloadRadius > loadRadius`) chống thrash ở biên. (Streaming thả terrain xa → hợp view mặt đất/sương, KHÔNG hợp overview toàn map → opt-in.)
- **WorldShowcase**: cờ `TERRAIN_CHUNKED` (mặc định **false** → giữ WorldTerrain đã kiểm chứng) + `TERRAIN_DYNAMIC_LOD` + `TERRAIN_LOAD_RADIUS` (0 = không stream). Export 4 hàm build texture từ WorldTerrain (cộng dồn `export`, không đổi hành vi).
- **Verify SỐ (Node, three thật)**: uniform grid 36 chunk = **294912 tam giác KHỚP chính xác single-mesh** (parity=true), 0 NaN, phủ ±600 đủ, **seam gap = 0.0e0** (liền mạch), skirt đúng. **Đo trần tài nguyên (tĩnh)**: overview 100%, walk 19%, fly góc 12%. **LOD động**: camera ở góc (0,0)→near lod0/far(5,5) lod2; sang góc đối diện → **tái tâm chi tiết** (lods A≠B), 0 NaN, ~14% tam giác uniform. → PASS.
- **Verify streaming (Node)**: world **16×16=256 chunk**, camera quét toàn map → **tối đa 22 chunk resident (9%)**, dựng lazy 100/thả 92 khi rời, 0 NaN → trần bộ nhớ chặn. PASS.
- **Verify khác**: tsc 0 · src Vitest **54/54** (+12 chunkLod: LOD động + streaming) · test:frontend **237/237** · lint 4 file M3 **0 lỗi** · `npm run build` ✅.
- **Trung thực**: bản LOD *live* (mượt popping/skirt trông thế nào) CHƯA nhìn tận mắt (Browser pane không hiển thị trên máy này) → để **opt-in OFF**; uniform mode đã chứng minh trùng khít + logic LOD động verify bằng harness/test. Bật: `TERRAIN_CHUNKED=true` (+ `TERRAIN_LOD_DISTANCES=[520,900]`/`TERRAIN_SKIRT=6` cho LOD).
- **Còn lại M3**: chỉ còn **Simulation-LOD backend** (agent trong active-radius chạy brain đầy đủ, ngoài đó cập nhật thống kê) — ở backend, nên phối hợp với phiên đang sửa backend để tránh xung đột. (Terrain chunk + LOD động + streaming: XONG.)
- **Tinh chỉnh**: `TERRAIN_CHUNKS_PER_SIDE`/`TERRAIN_LOD_DISTANCES`/`TERRAIN_SKIRT` (WorldShowcase); `resForLod`/`minRes` (chunkLod.ts).

---

# 🕳 M4 — HANG ĐỘNG THẬT: hốc 3D thay decal phẳng (2026-07-24)

Theo yêu cầu "hang động" trong danh sách môi trường. Thay decal ellipse đen phẳng (`WorldCaves` cũ) bằng **hình khối 3D thật**. **KHÔNG bump `WORLD_GEN_VERSION`** (data hang `cave*` không đổi — thuần render, cache giữ nguyên).

- **`utils/caveGeometry.ts` mới** (`buildCavesGeometry`): mỗi cửa hang thành **funnel đá noise-displaced** — loe rộng ở miệng (gồ nhẹ khỏi vách theo hướng downhill) rồi thóp dần về **túi tối kín**; vertex color tối dần vào trong (miệng sáng đá, đáy gần đen) nên hốc đọc là hang dù nắng chiều nào; jitter bán kính per-vertex cho vách gồ ghề. Ngồi trên **đúng mặt mesh** qua `sampleMeshHeight`. Tất cả gộp **1 BufferGeometry / 1 draw call** (97 vertex + 180 tam giác mỗi hang).
- **`WorldCaves.tsx`**: đổi từ InstancedMesh CircleGeometry unlit → 1 `<mesh>` gộp + `MeshStandardMaterial` vertexColors, DoubleSide, có ánh sáng (hết cảm giác sticker đen). Thêm prop `meshResolution`.
- **Giới hạn trung thực (heightmap)**: mesh terrain liên tục không đục lỗ được ở res 384, nên hang xuyên-núi đi-vào-được sẽ bị vách che. Hốc concave lồi nhẹ là cách render cửa hang KHÔNG bị che trên biểu diễn heightmap; hang xuyên/overhang thật cần **voxel/SDF terrain** (WORLD_DESIGN M4+). Vẫn là nâng cấp thật (khối 3D + parallax + đổ bóng) so với decal.
- **Verify:** tsc 0 · lint 2 file hang 0 lỗi · builder standalone Node **PASS** (@1024² 12 hang→1164 vertex/2160 tam giác; @2048² 50 hang→4850 vertex/9000 tam giác; **0 NaN, 0 index lỗi, có color+normal, Y ngồi trên vách 0–88u**) · src Vitest **42/42** · test:frontend **237/237** · `npm run build` ✅. World v20 nạp trong browser (50 hang, 0 lỗi three/WebGL). *(Screenshot live bất khả — Browser pane không hiển thị trên máy này; verify qua builder standalone + render path chuẩn + tests.)*
- **Tinh chỉnh:** `RINGS`/`SEG` (độ mịn), `depth`/`mouthR`/`sag` (hình dạng), tông đá `rim`/`deep` trong caveGeometry.ts.
- Lưu ý: 1 lỗi `npm run lint` ở `scripts/bench_baseline.mjs` là của phiên song song (M2/benchmark), không phải M4.

---

# 🏝 v20 — M1 HOÀN TẤT: delta cửa sông + hồ nội lưu (endorheic salt lake) (2026-07-24)

Vá nốt vật lý thủy văn (M1) theo yêu cầu "đủ môi trường + đúng vật lý". **Bump `WORLD_GEN_VERSION` 19→20.**

- **Delta cửa sông** (Pass 4d mới): nơi sông (`riverAmt≥150`) gặp biển, bồi tích cát theo quạt flow-scaled vào vùng nông (depth<0.03) → đáy nông dần, ô vượt mực biển nổi thành **cồn cát Beach** (delta thêm chút đất — đúng thực tế, đất vẫn ~38%). ~90 cửa sông @1024².
- **Hồ nội lưu (endorheic)** (Pass 4b-2 tách THEO TỪNG hồ): `computeLakes` giờ trả `outletPaths` per-basin (thay flat `outlets`). Mỗi bồn tính moisture TB trên ô hồ; **khô (< `ENDORHEIC_MOISTURE=0.24`) → hồ TẬN**: KHÔNG route sông thoát (bốc hơi cân bằng dòng vào), đặt cờ `saline` + viền **salt flat** (Beach) quanh hồ. Ẩm → vẫn spillway ra sông như v19. → **4/25 hồ nội lưu @1024² (~16%, khớp Trái Đất ~18% đất nội lưu)**. `LakeBasin` thêm cờ `saline?`.
- **Verify:** tsc 0 · smoke @1024² saline=4/25, mouths=90, 0 NaN, 22/22, đất 38.0% · @2048² 3.9s, 28 hồ, 450 thác, 0 NaN, 22/22 · cargo lib **36/36** (fixture world thật tái sinh v20) · src Vitest **21/21** · test:frontend **237/237** · lint 0 · build ✅. *(Render tái dùng đường Beach/Lake sẵn có — chưa screenshot.)*
- **🎉 M1 (vá vật lý thủy văn) HOÀN TẤT**: hồ thoát nước (v19) + delta cửa sông + endorheic (v20). Còn: M2 xác nhận runtime trực quan (cần app), M3 chunk/LOD, M4 hang thật.
- **Tinh chỉnh:** ngưỡng nội lưu → `ENDORHEIC_MOISTURE` (0.24); kích thước/độ nông delta → `DELTA_DEPOSIT`/`DELTA_MAX_DEPTH`/`DELTA_MIN_RIVER` (Pass 4d).

---

# 🌊 v19 — Sông THOÁT HỒ (spillway) + tài liệu WORLD_DESIGN + phát hiện map tách đôi (2026-07-24)

Người dùng đặt lại mục tiêu "map chân thực, đúng vật lý, đủ môi trường, cache, tối ưu cho triệu agent". Khảo sát 2 tầng phát hiện **map bị TÁCH ĐÔI**: world 3D đẹp (`worldGen.ts` 2048²/22 biome, có cache IndexedDB) chỉ là **trang trí**; agent thật sống ở `terrain.rs` **128²/11 biome, KHÔNG cache, sinh lại mỗi lần chạy**. Đã lập **[`WORLD_DESIGN.md`](WORLD_DESIGN.md)** (khảo sát hiện trạng + tham chiếu kỹ thuật + kiến trúc "một world quyền lực, sinh 1 lần, cache, dùng chung" + roadmap M0–M5). **Bump `WORLD_GEN_VERSION` 18→19.**

- **Vá vật lý #1 — mọi hồ đều THOÁT NƯỚC** (`computeLakes` trả thêm `outlets` + Pass 4b-2): Priority-Flood đã biết mực tràn + mặt `filled` không-lõm. Với mỗi bồn giữ lại, **BFS trên plateau ngập** (cell `filled ≈ level` nối với hồ) tới ngưỡng tràn thật (`filled < level`) → route steepest-descent trên `(filled, elev)` xuống biển/hồ thấp hơn → stamp `riverAmt=170` + `Biome.River` (guard giống River overlay: không đè băng/biển/hồ). Trước đây hồ là **vũng kín** (bản legacy `terrainGenerator.ts` từng route spillway, bản SoA làm mất) — nay **bảo toàn nước**: hồ→suối→biển/cascade.
- **Bài học**: hàng xóm trực tiếp của cell-hồ luôn ở `filled = level` (vành nông), không bao giờ `< level` → pour-detection ngây thơ cho `outlets=0`; phải **BFS qua plateau** mới chạm ngưỡng tràn.
- **Verify (frontend v19):** `tsc --noEmit` 0 lỗi · smoke @1024² **outlets=386/newRiver=215** (River 8901→9116), 0 NaN, 22/22, đất 38.0% · smoke @2048² 4.3s, 28 hồ, 450 thác, 0 NaN, 22/22 · src Vitest **17/17** · test:frontend **237/237** · lint **0 lỗi** (441 warning legacy) · `npm run build` ✅ (2 entry). *(Chưa chụp screenshot headless — thay đổi thuần data, tái dùng đường render river/`riverAmt` sẵn có.)*
- **M0 ĐÃ CHỐT: A→B** (frontend giàu làm nguồn trước, port Rust sau, giữ định dạng đọc).
- **M2 (một phần) — BACKEND sinh-1-lần + cache đĩa + reload** (nơi agent THẬT sống, trước đây sinh lại mỗi lần): `TerrainMap` thêm `Serialize/Deserialize`; **`TerrainMap::load_or_generate(settings, cache_dir)`** — băm key từ settings+`MapConfig`+`WORLD_CACHE_VERSION` → cache hit đọc thẳng bincode từ đĩa (bỏ qua sinh), miss thì sinh + ghi best-effort; `init_world` dùng nó (override `ANIMA_CACHE_DIR`, mặc định `temp/anima_world_cache`). Sinh vốn đã deterministic (RNG seed) nên cache là tối ưu tốc độ chồng lên bảo đảm đó. **Verify backend:** `cargo test --lib terrain::tests` **9/9** (3 mới: round-trip bincode exact, `generate_is_deterministic`, cache write→reload identical) · `cargo clippy --lib` **0 warning**.
- **M2 (cốt lõi) — WORLD ARTIFACT dùng chung + chứng minh liên-ngôn-ngữ** (ghép hai world làm một, verify KHÔNG cần chạy app): định dạng nhị phân LE trung tính (magic `ANMW`, `elevation/moisture/temperature/flow` f32 + `biome` u8 — **không dùng bincode Rust-only**). Cài 2 phía: Rust `core/world_artifact.rs` (`from_bytes/to_bytes` + `to_terrain_map` downsample + **map biome 22→11**) và TS `utils/worldArtifact.ts` (`encode/decodeWorldArtifact`). **Chứng minh:** fixture do **encoder TS** ghi (`scripts/gen_artifact_fixture.ts` → `src-tauri/tests/fixtures/world_4x4.anmw`) được **cargo-test đọc lại, assert byte-cho-byte khớp Rust** (`decodes_frontend_generated_fixture`). `init_world` nay đọc `ANIMA_WORLD_ARTIFACT` → agent sống trên **chính world frontend sinh ra** (fallback cache nếu vắng). **Verify:** cargo `world_artifact` **5/5** · lib **34/34** · clippy 0 · tsc 0 · src Vitest **20/20** (worldArtifact 3) · lint 0 lỗi · build ✅.
- **M2 (luồng runtime) — GHÉP HAI WORLD chạy thật, verify luồng DỮ LIỆU end-to-end (không cần app)**: frontend `worldCache.loadOrGenerateWorld` → `worldToArtifact(world, 256)` (downsample) → `invoke('save_world_artifact', bytes)`; backend command `save_world_artifact` **validate + ghi** ra `default_artifact_path()` (env `ANIMA_WORLD_ARTIFACT`, mặc định temp); `init_world` đọc path đó → `to_terrain_map` → **agent sống trên world frontend sinh ra** (fallback cache nếu chưa có). **Bằng chứng:** cargo test `real_frontend_world_becomes_valid_terrain_map` — world **THẬT 128²** từ `worldGen.ts` (fixture `world_real_128.anmw` do frontend encode) → TerrainMap backend hợp lệ **có cả biển lẫn đất**; `write_to_path_validates_then_reloads`. **Verify:** cargo lib **36/36** · clippy 0 · tsc 0 · src Vitest **21/21** (worldArtifact 4) · test:frontend **237/237** · lint 0 · build ✅.
- **Còn lại (roadmap WORLD_DESIGN.md)**: M1 (delta cửa sông + bồn nội lưu); **M2 chỉ còn XÁC NHẬN TRỰC QUAN runtime (dòng `invoke` chạy thật trong Tauri + sim đọc đúng) + 3D render TỪ world chung + terrain vào save-state — CẦN CHẠY APP (không làm trên Vostro yếu)**; M3 chunk/LOD/sim-LOD; M4 hang thật (voxel). *Thuật toán + luồng dữ liệu đã xong & verify.*
- **Tinh chỉnh:** độ đậm suối thoát → `riverAmt=170` (Pass 4b-2); ngưỡng plateau → eps `1e-6`; vị trí cache backend → `ANIMA_CACHE_DIR`; invalidate cache → `WORLD_CACHE_VERSION` (terrain.rs).

---

# 🧬 E11 — Metric đồng tiến hoá (Red Queen): phân kỳ niche + độ phủ archive (2026-07-03)

Tiếp Phase 7 (E11). Đo bằng chứng đua vũ trang predator-prey + open-endedness.

- **`niche_divergence(prey_mass, predator_mass)`** (ecology.rs, unit-test): khoảng cách chuẩn hoá giữa khối lượng cơ thể TB của con mồi vs săn mồi — **character displacement**: giá trị tăng = hai guild tách nhau về hình thái (đua vũ trang Red Queen) thay vì cạnh tranh trực diện.
- **Độ phủ archive MAP-Elites** = số ô niche đã chiếm (`archive.grid.len()`) — **proxy open-ended evolution** (tài liệu: coverage tăng liên tục = tiến hoá mở).
- **Backend**: publish block đọc thêm mean body mass mỗi guild (query `AgentGenotype` + `Prey`/`Predator`, `total_mass()`) + archive coverage từ `BevyMapElitesArchive` resource → mở rộng DTO `EcosystemState` (prey_mass, predator_mass, niche_divergence, archive_coverage).
- **Frontend**: panel thêm 2 readout (khối lượng guild; phân kỳ/độ phủ) + **sparkline thứ 3 phân kỳ niche theo thời gian** (1 series → title tự đặt tên, không legend — đúng luật dataviz).
- **Verify:** `cargo build`/`clippy` 0 cảnh báo · lib **26/26** (ecology) · tauri_ipc 6/6 · `npm run build` ✅ · src Vitest **11/11** (EcosystemPanel 4) · test:frontend 237/237 · lint 0 lỗi.
- **🎉 Phase 7 (Ecosystem Dynamics) E1–E11 HOÀN TẤT.** Còn lại E12 tuỳ chọn: connectance/food-chain length + thí nghiệm intermediate-disturbance (harness nghiên cứu, nên chạy trên máy đích).
- **Tinh chỉnh:** `MASS_REFERENCE` (thang phân kỳ) trong ecology.rs.

---

# 📈 E10 — Biểu đồ thời gian: chu kỳ quần thể + dòng năng lượng (2026-07-03)

Tiếp Phase 7 (E10). Thêm time-series vào `EcosystemPanel` — làm E1–E9 "sống dậy" bằng mắt.

- **Sparkline SVG inline** (không thêm thư viện — CSP/deps sạch): panel giữ lịch sử cuộn ~60 mẫu (1 phút @1Hz), vẽ **2 biểu đồ riêng, MỘT trục mỗi cái** (không dual-axis): (1) **con mồi vs săn mồi** theo thời gian — thấy rõ chu kỳ Lotka-Volterra (săn mồi trễ pha), (2) **3 ngăn sinh khối** (thực vật/động vật/mùn) theo thời gian — dòng năng lượng + dao động mùa.
- **Theo skill dataviz** (đã nạp trước khi viết chart): màu theo THỰC THỂ cố định; **validate bằng `validate_palette.js`** (không đoán) — biomass green/orange/purple ΔE 29.1, population blue/red ΔE 69.7, **tất cả PASS** CVD+contrast trên nền trắng; đường 2px, baseline mờ, end-dot đánh dấu giá trị mới nhất, **legend luôn có** + nhãn (định danh không chỉ bằng màu), chữ dùng ink token (không mặc màu series); gap 2px giữa các mảng thanh xếp chồng.
- **Nhìn tận mắt** (bước 7 của skill): render preview HTML với dữ liệu dao động thật + screenshot — chu kỳ predator-prey đọc rõ, không đè nhãn/tràn.
- **Test**: `EcosystemPanel.test.tsx` +1 (3 test) — assert 2 sparkline xuất hiện sau khi tích lũy ≥2 mẫu.
- **Verify:** `npm run build` ✅ · src Vitest **10/10** · test:frontend 237/237 · lint 0 lỗi. Backend không đổi.
- **Còn lại (E11)**: metric Red-Queen/character-displacement; connectance & food-chain length; thí nghiệm intermediate-disturbance.
- **Tinh chỉnh:** `HISTORY` (số mẫu); màu `PREY_COLOR`/`PREDATOR_COLOR` + `COMPARTMENTS` (đã validate — đổi thì chạy lại validator).

---

# 📊 E9 — Dashboard hệ sinh thái SỐNG (IPC + panel frontend) (2026-07-03)

Tiếp Phase 7 (E9). Đưa hệ sinh thái ra màn hình — chạm IPC + frontend lần đầu, theo pattern polling sẵn có (như `get_terrain_map`), KHÔNG đổi event contract.

- **Backend**: DTO `EcosystemState{detritus, plants, animals, total, prey_count, predator_count, shannon, simpson}` + command `get_ecosystem_state` (đọc shared `Arc<RwLock<>>` như các command khác). Sim **publish mỗi tick** sau schedule: đọc `EcosystemBiomass` resource + đếm prey/predator (`query_filtered`) + Shannon/Simpson trên [prey, predator]. Thêm field `ecosystem_state` vào `SimulationEngine` (init + clone trước thread). Đăng ký command trong `generate_handler!`.
- **Frontend**: `src/components/EcosystemPanel.tsx` — **tự poll** `get_ecosystem_state` mỗi 1s (guard null, nuốt lỗi khi sim chưa chạy), render **thanh sinh khối xếp chồng** (thực vật/động vật/mùn theo tỷ lệ tổng) + split con mồi/săn mồi + Shannon/Simpson. Match theme card trắng của simulation-view. Mount 1 dòng trong App.tsx sau card Canvas.
- **Test**: `src/__tests__/EcosystemPanel.test.tsx` (2 test, mock `get_ecosystem_state` trong setup-vitest) — render, poll, assert 3 compartment + population + diversity. Bài học: matcher `toHaveTextContent` phải `import '@testing-library/jest-dom'` trong TỪNG test (không global).
- **Verify:** `cargo build`/`clippy` 0 cảnh báo · lib 25/25 · tauri_ipc_tests 6/6 · `npm run build` ✅ · src Vitest **9/9** (thêm EcosystemPanel 2) · test:frontend **237/237** · lint 0 lỗi.
- **Còn lại (E10)**: metric Red-Queen/character-displacement; connectance & độ dài chuỗi thức ăn; time-series history trong panel; nghiên cứu intermediate-disturbance.

---

# ❄️ BACKEND E8 — Xác phân huỷ (corpse→detritus) + Mùa vụ (seasonal fertility) (2026-07-03)

Tiếp Phase 7 (E8). Hoàn tất nửa "chết" của vòng khép kín + động lực mùa vụ.

- **Corpse → detritus** (nửa chết của vòng): trong `apply_staggered_evolution_system`, khi 1 agent bị thay thế/despawn, **năng lượng dự trữ còn lại của nó trả về pool detritus** thay vì biến mất. Bảo toàn: census ngừng đếm reserve của agent đó ở tick sau, đúng lượng năng lượng ấy giờ nằm ở detritus. Vòng plants→animals→detritus→plants giờ đóng cả 2 chiều (chuyển hoá + săn + **chết**).
- **Mùa vụ (`SeasonClock` + `seasonal_fertility`)**: fertility = `1 + 0.5·sin(phase)` (hè tăng, đông giảm, clamp≥0), 1 vòng ~100s sim. `resource_field_regrowth_system` nhân regrowth theo fertility → sinh khối thực vật **bùng-tàn theo mùa** = nhiễu động chu kỳ giúp duy trì chu kỳ predator-prey (đúng khuyến nghị tài liệu). Insert `SeasonClock::default()` trong `init_world`.
- **Verify:** `cargo build` ✅ · lib **25/25** (ecology 25) ✅ · environmental/evolution_robustness/map_elites/combat/persistence đều pass · `cargo clippy` 0 cảnh báo.
- **Còn lại (E9)**: expose EcosystemBiomass/Shannon/Simpson/connectance qua IPC (dashboard sống); metric Red-Queen & character displacement tường minh; nghiên cứu intermediate-disturbance.
- **Tinh chỉnh:** `SEASON_AMPLITUDE` + `rate` (chu kỳ mùa) trong ecology.rs; corpse flux = reserve energy (đơn giản, bảo toàn — nếu muốn tính cả sinh khối cơ thể thì cần kế toán chi phí sinh sản).

---

# 🧭 BACKEND E7 — MAP-Elites descriptor SINH THÁI + tăng trưởng cây theo NPP (2026-07-03)

Tiếp Phase 7 (E7). Đưa **niche sinh thái** vào tầng tiến hoá quality-diversity.

- **Descriptor MAP-Elites đổi từ vận động → sinh thái**: trước là `[speed, efficiency]`; giờ là **`[body_mass, foraging_range]`** chuẩn hoá [0,1] qua `ecological_descriptors()`. `body_mass` = `MorphologyGenotype::total_mass()` (tổng mass các node — **trait chủ đạo của MTE**, quyết định chuyển hoá/tuổi thọ/sinh sản); `foraging_range` = quãng đường đi trong epoch (độ rộng niche). → archive giờ **soi sáng đa dạng SINH THÁI** (con nhỏ đi xa vs con to ở yên, generalist vs specialist) thay vì một tối ưu vận động duy nhất; đua vũ trang predator/prey (Red Queen) trải rộng lưới thay vì hội tụ 1 điểm. Grid contract IPC giữ nguyên (vẫn Vec<f64> 2 trục).
- **Tăng trưởng cây theo NPP**: `fruit_growth_system` scale tốc độ ra quả theo NPP biome tại vị trí cây (`0.3 + 0.7·r_max/rainforest_cap`) — rừng mưa ra quả nhanh, sa mạc/đá chậm. Dùng `Option<&Position>` + `Option<Res<ResourceField>>` → cây không có Position / world không field vẫn ra quả base rate (giữ test cũ `current_fruit == 12.0`).
- **Verify:** `cargo build` ✅ · lib **24/24** (ecology 22) ✅ · map_elites 6/6, environmental 6/6, combat, zero-alloc, evolution_robustness đều pass · `cargo clippy` 0 cảnh báo.
- **Còn lại (E8)**: corpse→detritus (nửa "chết" của vòng khép kín); metric Red-Queen / character displacement tường minh; intermediate-disturbance; seasonal fertility.
- **Tinh chỉnh:** `MASS_REFERENCE`/`FORAGING_REFERENCE` trong ecology.rs (thang chuẩn hoá niche); hệ số `0.3/0.7` NPP trong fruit_growth_system.

---

# 🌾 BACKEND E6 — Grazing + vòng năng lượng KHÉP KÍN (2026-07-03)

Tiếp Phase 7 (E6). Mắt xích còn thiếu: **thực vật → thú ăn cỏ → detritus → mọc lại**, khép kín và bảo toàn.

- **`herbivore_grazing_system`** (mới): con mồi (herbivore) **gặm `ResourceField`** tại vị trí → tăng energy. Intake bão hoà (Type II) + trần bite/tick; ô bị gặm cạn cho ít → thú tự **tản đi** tìm ô chưa gặm (giving-up density = refuge không gian, đúng Huffaker).
- **Regrowth GATED theo detritus** (`step_regrowth_gated`): thực vật CHỈ mọc bằng cách rút năng lượng tự do từ pool detritus (luật Bibites) → không thể mọc từ hư không, chống bùng nổ; phần tiêu thụ trừ khỏi detritus.
- **Metabolism → detritus**: năng lượng hô hấp mất đi giờ quay về pool (không biến mất) → bảo toàn.
- **`ecosystem_census_system`**: tổng energy thú sống → `EcosystemBiomass.animals` mỗi tick (cho dashboard + kiểm tra bảo toàn).
- **Vòng bảo toàn**: plants →(graze)→ animals →(metabolism/predation)→ detritus →(gated regrowth)→ plants. Unit test `full_trophic_cycle_conserves_energy` mô phỏng 50 bước, tổng năng lượng **bất biến** (sai số <1e-6).
- **Verify:** `cargo build` ✅ · ecology lib **20/20** ✅ · combat 6/6, zero-alloc 5/5, environmental, networking (chạy riêng) đều pass · `cargo clippy` lib 0 cảnh báo. Lưu ý Bevy: 1 tuple `add_systems` tối đa 20 phần tử → tách 3 system ecology ra lời gọi riêng (`.after()` vẫn resolve chéo). Exit 101 khi chạy `cargo test` toàn bộ là **flake tranh chấp port** (mỗi suite pass khi chạy riêng), không do thay đổi này.
- **Còn lại (E7)**: NPP-couple tăng trưởng cây; corpse→detritus (xác phân huỷ); MAP-Elites descriptor sinh thái + Red-Queen.
- **Tinh chỉnh:** `max_bite` (trần gặm) trong `herbivore_grazing_system`; `herbivore_intake`/`step_regrowth_gated` trong ecology.rs.

---

# 🧬 BACKEND — Nền tảng Ecosystem Dynamics (MTE + Holling + Closed Energy + NPP) (2026-07-03)

Theo tài liệu "Ecology & Environment Design for the Anima Engine". Chuyển trọng tâm sang **backend Rust/Bevy** (map frontend đã ổn ở v21). Module mới `src-tauri/src/core/ecology.rs` — thuần hàm, **zero-alloc hot path**, 17 unit test. Chi tiết milestone Phase 7 (E1–E7) ở PROJECT.md.

- **MTE (Metabolic Theory of Ecology)**: `metabolic_rate = i0·M^0.75·e^(−E/kT)` (Kleiber + Arrhenius chuẩn hoá về 1.0 tại 20°C). Thay số hạng khối lượng TUYẾN TÍNH cũ trong `metabolic_decay_system`: con lớn tốn ít năng lượng/gram hơn (mass-specific ∝ M^−¼), ấm → chuyển hoá nhanh (Q10≈2.4). E=0.65 eV (động vật)/0.30 (thực vật). Tách maintenance (MTE) vs activity (locomotion tuyến tính).
- **Holling Type III + Lindeman + closed loop** trong `combat_system`: `predation_capture()` — con mồi khoẻ KHÔNG bị hút cạn 1 đòn, con mồi hiếm/yếu gần như không bị đụng (rarity refuge chống tuyệt chủng). Predator hấp thụ ~30% (Lindeman), phần dư → `EcosystemBiomass.detritus` (bảo toàn năng lượng). **ĐỔI test** `test_predator_prey_collision_and_combat` (cũ assert 100% transfer = Type I mà tài liệu bảo SAI) → property-based.
- **NPP resource field** (`ResourceField`): logistic `R+g·R(1−R/R_max)`, `R_max` theo NPP biome Whittaker (rainforest 2200→desert 90→rock 50). SoA, `step_regrowth` in-place (zero-alloc), `graze()`, map world↔cell. Sinh từ `TerrainMap.biomes` trong `init_world`; system sống `resource_field_regrowth_system` trong tick schedule. Ledger `EcosystemBiomass{detritus,plants,animals}` bảo toàn tổng năng lượng.
- **Chỉ số đa dạng**: `shannon_index`/`simpson_index` (thuần hàm).
- **Verify:** `cargo build` ✅ · `cargo test` ✅ (ecology 17/17; combat/adversarial/zero-alloc pass) · `cargo clippy` lib 0 cảnh báo · frontend build ✅. Test networking (bind port thật) đôi khi flake — chạy lại xanh, KHÔNG do thay đổi này.
- **Tinh chỉnh:** hằng trong `ecology.rs` (`METABOLIC_NORM`/`E_ANIMAL_EV`/`LINDEMAN_EFFICIENCY`/`CAPTURE_ATTACK`/`NPP_TO_CAPACITY` — giữ carrying capacity vừa phải tránh paradox of enrichment); `growth_rate` trong `init_world`.

---

# 🎮 v21 — Theo báo cáo AI/Controller: habitat check, shoal tản đàn, slope limit, collider cây (2026-07-03)

User gửi báo cáo lỗi hệ sinh vật + walking sim. KHÔNG bump version (render/logic-side). Đã xử lý:

- **"Bầy sinh vật khổng lồ trên mặt nước, đội hình vòng tròn"** = đàn cá: (1) site đàn biển giờ **giãn cách ≥40 unit** nhau; (2) cá kẹp sâu **≥1.5 unit dưới mặt** (hồ ≥1.2), không nổi lên mặt; (3) màu **lerp về xanh sẫm chìm** 30–35% + opacity 0.92→0.6 + nhỏ hơn → đọc là bóng cá dưới nước, không phải sprite đứng trên mặt; (4) **shoal thay carousel**: mỗi con tự thở bán kính (`sin(t·0.6)` ±16%) + lệch tốc độ góc ±15% → đàn tơi, đổi chỗ liên tục, không lồng nhau.
- **Hươu lội hồ (bug thật)**: offset bầy ±4.5u từ anchor KHÔNG kiểm tra ô đáp → rơi vào hồ/sông. Fix: mỗi cá thể tự validate ô của mình (elev>sea, water=0, riverAmt=0, slope≤0.3) — chuẩn habitat như flora.
- **Đi bộ "xuyên vỏ trái đất"**: thủ phạm là **camera near=2** — nhìn lên dốc, mặt đất trong 2 unit bị near-clip. Fix: near 2→**0.6** (ratio far/near 22k, depth 24-bit vẫn ổn + fog phủ xa).
- **Max slope ~43°** khi đi bộ: bước di chuyển kiểm tra `Δground > step·0.95` → thử **trượt theo từng trục** (slide) rồi mới chặn — không leo được vách đứng nữa.
- **Collider thân cây**: grid không gian 8-unit chứa mọi flora có thân (7 loại, build 1 lần/world) → mỗi bước walk query 9 ô, đẩy capsule ra khỏi bán kính thân `0.45+scale·0.25`. Đi xuyên gốc cây hết.
- Ghi chú trung thực: sinh vật là ambient (đứng/lượn tại lãnh thổ), CHƯA có FSM wander/flee di chuyển tự do — nếu cần agent thật thì đó là hệ ECS backend (Bevy) của dự án, không phải lớp trang trí map.
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · screenshot hồ: cá = bóng mờ dưới nước, vịt nổi rời rạc, dê trên đá — 0 lỗi console.

---

# 🦌 v20 — ĐỘNG VẬT: cá hồ, vịt, diệc, bướm, hươu, dê núi (2026-07-03)

Yêu cầu: "thêm sinh vật trên cạn và dưới nước, đặc biệt ở sông suối ao hồ". **KHÔNG bump WORLD_GEN_VERSION** (render-side — cache giữ nguyên).

- **Cá nước ngọt** (`WorldFish` mở rộng): mỗi bồn hồ ≥90 cell² chưa đóng băng nhận 1 đàn (tối đa 10 đàn hồ) — bơi vòng giữa tầng nước dưới mặt hồ, màu nước ngọt (bạc/nâu vàng/xám xanh). Đã thấy đàn cá lượn giữa hồ núi lớn trong screenshot.
- **`WorldWildlife.tsx` mới** (5 instanced mesh, ~170 con): 
  - **Vịt** (≤48): 2–4 con/bồn hồ ấm, nổi tại mực tràn +0.05, trôi vòng chậm r 1.2–2.8 + bob; đầu xanh két/thân nâu/mỏ vàng (vertex paint).
  - **Diệc** (≤32): đứng bất động ở bờ hồ (`shore>0.85`) hoặc bờ sông (riverAmt láng giềng >150), đất thoải ấm; chân que + cổ nghiêng + mỏ vàng.
  - **Bướm** (≤52): rập rờn quanh bờ nước + đồng hoa ấm (lissajous + flap scale-Y 11Hz), 4 màu (trắng/vàng cam/hồng/xanh), meshBasic 2 tam giác.
  - **Hươu** (≤46): đàn 2–4 con trên Grassland/Shrubland/Savanna/Forest thoải (slope<0.25), geometry thân+4 chân+cổ+đầu, bob gặm cỏ nhẹ, castShadow.
  - **Dê núi** (≤16): cùng geometry tint trắng, trên Alpine/Rock slope 0–0.6.
- Placement deterministic (hash probe trên field world) — mỗi lần vào map thú vẫn ở đúng "lãnh thổ" cũ.
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · screenshot: đàn cá giữa hồ ✓, không lỗi console. (Mật độ là cap — chỉnh trong WorldWildlife nếu muốn nhiều/ít hơn.)

---

# 🐠 v19 — HỆ SINH THÁI DƯỚI NƯỚC: san hô, kelp, cỏ biển, đàn cá (2026-07-03)

Yêu cầu: "làm cả hệ sinh thái dưới nước". **Bump `WORLD_GEN_VERSION` 17→18** (flora thêm loài thủy sinh).

- **3 loài thủy sinh mới** (FloraType 11–13, Pass 5b trong `worldGen`): **Coral** (thềm nhiệt đới t>0.6, độ sâu 0.018–0.055, dens 0.24 — mảng rạn patchy), **Kelp** (ôn đới t 0.28–0.6, cao 2 unit), **Seagrass** (dải nông nhất ≤0.018). Cap 22k; @2048²: Coral 10.3k · Kelp 8.3k · Seagrass 3.4k. Mọc trên đáy biển (mesh terrain dưới nước), nhìn xuyên qua nước nông trong suốt; KHÔNG castShadow.
- **Màu san hô**: geometry base nhạt trung tính + instanceColor 5 tông (hồng/cam/tím/đỏ/kem) theo hash — như cơ chế hoa dại.
- **Đàn cá** (`WorldFish.tsx` mới): 12 đàn × 20 con = 1 InstancedMesh (1 draw call); site dò deterministic trên thềm nắng (depth 0.02–0.07); bơi vòng giữa tầng nước (floor↔mặt), mỗi con lệch pha + bob dọc + vẫy đuôi (scale-x sin); mỗi đàn 1 màu (bạc/vàng/xanh/cam/ngọc/hồng); instanceColor set 1 lần (t<0.5s).
- **Đáy nông = cát sáng** (bake): Ocean cell depth<0.06 blend về màu cát (ấm hơn ở vùng nóng) → nước ven bờ PHÁT màu ngọc lam, san hô nổi bật trên nền cát.
- **Nước nông trong hơn**: alpha shallow 0.4→0.32.
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke @2048²: 4.0s, flora tổng **117.7k**, 22/22 biome, 0 NaN · screenshot reef: rạn màu qua nước ngọc lam + đàn cá silhouette — 0 lỗi console.
- **Tinh chỉnh:** dải san hô → gates depth/temp trong Pass 5b; mật độ → dens 0.24/0.11/0.16; số đàn cá → props `schools/fishPerSchool`; độ trong nước → alpha `0.32`; màu san hô → bảng tint trong WorldVegetation.

---

# 🌊 v18 — Sông chảy shimmer + sóng vỗ bờ + bóng mây trôi + núi răng cưa (2026-07-03)

**Bump `WORLD_GEN_VERSION` 16→17** (ridge weight đổi → relief mới).

- **Sông CHẢY thật** (`WorldTerrain` onBeforeCompile mở rộng): thêm `uRiverMask` (texture R8 từ `riverAmt`) + `uTime` (shaderRef cập nhật trong useFrame) — 2 lớp detail-noise trượt ngược chiều nhân vào diffuse trên mặt sông (`riv*0.3`) → nước lấp lánh trôi, kết hợp roughness glint.
- **Sóng vỗ bờ thở nhịp** (`WorldWater` fragment): độ với của dải foam dao động `1.5 + surf*0.55` với `surf` = 2 sóng sin chạy dọc bờ (pha theo x+z và x−z) → mép biển phồng-xẹp lan từng đoạn như sóng cuộn; hồ dùng chung shader nên cũng có sóng nhẹ.
- **Bóng mây trôi**: lõi mỗi cụm mây bật `castShadow` — bóng elip mềm quét qua đồng cỏ khi mây trôi (mây trong shadow camera ±0.7·worldScale). Nếu bóng quá gắt trên máy thật: tắt bằng cách bỏ `castShadow` trong WorldSky.
- **Núi răng cưa hơn**: ridge weight `0.5→0.58` — sống núi arête sắc, đỉnh serrated (kéo theo +2 hồ băng do đỉnh cao lạnh hơn — logic tự nhất quán).
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke @2048²: 3.97s, 22/22 biome, flora 95.7k, 450 thác, 50 hang, 28 hồ (5 đóng băng), 0 NaN · screenshot 0 lỗi console; snowline preview lởm chởm tự nhiên quanh khiên băng.
- **Tinh chỉnh:** cường độ shimmer → `riv*0.3` + tốc độ scroll trong patch; biên độ sóng bờ → `0.55`; độ sắc núi → ridge `0.58`.

---

# 🏔 v17 — Theo báo cáo phân tích tự nhiên: snowline mềm, rừng ven sông, sông thon-nở (2026-07-03)

User gửi báo cáo phân tích 4 nhóm lỗi so với tự nhiên (thủy văn/bờ nước/biome/địa mạo). **Bump `WORLD_GEN_VERSION` 16** (code: 15→16). Đã xử lý:

- **Đường song song trên núi tuyết** (thủy văn #1): thủ phạm là ribbon vẫn **khoét lòng dưới nắp tuyết** → rãnh song song lộ qua normal map/AO. Fix: gate `tApprox < 0.2` (ước lượng nhiệt lapse ngay tại Pass 2b) — vùng đóng băng không có sông lỏng, không khoét; siết gate sườn dốc (`grad>0.004 & f<0.72`).
- **Sông thon–nở theo lưu lượng**: `rad = 0.4 + s²·widthScale·2.6`, `amt0 = 90+165s` — đầu nguồn chỉ mảnh, phình rõ sau mỗi hợp lưu (dendritic đúng nghĩa; mạng hợp lưu vốn là D8 nên cấu trúc rễ cây đã có, giờ nhìn thấy được).
- **Snowline mềm** (biome #3): ngưỡng Glacier/Snow/Rock/Alpine trong `classify` nhận `capJit = (moist−0.45)·0.06` (sườn ẩm tuyết XUỐNG thấp — đúng khí tượng) và **tuyết trượt khỏi mặt dốc**: `T_SNOW − slope·0.05`, đá lộ thêm `T_ROCK + slope·0.04` → ranh giới nhấp nhô hòa quyện, vách dốc lộ sọc đá.
- **Thực vật theo nước + treeline** (biome #3): `waterBoost = 1 + shore·1.1 + min(1,flow·1.6)·0.9` nhân vào density → **rừng hành lang ven sông/hồ, ốc đảo dọc wadi sa mạc** (flora 73k→**97k**); cây cao trên `slope>0.55` tự hạ thành bụi (đất không giữ được rễ).
- **Ecotone blend** (biome #3): bake trộn màu 4 láng giềng ±2 cell khi khác biome (56/44) — nâu↔xanh chuyển tiếp mềm; nước/băng giữ mép sắc.
- **Xói mòn** (địa mạo #4): droplets n·0.02→**n·0.03** (cap 120k) — khe rãnh thủy lực cắt sườn núi thật hơn.
- **Chấm đen** (địa mạo #4) = cửa hang quá dày/to: prob 0.018, slope≥0.45, **cấm vùng tuyết** (tApprox<0.135), scale 1.5×1.1, ép sát vách hơn → 40 hang chất lượng thay vì 100 chấm.
- **Thác hết ghim xuyên**: màn đẩy ra 1.0 unit khỏi vách, vũng bọt nhỏ (0.55/0.4) + nâng 0.3 + opacity 0.38.
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke: flora 97k, 450 thác, 40 hang, 24 hồ, 22/22 biome, 3.9s, 0 NaN · screenshot: rừng bám dọc sông rõ, sông thon-nở, hết mảng nước/sọc song song. 0 lỗi console.
- **Tinh chỉnh:** độ nhấp nhô snowline → `capJit`/hệ số slope; mật độ ven sông → `waterBoost`; blend ecotone → trọng số `0.56/0.11`; xói mòn → `n*0.03`.

---

# 💧 v16 — FIX nước xuyên sườn dốc + rừng dày hơn + 450 thác (2026-07-03)

Bug user báo: mesh nước "lồi ra thành mảng tam giác/chữ nhật sắc cạnh" cắt xuyên terrain ở sườn dốc. **Bump `WORLD_GEN_VERSION` 15→16... (thực tế 14→15** trong code — flora/thác đổi).

- **Nguyên nhân clipping**: hồ/ao render bằng quad phủ **bbox** bồn ở cao độ mặt tràn; địa hình **dưới mực nước nhưng NGOÀI lòng bồn** (sườn dốc xuôi) vẫn có depth>0 → depth-fade không giấu được → mảng nước lơ lửng bị terrain cắt thành hình sắc cạnh. Ao-trên-sườn từ v13 (carve pits) nhân lỗi lên.
- **Fix**: **`uLakeMask`** — texture R8 full-res từ trường `water[]` (1 = cell ngập thật), bilinear cho mép mềm; shader hồ nhân `alpha *= smoothstep(0.2, 0.55, mask)` (chỉ khi `uMaskOn=1`; ocean giữ nguyên). Nước bám đúng lòng bồn tuyệt đối, không phụ thuộc subdivision.
- **Rừng/thực vật dày hơn**: `maxFlora` 90k→130k, density tăng toàn bộ biome có cây (Forest 0.4→0.5, Jungle 0.55→0.62, Taiga→0.52, Grassland→0.22, Mangrove→0.4, Bog→0.3, Steppe→0.12…) → **73.4k instance** @2048² (+20%).
- **Nhiều thác hơn**: `MIN_DROP` 0.012→0.009 (base 1024), `MAX_FALLS` 320→**450** (đạt cap), khoảng cách tối thiểu 4→3 cell.
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke: 450 thác, flora 73.4k, hồ 27, 0 NaN · screenshot aerial vùng sông-ao trên sườn: **hết mảng nước sắc cạnh**, thấy hoa dại + chim + thác trong khe. 0 lỗi console.
- **Tinh chỉnh:** mép mask → cặp `0.2/0.55` trong shader; mật độ rừng → `floraDensity`; số thác → `MIN_DROP`/`MAX_FALLS`.

---

# 🐦 v15 — Vân đất cận cảnh + thác chảy động + quầng trăng + chim (2026-07-03)

Yêu cầu: "đẹp và chân thực hơn nữa". **KHÔNG bump WORLD_GEN_VERSION** (chỉ render-side — cache người dùng còn nguyên).

- **Micro-detail texture** (`WorldTerrain.buildDetailTexture` + `onBeforeCompile`): tile value-noise 256² (2 octave, mean-1.0) nhân vào diffuse sau `map_fragment` với repeat ×220, cường độ 0.34 → mặt đất có hạt/vân khi đi bộ (trước bị bilinear kéo mờ trong vài mét); mean-centred nên mipmap tự trung hòa về 1.0 = tự tắt theo khoảng cách, 0 chi phí tune. Lưu ý pattern: prepend `uniform sampler2D uDetail;` vào fragmentShader + replace include; uv dùng `vMapUv` (three ≥ r151).
- **Thác chảy động** (`WorldWaterfalls`): curtain đổi sang ShaderMaterial — sọc sáng 2 tốc độ trượt xuống theo `uTime`, vỡ theo cột (hash), mép trái/phải fade mềm, chân thác trắng bọt hơn. **Bài học InstancedMesh + ShaderMaterial**: three TỰ define `USE_INSTANCING` + khai báo `instanceMatrix` — chỉ cần NHÂN nó trong transform, khai báo tay sẽ lỗi "redefinition".
- **Quầng trăng** (`WorldSky` shader): uniforms `uMoonDir/uMoonGlow` — halo lạnh `pow(m,700)·0.55 + pow(m,26)·0.05`, glow = moonDirY khi trăng mọc. Đêm có vệt trăng lấp lánh trên nước (roughness map + spec).
- **Chim** (`WorldBirds.tsx` mới): 18 con, silhouette 2 tam giác chữ V, 1 InstancedMesh + 1 draw call; bay vòng tròn (tham số deterministic per-index), nghiêng cánh theo chiều lượn, vỗ cánh bằng squash scale-Y; frustumCulled=false (matrix update mỗi frame ~20 phần tử — không đáng kể).
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · screenshot: đất có grain, đêm 00:21 vệt trăng trên biển, curtain compile sạch, 0 lỗi console.
- **Tinh chỉnh:** độ đậm vân đất → `0.34` + repeat `220`; tốc độ sọc thác → `1.5/2.6`; độ sáng quầng trăng → hệ số `0.55/0.05`; số chim/độ cao → props `count` + hằng trong `flights`.

---

# 🌅 v14 — Trời gradient + AO địa hình + hồ băng + hoa dại + địa tầng (2026-07-03)

Yêu cầu: "đẹp và chân thực hơn nữa, làm đầy đủ map/môi trường/địa hình". **Bump `WORLD_GEN_VERSION` 13→14** (flora mix đổi).

- **Bầu trời gradient + quầng mặt trời** (`WorldSky`): vòm đổi từ 1 màu phẳng sang ShaderMaterial — zenith sẫm (skyColor×0.62), chân trời nhạt dần về màu sương (scale theo độ sáng trời nên đêm không bị bạc), halo mềm quanh mặt trời (`pow(s,300)·0.9 + pow(s,10)·0.16·sunIntensity`). Bỏ animation xoay vòm (halo cần phương vị ổn định). Mây 8→14 cụm.
- **AO độ cong bake vào color texture** (`WorldTerrain`): 2 vòng ring-sample (R≈3/9 cell @2048, world-space constant) — khe/hẻm/lòng chảo tối dần, đỉnh gờ sáng nhẹ; cap 0.22, dưới biển ×0.5 (giữ thềm turquoise nắng). Đủ mạnh để "đóng ghim" lòng sông/khe núi, không dìm cả bồn địa rộng.
- **Vân địa tầng**: sọc ngang ±5% (Badlands ±7.5%) theo `sin(e·320)` trên mặt Rock/Badlands dốc >0.45 — vách đá có lớp trầm tích thẳng hàng toàn vách.
- **Hồ đóng băng** (`WorldWater` + minimap + bake): bồn có nhiệt độ tâm <0.19 (cực hoặc núi cao nhờ lapse) → tấm băng đục `#d7e9f2` roughness 0.35 (mesh gộp riêng, +0.12 nổi trên mặt), lòng hồ bake màu băng nhạt (hết viền navy lộ mép), minimap tint trắng xanh. 3/27 bồn đóng băng ở seed hiện tại.
- **Hoa dại** (`WorldVegetation`): Tuft đổi base sang trung tính nhạt; instanceColor quyết định — 87% cỏ xanh (dải sắc), 5.5% hồng, 4.5% vàng, 3% trắng → đồng cỏ thành thảm hoa rải rác.
- **Đá tảng glacial erratic/scree**: pickFlora thêm Rock ~5-10% cho Taiga/Tundra/Alpine (1.5k tảng @2048²).
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke 22/22, 3.76s · screenshot: trời gradient+halo, thung lũng sông có chiều sâu, hồ băng xám giữa lòng chảo tuyết, 0 lỗi console.
- **Tinh chỉnh:** cường độ AO → hệ số `22/10` + cap `0.22`; ngưỡng đóng băng → `0.19` (WorldWater + Minimap + bake — giữ đồng bộ 3 nơi); tỷ lệ hoa → các mốc `0.055/0.1/0.13`; sọc địa tầng → `sin(e*320)`; màu trời → công thức zenith/horizon trong useFrame của WorldSky.

---

# 🏞 v13 — Sông suối đẹp: ribbon uốn lượn + lòng sông khoét + nước bóng (2026-07-03)

Yêu cầu: "sông suối đang trông rất tệ; nhẹ nhưng vẫn đẹp". Gốc rễ cái xấu: kênh D8 **thẳng tắp theo chéo lưới, 1 cell, song song cách đều** trên sườn dốc → mạng sọc xanh cơ học. **Bump `WORLD_GEN_VERSION` 12→13.**

## Generation (Pass 2b mới trong `worldGen.ts`)
- **Ribbon sông** (`riverAmt: Uint8Array` — field mới): stamp đĩa theo cường độ flow dọc kênh → sông **rộng dần về hạ lưu** (ramp bậc hai: chỉ trunk mới rộng, nhánh mảnh), bờ feather mềm 1 cell. Ngưỡng `CORE_T=0.615` (= ngưỡng biome River mới) nằm **trên dải rãnh-sườn-đồi** của D8.
- **Meander wiggle**: tâm stamp warp bằng noise mượt (`wig=1.6×widthScale`, freq 0.18) → mỗi kênh uốn lượn riêng, phá thế song song; kênh vẫn liền mạch vì cell kề offset gần như nhau.
- **Gate sườn dốc**: mặt rất dốc chỉ giữ ribbon khi flow ≥ 0.68 (sông lớn xuyên hẻm vẫn còn, rãnh fall-line mỏng bị loại — hết hatch).
- **Khoét lòng** −0.0035×amt vào heightmap TRƯỚC slope/biome/hồ → sông nằm trong bed thật, normal map tự đổ bóng bờ; giữ dưới `LAKE_MIN_DEPTH` để không thành hồ giả (lakeBasins 27 ✓).
- Biome River tô lại theo ribbon core (≥140, chừa Ocean/Lake/Glacier/Snow); flora né `riverAmt>100`; validator/worker/isRenderable thêm field.

## Render (`WorldTerrain.tsx`)
- Màu nước bake theo `riverAmt` gradient (tâm sẫm hơn 10%), **chừa Glacier/Snow** (hết sọc xanh vắt qua băng). Bỏ hẳn stream-tint cũ theo flow (0.44–0.6) — thủ phạm mạng nhện.
- **`roughnessMap` mới** (đọc kênh G): sông/hồ/ao 0.33, thềm ngập 0.5, cát ướt 0.7, đất 0.95 → **mặt nước bắt sáng lấp lánh theo mặt trời** (material roughness=1 × map).
- **Nhẹ hơn**: normal map + roughness bake ở **size/2** (2048→1024) → GPU texture 32MB→**24MB** dù thêm map mới; bake nhanh ~4×.
- Camera orbit: teleport nâng `target.y` theo mặt đất + **kẹp camera ≥ mặt đất+2** (hết chui gầm map thấy màu trời).

## Verify
- Smoke @2048²: 3.8s · ribbon 2.34% map (core 0.62%) · 27 hồ/ao · 181 thác · 22/22 biome · 0 NaN.
- Screenshot: aerial sông = mạng phân nhánh ao→suối→sông uốn khúc tự nhiên; walk-mode đứng bờ suối thấy ribbon lượn xuống dốc; **0 lỗi console**. Build ✅ 7/7 & 237/237 ✅ lint 0 lỗi.
- **Tinh chỉnh:** độ rộng sông → hệ số `1.8` trong `rad`; độ uốn → `wig`/freq `0.18`; ngưỡng bắt đầu → `CORE_T`; độ sâu lòng → `0.0035`; độ bóng nước → giá trị `84` trong `buildRoughnessTexture`.

---

# 🎥 v12 — Camera 5 chế độ + Thác/Suối/Ao/Hang + 11 loại thực vật + chế độ Nhẹ (2026-07-03)

Yêu cầu: "cam có các góc nhìn khác, nhiều môi trường hơn (thác, suối, ao, hang động…), nhiều thực vật hơn, nhẹ CPU/GPU". **Bump `WORLD_GEN_VERSION` 11→12.**

## Camera (`WorldCameraRig.tsx` — mới)
- **5 chế độ** (nút HUD): 🌀 Quay (orbit cũ) · 🕊 **Bay** (WASD + E/Q lên/xuống, giữ chuột trái kéo để nhìn, Shift ×3.2) · 🚶 **Đi bộ** (góc nhìn người, WASD, dính mặt đất qua `sampleMeshHeight`, lội ở mép nước, vào mode là đứng NGAY tại điểm target/teleport) · 🗺 Trên cao (khóa xoay, pan+zoom) · 🎬 Cine (tự quay quanh điểm quan tâm).
- Teleport minimap hoạt động ở MỌI chế độ; `viewRef` cập nhật từ rig cho minimap/HUD.

## Môi trường nước & hang (`worldGen.ts` v12)
- **Thác nước**: dò ô sông có drop/cell vượt ngưỡng (chuẩn hóa theo resolution) → SoA `waterfall*` (121 thác @2048²); render `WorldWaterfalls.tsx` = màn nước gradient trắng-xanh + vũng bọt chân thác, **2 draw call** cho toàn bộ.
- **Suối**: nhánh flow 0.44–0.6 tint xanh nước mảnh vào color texture (brook→stream→river liền mạch).
- **Ao**: `MIN_LAKE_CELLS` 16→5, `MAX_LAKES` 520; **toàn bộ hồ/ao gộp 1 mesh** (mergeGeometries — trước là 280 mesh/draw call). Shader nước chuyển sang **world-space swell** để phục vụ mesh gộp.
- **Hang động**: `cave*` SoA — đĩa gần đen unlit áp vào vách `Rock/Badlands` slope>0.35 (100 hang @2048², 1 draw call, `WorldCaves.tsx`); trên núi đọc như hốc đá thật.

## Thực vật 5 → 11 loại (`pickFlora` trộn theo biome)
Pine (thông 2 tầng) · Round (tán kép) · Jungle (tầng tán) · Cactus (có nhánh) · Rock · **Acacia** (dù savanna) · **Palm** (5 tàu lá — mangrove/jungle) · **DeadTree** (khô — desert/badlands/chaparral) · **Bush** (bụi kép) · **Reed** (lau sậy — đầm/bog) · **Tuft** (cỏ — grassland/steppe/alpine/tundra). Mix @2048²: Pine 20.8k, Bush 14.6k, Round 10.5k, Tuft 8.2k, Jungle 5.1k, Reed 1.6k, Palm 1.5k… Tundra giờ KHÔNG còn cây (đúng thực tế). Chỉ cây cao castShadow.

## Nhẹ CPU/GPU
- Nút **GPU: Đẹp/Nhẹ** trên HUD — áp LIVE không remount context (`QualityApplier`: setDpr 1.5→1, `gl.shadowMap.enabled` + recompile material, cây bỏ shadow + giữ 1/2 instance). Bài học: remount `<Canvas key>` làm chết render loop (màn đen) — đừng remount, toggle runtime.
- `dpr` mặc định cap [1, 1.5]; `powerPreference: high-performance`; hồ 280→1 draw call; ground-cover không đổ bóng.
- **Rộng hơn**: `RENDER_SIZE` 1000→**1200**.
- Cát ướt: dải beach sát mép nước tối đi 14% (swash zone) trong color bake.

## Verify
- Smoke @2048²: 4.1s · 22/22 biome · 121 thác · 100 hang · flora 63k/11 loại · 0 NaN. @1024²: 1.0s.
- Playwright: default/walk/fly/cine/low đều sạch, **0 lỗi console**; walk đứng đúng điểm teleport nhìn ra hồ+rừng+suối; fly thấy thác trắng trên vách; Nhẹ render tức thì.
- Build ✅ · test 7/7 & 237/237 ✅ · lint 0 lỗi (432 warning legacy-any, +3 từ pattern `args as any` có sẵn).

## Tinh chỉnh nhanh
- Ngưỡng thác → `MIN_DROP` (Pass 4e); số/kích thước hang → prob `0.02`/`slope>0.35` (Pass 4f) + scale trong WorldCaves; độ đậm suối → hệ số `0.55` trong buildColorTexture; mật độ từng loài → `floraDensity`/`pickFlora`; tốc độ bay/đi bộ → `base` trong WorldCameraRig; ngưỡng ao → `MIN_LAKE_CELLS`.

---

# 🧹 BACKEND — clippy sạch (lib) + rustfmt toàn crate (2026-07-03)

- `cargo clippy --fix` + sửa tay 4 `needless_range_loop` → **lib 0 warning**. `cargo fmt` chuẩn hóa toàn bộ crate (trước đây chỉ format từng file qua hook → diff lớn 1 lần, sạch về sau). Toàn bộ `cargo test` pass.
- Còn lại (chỉ trong test binaries, không đụng semantics): `unused Result` ×8, `MutexGuard across await` ×5 (adversarial_challenger), `clamp-like` ×3, dead-code TrackingAllocator — sửa sau nếu muốn clippy --all-targets sạch 100%.

---

# 🌏 KHÍ HẬU THẬT v11 — Rain shadow + 22 biome vùng lớn + texture full-res (2026-07-03)

Yêu cầu: "map chân thật, tối ưu, đẹp, nhiều môi trường nhất có thể". **Bump `WORLD_GEN_VERSION` 10→11** (cache tự sinh lại). Đã commit cùng ngày.

## Generation (`worldGen.ts`)
- **Khí hậu kiểu Trái Đất**: quét **bóng mưa (rain shadow)** 2 chiều gió + đai gió theo vĩ độ (mậu dịch đông→tây ở nhiệt đới, gió tây ôn đới, gió đông vùng cực); **đai khô Hadley** (~lat 0.62) bảo đảm sa mạc đúng chỗ trên mọi seed + **ITCZ** giữ rừng mưa xích đạo. Tốc độ khô/ẩm chuẩn hóa theo world-distance (không phụ thuộc resolution — 2048 không còn khô gấp đôi 1024).
- **Đường tuyết/đá theo nhiệt độ ĐÃ TRỪ LAPSE** (vĩ độ + độ cao, lapse 1.35): băng vĩnh cửu ở cực sát mực biển, đỉnh núi xích đạo vẫn đóng tuyết. Ngưỡng `T_GLACIER/T_SNOW/T_ROCK/T_ALPINE` trong `classify`.
- **Ma trận Whittaker đầy đủ → 22/22 biome hiện diện** thành VÙNG LỚN liền mạch (trường khí hậu mượt ở tầm lục địa); Badlands = khô + dốc; Mangrove bờ nóng-ẩm thoải; Bog (lạnh) / Swamp (ấm); Savanna đổi flora sang cây tán tròn (keo), chỉ Desert còn xương rồng.
- **Lọc đa số 3×3** khử speckle phân loại (nước/sông/bãi/mangrove/băng không tham gia — chúng mỏng hợp lệ).
- **seaLevel = phân vị histogram của elevation SAU erosion** → đất đúng `LAND_FRACTION` 38% với mọi seed; `FREQ` 1.35→0.95 → MỘT lục địa khổng lồ + đảo vệ tinh (hết quần đảo vụn).
- **Tối ưu**: counting sort 16-bit cho flow pass (O(n), thay sort so sánh 4M index) → **2048²: ~3.8s** (v10 ~5.0s), 1024²: ~1.0s, 0 NaN.

## Render
- `WorldTerrain`: màu biome bake vào **DataTexture sRGB full-res** (2048² ~16MB GPU; mesh 384 chỉ lo hình khối) + jitter sáng ±4.5%/cell + mipmap + anisotropy 8 → biên giới biome/sông/bãi sắc nét ở mọi khoảng cách, GPU tự blend chuyển tiếp; **normal map PHẦN DƯ** (elevation full-res − mặt mesh bilinear) thêm chi tiết xói mòn/gờ núi mà không double-shading với normal đỉnh. Lưu ý: bake texture ~0.5–1s main-thread 1 lần khi load (tương lai: chuyển vào worker).
- `WorldWater`: **fog thủ công đồng bộ `scene.fog` mỗi frame** (nước tan vào sương y như đất — hết vệt sáng/mép cứng chân trời); mọi chi tiết (sóng vertex, micro-normal, specular, foam) **tắt dần theo khoảng cách** (hết moiré + cột glitter aliasing); **fix lỗ thủng biên map**: ngoài footprint heightmap coi đáy = biển sâu (trước đây clamp UV lấy chiều cao ĐẤT ở hàng biên → depth 0 → alpha 0 → "quạt trời" xuyên biển tới vô cực); **foam theo độ sâu tuyệt đối 1.5 unit** (hết foam phủ kín hồ núi nông); Fresnel phản chiếu màu sương trời (không phải turquoise nông); plane biển ×30 renderSize + `frustumCulled=false` (mép vượt far-clip từ mọi vị trí orbit).
- `WorldVegetation`: cây ghép **2 tông trong 1 geometry** (thân nâu + tán lá, `mergeGeometries` + vertex color — vẫn 1 InstancedMesh/loại), tán kép, thông 2 tầng, xương rồng có nhánh; **instanceColor** biến thể sáng ±18% + lệch tông/cây (rừng hết cảm giác "1 cây nhân bản"); `castShadow` (trừ đá).
- `WorldSky`: mây = cụm ellipsoid dẹt low-poly (hết slab/cột — hệ số cũ ×4 tạo blob 400 unit chiếm nửa trời), cao hơn (~1080) + tản rộng ±1350.
- `WorldShowcase`: hook chẩn đoán `window.__worldScene` (tooling bật/tắt object khi debug render).

## Verify
- Smoke node (esbuild bundle **`world_smoke.ts` ở root — GIỮ LẠI để tune tiếp**): 22/22 biome @1024² & @2048², 0 NaN, đất 38.0%, phân bố lành mạnh (Steppe 15% · Taiga 13% · Grassland 12.5% · Shrubland 11% · Glacier 10% · … · Desert 0.85% · Badlands 0.33% đất). Chạy: `npx esbuild world_smoke.ts --bundle --platform=node --format=cjs --outfile=<tmp>/s.cjs && node <tmp>/s.cjs` (env `SMOKE_SIZE`, `SMOKE_OUT`).
- `world_preview.png` (root, tracked) đã tái sinh @2048² — đúng world app render (seed `seed`).
- Screenshot Playwright headless (SwiftShader, flags `--enable-unsafe-swiftshader`) trên `landscape.html`: default + teleport-zoom sạch — hết quạt trời/moiré/foam hồ; cần Pause ngay khi world ready vì đồng hồ ngày/đêm chạy theo wall-time.
- `npm run build` ✅ · `npm run test` 7/7 ✅ · `npm run test:frontend` 237/237 ✅ · lint 0 lỗi (429 warning cũ).

## Tinh chỉnh nhanh
- Tỷ lệ đất → `LAND_FRACTION`; độ lớn lục địa → `FREQ`; đai khô → biên độ `0.16`/tâm `0.62` của `hadleyDry`; cường độ bóng mưa → `RISE/DRIZZLE/LAND_ET/OCEAN_WET`; ngưỡng biome → ma trận trong `classify()`; đường tuyết → `T_*`; jitter màu đất → `0.95 + hash*0.09` trong `buildColorTexture`; độ sâu foam → hằng `1.5` trong WorldWater; kích thước/độ cao mây → hệ số trong `clouds` useMemo của WorldSky.

---

# 🌍 [ĐANG LÀM] World Terrain Overhaul — Lục địa khổng lồ + Biome + Cache

**Cập nhật:** 2026-07-01. **Trạng thái:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint 0 lỗi. Các bước 1→5 **ĐÃ COMMIT**; bản nâng cấp "to/rộng/nhiều biome" ở mục 🚀 bên dưới.

## Bối cảnh
Đập đi xây lại phần sinh + render địa hình cho `landscape.html`: lục địa khổng lồ (data 1024²), noise sắc nét (ridged + domain warp), biome Whittaker (nhiệt×ẩm), và cache nhị phân để load tức thì từ lần 2.

## Kiến trúc (đã chạy thật)
- **Dữ liệu SoA TypedArrays** (KHÔNG còn `TerrainCell[][]`): `World` gồm `elevation/moisture/temperature/flow: Float32Array`, `biome: Uint8Array`, flora SoA. ~17MB @1024².
- **Cache nhị phân**: IndexedDB structured-clone (binary, không JSON) → lần 2 đọc thẳng buffer vào RAM, bỏ qua thuật toán. Key có `WORLD_GEN_VERSION` để invalidate.
- **Tách data-resolution khỏi mesh-resolution**: data 1024² nhưng mesh render ~256 segment sample heightmap → chi tiết cao, không cần 1M vertex.

## Các file chính (đều mới, dưới `src/components/Landscape/`)
| File | Vai trò |
|---|---|
| `utils/worldGen.ts` | Bộ sinh SoA: fBm 8-octave + ridged + domain warp + D8 flow (sông) + temperature(vĩ độ+lapse) + moisture(2 tầng + bốc hơi) + Whittaker 14 biome + flora SoA. Sinh 1024² ~1s, deterministic. |
| `utils/worldCache.ts` | memo + IndexedDB binary + `loadOrGenerateWorld()` / `getMemoizedWorld()` / `clearWorldCache()`. |
| `utils/worldGen.worker.ts` | **(Bước 1 vừa xong)** Web Worker sinh map off-thread, transfer zero-copy buffer về. |
| `WorldTerrain.tsx` | Mesh biome-color, displaced, winding CCW (normal +Y). Props: `renderSize`(400), `heightRatio`(0.13), `meshResolution`(256). |
| `WorldVegetation.tsx` | Cây instanced từ flora SoA (thông/tán tròn/jungle/xương rồng/đá theo biome). |
| `WorldShowcase.tsx` | Canvas + async-load world (cache) + terrain + vegetation + ocean plane + OrbitControls + fog/light. Hằng: `WORLD_SIZE=1024`, `RENDER_SIZE=400`, `HEIGHT_RATIO=0.13`, `MESH_RES=256`. |
| `src/landscape.tsx` | Đã trỏ entry `landscape.html` → `WorldShowcase` (component cũ `LandscapeShowcase` vẫn còn cho test). |

## ✅ Đã hoàn thành
- [x] Bộ sinh SoA huge-scale + noise sắc nét + Whittaker 14 biome (verify @1024²: ~1s, 0 NaN, đủ biome).
- [x] Cache nhị phân IndexedDB (versioned).
- [x] Render 3D: WorldTerrain (mesh-decoupled) + WorldVegetation (instanced) + ocean + orbit + fog.
- [x] Wire vào `landscape.html`.
- [x] **BƯỚC 1 — Web Worker**: sinh 1024² off-thread, **không treo UI**; có fallback sync (test/SSR). Vite bundle thành chunk riêng (`worldGen.worker-*.js`). Verify: render giống hệt, build+test xanh.

## ✅ BƯỚC 2 — Nước đẹp hơn (Water shader) — XONG (2026-07-01)
File mới `src/components/Landscape/WorldWater.tsx` thay `ocean plane` phẳng cũ trong `WorldShowcase.tsx`.
- [x] **Custom GLSL ShaderMaterial** cho mặt biển: sóng swell động theo time (vertex displace), màu theo **độ sâu** (sample heightmap `DataTexture` Float32 → đáy: nông teal `#3fcfe0`, sâu xanh đậm `#06203f`).
- [x] **Bọt bờ biển (foam)**: dải trắng fbm-noise nơi `depth → 0`, fade mềm đúng waterline (không còn cạnh plane cứng).
- [x] **Sông**: quads cho cell `Biome.River` (từ `world.flow`), nâng nhẹ trên terrain, ripple chảy nhanh. (Hồ tách riêng — xem Bước 5.)
- [x] Fresnel sky-tint + sun specular (uniform `uSunDir` = hướng tới mặt trời, khớp directional light scene).
- [x] `fog={false}` trên cả 2 ShaderMaterial (tránh crash `refreshFogUniforms`).
- **Verify:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint 0 lỗi. Heightmap tex 1024² Float32 ~4MB GPU.
- **Tinh chỉnh:** màu nông/sâu → `uShallow`/`uDeep`; độ trong → `uOpacity`; biên độ sóng → `uWaveAmp`; bề rộng ribbon sông → `half` trong `riverGeom`.

## ✅ BƯỚC 3 — Bầu trời + ngày/đêm + thời tiết — XONG (2026-07-01)
2 file mới + wire vào `WorldShowcase.tsx`. Scale lớn cho world mới (terrain ±200, camera tới 1200, far 4000) — KHÔNG dùng lại Sky/Weather cũ vì chúng tuned cho world ~±100 (FogExp2 0.005 sẽ trắng xoá map 400).
- `WorldSky.tsx`: dome BackSide R≈2600 (`fog={false}` để giữ gradient trời), mặt trời/mặt trăng quỹ đạo R≈1800 + directional light (shadow), hemisphere + ambient theo `getSkyParams(timeOfDay)`, sao (700), mây trôi (8 cụm, scale ×4). Tự set `scene.background` theo skyColor mỗi frame. Export `sunDirectionForTime(t)` → dùng chung cho water specular.
- `WorldWeather.tsx`: mưa/tuyết (particle box ±RENDER_SIZE) + **linear `THREE.Fog`** near/far theo `worldScale` (clear: 800→3200; rain/snow/fog dày dần), màu fog đổi theo weather + ngày/đêm, ease mượt khi chuyển. Sở hữu `scene.fog`.
- `WorldShowcase`: state `timeOfDay`/`speed`/`weather`, đồng hồ ngày/đêm auto (`setInterval`, pause khi speed=0), HUD nhỏ (clock + play/pause + speed 0.5/1/2/4× + 4 nút weather), truyền `sunDir` động vào `WorldWater` (specular bám mặt trời, cập nhật mỗi frame trong useFrame). Bật `shadows` trên Canvas. Bỏ ambient/directional/fog tĩnh cũ.
- **Verify:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint 0 lỗi.
- **Tinh chỉnh:** tốc độ ngày/đêm → bước `0.05*speed` trong interval; độ dày fog → `fogProfile()` trong WorldWeather; bán kính dome/sao → hằng trong WorldSky.

## ✅ BƯỚC 4 — Minimap + HUD đầy đủ — XONG (2026-07-01)
1 file mới + nâng cấp HUD + cầu nối camera↔overlay trong `WorldShowcase.tsx`.
- `WorldMinimap.tsx`: bản đồ top-down vẽ `world.biome` qua `BIOME_RGB` + hillshade nhẹ theo elevation (pre-render 192² ImageData 1 lần/world). Marker mục tiêu (đỏ) + mũi tên hướng nhìn, cập nhật qua rAF (không re-render React mỗi frame). **Click để teleport** camera. Toggle **legend** 10 biome. Export type `CameraView`.
- **Cầu nối camera↔HTML overlay:** không dùng `window.activeCamera` global như Minimap cũ — thay bằng `viewRef` (mutable ref) mà `OrbitCam` ghi vào mỗi frame trong `useFrame` (targetX/Z, camX/Z); minimap & HUD đọc ref. `teleportRef` chứa lệnh teleport đang chờ → OrbitCam dời cả `controls.target` lẫn camera (giữ nguyên góc nhìn).
- **HUD nâng cấp:** đồng hồ + **phase** (🌙/🌅/☀/🌇), Play/Pause, speed 0.5–4×, 4 nút weather, **toạ độ 📍 x/z** (đọc từ viewRef qua interval 300ms, không re-render mỗi frame), nút **⟲ Reset** view (teleport về gốc).
- **Refactor:** `sunDirectionForTime()` chuyển sang `utils/skyParams.ts` (dùng chung sky+water, tránh warning fast-refresh).
- **Verify:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint **0 lỗi** (429 warning cũ, không phát sinh mới).

## ✅ BƯỚC 5 — Hydraulic erosion + hồ nước — XONG (2026-07-01)
Thay đổi ở tầng generation (`worldGen.ts`) + render (`WorldWater.tsx`, `WorldMinimap.tsx`). **Bump `WORLD_GEN_VERSION` 1→2** (cache cũ tự invalidate); worker transfer thêm `water.buffer`.
- **Erosion (droplet):** `hydraulicErosion()` chạy Pass 1b (sau elevation, TRƯỚC flow+biome nên sông/biome bám lòng đã khắc). Mô phỏng hạt mưa cuốn/lắng phù sa (inertia/capacity/erode/deposit/evaporate/gravity, 30 bước/hạt), phân bổ trên 4 góc bilinear. Deterministic (rng riêng seed từ baseSeed). Số hạt scale theo diện tích: `min(120k, n*0.06)`. Clamp [0,1] sau khi khắc.
- **Hồ (priority-flood, Barnes et al.):** `computeLakes()` Pass 4b — flood từ biển/rìa map vào trong bằng min-heap (typed-array), nâng mỗi cell tới ngưỡng tràn (sill). Nơi mặt filled > đất thật → hồ; lưu `water: Float32Array` (cao độ mặt nước, 0 nếu không có hồ). `LAKE_MIN_DEPTH=0.006` lọc vũng nông (speckle từ erosion). Cây không mọc trên ô hồ.
- **Render:** `WorldWater` thêm mesh `world-lakes` — quads phẳng tại `water*heightUnits`, shader `uWaterType=2` (dùng nhánh depth-color + foam như đại dương, khác nhánh river). `WorldMinimap` tint ô hồ sang xanh nước.
- **Verify:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint 0 lỗi. Smoke `generateWorld(256²)`: 115ms, elev∈[0,1], 735 ô hồ, 0 NaN. (1024² ước tính ~1.5–2.5s off-thread, cache sau lần đầu.)
- **Tinh chỉnh:** độ mạnh khắc → `erosionDroplets` / các hằng trong `hydraulicErosion`; ngưỡng hồ → `LAKE_MIN_DEPTH`; màu hồ → `lakeUniforms` (`uShallow`/`uDeep`); tắt hẳn → opts `erosion:false` / `lakes:false`.

> 🎉 **World Terrain Overhaul HOÀN TẤT các bước 1→5.** Còn tùy chọn tương lai: thermal erosion, hồ có sông nối (inlet/outlet), phản chiếu mặt nước (reflection probe).

---

# 🚀 NÂNG CẤP MAP — To hơn, Rộng hơn, Nhiều môi trường nhất (2026-07-01)

Mục tiêu người dùng: "map to, rộng, nhiều môi trường nhất". **Bump `WORLD_GEN_VERSION` 2→3** (cache tự sinh lại).

## To hơn (data) & Rộng hơn (world-space)
- `WORLD_SIZE` **1024 → 2048** (~4M cell, gấp 4× chi tiết). Sinh 1 lần off-thread **~5s @2048²** rồi cache; RAM thường trú ~90MB.
- `RENDER_SIZE` **400 → 1000** (rộng gấp 2.5×), `HEIGHT_RATIO` 0.13→0.14, `MESH_RES` 256→**384** (mesh chi tiết hơn).
- Camera: `near=2`, `far=RENDER_SIZE*11` (phải > dome `worldScale*6.5`); orbit `min=RENDER_SIZE*0.06`, `max=RENDER_SIZE*3.2` (< dome để camera luôn trong vòm trời). Sky/fog/dome tự scale theo `worldScale=RENDER_SIZE`. `maxFlora` 60k→**90k**.

## Nhiều môi trường (biome) — 14 → **22**
Thêm 8 biome: **Lake, Mangrove, Chaparral, Steppe, Alpine, Badlands, Glacier, Bog** (enum + `BIOME_RGB` + flora + legend minimap đủ 22 mục).
- `classify()` viết lại theo **dải elevation × Whittaker (nhiệt×ẩm)**: bờ nóng-ẩm→Mangrove; đỉnh→Glacier/Snow/Rock; dải alpine→Alpine/Rock; trũng ẩm→Bog(lạnh)/Swamp(ấm); nóng: Desert→Badlands→Savanna→Chaparral→Jungle; ấm/ôn: Steppe→Grassland→Shrubland/Forest; lạnh: Tundra/Taiga.
- **Tinh chỉnh khí hậu để mọi biome xuất hiện thật:** nhiệt độ trải rộng hơn (`lat*0.78 + tNoise*0.22 - lapse`), bay hơi `(temp-0.5)*0.5`, ẩm nền `mBase*0.95` → dải ẩm rộng đủ chứa cả Desert (khô kiệt) lẫn Jungle (sũng nước).
- Ô hồ được recolor `Biome.Lake`; minimap tint xanh; cây không mọc trên hồ.

## Verify (đo thật bằng smoke test tạm, đã xoá)
- **@2048² (độ phân giải thật): present = 22/22 biome**, 5.0s, 49.3k ô hồ, 0 NaN. Phân bố lành mạnh (Grassland/Shrubland/Forest/Taiga/Chaparral nhiều; Desert/Steppe/Tundra/Glacier hiếm nhưng có).
- @512²: 22/22, 0.36s. `npm run build` ✅ · `npm run test:frontend` ✅ 237/237 · lint 0 lỗi.
- **Tinh chỉnh nhanh:** kích thước data → `WORLD_SIZE`; độ rộng → `RENDER_SIZE`; ngưỡng biome → `classify()`; độ khô sa mạc → hệ số `evaporation` + ngưỡng `Desert/Badlands`; tắt biome nước → opts `lakes:false`.

---

# 🛠 FIX RENDER — Sông/Hồ/Đá bay (2026-07-01)

Sửa triệt để 3 lỗi hiển thị. **Bump `WORLD_GEN_VERSION` 3→4** (cache sinh lại). File mới `utils/worldSample.ts` (sampler dùng chung).

1. **Sông/suối** — BỎ hoàn toàn quad rời rạc trong `WorldWater`. Giờ **bake vào vertex color của terrain** (`WorldTerrain`): sample `flow` bilinear → trộn màu nước xanh liên tục theo dòng chảy + **khoét rãnh nông** (`e -= riverAmt*0.02`) để sông nằm trong lòng. Hòa vào địa hình, không còn hình vuông đứt đoạn.
2. **Hồ nước** — thay vì ghép nhiều ô vuông: `computeLakes()` giờ gom **connected-component** thành từng bồn (lọc bồn nhỏ, giữ ≤280 bồn lớn nhất), trả về `lakeBasins[{level,bbox}]`. `WorldWater` sinh **MỘT plane/bồn** phủ bbox tại `level`; shader fade alpha→0 nơi cạn nên plane chỉ hiện trên phần chìm (không tràn ra đất). Thêm **viền cát**: `computeShore()` (BFS khoảng cách tới nước) → `WorldTerrain` trộn màu cát ẩm sát mép nước (biển + hồ).
3. **Đá bay** — nguyên nhân: flora lấy Y từ elevation full-res 2048² nhưng terrain render ở mesh 384 → lệch trên núi. Fix: `sampleMeshHeight()` lấy đúng cao độ **mặt mesh** (bilinear trên lưới mesh), và seat mỗi geometry bằng `boundingBox.min.y` để đáy chạm đất. Đá/cây bám sát mặt đất tuyệt đối.
- **Verify:** `npm run build` ✅ · `npm run test:frontend` ✅ 237/237 · lint 0 lỗi. Smoke @1024²: 95 bồn hồ, shore band 120k ô, mesh-snap khớp tuyệt đối (sai số <1e-5), 0 NaN. @2048² bồn hồ ≤280 planes.

---

# 🌱 NÂNG CẤP BIOME — Slope map + màu tương phản + foliage theo hệ sinh thái (2026-07-01)

Địa hình trước đây "đơn điệu" vì thiếu **độ dốc (slope)** (núi vẫn xanh cây như đồng bằng) và màu biome hơi nhạt. **Bump `WORLD_GEN_VERSION` 4→5.**
- **Slope map (`computeSlope`)** — signal thứ 3 (ngoài height+moisture): gradient elevation đo trên stencil rộng (`step≈size/200`) để phản ánh sườn núi lớn, không phải nhiễu erosion; chuẩn hoá về [0,1]. Lưu `slope: Float32Array`.
- **Bãi cát**: dải Beach rộng hơn (`seaLevel+0.022`) + viền cát `shore` — màu cát, **không cây**.
- **Biome theo Height×Moisture×Temp×Slope**: giữ 22 biome (Desert khô/thấp, Swamp ẩm/thấp, Forest·Jungle ẩm/trung, Grassland·Steppe trung, Snow/Glacier cực cao). **Thêm vách đá**: `slope>0.85` → `Rock` (chỉ ~3% đất — sườn dốc nhất), phần đất thoải giữ nguyên biome xanh. Thung lũng = vùng thoải+ẩm (flow tụ) → Forest/Swamp.
- **Màu tương phản hơn** (`BIOME_RGB`): sa mạc vàng đậm, rừng xanh tươi, cỏ xanh sáng, tuyết trắng tinh, badlands nâu đỏ…
- **Foliage theo hệ sinh thái**: `floraForBiome` + `floraDensity` theo biome; **mật độ ×(0.5+moisture)** (rừng ẩm dày, đồng khô thưa); **không cây trên slope>0.78** (vách đá) và trên nước/bãi cát.
- **Render**: `WorldTerrain` trộn **màu đá xám theo slope** (`smoothstep(0.55,1.0)`) → sườn dốc lộ đá, phá thế xanh đơn điệu; giữ blend cát + sông.
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: 22/22 biome, slope>0.85 = 3.2% đất (không phải cả bản đồ hoá đá), 0 cây trên vách dốc, 0 NaN.
- **Tinh chỉnh:** độ nhạy slope → hệ số `0.06` + `step` trong `computeSlope`; ngưỡng vách đá → `slope>0.85` trong `classify`; mật độ cây → `floraDensity` × `(0.5+moisture)`.

---

# 🎨 FIX 3D vs MINIMAP — Màu terrain & luật trồng cây (2026-07-01)

Terrain 3D bị "xám/trắng nhợt nhạt loang lổ", không khớp minimap; cây mọc dưới nước/ven hồ/trên đá. **Bump `WORLD_GEN_VERSION` 5→6.**
1. **Màu 3D = màu minimap.** Nguyên nhân: `WorldTerrain` chồng 3 lớp tint (cát theo `shore`, đá xám theo `slope`, nước theo `flow`) lên màu biome → ở map 2048² bờ biển/hồ rất dài nên tint cát+đá phủ phần lớn đất, thành xám nhạt loang lổ. **Fix:** bỏ hết tint chồng, màu vertex = `BIOME_RGB[biome]` **thuần** — đúng bảng màu minimap dùng (sông/bãi cát/hồ vốn đã là biome River/Beach/Lake nên vẫn hiện). Đổ bóng địa hình do ánh sáng scene lo, không tint vào màu.
2. **Luật trồng cây chặt** (`worldGen` flora pass): chỉ mọc khi `elevation > seaLevel` **và** `water==0` **và** `slope ≤ 0.78` **và** `shore ≤ 0.8` (không sát mép nước) **và** biome hợp lệ. `floraForBiome` giờ trả `-1` cho Ocean/Beach/River/Lake/Snow/Glacier/**Rock/Badlands** (bỏ đá/boulder trên núi trọc). Validate lại **ô sau khi jitter** để cây không lệch ra nước. Mật độ vẫn ×(0.5+moisture).
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: flora=67.6k, **0 cây dưới nước / trên bãi/tuyết/đá / dưới mực nước / vách dốc**.

---

# 🏖 BÃI BIỂN — dải cát ven bờ (2026-07-01)

Biome `Beach` đã có nhưng dải quá hẹp (`seaLevel+0.022`) nên gần như không thấy. **Bump `WORLD_GEN_VERSION` 6→7.**
- **Nới dải cát**: đất có `elevation ∈ [seaLevel, seaLevel + 0.05)` → luôn là `Biome.Beach` (màu cát `#EAD8A2`). `WorldTerrain` tô thẳng `BIOME_RGB[Beach]` nên bãi cát **khớp giữa minimap và 3D**.
- **Không cây/đá trên cát**: `floraForBiome(Beach) = -1` + gate `shore≤0.8` + validate ô jitter → bãi cát nhẵn hoàn toàn.
- Mangrove lùi vào **trong** bãi cát (`elev < beach+0.04 && hot && very-wet`) để không đè lên dải cát.
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: 97.6k ô cát, **97.6% dải ven bờ là Beach**, **0 flora trên cát**.
- **Tinh chỉnh:** độ rộng bãi → `BEACH_WIDTH` trong `classify`; màu cát → `BIOME_RGB[Beach]`.

---

# 🏝 BÃI BIỂN v2 — Làm phẳng ven bờ + Beach theo độ dốc (2026-07-01)

Lỗi: cát dính lên vách đá dốc, bãi hẹp/dốc. **Bump `WORLD_GEN_VERSION` 7→8.**
1. **Làm phẳng ven biển (Pass 1c, heightmap)**: ô có `elevation ∈ [seaLevel, seaLevel+0.14]` được kéo về **ramp thoải** (`flat = seaLevel + 0.35·(e−seaLevel)`) rồi blend ngược về địa hình thật ở trên bằng `smoothstep` → bờ dâng rất từ từ từ mặt nước. Nhiều ô rơi vào dải cát hơn → **bãi rộng & thoai thoải**; mũi đất dốc vẫn dâng nhanh nên vẫn là vách đá.
2. **Beach theo slope (`classify`)**: ô ven biển **CHỈ** là `Beach` khi `slope ≤ 0.3` (bằng phẳng); nếu dốc (vách đá đâm xuống biển) → `Rock`. Mangrove cũng chỉ mọc ở bờ thoải.
- **Frontend không đổi**: `WorldTerrain` vẫn tô `BIOME_RGB[biome]` → cát/vách đá do biome quyết định (theo slope) tự hiện đúng và **khớp minimap** (không lặp tint gây xám).
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: beach=92k (rộng), **maxBeachSlope=0.30 (cát không bao giờ dốc)**, coastalRock=59k (vách đá ven biển), **0 flora trên cát**, 0 NaN, elevation∈[0,1].
- **Tinh chỉnh:** độ rộng/thoải bãi → `COAST_BAND`/`COAST_GAIN` (Pass 1c); ngưỡng cát↔vách → `BEACH_MAX_SLOPE` trong `classify`.

---

# 🌊 BỜ BIỂN VẬT LÝ v3 — Thềm 2 bên mực nước + đáy cát nông (turquoise) (2026-07-01)

Làm đúng 3 nguyên lý vật lý của coastline. **Bump `WORLD_GEN_VERSION` 8→9.**
1. **Thềm lục địa đối xứng (Pass 1c)**: terrace giờ làm phẳng **CẢ hai bên** mực nước — ramp thoải phía trên (`COAST_BAND=0.14`) **và thềm nông phía dưới** (`SHELF_BAND=0.09`), blend về địa hình thật bằng `smoothstep`. Bờ dâng từ từ từ mặt nước và **lài dần xuống đáy nông**, tạo chỗ cho bãi rộng.
2. **Cát theo độ dốc (`classify`)**: ô thuộc thềm ven bờ `[seaLevel−SHALLOW, seaLevel+BEACH_WIDTH]` → **Beach nếu `slope ≤ 0.3`**, **Rock nếu dốc** (vách đá — dù nhô lên hay đâm xuống nước). Cát không dính trên vách.
3. **Đáy cát nước nông**: ô vừa chìm dưới nước `[seaLevel−0.06, seaLevel)` mà thoải → **Beach (đáy cát)**. Qua Water Shader trong suốt → **ánh xanh ngọc lam (turquoise)** ở nông rồi chuyển **xanh thẫm** ở sâu. Shader: nông trong hơn (`alpha=mix(0.4,0.92,depthN)`) + màu nông turquoise `#48DDCA` → đáy cát hắt lên.
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: bãi khô 98k + **đáy cát nông 87k** + vách đá ven biển 110k, **0 ô cát dốc**, **0 flora trên cát**, 0 NaN.
- **Tinh chỉnh:** độ sâu thềm nông → `SHELF_BAND`; bề rộng đáy cát → `SHALLOW`; sắc turquoise/độ trong → `uShallow`/`alpha` trong WorldWater.

---

# 🗺 ĐẠI TU TERRAIN — Bản đồ sạch: lục địa lớn, đồng bằng phẳng, biome theo tầng cao (2026-07-02)

Terrain trước bị "lốm đốm như bùn" + cát phủ khắp đảo. Đại tu theo 3 nguyên lý. **Bump `WORLD_GEN_VERSION` 9→10.** (Ánh sáng scene cũng đã hạ ở commit trước để màu không cháy trắng.)
1. **Noise (cấu trúc hình học)**: hạ base `FREQ 2.4→1.35` (lục địa/đại dương lớn), giảm gain fBm `0.5→0.42` + **bỏ lớp roughness `*6`** (hết vụn), `pow(e) 1.25→1.7` (đồng bằng phẳng rộng), **giảm erosion** `droplets n*0.06→n*0.02` (bớt nhấp nhô). Bỏ hẳn `Pass 1c` coastal flattening (thủ phạm tạo cát tràn).
2. **Đồng bộ minimap**: cả minimap và 3D **đã dùng chung `BIOME_RGB[biome]`** — chỉ cần phân loại biome sạch là khớp. Không còn tô 2 kiểu.
3. **Biome theo tầng độ cao (strict)**: `< seaLevel` Ocean → `+0.02` **cát mỏng** (gate thêm bằng **khoảng cách tới biển `coast`** để không tràn ra đồng bằng phẳng; dốc→Rock cliff) → tới `ROCK_LEVEL=0.66` là **phần xanh chủ đạo** (Grassland/Forest/Jungle/Taiga/Tundra/Swamp theo khí hậu) → `0.66–0.86` **Rock** → `>0.86` **Snow**. Bỏ các biome tông vàng (Desert/Savanna/Steppe/Chaparral/Badlands…) khỏi đất thường để hết "sandy".
- **Verify (smoke @1024²):** **green 75%** (chủ đạo) · **beach 5%** (mỏng) · rock 14% · snow 3% · **rough 5.4e-4** (mượt, hết lốm đốm) · 0 NaN. Build ✅ · 237/237 ✅ · lint 0 lỗi. Screenshot in-browser: đảo xanh mướt + bãi cát mảnh + núi tuyết, **khớp minimap**.
- **Tinh chỉnh:** kích thước lục địa → `FREQ`; độ phẳng đồng bằng → số mũ `pow`; ngưỡng tầng → `ROCK_LEVEL`/`SNOW_LEVEL`; độ mỏng bãi → `BEACH_TOP` + ngưỡng `coast>0.65`.

## Cách chạy / kiểm tra nhanh
- Dev: `npm run dev` → mở `http://localhost:5173/landscape.html` (lần đầu sinh ~1s off-thread, sau đó cache → tức thì).
- Sinh thế giới mới: gọi `clearWorldCache()` hoặc bump `WORLD_GEN_VERSION` trong `worldGen.ts`.
- Tinh chỉnh: núi cao/thấp → `HEIGHT_RATIO`; to/nhỏ → `RENDER_SIZE`; chi tiết mesh → `MESH_RES`; nhiều desert hơn → ngưỡng trong `classify()` của `worldGen.ts`.
- Ảnh preview tĩnh (biome+hillshade) đã sinh: `world_preview.png` ở thư mục gốc.

## Lưu ý
- Các bước trên đã được commit (xem git log quanh 2026-07-01→03).
- `landscape.html` giờ hiển thị World mới; `LandscapeShowcase` (cũ) vẫn được 237 test dùng — **đừng xóa**.

---

# 🐰 [LƯU TRỮ] Mô hình Thỏ 3D Papercraft (task cũ)

Hệ thống vừa bị khởi động lại (restart) do tài nguyên máy yếu khi chạy toàn bộ dự án Bevy/Tauri. Dưới đây là tình trạng hiện tại và các công việc cần làm tiếp theo để tối ưu hóa và hoàn thiện mô hình Thỏ 3D theo phong cách Papercraft (gấp giấy có góc cạnh và viền đen).

---

## 1. Các công việc đã hoàn thành
- **Tạo Sandbox Độc lập (rabbit-standalone)**: Giúp chạy thử mô hình thỏ trực tiếp bằng HTML/Three.js mà không cần chạy backend Tauri/Rust nặng nề.
- **Sửa lỗi chân thụt thò (Leg Clipping)**: Đã áp dụng công thức co giãn chân động (Dynamic Leg Extension) theo nhịp nhảy `hopY` trên cả bản standalone và component React. Chân không còn bị tách rời hay lún sâu vào thân.
- **Nâng cấp thẩm mỹ Cute**: Đã thêm má hồng, mũi hồng, miệng và mắt bóng có tia sáng phản chiếu (eye glints).
- **Vượt qua kiểm thử**: Toàn bộ 56 frontend tests và quy trình build đều thành công (`npm run test:frontend`, `npm run build`).

---

## 2. Các phần CHƯA HOÀN THIỆN (Cần làm tiếp theo)
Theo yêu cầu tham khảo mô hình 3D mới (phong cách Papercraft/Low-poly gấp giấy):

### Tác vụ 1: Chuyển đổi mô hình sang phong cách Papercraft (Faceted Low-Poly)
- [ ] **Giảm phân đoạn hình học (Low Segment Counts)**: 
  - Chuyển các khối cầu (Sphere), khối con nhộng (Capsule) của thân, đầu, tai, chân thành các khối có số phân đoạn cực thấp (ví dụ: 4 đến 8 phân đoạn) để tạo góc cạnh sắc nét, thô ráp.
- [ ] **Áp dụng Flat Shading**: 
  - Bật thuộc tính `flatShading: true` trên tất cả các chất liệu (`MeshStandardMaterial`) để ánh sáng phản chiếu phẳng theo từng mặt đa giác, không làm mịn bề mặt.
- [ ] **Thêm viền đen sắc nét (Black Outlines)**:
  - Sử dụng `THREE.EdgesGeometry` kết hợp với `THREE.LineSegments` vẽ đè lên mỗi bộ phận để tạo viền đen chạy dọc theo các cạnh sắc của khối đa giác, mô phỏng đúng phong cách mô hình giấy gấp.

### Tác vụ 2: Điều chỉnh dáng đứng (Sitting Posture) & Tỷ lệ
- [ ] **Dáng đứng co chân**: Điều chỉnh tư thế mặc định của thỏ giống dáng ngồi khép chân trong ảnh tham khảo.
- [ ] **Tai nhọn góc cạnh**: Thiết kế lại tai thỏ sử dụng hình nón hoặc hình trụ 4 cạnh vuốt nhọn ở đỉnh.

### Tác vụ 3: Đồng bộ động học hoạt ảnh
- [ ] Cập nhật hoạt ảnh nhảy (hopping) và nhai (chewing) để hoạt động mượt mà với cấu trúc hình học Papercraft góc cạnh mới mà không làm mất đi tính tương tác của các thanh trượt điều khiển.

---

## 3. Hướng dẫn chạy nhẹ nhàng cho máy yếu (Tránh treo máy)
Để không làm quá tải CPU/RAM dẫn đến việc máy phải tự khởi động lại:
1. **KHÔNG chạy Tauri backend** (`npm run tauri dev` hoặc các lệnh Cargo).
2. **Chỉ chạy server tĩnh siêu nhẹ** phục vụ thư mục độc lập:
   ```bash
   # Đã được khởi chạy ở cổng 8000
   py -m http.server 8000
   ```
3. Mở trình duyệt truy cập: `http://localhost:8000/` để xem và tương tác trực tiếp với mô hình thỏ mà không tốn tài nguyên hệ thống.
