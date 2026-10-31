---
title: Kế hoạch thực thi chi tiết — từ hai cây đỏ về một cây xanh
status: proposed
owner: maintainers
last_reviewed: 2026-10-31
review_cycle: weekly
---

# Kế hoạch thực thi chi tiết

> **Chẩn đoán và số đo nền cho kế hoạch này ở
> [`COMPLETION_ROADMAP_2026-10-31.md`](COMPLETION_ROADMAP_2026-10-31.md).** Tài liệu đó trả lời
> *"đang ở đâu"*; tài liệu này trả lời *"làm gì, theo thứ tự nào, và biết xong bằng lệnh nào"*.
> Giai đoạn khoa học (HT2–HT5) **không** được viết lại ở đây — nó vẫn ở
> `COMPLETION_ROADMAP_2026-10-16.md` trên `origin/main`.
>
> Mỗi task có **một lệnh xác minh**. Một task chỉ được gọi là xong khi lệnh đó exit 0 **và** output
> thô của nó được dán vào PR. Đây không phải hình thức: §1.2 của roadmap 16/10 ghi sáu lần một gate
> "exit 0 và trông như xanh" đã che một lỗi thật.

---

## 0. Hình dạng của kế hoạch

Có **hai** cây cần xanh, và chúng xanh theo thứ tự khác nhau:

```
HT-A  nhánh feat/ecological-selection  ──┐
      (gần xanh: cargo test 1153/0)      │
                                         ├──> HT-C  hợp nhất ──> HT-D khoa học
HT-B  nhánh mới cắt từ origin/main    ──┘      (HT2 của roadmap 16/10)
      (đỏ 10 gate, gồm cargo test)
```

**HT-A và HT-B độc lập** — làm song song được, bởi hai người hoặc hai phiên agent khác nhau, trên
hai worktree khác nhau. **HT-C chặn trên cả hai.** HT-D chặn trên HT-C.

Ước lượng là **mục tiêu**, không phải cam kết: HT-A 2–4 ngày · HT-B 4–8 ngày · HT-C 3–6 ngày.

---

## HT-A — Làm nhánh `feat/ecological-selection` xanh

Cây này đã có thứ khó nhất: **1153 test backend xanh, 0 fail, và hai gate tái lập xanh**. Việc còn
lại là vệ sinh, cộng đúng một quyết định phạm vi.

