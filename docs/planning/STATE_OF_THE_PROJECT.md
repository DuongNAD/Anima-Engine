---
title: Trạng thái dự án và việc cần làm tiếp
status: active
owner: maintainers
last_reviewed: 2026-07-27
review_cycle: per-release
---

# Trạng thái dự án và việc cần làm tiếp

Tài liệu **sống**. Đây là chỗ một phiên mới đọc đầu tiên để biết dự án đang ở đâu và
làm gì kế tiếp. Nó không định nghĩa lại hợp đồng nào — hợp đồng ở
[`docs/reference/`](../reference/README.md) và các ADR. Nó chỉ trả lời hai câu:
**điều gì đã đo được là đúng**, và **việc gì đáng làm tiếp, theo thứ tự nào**.

Quy tắc cập nhật: khi một mục ở §3 xong, chuyển nó lên §2 kèm số đo, đổi
`last_reviewed`, và ghi một dòng vào [`TODO.md`](../../TODO.md). Đừng đánh dấu xong khi
gate chưa xanh.

---

## 1. Trạng thái đo được (**2026-07-27**, nhánh `feature-anima-completion` trên `6caeeb4`)

Đo lại toàn bộ trong worktree `.worktrees/feature-anima-completion`. Bằng chứng đầy đủ kèm exit code
ở [`docs/ai/testing/2026-07-27-feature-anima-completion.md`](../ai/testing/2026-07-27-feature-anima-completion.md).

> **Đo lại 2026-07-27 (đợt ba, gói live-adapter + tick capture, trên `b6a579e`).** Bảy hàng đầu là
> số chạy lại trong đợt này; phần còn lại giữ nguyên từ đợt trước vì gói này không chạm vào chúng
> (không có thay đổi nào trong `src/`, `dist/`, lockfile hay danh sách phụ thuộc — chỉ thêm file
> kiểu do ts-rs sinh trong `src/types/generated/`).

| Gate | Lệnh | Kết quả |
|---|---|---|
| Backend test (desktop) | `cargo test --features desktop --no-fail-fast` | **851 pass · 0 fail · 2 ignored**, exit 0 |
| Backend test (mặc định) | `cargo test --no-default-features --no-fail-fast` | **833 pass · 0 fail · 2 ignored**, exit 0 |
| Chính sách target/ignore | `node scripts/check_test_targets.mjs <capture> --profile {default,desktop}` | exit 0 cả hai. desktop: 82 target, 3 rỗng (đủ 3 trong allow-list), 2 ignore (đủ 2), **7 target feature-gated chạy**. default: 75 target, 3 rỗng, 2 ignore, **0 feature-gated được lên lịch** |
| Format + clippy (cả 2 cấu hình) | `cargo fmt --check`, `cargo clippy --all-targets {--features desktop, --no-default-features} -- -D warnings` | sạch |
| Test frontend (src) | `npm run test` | 14 file · **109 pass** |
| Test frontend (tests/) | `npm run test:frontend -- --maxWorkers=4` | **38 file · 432 pass · 0 skip** — xem §4 |
| Lint + ratchet + typecheck `tests/` + build | `npm run lint`, `node scripts/eslint_ratchet.mjs`, `npm run typecheck:tests`, `npm run build` | 0 error · 0 warning (baseline 0) · 0 error · pass |
| E2E Playwright | `npm run test:e2e` | **9 pass · 0 fail · 5 skip có lý do**, server riêng cổng 5177 + kiểm định danh |
| CSP | `npm run check:csp` | 2 file HTML ship, 0 origin ngoài, 0 inline script |
| Ngân sách bundle | `npm run check:bundle` | 23 chunk, **1711,3 / 2000 KiB** (đo lại 2026-07-27 đợt ba; gói này **không** đổi file nào trong `src/` ngoài kiểu do ts-rs sinh, mà kiểu thì bị xoá khi biên dịch — chênh so với 1695,8 của đợt trước không quy được cho nó) |
| NOTICE | `npm run check:notice` | 419 crate + **21 gói npm được phân phối** + 18 gói cài-nhưng-không-ship |
| Văn bản license bên thứ ba | `npm run check:licenses` | 440 thành phần phân phối · **266 văn bản khác nhau** · **1 chưa có văn bản** (408 đọc từ artifact + 31 vendor từ commit upstream đã ghim) |
| Kho license upstream đã vendor | `npm run verify:upstream-licenses` (cần mạng, chạy tay) | **39 file · 24 commit · 19 repository**, khớp byte-cho-byte với URL đã ghim |
| SBOM | `npm run check:sbom` | **458 thành phần**, 459 bản ghi dependency, CycloneDX 1.5 |
| SBOM đúng schema | `npm run check:sbom-schema` | hợp lệ với schema chính thức, ghim ở commit `c320fc0f0b46` |
| Ranh giới bundle npm | `npm run check:bundle-closure` | 21 gói có byte trong `dist/` (3 do toolchain nhúng) |
| Byte điều khiển trong source | `npm run check:text-hygiene` | 525 file, **0** byte điều khiển thô |
| Link tài liệu | `node scripts/check_docs_links.mjs` | 0 gãy |

**Cái đã đổi so với 2026-07-26, và đáng đọc nhất:** hàng `tests/` **không phải hồi quy**. 28 lỗi là
thật nhưng do **tranh chấp CPU**, và đã đo được cơ chế: chạy riêng một file → 4/4 pass; chạy cả suite
với `--maxWorkers=4` → 243 pass, trong khi một tiến trình Vitest của dự án khác vẫn đang chiếm một
core và 1,4 GB. Không assertion nào bị nới.

<details>
<summary>Bảng cũ (2026-07-26, tại <code>d006f64</code>) — giữ để đối chiếu</summary>

