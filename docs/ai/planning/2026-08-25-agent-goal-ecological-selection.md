---
phase: planning
feature: ecological-selection
title: Bản giao việc — Đóng vòng sinh thái
description: Hợp đồng cho agent triển khai, kèm bẫy đã biết, ranh giới file và điều kiện dừng
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
requirements: ../requirements/2026-08-25-feature-ecological-selection.md
design: ../design/2026-08-25-feature-ecological-selection.md
planning: 2026-08-25-feature-ecological-selection.md
implementation: ../implementation/2026-08-25-feature-ecological-selection.md
testing: ../testing/2026-08-25-feature-ecological-selection.md
prompt: 2026-08-25-handoff-prompt-ecological-selection.txt
---

# Bản giao việc — Đóng vòng sinh thái

Tài liệu này là **hợp đồng** giữa người giao việc và agent triển khai. Prompt gửi cho agent nằm ở
[`2026-08-25-handoff-prompt-ecological-selection.txt`](2026-08-25-handoff-prompt-ecological-selection.txt);
tài liệu này là bản đầy đủ mà prompt trỏ về.

---

## 1. Mục tiêu

Nối tầng sinh thái đã mô phỏng vào cơ chế chọn lọc, để môi trường quyết định được ai truyền gene.

Hôm nay không có đường nào từ sinh thái tới fitness: cả crate có **đúng một** định nghĩa fitness,
và nó là `cumulative_distance + tick_count`. Không cá thể nào chết đói. Hệ quả là mọi luật sinh
thái được viết rất cẩn thận đều là trang trí đối với kết quả tiến hoá, và mọi thí nghiệm về hành
vi chạy trên engine hôm nay đều đo phương sai của bộ tạo số ngẫu nhiên.

**Không** thiết kế lại sinh thái. **Không** đụng hợp đồng năng lượng. **Không** đổi hành vi mặc
định cho tới khi một cờ được bật tường minh.

---

## 2. Đọc bắt buộc, theo thứ tự

1. [`CLAUDE.md`](../../../CLAUDE.md) — luật không đổi giữa các phiên
2. [`docs/planning/STATE_OF_THE_PROJECT.md` §1](../../planning/STATE_OF_THE_PROJECT.md) — bảng số **duy nhất** có thẩm quyền
3. [Requirements](../requirements/2026-08-25-feature-ecological-selection.md) — đặc biệt bảng T1–T15 ở §2.2
4. [Design](../design/2026-08-25-feature-ecological-selection.md) — §0 (bốn quy tắc chung) là bắt buộc
5. [Task backlog](../implementation/2026-08-25-feature-ecological-selection.md) — danh sách việc
6. [Testing](../testing/2026-08-25-feature-ecological-selection.md) — **§0 trước tiên**: gate kênh nhân quả và đối chứng âm
7. [`ENERGY_LEDGER_CONTRACT.md`](../../reference/ENERGY_LEDGER_CONTRACT.md)
8. [`ADR-0003`](../../decisions/ADR-0003-evolved-per-agent-brains.md) — nếu chạm nhánh R

**Quy tắc thẩm quyền:** code hiện tại + test mới thắng văn bản. Mục ghi `SUPERSEDED` / `FIRST PASS`
là lịch sử. [`docs/archive/`](../../archive/README.md) không bao giờ là kế hoạch. Đọc lại symbol
trước khi sửa; số dòng chỉ để hỗ trợ đọc.

---

## 3. Mười cái bẫy — mỗi cái sinh ra code chạy được và sai âm thầm

1. **`src-tauri/anima-core/` KHÔNG nằm trong workspace.** Nó có `Cargo.lock` và `target/` riêng;
   `cargo metadata --no-deps` chỉ ra `anima-engine` và `anima-domain`. `MorphologyGenotype` tồn tại
   ở **hai** nơi. App biên dịch `src-tauri/src/`, **không** phải `anima-core`. Sửa nhầm chỗ sẽ
   compile sạch, test xanh, và không đổi gì trong ứng dụng.

2. **Luôn `--features desktop`.** `cargo test` trần không lên lịch 7 test file feature-gated.

3. **Mọi thay đổi năng lượng phải NGUYÊN TỬ với thay đổi vòng đời gây ra nó, tại MỘT sync point.**
   Đây là chế độ hỏng mà `ReclaimAndDespawnAgentCommand` đã sửa một lần, và nó sẽ quay lại nguyên
   vẹn nếu ai đó trừ năng lượng rồi despawn ở hai bước. **KHÔNG BAO GIỜ** nâng
   `RESIDUAL_ABS_TOLERANCE_EU = 1e-3` để test xanh.