| ID | Việc | File / điểm neo | Lệnh xác minh | Ghi chú |
|---|---|---|---|---|
| **A1** | **Gỡ ba khối conflict marker** đã commit trong `scripts/check_test_targets.mjs` (`:78`, `:108`, `:155`). **Giữ cả hai nửa** của gate: nửa `ours` là allow-list chính sách, nửa `theirs` là phép kiểm "một target đã khởi chạy có in số test hay không". Bản hợp nhất đúng **đã tồn tại** trên `origin/main` tại `286e45a` — port nó, đừng tự viết lại | `scripts/check_test_targets.mjs` | `node scripts/check_test_targets.mjs <capture> --profile desktop` exit 0 | Đóng **F-017**, mở từ 2026-08. Trong lúc nó chết, mọi con số "bao nhiêu target" đều không được kiểm |
| **A2** | **`cargo fmt`** — 67 khối lệch, gần như toàn bộ trong các file test do `c0344c9` thêm | `src-tauri/tests/**` | `cargo fmt --check` exit 0 | Chỉ chạy `cargo fmt`; không sửa tay |
| **A3** | **Quyết định `playground/`** ([QĐ-D](COMPLETION_ROADMAP_2026-10-31.md)) rồi thực hiện. Hai nhánh, chọn một: **(a)** `playground/fantasy-map/`, `playground/frontend/` là mã thật ⇒ bỏ `/playground/` khỏi `.gitignore:73`, `git add` ba module, khai `echarts-for-react` trong `package.json`, sửa 7 lỗi ESLint ở đó. **(b)** là nháp ⇒ `git rm` bốn file test đang kiểm nó và loại `playground/` khỏi phạm vi ESLint | `.gitignore:73`, `package.json`, `tests/frontend/{fantasy_map,fantasy_map_stress,map_viewport_stress}.test.tsx`, `src/__tests__/MockDataGenerator.test.ts`, `eslint.config.js` | `npm run lint` **và** `npm run test:frontend` **và** `npm run typecheck:tests`, cả ba exit 0 | **Không** chọn được bằng cách đọc mã — xem §1.e của roadmap 31/10. Cấm tuyệt đối: `npm i echarts-for-react` rồi để mã vẫn bị gitignore |
| **A4** | **ESLint ratchet về 0**: 8 error + 72 warning. Sau A3, phần `playground/` biến mất hoặc được sửa; phần còn lại nằm ở `tests/frontend/*` (biến không dùng, `any`) | `tests/frontend/**`, `playground/**` (tuỳ A3) | `npm run lint && node scripts/eslint_ratchet.mjs` exit 0 | Baseline là **0** và chỉ được hạ, không được nâng |
| ~~**A5**~~ | ✅ **XONG 2026-10-31.** Và chẩn đoán ban đầu ở ô này **sai**: nó gọi hai lỗi `Mock<[], void>` vs `Mock<() => void, any>` ở `phase3_adversarial.test.tsx:153,158` là "lỗi kiểu thật". Chúng là **triệu chứng** của một install cũ: `tests/package-lock.json` khai **vitest 4.1.10**, `tests/node_modules` ở máy này còn **1.6.1**, nên `tsc -p tests/tsconfig.json` đọc type v1 trong khi runner chạy v4. Sửa 14 annotation cho khớp v1 sẽ làm chúng **sai so với runner**. `npm ci --prefix tests` — đúng việc CI làm ở `ci.yml:229` — đóng **18/19** lỗi, không một dòng mã nào bị sửa | `tests/mocks/segment_fixtures.ts` | `npm run typecheck:tests` exit 0 | Lỗi thật duy nhất là `segment_fixtures.ts:29` thiếu `length`/`radius` — `SegmentState` có thêm hai trường phenotype (A1) mà fixture chưa theo |
| ~~**A6**~~ | ✅ **XONG 2026-10-31.** `cargo audit` exit 0. **Nâng (3 crate, 6 advisory):** `rustls` 0.23.42→0.23.46 (port `0d224ec`, 4 advisory), `rustls-webpki` →0.103.16 (`0d224ec` chỉ tới 0.103.15 vì RUSTSEC-2026-0382 công bố sau nó 4 ngày), `webpki-roots` 1.0.9→1.0.11. **Miễn trừ có bằng chứng (5 advisory):** `zip` ×3, `zstd-safe`, `zstd-sys` — vào lockfile chỉ qua `burn-candle`/`burn-tch` không được bật. ⚠️ **Ô này trước đây ghi 6 miễn trừ và sai một cái:** `webpki-roots` được liệt là lockfile-only trên bằng chứng `cargo tree -i webpki-roots … -e all` không in gì — nó không in gì vì **exit non-zero** (`specification … is ambiguous`, hai phiên bản trong lock). Nó **có** ship, qua `webpki-roots 0.26.11` (bridge semver) ← `neo4rs`/`ureq`. Miễn trừ nó sẽ là che một lỗ hổng đang phân phối. Phép kiểm đúng không dùng `-i`: `cargo tree -e all --features desktop --target all \| grep -c " <crate> v"` — cho **0** với zip/zstd và **2** với webpki-roots | `src-tauri/Cargo.lock`, `src-tauri/.cargo/audit.toml` | `cargo audit` exit 0 · `cargo test --features desktop` **1153 passed · 0 failed** (xác minh độc lập) · gate tách feature không rò | Bài học: **một lệnh fail và không in gì không phải một kết quả âm.** Luôn xem exit code |

**Đã làm xong trong phiên 2026-10-31** (có trong cây làm việc, chưa commit):

