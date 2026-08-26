---
kind: agent-goal
agent: unassigned
feature: living-world
title: Agent Goal — Thế giới sống (hình thái · animation · tính toán phân tán)
status: proposed
created: 2026-08-25
owner: project owner
tasks: ../implementation/2026-08-25-feature-living-world-tasks.md
requirements: ../requirements/2026-08-25-feature-living-world.md
testing: ../testing/2026-08-25-feature-living-world.md
---

# Agent Goal — Thế giới sống

Tài liệu này là **bản giao việc** cho agent triển khai. Nó không lặp lại thiết kế; nó nói agent
được phép làm gì, phải giữ gì, dừng lại khi nào, và chứng minh bằng lệnh nào.

---

## 1. Mục tiêu

Biến Anima-Engine từ "hình cầu chạy qua lại" thành một thế giới sống:

1. **Hình thái** — sinh vật có model 3D thật, hình dạng suy ra từ gene + môi trường, sinh bằng
   text-to-3D API theo kiến trúc **archetype** (một mesh phục vụ nhiều cá thể).
2. **Animation** — đi, chạy, bơi, ăn, uống, đánh, ngủ, nghỉ; mượt; đúng với hình thái tiến hoá.
3. **Tính toán phân tán** — nhiều máy cùng tick một thế giới; ngân sách agent theo phần cứng đo
   được; sinh vật xuất hiện/biến mất theo thiết bị **mà không mất một EU nào**.

---

## 2. Đọc bắt buộc, theo thứ tự

1. [`CLAUDE.md`](../../../CLAUDE.md) — lệnh, gotcha, quy tắc không đổi giữa các phiên.
2. [`docs/planning/STATE_OF_THE_PROJECT.md`](../../planning/STATE_OF_THE_PROJECT.md) §1 — **bảng số
   duy nhất có thẩm quyền**. Không chép số từ chỗ khác.
3. [Requirements — Thế giới sống](../requirements/2026-08-25-feature-living-world.md) — có bảng
   điều hướng bộ tài liệu.
4. [`CREATURE_PROMPT_CONTRACT.md`](../../reference/CREATURE_PROMPT_CONTRACT.md) — **MUST/MUST NOT**
   của prompt, lexicon, ngân sách ký tự.
5. [Task backlog](../implementation/2026-08-25-feature-living-world-tasks.md) — danh sách việc có
   neo file và DoD. **Đây là danh sách công việc.**
6. Theo nhánh đang làm:
   - A → [design genome-to-mesh](../design/2026-08-25-feature-genome-to-mesh.md), và
     [`CREATURE_DEVELOPMENT_CONTRACT.md`](../../reference/CREATURE_DEVELOPMENT_CONTRACT.md)
   - N → [design animation](../design/2026-08-25-feature-creature-animation.md)
   - F → [design federated compute](../design/2026-08-25-feature-federated-compute.md),
     [`ENERGY_LEDGER_CONTRACT.md`](../../reference/ENERGY_LEDGER_CONTRACT.md),
     [`DETERMINISM_CONTRACT.md`](../../reference/DETERMINISM_CONTRACT.md), ADR-0004

**Quy tắc thẩm quyền:** code hiện tại + test mới thắng văn bản. Mục nào đánh dấu `SUPERSEDED`,
`FIRST PASS` hay lịch sử là bằng chứng về quá khứ, không phải chỉ dẫn triển khai. `docs/archive/`
không bao giờ là kế hoạch. Đọc lại symbol trước khi sửa; số dòng trong tài liệu là hỗ trợ đọc, không
phải hợp đồng.

---

## 3. Mười một cái bẫy — mỗi cái sinh ra code chạy được và sai âm thầm

Đây là phần quan trọng nhất của tài liệu này. Từng mục đã thực sự xảy ra hoặc được ghi thành quy tắc
sau khi xảy ra.

1. ⚠️ **`src-tauri/anima-core/` KHÔNG nằm trong workspace.** `cargo metadata --no-deps` chỉ liệt kê
   `anima-engine` và `anima-domain`. `MorphologyGenotype` tồn tại ở **hai** nơi —
   `anima-core/src/genotype.rs:21` và `src-tauri/src/core/spawn.rs:21` — và **app biên dịch cái thứ
   hai**. Sửa cái thứ nhất sẽ compile sạch, test xanh, và không đổi gì trong ứng dụng.

2. **`cargo test` trần bỏ qua 7 test file.** Luôn dùng `--features desktop`. Bảy file mang
   `#![cfg(feature = "networking")]` hoặc `ml-wgpu` **và** một mục `[[test]] required-features`;
   thiếu feature thì Cargo không lên lịch chúng, không phải chạy rỗng. Từng che 1.877 dòng coverage.