4. **Đọc `AgentEvaluation`, đừng viết bộ đếm thứ hai.** `survival_ticks` đã có điều kiện sống gắn
   sẵn và đã được tính mỗi tick. Viết một bộ đếm song song là tạo hai nguồn sự thật, và chúng sẽ
   lệch nhau sau checkpoint đầu tiên.

5. **`LocomotionFitness` phải chép NGUYÊN VĂN dòng hiện tại.** Kể cả khi nó trông thừa. Đổi thứ tự
   hai số hạng `f32` có thể đổi làm tròn, và triệu chứng sẽ hiện ra ở một checksum cách đó rất xa.

6. **Thiếu resource ≠ tắt.** Theo đúng bẫy `ObserverPolicy` ở ADR-0004: thiếu nghĩa là *chưa ai
   khai báo*, và phải giữ hành vi cũ. Viết `Option<Res<…>>` và xử lý `None` tường minh.

7. **Lịch tick được dựng ở ĐÚNG MỘT CHỖ:** `core::simulation_schedule::build_tick_schedule`. Một
   test headless tự khai `.chain()` riêng là đang test một lịch mà app không chạy.

8. **`EntityHashMap` không duyệt theo thứ tự chèn.** Chỗ nào kết quả phụ thuộc thứ tự duyệt —
   điển hình là ai giành miếng ăn cuối trong một ô — phải sắp xếp tường minh. Nếu checksum đổi sau
   một thay đổi thuộc nhánh H, đó là **bug của H**, không phải hệ quả phụ chấp nhận được.

9. **Một cạnh lineage mồ côi phá TOÀN BỘ đồ thị.** `add_reproduction` từ chối cạnh có cha không
   tồn tại, và đó là hành vi **đúng** — cả `to_newick` lẫn `simplify` từ chối xử lý đồ thị chứa
   một cạnh như vậy. Đừng "sửa" thành ghi vô điều kiện. Ghi cạnh trong **cùng** command với việc
   sinh.

10. **Tick path không cấp phát heap.** `Local<…>` giữ bộ đệm qua các tick; `clear()` chứ không
    `new()`. Test khẳng định `allocs == 0` và nó sẽ bắt được.

**Bẫy thứ mười một, thuộc loại khác:** invariant `"starvation must not kill"` ở
`aggregate_population.rs:1653` nói về **quần thể tổng hợp của AE3**, một mô hình khác, có hợp đồng
khác. Feature này **không** chạm nó. Sửa nó là sửa nhầm mô hình.

---

## 4. Hợp đồng an toàn với repository

### 4.0 Hai điều kiện tiên quyết thuộc về chủ dự án — kiểm trước khi bắt đầu

**(a) `AGENTS.md` mâu thuẫn trực tiếp với nhiệm vụ này.** Nó khai `src-tauri/src` là **CHỈ ĐỌC**
và `src-tauri/tests` là vùng ghi **duy nhất**, kèm câu *"Vi phạm = task fail, không ngoại lệ"*.
Hợp đồng đó được viết cho agent săn mutant — nhưng nó phát biểu ở phạm vi toàn repo, và feature
này bắt buộc phải ghi vào `src-tauri/src`.

Đây là **quyết định của chủ dự án**, không phải việc kỹ thuật.