| | Việc | Bằng chứng |
|---|---|---|
| ✅ | **Mock `pixi.js` arrow → `function`** ở `tests/frontend/map_viewport_stress.test.tsx` — đúng cái bẫy `CLAUDE.md` đã ghi; thêm cả `Sprite` mà factory thiếu | `npx vitest run --root tests frontend/map_viewport_stress.test.tsx` → **8 passed** |
| ✅ | **Template `gen_notice.mjs`** — port bản sửa relicense từ `origin/main` (`363fd0e`); bản trên nhánh còn gọi dự án là *proprietary* ở 4 chỗ, nên regenerate sẽ đưa chữ đó trở lại | `diff` với `origin/main:scripts/gen_notice.mjs` |
| ✅ | **Dọn tài liệu** — xoá 5 file + `projects/` (36 file, gồm `.pyc`) của một lượt agent **không liên quan** (palindrome checker); `plan.md`/`handoff.md` → `docs/archive/`; `ORIGINAL_REQUEST`/`TEST_INFRA`/`TEST_READY`/`AI_Context` → `docs/ai/`; sửa 7 link gãy | `node scripts/check_docs_links.mjs` → **762 link, 0 gãy** (trước: 7 gãy) |

**Cố ý KHÔNG làm**: regenerate `NOTICE`. `scripts/lib/licensing.mjs:27` nói rõ artifact licence
được sinh cho **host target**, mục tiêu của repo là **Windows**, và CI kiểm `check:notice` trên
`windows-latest`. Sinh lại trên macOS cho **398** crate thay vì **419** — 5 test local xanh, gate CI
đỏ. Đây là task **HT-A7, cần máy Windows**, không phải việc làm ở đây.

---

## HT-B — Làm `origin/main` xanh

Cắt một nhánh mới từ `origin/main`. **Không** mở tính năng nào ở giai đoạn này.

| ID | Việc | Lệnh xác minh | Phụ thuộc |
|---|---|---|---|
| **B1** | **Hai gate tái lập.** `a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards` và `run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run`. Nghi phạm đã xác định: `src-tauri/src/core/agent_systems.rs:411` sort `to_despawn` theo `lineage_id`, **không** là khoá duy nhất giữa anh chị em, bằng `sort_unstable_by` (không giữ thứ tự phần tử bằng nhau). **Bisect theo cụm trước khi sửa** — đừng tin nghi phạm này mà không có một lượt bisect | `cargo test --features desktop -p anima-engine --test live_experiment_tests` → `17 passed; 0 failed` | — |
| **B2** | **Cấp phát trong tick path.** Cùng chỗ: `:400` `Vec::new()` mỗi tick, `:408` `lineage_id.0.clone()` mỗi cá thể chết. Vi phạm hợp đồng zero-alloc của `CLAUDE.md`, và **gate alloc hiện tại không bắt** — nên task này gồm cả việc **mở rộng gate** để nó bắt được | gate alloc hiện có, mở rộng sang `agent_death_system`, exit 0 | B1 |
| **B3** | **Gate tách feature đỏ** — "Default build excludes Neo4j, the WebSocket stack and Burn/WGPU". Có một `use` vô điều kiện mới do khối 22/10 thêm | `cargo tree --no-default-features -e normal` → 0 kết quả cho `neo4rs\|tokio-tungstenite\|burn-wgpu\|naga` | — |
| **B4** | **Phần đỏ còn lại của CI**: `cargo fmt`, 12 lint clippy (`is_multiple_of`, `needless_range_loop` ×4, `derivable_impls`, `clone_on_copy`…), `cargo deny`, `cargo audit` (5 vuln), hai `npm audit`, ratchet ESLint | `.github/workflows/ci.yml` xanh **cả hai job** | — |
| **B5** | **Hoàn nguyên việc thu hẹp `test:e2e`.** `tests/package.json:8` nay là `playwright test e2e/tiers/`; `ipc_contract`, `console_hygiene`, canonical views, phase3/phase6 **không còn chạy**. Thêm gate danh sách spec: danh sách được commit, so khớp, bỏ một spec ⇒ đỏ và nêu tên | bỏ một spec khỏi lệnh ⇒ gate đỏ **và** nêu tên spec | B4 |
| **B6** | **Ba khoản bịa/chết ở §1.d của roadmap 31/10**: (a) giá trị khoa học bịa — `telemetry_export.rs:264-265` tỉ lệ 70/30, `:271` `biomass: 10000.0`, `commands/simulation.rs` `RunProvenance::new(42, …)`; (b) hai lệnh IPC `dispatch_causal_intervention` / `get_causal_branch_tree` được frontend gọi mà **không tồn tại** bên Rust; (c) bypass sổ cái ở `agent_systems.rs:522` và đường `PathogenTransmission` ở `live_experiment.rs` + `environmental_systems.rs:177` | mỗi khoản: **một test đỏ trước**, xanh sau. Với (c): một test sổ cái đóng bắn `PathogenTransmission` rồi đo residual EU ≤ `1e-3` | B1 |