Đây là số đo, không phải trích dẫn tài liệu. Mọi hàng **trừ một** được chạy lại tại `d006f64`
(nhánh `feat/oss-071b-live-tracker`, trên `main` sau khi #19 merge); hàng ngoại lệ được đánh dấu
trong bảng.

| Gate | Lệnh | Kết quả |
|---|---|---|
| Backend test | `cargo test --features desktop --no-fail-fast` | **746 pass · 0 fail · 4 ignored**, 75 test binary, 0 warning biên dịch |
| Target rỗng | `node scripts/check_test_targets.mjs <output>` | 75 target, **0 target chạy rỗng** |
| Format | `cargo fmt --check` | sạch |
| Lint backend | `cargo clippy --all-targets --features desktop -- -D warnings` | sạch |
| Lint backend (default) | `cargo clippy --all-targets --no-default-features -- -D warnings` | sạch |
| Test frontend (src) | `npm run test` | 14 file · **109 pass**, 0 skip — đo trên `feature-anima-completion`, 2026-07-27 |
| Test frontend (tests/) | `npm run test:frontend` | 36 file · **339 pass**, 0 skip — đo trên `feature-anima-completion`, 2026-07-27 (máy rảnh) |
| Lint frontend | `npm run lint` + `node scripts/eslint_ratchet.mjs` | **0 error, 0 warning** (baseline **0**) — đo lại trên `feature-anima-completion`, 2026-07-27 |
| Typecheck `tests/` | `npm run typecheck:tests` | **0 error** — gate mới, xem §3.18 |
| Build | `npm run build` | pass |
| Link tài liệu | `node scripts/check_docs_links.mjs` | 417 link trong 90 file, **0 gãy** |

> ⚠️ **Suite `tests/` đã đỏ giả trong lần đo tại `d006f64`, và bản chạy lại chưa hoàn thành** —
> máy liên tục bận vì phiên khác. Nó tái hiện **đúng chữ ký** ghi ở §4: **28 lỗi**, thời gian tường
> 45,25s (so với ~19,5s lúc rảnh), và `Get-Process cargo` lúc đó cho thấy **4 tiến trình build của
> phiên khác** đang chạy.
>
> **Việc đầu tiên của phiên sau, nếu cần một bảng §1 sạch:** chạy lại `npm run test:frontend` **một
> mình** trên máy rảnh và điền số vào hàng đó. Con số **28** là dấu hiệu nhận dạng lỗi giả — trước
> khi kết luận suite này đỏ là hồi quy, hãy hỏi máy lúc đó đang chạy gì.

</details>

Quy mô (chưa đếm lại từ `c0a3cff`, chỉ dùng để cảm nhận bậc độ lớn): Rust ~47,7k dòng / 128 file ·
TS ~25,6k dòng / 126 file · 7 spec Playwright.

Chỉ số kỷ luật đáng giữ, vì mất đi thì khó lấy lại:

- **5** `.unwrap()/.expect()` trong toàn bộ Rust *production* (phần còn lại nằm trong `#[cfg(test)]`).
- **2** khối `unsafe` trong cả backend — cả hai ở [`ai/model.rs:360`](../../src-tauri/src/ai/model.rs).
- **3** marker `TODO/FIXME` trong mã nguồn.

---

## 2. Điều đã đúng, và đúng ở mức nào

Dùng thang bậc của chương trình G0–G4 (xem
[`docs/ai/planning/2026-07-25-claude-overnight-goal-g0-g4-remediation.md`](../ai/planning/2026-07-25-claude-overnight-goal-g0-g4-remediation.md)),
vì "DONE" một mình không phân biệt được "có hàm thuần đã test" với "đường sống chạy qua nó".

| Vùng | Bậc đạt được | Ghi chú |
|---|---|---|
| Hạ tầng kiểm thử và gate CI | Live integrated | Gate nhắm vào chế độ hỏng thật, không phải tick checkbox — xem §2.1 |
| Năng lượng đóng (EU) ở thế giới sống | Live integrated | G1.1 — bit-exact, không phải "trong dung sai" |
| Snapshot/checkpoint | Live integrated | G1.2 — `rand_chacha` trực tiếp để lưu được vị trí draw |
| Lõi tất định | Live integrated | G1.3 — `SimRng`, `BTreeMap` thay `HashMap`, test quét mã nguồn chặn `thread_rng()` |
| Hợp đồng Rust↔TS | Live integrated | G1.4 — gate parity `ts-rs` trong CI |
| Tách feature build | Live integrated | G2 gate #2 — CI kiểm bằng `cargo tree`, không chỉ "biên dịch được" |
| Trần tài nguyên runner | Live integrated | G2 gate #3 — `MAX_ENSEMBLE_RESULT_BYTES`, ước lượng bão hoà thay vì tràn |
| Não tiến hoá per-agent (ADR-0003) | Đã triển khai, **tắt mặc định** | 11/12 gate EB pass — xem §3.1 |
| Lab tiến hoá AE1–AE3 | Headless, opt-in | `ReferenceEvolutionWorld` |
| Adapter thí nghiệm cho thế giới sống | **Headless verified** | `LiveExperimentAdapter` chạy **đúng** lịch trình app dùng, qua runner chung; 15 gate ở `live_experiment_tests.rs`. **Chưa** chạy app desktop; không có exotic energy; không có quần thể AE3 — xem §3.3 |
| Đo tick trong tiến trình | **Đã ship, chưa có số app** | `core/tick_capture.rs` + 4 lệnh IPC; đo không làm đổi quỹ đạo (có gate). Số thật cần một lần chạy app — xem §3.2 |
| Thế giới chung frontend↔backend | Live integrated | `src/utils/sharedWorld.ts` là identity duy nhất; artifact đẩy sang `init_world` |

### 2.1 Vì sao bộ gate này đáng tin

Ghi lại để phiên sau không vô tình gỡ mất:

- `scripts/check_test_targets.mjs` bắt **target biên dịch thành binary rỗng rồi exit 0**.
  Đây là chế độ hỏng đã giấu 1.877 dòng coverage: bảy file có `#![cfg(feature = ...)]`
  ở cấp crate, và một `cargo test` trần biến chúng thành `running 0 tests`.
- CI kiểm tách feature bằng **đồ thị phụ thuộc** (`cargo tree`), không phải bằng việc
  biên dịch được. Biên dịch được sẽ vẫn xanh vào lần đầu ai đó thêm một `use` vô điều kiện.
- Gate parity `ts-rs`: `cargo test` sinh `src/types/generated/`, rồi `git diff --exit-code`.
  Đây là thứ bắt được `head_directions` là map ở một phía và array ở phía kia.
- `sim_determinism_tests.rs` có một test **quét mã nguồn** và fail nếu `thread_rng()` quay lại.
- Ratchet ESLint chặn warning tăng, kể cả khi không ai giảm.

---

## 3. Việc cần làm, theo thứ tự

Thứ tự là **theo giá trị trả về**, không theo độ khó. Mỗi mục có điểm neo cụ thể và một
định nghĩa hoàn thành kiểm tra được.

### 3.0 Phiên sau bắt đầu ở đây (bàn giao **2026-07-27**)

> **Việc #1 của bàn giao trước đã XONG.** Số đếm đột biến theo node đã ship và nén lưu trữ đã bật —
> xem [§3.15.1](#3151-việc-còn-lại--đọc-mục-này-trước), nay chỉ còn OSS-072 (MRCA). Chi tiết cả đợt
> ở [`docs/ai/planning/2026-07-27-feature-anima-completion.md`](../ai/planning/2026-07-27-feature-anima-completion.md)
> và bảng bằng chứng ở [`docs/ai/testing/2026-07-27-feature-anima-completion.md`](../ai/testing/2026-07-27-feature-anima-completion.md).

| # | Việc | Vì sao là việc này | Đọc |
|---|---|---|---|
| 1 | **Chạy app desktop một lần với `ANIMA_TICK_CAPTURE`** | Việc duy nhất còn lại của §3.2, và **máy không làm được** — cần một con người bấm chạy. Dụng cụ đã ship và đã test; thiếu đúng một lần chạy | [§3.2](#32-thay-số-hiệu-năng-proxy-bằng-số-đo-thật--một-nửa-đã-xong-2026-07-26), [BENCHMARKING.md](../how-to/BENCHMARKING.md) |
| 2 | **Quyết định EB-S04, rồi mới bàn mặc định não per-agent** | P0 lâu nhất chưa nhúc nhích. Việc thật là **re-baseline một gate không thể pass bằng cách sửa code đúng**, không phải lật cờ. Đã ghi thành quyết định DEC-1 kèm 3 phương án và khuyến nghị | [§3.1](#31-bật-não-tiến-hoá-per-agent-trên-đường-mặc-định) |
| 3 | **OSS-072 MRCA** | Nửa khoa học còn lại của phả hệ. Nén đã xong nên `simplify` đã có sẵn cấu trúc cha/con để dùng lại | [§3.15.1](#3151-việc-còn-lại--đọc-mục-này-trước) |
| 4 | ~~**In-app tick capture**~~ **XONG 2026-07-27** | Đã ship kèm 4 lệnh IPC và gate "đo không làm đổi quỹ đạo" | [§3.2](#32-thay-số-hiệu-năng-proxy-bằng-số-đo-thật--một-nửa-đã-xong-2026-07-26) |
| 5 | ~~**§3.3 adapter thí nghiệm cho thế giới sống**~~ **XONG headless 2026-07-27** | `LiveExperimentAdapter` qua runner chung, trên đúng lịch trình app chạy | [§3.3](#33-đưa-thế-giới-bevy-sống-qua-gate-thí-nghiệm--adapter-headless-đã-xong-2026-07-27) |

**Một việc cần con người, máy không làm được:** chạy `npm run tauri:dev` một lần để xác minh CSP mới
(`tauri.conf.json` nay có `csp` + `devCsp` thay cho `null`). `npm run check:csp` chỉ kiểm **artifact
đã build** so với chính sách đã khai; nó **không** chứng minh app khởi động được dưới chính sách đó,
vì CLAUDE.md cấm chạy full backend trên máy này.

**Ba cái bẫy mới, trả giá ngày 2026-07-27:**

- **`compact` lọc quan hệ GỐC theo tập node sống sót — đúng khi chưa nén, phá đồ thị ngay khi nén.**
  Đường `A → B → C` có cả hai cạnh đều nhắc `B`; `B` bị cắt thì cả hai cạnh bị loại và `C` **âm thầm
  thành root**. Phải dựng lại quan hệ từ `plan.edges`. Gate: `compaction_leaves_no_orphans`.
- **`unsafe impl Send/Sync` trong `ai/model.rs` KHÔNG thừa, và lý do không phải `WgpuDevice`.**
  `Param` của burn 0.13 chứa `OnceCell` + closure `dyn Fn + Send` (không `Sync`), nên hỏng ở **cả hai**
  backend. Chỉ đường wgpu ép sẵn một forward pass; đường ndarray thì không, tức là trao ra một giá trị
  có cell rỗng cho các reader song song của Bevy. Đừng gỡ `unsafe` mà không đọc `// SAFETY:` ở đó.
- **`esbuild` KHÔNG được cài** (Vite 8 dùng rolldown/oxc). Mọi script gọi `esbuild` đều chết. Dùng
  `node scripts/run_ts.mjs <file.ts>`.

**Ba cái bẫy đã trả giá trong ngày 2026-07-26 — đọc trước khi sửa quanh những vùng đó:**

- **`add_reproduction` từ chối ghi cạnh có cha không tồn tại.** Đừng "sửa" thành ghi vô điều kiện:
  một cạnh mồ côi làm hỏng **toàn bộ** đồ thị lineage, vì cả `to_newick` lẫn `simplify` từ chối xử
  lý đồ thị chứa nó.
- **Tập sample của `compact` không phải "ai đang sống"** — phải gồm mọi `lineage_id` trong archive
  MAP-Elites.
- **Số `cargo bench` in ra là *slope estimate*, không phải trung vị.** Chênh thật: `step_water`
  297,6 µs (slope) so với 271,5 µs (trung vị). Bảng số dùng trung vị, đọc từ `estimates.json`.

**Một finding đang mở, chưa ai đối chứng:**

- Con số **~4,2 ms** trong doc comment của `ResourceField::REGROWTH_STRIDE` **không tái lập được** —
  release build cho ~0,36 ms, thấp hơn ~12 lần. Việc stride vẫn đúng (đo được 3,97×); chỉ con số
  biện minh là chưa đối chứng.

**Finding `DEFAULT_GRID_DIM` đã đóng — đừng mở lại từ một bản sao cũ.** Câu cũ ở đây nói hằng số là
**128**; nó là **256** kể từ 2026-07-27 và bị test `s03_default_grid_dim_tracks_map_settings_default`
buộc phải bằng `MapSettings::default().width`. `COORDINATE_CONTRACT.md` §"Backend sim (mặc định)"
đã ghi `200 / 256 = 0.78125`. Đường đo mới không dùng hằng số nào cả: tick capture đọc kích thước
**từ `ResourceField` của thế giới đang chạy** và ghi vào `CaptureExport.workload` kèm cờ
`dimensions_measured`, nên một số sai không thể lọt vào một report mà không lộ ra. Con số 128 còn sót
lại **chỉ** ở `config.gridDim` trong `benchmark_report.json` (file kết quả cũ), không ở đường code nào.

### P0 — Đóng vòng lặp khoa học

Đây là khoảng cách thật của dự án. Chất lượng kỹ thuật đã cao; cái thiếu là **bằng chứng
trên đường mặc định**.

#### 3.1 Bật não tiến hoá per-agent trên đường mặc định

**Vì sao P0.** Một run mặc định hiện cho **mọi agent dùng chung một `BrainModel`**. Đó
chính là gap mà [`docs/research/MAP_AND_ML_UPGRADE_RESEARCH.md`](../research/MAP_AND_ML_UPGRADE_RESEARCH.md)
gọi là lớn nhất. Toàn bộ máy móc đã có và đã test — nó chỉ đang tắt.

**Trạng thái thật:** 11/12 gate EB pass (bảng gate trong
[ADR-0003](../decisions/ADR-0003-evolved-per-agent-brains.md)). Chỉ **EB-S04** là 🟡 một phần.

**Điểm neo:** `BrainPolicy::default()` tại
[`src-tauri/src/core/resources.rs`](../../src-tauri/src/core/resources.rs) — `evolved: false`,
`lifetime_learning` tắt, `brain_metabolic_cost: 0.0`. Cờ `ANIMA_EVOLVED_BRAINS`,
`ANIMA_LIFETIME_LEARNING`.

**Việc thật cần làm trước tiên là quyết định về EB-S04, không phải lật cờ.** EB-S04 đòi
quỹ đạo *bit-identical* với baseline. Nó fail vì khởi tạo model dùng chung đã đổi từ
ngẫu nhiên sang **có seed** — tức là fail vì một **cải tiến có chủ ý**, không phải vì hồi quy.
Một gate không thể pass bằng cách sửa code đúng thì phải được **re-baseline một cách tường minh**:
đặt lại mốc so sánh về bản dựng có seed, ghi vào ADR *vì sao* mốc cũ bị bỏ, rồi mới bàn đến mặc định.

**Định nghĩa hoàn thành:** EB-S04 chuyển 🟡 → ✅ với mốc mới ghi rõ trong ADR-0003 · quyết định
mặc định (`evolved: true` hay giữ opt-in) được ghi thành mục quyết định trong ADR · nếu bật mặc định
thì `cargo test --features desktop` vẫn xanh và EB-S03 (`allocs == 0`) vẫn giữ.

#### 3.2 Thay số hiệu năng proxy bằng số đo thật — **một nửa đã xong (2026-07-26)**

**Vì sao vẫn P0.** Tuyên bố "60 FPS real-time" của dự án nay **đã có số đỡ, nhưng chưa được chứng
minh**. Mọi quyết định về scale (LOD, ngân sách bộ nhớ não, số agent thường trú) trước đây dựa trên
ước lượng; giờ chúng dựa trên một **cận dưới đo được**, không phải một khung hình đo được.

**Ràng buộc vận hành có thật:** không chạy `npm run tauri dev` / `cargo run` trên máy dev.
Đây là ràng buộc, không phải lời khuyên — nó đã crash máy.

##### Đã xong

- **OSS-010 Criterion đã ship.** `dev-dependency` + [`src-tauri/benches/tick_systems.rs`](../../src-tauri/benches/tick_systems.rs),
  bench từng system headless — không Tauri, không GPU device. Bảng số và cảnh báo diễn giải:
  [`docs/how-to/BENCHMARKING.md`](../how-to/BENCHMARKING.md).
- **Phần cứng mục tiêu đã đổi và đã khớp.** Nay là Intel Core i5-14600KF; khai báo *Dell Vostro
  3530* cũ **không còn hiệu lực**. Máy capture và máy đích là một, nên số đo tính là đo-trên-đích.
- **`benchmark_report.json` mang 16 số đo thật** dưới tiền tố `criterion/`, kèm khối hardware đúng.
  `scripts/bench_baseline.mjs` đọc thẳng từ output Criterion, và **từ chối** ghi đè số thật bằng
  proxy (exit ≠ 0) trừ khi `ANIMA_BENCH_ALLOW_PROXY_ONLY=1`.
- Gate `cargo tree --no-default-features -e normal` xác nhận Criterion **không** vào bản dựng mặc
  định (G2 #2), và test S04 vẫn xanh.

##### Đã xong 2026-07-27 — dụng cụ đo in-app

**In-app tick capture đã ship**: [`core/tick_capture.rs`](../../src-tauri/src/core/tick_capture.rs)
+ ba checkpoint trong **đúng lịch trình sống** + kẹp `schedule` / `telemetry_publish` / `full_tick`
trong chính vòng lặp của sim thread + bốn lệnh IPC (`start`/`status`/`stop`/`export`, kiểu TS sinh
bằng ts-rs). Ring cấp phát sẵn, có warm-up, tần suất lấy mẫu, trần số mẫu, kế toán mẫu bị ghi đè /
bị bỏ, và phân vị **nearest rank**. Cách dùng và giới hạn diễn giải:
[`docs/how-to/BENCHMARKING.md`](../how-to/BENCHMARKING.md).

Hai điều được kiểm bằng máy chứ không hứa:
`capture_does_not_change_the_live_trajectory` (checksum, observable và **vị trí stream RNG** giống
hệt khi tắt, khi có sink nhưng idle, và khi đang ghi) và
`a_capture_of_the_real_schedule_produces_phases_that_add_up` (bốn pha checkpoint cộng lại **đúng
bằng** pha `schedule`).

##### Còn thiếu — và nay chỉ còn một lần chạy app thật

Cái đo được là **cận dưới của một tick, không phải khung hình**: tổng các system chạy mỗi tick ở
1.000 agent ≈ 493 µs ≈ 3,0 % của 16,67 ms, nhưng con số đó **chưa gồm** suy luận não, lập lịch ECS,
change detection, thread emit, va chạm và trao đổi chất.

1. **Chạy app desktop một lần với `ANIMA_TICK_CAPTURE`** rồi điền các hàng `Physics tick` /
   `Brain/sensor` / `Full-brain agents` trong
   [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) từ `p50_ns` của file export. Dụng cụ đã
   có và đã được test; **chưa có lần chạy app đầy đủ nào** trong gói 2026-07-27, vì CLAUDE.md cấm
   chạy full backend trên máy này. Thủ tục ba bước ghi ở BENCHMARKING.md.
2. **Suy luận não per-agent** là phần đắt nhất còn lại và **đang tắt mặc định** (§3.1), nên chưa có
   gì trên đường mặc định để đo. §3.1 và §3.2 vì thế nối vào nhau.

**Định nghĩa hoàn thành (phần còn lại):** ba hàng nói trên có số thật · `BENCHMARK_BASELINE.md`
tuyên bố khoá số theo `WORLD_SIMULATION_PLAN.md` §10.2 · ngân sách EB-S12 (22,5 KiB/agent,
~46.500 agent/GiB) được đối chiếu với số thật thay vì với ngoại suy tuyến tính hiện tại (~4,02 ms
≈ 24 % khung hình, **là ngoại suy chứ không phải phép đo**).

**Một finding mở, sinh ra từ chính đợt đo:** doc comment của `ResourceField::REGROWTH_STRIDE` ghi
đường trước khi stride tốn ~4,2 ms/tick; đo lại release build cho **~0,36 ms**, thấp hơn ~12 lần.
Việc stride vẫn đúng và có lợi (đo được 3,97×) — chỉ con số biện minh cho nó là chưa đối chứng được.
Theo quy tắc 6 của [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md), đây là finding cần
đối chứng, không phải lỗi đã xác định.

#### 3.3 Đưa thế giới Bevy sống qua gate thí nghiệm — **adapter headless đã xong (2026-07-27)**

**Trạng thái mới.** [`core/live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs) là
`LiveExperimentAdapter: ExperimentModel`, chạy qua **cùng** `experiment_runner` với
`ReferenceEvolutionWorld` và trên **đúng** lịch trình app dùng —
[`core/simulation_schedule.rs`](../../src-tauri/src/core/simulation_schedule.rs), hàm mà
`SimulationEngine::start` gọi. Gate:
[`tests/live_experiment_tests.rs`](../../src-tauri/tests/live_experiment_tests.rs) (15 test) —
cùng seed + manifest ⇒ cùng checksum · can thiệp nổ **đúng tick khai báo** và không sớm hơn ·
checkpoint fork từ tick 60 (nhánh control khớp bit với một run liền mạch) ·
`run N == run K → ghi ra file → đọc lại → run N−K` · registry observable hợp lệ và trùng đơn vị với
registry tham chiếu ở đúng hai id chia sẻ (`plants`, `detritus`) · và một test **hướng** giữa hai
đường cho một luật chung.

**Hai lỗ persistence thật do gate này lộ ra, đã vá:**

- **Pha stride của regrowth sống trong `Local<usize>`.** `REGROWTH_STRIDE = 4` nghĩa là mỗi tick chỉ
  một phần tư số ô mọc lại, nên *phần tư nào* là trạng thái quỹ đạo — mà `Local` thì không snapshot
  nào đọc được. Nay là `ResourceField::regrowth_phase`, được lưu (schema 5) và **vào `world_checksum`**.
  Gate cũ không bắt được vì `K = 1500` chia hết cho 4.
- **Một tick để lại suy luận "đang bay".** App giao request cho thread worker và nhận trả lời ở tick
  *sau*; ở ranh giới checkpoint, hai batch khoá theo `Entity` đang nằm trong kênh, mà id entity
  không ổn định qua restore. Adapter thí nghiệm nay trả lời **trong chính tick đã hỏi**
  (`live_inference_pump_system`), nên mỗi tick là nguyên tử và toàn bộ quỹ đạo nằm trong world.

**Chưa tuyên bố, và đừng tuyên bố:** chưa có lần chạy **app desktop đầy đủ** nào (executor đa luồng);
adapter **từ chối** `laws.exotic_energy` vì thế giới sống không có trường MU, và không có quần thể
AE3; và **không** tuyên bố trùng số với `ReferenceEvolutionWorld` — chỉ trùng **hướng và ý nghĩa**
của một luật chung.

**Điểm neo:** `src-tauri/src/core/live_experiment.rs`, `core/simulation_schedule.rs`,
`core/reference_world.rs`, `core/evolution_pathway.rs`, `core/scenario.rs`. Đọc
[`docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) trước.

**Điều kiện tiên quyết — đọc trước khi bắt tay:** mục này **và** §3.6 là hai mặt của cùng một việc,
chính là **G2 task 1** trong
[kế hoạch G0–G4](../ai/planning/2026-07-25-claude-overnight-goal-g0-g4-remediation.md) ("Headless và
Bevy trở thành hai adapter trên `anima-domain`… AE4 *là* sự hội tụ này"). Không thể đưa thế giới
sống qua gate thí nghiệm khi `WorldLawSet`, `ExperimentManifest`, `CausalLedger` và `SimClock` chưa
trở thành tài nguyên/luật của engine sống — mà đó đúng là nội dung §3.6. Một phiên nhận §3.3 mà bỏ
qua §3.6 sẽ chạm tường ngay. Chúng được đánh số ở hai bậc ưu tiên chỉ vì thứ tự trình bày;
**trên thực tế §3.6 cũng là P0.**

**Nhãn:** `DETERMINISM_CONTRACT` §5 và kế hoạch G0–G4 (dòng 852) gọi phần đường khởi động live này
là **G2**. Tài liệu này gọi nó là §3.3. Cùng một việc — giữ cả hai tên khi tra cứu chéo.

**Định nghĩa hoàn thành:** một manifest chạy được trên thế giới sống cho cùng checksum qua
hai lần chạy cùng seed ✅ · save/load giữ nguyên quỹ đạo ✅ · gỡ được câu cấm tương ứng trong
CLAUDE.md ✅ (đã thay bằng một câu **hẹp hơn** nói rõ cái gì đã kiểm và cái gì chưa) · **còn lại:**
một lần chạy app desktop dưới executor đa luồng.

---

### P1 — Nợ nền tảng đã biết

#### 3.4 Gate `burn`/`wgpu` sau feature flag

**Blocker là hình dạng code, không phải khối lượng.** `learn_handle` tại
[`src-tauri/src/core/simulation_loop.rs:182`](../../src-tauri/src/core/simulation_loop.rs)
được gán từ một `if has_wgpu { … } else { … }` mà hai nhánh **không `cfg` riêng từng nhánh được**.
Cần tái cấu trúc chỗ đó, không phải rắc attribute. `burn` chỉ dùng ở **hai** file — bảy kết quả
grep còn lại là văn xuôi khớp chữ "burned energy".

**Định nghĩa hoàn thành:** `cargo tree --no-default-features` không còn `burn-wgpu`/`naga` ·
gate `cargo tree` trong CI mở rộng để phủ chúng · fallback CPU vẫn chạy.

#### 3.5 Thu hẹp `tokio = { features = ["full"] }`

[`src-tauri/Cargo.toml:56`](../../src-tauri/Cargo.toml). Hai hệ con cần tokio đã nằm sau feature
(`networking`, `neo4j`), nên `full` giờ là thừa. Đây là bước tự nhiên tiếp theo sau G2 gate #2.

**Định nghĩa hoàn thành:** chỉ liệt kê feature thật sự dùng · cả hai cấu hình
(`--no-default-features` và `--features desktop`) đều clippy sạch và test xanh.

#### 3.6 G2 gate #1 — một thay đổi luật đổi cả hai engine

> **Thực chất là P0, không phải P1.** Đây là điều kiện tiên quyết của §3.3 — cùng là G2 task 1 /
> hội tụ AE4. Nó nằm dưới đề mục P1 vì thứ tự đánh số, không vì ưu tiên. Đừng nhận §3.3 mà bỏ mục này.

`crates/anima-domain` đã tách và đang giữ `causal`, `energy`, `intervention`, `laws`,
`sim_clock`, `units`. Gate đầy đủ đòi tách tiếp thành nhiều workspace member. Đây là mục
G2 duy nhất thật sự nhiều phiên.

**Định nghĩa hoàn thành:** một hằng số luật đổi ở `anima-domain` làm cả hai engine đổi hành vi,
có test chứng minh.

#### 3.7 Vòng đời thread: supervisor **xong**, cancellation token còn lại

**Cách đặt vấn đề cũ của mục này sai, và đó là phần đáng đọc nhất.** Nó nói "rò thread". Một lượt
kiểm kê **cả bảy** chỗ `thread::spawn` trong crate cho thấy handle nào cũng được thu hồi: năm cái vào
vector mà `stop` join, `inference_handle` được join trong sim thread, và hai biến thể learner là một
thread mà chỉ một cái được dựng. **Không có thread Rust nào bị rò.** Ai nhận mục này mà đi tìm chỗ rò
sẽ tìm một thứ không tồn tại.

Khoảng trống thật là `stop` không trả lời được hai câu hỏi duy nhất quan trọng khi shutdown sai — và
chế độ hỏng thì vô hạn.

##### Đã xong (2026-07-26, PR #14 → `e5fb8be`)

[`core/thread_supervisor.rs`](../../src-tauri/src/core/thread_supervisor.rs). Mỗi thread mang một
`ExitToken` có tên, **move vào closure** nên nó drop khi stack unwind — kể cả khi panic. Đó là phân
biệt mà `let _ = handle.join()` không làm được: thread chết bẩn báo "đã thoát", không phải "treo".

`stop` giờ **chờ báo cáo trước khi join**. Thứ tự là cốt lõi: `JoinHandle::join` không có timeout, nên
join trước nghĩa là một thread phớt lờ `running` làm `stop` chờ mãi — đúng hình dạng cú treo CI, một
cú chờ không giết được và không in gì.

Gate: [`tests/thread_lifecycle_tests.rs`](../../src-tauri/tests/thread_lifecycle_tests.rs) (6) +
6 unit test của module. Hợp đồng "sau `stop`, không còn gì `start` sinh ra đang chạy" nay **được máy
kiểm** thay vì giả định, cộng 8 chu kỳ start/stop vì lỗi CI xuất hiện ở test cycle 101 lần — nơi rò
một thread mỗi chu kỳ mới cộng dồn.

**Đánh đổi, ghi ra chứ không giấu:** thread quá hạn 30 giây thì **không** được join. Nó bị detach và
rò suốt đời tiến trình. Rò một thread **có tên** kèm thông báo hơn một `stop()` không bao giờ trả về
và không nói gì — nhưng đó là đánh đổi, không phải bản sửa. Bản sửa là thứ khiến thread đó phớt lờ
`running`.

##### Còn lại

- **Cancellation token thống nhất.** Hiện chỉ có một tín hiệu, `running: Arc<AtomicBool>`, và mỗi
  thread tự chọn tần suất kiểm. Supervisor cho biết *ai* không về; nó không làm thread đó về nhanh hơn.
- **Cú treo CI vẫn chưa được giải thích.** Nó không tái hiện kể từ PR #10, và phần vừa xong gỡ chế độ
  hỏng *vô hạn* mà **chưa** tái hiện được thứ kích hoạt nó. Ba dụng cụ giờ đã ở trên `main` (watchdog
  PR #10, `--nocapture` PR #12, supervisor PR #14); lần treo kế tiếp sẽ nêu tên chu kỳ, pha, hoặc
  thread — thay vì 90 phút im lặng.
- Closure sim-thread ~900 dòng và evo thread ~290 dòng trong `start()` vẫn chưa tách; xem §4 và ghi
  chú nợ của lượt refactor trước.

#### 3.8 Hai ADR đang ở trạng thái `proposed`, và cả hai đều đang đỡ tải

`proposed` chỉ vô hại khi chưa có gì dựa lên nó. Hiện không còn như vậy:

- **[ADR-0002](../decisions/ADR-0002-world-laws-and-exotic-energy.md)** — AE1–AE3 đã ship trên nó
  (xem §3.10). Điều mới: ADR-0004 viện dẫn quy tắc **ER01** của nó (`WorldLawSet` bất biến trong một
  run; nhánh checkpoint không đổi law fingerprint) làm ràng buộc thiết kế. Một ADR `proposed` giờ là
  nền của một ADR `proposed` khác — hoà giải §3.10 vì thế đã lên giá.
- **[ADR-0004](../decisions/ADR-0004-observer-as-declared-intervention.md)** — **accepted
  2026-07-26, O1 đã ship.** Người quan sát nhập vai là can thiệp được khai báo. Bối cảnh là một phát
  hiện chứ không phải đề xuất tính năng: `LodFocus` do camera lái **đã** là forcing lên thế giới và
  nằm ngoài mọi provenance, vì [`tier_at`](../../src-tauri/src/core/simulation_lod.rs) phân tier theo
  khoảng cách tới observer và
  [`cold_agents_stop_asking_entirely`](../../src-tauri/tests/simulation_lod_tests.rs) chốt rằng agent
  `Cold` **thật sự không suy nghĩ**.

  O1 đã đóng chỗ đó: [`core/observer.rs`](../../src-tauri/src/core/observer.rs) +
  enforcement ở `sync_lod_focus_system` + field `observer` trên `ExperimentManifest`
  (`#[serde(default)]`, **không** bump schema version). Gate:
  [`tests/observer_policy_tests.rs`](../../src-tauri/tests/observer_policy_tests.rs) — 7 pass, gồm
  `spectate_matches_absent` **và** control âm `an_inhabited_camera_actually_changes_who_thinks`;
  cộng 11 unit test cho kiểu và cho manifest.

  **Bẫy còn sống, đọc trước khi sửa quanh đây:** *thiếu resource `ObserverPolicy`* ≠ `Absent`. Thiếu
  nghĩa là chưa ai khai báo, và phải giữ nguyên hành vi cũ — **tuân theo camera**. `Absent` là khai
  báo ngược lại và **cấm** camera. Lẫn hai cái này sẽ âm thầm tắt LOD của app đang chạy, vì
  `PixiViewport.tsx` vẫn gọi `set_lod_focus` thật.

  **O2 cũng đã ship (2026-07-26).** `ObserverTrace` ghi focus **hiệu lực** (sau policy) vào buffer
  cấp phát sẵn, ghi-khi-đổi, và **đếm** mẫu tràn thay vì bỏ im lặng. `CAUSE_OBSERVER` nằm ở đỉnh dải
  `CauseId` vì scenario cấp tay từ dưới lên; manifest bị cấm giành id đó. App sống nay khai báo
  `Inhabit` thật thay vì "chưa khai báo" — hành vi không đổi, nhưng hệ quả đã có gốc.
  `DETERMINISM_CONTRACT` §2 nay là **năm** nguồn rò rỉ, có §2.1 cho camera.

  Gate: `tests/observer_trace_tests.rs` (6) + `tests/observer_trace_zero_alloc_tests.rs` (1 test,
  3 pha) — gồm control âm `a_chain_the_observer_did_not_start_does_not_name_them`.

  **Bẫy thứ hai, do chính test bắt được:** từ chối focus phải thay **trọn** `LodFocus::default()`,
  không chỉ tắt `enabled`. Giữ lại `center` sẽ để nguyên một đường camera sống bên trong world cho
  system sau này đọc phải — tái lập đúng cái nhiễu vừa cấm.

  **O3 — cơ chế đã ship (2026-07-26), tuyên bố phiên sống vẫn chặn.** `ObserverReplay` phát lại một
  trace thay cho camera sống và **loại trừ** camera chứ không xếp trên nó. Nội suy được khai báo:
  focus **giữ nguyên** giữa hai mẫu — không phải xấp xỉ, vì `record` chỉ lưu khi giá trị đổi. Gate:
  `tests/observer_replay_tests.rs` (6), gồm control âm và một test camera thù địch không lái được
  replay.

  Ranh giới, đọc kỹ trước khi tuyên bố gì: gate đó ghim **hệ con** và tự khai báo thứ tự schedule —
  đúng phạm vi mà `SNAPSHOT_CONTRACT` §8 tự nhận về mình. Gate
  `an_inhabited_run_replays_from_its_trace_without_a_human` đo *quỹ đạo thế giới sống* và vẫn
  **pending**: physics/CPG chạy song song nên một run liền mạch còn không khớp chính nó.

  **Chưa làm, có lý do:** lưu trace vào save state cần bump `SCHEMA_VERSION` 4→5, mà
  `MIN_SUPPORTED_SCHEMA = SCHEMA_VERSION - 2` nên bump sẽ **mất khả năng đọc save v2**. Trả cái giá
  đó cho dữ liệu chưa mode nào tiêu thụ là sai thứ tự. Khi làm, phải vào **cả** `SavedSimulationState`
  lẫn `world_checksum` một lượt (§8) — lưu ý khi *ghi* thì trace là đầu ra và không thuộc checksum,
  khi *phát lại* thì phần còn lại là đầu vào và thuộc.

  **Hành động nhập vai: đã ghi nhận (2026-07-26), chưa cưỡng chế.** Câu cũ ở đây nói engine chưa có
  hành động nhập vai nào. Sai — chúng đã tồn tại từ trước, chỉ không được gọi bằng tên đó. Bốn lệnh
  IPC ghi thẳng vào thế giới đang chạy mà không khai báo: `update_evolution_settings`,
  `toggle_evolution`, `trigger_migration`, `set_sharding_config`. Và chúng **mạnh hơn camera** —
  camera đổi *con nào được suy nghĩ*, còn cái đầu tiên đổi **luật mà chọn lọc vận hành dưới đó**,
  giữa run. Nghĩa là `DETERMINISM_CONTRACT` §2 chưa đủ ở nguồn thứ năm.

  Đã ship: `ObserverAction` + `SharedObserverActions` + `drain_observer_actions_system` + buffer
  `actions` riêng trong `ObserverTrace` (đếm tràn riêng, vì mất một hành động là lỗ **provenance**
  còn mất một mẫu focus chỉ mất độ trung thực replay). Gate:
  [`tests/observer_action_tests.rs`](../../src-tauri/tests/observer_action_tests.rs) (11), gồm một
  test **quét mã nguồn** chặn bốn lệnh đó lặng lẽ quay lại đường cũ — kèm control âm chứng minh scan
  có thể fail.

  Còn lại là **cưỡng chế**: một lệnh vẫn có thể với qua queue mà ghi thẳng shared state. Việc đó cần
  ledger có mặt trong world sống, tức §3.3/§3.6.

#### 3.15 Phả hệ: bộ nhớ không có trần, và không truy được dòng dõi

> **Số thứ tự là append-only.** Mục này thuộc **P1** nhưng mang số 3.15 vì 3.9–3.14 đã được bảng P2
> dùng, và `CLAUDE.md` cùng các kế hoạch khác tham chiếu chéo theo số. Đổi số sẽ làm gãy các tham
> chiếu đó — đắt hơn nhiều so với một số thứ tự trông lệch.

> **Cập nhật 2026-07-26 — ba phần tư đã xong.** Bản gốc của mục này mô tả một tình trạng nay đã
> khác. Phần còn lại được viết ở [§3.15.1](#3151-việc-còn-lại--đọc-mục-này-trước) với đủ điểm neo để
> một phiên mới bắt tay ngay.

**Vì sao vẫn P1.** [`evolution/lineage.rs`](../../src-tauri/src/evolution/lineage.rs) lưu mỗi lần
sinh sản thành một `LineageNode` kèm **bản sao đầy đủ** `MorphologyGenotype`. Đường tăng nay **đã có
trần một phần** (compaction bỏ nhánh tuyệt chủng mỗi 50 epoch) nhưng **chưa đạt O(cá thể sống)**, vì
compaction chạy với nén tắt. Và vẫn **không trả lời được "hai cá thể này rẽ nhánh ở đâu"**: chưa có
truy vấn MRCA.

Hệ quả khoa học đáng kể hơn hệ quả kỹ thuật: **không có MRCA thì không bám được *line of descent***,
tức là không dùng được giao thức mà Avida dùng để **đo** một sự kiện tiến hoá thay vì kể lại nó. Với
một dự án mà §3.3 đang cố chứng minh thế giới sống là experiment-ready, đó là một lỗ hổng bằng chứng.

**Đã xong (2026-07-26), theo thứ tự chúng khoá vào nhau:**

| Mục | Kết quả | Neo |
|---|---|---|
| OSS-070 xuất Newick | ✅ DendroPy 5.0.10 đọc được; gate hai nửa cùng một fixture | `evolution/newick.rs`, `scripts/verify_newick.py` |
| OSS-071 thuật toán `simplify` | ✅ 2.047 node / 16 sống → **31 node**, đúng cận `2·samples` | `evolution/simplify.rs` |
| OSS-071b nối vào tracker sống | ✅ **2026-07-27** — `compact()` chạy mỗi 50 epoch, **nén BẬT**; số đột biến lưu theo node | `lineage.rs`, `simulation_loop.rs` |

**Đường đi không tốn dependency nào**, và điều đó vẫn đúng: lấy **thuật toán** `simplify()` của
tskit chứ không lấy crate, lấy **định dạng** Newick chứ không lấy code R. Chi tiết ở OS7 trong
[kế hoạch áp dụng nguồn mở](OPEN_SOURCE_ADOPTION_PLAN.md) và
[khảo sát §5](../research/OPEN_SOURCE_LANDSCAPE.md).

##### 3.15.1 Việc còn lại — đọc mục này trước

> **Việc (1) đã XONG 2026-07-27** (`57a8246`). `LineageNode.cumulative_mutations: Option<u32>` +
> `#[serde(default)]` đã ship, `compact` nay chạy `compress_unary_paths: true`, và gate ở
> `tests/lineage_mutation_count_tests.rs` (7 test) + `lineage_compaction_tests.rs` (9). Cận
> `2·samples` đã đo được. **Chỉ còn việc (2), MRCA.**
>
> **Một bẫy kế hoạch dưới đây KHÔNG nói tới, và nó phá đồ thị:** `compact` lọc quan hệ **gốc** theo
> tập node sống sót. Đúng khi chưa nén; nhưng khi nén, đường `A → B → C` có **cả hai** cạnh đều nhắc
> `B`, nên cắt `B` là mất cả hai và `C` âm thầm thành root. Phải dựng lại quan hệ từ `plan.edges`.
> Gate: `compaction_leaves_no_orphans` (dùng số root của `to_newick` làm máy dò).
>
> Hệ quả đã ghi rõ chứ không giấu: cạnh bị nén mang `path_events: Some(n)` và `relation_type` khi đó
> là **tóm tắt đường đi**, không phải một sự kiện có thật. Genotype của node bị cắt thì mất hẳn. Tổng
> số đột biến vẫn chính xác — đó là toàn bộ lý do trường kia tồn tại.

**(1) Lưu số đếm đột biến tích luỹ theo node, rồi bật nén cho tầng lưu trữ.** ✅ **ĐÃ XONG**

*Vì sao chưa làm được ngay:* `get_mutations_count` trong
[`commands/evolution.rs`](../../src-tauri/src/commands/evolution.rs) suy ra con số đột biến hiển thị
trên UI bằng cách **đi qua `RelationType` từng cạnh**. Một cạnh đã nén đại diện cho một *đường* chứ
không phải một sự kiện, nên nó không mang được kiểu đó — bật nén mà không xử lý trước sẽ làm số đếm
sai (gộp 5 cạnh `Mutate` thành 1 thì con số đọc ra là 1, không phải 5).

*Cách làm:* thêm một trường vào `LineageNode` giữ **số đột biến tích luỹ từ gốc**. Nó biến phép đi
kia thành thừa **và** nhanh hơn.

- **Kiểu phải là `Option<u32>`, không phải `u32`.** `LineageNode` nằm trong `SavedSimulationState`;
  một `u32` mặc định `0` sẽ đọc thành *"không có đột biến"* cho **mọi save cũ** — hữu hạn, hợp lý,
  và sai. `None` nghĩa là "chưa ghi, hãy tính theo cách cũ", và `get_lineage_graph` fallback về phép
  đi qua cạnh khi gặp `None`.
- Thêm trường có `#[serde(default)]` thì **không cần** bump `SCHEMA_VERSION`: save cũ đọc ra `None`,
  và code cũ đọc save mới sẽ bỏ qua trường lạ. Kiểm lại giả định này trước khi dựa vào nó.
- Rồi đổi `LineageTracker::compact` sang `SimplifyOptions { compress_unary_paths: true }`.

*Định nghĩa hoàn thành:* số đột biến trên UI **không đổi** trước và sau khi nén (test so hai đường) ·
một save v4 cũ vẫn đọc được và cho đúng số · `compact` đưa số node về cận `2·samples` trên một
fixture đã biết · `tests/lineage_compaction_tests.rs` vẫn xanh.

**(2) OSS-072 — truy vấn MRCA.**

*Neo:* `evolution/simplify.rs` đã có sẵn cấu trúc cha/con và phép kiểm chu trình để dùng lại.

*Định nghĩa hoàn thành:* MRCA tất định, có test trên cây biết trước đáp án · xử lý đúng trường hợp
**DAG** (crossover cho hai cha, nên MRCA không nhất thiết duy nhất — phải quyết định trả về gì và
**ghi rõ**, đừng chọn bừa một nhánh) · trường hợp không có tổ tiên chung trả về gì cũng phải khai
báo, vì rừng nhiều gốc là chuyện bình thường ở đây.

Sau đó mới tới OSS-073 (giao thức đo "line of descent" kiểu Avida) — nó cần MRCA.

**Hai cái bẫy đã trả giá rồi, đừng đạp lại:**

- **`add_reproduction` nay từ chối ghi cạnh có cha không tồn tại.** Đừng "sửa" nó thành ghi vô điều
  kiện: một cạnh mồ côi làm hỏng **toàn bộ** đồ thị, vì cả `to_newick` lẫn `simplify` đều từ chối xử
  lý đồ thị chứa nó.
- **Tập sample của `compact` không phải "ai đang sống".** Phải gồm mọi `lineage_id` trong archive
  MAP-Elites, vì một elite có thể được chọn làm cha ở epoch sau mà không phải tổ tiên của ai đang
  sống.

**Chưa nối vào IPC.** `to_newick` và `simplify` đều chưa có lệnh Tauri nào gọi. Cần sửa hợp đồng ở
[`PROJECT.md`](../../PROJECT.md) §"Interface Contracts".

**Neo4j:** `compact` chỉ co bộ nhớ trong. Khi Neo4j online, `get_lineage_graph` đọc từ database nên
vẫn trả đồ thị **đầy đủ**.

---

### P2 — Vệ sinh, làm được lẻ

| # | Việc | Điểm neo | Vì sao đáng làm |
|---|---|---|---|
| 3.9 | Thêm `// SAFETY:` cho hai `unsafe impl Send/Sync`, hoặc bỏ nếu không còn cần | [`ai/model.rs:360`](../../src-tauri/src/ai/model.rs) | Đây là **2/2** khối unsafe của cả backend, và không cái nào có luận chứng. Type này ôm `WgpuDevice` |
| 3.10 | Hoà giải trạng thái ADR-0002 | [`ADR-0002`](../decisions/ADR-0002-world-laws-and-exotic-energy.md) vẫn `proposed` | AE1–AE3 đã ship. Theo quy tắc 6 của [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md), khi code và tài liệu xung đột thì **mở finding**, không tự coi code là đúng. **Đã lên giá:** ADR-0004 nay dựa vào ER01 của nó — xem §3.8 |
| ~~3.11~~ | ~~Giảm 491 warning ESLint~~ **XONG** (2026-07-27, `feature-anima-completion`) | `scripts/eslint_ratchet.mjs` baseline = **0** | 491 → 483 → 267 → **0**. Không nới rule, không thêm `eslint-disable`: sáu directive cũ đã biến mất, mọi `any` được thay bằng kiểu thật, và bốn rule React Compiler pass nhờ đổi code — frame loop đọc `state.scene`/`state.camera` thay vì đóng gói giá trị render, phần ghi three.js mệnh lệnh còn lại thành hàm có tên nhận đối tượng qua tham số. Ba finding hoá ra là lỗi thật (xem commit) |
| ~~3.18~~ | ~~Bỏ mọi lối thoát kiểu (`as any` / `as unknown as` / `as never` / `eslint-disable`) và bật typecheck cho `tests/`~~ **XONG** (2026-07-27, `feature-anima-completion`) | `npm run typecheck:tests` (gate mới trong CI) · [`src/window-globals.d.ts`](../../src/window-globals.d.ts) · [`tests/mocks/segment_fixtures.ts`](../../tests/mocks/segment_fixtures.ts) | Grep chính xác trên `src`/`tests`/`playground` **= 0** cho cả bốn mẫu, kể cả trong comment. Không suppress, không nới rule, không xoá test. Mỗi chỗ được gỡ bằng cách **làm cho điều đang bị khẳng định trở thành đúng**: các thuộc tính `window` được khai báo thật; `buildAgentHierarchy` nhận `readonly unknown[]` đúng như dữ liệu IPC nó vốn phòng thủ (lộ ra 2 lỗi thật: `segment_id` không phải số bị key bằng `undefined`, và `s.x \|\| 0` cho chuỗi lọt vào trường `number`); `biomeAt`/`findSpawn` nhận đúng các field chúng đọc; mock r3f dựng `THREE.Scene`/`PerspectiveCamera` thật. **Phát hiện phụ:** gói `tests/` chưa từng được typecheck — **86 lỗi** không lệnh nào báo, nay = 0 và có gate | |
| 3.12 | Tách file lớn | `experiment_runner.rs` 3.173 dòng · `experiment.rs` 2.192 · `exotic_energy.rs` 1.839 | Tiền lệ tốt đã có: `aae673e` tách learner và emit thread khỏi `simulation_loop.rs` |
| 3.13 | Nợ phụ thuộc | `burn` 0.13.2 (ghim, có lý do ghi trong CLAUDE.md) · `bevy_ecs` 0.13 · React 18→19 · `@react-three/fiber` 8→9 · `three` 0.184→0.185 | **Không phải nợ bảo mật** — advisory đã sạch và đã có gate. Là nợ framework |
| 3.14 | Dọn tài liệu cũ ở root | [`handoff.md`](../../handoff.md), [`plan.md`](../../plan.md) | Mô tả công việc Phase 1 / Phase 6 đã xong từ lâu. Đã gắn nhãn lịch sử; bước sau là chuyển vào `docs/archive/` |
| 3.16 | Đóng phần còn nợ của OSS-003 | [`licensing/UNRESOLVED.md`](../../licensing/UNRESOLVED.md) · [`LICENSE`](../../LICENSE) | **Phần kỹ thuật đã xong (2026-07-27).** `NOTICE` + `licensing/THIRD_PARTY_LICENSES.txt` + SBOM đã validate schema, tất cả đều sinh tự động và có gate CI. Dòng cũ ở đây viết *"chưa có `NOTICE`"* — **sai sự thật** từ `766609e`. **Cập nhật 2026-07-27 (đợt hai):** 31/32 khoảng trống đã đóng bằng kho vendor `licensing/upstream/` — 39 file license lấy từ đúng commit bất biến của bản phát hành (bằng chứng: `.cargo_vcs_info.json` trong `.crate` đã publish, `gitHead` npm, tag đã resolve), generator đọc fail-closed, `npm run verify:upstream-licenses` đối chiếu lại byte từ URL đã ghim. **Còn nợ, vẫn chặn phát hành:** (a) **1** thành phần — `hexf-parse` 0.2.1 (CC0-1.0) — upstream chưa từng publish văn bản license cho bản đó; đóng dòng này là **quyết định pháp lý**, không phải engineering, xem `UNRESOLVED.md`; (b) `LICENSE` vẫn không tách phạm vi code / model / dataset / asset; (c) `neo4rs`/`neo4rs-macros` chỉ có tuyên bố license trong README và `zune-inflate` chỉ publish văn bản Zlib trong ba lựa chọn — cần pháp lý đọc, xem `licensing/README.md` |
| 3.17 | Inventory dependency (OSS-004) | [`sbom.cdx.json`](../../sbom.cdx.json) · [`OPEN_SOURCE_LANDSCAPE.md`](../research/OPEN_SOURCE_LANDSCAPE.md) | **Inventory tự động đã có:** 458 thành phần với purl, biểu đồ phụ thuộc và SPDX, sinh từ lockfile đã ghim. Lỗ hổng cũ (`burn`/`burn-wgpu` vắng khỏi ma trận khảo sát tới 2026-07-26) nay không thể tái diễn cho dep runtime — nhưng ma trận khảo sát thủ công cho **model / dataset / asset** vẫn chưa có, và SBOM không thay thế được nó |

---

## 4. Bẫy đã biết — đọc trước khi sửa

Không lặp lại nội dung CLAUDE.md; đây là những cái tốn nhiều giờ nhất và dễ tái phát nhất.

- **Gate im lặng.** Một test target biên dịch mà không chạy test nào sẽ **exit 0**. Luôn chạy
  `check_test_targets.mjs` sau `cargo test`, và luôn truyền `--features desktop`.
- **Layout trọng số não là `w[out * fan_in + in]`** — chuyển vị so với `[d_input, d_output]` của Burn.
  Chép phẳng không chuyển vị thì mạng **vẫn chạy, vẫn ra số hữu hạn, và sai âm thầm**.
- **Mọi khoản trừ năng lượng mới phải vào `total_cost`** trong `metabolic_decay_system`. Một phép
  trừ riêng trông rất hợp lý và làm rò EU, vì chỉ `total_cost` chảy qua `respired` vào detritus.
- **`ActionGates` vắng mặt phải đọc là MỞ**, không bao giờ là đóng. Mặc định ngược lại sẽ âm thầm
  làm agent ngừng ăn.
- **Không Lamarck.** `AgentBrain.learned` không bao giờ được ghi ngược vào `.genotype`.
- **Code số học cần HAI loại test.** Gradient check bằng sai phân hữu hạn bắt được đạo hàm sai,
  nhưng **vẫn pass với một mục tiêu sai**. Phải ghép với một khẳng định về hành vi.
- **Test đọc/ghi biến môi trường phải chia sẻ một mutex.** Chúng chạy song song trong cùng tiến trình;
  xanh khi chạy riêng file, đỏ khi chạy cả suite.
- **`terrain_challenger_tests` zero-alloc từng flaky** khi bốn test trong file chạy song song
  (lệch đúng 1 allocation). Lần chạy 2026-07-26 xanh; nếu thấy đỏ, chạy riêng file trước khi
  kết luận là hồi quy.
- **Chạy `cargo` từ PowerShell, không phải Git Bash.** Đo 2026-07-26: cùng một lệnh, cùng một cây mã
  nguồn — qua Git Bash cho `587 passed, 0 failed` với **15 target chết ở `STATUS_ENTRYPOINT_NOT_FOUND`
  (0xc0000139)** trước khi chạy nổi một test; qua PowerShell cho **629 passed, 0 failed, exit 0**.
  15 target đó đúng là nhóm feature-gated nạp DLL native nặng (`migration_*`, `persistence_*`,
  `lineage_stress`, `environmental_elements_*`, `tauri_ipc`, `adversarial_challenger`): Git Bash chèn
  `/usr/bin` của MSYS2 vào `PATH` và chúng phân giải nhầm một DLL ở đó. 0xc0000139 nghĩa là "tìm thấy
  DLL, thiếu entry point mong đợi".
  **Hai ngõ cụt, ghi ra để không ai đi lại:** đây *không* phải artifact cũ — `cargo clean -p anima-engine`
  rồi dựng lại tái hiện đủ 15 crash y hệt. Và *không* phải hồi quy — cây mã nguồn khi đó giống hệt một
  `main` vừa đo xanh vài giờ trước.
- **Chạy các suite tuần tự, đừng chồng nhau.** Một bản build `cargo` chạy song song làm suite Vitest
  `tests/` báo ~28 lỗi giả: timeout render bị đọc thành "không tìm thấy DOM node", nên lỗi trông như
  một khẳng định sai chứ không như một timeout. Chạy lại lúc máy rảnh: 243 pass, và thời gian tường
  rơi từ 50,5s xuống 19,5s. Trước khi kết luận đỏ là hồi quy, hãy hỏi máy lúc đó đang chạy gì.
- **Nhiều agent có thể cùng sửa cây này.** Một `cargo check` gãy giữa chừng có thể là bản ghi dở
  của phiên khác, không phải lỗi của bạn.
- **Và hệ quả nặng hơn: công việc của phiên khác có thể đang nằm trên nhánh của bạn.** Đã xảy ra
  ngày 2026-07-26 — toàn bộ tài liệu kiểm toán (`STATE_OF_THE_PROJECT.md`, `CLAUDE.md`, `TODO.md`,
  `handoff.md`, `plan.md`) chưa commit và nằm trên `fix/temp-path-collisions`, một nhánh có PR #6
  đang mở với tiêu đề chỉ nói về temp path. **Chạy `git status` và `gh pr list` trước khi commit**,
  và kiểm nhánh hiện tại có khớp việc mình đang làm không. Một PR đúng nội dung nhưng sai tiêu đề là
  một PR không ai review đúng.
- **Phiên khác `checkout` nhánh của họ *từ nhánh của bạn*, và việc chưa commit của bạn đi theo.**
  Đây không phải biến thể của mục trên — nó là cơ chế khác và nguy hiểm hơn, vì nó xảy ra **giữa lúc
  bạn đang làm** chứ không phải trước khi bạn bắt đầu. Reflog ghi thẳng ra:
  `checkout: moving from fix/thread-supervisor-3-7 to docs/oss-010-status-reconcile`.

  Hai lần trong một phiên ngày 2026-07-26, và **cách phòng hiển nhiên không đủ**:

  1. Lần một, việc chưa commit trôi sang nhánh của phiên khác **trong lúc bốn file của họ đang
     `staged`**. Một lệnh `git commit` ở thời điểm đó sẽ quét trọn việc của họ vào commit của mình.
  2. Lần hai, tôi đã kiểm nhánh ở đầu lệnh — nhưng nhánh bị đổi **giữa lệnh kiểm và lệnh commit,
     trong cùng một khối**, nên commit rơi vào `feat/oss-070-newick-export`. Kiểm rồi tin là không đủ.

  **Cách phòng thật sự hiệu quả là guard có abort, không phải kiểm rồi tin:**

  ```powershell
  $want = "your-branch"
  $have = git branch --show-current
  if ($have -ne $want) { "ABORT: on '$have', expected '$want'"; exit 1 }
  ```

  Và **stage theo từng đường dẫn, không bao giờ `git add -A`** — đó là thứ giữ cho việc của người khác
  không bị cuốn vào ngay cả khi guard trượt.

  Nếu commit đã rơi sai chỗ: `git cherry-pick <sha>` sang nhánh đúng, rồi `git branch -f <nhánh-của-họ>
  <sha-cũ>` để trả con trỏ về. Dùng `branch -f` chứ **không** `reset --hard` — reset sẽ xoá working tree
  mà phiên khác đang viết vào. Kiểm `git ls-remote --heads origin <nhánh>` trước: nếu nhánh đó chưa
  được push và chưa có commit nào của họ thì sửa hoàn toàn sạch.

  Nếu việc của bạn đang lẫn trong tree của họ: `git stash push --include-untracked -- <đúng các đường
  dẫn của bạn>`. Có pathspec thì nó không đụng index của họ; `git stash` trần thì có.
- **Bài học đầy đủ hơn cả hai mục trên: trong checkout dùng chung, đừng dựa vào `HEAD` cho bất cứ
  việc gì.** Guard ở trên chặn được commit sai nhánh, nhưng nó không chặn được mọi thứ:

  - `git switch main` **im lặng không có hiệu lực** khi phiên khác vừa chiếm `HEAD` — không lỗi,
    không cảnh báo, và mọi lệnh sau đó chạy trên nhánh của họ.
  - `git pull --ff-only` khi đó fail với `Not possible to fast-forward` vì bạn đang trên nhánh đã
    phân kỳ của họ, chứ không phải `main`.
  - `git branch -d <nhánh>` so với **remote-tracking ref**, không phải với `main`, rồi cảnh báo
    "not yet merged to HEAD". Nó vẫn xoá. Cái thật sự bảo vệ bạn là kiểm bằng ref tường minh trước:
    `git log origin/main..<nhánh>` phải trống, hoặc `git merge-base --is-ancestor <sha> origin/main`.

  Nên: **dùng ref tường minh (`origin/main`, sha) cho mọi phép kiểm**, và khi cần một `HEAD` mà
  không ai giành được, dùng **worktree** thay vì `git switch`:

  ```powershell
  git worktree add -b <nhánh-của-bạn> <đường-dẫn-tạm> origin/main
  # ... làm việc, commit, push trong đó ...
  git worktree remove <đường-dẫn-tạm>
  ```

  Worktree có `HEAD` và working tree riêng, nên nó **không** chạm checkout dùng chung — không kéo theo
  việc chưa commit của phiên khác, và không đặt commit của bạn lên nhánh của họ. Mục §4 này được viết
  chính bằng cách đó, sau khi `git switch` thất bại trong im lặng hai lần.

---

## 5. Lệnh xác minh đầy đủ

Chạy từ repo root, trừ chỗ ghi khác. Đây là đúng bộ CI chạy.

```bash
npm run lint && node scripts/eslint_ratchet.mjs && npm run typecheck:tests && npm run test && npm run test:frontend && npm run build && node scripts/check_docs_links.mjs
```

```bash
cd src-tauri && cargo fmt --check && cargo clippy --all-targets --features desktop -- -D warnings && cargo test --features desktop
```

Skill [`verify-anima`](../../.claude/skills) gói backend + frontend + build vào một lượt.