3. **Không Lamarck.** Không bao giờ ghi môi trường vào genotype. Môi trường đi qua ba kênh riêng:
   genesis prior (phân bố genotype ban đầu), plasticity (genotype × env → phenotype, **một lần**),
   selection (ai sống sót). `AgentBrain.learned` không bao giờ được ghi ngược vào `.genotype`.

4. **D01 — Restore, Migration và `WakeAgentCommand` KHÔNG BAO GIỜ develop lại.** Chúng mang theo
   phenotype và brain đã có. Chỉ genesis và evolutionary replacement tạo mới.

5. **Không cấp phát heap trong tick path.** Test khẳng định `allocs == 0`. Dùng buffer cấp phát sẵn.

6. **Mọi thay đổi năng lượng phải nguyên tử với thay đổi vòng đời gây ra nó.** Ghi có ngăn rồi
   despawn qua `Commands` tạo cửa sổ để hệ thống chạy sau ghi có **lần hai**. Làm cả hai ở **một**
   sync point.

7. **Không bao giờ nâng `RESIDUAL_ABS_TOLERANCE_EU = 1e-3` để test xanh.** Residual vượt ngưỡng
   nghĩa là năng lượng đang chạy ngoài `EnergyLedger`; sửa đường đi, không nới ngưỡng.

8. **`src/types/generated/` do ts-rs sinh — không sửa tay.** Gate là
   `cargo test` rồi `git diff --exit-code -- src/types/generated`.

9. **Renderer không được sample biome để đổi màu/kích thước sinh vật.** Nếu nó tự tô theo ô đang
   đứng thì con vật đổi màu khi đi bộ, và ta mất bằng chứng thích nghi qua cửa sau.

10. **Mock dựng bằng `new` cần `function`, không phải arrow.** Dưới `@vitest/spy` 4,
    `vi.fn().mockImplementation(() => ({...}))` ném `TypeError: … is not a constructor`. Ảnh hưởng
    mọi `vi.mock('pixi.js', …)`.

11. **Vite 8 không ship esbuild.** Chạy script TypeScript bằng `node scripts/run_ts.mjs <file.ts>`.
    Giữ `build.minify: true`. Không hand-edit `map_manifest.json` — dùng `npm run gen:world-manifest`.

Thêm: lịch tick được dựng ở **đúng một chỗ**, `core::simulation_schedule::build_tick_schedule`. Một
test headless tự khai báo `.chain()` riêng là đang test một lịch mà app không chạy.

---

## 4. Hợp đồng an toàn với repository

- Cây làm việc hiện có file untracked (`src-tauri/anima-core/mutants.out/`,
  `src-tauri/mutants.out/`). **Không** dọn, không `git clean`, không `stash`.
- Không stage, commit, push, reset, restore, checkout, xoá hay đổi tên file **không liên quan**.
- Đọc lại từng file **ngay trước khi sửa**. Không ghi đè thay đổi của người dùng.
- Ghi lại số mục `git status --short` lúc bắt đầu và lúc kết thúc; liệt kê **chỉ** các file mục tiêu
  này cố ý đổi. Bất kỳ thay đổi nào ngoài danh sách cho phép là **điều kiện dừng**.
- Nhánh hiện tại là `hardening/phase0-determinism`. Tạo nhánh mới trước khi commit; không commit
  thẳng lên `main`.

---

## 5. File được phép chạm

**Nhánh A:** `src-tauri/src/core/simulation_state.rs`, `simulation_loop.rs`, `lib.rs` (commands),
`src-tauri/crates/anima-morpho/` (mới), `src/components/Landscape/LiveAgents.tsx`,
`src/PixiViewport.tsx`, `src/types/generated/` (chỉ qua ts-rs), `artifacts/creatures/` (mới),
`PROJECT.md`.

**Nhánh N:** `src-tauri/src/ai/cpg.rs`, `src-tauri/src/core/simulation_state.rs`,
`src/components/Landscape/` (LiveAgents + utils).

**Nhánh F:** `src-tauri/src/core/` (ledger, `components.rs`, `networking_systems.rs`,
`world_systems.rs`, `simulation_state.rs`), `scripts/test_target_policy.mjs`.

**Chung:** tài liệu dưới `docs/ai/`, `docs/decisions/` (ADR mới), `docs/planning/STATE_OF_THE_PROJECT.md`,
`CLAUDE.md`, test dưới `src-tauri/tests/` và `tests/frontend/`.

**Không chạm** trong phạm vi này: terrain/world generation, WorldArtifact, species clustering,
MAP-Elites, `BrainGenotype`/A2C, phiên bản bevy/burn, `src-tauri/anima-core/` (xem bẫy 1 và X8).