> ### ✅ MIỄN TRỪ ĐÃ ĐƯỢC CẤP — 2026-08-25
>
> Chủ dự án (`duonganhdn2000@gmail.com`) miễn trừ `AGENTS.md` cho **đúng task này**, tường minh,
> trong phiên giao việc ngày 2026-08-25. Agent triển khai **được phép ghi vào `src-tauri/src`**
> trong phạm vi danh sách file ở [§5](#5-file-được-phép-chạm).
>
> **Miễn trừ này hẹp và không lan ra:**
>
> - chỉ áp cho feature `ecological-selection`, không cho task khác;
> - **không** mở rộng sang `scripts/`, `justfile`, `AGENTS.md`, `.hardening.env` hay file config
>   build — bốn mục đó vẫn cấm tuyệt đối;
> - **không** mở rộng sang `src-tauri/anima-core/` (xem bẫy 1);
> - mọi cấm khác của `AGENTS.md` **vẫn còn hiệu lực**, đặc biệt: không thêm `#[ignore]` /
>   `#[should_panic]` để né lỗi, không xoá hay nới lỏng assertion đã có, không dùng nguồn phi tất
>   định (`thread_rng`, `SystemTime::now`, `Instant::now`) trong test, và không có test nào chỉ
>   khẳng định "không lỗi".
>
> Nếu nhiệm vụ của bạn **không** phải feature này, miễn trừ trên không nói gì về bạn: đọc
> `AGENTS.md` như bình thường.

**(b) Một cổng xác minh đang đỏ từ trước — [FINDINGS F-017](../../../FINDINGS.md).**
[`scripts/check_test_targets.mjs`](../../../scripts/check_test_targets.mjs) đã được **commit với
ba khối merge conflict còn nguyên**; chạy nó cho `SyntaxError`, và `npm run lint` báo
`Merge conflict marker encountered` cho cùng file, cộng 9 lỗi eslint tồn đọng ở `playground/` và
`tests/frontend/` — nên **baseline ratchet 0 không phản ánh cây mã hiện tại**.

Ghi nó vào baseline như một cổng đỏ có sẵn rồi đi tiếp. **Không** sửa `scripts/`: `AGENTS.md` cấm,
và F-017 đang chờ chủ dự án. Không báo cáo nó như hồi quy của mình, và không dùng nó làm lý do bỏ
qua các cổng khác.

### 4.1 Ranh giới thao tác


- Cây làm việc hiện có file untracked (`src-tauri/mutants.out/`, `src-tauri/anima-core/mutants.out/`,
  `src-tauri/mutants.out.old/`). **Không** dọn, không `git clean`, không `stash`.
- Không stage, commit, push, reset, restore, checkout, xoá hay đổi tên file **không liên quan**.
- Đọc lại từng file **ngay trước khi sửa**. Không ghi đè thay đổi của người dùng.
- Ghi số mục `git status --short` lúc bắt đầu và lúc kết thúc; liệt kê **chỉ** các file mục tiêu
  này cố ý đổi. Bất kỳ thay đổi nào ngoài danh sách §5 là **điều kiện dừng**.
- Nhánh hiện tại là `feat/living-world`. Tạo nhánh mới trước khi commit; không commit thẳng lên
  `main`.

---

## 5. File được phép chạm

**Nhánh M:** `src-tauri/src/ai/hrrl.rs`, `src-tauri/src/core/world_systems.rs`,
`src-tauri/src/core/resources.rs`, `src-tauri/src/core/mortality.rs` (mới),
`src-tauri/src/core/simulation_schedule.rs`, `src-tauri/src/core/mod.rs`.

**Nhánh F:** `src-tauri/src/evolution/fitness.rs` (mới), `src-tauri/src/evolution/mod.rs`,
`src-tauri/src/core/agent_systems.rs`, `src-tauri/src/core/resources.rs`,
`src-tauri/src/core/experiment.rs`, `src-tauri/src/core/tick_capture.rs`.

**Nhánh Q:** `src-tauri/src/core/ecology.rs`, `src-tauri/src/core/components.rs`,
`src-tauri/src/evolution/map_elites.rs`.

**Nhánh R:** `src-tauri/src/core/agent_systems.rs`, `src-tauri/src/core/resources.rs`,
`src-tauri/src/evolution/lineage.rs`, `src-tauri/src/core/experiment.rs`.

**Nhánh O:** `src-tauri/src/core/live_experiment.rs`, `src-tauri/src/core/simulation_loop.rs`,
`src-tauri/src/evolution/map_elites.rs`, `src-tauri/src/core/experiment.rs`.

**Nhánh H:** `src-tauri/src/core/world_systems.rs`, `src-tauri/src/core/agent_systems.rs`,
`src-tauri/src/core/environmental_systems.rs`.

**Chung:** test dưới `src-tauri/tests/`, tài liệu dưới `docs/ai/`,
`docs/planning/STATE_OF_THE_PROJECT.md`, `FINDINGS.md`, `scripts/test_target_policy.mjs`,
`src/types/generated/` (**chỉ** qua `ts-rs`, không sửa tay).

**Không chạm trong phạm vi này:** `src-tauri/anima-core/` (bẫy 1), terrain / world generation,
`WorldArtifact`, `BrainGenotype` / A2C, `exotic_energy` / AE3, `aggregate_population.rs`,
phiên bản `bevy` / `burn`, toàn bộ `src/components/Landscape/`.

---

## 6. Vòng lặp xác minh

Trước khi viết dòng code nào, chạy **toàn bộ** khối dưới và ghi kết quả làm baseline. Không có
baseline thì không phân biệt được "mình làm hỏng" với "nó vốn đã đỏ".

```bash
cd src-tauri && cargo test --features desktop --no-fail-fast 2>&1 | tee /tmp/anima-test.txt
cd src-tauri && cargo fmt --check
cd src-tauri && cargo clippy --all-targets --features desktop -- -D warnings
cd src-tauri && cargo clippy --all-targets --no-default-features -- -D warnings
node scripts/check_test_targets.mjs /tmp/anima-test.txt --profile desktop
npm run lint && node scripts/eslint_ratchet.mjs
npm run test && npm run test:frontend && npm run build
cd src-tauri && cargo test --features desktop && cd .. && git diff --exit-code -- src/types/generated
node scripts/check_docs_links.mjs
```

Máy này là macOS/zsh. Cảnh báo `cmd /c` trong CLAUDE.md là dành cho PowerShell trên CI Windows;
trên máy này `tee` giữ đúng thứ tự hai luồng.

---

## 7. Milestone

| # | Nội dung | Điều kiện vào | Xong khi |
|---|---|---|---|
| **E0** | Khoá hợp đồng | — | 13 sự thật ở §0 task backlog được xác nhận lại; baseline §6 đã ghi; test **ĐỎ** đã viết trước |
| **E1** | M — tử vong | E0 | MO-S01…MO-S06 + CA-S02 xanh, kèm đối chứng âm |
| **E2** | H — hiệu năng | E0 (song song với E1) | PF-S01, PF-S02, PF-S04 xanh; PF-S03 dừng chờ người |
| **E3** | F — fitness | E1 | FI-S01…FI-S05 + CA-S01 xanh, kèm đối chứng âm |
| **E4** | Q — trục mô tả | E3 | QD-S01…QD-S05 xanh |
| **E5** | O — quan sát | E1, E4 | OB-S01…OB-S05 + CA-S03 xanh, kèm đối chứng âm |
| **E6** | R — sinh sản | E3 xanh | RE-S01…RE-S06 xanh |
| **E7** | Đối chiếu tài liệu | E5 | STATE_OF_THE_PROJECT cập nhật; FINDINGS ghi kết quả; task backlog đánh dấu |

**Mốc đáng nhắm nhất là sau E5.** Nó đóng đúng vòng nhân quả và không mang rủi ro phân kỳ quần thể
của E6. Dừng ở đó vẫn là một trạng thái tốt và đầy đủ.

---

## 8. Điều kiện dừng — báo cáo, không tự ứng biến

Dừng ngay và báo cáo nếu:

- không giữ được closed-EU, hoặc phải nâng `RESIDUAL_ABS_TOLERANCE_EU`;
- phải làm yếu, sửa hoặc xoá một test hiện có để gate xanh;
- checksum của một run có seed đổi khi mọi cờ mới đều tắt (vi phạm N3);
- checksum đổi sau một thay đổi thuộc nhánh H (H là refactor thuần);
- một gate kênh nhân quả (CA-S01…CA-S03) xanh trên **cả** bản mới lẫn đối chứng âm — nghĩa là phép
  đo đang bắt một thứ khác;
- phải đổi `WorldLawSet`, schema manifest, hoặc `MorphologyEdge`;
- xuất hiện thay đổi ngoài danh sách file ở §5;
- quần thể tuyệt chủng ở tham số mặc định, hoặc phân kỳ không trần.

---

## 9. Ngôn ngữ báo cáo trung thực

Sau mỗi milestone, báo cáo phải có:

- **Danh sách file đã đổi** — đầy đủ, không rút gọn.
- **Output MỚI** của các lệnh xác minh, kèm ngày và cấu hình. Không mang số cũ sang.
- Số mục `git status --short` lúc bắt đầu và lúc kết thúc.
- Gate nào **xanh**, gate nào **chưa chạy**, gate nào **đỏ** — nói thẳng, không làm tròn.
- Với mỗi gate kênh nhân quả: **cả hai** kết quả, bản mới và đối chứng âm. Một mình bản mới là báo
  cáo chưa đủ.
- "Đã xác minh headless" **≠** "đã chạy trong desktop app". Nói đúng cái đã chạy.
- Nếu một phần bị chặn: làm xong **mọi** phần còn lại, rồi nói rõ phần nào bỏ lại và vì sao.
