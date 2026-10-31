# Bề mặt hợp nhất giữa hai cây — đo ngày 2026-11-01

Tài liệu này **không** đưa ra QĐ-A. Nó đo cái mà QĐ-A cần biết để trả lời được, vì cho đến
nay quyết định ấy được bàn bằng chữ ("cây nào lớn hơn", "cây nào hơn") chứ chưa bằng số.
Dòng quyết định vẫn thuộc chủ dự án và vẫn ghi vào
[roadmap 31/10 §4](../../planning/COMPLETION_ROADMAP_2026-10-31.md).

Đo trên `feat/ecological-selection` so với `origin/fix/ci-gates-green` (đỉnh chồng PR
#38 → #39 → #40, `78b2e85`). Lệnh: `git merge-tree --write-tree --name-only`, tức Git tự
trả lời chứ không phải tôi suy ra.

## 1. Hai cây phân kỳ ở đâu, và mỗi bên có gì

Điểm rẽ: `043930c` — chính commit đổi giấy phép, cũng là commit chép hai miễn trừ `gix`
sang `deny.toml` ([F-032]).

| | commit bên kia không có | nặng ở đâu |
|---|---|---|
| `feat/ecological-selection` | **102** | `src-tauri/tests/` (153 lượt sửa file), `src/core/` (62), `docs/ai/` (51), `FINDINGS.md` (27), `justfile` (12), hạ tầng mutation (`mutants.toml`, skill mutant-killer) |
| chồng PR #38→#40 | **36** | `src/components/` (57), `src-tauri/tests/` (39), `src/core/` (33), `tests/` (30), `scripts/` (28), `src/types/` (17), artifact giấy phép |

Đây không phải "cây nào hơn". Hai cây làm **hai loại việc khác nhau**: một bên là tuyến
bằng chứng/kiểm định, một bên là tuyến CI-xanh + frontend. Không bên nào bao bên nào.

## 2. Phần máy móc thì nhỏ

**29 file tự gộp, 9 file xung đột**, và cả 9 đều là `CONFLICT (content)` — không có
modify/delete, không rename, không binary.

| file | loại | kích thước xung đột |
|---|---|---|
| `PROJECT.md` | docs (R5) | 1 hunk, 265 dòng |
| `AGENTS.md` | docs (R5) | 1 hunk, 71 dòng |
| `docs/planning/STATE_OF_THE_PROJECT.md` | docs (R5) | 1 hunk, 7 dòng |
| `docs/planning/README.md`, `docs/ai/ORIGINAL_REQUEST.md`, `docs/ai/TEST_INFRA.md`, `docs/ai/TEST_READY.md` | docs | nhỏ |
| `src-tauri/.cargo/audit.toml` | config | 5 hunk, 25 dòng |
| `src-tauri/src/evolution/genotype.rs` | **code — cái duy nhất** | 1 hunk, **14 dòng** |

## 3. Xung đột code duy nhất: lấy bên nào cũng được, nhưng chỉ sau khi trình biên dịch chặn

`genotype.rs` xung đột ở một chỗ:

- nhánh: `HomeostaticState { energy: 100.0, …, starving_ticks: 0 }` (8 trường, viết tay)
- chồng: `HomeostaticState::default()` (bản clippy của PR #40 sửa gọn)

Đo `HomeostaticState` ở `src-tauri/src/ai/hrrl.rs` hai bên:

| | số trường | `impl Default` |
|---|---|---|
| chồng PR | **7** | **có**, viết tay: energy 100.0, hydration 100.0, temperature 37.0, previous_deviation 0.0 |
| nhánh | **8** — thêm `starving_ticks: u32` kèm `#[serde(default)]` | **không có** (`grep 'impl Default' → rc=1`) |

`hrrl.rs` **tự gộp** (nằm trong danh sách 29). Nên cây gộp sẽ có struct **8 trường** cộng
với `impl Default` **7 trường** → **lỗi biên dịch cứng**, và trình biên dịch gọi tên đúng
trường thiếu. Đó là kết cục an toàn: hỏng to tiếng.

Thêm `starving_ticks: 0` vào `impl Default` là xong, và khi đó `::default()` **bằng đúng**
literal của nhánh, nên hai bên xung đột tương đương thật — lấy bên nào cũng đúng.

**Cái bẫy** là sửa lỗi biên dịch bằng cách **xoá** `starving_ticks` khỏi struct. Nó làm
`mortality_system` thôi đếm ngày đói, mà **mọi checksum vẫn y nguyên** — chính doc comment
của trường nói nó nằm **ngoài** `world_checksum` để run tắt cờ còn bit-identical. Hỏng lặng
lẽ, suite vẫn xanh.

## 4. Phần phải kiểm thì không nhỏ — và đây mới là chỗ đáng lo

**19 file `.rs` do CẢ HAI nhánh sửa**, và **18 trong số đó tự gộp không xung đột một dòng
nào**. Danh sách gồm trọn bề mặt tất định:

```
src-tauri/src/core/simulation_schedule.rs    <- nơi DUY NHẤT dựng schedule, và là file B1b thêm 5 cạnh thứ tự
src-tauri/src/core/simulation_loop.rs        <- a2c_loss, một objective duy nhất
src-tauri/src/core/agent_systems.rs          <- mortality, reproduction, các lần rút SimRng
src-tauri/src/core/world_systems.rs          <- spawn_food_system (rút 100 lần ở tick 1)
src-tauri/src/core/environmental_systems.rs  <- seed_dropping_system
src-tauri/src/core/ecs.rs                    <- manual_migration_system, hai Command rút qua &mut World (F-030)
src-tauri/src/core/live_experiment.rs        + tests/live_experiment_tests.rs
src-tauri/tests/common/allocator.rs          <- cổng đo cấp phát (F-031)
```

Một lần **tự gộp theo văn bản** của `simulation_schedule.rs` từ hai nhánh đều sửa nó cho ra
một schedule **biên dịch được, chạy được, và có thứ tự rút không thuộc bên nào**. Đó đúng là
dạng hỏng của [F-029], thứ đã tốn trọn một cuộc điều tra.

Và ba sự thật đã đo khiến nó **vô hình**:

1. Cổng bắt được chuyện này (`SIMRNG_SYSTEMS`) chỉ có ở **chồng PR** (3 lần xuất hiện), nhánh
   có **0** — nên sau gộp nó có mặt. Nhưng nó khẳng định `offenders.is_empty()`, tức *không
   còn cặp nào chưa có thứ tự*. Nó **không** ghim thứ tự **nào**. Một schedule có thứ tự đầy
   đủ nhưng **khác** vẫn qua cổng.
2. **Không test nào ghim checksum thành literal** — `git grep` trên toàn `src-tauri/tests`
   không ra. Đó là lý do đổi quỹ đạo ở B1b không vỡ baseline nào, và cũng là lý do một thay
   đổi thứ tự do gộp sẽ không làm đỏ một test nào trong 1216 test.
3. Checksum là thứ **mỗi process một kết quả** nếu còn cặp chưa thứ tự — đã đo 8 process ra
   4 thế giới trước B1b. Nên "chạy một lần thấy giống" không phải bằng chứng.

## 5. Vì vậy

Phần máy móc: 9 xung đột, 1 là code, 14 dòng. Một buổi.

Phần bằng chứng: **R8 áp dụng nguyên văn** — mọi thay đổi semantics mô phỏng, kể cả thứ tự
schedule, phải đo checksum tất định trước và sau, kèm phát biểu ảnh hưởng tới bằng chứng
thí nghiệm đã công bố. Cụ thể cho lần gộp này: chạy checksum 180 tick **nhiều process** trên
**cả hai cha** và trên **bản gộp**. Nếu checksum bản gộp không bằng cha nào, đó là **dự
kiến** (schedule đã đổi) nhưng phải được **ghi lại**, không phải phát hiện ra sau.

Khuyến nghị kỹ thuật, không phải quyết định: gộp theo chiều **chồng PR → nhánh**, thành một
PR **không chứa gì khác**, và lấy phép đo checksum làm bằng chứng của chính PR đó.

[F-029]: ../../../FINDINGS.md
[F-030]: ../../../FINDINGS.md
[F-031]: ../../../FINDINGS.md
[F-032]: ../../../FINDINGS.md