---

## 6. Vòng lặp xác minh

Chạy test tập trung sau mỗi milestone; kết thúc bằng output **mới** từ:

```bash
cd src-tauri && cargo test --features desktop --no-fail-fast 2>&1 | tee /tmp/anima-test.txt
```

```bash
cd src-tauri && cargo fmt --check && cargo clippy --all-targets --features desktop -- -D warnings
```

```bash
node scripts/check_test_targets.mjs /tmp/anima-test.txt --profile desktop
```

```bash
npm run lint && node scripts/eslint_ratchet.mjs && npm run test && npm run test:frontend && npm run build
```

```bash
cd src-tauri && cargo test --features desktop && cd .. && git diff --exit-code -- src/types/generated
```

Ghi chú thi hành:

- Máy này là **macOS/zsh**. Cảnh báo "dùng `cmd /c`" trong `CLAUDE.md` là dành cho PowerShell trên
  CI Windows; trên zsh thì `2>&1 | tee` giữ đúng thứ tự hai luồng.
- `eslint_ratchet.mjs` giữ baseline ở **0**. Một warning nghĩa là commit trước mặt vừa tạo ra nó.
  Sửa ở đó; baseline chỉ được hạ, và nó đã ở đáy.
- Không tuyên bố phần trăm coverage nếu không có công cụ coverage nào thực sự sinh ra con số đó.

---

## 7. Milestone

| M | Nội dung | Xong khi |
|---|---|---|
| **M0** | Khoá hợp đồng: ghi baseline `git status --short`, xác nhận 11 sự thật ở [task §0](../implementation/2026-08-25-feature-living-world-tasks.md), viết test **đỏ** trước | test đỏ tồn tại và đỏ vì đúng lý do |
| **M1** | A1 — payload chở kích thước | Gate A1 xanh; ảnh before/after |
| **M2** | N1 — nội suy tick↔frame | AN-S02 ở 30/60/144 Hz |
| **M3** | F1 — ngăn `dormant`, hibernate/wake | FD-S01…FD-S03 |
| **M4** | A2 — `MorphoSpec` + mesh procedural + bộ xương | GM-S01…GM-S05 |
| **M5** | N2 + N3 — CPG ghép nối, gait, overlay, `AgentCue` | AN-S01…AN-S08 |
| **M6** | F2 + F3 — probe, ngân sách, lockstep, T1–T5 | **FD-S05** `contribute_matches_solo` |
| **M7** | A3 + A4 — prompt, kho asset, API, nghiệm thu | PR-S01…PR-S10, GM-S06…GM-S08 |
| **M8** | Đối chiếu: cập nhật implementation/testing/`STATE_OF_THE_PROJECT`, ghi khoảng trống còn lại | mỗi số có lệnh + ngày + cấu hình |

M1, M2, M3 độc lập nhau — làm song song được. M3 nên bắt đầu sớm vì nó là mục **không rollback được**.

---

## 8. Điều kiện dừng

Dừng và báo cáo thay vì tự ứng biến, nếu:

- phải sửa `WorldLawSet`, schema manifest, hoặc `MorphologyEdge` của engine;
- một test hiện có phải bị làm yếu đi hoặc xoá đi mới xanh;
- không giữ được closed-EU, hoặc phải nâng bất kỳ tolerance nào;
- prompt không thể xuống dưới 1000 ký tự mà không bỏ slot bắt buộc — đây là lỗi lexicon, phải báo,
  không được cắt cụt;
- đường sạch nhất đòi hỏi đổi terrain/world generation, species, MAP-Elites hay `BrainGenotype`;
- xuất hiện thay đổi ngoài danh sách file cho phép ở §5;
- một gate chỉ xanh được bằng cách để asset AI hoặc dữ liệu từ contributor **ghi thẳng** vào
  hình học, năng lượng, fitness hay lineage;
- cần mở host ra internet công cộng trước khi F4-1/F4-2 xong.

---

## 9. Ngôn ngữ báo cáo trung thực

- "Đã xác minh headless" ≠ "đã chạy trong desktop app". Nói đúng cái đã chạy.
- Một gate xanh phải kèm **lệnh đã chạy + ngày + cấu hình**. Số không kèm ba thứ đó là lịch sử.
- Nếu một phần bị chặn: **làm xong mọi phần còn lại**, rồi nói rõ phần nào bỏ lại và vì sao. Thu
  hẹp phạm vi là quyết định của chủ dự án, không phải của agent.
- Không tuyên bố model 3D "đẹp" — không gate tự động nào chứng minh được; đó là việc của A4-8
  (duyệt bằng mắt, một lần cho mỗi archetype).