---

## HT-C — Hợp nhất

| ID | Việc | Định nghĩa xong |
|---|---|---|
| **C1** | **Quyết định nền** ([QĐ-A](COMPLETION_ROADMAP_2026-10-31.md)). Khuyến nghị: `main` là nền; bốn file của nhánh (`core/mortality.rs`, `core/rng.rs`, `core/spawn.rs`, `evolution/fitness.rs`) **thay** phần demographics của khối 22/10, không nằm cạnh nó | Một dòng quyết định ghi vào roadmap 31/10 |
| **C2** | **`evolution/fitness.rs` + `FitnessRuleSetting`** vào `main`. Đây là **F-018** — gốc của kết quả null ở E2. Tắt cờ ⇒ checksum trùng bit với trước khi merge | checksum tất định **trùng bit** khi cờ tắt; một test cho thấy sinh thái đổi ⇒ fitness đổi khi cờ bật |
| **C3** | **`core/mortality.rs` + `MortalityPolicy`** vào `main`, thay `agent_death_system`. Đo hai gate tái lập **trước và sau** | `live_experiment_tests` `17 passed`; residual EU ≤ `1e-3` ở **mọi** cấu hình cờ |
| **C4** | **Bộ test nhánh O / Q / H** (observables, trục MAP-Elites thứ hai, O(agent×segment) → O(segment)) | `check_test_targets.mjs` sống (A1) rồi mới tin số target; mọi gate của STATE §5 xanh tại **một** commit |
| **C5** | **Đo lại toàn bộ** và chuyển số vào `STATE_OF_THE_PROJECT.md` **§1.g**; xoá bảng §1 của roadmap 31/10 và trỏ về §1.g | STATE §1 có một lượt đo đầy đủ, mọi hàng cùng một commit |

---

## HT-D — Khoa học

Chỉ mở sau C5. Dùng **HT2 của `COMPLETION_ROADMAP_2026-10-16.md`** nguyên văn (HT2.1 quần thể luân
chuyển → HT2.2 E3 → HT2.3 G2 gate #1 → …). Một ghi chú đã đổi: **HT2.1 rẻ hơn nhiều sau C2 + C3**,
vì hai task đó cung cấp đúng hai mảnh nó cần — một fitness mà sinh thái chạm được, và một cái chết
tất định. Chạy E3 **trước** C2/C3 là lặp lại E2: đo phương sai của bộ sinh số ngẫu nhiên.

---

## Việc chỉ con người làm được

| | Việc | Vì sao |
|---|---|---|
| H1 | **HT-A7** — regenerate `NOTICE` + `licensing/third-party-index.json` + SBOM trên **Windows** | Artifact licence là theo host target; repo nhắm Windows, CI kiểm trên `windows-latest` |
| H2 | Chạy app desktop **bản release** với `ANIMA_TICK_CAPTURE` ở 1.000 agent | `CLAUDE.md` cấm chạy full backend trên máy dev; cần một người bấm chạy |
| H3 | Xác minh app khởi động dưới CSP mới | `npm run check:csp` chỉ kiểm artifact, không chứng minh app boot |
| H4 | **QĐ-A…QĐ-D** ở §4 roadmap 31/10, và **QĐ-B** (phạm vi `AGENTS.md`) trước khi bất kỳ agent nào ghi vào `src-tauri/src` | Quyết định phạm vi và hợp đồng, không phải việc kỹ thuật |
