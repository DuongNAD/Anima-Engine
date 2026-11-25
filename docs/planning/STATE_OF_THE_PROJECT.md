---
title: Trạng thái dự án và việc cần làm tiếp
status: active
owner: maintainers
last_reviewed: 2026-11-25
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

## 1. Bảng bằng chứng có thẩm quyền

> ## ⚠️ Đọc trước — bảng 2026-07-27 dưới đây đã cũ; số mới nhất là [§1.i](#1i-đo-2026-11-18-tại-46e25d5--lượt-đo-lại-đầy-đủ-sau-đợt-2-trên-windows-11-x64-thẻ-r0)
>
> Hai cây mã đã được **hợp nhất ngày 2026-11-16**: `main` được fast-forward lên thân cây
> `feat/ecological-selection` (a43872c → bb78e63; `main` là tổ tiên trực tiếp, 0 commit riêng). Thân
> cây vẫn giữ tên và các PR vẫn nhắm vào nó; `main` được fast-forward theo nó khi chủ dự án cho phép.
>
> **Status sync (2026-11-25, `D1. STATE-SYNC r4`):** `main` was most recently synced with `feat/ecological-selection`
> at `36234e47` on `2026-11-20` (`#184`, `#189`); the active trunk `feat/ecological-selection` is at
> `9a53bad6` (merged since `r3` `#243`: `#242`, `#244–#273`, through `2026-11-25`). Stage 4 passed via `E3b`
> (`#252`, `6d0fdeb2`), `QD-SCALE-TRIAL` (`#255`) sets `~100` animals as the trial-island target, `WV2-3`/`WV2-4`/`WV2-5`/`WV2-PHYS2`/`MAP-D5` (`#242`, `#249`, `#265`, `#270`, `#273`), `ADR-0014` (`#256`), `I1`/`I2` (`#266`, `#272`), `ADR-0015` (`#268`), `ADR-0016` (`#271`), `ADR-0018` (`#263`), `ADR-0019` (`#259`), `D20` (`#257`), `SEL2-P2 VIAB-2` (`#258`), `C12 E3B-PIN` (`#260`), accepted `ADR-0009`/`ADR-0013` (`#262`), `C15. HOTLOOP-PERSIST` (`#264`), `C14. HONEST-UI` (`#269`), `D21. DOCS-CLEANUP` (`#267`), and root-allowlist cleanup (`#261`) are merged. This sync updated
> the §1 banner, [§3.00](#300-việc-kế-tiếp--cập-nhật-2026-11-17) stage board and done lists, and §4 dated traps; the newest
> authoritative full measurement section remains
> **[§1.i](#1i-đo-2026-11-18-tại-46e25d5--lượt-đo-lại-đầy-đủ-sau-đợt-2-trên-windows-11-x64-thẻ-r0)**
> (`2026-11-18`, `46e25d5`, card `R0`). Work merged after `46e25d5` is indexed by PR number and source
> document under §3.00 and has not yet been re-measured into a new `§1.x` table (Rule 8: no unmeasured
> counts are copied here).
>
> Lượt đo mới nhất là **§1.i** ngay dưới (2026-11-18, `46e25d5`: thẻ R0, lượt đo lại đầy đủ trên Windows 11
> x64, kiểm chéo với CI): đó là số có thẩm quyền, đọc nó trước. §1.h (2026-11-17, `6f14124`), §1.g (2026-11-16, `bb78e63`),
> bảng 2026-07-27 (§1.a–§1.e) và §1.f (2026-10-16) là **lịch sử** của từng thời điểm, không phải
> trạng thái hôm nay.
>
> Ngoài số đo, bug đã chứng minh nằm ở [`FINDINGS.md`](../../FINDINGS.md) (mục mới nhất ở trên cùng).
> Căn cứ của quyết định hợp nhất (QĐ-A: `feat/ecological-selection` là nền) ở
> [bề mặt hợp nhất, đo 2026-11-01](../ai/planning/2026-11-01-merge-surface-measurement.md); lượt đo
> 2026-10-31 trên hai cây còn ở [`COMPLETION_ROADMAP_2026-10-31.md` §1](COMPLETION_ROADMAP_2026-10-31.md#1-đo-hôm-nay)
> (đọc cả §4 của nó: QĐ-D và QĐ-E đã đổi vài hàng).
>
> Banner **không** chép số nào — nó chỉ trỏ (quy tắc 8, "một con số chỉ có một nhà").

> ## 📌 Đây là **bảng số duy nhất có thẩm quyền** của dự án.
>
> Mọi tài liệu khác **trỏ về đây** thay vì chép lại một con số dễ ôi. Nếu bạn thấy một con số
> pass/fail/warning/target ở file khác mà không kèm lệnh + ngày, nó là **lịch sử**, không phải
> trạng thái — xem [quy ước phân loại](#11-phân-loại-mọi-con-số-trong-tài-liệu).
>
> **Lần xác minh gần nhất: 2026-07-27**, worktree `.worktrees/feature-anima-completion`, nhánh
> `feature-anima-completion`, tại commit **`068a750`** — commit **merge** đưa
> `feat/oss-071b-live-tracker` vào nhánh này. Đây là một lượt chạy **toàn bộ** gate CI **sau merge**,
> không phải chạy chọn lọc, và **không** tái sử dụng kết quả nào từ trước merge. Toolchain:
> rustc/cargo **1.95.0**, Playwright **1.62.0**. Mọi hàng dưới đây đến từ **cùng một lượt chạy đó**,
> trừ đúng một hàng còn ⏳ và đã nói rõ lý do.
>
> Mọi con số ở lượt này **trùng với lượt đo tại `1cddeb5`**, và điều đó là kết quả mong đợi chứ không
> phải dấu hiệu chép lại: merge chỉ đổi một file tài liệu, nên không có hành vi đo được nào thay đổi.
> Số ở đây được **đo lại**, không mang sang.
>
> **Sau `068a750`, PR #21 nhận thêm ba commit và đã merge vào `main` tại `a053083`.** Không commit nào
> chạm mã Rust hay mã ứng dụng: hai commit sửa **cấu hình chạy test** (`72a6e34`, `0f5b4d3`) và một
> commit sửa tài liệu (`b744df0`). Vì vậy §1.a–§1.c vẫn mô tả đúng cây mã hôm nay; cái đổi là **cách
> gate được chạy**, và cái đó được đo riêng ở [§1.e](#1e-sau-pr-21--đo-2026-07-27-tại-0f5b4d3). Đọc
> §1.e trước khi trích một hàng frontend hay E2E ở §1.b.

Mỗi hàng ghi **lệnh đã chạy**, **ngày chạy**, **cấu hình**, và **loại khẳng định**:

- 📏 **Đo** — kết quả của lệnh ở cột bên cạnh, chạy đúng ngày ghi trong hàng.
- 📋 **Chính sách** — một khoản miễn trừ đã được ghi thành quyết định, không phải một phép đo.
- 🔧 **Đã ship** — mã/công cụ tồn tại và có test. **Không** đồng nghĩa "đã có số đo".
- ⏳ **Chưa chạy lại trong gói này** — số gần nhất còn hiệu lực, kèm ngày và lý do không chạy lại.

### 1.i Đo 2026-11-18 tại `46e25d5` — lượt đo lại đầy đủ sau Đợt 2 trên Windows 11 x64 (thẻ R0)

> Đây là **số mới nhất có thẩm quyền**; `§1.h` trở thành lịch sử. Mọi hàng là **cùng một commit**
> `46e25d5585884cb2a1e2c51ef15a5aa18535ec4d` trên thân cây `feat/ecological-selection` (sau khi
> Đợt 2 đóng và PR `#117` S9 merge). Khác với `§1.h`, commit này **chưa** được fast-forward lên
> `main`, và `.github/workflows/ci.yml` chỉ kích hoạt trên `push: branches: [main]`, `pull_request` và
> `workflow_dispatch`, nên `gh run list --branch feat/ecological-selection --workflow ci.yml` trả **0
> run** cho `46e25d5`. Toàn bộ số liệu dưới đây đến từ một nguồn duy nhất:
>
> - **Lượt đo cục bộ trên Windows 11 Pro x64** (`10.0.26300`, Intel Core i5-14600KF 14 nhân / 20
>   luồng, 47,8 GB RAM, `CARGO_BUILD_JOBS=6`): `rustc 1.98.1`, `cargo 1.98.1`, `clippy 0.1.98`,
>   `cargo-audit 0.22.2`, `cargo-deny 0.20.2`, `cargo-modules 0.28.0`, Node `v22.23.2`, npm `10.9.8`,
>   Python `3.11.9`, sau `npm ci` sạch ở cả hai gói (`root` và `tests/`). Mọi lệnh `cargo` chạy tuần
>   tự từng tiến trình một (không song song), `cargo test` chạy qua `cmd /c` để giữ nguyên thứ tự hai
>   luồng stdout/stderr, và `cargo build --release --features desktop` được build trước `npm run test:e2e`.
>
> **Kết quả tổng quan:** Backend (`desktop` và `default`), `audit`/`deny`, toàn bộ giấy phép/SBOM/flora,
> đồ thị module, 6+6 lượt tái lập, release build, Playwright E2E, `lint`/`eslint_ratchet`, `typecheck:tests`,
> `npm run test`, `build`/`bundle`/`csp`, `check:e2`, `check:audit`, `check:root-hygiene`,
> `check:text-hygiene`, `check:docs` và `check:ipc` đều **xanh**. Duy nhất một cổng **đỏ** trên
> Windows: `npm run test:frontend` thất bại 1/700 test tại `tests/frontend/map2/map2ManifestEvidence.test.ts:50`
> do `git` trên Windows (`core.autocrlf=true`) chuyển `artifacts/maps/map2/map2_manifest.json` sang CRLF
> khi checkout vì `.gitattributes` chưa ghim `artifacts/maps/**`.
>
> **Lượt này không phủ:** ứng dụng desktop (`tauri:dev`); chạy trực tiếp `ubuntu-latest` (CI không có run
> cho nhánh thân cây `46e25d5`); vòng lặp `hardening` 5 lượt `RAYON_NUM_THREADS=1 cargo test --release`
> trên Linux; hiệu năng FPS thực tế.

| Loại | Gate | Lệnh | Cục bộ Windows 11 x64 (`2026-11-18`, `46e25d5`) | CI (`feat/ecological-selection` @ `46e25d5`) |
|---|---|---|---|---|
| 📏 | Format | `cargo fmt --check` | success (exit 0) | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Advisory Rust | `cargo audit` · `cargo deny check` | success — quét `773` crate với `1527` advisory; 2 cảnh báo *yanked* được cho phép (`async-executor`, `chacha20`); deny: `advisories ok, bans ok, licenses ok, sources ok` | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Lint backend | `cargo clippy --all-targets --features desktop -- -D warnings` | success — `Finished` sau `1m 40s`, không cảnh báo | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Tách feature (G2 #2) | `cargo clippy --all-targets --no-default-features -- -D warnings` + `cargo tree --no-default-features -e normal` | success — `Finished` sau `35.00s`, `gated crates present in a default build: 0` | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Parity binding | `cargo test --lib export_bindings` rồi `git diff --exit-code -- ../src/types/generated` | success — `50 passed; 0 failed; 0 ignored; 508 filtered out`, `git diff` exit 0 | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Backend test (`desktop`) | `cmd /c "cargo test --features desktop > ..\out-desktop.txt 2>&1"` | **1479 passed · 0 failed · 2 ignored**, `150` dòng `test result` (`1124.7s`) | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Backend test (`default`) | `cmd /c "cargo test > ..\out-default.txt 2>&1"` | **1453 passed · 0 failed · 2 ignored**, `142` dòng `test result` (`861.2s`); kém `desktop` đúng 26 test (8 file feature-gated = 25 + 1 test trong `gpu_brain_batch_parity_tests`) | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏📋 | Chính sách target | `node scripts/check_test_targets.mjs out-desktop.txt --profile desktop` · `--profile default` | success — `desktop`: `test targets: 150, empty: 3 (allow-listed 3), ignored: 2 (allow-listed 2), feature-gated running under "desktop": 8`; `default`: `test targets: 142, empty: 3, ignored: 2, feature-gated: 0` | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Licence · SBOM · schema · NOTICE · flora | `check:licenses` / `check:sbom` / `check:sbom-schema` / `check:notice` / `check:flora-footprint` | success cả năm — `443` thành phần phân phối, `266` văn bản, `1` chưa giải quyết · `461` thành phần, `462` bản ghi phụ thuộc · CycloneDX 1.5 hợp lệ · NOTICE: `422` crate, `21` npm phân phối, `18` cài-không-ship, `1` chưa giải quyết · `7` loại thực vật đặc, drift lớn nhất `3.9e-5` | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Đồ thị module | `cargo modules dependencies --lib --no-externs --package anima-engine` + `verify_dataflow.py` | success — `cargo-modules 0.28.0`, `16` cạnh khớp hoàn toàn; 6 cặp vòng lặp SMELL không đổi | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Tái lập `live_experiment_tests` | `cargo test --features desktop --test live_experiment_tests` (6 tiến trình riêng) | success — 6/6 tiến trình **17 passed · 0 failed · 0 ignored** (wall `9.80s`–`18.43s`) | không có trong `ci.yml` |
| 📏 | Checksum fork 180 tick | `cargo test --features desktop --test live_experiment_tests a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards -- --nocapture` (6 tiến trình riêng) | success — **6/6 tiến trình khớp nhau tuyệt đối**: `uninterrupted: 3717949667, control: 3717949667, treatment: 3828929454` | không có trong `ci.yml` |
| 📏 | Advisory npm (policy) | `npm run check:audit` (`scripts/check_npm_audit.mjs`) | success — root `0 critical, 7 high, 8 moderate`, `tests/`: `0 critical, 6 high`; `0` trong gói ship; `1` miễn trừ dev (`picomatch`) xác minh qua `--omit=dev`; self-test `5` control OK | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Vệ sinh thư mục gốc | `npm run check:root-hygiene` | success — `64` mục ở gốc, đều nằm trong `scripts/root_allowlist.json`; self-test `4` control OK | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Vệ sinh văn bản | `npm run check:text-hygiene` | success — `968` file text theo dõi trên `46e25d5`, 0 byte điều khiển thô | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Artifact E2 | `npm run check:e2` | success — tính lại `264` delta analysis và `22` smoke, `OK` | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Lint frontend | `npm run lint` · `node scripts/eslint_ratchet.mjs` | **0 error, 0 warning** (baseline `0`) | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Typecheck `tests/` | `npm run typecheck:tests` | success (exit 0) | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Test `src/**` | `npm run test` | **14 file · 109 passed** (`7.89s`) | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Test `tests/**` | `npm run test:frontend` | 🔴 **failure (exit 1) — 67 passed \| 1 failed (68 file) · 699 passed \| 1 failed (700 test)**: `frontend/map2/map2ManifestEvidence.test.ts:50` hỏng so chuỗi nguyên byte vì Windows checkout CRLF | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Build + artifact | `npm run build` · `check:bundle-closure` · `check:csp` · `check:bundle` | success — `838` module (`353ms`), `21` gói có byte trong `dist/` (`3` toolchain-injected); `3` HTML ship, `0` origin ngoài, `0` inline script; `25` chunk, **1890.1 / 2000 KiB** | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Danh sách spec E2E | `node scripts/check_e2e_specs.mjs` | success — `10` file spec khớp danh sách cam kết; `2` loại trừ có lý do | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Release binary + E2E Playwright | `cargo build --release --features desktop` rồi `npm run test:e2e` | success — release build `3m 01s` (`181.5s`), E2E **288 passed** (`1.2m`, 10 worker) | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Link & metadata tài liệu | `npm run check:docs` (`check_docs_links` + `check_docs_metadata` + `check_ipc_reference`) | success — `1236` link + `93` anchor / `182` file, `0` gãy · `125` file, `0` lỗi, `2` review quá hạn, `19` negative control OK | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | IPC | `check_ipc_reference.mjs` (trong `check:docs`) · `npm run check:ipc` | **35 command · 7 event · 0 khác biệt** · `37` call site `invoke`, `14` khoá tham số khớp hết | chưa chạy trên thân cây (0 run cho `46e25d5`) |
| 📏 | Hardening cục bộ | `cargo test --release --test determinism_tests` · grep `rand::thread_rng()` trong `src-tauri/src` | success — **5 passed** (`0.21s`); `thread_rng calls in src-tauri/src: 0` *(vòng 5 lượt `RAYON_NUM_THREADS=1` của `hardening.yml` không chạy cục bộ)* | chưa chạy trên thân cây (0 run cho `46e25d5`) |

**Kiểm chéo của phiên điều phối** (2026-11-18; báo cáo đầy đủ của R0 ở [`remeasure-2026-11-18-46e25d5.md`](../ai/testing/remeasure-2026-11-18-46e25d5.md)): job Rust của CI cho PR #122 (run `44238226269`, `windows-latest`, thân cây `071e43c` có `src-tauri/` trùng `46e25d5`) báo `152` dòng `test result`, `1529` đạt, `0` hỏng, `2` bỏ qua, `test targets: 150`. `1529` = `1479` của R0 cộng `50` test `export_bindings` mà CI chạy ở một bước riêng. Cổng đỏ duy nhất là báo động giả trên Windows: `.gitattributes` chưa ghim `artifacts/maps/**`; PR #125 ghim nó. Chưa đo lại trên Windows sau bản sửa.

### 1.h Đo 2026-11-17 tại `6f14124` — CI của lần fast-forward `main` và một lượt chạy cục bộ macOS

> Đây là **số mới nhất có thẩm quyền**; §1.g trở thành lịch sử. Mọi hàng là **cùng một commit**
> `6f14124d4a5cc5406c799b99bcd485a784baa390`: thân cây `feat/ecological-selection` sau #92, và cũng là
> `main` sau lần fast-forward 2026-11-17 (`8a3558a..6f14124`, 32 commit, không force). Hai nguồn độc lập:
>
> - **CI GitHub** do chính lần push `main` kích hoạt: run `44012998030` (workflow `CI`: job Rust trên
>   `windows-latest`, job Frontend trên `ubuntu-latest` với Node 22.23.4) và run `44012998040`
>   (workflow `hardening`, job `determinism`). Số chép từ log, không tính lại gì ngoài tổng các dòng
>   `test result:`.
> - **Lượt chạy cục bộ trên macOS arm64** (10 nhân): rustc/cargo **1.98.1**, Node **24.19.0**, npm
>   **11.17.0**, Playwright **1.62.0**, sau `npm ci` sạch ở cả hai gói. Mỗi lệnh chạy **một lần**, theo
>   thứ tự của `ci.yml`, trên cache build Rust có sẵn (không phải build sạch).
>
> **Kết quả: xanh ở cả hai nguồn.** CI: run `44012998030` success (job Rust 03:50–04:28 UTC, job
> Frontend 03:50–03:57 UTC) và run `hardening` `44012998040` success (03:50–04:10 UTC). Cục bộ: mọi gate
> xanh, trừ đúng một gate không có nghĩa trên macOS (`check:notice`, xem hàng của nó). Backend test cho
> **cùng một tổng** trên Windows và macOS: 1437 passed · 0 failed · 2 ignored trên 143 target.
>
> **Lượt này không phủ:** ứng dụng desktop (`tauri:dev`; không có lượt chạy app thật nào); binary
> ứng dụng release (`cargo build --release --features desktop`); các PR đang mở #56 và #68; hiệu năng. Cục bộ **không chạy** các bước licence/SBOM/flora, đồ
> thị module và hardening; với chúng chỉ có cột CI.

| Loại | Gate | Lệnh | CI (`44012998030` / `44012998040`) | Cục bộ macOS |
|---|---|---|---|---|
| 📏 | Format | `cargo fmt --check` | success | success |
| 📏 | Advisory Rust | `cargo audit` · `cargo deny check` | success — quét 773 crate với 1509 advisory; 2 cảnh báo *yanked* được cho phép (`async-executor`, `chacha20`); deny: `advisories ok, bans ok, licenses ok, sources ok` | success — quét 773 crate với 1509 advisory; 2 cảnh báo *yanked* được cho phép (`async-executor`, `chacha20`); deny: `advisories ok, bans ok, licenses ok, sources ok` |
| 📏 | Lint backend | `cargo clippy --all-targets --features desktop -- -D warnings` | success — `Finished` sau 45,75 s, không cảnh báo (rustc/cargo 1.98.1, clippy 0.1.98) | success, không cảnh báo |
| 📏 | Tách feature (G2 #2) | `cargo clippy --all-targets --no-default-features -- -D warnings` + `cargo tree --no-default-features -e normal` | success — `gated crates present in a default build: 0` | success — 0 crate bị gate lọt vào bản build mặc định. Cargo in một báo cáo *future-incompat* cho `block v0.1.6`, crate chỉ có trên macOS; đó không phải cảnh báo clippy và không làm đỏ |
| 📏 | Parity binding | `cargo test --lib export_bindings` rồi `git diff --exit-code -- ../src/types/generated` | success — 50 test `export_bindings` passed, `git diff` không lệch | 50 test `export_bindings` pass trong lượt `cargo test` dưới đây; cây làm việc sạch sau lượt đó |
| 📏 | Backend test | `cargo test --features desktop -- --nocapture` | **1437 passed · 0 failed · 2 ignored**, 143 dòng `test result` (bước build test 4m 58s) | **1437 passed · 0 failed · 2 ignored**, 143 dòng `test result` (12 phút 9 giây) |
| 📏📋 | Chính sách target | `check_test_targets.mjs` (CI gọi không có `--profile`; cục bộ gọi với `--profile desktop`) | success — `test targets: 143, empty: 3 (allow-listed 3), ignored: 2 (allow-listed 2)`; log ghi rõ không có `--profile`, nên việc lên lịch target feature-gated **không** được kiểm ở CI (cột cục bộ kiểm nó) | success — `test targets: 143, empty: 3 (allow-listed 3), ignored: 2 (allow-listed 2), feature-gated running under "desktop": 8` |
| 📏 | Licence · SBOM · schema · NOTICE · flora | `check:licenses` / `check:sbom` / `check:sbom-schema` / `check:notice` / `check:flora-footprint` | success cả năm — 443 thành phần phân phối, 266 văn bản, 1 chưa giải quyết · 461 thành phần, 462 bản ghi phụ thuộc · CycloneDX 1.5 hợp lệ · NOTICE: 422 crate, 21 gói npm phân phối, 18 cài-không-ship, 1 văn bản chưa giải quyết · 7 loại thực vật đặc, drift lớn nhất 3,9e-5 | Chỉ chạy `check:notice` (`gen_notice.mjs --check`): **đỏ, và đó là thiết kế, không phải NOTICE cũ**. `cargo tree` phân giải theo target của máy chạy (`ci.yml` ghi rõ các artifact này mô tả bản build Windows), nên trên macOS nó ra 398 crate thay vì 422: có thêm các crate `core-foundation`/`core-graphics`, thiếu các crate Windows/Vulkan như `d3d12` và `ash`. File đã được khôi phục, không commit gì. Gate này chỉ có nghĩa ở cột CI |
| 📏 | Đồ thị module | so đồ thị phụ thuộc với `dataflow.json` | success — 16 cạnh khớp hoàn toàn với `cargo-modules` 0.28.0; 6 cặp phụ thuộc vòng vẫn được ghi là SMELL, không phải lỗi (cùng 6 cặp như §1.g) | không chạy |
| 📏 | Advisory npm | `npm audit --audit-level=high` root và `--prefix tests` | success ở ngưỡng `high`: root **9 moderate** (chuỗi `minimatch` → `@typescript-eslint`), 0 high; `tests/`: `found 0 vulnerabilities` | như CI |
| 📏 | Vệ sinh văn bản | `check:text-hygiene` | success — 929 file đã track, không có byte điều khiển thô | như CI |
| 📏 | Artifact E2 | `npm run check:e2` | success — tính lại 264 delta per-seed (analysis) và 22 (smoke), mọi artifact khớp preregistration | như CI |
| 📏 | Lint frontend | `npm run lint` + `eslint_ratchet.mjs` | **0 error, 0 warning** (baseline 0) | như CI |
| 📏 | Typecheck `tests/` | `npm run typecheck:tests` | success | success |
| 📏 | Test frontend | `npm run test` · `npm run test:frontend` | 14 file · **109 passed** · 67 file · **692 passed** | như CI |
| 📏 | Build + artifact | `npm run build` · `check:bundle-closure` · `check:csp` · `check:bundle` | success — 21 gói có byte trong `dist/` (3 do toolchain chèn); 3 HTML ship, 0 origin ngoài, 0 inline script; 25 chunk, **1905,8 / 2000 KiB** | như CI |
| 📏 | Danh sách spec E2E | `node scripts/check_e2e_specs.mjs` | 10 file spec được lên lịch khớp danh sách đã commit; 2 file loại trừ có lý do | như CI |
| 📏 | E2E Playwright | `npm run test:e2e` | **292 passed** (2,5 phút) | **292 passed** (1,2 phút), 5 worker |
| 📏 | Link tài liệu | `node scripts/check_docs_links.mjs` | 1021 link tương đối + 83 anchor cùng trang trong 173 file, 0 gãy | như CI |
| 📏 | Metadata tài liệu | `node scripts/check_docs_metadata.mjs` | 113 file, 0 lỗi, 15 review quá hạn (chỉ báo, không làm đỏ); 19 negative control OK | như CI |
| 📏 | IPC | `check_ipc_reference.mjs` · `npm run check:ipc` | **35 command · 7 event · 0 khác biệt** (cục bộ thêm `check:ipc`: 39 call site `invoke`, 14 khoá tham số, khớp hết) | như CI |
| 📏 | Hardening — tất định | `cargo test --release` rồi 5 lượt `RAYON_NUM_THREADS=1` | success, trên rustc **1.100.0** (`dtolnay/rust-toolchain@stable`, khác bản ghim 1.98.1 của job Rust). `cargo test --release` (không `--features desktop`, nên 135 target thay vì 143): 1411 passed · 0 failed · 2 ignored; 5 lượt ổn định: 675 dòng `test result`, 7055 passed, 0 failed | không chạy |
| 📏 | Hardening — golden hash | `cargo test --release --test determinism_tests` | success — 5 passed (gồm `determinism_hash_ghim`, `world_hash_bat_duoc_thay_doi_that`, `determinism_200_ticks`, `live_schedule::the_live_checksum_distinguishes_one_tick_from_many`). **Log không in giá trị hash.** Bước grep `rand::thread_rng()` dưới `src-tauri/src` cũng success | không chạy |

Hai cột chỉ khác nhau ở môi trường, không ở commit: một hàng đỏ ở một cột và xanh ở cột kia là một
khác biệt **nền tảng**, cần đọc trước khi coi là bug (như hàng NOTICE). Từ §1.g đến đây số test tăng
vì các PR #59–#92 thêm test, không phải vì đổi cách đếm: muốn biết PR nào thêm test nào thì đọc PR đó,
đừng suy từ hiệu hai con số.

### 1.g Đo 2026-11-16 trên thân cây `bb78e63` — CI của lần fast-forward `main`

> Đây là **số mới nhất có thẩm quyền**, thay cho các số của banner đầu §1 trỏ tới. Nó là kết quả của
> **một lần CI GitHub** trên đúng commit `bb78e63d93c5ad2c680427bacc7209ce201c8f48`, được kích hoạt bởi
> chính lần push fast-forward `main` lên thân cây `feat/ecological-selection` (2026-11-16). Số được
> chép từ log của hai run, không tính lại gì ngoài tổng các dòng `test result:`.
>
> - **Rust (`windows-latest`): xanh.** Mọi bước success.
> - **Frontend (`ubuntu-latest`): đỏ, và chỉ đỏ ở đúng một bước** — `check:e2`
>   ("E2 artifacts match the committed preregistration"). Mọi bước còn lại của job, kể cả lint,
>   hai suite Vitest, build, CSP, bundle, Playwright, docs và IPC, đều success. Bản sửa là PR #59
>   (`fix/e2-amendment-1-rerun`: sửa preregistration có ghi nhận + chạy lại thật tập mẫu E2); **tại thời
>   điểm viết, PR #59 chưa merge** nên lượt đo này không gồm nó.
> - **Workflow `hardening` (job `determinism`): xanh.**
>
> **Lượt này không phủ:** chạy cục bộ trên macOS; ứng dụng desktop (`tauri:dev`, không có lượt chạy app
> thật nào); các PR đang mở #47–#50 và #59; `cargo audit`/`npm audit` chỉ ở mức CI cho phép (xem hàng
> audit). Toolchain Rust trong CI được ghim: rustc/cargo **1.98.1**, clippy **0.1.98**; Node **22.23.4**
> (job Frontend). Log của job `determinism` **không in** phiên bản rustc (dùng `dtolnay/rust-toolchain@stable`).

| Loại | Gate | Lệnh/Nguồn | Kết quả |
|---|---|---|---|
| 📏 | Toàn run | Actions run `43817056729`, workflow `CI`, event `push` vào `main`, 2026-11-16 | **failure** — job Rust success (06:31–07:02 UTC), job Frontend failure (06:30–06:35 UTC), bước đỏ duy nhất là `check:e2` |
| 📏 | Format | `cargo fmt --check` (run `43817056729`, 2026-11-16) | success |
| 📏 | Advisory Rust | `cargo audit` · `cargo deny check` (cùng run) | success — quét 773 crate với 1509 advisory; chỉ 2 cảnh báo *yanked* được cho phép (`async-executor`, `chacha20`); deny: `advisories ok, bans ok, licenses ok, sources ok` |
| 📏 | Lint backend | `cargo clippy --all-targets --features desktop -- -D warnings` (cùng run) | success — `Finished` sau 2m 03s, không cảnh báo |
| 📏 | Tách feature (G2 #2) | `cargo clippy --all-targets --no-default-features -- -D warnings` + `cargo tree --no-default-features -e normal` (cùng run) | success — `gated crates present in a default build: 0` |
| 📏 | Parity binding | `cargo test --lib export_bindings` rồi `git diff --exit-code -- ../src/types/generated` (cùng run) | success — 48 test `export_bindings` passed, `git diff` không lệch |
| 📏 | Backend test | `cargo test --features desktop -- --nocapture` (cùng run, Windows) | **1306 passed · 0 failed · 2 ignored**, 123 dòng `test result` (tổng các dòng `test result:`) |
| 📏📋 | Chính sách target | `node ../scripts/check_test_targets.mjs ../cargo-test-output.txt` (cùng run) | success — `test targets: 123, empty: 3 (allow-listed 3), ignored: 2 (allow-listed 2)`. Log ghi rõ CI gọi **không có `--profile`**, nên việc lên lịch target feature-gated **không** được kiểm trong run này |
| 📏 | Licence · SBOM · schema · NOTICE · flora | `check:licenses` / `check:sbom` / `check:sbom-schema` / `check:notice` / `check:flora-footprint` (cùng run) | success cả năm — 443 thành phần phân phối, 266 văn bản, 1 chưa giải quyết · 461 thành phần, 462 bản ghi phụ thuộc · CycloneDX 1.5 hợp lệ · 422 crate, 21 gói npm phân phối, 18 cài-không-ship, 1 văn bản chưa giải quyết · 7 loại thực vật đặc, drift lớn nhất 3,9e-5 |
| 📏 | Đồ thị module | `check` đồ thị phụ thuộc so với `dataflow.json` (cùng run) | success — 16 cạnh khớp hoàn toàn với `cargo-modules`; 6 cặp phụ thuộc vòng được ghi là SMELL, không phải lỗi (`ai`↔`core`, `ai`↔`evolution`, `ai`↔`physics`, `commands`↔`core`, `core`↔`evolution`, `core`↔`physics`) |
| 📏 | Advisory npm | `npm audit --audit-level=high` root và `--prefix tests` (cùng run) | success ở ngưỡng `high`: root báo **9 moderate** (chuỗi `minimatch` → `@typescript-eslint`), 0 high; `tests/`: `found 0 vulnerabilities` |
| 📏 | Vệ sinh văn bản | `check:text-hygiene` (cùng run) | success — 829 file đã track, không có byte điều khiển thô |
| 📏 | **Artifact E2** | `npm run check:e2` → `node scripts/verify_e2_artifacts.mjs` (cùng run, bước "E2 artifacts match the committed preregistration") | 🔴 **failure, exit 1, 4 FAIL**, cả bốn cùng một lỗi — manifest mà lượt chạy đã dùng không phải file đã commit ("The run used a manifest that is not the committed file"): `analysis: manifests/e2-control-shared-brain.json` (sha256 lượt chạy `b5c68207…`, file commit `30b86653…`), `analysis: manifests/e2-treatment-evolved-brain.json` (`b5618a58…` so với `61b87222…`), `smoke: manifests/e2-smoke-control-shared-brain.json` (`9513382f…` so với `86113ca9…`), `smoke: manifests/e2-smoke-treatment-evolved-brain.json` (`03c9d4e5…` so với `78a4f960…`). Trước đó có hai dòng `note`: tính lại 132 delta per-seed (analysis) và 11 (smoke). Chỉ chép 8 ký tự đầu mỗi hash; hash đầy đủ ở log run `43817056729` |
| 📏 | Lint frontend | `npm run lint` + `eslint_ratchet.mjs` (cùng run) | **0 error, 0 warning** (baseline 0) |
| 📏 | Typecheck `tests/` | `npm run typecheck:tests` (cùng run) | success |
| 📏 | Test frontend | `npm run test` · `npm run test:frontend` (cùng run) | 14 file · **109 passed** · 61 file · **648 passed** |
| 📏 | Build + artifact | `npm run build` · `check:bundle-closure` · `check:csp` · `check:bundle` (cùng run) | success — 21 gói có byte trong `dist/` (3 do toolchain chèn); 3 HTML ship, 0 origin ngoài, 0 inline script; 24 chunk, **1852,5 / 2000 KiB** |
| 📏 | E2E Playwright | `npm run test:e2e` (cùng run) | **270 passed** (38,9 s), 2 worker |
| 📏 | Link tài liệu | `node scripts/check_docs_links.mjs` (cùng run) | 974 link / 155 file, 0 gãy |
| 📏 | Metadata tài liệu | `node scripts/check_docs_metadata.mjs` (cùng run) | 107 file, 0 lỗi, 14 review quá hạn (chỉ báo, không làm đỏ); 19 negative control OK |
| 📏 | IPC | `node scripts/check_ipc_reference.mjs` (cùng run) | **34 command · 7 event · 0 khác biệt** |
| 📏 | Hardening — tất định | Actions run `43817057094`, workflow `hardening`, job `determinism`, 2026-11-16 | success (06:36–06:51 UTC). `cargo test --release` rồi bước "kiểm ổn định (5 lần)" (`RAYON_NUM_THREADS=1`, 5 lượt) đều success: 575 dòng `test result` trong 5 lượt, tổng 6400 passed, 0 dòng có test fail |
| 📏 | Hardening — golden hash | `cargo test --release --test determinism_tests` (cùng run) | success — 3 passed (`determinism_hash_ghim`, `world_hash_bat_duoc_thay_doi_that` và một test nữa). **Log không in giá trị hash**, chỉ in kết quả test |
| 📏 | Hardening — nguồn phi tất định | grep `rand::thread_rng()` dưới `src-tauri/src` (cùng run) | success; log không in con số |

**Không tìm thấy trong log:** phiên bản rustc của job `determinism`; giá trị golden hash; tổng thời lượng từng suite ngoài các mốc giờ trong bảng; số lần lặp lại ổn định ở mức từng test.

> **Ghi chú 2026-11-16 (cuối ngày) — sau §1.g, chưa phải một lượt đo mới.** Bước `check:e2` đỏ ở §1.g đã
> được sửa bởi PR #59 (preregistration có sửa đổi và chạy lại thật tập mẫu E2; kết quả ở đó, không chép
> lại). Cùng ngày thân cây còn nhận #46–#51, #54, #55, #57, #58, #60, #62–#65 và #67, tức hạ tầng cổng
> (G2, G1, G7), ba bản sửa tất định/sinh sản (F-037…F-041 trong [`FINDINGS.md`](../../FINDINGS.md)) và
> phần lớn thẻ "thế giới đọc được". **Không có lượt CI toàn bộ nào được ghi lại sau §1.g trong tài liệu
> này**, nên §1.g vẫn là bảng số gần nhất; số mới phải đo lại, không suy từ danh sách PR. *(Lượt đo lại
> đó là [§1.h](#1h-đo-2026-11-17-tại-6f14124--ci-của-lần-fast-forward-main-và-một-lượt-chạy-cục-bộ-macos), 2026-11-17.)* Việc tiếp theo và
> chướng ngại chính của nhánh khoa học (F-042) ở [§3.00](#300-việc-kế-tiếp--cập-nhật-2026-11-17).

#### Sau §1.g — `check:e2` sau PR #59 (QĐ-E2M = B), đo 2026-11-16

> Hàng `check:e2` đỏ ở §1.g đã được PR #59 sửa: thêm khối đính chính `amendment_1_live_dormant_eu`
> và chạy lại thật toàn bộ ensemble E2 (xem QĐ-E2M ở
> [roadmap 31/10 §4](COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được)).
> Bảng này đo **sau** merge đó. Các hàng cục bộ chạy trên macOS, rustc **1.98.1**, Node
> **24.19.0**, tại `df7c30f` cộng thêm đúng hai thứ: file test mới
> `src-tauri/tests/e2_frozen_manifest_tests.rs` và chú thích "đã đóng băng" trên `LIVE_OBSERVABLE_IDS`.
> Lượt này không đo frontend cục bộ, E2E, audit hay `cargo test` profile mặc định.

| Loại | Gate | Lệnh/Nguồn | Kết quả |
|---|---|---|---|
| 📏 | Artifact E2 (CI) | Actions run `43837617631` trên `main` @ `df7c30f`, job Frontend, bước "E2 artifacts match the committed preregistration" | **success**. Cả job Frontend success; job Rust của run này **chưa xong** lúc ghi, nên không ghi kết quả của nó |
| 📏 | Artifact E2 (cục bộ) | `npm run check:e2` | **exit 0**: tính lại 264 delta theo seed (analysis) và 22 (smoke) |
| 📏 | Backend test | `cargo test --features desktop --no-fail-fast` | **1318 passed · 0 failed · 2 ignored**, **128** dòng `test result`, exit 0. Gồm 2 test của `e2_frozen_manifest_tests` |
| 📏📋 | Chính sách target | `node scripts/check_test_targets.mjs <capture> --profile desktop` | exit 0: 128 target, 3 rỗng và 2 ignore, tất cả đều có trong allow-list; 8 target feature-gated chạy |
| 📏 | Lint backend · Tách feature · Format | `cargo clippy --all-targets --features desktop -- -D warnings` · `cargo clippy --all-targets --no-default-features -- -D warnings` · `cargo fmt --check` | exit 0 · exit 0 · exit 0 |
| 📏 | Đối chứng âm | `cargo test --features desktop --test e2_frozen_manifest_tests`, với `e2-control-shared-brain.json` tạm lấy bản tháng 7 (`970d963^`, chưa có `live.dormant_eu`) | **FAILED** đúng như cần, chỉ ra dòng 65 (run dùng `live.dormant_eu`, fixture có `live.closed_eu_total`). Sau đó fixture được trả lại byte của HEAD |

Vì sao có test mới: `check:e2` chỉ chạy trong job Frontend. Tác giả `970d963` đã chạy
`cargo test --features desktop` (94/94 target xanh, theo commit message) mà vẫn làm hỏng bằng chứng.
`e2_frozen_manifest_tests` so từng fixture với bản chép trong artifact, nên lần sửa fixture sau sẽ đỏ
ngay ở `cargo test`.

---

### 1.f Đo 2026-10-16 — `main` đỏ, cây làm việc đỏ hai gate tái lập

> Lượt này được chạy để **lập kế hoạch**, không phải để tuyên bố xanh. Nó đo **ba đối tượng khác
> nhau** — đừng trộn số của chúng:
>
> 1. **`main` @ `043930c`** — chỉ có số của CI GitHub, không đo lại cục bộ.
> 2. **`389de4b`** — commit duy nhất của `feat/oss-071b-live-tracker` trước `main`, **chưa push**.
>    Đo trong một worktree tạm, sạch, chỉ hai target.
> 3. **Cây làm việc** = `389de4b` + khối chưa commit (45 file sửa, khoảng 70 file mới, sửa lần cuối
>    2026-08-16). Đo toàn bộ gate cục bộ.
>
> Toolchain: rustc **1.98.1** (2026-09-01) — **khác** 1.95.0 của §1.a, nên một lint clippy mới xuất
> hiện ở code không ai sửa; Node 22.23.2. Không có `cargo`/`rustc` nào khác chạy; các suite chạy
> **tuần tự** theo §4.

**`main` @ `043930c` — CI GitHub**

| Loại | Gate | Nguồn | Kết quả |
|---|---|---|---|
| 📏 | CI Rust (`windows-latest`) | Actions run `31242649630`, 2026-08-08 | **failure** — `cargo fmt --check`, clippy, tách feature, parity binding, `cargo test` và policy target cùng đỏ vì `error: encountered diff marker`: conflict marker đã commit ở `src-tauri/tests/common/allocator.rs:21` và `scripts/check_test_targets.mjs:78,108,155`, vào từ `4b6258d`. Thêm `NOTICE is out of date` |
| 📏 | CI Frontend (`ubuntu-latest`) | cùng run | **failure** — `npm audit` root và `tests/`; link tài liệu gãy `STATE_OF_THE_PROJECT.md:916 → ../../LICENSE` (file bị xoá khi relicense). E2E 18/18 pass |
| 📏 | Advisory Rust trên lockfile của `main` | `cargo audit`, 2026-10-16, trong worktree có cùng `Cargo.lock` với `main` | **exit 1 — 4 lỗ hổng** `rustls` 0.23.42: RUSTSEC-2026-0285 (công bố 2026-09-14), -0346, -0348, -0349 (2026-10-14), sửa ở ≥ 0.23.46. Chúng ra đời **sau** lượt CI cuối, nên gỡ marker thôi thì `main` vẫn đỏ. Sửa ở nhánh `fix/main-ci-green` |
| 📏 | Advisory npm, **kể cả** với lockfile đã refresh của `819ad2d` (PR #32) | `npm audit --audit-level=high` và `--prefix tests`, 2026-10-16 | **exit 1** — root 7 (5 high), `tests/` 4 (2 high): `undici`, `fast-uri`, `brace-expansion`, `source-map-js`, `@xmldom/xmldom`, `@vitest/mocker`, đều công bố sau 2026-08-08. `npm audit fix` của npm 10.9.8 (bản CI dùng) **crash** (`reading 'edgesOut'`); sửa bằng npm 11 rồi kiểm lại bằng npm 10 ở nhánh `fix/main-ci-green` |

**`389de4b` sạch — worktree tạm, 2026-10-16**

| Loại | Gate | Lệnh | Kết quả |
|---|---|---|---|
| 📏 | Hai target nghi vấn | `cargo test --features desktop --test live_experiment_tests --test physics_tests` | **17 passed** + **10 passed**, 0 failed — gồm `run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run`, `a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards` và test hồi quy `sensor_ray_from_agent_centre_sees_past_its_own_collider` |

**Nhánh `fix/main-ci-green` @ `4cf71ee` — đo cục bộ 2026-10-16, đúng lệnh của `ci.yml`**

> Bản sửa đỏ của HT0.2 + HT0.3: `origin/main` + gỡ marker, hai commit của PR #32, template `NOTICE`,
> toolchain ghim 1.98.1, `rustls` 0.23.46, binding sinh lại, `npm audit fix`. Commit sau `4cf71ee`
> chỉ thêm bảng này. Đây là lượt **cục bộ trên Windows**; job frontend của CI chạy trên
> `ubuntu-latest`, nên CI của PR mới là bằng chứng cho `main`.

| Loại | Gate | Lệnh | Kết quả |
|---|---|---|---|
| 📏 | Toolchain | `rustc --version` | 1.98.1, chọn bởi `rust-toolchain.toml` |
| 📏 | Format | `cargo fmt --check` | exit 0 |
| 📏 | Advisory Rust | `cargo audit` · `cargo deny check` | exit 0 · exit 0 — 0 lỗ hổng; 2 cảnh báo yanked đã cho phép, chỉ ở lockfile |
| 📏 | Lint backend | `cargo clippy --all-targets --features desktop -- -D warnings` | exit 0 |
| 📏 | Tách feature (G2 #2) | `cargo clippy --all-targets --no-default-features -- -D warnings` + `cargo tree --no-default-features -e normal` | exit 0 — **0** kết quả cho `neo4rs\|tokio-tungstenite\|burn-wgpu\|naga` |
| 📏 | Parity binding | `cargo test --lib export_bindings` rồi `git diff --exit-code -- src/types/generated` | exit 0 |
| 📏 | Backend test | `cargo test --features desktop -- --nocapture` (`ANIMA_USE_GPU=0`) | **1069 passed · 0 failed · 2 ignored**, 93 dòng `test result` |
| 📏📋 | Chính sách target | `check_test_targets.mjs` không `--profile` (như CI) và `--profile desktop` | exit 0 — 93 target, 3 rỗng + 2 ignore đều trong allow-list, 8 target feature-gated chạy, không target nào khởi chạy mà không báo số |
| 📏 | Licence · SBOM · schema · NOTICE · flora | `npm run check:licenses` / `check:sbom` / `check:sbom-schema` / `check:notice` / `check:flora-footprint` | exit 0 cả năm |
| 📏 | Advisory npm | `npm audit --audit-level=high` (root và `tests/`) | **0** lỗ hổng cả hai |
| 📏 | Vệ sinh + E2 | `check:text-hygiene` · `check:e2` | exit 0 · exit 0 |
| 📏 | Lint frontend | `npm run lint` + `eslint_ratchet.mjs` | **0 error, 0 warning** (baseline 0) |
| 📏 | Typecheck `tests/` | `npm run typecheck:tests` | exit 0 |
| 📏 | Test frontend | `npm run test` · `npm run test:frontend` | 14 file · **109 passed** · 41 file · **446 passed** |
| 📏 | Build + artifact | `build` · `check:bundle-closure` · `check:csp` · `check:bundle` | exit 0 — 21 gói có byte trong `dist/`, 2 HTML ship, 1652,4 / 2000 KiB |
| 📏 | E2E Playwright | `npm run test:e2e` | **18 passed**, 0 `console.error` / `pageerror` / `TypeError` |
| 📏 | Link tài liệu | `node scripts/check_docs_links.mjs` | 658 link / 116 file, 0 gãy |

**Cây làm việc — 2026-10-16**

| Loại | Gate | Lệnh | Kết quả |
|---|---|---|---|
| 📏 | Backend test | `cargo test --features desktop --no-fail-fast` | **1212 passed · 4 failed · 2 ignored**, 107 dòng `test result`, exit 101. Bốn test đỏ là **một** hồi quy — nhánh resume/fork lệch checksum với run liền mạch (`1740163834` so với `1989393324`): `live_experiment_tests::{a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards, run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run}` và hai test mới chưa track trong `adversarial_challenger_m2_r2_stress_tests`. Hai test đầu **pass** tại `389de4b` (bảng trên), nên hồi quy nằm trong khối chưa commit |
| 📏📋 | Chính sách target | `node scripts/check_test_targets.mjs <capture> --profile desktop` | exit 0 — 107 target, 3 rỗng + 2 ignore (đều trong allow-list), 8 target feature-gated chạy |
| 📏 | Parity binding | `git status -- src/types/generated` sau `cargo test` | file đã track không đổi byte nào; 15 file sinh mới vẫn untracked |
| 📏 | Format | `cargo fmt --check` | **exit 1** — 245 chỗ lệch trong 36 file, **tất cả** là file chưa commit (17 mới, 19 sửa); 0 file đã commit |
| 📏 | Clippy | `cargo clippy --all-targets --features desktop -- -D warnings` | **exit 101** — 14 lỗi: 12 trong module mới chưa track (`core/branch_tree.rs`, `core/sweeps.rs`, `core/telemetry_export.rs`, mỗi file 4); **2 trong code đã commit**: `core/sha256.rs:49` và `core/world_artifact.rs:166`, lint mới `chunks_exact_to_as_chunks` của clippy 1.98 |
| 📏 | Lint frontend | `npm run lint` | exit 0 — 0 error, **102 warning**: 101 trong file chưa track, 1 trong `Terrain.tsx` (đang sửa dở), 0 trong file đã commit |
| 📏 | Ratchet ESLint | `node scripts/eslint_ratchet.mjs` | **exit 1** — 102 warning so với baseline 0 |
| 📏 | Typecheck `tests/` | `npm run typecheck:tests` | exit 0 |
| 📏 | Test frontend (`src/`) | `npm run test` | **14 file · 109 passed**, exit 0 |
| 📏 | Test frontend (`tests/`) | `npm run test:frontend` | **57 file · 632 passed**, exit 0 — gồm test của khối chưa commit |
| 📏 | Build | `npm run build` | exit 0 |
| 📏 | CSP (artifact) | `npm run check:csp` | exit 0 — **3** file HTML ship (thêm `map2.html`), 0 origin ngoài, 0 inline script |
| 📏 | Ngân sách bundle | `npm run check:bundle` | exit 0 — 1849,5 / 2000 KiB |
| 📏 | Case tham số IPC | `npm run check:ipc` | exit 0 — 34 command · 39 call site · 14 key |
| 📏 | Artifact E2 | `npm run check:e2` | exit 0 |
| 📏 | NOTICE | `node scripts/gen_notice.mjs --check` | exit 0 — up to date (419 crate · 21 gói npm phân phối · 18 cài-không-ship · 1 văn bản chưa giải quyết). Nội dung **vẫn** ghi Anima-Engine là "proprietary; see LICENSE" |
| 📏 | Link tài liệu | `node scripts/check_docs_links.mjs` | **exit 1** — 634 link / 115 file, 1 gãy (`→ ../../LICENSE`). Link đó đã gỡ trong cùng lượt sửa tài liệu này |
| ⚠️ | E2E Playwright | `npm run test:e2e` | exit 0 — 270 passed, 0 `console.error` / `pageerror` / `TypeError` — **nhưng không so được với §1.b**: khối chưa commit đổi `tests/package.json` thành `playwright test e2e/tiers/`, nên **18 spec của CI không chạy**; `e2e/tiers` kiểm một bản viết lại bằng TypeScript của engine |

**Không chạy lại trong lượt này:** `cargo audit`, hai `npm audit`, `check:licenses-complete`, SBOM,
`cargo test` profile mặc định, `capture:views`. Lượt đo đầy đủ trên `main` sau khi sửa là HT0.7 của
[lộ trình hoàn thiện](COMPLETION_ROADMAP_2026-10-16.md).

### 1.a Backend (Rust) — chạy lại 2026-07-27 tại `19e67fa`

> `19e67fa` là commit **mã nguồn** cuối cùng; `e4eff6c` sau nó **chỉ đổi tài liệu**, nên số ở mục
> này vẫn là số của cây mã hiện tại. Nhãn ghi đúng commit mà lệnh đã chạy, không phải HEAD.

| Loại | Gate | Lệnh | Cấu hình | Kết quả |
|---|---|---|---|---|
| 📏 | Backend test | `cargo test --features desktop --no-fail-fast` | `desktop` | **877 passed · 0 failed · 2 ignored**, **87** dòng `test result`, exit 0 |
| 📏 | Backend test | `cargo test --no-default-features --no-fail-fast` | mặc định | **859 passed · 0 failed · 2 ignored**, **80** dòng `test result`, exit 0 |
| 📏📋 | Chính sách target/ignore | `node scripts/check_test_targets.mjs <capture> --profile desktop` | `desktop` | exit 0 — **87 target**, 3 rỗng *(có chủ ý, có trong allow-list)*, 2 ignore *(có chủ ý)*, **7/7 target feature-gated chạy** |
| 📏📋 | Chính sách target/ignore | `node scripts/check_test_targets.mjs <capture> --profile default` | mặc định | exit 0 — **80 target**, 3 rỗng *(có chủ ý)*, 2 ignore *(có chủ ý)*, **0 target feature-gated được lên lịch** |
| 📏 | Format | `cargo fmt --check` | — | exit 0 |
| 📏 | Lint backend | `cargo clippy --all-targets --no-default-features -- -D warnings` | mặc định | exit 0 |
| 📏 | Lint backend | `cargo clippy --all-targets --features desktop -- -D warnings` | `desktop` | exit 0 |
| 📏 | Advisory Rust | `cargo audit` | — | exit 0 — 1.169 advisory nạp, quét **773 crate** trong `Cargo.lock` |
| 📏 | Tách feature (G2 #2) | `cargo tree --no-default-features -e normal` | mặc định | **0** kết quả cho `neo4rs\|tokio-tungstenite\|burn-wgpu\|naga`, và **0** cho `criterion` |
| 📏 | Parity binding Rust↔TS (G1.4) | `cargo test` rồi `git diff --exit-code -- src/types/generated` | `desktop` | exit 0 — binding sinh lại **không đổi byte nào** |

Ba target rỗng và hai test `#[ignore]` **là quyết định đã ghi**, không phải nợ: lý do từng mục nằm
trong [`scripts/test_target_policy.mjs`](../../scripts/test_target_policy.mjs), và gate fail với bất
kỳ mục nào ngoài danh sách **hoặc** bất kỳ mục nào trong danh sách đã hết hiệu lực.

### 1.b Frontend / lint — chạy lại 2026-07-27 tại `e4eff6c`

> ✅ **Cập nhật 2026-11-19 (PR #175):** dòng giấy phép cuối cùng đã đóng, `check:licenses-complete`
> exit 0; xem đầu §1.c. Dòng 🔴 ngay dưới là số đo 2026-07-27, giữ làm lịch sử.
>
> 🔴 **`npm run check:licenses-complete` FAIL (exit 1) — đây là chặn phát hành, xem 1.c.**
> Mọi hàng khác dưới đây xanh. Một hàng xanh không bù được hàng đỏ đó.
>
> **E2E, typecheck và cả hai `npm audit` được chạy LẠI tại `e4eff6c`, tức SAU khi sửa IPC** — vì bản
> sửa đó đụng cả `src/App.tsx`, `src/components/LegacyImportPanel.tsx` và 5 file test, nên số cũ
> không còn hiệu lực. Spec `ipc_contract.spec.ts:127` (đường import save cũ, chính đường có key bị
> sửa) **pass**.

| Loại | Gate | Lệnh | Kết quả |
|---|---|---|---|
| 📏 | **Case tham số IPC** | `npm run check:ipc` | **28 command · 22 call site · 8 key** — exit 0. Gate **mới**: trước khi có nó, **5 key sai trên 4 command** (`save_simulation_state`, `load_simulation_state`, `trigger_migration`, `import_legacy_save`) — save/load **chưa bao giờ chạy được trong app thật**, và 6 assertion trong 5 file test đã **đóng băng key sai** nên suite xanh *nhờ* có lỗi. Sửa ở `19e67fa` |
| 📏 | Artifact thí nghiệm E2 | `npm run check:e2` | exit 0 — mọi digest tính lại bằng `node:crypto`, 132 + 11 delta theo seed |
| 📏 | Lint frontend | `npm run lint` | exit 0 |
| 📏 | Ratchet ESLint | `node scripts/eslint_ratchet.mjs` | **0 error, 0 warning** (baseline **0**), exit 0 |
| 📏 | Typecheck `tests/` | `npm run typecheck:tests` | exit 0 |
| 📏 | Test frontend (`src/`) | `npm run test` | **14 file · 109 passed · 0 fail · 0 skip**, exit 0 |
| 📏 | Test frontend (`tests/`) | `npm run test:frontend` | **38 file · 432 passed · 0 fail · 0 skip**, exit 0 — chạy khi máy rảnh; xem §4 về nhiễu do tranh chấp CPU. Số worker do `tests/vitest.config.ts` ghim (4 ở máy này, **2 khi `CI` được đặt**) — xem §1.e |
| 📏 | Build | `npm run build` | exit 0 — `tsc` sạch, 806 module transform |
| 📏 | E2E Playwright | `npm run test:e2e` | **18 passed · 0 failed · 0 skipped**, exit 0, **và 0 `console.error` / `pageerror` / `TypeError` trong toàn bộ output thô** — server riêng cổng 5177 + kiểm định danh trong `global-setup.ts`; `ANIMA_E2E_REQUIRE_BACKEND` **cố ý không đặt**, nên `real_backend.spec.ts` không khai test nào |
| 📏 | CSP (artifact) | `npm run check:csp` | 2 file HTML ship · 0 origin ngoài · 0 inline script · đủ directive hardening — đọc `dist/` **vừa build ở hàng trên** |
| 📏 | Ngân sách bundle | `npm run check:bundle` | 23 chunk · **1711,3 / 2000 KiB**, mọi chunk trong ngân sách |
| 📏 | Advisory npm (root) | `npm audit --audit-level=high` | **0 vulnerability**, exit 0 |
| 📏 | Advisory npm (`tests/`) | `npm audit --audit-level=high --prefix tests` | **0 vulnerability**, exit 0 |
| 📏 | Bản đồ — gate tất định | MCP `animal-map-vision::validate_map_manifest` | **pass, 100/100**, **0 critical · 0 high · 0 medium · 0 low** trên **429 entity**, `animal-map.manifest.json` |
| 📏 | Bản đồ — 8 view chuẩn | MCP `inspect_map_views` (overview, navigation, collision, spawn, lighting, water, biome_transition, ecosystem) | `missingViewKinds: []`; tuyến navigation liền mạch từ marker xuất phát tới đích; **không** tìm thấy mâu thuẫn sinh thái. **Kiểm tra chỉ-đọc** — nhánh này **không đổi** file nguồn bản đồ nào, nên **không** có cặp before/after để so và **không** tuyên bố "map hoàn thành" |

> **Sửa một số cũ:** hàng E2E trước đây ghi *9 passed / 5 skipped* (đo 2026-07-26 tại `d006f64`).
> Lượt chạy này cho **18 passed / 0 skipped** — năm spec skip cũ đã được thay bằng
> `ipc_contract.spec.ts` chạy trên transport mock có kiểu sinh từ `ts-rs`, và `real_backend.spec.ts`
> **không khai test nào** trừ khi `ANIMA_E2E_REQUIRE_BACKEND=1`. Không spec nào spawn binary desktop.
>
> ⚠️ **`18 passed` một mình từng KHÔNG phải một gate sạch, và đó là lý do hàng này nay ghi thêm vế
> thứ hai.** Ở `e7bac09`, cùng một lượt chạy exit 0 vẫn in **bốn `TypeError`, mỗi cái hai lần**:
> `global-setup.ts` hâm nóng module graph bằng cách mở dashboard **không cài transport Tauri**, nên
> mọi `invoke`/`listen` ném lỗi — và không có gì theo dõi console của setup, vì `isOwned` nằm trong
> `console_hygiene.spec.ts` nên chỉ quản đúng hai trang spec đó lái. Đã sửa ở **`1cddeb5`**:
> global-setup cài đúng transport các spec dùng, tự kiểm console của chính nó, và bộ phân loại
> chuyển sang `tests/e2e/console_policy.ts` để dùng chung. Kiểm ngược: bỏ transport ra thì lượt chạy
> **exit 1** và nêu tên cả sáu thông điệp. Từ nay một hàng E2E chỉ được ghi là xanh khi **cả** exit
> code **và** output thô đều sạch.

### 1.c Licensing / SBOM / vệ sinh — chạy lại 2026-07-27 tại `e4eff6c`

> `cargo audit`, cả hai `npm audit`, `check:compliance` và tách feature mặc định đều chạy tại
> `e4eff6c`; các gate licence khác chạy tại `19e67fa` và `e4eff6c` không đụng dependency nào.

> ✅ **Cập nhật 2026-11-19 (PR #175): giấy phép không còn chặn phát hành.** `hexf-parse` 0.2.1 đã rời
> đồ thị phụ thuộc ở R1c (#127); dòng duy nhất còn lại sau đó là `utf8-chars` 3.0.7 (`MIT OR Apache-2.0`,
> không có file giấy phép, NOTICE hay dòng copyright nào). Dòng đó được đóng bằng cách **chọn Apache-2.0**
> trong biểu thức crate khai: văn bản Apache-2.0 đầy đủ như khi công bố và §4 không có gì để giữ lại ở
> đây, còn điều kiện thông báo của MIT trỏ tới thông báo của chủ sở hữu mà crate không có. Chủ dự án cho
> phép lựa chọn này trong chat ngày 2026-11-19 (QD-C2). Cơ chế hẹp, fail-closed: mục `elections` trong
> `licensing/upstream/sources.json`, kiểm ở `scripts/lib/upstream_licenses.mjs`, có quét đệ quy crate đã
> cài. `licensing/UNRESOLVED.md` trống; `check:licenses` trên CI Windows tại `dc66ec82` in "1 elected, 0
> unresolved"; `check:licenses-complete` exit 0 (đo 2026-11-19 bằng bản dựng lại inventory offline trên
> Mac). **Câu hỏi pháp lý mà khối dưới nêu vẫn còn, dưới dạng hẹp hơn:** việc chọn đáp ứng các điều kiện
> của Apache-2.0, nhưng một trường `license` trơn của Cargo có phải là lời cấp phép của tác giả hay không
> là câu hỏi pháp lý; dự án coi license đã khai của crate đã publish là lời cấp phép, như hệ sinh thái
> Rust vẫn làm. Không phải tư vấn pháp lý. Khối 🔴 dưới đây là số đo 2026-07-27, giữ làm lịch sử.

> 🔴 **CHẶN PHÁT HÀNH — `npm run check:licenses-complete` exit 1.**
> `hexf-parse 0.2.1` khai `CC0-1.0` nhưng artifact đã cài **không chứa file licence nào**, và
> repository upstream tại đúng commit phát hành (`4225763`, tag `0.2.1`) cũng **không có**. File
> `LICENSE` duy nhất từng được commit vào repo đó xuất hiện 2024-12-04 — **ba năm rưỡi sau** bản
> phát hành này — và nội dung là **0BSD**, tức một licence *khác* với CC0-1.0 mà bản này khai; vendor
> nó vào sẽ gán **sai điều khoản** cho code đang ship. Nó vào binary qua `naga` → wgpu → `ml-wgpu` →
> `desktop`, nên **có phân phối**.
>
> Đây là **quyết định pháp lý**, không phải việc kỹ thuật — chính `licensing/UNRESOLVED.md` viết vậy:
> *"Closing this row is a legal decision about whether the crates.io declaration alone suffices, not
> an engineering one."* **Chưa có quyết định pháp lý nào được ghi ở đâu cả.** 31/32 hàng khác đã
> đóng đúng cách (39 file vendor, 24 commit, 19 repository); hàng này thì không.
>
> Cho đến khi có một quyết định pháp lý thật được ghi lại, **dự án không ở trạng thái phát hành
> được.** Gate đã nói đúng câu đó: *"This gate is what 'ready to distribute' means; it is failing."*

| Loại | Gate | Lệnh | Kết quả |
|---|---|---|---|
| 📏 | NOTICE | `npm run check:notice` | up to date — 419 crate · **21 gói npm phân phối** · 18 gói cài-nhưng-không-ship · **1 văn bản chưa giải quyết** |
| 📏 | Văn bản license bên thứ ba | `npm run check:licenses` | up to date — **440 thành phần phân phối** · **266 văn bản khác nhau** · **1 chưa có văn bản** |
| 📏 | SBOM | `npm run check:sbom` | up to date — **458 thành phần** · 440 phân phối · 459 bản ghi dependency |
| 📏 | SBOM đúng schema | `npm run check:sbom-schema` | hợp lệ CycloneDX 1.5 — 458 thành phần · 458 purl duy nhất · **1 khoảng trống license đã ghi** · schema ghim ở `c320fc0f0b46` |
| 📏 | Ranh giới bundle npm | `npm run check:bundle-closure` | **21 gói** có byte trong `dist/` (3 do toolchain nhúng), khớp ranh giới đã commit |
| 📏 | Dấu chân flora khớp hình học | `npm run check:flora-footprint` | mọi bán kính khai báo khớp geometry (drift lớn nhất 3,5e-5) |
| 📏 | Byte điều khiển trong source | `npm run check:text-hygiene` | **545 file** text được track, 0 byte điều khiển thô |
| 📏 | Link tài liệu | `node scripts/check_docs_links.mjs` | **518 link tương đối trong 99 file**, **0 gãy** |
| ⏳ | Kho license upstream đã vendor | `npm run verify:upstream-licenses` | **39 file · 24 commit · 19 repository** khớp byte-cho-byte — **2026-07-27 tại `b6a579e`**; cần mạng và chạy tay, **không** nằm trong lượt gate này |

*Lịch sử, 2026-07-27; đã thay bằng cập nhật 2026-11-19 ở đầu §1.c:* **"1 chưa có văn bản" là `hexf-parse` 0.2.1 (CC0-1.0), và nó vẫn đang chặn phát hành.** Upstream chưa
bao giờ publish văn bản license cho bản đó; đóng dòng này là **quyết định pháp lý**, không phải việc
kỹ thuật. Xem [`licensing/UNRESOLVED.md`](../../licensing/UNRESOLVED.md) và §3.16.

### 1.d Điều **chưa** được đo — đừng đọc bảng trên thành nhiều hơn nó nói

| Khẳng định | Trạng thái thật |
|---|---|
| Thế giới Bevy sống chạy được thí nghiệm | **Headless adapter verified** (đo lại 2026-07-27 tại `068a750`: `tests/live_experiment_tests.rs` chạy **17 test**, 0 fail). **Không** gọi là "experiment-ready": chưa có lần chạy app desktop nào dưới executor đa luồng — xem §3.3 |
| Hiệu năng tick trong app | 📏 **Đã đo 2026-07-27** — `full_tick` p50 **1642,2 µs**, 1800 mẫu, bản **debug**, executor multi-threaded, 256², **10 agent**. Đóng 3 hàng `chưa đo`; hàng `Full-brain agents MVP` **vẫn mở** vì nó định nghĩa ở 1.000 agent. Chưa có lần chạy **release** nào — xem [BENCHMARK_BASELINE.md § Đo trong app](../../BENCHMARK_BASELINE.md#đo-trong-app--2026-07-27-bản-debug) |
| Hạ tầng thí nghiệm | 🔧 Manifest/runner/fork/ledger **tồn tại và có test**. Đó **không phải** một kết quả khoa học — chưa có run nào được thiết kế, chạy và diễn giải như một thí nghiệm |
| App khởi động dưới CSP mới | 📏 **Đã xác minh 2026-07-27.** Chủ dự án mở app, reload để Console phủ trọn một lần tải trang: **0 vi phạm CSP**, chỉ 4 cảnh báo `[TAURI] Couldn't find callback id …` (hệ quả bình thường của reload). IPC chạy thật từ webview — 4 lệnh capture đều trả giá trị. Xem [deployment §2.1](../ai/deployment/2026-07-27-feature-anima-completion.md) |
| Ảnh chụp / bằng chứng thị giác của app | 📏 **Có, 2026-07-27** — một ảnh gồm tiêu đề cửa sổ, `Số Ticks 91 781` và Console sạch. **Ảnh do chủ dự án giữ; repo vẫn chưa có chỗ commit ảnh app** và BENCHMARKING.md cố ý không tạo ra một chỗ mới |
| Map đã hoàn tất | ❌ **Không có review map nào trong gói này.** Đừng suy ra trạng thái map từ bảng trên |
| `hexf-parse` | ❌ **Chưa giải quyết** — xem §1.c |

### 1.e Sau PR #21 — đo 2026-07-27 tại `0f5b4d3`

> Ba commit sau `068a750` đều là **hạ tầng test**, và cả ba sửa cùng một loại lỗi: một gate xanh trên
> máy này và đỏ — hoặc không chạy nổi — ở nơi khác. Vì vậy mục này không lặp lại §1.a–§1.c; nó ghi
> những gì §1 chưa từng có: **một lượt CI hoàn chỉnh trên runner GitHub**, và **một lần đo lại khả
> năng tái lập của tám view chuẩn**.

| Loại | Gate | Lệnh / nguồn | Kết quả |
|---|---|---|---|
| 📏 | **CI đầy đủ, runner GitHub** | Actions run `30271127942` tại `0f5b4d3` | Rust (`windows-latest`) **success, 15m03s** · Frontend (`ubuntu-latest`) **success, 4m15s** · GitGuardian success. **Lần đầu tiên cả hai job cùng xanh** trên nhánh này — hai lượt trước đó: một lượt Frontend đỏ ba bước, một lượt bị cancel giữa chừng |
| 📏 | Test frontend (`tests/`) dưới cấu hình CI | `CI=1 npm run test:frontend` | **38 file · 432 passed · 0 fail**, 78,7s — worker do config chọn, không truyền cờ tay |
| 📏 | E2E Playwright | `npm run test:e2e` | **18 passed · 0 failed · 0 skipped** |
| 📏 | **Tái lập view chuẩn** | `npm run capture:views` | **8/8 view byte-identical**, 3,0 phút — mỗi view chụp hai lần từ hai browser context sạch, hai SHA-256 bằng nhau; `git status` **sạch** sau cả 16 lần chụp, nên byte sinh lại **trùng ảnh đã commit** |
| 📏 | Lint frontend + ratchet | `npm run lint` + `node scripts/eslint_ratchet.mjs` | **0 error, 0 warning** (baseline 0) |
| 📏 | Link tài liệu | `node scripts/check_docs_links.mjs` | **562 link trong 110 file**, 0 gãy (§1.c ghi 518/99 tại `e4eff6c`; chênh lệch là tài liệu thêm ở PR #21) |
| 📏 | Lifecycle lint | `npx ai-devkit lint --feature anima-completion` | All checks passed |

> **Ba lỗi đã sửa, và điều đáng nhớ ở mỗi lỗi không phải bản vá.**
>
> 1. `tests/vitest.config.ts` ghim `maxWorkers: 4`, đo trên desktop 14 nhân. Runner `ubuntu-latest`
>    có **4 vCPU**, nên 4 ở đó là một worker mỗi nhân — đúng cái tranh chấp mà dòng đó sinh ra để
>    tránh, và hai file `render(<App />)` chết ở `Test timed out in 15000ms`. Nay là
>    `process.env.CI ? 2 : 4`. **Một hằng số điều chỉnh theo phần cứng không được ghim cứng khi nó
>    phải chạy trên hai loại máy.**
> 2. `playwright.config.ts` dò `http://127.0.0.1:<port>` còn lệnh `npm run dev` không đặt host, nên
>    Vite bind theo cái `localhost` phân giải ra — từ Node 17 là thứ tự *verbatim*, `::1` đứng trước.
>    Server lên khoẻ (`ready in 190 ms`) và Playwright gõ cửa khác suốt 120 giây. Nay có
>    `--host 127.0.0.1`.
> 3. `capture.config.ts` **cùng lỗi đó**, và nó hỏng **ngay trên Windows** — nên "lỗi này chỉ xảy ra
>    trên Linux" là sai, Node phân giải verbatim trên mọi nền tảng. Hệ quả nặng hơn hai lỗi trên:
>    `npm run capture:views` là **producer duy nhất** của tám view chuẩn, và nó **không khởi động
>    được**. Không gate nào đỏ, vì gate CI (`mapManifestEvidence.test.ts`) **hash ảnh đã commit** chứ
>    không sinh lại chúng. **Một gate đặt trên artifact không thể phát hiện producer của artifact đó
>    đã chết** — hàng `capture:views` ở bảng trên tồn tại để lần sau có người thực sự chạy nó.

### 1.1 Phân loại mọi con số trong tài liệu

Quy ước áp cho **toàn bộ** cây tài liệu, để một con số không bao giờ bị đọc nhầm loại:

| Loại | Nghĩa | Yêu cầu |
|---|---|---|
| **Đo hiện tại** | Kết quả một lệnh, còn đúng hôm nay | Phải có **lệnh + ngày**, hoặc trỏ về bảng §1 |
| **Đo lịch sử** | Kết quả một lệnh **lúc đó** | Phải ghi rõ là lịch sử + commit/ngày sinh ra nó |
| **Tham số thiết kế / hợp đồng** | Hằng số, ngưỡng, schema version, số hiệu yêu cầu | **Không** gắn lệnh — nó không phải phép đo |
| **Mục tiêu / ngân sách** | Điều muốn đạt | Ghi là *mục tiêu*, không phải kết quả |
| **Chính sách** | Khoản miễn trừ đã được duyệt | Phải có lý do ghi kèm, và một gate cưỡng chế |

**Ledger theo gói ở [`docs/ai/planning/`](../ai/planning/README.md) và các thư mục lifecycle cạnh nó
là bản ghi lịch sử theo ngày.** Số trong đó đúng với commit của gói đó và **không** được đọc như
trạng thái hôm nay; mỗi file đã mang một banner nói đúng điều này ở đầu.

<details>
<summary>Bảng lịch sử (2026-07-26, tại <code>d006f64</code>) — <b>số cũ, giữ để đối chiếu</b></summary>

> ⚠️ **Đo lịch sử.** Mọi con số dưới đây là kết quả tại `d006f64` ngày 2026-07-26 và **không** mô tả
> cây mã nguồn hôm nay. Trạng thái hiện tại ở [§1](#1-bảng-bằng-chứng-có-thẩm-quyền).
>
> **Toàn bộ bảng** — kể cả hàng `tests/`, vốn từng là hàng duy nhất chưa đo lại — đã được chạy tại
> `d006f64` (nhánh `feat/oss-071b-live-tracker`, trên `main` sau khi #19 merge).

| Gate | Lệnh | Kết quả lúc đó |

> **Đo lại 2026-07-29 — bảng vẫn đúng, nhưng ba gate đã từng nói dối để cho ra nó.** Lượt đo độc lập
> tại `8b3d91a` ban đầu cho `npm run lint` **đỏ 4.656 lỗi**, `cargo test` **745 pass · 1 fail**, và
> `check_test_targets.mjs` báo "0 rỗng" sau khi **chỉ đọc 61 trong 75 target**. Không cái nào là hồi
> quy mã nguồn; cả ba là lỗi của chính bộ gate, và đã sửa (§4). Sau khi sửa, đo lại đầy đủ:
> **746 pass · 0 fail · 4 ignored**, **75 target · 0 rỗng**, `fmt`/`clippy` sạch, lint **0 error ·
> 491 warning**, `test` 90 pass, `test:frontend` 243 pass, build pass, docs link 417/0 gãy.
>
> Bài học đáng giữ hơn con số: **một bộ gate tốt vẫn cần gate cho chính nó.** Cả ba lỗi đều thuộc
> loại mà dự án này tồn tại để chặn — chúng exit 0 và trông như xanh.

| Gate | Lệnh | Kết quả |
|---|---|---|
| Backend test | `cargo test --features desktop --no-fail-fast` | **746 pass · 0 fail · 4 ignored**, 75 test binary, 0 warning biên dịch |
| Target rỗng | `node scripts/check_test_targets.mjs <output>` | 75 target, **0 target chạy rỗng** |
| Format | `cargo fmt --check` | sạch |
| Lint backend | `cargo clippy --all-targets --features desktop -- -D warnings` | sạch |
| Lint backend (default) | `cargo clippy --all-targets --no-default-features -- -D warnings` | sạch |
| Test frontend (src) | `npm run test` | 13 file · **90 pass** |
| Test frontend (tests/) | `npm run test:frontend -- --maxWorkers=2` | 26 file · **243 pass**, 1 skip — cần giới hạn worker, xem dưới |
| Lint frontend | `npm run lint` + `node scripts/eslint_ratchet.mjs` | **0 error**, 491 warning (baseline 491) |
| Build | `npm run build` | pass |
| Link tài liệu | `node scripts/check_docs_links.mjs` | 417 link trong 90 file, **0 gãy** |

> ⚠️ **Suite `tests/` cần `--maxWorkers=2` khi máy đang bận. Đây là cách khắc phục đã kiểm chứng,
> không phải phỏng đoán.** Ba lần chạy liên tiếp tại `d006f64`, máy có phiên khác chạy `cargo` suốt:
>
> | Lần | Lệnh | Kết quả | Thời gian tường |
> |---|---|---|---|
> | 1 | mặc định | **28 lỗi** | 45,3s |
> | 2 | mặc định (sau khi chờ 14 phút) | **61 lỗi** | 40,0s |
> | 3 | `--maxWorkers=2` | ✅ **243 pass, 0 lỗi** | 85,3s |
>
> **Sửa một điều §4 và các bản trước của mục này ghi sai: con số KHÔNG phải dấu hiệu nhận dạng.** Nó
> thay đổi theo tải — 28 rồi 61. Dấu hiệu thật là **hình dạng**:
>
> - mỗi file test mất **~25–28 giây** thay vì ~1 giây;
> - tổng thời gian `tests` lớn hơn **nhiều lần** thời gian tường (327s so với 40s) — dấu hiệu tranh CPU;
> - lỗi tập trung ở các file `render(<App />)`, vì lần render đầu phải trả cho cả đồ thị module lazy
>   và `testTimeout` là 15.000ms. Đây là timeout, không phải khẳng định sai — nhưng nó **hiện ra như**
>   "không tìm thấy DOM node", nên đọc thoáng sẽ tưởng là hồi quy.
>
> Giới hạn worker chữa được vì mỗi worker có đủ CPU để lần render đầu về dưới ngưỡng 15s. Nó **chậm
> hơn** (85s so với ~20s lúc máy rảnh) nhưng cho ra một phép đo tin được, và đó là đánh đổi đúng khi
> không kiểm soát được tải.
>
> **Trước khi gọi suite này là hồi quy, kiểm hai thứ:** máy lúc đó đang chạy gì, và
> `git diff --name-only origin/main...HEAD` **có đụng file frontend nào không**. Ở `d006f64` câu trả
> lời là 4 file Rust + 3 file Markdown, **không một file frontend** — suite này chạy React/jsdom, nên
> không có đường nào để một thay đổi trong `evolution/lineage.rs` chạm tới nó.
>
> Bài học vẫn còn hiệu lực hôm nay và được nhắc lại ở §4. Số đo **hiện tại** của suite `tests/` nằm ở
> §1.b — chạy trên máy rảnh, không cần giới hạn worker.

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
| Não tiến hoá per-agent (ADR-0003) | Đã triển khai, **giữ tắt mặc định sau khi đo** | **12/12** gate EB pass (2026-07-27, `2b61d07`); **E2 đã chạy 2026-07-27 tại `9c57184`** — H1 null (delta đúng 0 trên 12/12 cặp, hiệu ứng sàn), mặc định giữ opt-in theo quy tắc đã đăng ký; xem §3.1 |
| Lab tiến hoá AE1–AE3 | Headless, opt-in | `ReferenceEvolutionWorld` |
| Adapter thí nghiệm cho thế giới sống | **Headless adapter verified** | `LiveExperimentAdapter` chạy **đúng** lịch trình app dùng, qua runner chung; **17 test** ở `live_experiment_tests.rs` (đếm từ lần chạy 2026-07-27 tại `068a750` trong §1.a). **Chưa** chạy app desktop; adapter từ chối exotic energy; không có quần thể AE3 — xem §3.3. Đừng viết "experiment-ready" |
| Đo tick trong tiến trình | 🔧 **Instrumentation đã ship — chưa có số** | `core/tick_capture.rs` + 4 lệnh IPC; đo không làm đổi quỹ đạo (có gate ở `tick_capture_tests.rs`). "Đã ship" **không phải** "đã có số đo phần cứng" — xem §1.d và §3.2 |
| Thế giới chung frontend↔backend | Live integrated | `src/utils/sharedWorld.ts` là identity duy nhất; artifact đẩy sang `init_world`. Từ worldgen 22 (2026-11-19) shape là `supercontinent` (`SUPERCONTINENT` trong `worldGen.ts`): ocean 45%, mọi biome đất có vùng liền nhìn thấy được từ overview; `island`/`continent` sinh y hệt v21. Số đo và cách đo ở PR "world: retune the shared world so every environment is visible". Từ 2026-11-19 thành phần này có cổng chặn: `artifacts/map_evidence.json` mục `composition` (biển, ô theo biome, vùng liền 4 hướng lớn nhất, thác, hang) do `npm run gen:map-evidence` ghi, `tests/frontend/mapEvidence.test.ts` tính lại và giữ trong `COMPOSITION_BANDS` (`utils/worldComposition.ts`: biển 0,40–0,50, mỗi biome đất một vùng ≥ 2.000 ô ở 2048², 20–80 thác, 30–48 hang). Thế giới `continent` cũ trượt bảy điều kiện |

Các hàng dưới đây được thêm ngày **2026-11-17**, cho những vùng đạt bậc sau tháng 7. Chúng trỏ tới PR, ADR
và file test có trên thân cây tại `6f14124`; **không test nào được chạy lại để viết chúng**, và chúng không
mang số đo. "Opt-in" nghĩa là khoá manifest vắng thì thế giới chạy như trước.

| Vùng | Bậc đạt được | Ghi chú |
|---|---|---|
| Tử vong và fitness sinh thái (F-018…F-022) | Headless, opt-in | `live.mortality`, `live.ecological_fitness`; gate ở `mortality_tests.rs`, `fitness_tests.rs`, `qd_tests.rs`, `observables_tests.rs`. Chi tiết và phần còn mở ở §3.19 |
| Tất định của lịch tick giữa các tiến trình | Live integrated | Một thứ tự tổng cho lịch (F-041, #65) và restore sau ca chết (F-040, #64); gate ở `schedule_total_order_tests.rs`, `schedule_ambiguity_tests.rs`. Giữa các nền tảng thì hợp đồng tất định vẫn **không hứa** |
| Quần thể luân chuyển trong adapter sống | Headless, opt-in | `live.reproduction` (thẻ S1, #66, [ADR-0005](../decisions/ADR-0005-live-population-turnover.md)); gate ở `live_turnover_tests.rs`. `EvolutionaryReplacement` của app không đổi. Khoá này chỉ điều khiển adapter: trong app desktop `autonomous_reproduction_system` chạy mặc định, vì `ReproductionPolicy` vắng mặt nghĩa là bật (`core/resources.rs`) |
| Vận động | Headless, opt-in | `live.locomotion` (#76, [ADR-0006](../decisions/ADR-0006-locomotion-actuation.md)); gate ở `locomotion_steering_tests.rs` và bản zero-alloc. Chưa bật mặc định trong app; ba câu hỏi mở ở §3.00 P3 |
| Sinh thái sống được cho thú săn và con mồi | Headless, opt-in | Sàn bộ lái, lái theo khát và ba khoá thu nhập của thú săn (#82, #87, [ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md)); test khoá P1 là `predator_prey_coexistence_tests.rs` (#89). **Không** phải S43 hay Red Queen |
| Phả hệ | Live integrated, trừ OSS-073 | Newick, `simplify`, nén trong tracker sống và MRCA (#75, lệnh `get_lineage_mrca`). Ca sinh/chết live vào tracker với id duy nhất qua khoá opt-in `live.lineage_tracking` (F-066, 2026-11-22). Line of descent (OSS-073) chưa làm — xem §3.15 |
| Bảo vệ nhánh và CI bắt buộc | Live integrated | Hai nhánh được bảo vệ từ 2026-11-17 — xem §3.00 |
| Thế giới đọc được (frontend) | Đã merge mười thẻ | RW-A1…A3, RW-B1…B3, RW-C1…C4. Ba thẻ cần phần cứng còn lại ở §3.00 P4 |

### 2.1 Vì sao bộ gate này đáng tin

Ghi lại để phiên sau không vô tình gỡ mất:

- `scripts/check_test_targets.mjs` bắt **target biên dịch thành binary rỗng rồi exit 0** *và* mọi
  test `#[ignore]`. Cả hai chế độ hỏng đều im lặng: một target rỗng exit 0, và một test bị ignore
  không phải pass cũng không phải fail. Từ 2026-07-27 nó cưỡng chế một **allow-list có lý do**
  ([`scripts/test_target_policy.mjs`](../../scripts/test_target_policy.mjs)) theo **cả hai chiều** —
  mục ngoài danh sách fail, và mục trong danh sách đã hết hiệu lực cũng fail, nên danh sách không thể
  âm thầm phình ra hay mục rữa. Bảy file feature-gated nay mang `required-features` nên Cargo **không
  lên lịch** chúng ở cấu hình không dựng được, thay vì dựng ra binary rỗng.
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

### 3.00 Việc kế tiếp — cập nhật 2026-11-17

> Mục này **thay thứ tự ưu tiên** của bàn giao 2026-07-27 ở §3.0 bên dưới; các mục §3.x còn lại giữ làm
> bối cảnh. Nó trỏ tới PR và finding thay vì chép số (quy tắc 8). Quyết định thuộc chủ dự án nằm ở
> [roadmap 31/10 §4](COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được) và
> [hướng dẫn chạy agent](../how-to/AGENT_WORKFLOW.md).

#### Bảng tiến độ theo chặng (đối chiếu 2026-11-17)

Stage order and exit gates come from
[Direction & Coordination Plan §5](DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md#5-lộ-trình-theo-chặng);
this table records where each stage stands, while per-card queue details stay in
[`AGENT_WORKFLOW.md` §3](../how-to/AGENT_WORKFLOW.md#3-hàng-đợi-thẻ). Reconciled `2026-11-25` (`D1. STATE-SYNC r4`) against
`gh pr list --state merged --limit 120 --json number,title,mergedAt,headRefName,mergeCommit`,
`gh pr list --state open`, and `git log --first-parent origin/feat/ecological-selection` at trunk
`9a53bad6` (recording every PR merged since `r3` `#243`: `#242` and `#244–#273`; PRs `#274–#276`, `#278` remain open).
**No build or test command was run for this board**; every number below links to its source.

| Stage | Exit gate | Status | Remaining |
|---|---|---|---|
| 0. Desk cleanup & decisions | No open Stage 1–2 decision slots; every open PR has a disposition | **Exit gate passed.** Initial 10 open PRs handled (`#38` merged; `#30`, `#31`, `#34–#37`, `#39–#41` closed), branch/audit triaged (`#42` D0, `#43` D1), working tree cleaned (`MP-0.5`); `QD-C`, `QD-1`, `QD-EN` decided `2026-11-17` (`#97`); `QD-M1` decided `2026-11-24` via `QD-OPENQ` (`PR #255`: `EXECUTION_PLAN` + swarm plan for work, `DIRECTION` for stage order); six `REVIEW` entries in `scripts/root_allowlist.json` resolved (`#261`). | — (plan-doc status labels updated as docs hygiene). |
| 1. One green tree | Both CI jobs green on trunk, then branch protection enabled | **Closed `2026-11-17`** (`#93`; details below). | — |
| 2. Merge two trunks | `C5`: full re-measurement into `§1.g` | **Closed `2026-11-16`.** `T2` merged in `#45`; `main` later synced with `feat/ecological-selection` at `36234e47` on `2026-11-20` (`#184`, `#189`). | — |
| 3. Post-merge debt | `R0`: full re-measurement; no fabricated values, no loosened tests, full `test:e2e` | **Closed `2026-11-18`.** `R0` `#123` (`46e25d5`, §1.i); Windows CRLF fix `#125`; Wave 2 `G1` (`#48`), `G2` (`#47`), `G3` (`#107`), `G4` (`#98`), `G5` (`#95`), `G6` (`#94`), `G7` (`#49`), `G9` (`#114`), and `G8` mostly done (`IPC_CONTRACT.md` & `check_ipc_reference.mjs`, 2026-11-14; per-entry `file:line` remaining per [`AGENT_WORKFLOW.md` §3](../how-to/AGENT_WORKFLOW.md#3-hàng-đợi-thẻ)); reasoned `npm audit` policy `#113`. | `G8` per-entry `file:line` anchors (see §3 note). |
| 4. First scientific loop | At least one preregistered experiment run once with `RESULT.md` and stated power | **Exit gate passed (`2026-11-24`).** **`E3b` (`#252`, merged `6d0fdeb2`, [`RESULT.md`](../../artifacts/experiments/e3b-productivity-gradient-body-mass/RESULT.md), [E3b summary](../ai/planning/2026-11-24-e3b-results.md)) is a preregistered, powered run (`192` runs = `64` seeds × `3` arms on `Windows 1`, `#245`) with `RESULT.md`: `H1` (`T / S`) and `H3` (`S / U`) were inconclusive (`insufficient_evidence` / `inconclusive_wide_ci`) and `H2` (`T / U`) was a powered equivalence (`equivalent_to_zero` / `equivalent_to_zero_powered_null`; model status: `headless adapter verified`).** Earlier completed/stopped experiments: `E2` `#59` (null, floor effect; did not satisfy Stage 4 gate); `E-SEL1` (`#77`, `#83`, `#84`, closed at smoke); `SEL2-P` (`#147`, `#152`, `#165`, `#170`, `#173`, stopped at smoke with `gate_pass = false`, [smoke note](../ai/planning/2026-11-19-sel2-p-smoke.md)); `E3-X` (`#164`, `#178`, `#181`, 36/36 confirmation runs on Windows 1, `insufficient_evidence` with 6/12 valid pairs, [E3-X results](../ai/planning/2026-11-20-e3-x-confirmation-results.md)). Core, audit & experiment work merged: `S1` `#66`, `S4` `#75`, `M1` `#100`/`#102`, locomotion/ecology `#76`/`#82`/`#87`/`#89`, `S8` `#106`, `L0`/`ADR-0008` `#109`/`#119`, `S6` `#110`, `S9` `#117`, `S10` `#124`, `SEL2-E` `#128`, `SEL2-D` `#130`, `S12` `#135`, `M4` `#137` ([eu_residual](../paper/data/eu_residual.md)), `M3` `#138` ([scaling](../paper/data/scaling.md)), `S13` `#139`, `F045` `#140`, `S14` `#142`, `S15` `#145`, `F048` `#146`, `S2` `#148`, `LAND1`/`LAND1-T` `#166`/`#176`, `F051`/`F051b` `#174`/`#179`, `FIX-CAP`/`FIX-CAP2` `#177`/`#180`, `SEL2P2-PREP` `#182`, `FOUNDER-LAYOUT` `#186`, `M2` `#196` ([62 gate controls](../paper/data/gate_controls.md)), `#197`, `#198` (`F-061`), `R5` `#200`, `F-066` `#207`, `#210`, **`C8. WR1` `#236` merged**, **`C9. WR2` `#246`** ([WR2 note](../ai/planning/2026-11-24-wr2-write-path.md)) + **`#254`** (`world_record_writer_tests.rs` `sim_world_sha256` fix), **`D9. FINDINGS-SYNC` `#250`**, **`D10. DOC-REVIEW` `#251`**, **`D20. AUDIT-FINDINGS` `#257`** ([`FINDINGS.md`](../../FINDINGS.md) `F-069..F-080`), **`C15. HOTLOOP-PERSIST` `#264`** (`a0522af5`: `F-067` zero-alloc census & `F-068` morphology cursor restore `50/50`), **`C14. HONEST-UI` `#269`** (`617fb1c0`: `GeneticInjection` disabled with `F-071` & Map2 decoration labels), **`D21. DOCS-CLEANUP` `#267`**, and **`#261`** (`1803aa85` root hygiene). `E3b` & follow-up pipeline: `VIAB-1` `#215`, `mut0` diagnosis `#216`, `FOUNDER-2SEG` `#224`, `VIAB-1b` `#228`, `E3B-FREEZE` `#231`, `QD-E3B-D1a` `#230`, `C1. E3B-RUNNER` `#234`, `C2. E3B-SMOKE` `#240`, **`C3. E3B-RUN` `#245`** ([run note](../ai/planning/2026-11-24-e3b-run-execution.md)), **`C4. E3B-RESULT` `#252`**, **`C12. E3B-PIN` `#260`** (`7e383b8b`, `QD-BIO-D2` freeze-time `LIVE_KEYS`/`observables` pin), **`SEL2-P2` draft `#247` + `C5b. SEL2P2-VIAB2` `#258`** (`QD-SEL2P2-D1..D7` locked; `12/12` survival across all arms at `200,000` ticks, `0` `trait_mass`, [VIAB-2 results](../ai/planning/2026-11-24-sel2-p2-viab2-results.md)), and **`C6. LAW1-DRAFT` `#253`** ([draft](../ai/planning/2026-11-24-law1-fork-experiment-draft.md), `LAW1-D1..D6` decided via `QD-OPENQ`). Per **`QD-SCALE-TRIAL`** (`PR #255`), trial/demo islands target `~100` animals (`~0.33 ms/tick` vs `5.64 ms` at `1,000` in [`scaling.md`](../paper/data/scaling.md); `1,000` stays the long-term benchmark target without blocking cards; `H6`/`H7` uses `~100` agents). | `SEL2-P2` freeze/smoke/confirmation; `LAW1` freeze (`QD-OPENQ` sets `LAW1-ref` first); `WR3–WR5`; `S3` (G2 gate #1); `S7` (deferred per `QD-SHAREDISLAND`); `H6`/`H7` owner desktop run (`~100` agents per `QD-SCALE-TRIAL`); open `physics_tests` allocation flake (`P11` with `anima-case`, CI run [45516447719](https://github.com/DuongNAD/Anima-Engine/actions/runs/45516447719)); `P9` M2 patch refresh. |
| 5. Release 0.9 & methods paper | `v0.9.0` tag, downloadable installer, 3 `MP-5.1` sessions | **In progress (CI build workflow, release docs & English public translations merged; release gates decided `2026-11-24`; no release tag yet).** `QD-C = C`, `QD-1 = level 2`, `QD-P1..P3` (`#97`), `R1c` `#127`, `utf8-chars` `#175` (`QD-C2`, `QD-OPENQ`), `P1` `#211`, English README (`#187`/`#189`), `D1` (`#232`, `#241`, `#243`), `QD-WAVE1124` `#230`, `D2. REL09-CI` `#233`, `D3. REL09-DOCS` `#238`, `D6. DOC-EN` `#237`, `QD-TRIAL-V2` in `#248`, and `2026-11-24` release decisions recorded in `PR #255` (`QD-REL09`: `G1 = B` `v0.9.0-rc.1` first after `WV2-7`, `G2 = A` unsigned installers, `G3 = C` workflow artifacts for RC / GitHub Release for `v0.9.0`, `G4 = A` fast-forward `main` before tagging; `QD-OPENQ`/`QD-OPENC`: `QD-M4` macOS unsigned if `NM-2` matches, `MP-5.6` bundled sample world in `0.9`, export/import in `1.0`). | Execute `v0.9.0-rc.1` cut (`D16 REL09-RC`) after `WV2-7` (`QD-TRIAL-V2`), `MP-5.1`–`MP-5.3`, private paper prose repo (name from owner), `DOC4` & `DOC7`. |
| 6. v1.0 vertical slice | `H11`: tag `v1.0.0` | **Design drafts `V0–V11`, `H10`/`H11`, `ADR-0015`/`0016`/`0018` (`proposed`), & `ADR-0019` (`accepted`) merged (`#235`, `#239`, `#244`, `#259`, `#262`, `#263`, `#268`, `#271`); zero `V0–V11` implementation code merged.** Merged specs: `D4. V-ADR` `#235` ([ADR-0013](../decisions/ADR-0013-live-creature-lifecycle.md), [V0](../ai/planning/2026-11-24-v0-develop-at-birth-spec.md)–[V1](../ai/planning/2026-11-24-v1-dynamic-fields-schedule-spec.md)); `D7. V-SPECS` `#239` ([V2](../ai/planning/2026-11-24-v2-species-profile-spec.md)–[V6](../ai/planning/2026-11-24-v6-interventions-causeid-spec.md)); **`D8. V-SPECS-2` `#244`** ([V7](../ai/planning/2026-11-24-v7-causal-timeline-ui-spec.md)–[V11](../ai/planning/2026-11-24-v11-example-scenarios-spec.md), [H10/H11](../ai/planning/2026-11-24-h10-h11-release-gates.md)); **`C13. BRAIN-BODY-ADR` `#259`** (`1bd2483d`, `accepted` [ADR-0019](../decisions/ADR-0019-brain-body-interface.md)); **`D14. PLANT-EVO-ADR` `#268`** (`01b24c37`, `proposed` [ADR-0015](../decisions/ADR-0015-evolving-plants.md) & [`P1–P8`](../ai/planning/2026-11-24-plant-cards.md)); **`D15. WORLD-CHANGE-ADR` `#271`** (`0e90f0d5`, `proposed` [ADR-0016](../decisions/ADR-0016-changing-world-surface.md) & [`W1–W7`](../ai/planning/2026-11-24-world-change-cards.md)). In `PR #255`, `QD-OPENQ` & `QD-OPENC` resolved `V0–V11`, `H10`/`H11` (`D-H11-3` = v1.0 requires World v2 in the app), and `ADR-0009`/`ADR-0013` acceptance (`D22` `PR #262`), while companion `2026-11-24` decisions resolved `QD-BRAINBODY` (`ADR-0019` `#259`) and `QD-BIO-D1..D4` (`ADR-0018` `D19` `#263` & `C12` `#260`). | Open ADR PR `#275` (`ADR-0017`); `V0–V11` implementation cards & `WV2` integration for `v1.0.0`. |
| 7. Multi-machine world | `contribute_matches_solo` green with two real machines | **In progress (`QD-SHAREDISLAND`, `ADR-0014` `#256`, `I1` `#266`, & `I2` `#272` merged).** Owner promoted host-simulated shared islands (guests spectate/inhabit over LAN) to the main goal (`QD-SHAREDISLAND` superseding `QD-M7`'s old `Inhabit` definition); **`D12. ISLAND-ADR` `#256`** merged accepted [ADR-0014](../decisions/ADR-0014-shared-island-host-and-guests.md) (`23/24` D-slots decided, `ISLAND-D7` deferred to `I10`) and [`2026-11-24-island-cards.md`](../ai/planning/2026-11-24-island-cards.md) (`I1–I11`). **`N2. I1` (`#266`, `65632382`)** landed guest policy, reserved `CauseId` band, genesis-phase provenance, and zero-alloc `SharedGuestIntents` ingress; **`N3. I2` (`#272`, `9a53bad6`)** landed host `Inhabit` actuation/release (`disconnect`/`kick`/death), `WorldRecordHeader.island` metadata, and offline World Record replay (`world_record_writer_tests.rs`, `island_determinism_tests.rs`). Trial islands target `~100` individual animals (`QD-SCALE-TRIAL`). | `I3` (`#276`)–`I11` LAN cards, `NM-4`, `contribute_matches_solo` gate; `ISLAND-D7` & `QD-M3` Internet transport remain open until LAN milestone 1 works. |
| Map v2 (parallel track, `QD-MAPV2`) | Phase gates `G-REF`, `G-SHAPE`, `G-COMP`, `G-PERF`, `G-SIM` | **In progress (generator & offline bake engine landed; `WV2-1`–`WV2-5`, `WV2-PHYS2`, & `MAP-D1..D5` merged).** Plan & design `#185`/`#192`; `WV2-1a/b/c` `#193–#195`; `NR-SPIKE-PROTOCOL` `#221` (`NR-D1..NR-D7` decided via `QD-OPENQ`, keeping `"no installs without approval"`); `ADR-0012` `#222`; `MAP-D1..D3` `#223`/`#227`/`#230`; `WV2-2` `#199`; **`WV2-3` `#242`** (`ANMW v3` reader, `WorldIdentity::sim_world_sha256`, `world_v2_sim_gate_tests.rs`); **`WV2-4` `#249`** (`f195a361`: `G-SHAPE` + `G-COMP v2` verifiers & provisional `C01` `artifacts/world-v2/`, `MAP-D4` `C01` `2/15` gates); **`WV2-5` `#265`** (`e7b99745`, [note](../ai/planning/2026-11-24-wv2-5.md): offline bake engine `anima-bake`, `QD-WV25` closure, grid scripts); **`WV2-PHYS2` `#270`** (`a146d2f2`, [note](../ai/planning/2026-11-24-wv2-phys2.md): generator physics rework — `S2_1 = P01` passes `9/15` gates at `4096²` with `0` spikes vs `C01` `2/15`, leaving `artifacts/world-v2/` untouched); **`MAP-D5` `#273`** (`33a4c197`, [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md): keep `C01` `cbdd3ba9...`, open `WV2-PHYS3`, note `#270` provenance gap `f195a361`). | `WV2-PHYS3` (`#274`), `WV2-7`, `WV2-6-PREVIEW` / `WV2-6`, `G-REF` owner visual sign-off, `NR-SPIKE` execution & `ADR-0011`. |

Four notes the table above does not show:

- **Map v2 runs in parallel outside Stages 0–7 (`QD-MAPV2`, `2026-11-20`):** see the dedicated `Map v2` row in the stage board above (`WV2-D` `#192`, `WV2-1a/b/c` `#193`–`#195`, `WV2-2` `#199`, `WV2-3` `#242`, `WV2-4` `#249`, `WV2-5` `#265`, `WV2-PHYS2` `#270`, and `MAP-D1`–`MAP-D5` `#273` all merged; `WV2-PHYS3`, `WV2-6-PREVIEW`/`WV2-6`, and `WV2-7` remaining).

- **Thứ tự đã bị đi trước ở một chỗ.** Kế hoạch swarm đặt S1 và S4 sau R0. Cả hai, cùng ba ADR 0005–0007 và
  bản đăng ký E-SEL1, đã merge ngày 16–17/11 khi Đợt 2 chưa xong, theo quyết định của phiên lead dưới uỷ
  quyền trong chat ([`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md) §1). §1.h đã đo một commit có cả những
  thay đổi này, nhưng việc đó không làm chặng 3 qua cổng: cổng là R0, chạy sau khi Đợt 2 merge.
- **E2 không tính vào cổng chặng 4.** Cổng nhận kết quả null, nhưng chỉ khi thí nghiệm có sức mạnh đã khai
  trước. E2 và lượt chạy lại theo sửa đổi 1 (#59) null do hiệu ứng sàn (§3.1), nên chúng không phải thí
  nghiệm mà cổng này đòi.
  *(Note 2026-11-24: Stage 4 subsequently passed on `2026-11-24` via preregistered, powered run `E3b` (`#252`, `6d0fdeb2`, [`RESULT.md`](../../artifacts/experiments/e3b-productivity-gradient-body-mass/RESULT.md)); this historical note about `E2` stands.)*
- **Hai PR từng nhắm `main` thay vì thân cây** (luật L1 của kế hoạch 13/11) **đã được xử lý ngày 2026-11-17**
  theo quyết định của chủ dự án. Cả hai được đổi nhánh đích và merge vào thân cây: #56 (`CITATION.cff`), và
  #68 (test Rust khoá fixture E2 theo bản manifest của lượt chạy) sau khi CI của nó chạy lại xanh trên nền mới.

#### Chặng 1 đã đóng: bảo vệ nhánh bật ngày 2026-11-17

Cổng ra của chặng 1 trong [kế hoạch 13/11](DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md) §5 có hai vế.
Vế đầu là CI xanh cả hai job trên thân cây (§1.g, và mọi PR merge ngày 16–17/11). Vế sau là hai job đó thành
gate bắt buộc; chủ dự án bật nó ngày 2026-11-17. Kiểm bằng
`gh api repos/DuongNAD/Anima-Engine/branches/<nhánh>/protection` cùng ngày:

| Nhánh | Check bắt buộc | `enforce_admins` | Force-push · xoá nhánh |
|---|---|---|---|
| `feat/ecological-selection` | `Rust (src-tauri)`, `Frontend` | `true`: áp cho mọi người, kể cả admin | cấm · cấm |
| `main` | `Rust (src-tauri)`, `Frontend` | `false`: chỉ áp cho người không phải admin | cấm · cấm |

Cả hai nhánh: `strict = false` (PR không phải cập nhật theo nhánh đích trước khi merge), không bắt buộc
review. Job `determinism` của workflow `hardening` **không** nằm trong danh sách bắt buộc.

Hai nhánh đặt khác nhau vì `ci.yml` chỉ chạy khi push vào `main` và khi có PR. PR merge vào thân cây, nên
check của nó chạy ngay trên PR. Còn commit mà `main` được fast-forward lên thì chưa có check nào cho tới khi
nó được push lên `main`; nếu bắt cả admin qua gate, GitHub sẽ từ chối chính lượt fast-forward đó.

Hệ quả cho người và agent:
- Mọi thay đổi vào thân cây phải qua PR, kể cả tài liệu. Push thẳng bị từ chối, vì không CI nào chạy khi push
  vào thân cây. Kiểm cùng ngày: cả 47 commit first-parent của thân cây từ 2026-11-14 đều là merge PR, nên luật
  này không đổi cách làm hiện có.
- PR chỉ merge được khi cả hai job xanh. Job Rust chạy khoảng 31 phút trên Windows (§1.g). Sau R1c, cây phụ
  thuộc lớn hơn nhiều: lượt chạy lạnh của #127 mất 1 giờ 31 phút (run `44319652578`), nên `timeout-minutes` là 120.
  Từ #127, cache Rust dùng `Swatinem/rust-cache` và chỉ được **lưu** khi CI chạy trên `main`. PR chỉ khôi phục cache
  đó, vì cache do một PR lưu thì chỉ chính PR ấy thấy. Hệ quả: fast-forward `main` sau mỗi lần đổi `Cargo.lock`,
  nếu không mọi PR sau đó đều build lạnh.
  **Đổi ngày 2026-11-19:** mọi PR trong ngày đều ghi "No cache found" và mất khoảng 95 phút. Hash môi trường Rust
  của PR (`e5832507`) khác hash `main` đã lưu (`44e942f4`), dù cùng rustc 1.98.1. Ngoài ra, job determinism lưu một
  bản 667 MB cho mỗi PR, làm đầy hạn mức 10 GB của repo. Từ PR sửa CI ngày 2026-11-19, cache được lưu từ `main` **và**
  từ thân cây (`feat/ecological-selection` có thêm trigger `push`; lượt chạy trên thân cây không bị huỷ giữa
  chừng). Khoá cache theo file `rust-toolchain.toml` thay vì hash môi trường. PR không lưu cache nữa, kể cả job
  determinism. PR khôi phục cache do nhánh đích của nó lưu.
- Gỡ hoặc nới: GitHub → Settings → Branches, hoặc `gh api -X DELETE …/branches/<nhánh>/protection`.

Chặng 2 đã đóng từ 2026-11-16 (C5 là §1.g). Theo thứ tự của kế hoạch 13/11, chặng kế là chặng 3: các thẻ G3,
G4, G5, G6, G9 và R0 của [kế hoạch swarm](../ai/planning/2026-11-05-swarm-master-plan.md) chưa có PR nào
(đối chiếu danh sách PR ngày 2026-11-17).

#### Done 2026-11-25 (merged into `feat/ecological-selection`)

- **#272** (`N3. I2`, merged `9a53bad6`, `src-tauri/src/core/island/inhabit.rs`, `src-tauri/src/core/world_record/`, `src-tauri/tests/island_determinism_tests.rs`): implemented host `Inhabit` actuation, release (`disconnect`/`kick`/death), `WorldRecordHeader.island` (`serde_json`) recording, and offline deterministic replay (`56bdf8ab` / `9a53bad6`).
- **#271** (`D15. WORLD-CHANGE-ADR`, merged `0e90f0d5`, [ADR-0016](../decisions/ADR-0016-changing-world-surface.md), [`2026-11-24-world-change-cards.md`](../ai/planning/2026-11-24-world-change-cards.md), [`docs/decisions/README.md`](../decisions/README.md)): added `status: proposed` changing world surface ADR (`QD-BIO-D3`, `WorldRecordHeader::geological_era`, 3-timescale architecture) and `W1–W7` implementation cards.
- **#273** (`MAP-D5`, merged `33a4c197`, [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md)): recorded Option B from the `WV2-PHYS2` note — provisional `C01` stays (`cbdd3ba9...`, no regeneration, `WV2-7` boots on `C01`), opened focused `WV2-PHYS3` (`PR #274`), and noted `#270` provenance gap (`preset_v2.toml` no longer reproduces `C01` on trunk; reproduce `C01` from `#249` `f195a361`).
- **#270** (`WV2-PHYS2`, merged `a146d2f2`, [note](../ai/planning/2026-11-24-wv2-phys2.md), `src-tauri/crates/anima-worldgen/src/`): reworked World v2 generator physics inside `anima-worldgen` while leaving `src/verify/**`, thresholds, and `artifacts/world-v2/` untouched; `S2_1 = P01` at `4096²` passed `9/15` gates (`0` spikes, `30` waterfalls, `40` caves, `16/16` zoned biomes) vs provisional `C01` `2/15`, with `6` hydrology/shoreline/lake gates remaining open (`caprock_tile_cv = 0.695`, `Hack R^2 = 0.614`, `b_down = 1.073`, `d_shore = 1.135`, `C3` `lake_frac = 0.0047` full-grid / `0.0084` land).

#### Done 2026-11-24 (merged into `feat/ecological-selection`)

- **#269** (`C14. HONEST-UI`, merged `617fb1c0`, `src/components/ResearchDashboard/CausalInterventionPanel.tsx`, `src/components/Map2/`, `tests/frontend/honestUi.test.tsx`): disabled unimplemented `GeneticInjection` in UI with explicit `FINDINGS.md#f-071` link and added visible Map2 `"Decoration, not simulated"` legend/labels for wildlife and crystals.
- **#268** (`D14. PLANT-EVO-ADR`, merged `01b24c37`, [ADR-0015](../decisions/ADR-0015-evolving-plants.md), [plant cards](../ai/planning/2026-11-24-plant-cards.md), [`docs/decisions/README.md`](../decisions/README.md)): added `status: proposed` evolving plants ADR (`QD-LIFE`, `QD-GENESIS` `G1`/`G2`/`G3`, `QD-PLAYRULES` #2) and `P1–P8` implementation cards.
- **#267** (`D21. DOCS-CLEANUP`, merged `40776625`, `STATE_OF_THE_PROJECT.md`, `TODO.md`, `docs/ai/planning/2026-11-24-v0..v11`, `docs/release/0.9.0.md`): recorded `§E` dated notes on `8` frozen planning/release docs, marked `60` `QD-OPENQ` slots `DECIDED` across `15` spec/protocol docs, and synced `STATE_OF_THE_PROJECT.md` through `aef12cb6`.
- **#266** (`N2. I1`, merged `65632382`, `src-tauri/crates/anima-domain/src/causal.rs`, `src-tauri/src/core/island/`, `src-tauri/src/core/observer.rs`, `src-tauri/src/core/experiment/manifest.rs`, `src-tauri/src/core/world_artifact.rs`, `src-tauri/tests/island_ingress_tests.rs`): implemented shared-island guest policy, reserved `ISLAND_GUEST_CAUSE_BASE` band (`0xFFFF_0000`), `G1`/`G2`/`G3` genesis-phase provenance, and zero-alloc `SharedGuestIntents` / `drain_guest_intents_system` ingress.
- **#261** (`mac/root-hygiene-1124`, merged `1803aa85`, `scripts/root_allowlist.json`, `docs/paper/data/README.md`): resolved the six `REVIEW` entries in `scripts/root_allowlist.json` (moved `baseline-2026-08-10.txt` and `baseline-2026-08-11.txt` under `docs/paper/data/`, deleted `debug_ecosystem.html` and `update_vegetation.js`).
- **#259** (`C13. BRAIN-BODY-ADR`, merged `1bd2483d`, [ADR-0019](../decisions/ADR-0019-brain-body-interface.md), [`docs/decisions/README.md`](../decisions/README.md)): added `accepted` (`QD-BRAINBODY` `D1–D5 = A`) brain-body interface ADR fixing the Double CPG-Slot Defect via per-edge heritable `JointCpgGene` while preserving fixed `EVOLVED_ARCH` (`15 -> 64 -> 64 -> 8`) and Schema 8.
- **#265** (`M3. WV2-5`, merged `e7b99745`, [note](../ai/planning/2026-11-24-wv2-5.md), `src-tauri/crates/anima-worldgen/src/bake/`, `scripts/grid_bake_world_v2.sh`, `tests/wv2_5_bake_tests.rs`): added offline World v2 bake engine (`anima-bake`, `JOB-W0`..`W6`, KTX2 BC4/BC5 encoder, `.tar.gz` packager, `QD-WV25` closure).
- **#264** (`C15. HOTLOOP-PERSIST`, merged `a0522af5`, [`FINDINGS.md`](../../FINDINGS.md), `src-tauri/src/core/environmental_systems.rs`, `telemetry_export.rs`, `simulation_state.rs`, `simulation_loop.rs`, `f067_census_zero_alloc_tests.rs`, `f068_morphology_cursor_restore_tests.rs`): fixed `F-067` (zero-alloc demographic census update in place) and `F-068` (morphology cursor sync across save/load, passing `50/50` `test_100_save_load_cycles`).
- **#255** (`mac/decisions-1124c`, merged `aef12cb6`, [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md), [`docs/ai/README.md`](../ai/README.md)): recorded `QD-SHAREDISLAND`, `QD-LIFE`, `QD-GENESIS`, `QD-REL09`, `QD-OPENQ`, `QD-OPENC`, `QD-PLAYRULES`, `QD-SIGNALS`, `QD-ECOSCALE`, `QD-WV25`, `QD-SCALE-TRIAL`, `QD-BRAINBODY`, `QD-BIO-D1..D4`, and `QD-ADR0014` owner decisions and authorizations.
- **#262** (`D22. ADR-STATUS`, merged `1991d924`, [ADR-0009](../decisions/ADR-0009-burn-022-flex-wgpu-upgrade.md), [ADR-0013](../decisions/ADR-0013-live-creature-lifecycle.md), [`docs/decisions/README.md`](../decisions/README.md)): accepted `ADR-0009` and `ADR-0013` (`QD-OPENQ` `D1=B`, `D2=A`, `D3=B`, `D4=A`, `D5=A`, folding `D13` physical growth & biological sexual reproduction).
- **#260** (`C12. E3B-PIN`, merged `7e383b8b`, `src-tauri/tests/prereg_e3b_manifest_tests.rs`): pinned `31` freeze-time `LIVE_KEYS` (`E3B_FREEZE_LIVE_KEYS`) and `28` observables (`E3B_FREEZE_OBSERVABLE_IDS`) with post-freeze legacy-default gate (`QD-BIO-D2`).
- **#263** (`D19. ECOSYSTEM-ADR`, merged `42ccaec8`, [ADR-0018](../decisions/ADR-0018-ecosystem-scales-and-cycles.md), [ecosystem cards](../ai/planning/2026-11-24-ecosystem-cards.md), [`docs/decisions/README.md`](../decisions/README.md)): added `status: proposed` open energy flow (`QD-BIO-D1`), scales & cycles (`QD-ECOSCALE`), and `E1–E7` cards.
- **#249** (`WV2-4`, [note](../ai/planning/2026-11-24-wv2-4.md), `artifacts/world-v2/`, `src-tauri/crates/anima-worldgen/src/verify/`): added `G-SHAPE` and `G-COMP v2` verifiers, pinned LF line endings for `artifacts/world-v2/`, and committed provisional `C01` World v2 artifacts (`MAP-D4`: `2/15` Card 3 gates passed).
- **#258** (`C5b. SEL2P2-VIAB2`, [results](../ai/planning/2026-11-24-sel2-p2-viab2-results.md), [SEL2-P2 draft](../ai/planning/2026-11-24-sel2-p2-preregistration-draft.md), `artifacts/experiments/sel2-p2/viab2/`): recorded `QD-SEL2P2-D1..D7` and ran the blind `200,000`-tick `VIAB-2` pre-check (`12/12` survival across all four arms, `min_N >= 59`, `0` `trait_mass`, `D2(b)` escalation not triggered).
- **#257** (`D20. AUDIT-FINDINGS`, [`FINDINGS.md`](../../FINDINGS.md)): filed biology-audit defects `F-069` through `F-080` verified against trunk files.
- **#256** (`D12. ISLAND-ADR`, [ADR-0014](../decisions/ADR-0014-shared-island-host-and-guests.md), [island cards](../ai/planning/2026-11-24-island-cards.md), [`docs/decisions/README.md`](../decisions/README.md)): added shared-island host/guest architecture ADR (accepted with `23/24` D-slots decided, `ISLAND-D7` deferred to `I10` under `QD-ADR0014` & `QD-SHAREDISLAND`) and cards `I1–I11`.
- **#253** (`C6. LAW1-DRAFT`, [draft](../ai/planning/2026-11-24-law1-fork-experiment-draft.md)): added body-only preregistration draft for `LAW1` (`QD-AE-C` 6-arm counterfactual law-fork experiment; `LAW1-D1..D6` subsequently decided via `QD-OPENQ`).
- **#252** (`C4. E3B-RESULT`, [`RESULT.md`](../../artifacts/experiments/e3b-productivity-gradient-body-mass/RESULT.md), [summary](../ai/planning/2026-11-24-e3b-results.md)): unblinded E3b confirmation results passing Stage 4 (`H1` and `H3` `insufficient_evidence` / `inconclusive_wide_ci`, `H2` `equivalent_to_zero` / `equivalent_to_zero_powered_null`; `headless adapter verified`).
- **#254** (`mac/fix-wr2-world-identity`, `src-tauri/tests/world_record_writer_tests.rs`): restored trunk test build by supplying `sim_world_sha256` after `#242` & `#246` merged.
- **#251** (`D10. DOC-REVIEW`, [`2026-11-15-three-machine-drive-swarm.md`](../ai/planning/2026-11-15-three-machine-drive-swarm.md), [`MAP_AND_ML_UPGRADE_RESEARCH.md`](../research/MAP_AND_ML_UPGRADE_RESEARCH.md), [`RD_TECH_UPGRADE_ASSESSMENT_2026-08-07.md`](../research/RD_TECH_UPGRADE_ASSESSMENT_2026-08-07.md)): audited and refreshed 3 overdue planning/research docs against trunk reality.
- **#250** (`D9. FINDINGS-SYNC`, [`FINDINGS.md`](../../FINDINGS.md)): marked `F-054` (`#197`) and `F-061` (`#198`) `FIXED`, kept `F-053` `mitigated` with trunk verification across `16` existing entries, and filed `F-067` & `F-068` (`F-055..F-060, F-062..F-065` were already marked `FIXED` in `#200`).
- **#248** (`mac/decisions-1124b`, [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md), [`docs/ai/README.md`](../ai/README.md)): recorded `2026-11-24` afternoon owner decisions and wave authorizations.
- **#246** (`C9. WR2`, [note](../ai/planning/2026-11-24-wr2-write-path.md), `src-tauri/src/core/world_record/`, `world_record_writer_tests.rs`, `world_record_zero_alloc_tests.rs`): implemented pre-allocated `WR2` ring buffer, background writer thread, and zero-alloc hot-tick gate.
- **#247** (`C5. SEL2-P2` draft, [SEL2-P2 draft](../ai/planning/2026-11-24-sel2-p2-preregistration-draft.md)): added body-mass-blind `SEL2-P2` preregistration draft (subsequently updated by `#258`).
- **#245** (`C3. E3B-RUN`, [note](../ai/planning/2026-11-24-e3b-run-execution.md), `artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/`): executed all `192` E3b confirmation runs (`64` seeds × `3` arms) on `Windows 1` (`body_mass_consulted: false`) and scoped post-run manifest test.
- **#244** (`D8. V-SPECS-2`, [V7](../ai/planning/2026-11-24-v7-causal-timeline-ui-spec.md)–[V11](../ai/planning/2026-11-24-v11-example-scenarios-spec.md), [H10/H11](../ai/planning/2026-11-24-h10-h11-release-gates.md)): added `status: proposed` specs for `V7–V11` and checklists `H10–H11` (subsequently decided via `QD-OPENQ`/`QD-OPENC`).
- **#242** (`WV2-3`, [note](../ai/planning/2026-11-24-wv2-3.md), `anmw_v3.rs`, `world_v2_sim_gate_tests.rs`, [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)): integrated `ANMW v3` reader, `WorldIdentity::sim_world_sha256`, and `G-SIM` harness.
- **#243** (`D1. STATE-SYNC` r3, `STATE_OF_THE_PROJECT.md`, `TODO.md`): reconciled state docs with trunk `8d523415`.

- **#233** (`D2. REL09-CI`, [release-build.yml](../../.github/workflows/release-build.yml), [CI note](../ai/planning/2026-11-24-rel09-ci.md)): added unsigned Release Build CI workflow (`workflow_dispatch` + `pull_request` on workflow path, `contents: read`, SHA-pinned actions, `upload-artifact` 14-day retention) proven green on `windows-latest` (`Anima-Engine_0.1.0_x64_en-US.msi` `39,321,600` B + `Anima-Engine_0.1.0_x64-setup.exe` `27,552,675` B; `66,435,040` B ZIP) and `macos-latest` (`Anima-Engine_0.1.0_aarch64.dmg` `42,271,041` B + `Anima-Engine.app`; `85,424,984` B ZIP) in run `45268921473`.
- **#235** (`D4. V-ADR`, [ADR-0013](../decisions/ADR-0013-live-creature-lifecycle.md), [`docs/decisions/README.md`](../decisions/README.md), [V0 spec](../ai/planning/2026-11-24-v0-develop-at-birth-spec.md), [V1 spec](../ai/planning/2026-11-24-v1-dynamic-fields-schedule-spec.md)): added `status: proposed` live lifecycle ADR and v1.0 specs for `develop_at_birth` / `DevelopedPhenotype` and opt-in `DynamicFields` schedule wiring (`D1–D5`, `D-V0-*`, `D-V1-*` open).
- **#239** (`D7. V-SPECS`, [V2](../ai/planning/2026-11-24-v2-species-profile-spec.md), [V3](../ai/planning/2026-11-24-v3-lifecycle-spec.md), [V4](../ai/planning/2026-11-24-v4-carcasses-decomposition-spec.md), [V5](../ai/planning/2026-11-24-v5-fields-plants-feedback-spec.md), [V6](../ai/planning/2026-11-24-v6-interventions-causeid-spec.md)): added `status: proposed` v1.0 vertical-slice card specs for `SpeciesProfile`, lifecycle, carcasses/decomposition EU ledger, coupled dynamic soil/plant feedback, and `CauseId` interventions (`D-V2-*..D-V6-*` open).
- **#238** (`D3. REL09-DOCS`, [INSTALL.md](../how-to/INSTALL.md), [RELEASE_CHECKLIST.md](../how-to/RELEASE_CHECKLIST.md), [0.9.0.md](../release/0.9.0.md), [release/README.md](../release/README.md)): added unsigned 0.9 install guide, release-gate checklist, and `status: proposed` `0.9.0` draft release notes with open Release Gates `1–4`.
- **#240** (`C2. E3B-SMOKE`, [smoke results](../ai/planning/2026-11-24-e3b-smoke-results.md), `artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/`): ran blind smoke triple on `Windows 1` (`seed = 999953`, `wp2_passed = true`, `WP-2(b1) = -0.4862814293602915`, `WP-2(b2) = -0.005175446571556897`, `max_abs_eu_drift <= 4.5547494664788246e-8 <= 1e-6`, `body_mass_consulted = false`, `0` `trait_mass` substrings), unblocking `C3. E3B-RUN`.
- **#236** (`C8. WR1`, [WR1 spec](../ai/planning/2026-11-24-wr1-world-record-format.md), `src-tauri/src/core/world_record/`, `world_record_format_and_immutability_tests.rs`, `world_record_no_feedback_tests.rs`): implemented `ADR-0008` H1 canonical segmented SHA-256 World Record v1 format and zero-feedback end-of-tick recorder (`LIVE_KEYS` unchanged at `31`).
- **#241** (`D1. STATE-SYNC` r2): reconciled `STATE_OF_THE_PROJECT.md` and `TODO.md` with trunk through `#199`/`#234` (`ed634d8c`).
- **#237** (`D6. DOC-EN`, [how-to/README.md](../how-to/README.md), [explanation/README.md](../explanation/README.md)): translated the 5 remaining Vietnamese public guides (`MAP_VISUAL_CHECK.md`, `OWNER_TRIAL_RUN.md`, `BENCHMARKING.md`, `ALTERNATE_EVOLUTIONARY_REGIMES.md`, `CREATURE_MORPHOGENESIS.md`) to English (`QD-DOCLANG`) alongside byte-identical `.vi.md` siblings (`AGENT_WORKFLOW.md` excluded).
- **#230** (`MAP-D3`, `QD-E3B-D1a`, `QD-WAVE1124`, [design §MAP-D3](../ai/design/2026-11-20-world-v2-design.md#map-d3-2026-11-24-card-3-gates-not-met-after-the-physics-rework), [`AGENT_WORKFLOW.md` §4](../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0)): recorded owner approval to leave Card 3 shortfalls unchanged and track `#[ignore]`d holding tests by name in `scripts/test_target_policy.mjs`, locked E3b as two-trophic (`live.predator_fraction = 0.3`), and recorded the `2026-11-24` three-machine wave authorizations & peer-review ring (`QD-WAVE1124`).
- **#232** (`D1. STATE-SYNC`): reconciled `STATE_OF_THE_PROJECT.md` and `TODO.md` with trunk through `#231` (`1a2d1d62`).
- **#234** (`C1. E3B-RUNNER`, [spec](../ai/planning/2026-11-24-e3b-runner.md), `src-tauri/examples/run_e3b_experiment.rs`, `src-tauri/tests/e3b_runner_tests.rs`): added release-only E3b runner (`--smoke`, resumable `--ensemble`, `--replay`, `--self-test`) with world-artifact refusal and `trait_mass` blindness enforcement (`177` desktop targets, `1,680` passed in `#234` report; zero edits under `src-tauri/src/**`/`scripts/`/fixtures).
- **#199** (`WV2-2`, [note](../ai/planning/2026-11-23-wv2-2.md), [`scripts/test_target_policy.mjs`](../../scripts/test_target_policy.mjs#L100-L150)): implemented `anima-worldgen` Stages `S4`/`S6`/`S7`/`S8`/`S9`, lean `4096²` multigrid pipeline, CLI, and `WV2-2-PHYS` Step B under `MAP-D2`/`MAP-D3` (`96 passed; 0 failed; 5 ignored` under `--profile worldgen-cpu`; `-- --ignored` fails `5/5` on Card 3 gates; single-run `4096²` CLI `1.165 GiB < 1.2 GiB` max RSS and deterministic `SHA-256` `e0db7dc2...`).

#### Done 2026-11-22 → 2026-11-23 (merged into `feat/ecological-selection`)

- **Viability & `E3b`:**
  - **#215** (`VIAB-1`, [results](../ai/planning/2026-11-23-viab1-results.md)): 55-cell / 1,920-run body-mass-blind exploration (`d90be0bd` + `4794b6fc`); `e3_b1_g4_fl1` was the 1/30 passing E3b cell (`pred_final > 0`: `0/0/0`), while `0/25` SEL2-P2 cells passed due to `mut0` collapse without scarcity (`6` exploratory seeds `scale=1.0`, `prey_cap=80`: `baseline_ref` `5/6` vs `scarce_mut0` `1/6`).
  - **#216** (`mut0` diagnosis, [note](../ai/planning/2026-11-23-mut0-collapse-diagnosis.md)): identified maladapted clonal founder rather than a code defect.
  - **#224** (`FOUNDER-2SEG`, [note](../ai/planning/2026-11-23-founder-2seg.md)): added opt-in 2-segment founder behind `live.founder_genotype`.
  - **#228** (`VIAB-1b`, [results](../ai/planning/2026-11-23-viab1b-results.md)): 72-run body-mass-blind check at `98054ba`; both `e3b_fg1_g4_fl1` and `_mut0` passed `12/12/12/12` survival and `min_N >= 10` (`pred_final > 0`: `12/11/12`, mean generations `3.091/2.111/3.041` and `3.170/2.178/2.993`).
  - **#214, #217, #218, #223, #229, #231** (`E3b` preregistration): statistics [template](../ai/planning/2026-11-23-prereg-statistics-template.md) (`#214`), initial E3b draft & Monte Carlo power analysis (`#217`, `#218`), owner decisions `QD-E3B` `D1–D5` (`#223`), E3b draft v2 (`#229`, [draft](../ai/planning/2026-11-23-e3b-preregistration-draft.md)), and **`E3B-FREEZE` (`#231`)** freezing [2026-11-24-e3b-preregistration.md](../ai/planning/2026-11-24-e3b-preregistration.md), 7 manifests (`src-tauri/tests/fixtures/experiments_e3b/`), `prereg_e3b_manifest_tests.rs`, and `scripts/verify_e3b_artifacts.mjs` (`npm run check:e3b`).
- **Hardening, lineage & gates:**
  - **#196** (`M2`, [gate_controls.md](../paper/data/gate_controls.md)): negative controls for 62 automated gates (`0 VACUOUS`, `0 UNMATCHED_FAILURE`), superseding draft `#188`.
  - **#197**: closed four formerly-vacuous workflow gates (`thread_rng_scan`, `check_test_targets_unscheduled`, `audit_toml_ignores`, `esel1_skip_zero_guard`) and fixed `F-054` content-based route-split detection in `scripts/check_bundle_budget.mjs`.
  - **#198**: kept `start_from_manifest_tests.rs` crate-local for `cargo-mutants` (`F-061`; marked `FIXED` in `#250`).
  - **#200**: resolved `F053-FIX` (mitigated via additive `live.cumulative_mean_lifespan`) and `F-055..F-060, F-062..F-065` with regression tests (`FINDINGS.md`).
  - **#207** (`F-066`): live births and deaths in the tracker with unique IDs behind opt-in `live.lineage_tracking` ([§3.15.1](#3151-việc-còn-lại--đọc-mục-này-trước)).
  - **#208** (`DOC8`), **#210**: automated doc-commands gate (`scripts/check_doc_commands.mjs` + `.github/workflows/docs-commands.yml`) and conflict-marker scan (`scripts/check_conflict_markers.mjs`).
  - **#203, #225, #226**: dependency advisory bumps (`url` `2.5.9` for `RUSTSEC-2026-0592`, `cookie` `0.18.3` for `RUSTSEC-2026-0594`) and dev/test dependency optimization to reduce Rust CI runtime.
- **Map v2 & architecture:**
  - **#195** (`WV2-1c`, [note](../ai/planning/2026-11-20-wv2-1c.md)): hybrid fast-path fluvial implicit solver and residual audit in `anima-worldgen`.
  - **#221** (`NR-SPIKE-PROTOCOL`, [note](../ai/planning/2026-11-23-nr-spike-protocol.md)), **#222** ([ADR-0012](../decisions/ADR-0012-ui-free-simulation-core.md), accepted UI-free simulation core; `ADR-0011` reserved for native renderer), **#223** (`MAP-D1`), **#227** (`MAP-D2` design amendments `A1, A2, A3, A5` in [World v2 design](../ai/design/2026-11-20-world-v2-design.md)).
- **Docs, governance & research:**
  - **#201, #202, #204, #205, #209, #213**: Diátaxis guides (`DOC1–DOC3`, `DOC5+6`, `DOC7` proposal in [proposal](../ai/implementation/2026-11-23-readme-claude-md-proposal.md), `CONTRIBUTING.md` CI alignment, and `CLAUDE.md`/`README` drift corrections).
  - **#206, #220**: committed research notes (`docs/research/`) and two external Gemini reports ([evaluation](../research/GEMINI_PROJECT_EVALUATION_2026-11-21.md), [LOD streaming](../research/OPEN_WORLD_LOD_STREAMING_RESEARCH_2026-11-19.md)).
  - **#211** (`P1`, [incidents.md](../paper/data/incidents.md)): 47-incident catalogue (`incidents.csv`) and `scripts/paper/incident_dates.mjs` verifier.
  - **#212, #219**: recorded `2026-11-23` owner decisions (`QD-OFFQ`, `QD-DOCLANG`, `QD-VIAB1C`, `R5-AUTH`) and refreshed `docs/how-to/AGENT_WORKFLOW.md`.

#### Done 2026-11-20 (merged into `feat/ecological-selection` & `main`)

- **#180** (`FIX-CAP2`): enforced strict caps in neutral-drift replay.
- **#181** (`E3-X`, [results](../ai/planning/2026-11-20-e3-x-confirmation-results.md)): 36-run E3 confirmation campaign on Windows 1 (`insufficient_evidence`, 6/12 valid pairs).
- **#182** (`SEL2P2-PREP`, [note](../ai/planning/2026-11-20-sel2p2-prep-grass-steering.md)): opt-in `live.grass_steering` and `missed_births_capped`.
- **#183, #190, #191**: recorded `F-052`, `F-053`, `F-054`, E3 outcome, `QD-E3HOST`, `QD-VIAB`, and end-of-day state (`#191`).
- **#184, #187, #189**: made English `README.md` default (`#187`) and synced `main` to `feat/ecological-selection` at `36234e47` (`#184`, `#189`).
- **#185, #192, #193, #194** (`Map v2`): plan & reference board (`#185`), accepted World v2 design `WV2-D` + `LD-1..LD-5` (`#192`), `WV2-1a` (`#193`), and `WV2-1b` (`#194`).
- **#186** (`FOUNDER-LAYOUT`, [note](../ai/planning/2026-11-20-founder-layout.md)): opt-in `live.founder_layout`, `SEL2-P` skip-zero guard, and `verify_e3_artifacts.mjs` shallow-clone fallback.

#### Done 2026-11-19 (merged into `feat/ecological-selection`)

- **#144, #146** (`F047`/`F048`, [note](../ai/planning/2026-11-19-f048-pending-inference-restore.md)): isolated desktop restore divergence and keyed restored `pending_inference` by `(lineage_id, request_id)` (superseding `#144`).
- **#145, #148, #149**: `S15` reference check (`#145`), `S2` observables (`#148`, [note](../ai/planning/2026-11-19-s2-observables.md)), and `QD-SEL2g` record (`#149`).
- **#147, #152, #165, #170, #173**: `SEL2-P` draft, implementation, hardening (`D1–D7`), blind 200k-tick smoke (`#170`, `gate_pass = false`, [smoke note](../ai/planning/2026-11-19-sel2-p-smoke.md)), and `QD-SEL2P2` close-out (`#173`).
- **#150, #151, #153, #154, #159, #161, #162, #168, #172**: 3D prototype & map visual updates (`V1` `#151`, `B1` `#153`, `capture:map` `#154`, `W2-lite` `#159`, `supercontinent` retune `#161`, `W3-lite` `#162`, `C5` composition gate `#168`, `W11` `#172`).
- **#155, #156, #157, #158, #160, #163, #171**: `ADR-0010` (`#155`), generated `docs/ai/README.md` index (`#156`), CI Rust cache from trunk (`#157`), trial guide (`#158`), `QD-ISLAND` & `QD-SCOPE` (`#160`), `QD-OXS` & `QD-ASSETS` (`#163`), bilingual READMEs + logo (`#171`).
- **#164, #167, #178**: `E3-P` preregistration freeze (`#164`), `esel1-runner --trial` (`#167`), and `E3-X-impl` runner/verifier (`#178`).
- **#166, #169, #174, #175, #176, #177, #179**: `LAND1` & `LAND1-T` (`#166`, `#176`), `F-050` Vitest warm-world (`#169`), `F051`/`F051b` world-source isolation (`#174`, `#179`), `utf8-chars` Apache-2.0 licence election (`#175`), and `FIX-CAP` `live.strict_prey_cap` (`#177`).

#### Done 2026-11-17 → 2026-11-18 (merged into `feat/ecological-selection`, extending `2026-11-17` list)

- **2026-11-16..17 UTC (`#56`, `#68`, `#92`–`#111`, `#113`, `#114`):** `CITATION.cff` (`#56`), E2 fixture pinning (`#68`), state/branch-protection records (`#92`, `#93`, `#96`), accuracy & decision ledger (`#97`), Wave 2 gates (`G4` `#98`, `G5` `#95`, `G6` `#94`, `G3` `#107`, `G9` `#114`), `experiment_runner` split (`#99`), `M1` cross-platform matrix & libm fingerprint (`#100`, `#102`), `REGROWTH_STRIDE` doc fix (`#101`), `F-024`/`F-026` & runner/hardening fixes (`#103`–`#105`, `#108`, `#113`), `S8` (`#106`), `L0`/`ADR-0008` draft (`#109`), `S6` (`#110`), and `R1c` handoff (`#111`).
- **2026-11-18 UTC (`#112`, `#115`–`#133`, `#135`–`#143`):** overdue doc review (`#112`), Wave 2 closed record (`#115`), `SEL2` redesign (`#116`), `S9` (`#117`, `#120`), `Burn` & law-fork research records (`#118`, `#122`), `ADR-0008` accepted (`#119`), `QD-WIN` & `QD-SEL2e` (`#121`), `R0` re-measurement at `46e25d5` (`#123`, `#126`, §1.i), Windows CRLF fix (`#125`), `R1c` `burn` `0.22.1` + schema 8 (`#127`, `#136`), `S10` (`#124`, `#129`), `SEL2-E` negative (`#128`, `#131`), `SEL2-D` (`#130`, `#132`), erosion gate thread fix (`#133`), `S11` closed unmerged as negative finding (`#134`, branch kept at `95044c4b`, [note](../ai/planning/2026-11-18-s11-grass-steering.md)), `S12` (`#135`), `M4` (`#137`), `M3` (`#138`), `S13` (`#139`), `F045` (`#140`), `picomatch` exemption dropped (`#141`), `S14` (`#142`), and `STATE` update (`#143`).

#### Đã xong 2026-11-17 (đã merge vào `feat/ecological-selection`)

- **#77** — E-SEL1: đăng ký trước được **đóng băng** (fixture, test hình dạng, `run_esel1_experiment`,
  `scripts/verify_esel1_artifacts.mjs`) và siết tám kẽ hở G1–G8; người kiểm chứng độc lập: VERIFIED.
- **#83** — **F-043**: `live.food_cap` chưa từng có tác dụng (bị ghi đè về 50 mỗi tick); sửa bằng `LiveFoodCap`,
  đường mặc định không đổi. Kèm sửa đổi 1 của E-SEL1 (ghi lại smoke lần 1, thêm kiểm tra thao tác).
- **#84** — sửa đổi 2: E-SEL1 **đóng tại smoke, không có thí nghiệm**. Có test giữ cho manifest thí nghiệm không
  chạy được.
- **#85** — test: agent hết năng lượng có `target_velocity` bằng 0 (P3).
- **#86** — cổng zero-alloc của emit chỉ đếm luồng của chính test (flake trên CI của #84, như #80).
- **#87** — thẻ B của ADR-0007: ba khoá opt-in cho thu nhập và nước của thú săn (QĐ-ECO-B = opt-in).
- **#88** — chẩn đoán sau E-SEL1, QĐ-SEL1b, F-044.
- **#89** — test khoá P1 (thú săn và con mồi cùng sống tới t = 6.000 ở ≥ 3/10 seed; xanh trên CI Windows), QĐ-ECO-B,
  bằng chứng cho phương án C trong ADR-0007.
- **#90** — test đặc tả F-044 (founder đói lâu chết ngay khi cạn nước; đối chứng no sống trọn 9 tick ân hạn).
- **#91** — `tokio` thu hẹp từ `full` về năm feature đang dùng (§3.5).

#### Đã xong 2026-11-16 (đã merge vào `feat/ecological-selection`)

- **#59** — E2 chạy lại theo sửa đổi 1 (QĐ-E2M = B): H1 vẫn null, `check:e2` xanh trên thân cây.
- **#46, #47, #48, #49** — cổng: clippy của thân cây, spec E2E được khoá danh sách (G2), zero-alloc khi
  sinh/chết (G1), khôi phục các assertion đã nới (G7).
- **#50** — F-037 ân hạn đói, F-038 trần kích thước lai ghép, F-039 `live.births`.
- **#64** — F-040: restore sau ca chết đi đúng quỹ đạo gốc.
- **#65** — F-041: MỘT thứ tự tổng cho lịch tick; cùng seed là cùng một thế giới ở mọi tiến trình.
- **#70** — F-042 được ghi nhận kèm test đặc tả; **#72, #74** — hai test CI chập chờn được làm tất định.
- **#75** — OSS-072: MRCA của DAG là một tập (lệnh IPC `get_lineage_mrca`; thay #31).
- **#61** — ADR-0005 và ADR-0006 `accepted` (QĐ-4 = A, QĐ-5 = có, QĐ-LOC = A), do phiên lead quyết theo
  uỷ quyền của chủ dự án trong chat; chủ dự án đảo lại được (xem [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md) §1, §4).
- **#66** — Card S1: khoá `live.reproduction` (vắng = tắt) và test (a)–(e) của thẻ.
- **#76** — bộ lái vận động **opt-in** (`live.locomotion`, ADR-0006 phương án A): sinh vật dịch chuyển thật
  khi bật; thế giới mặc định không đổi. Chi tiết và số đo ở F-042.
- **#51, #57, #71** — kế hoạch "thế giới đọc được", §1.g, và mục này.
- **Thẻ thế giới đọc được** ([kế hoạch ba máy](../ai/planning/2026-11-16-readable-world-three-machines.md)):
  RW-A1 #54, RW-A2 #60, RW-A3 #63, RW-B1 #52, RW-B2 #55, RW-B3 #62, RW-C1 #53, RW-C2 #58, RW-C3 #67, RW-C4 #69.

#### P1 — Thú săn sống được: ĐÃ ĐẠT định nghĩa hoàn thành (2026-11-17); S43 vẫn mở

**Đã làm.**
- **#87**: thẻ B của ADR-0007, ba khoá opt-in `live.kill_hydration`, `live.kill_efficiency`,
  `live.target_min_energy`. Mặc định không đổi; QĐ-ECO-B = opt-in.
- **Test khoá P1** (`predator_prey_coexistence_tests`, PR này): cấu hình thẻ A + B cho thú săn và con mồi cùng
  sống tới t = 6.000 ở **5/10 seed**. Bỏ thẻ B thì chỉ còn **1/10**, và test đỏ.
- EU kín, tất định nhiều tiến trình và restore đã được `eco_card_b_tests` kiểm.

Vậy định nghĩa hoàn thành dưới đây đã đạt. Nó **không** có nghĩa S43 hay Red Queen đã có.

**Còn lại.**
- **Bằng chứng cho phương án C** ([ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md), phần "Khoá
  P1 và bằng chứng cho phương án C"):
  - Thú săn founder sinh sản trước lần săn đầu tiên, rồi chết đói trên đường tới con mồi.
  - Giảm chi phí thú săn × 0,5 kèm bán kính đòn 5 cho 8/10 seed.
  - Seed còn hỏng là do con mồi thiếu nước.
- **Quyết định của chủ dự án — đã ra ngày 2026-11-17: làm cả hai, opt-in** (thẻ S7; bản ghi ở
  [roadmap 31/10 §4.1](COMPLETION_ROADMAP_2026-10-31.md#41-quyết-định-sau-2026-11-14-ghi-thêm-2026-11-17)).
  Câu hỏi gốc:
  - QĐ-ECO-C1: có làm thẻ opt-in `live.predator_cost_mult` + `live.strike_radius` không, kèm phép đo đăng ký trước
    trên ≥ 20 seed.
  - Có làm một khoá về nguồn nước không. Khoá đó phục vụ cả S43 lẫn thí nghiệm chọn lọc.

**Định nghĩa hoàn thành (đạt):** một cấu hình được khai báo (khoá manifest, vắng = như cũ) cho thú săn và con mồi
cùng sống tới t = 6.000 ở ≥ 3 seed, EU kín, tất định nhiều tiến trình; test hành vi khoá lại.

#### P2 — E-SEL1: ĐÃ ĐÓNG tại smoke, không có thí nghiệm (2026-11-17)

**Đã xảy ra.** Smoke lần 1 dừng ở bước C và tìm ra F-043 (#83). Smoke lần 2 trên bản đã sửa qua kiểm tra thao
tác, tức trần thức ăn giờ thay đổi thế giới thật. Nhưng hạ trần từ 50 xuống 25 / 17 / 13 / 6 chỉ làm standing crop
của đồng cỏ đổi ±6 % (r từ 0,943 đến 1,027; thuật toán cần 0,4–0,6). Theo điều sửa đổi 1 đã cam kết trước, bản đăng
ký **đóng, không có thí nghiệm** (sửa đổi 2). Các manifest thí nghiệm giữ giá trị đệm âm nên không chạy được, có test
khẳng định. Không seed thí nghiệm nào chạy; không giá trị khối lượng cơ thể nào được đọc. Bản ghi hai lần smoke ở
`artifacts/experiments/e-sel1-smoke-record/`; chi tiết ở
[kế hoạch E-SEL1](../ai/planning/2026-11-16-experiment-e-sel1-food-scarcity-body-mass.md).

**Quyết định của chủ dự án: QĐ-SEL1b — đã ra ngày 2026-11-17: phương án A** (thẻ S6, rồi SEL2; F-044 sửa
trước ở thẻ S8). Câu hỏi gốc: có làm lại thí nghiệm chọn lọc theo khan hiếm không, và với yếu tố nào.
Bằng chứng là [chẩn đoán sau E-SEL1](../ai/planning/2026-11-17-esel1-scarcity-diagnosis.md): đo thăm dò, mù với
khối lượng cơ thể, seed 1–6. Nó cho thấy:
- Gặm đồng cỏ là 90–93 % thu nhập của con mồi; `Food` chỉ vài phần trăm.
- Đồng cỏ không bao giờ cạn, vì sinh sản dừng cứng ở 80 con mồi (`agent_systems.rs:715`).
- Nước, không phải thức ăn, giới hạn quần thể.
- Chỉ hạ sức chứa đồng cỏ mới đưa standing crop vào dải 0,4–0,6.

Phương án:
- **A** (khuyến nghị): khoá trần sinh sản `live.prey_cap`, rồi khan hiếm đồng cỏ; đo thăm dò trước khi đăng ký.
- **B**: khan hiếm nước.
- **C**: chỉ hạ sức chứa.
- **D**: dừng dòng này, dồn sức cho S43.

Có hai quan sát từ một seed, chưa kết luận:
- Nhánh `live.mutation_rate = 0` tuyệt chủng ở mọi lượt smoke.
- Nhánh dồi dào có quần thể tối thiểu thấp hơn nhánh khan hiếm.

**Định nghĩa hoàn thành, nếu làm bản đăng ký mới:** đăng ký trước mới thành fixture; smoke qua kiểm tra thao tác và
bước C trước mọi seed thí nghiệm; kết quả, kể cả null, báo cáo theo mẫu E2.

#### P3 — Câu hỏi mở nhỏ đi kèm #76

`LIVE_MODEL_VERSION` chưa tăng vì đường mặc định không đổi (chủ dự án xác nhận hay tăng) · cảm biến và bộ lái
dùng khoảng cách Euclid không bọc biên, nên agent sát biên có thể đi đường vòng · bật vận động mặc định trong app
là một quyết định riêng sau khi xem số đo. (Test cho `target_velocity` về 0 khi năng lượng ≤ 0: xong ở #85.)

> **Note (2026-11-24):** `LIVE_MODEL_VERSION` was subsequently bumped to `"live-bevy-world/2"` in `R1c` (`#127`, [ADR-0009](../decisions/ADR-0009-burn-022-flex-wgpu-upgrade.md)); per `QD-OPENC` (`PR #255`, [`AGENT_WORKFLOW.md` §4](../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0)), `brain_metabolic_cost` and locomotion stay off by default in the desktop app until the island slice, and shared islands turn them on through the real-time ecology profile.

#### P4 — Phần còn lại của "thế giới đọc được" (phần cứng/quota, không phải thiết kế)

Ba tác tử Antigravity dùng chung một hạn mức, hết ngày 2026-11-16 (làm mới 2026-11-21 09:24 +07). Còn:
**RW-A4** (cần quyết định RW-G-A trên `anima-mac` sau RW-A3), **RW-B4** (chụp lại 8 ảnh chuẩn trên RTX 5060
Ti), **RW-C5** (bảng FPS của ba máy; cần RW-A4, RW-B3, RW-C4 — RW-C4 đã merge, #69). Neo và điều kiện từng thẻ ở
[kế hoạch ba máy](../ai/planning/2026-11-16-readable-world-three-machines.md).

**Định nghĩa hoàn thành:** từng thẻ theo phần "xong khi" của nó trong kế hoạch ba máy.

---

### 3.0 Phiên sau bắt đầu ở đây (bàn giao **2026-07-27**)

> **⚠️ Bàn giao dưới đây là của 2026-07-27 và đã cũ** *(ghi chú 2026-11-14)*. Việc hiện hành không
> còn nằm ở bảng này. Đọc theo thứ tự:
>
> 1. [Roadmap 31/10 §4](COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được) —
>    quyết định của chủ dự án. QĐ-A (nửa "cây nào là nền"), QĐ-D, QĐ-E đã quyết; QĐ-A′, QĐ-B,
>    QĐ-F028 quyết ngày 2026-11-14; **QĐ-C còn mở**.
> 2. [`EXECUTION_PLAN_2026-10-31.md`](EXECUTION_PLAN_2026-10-31.md) — task HT-A / HT-B / HT-C, mỗi
>    task có file, lệnh xác minh và phụ thuộc.
> 3. [Kế hoạch định hướng và điều phối 13/11](DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md)
>    (`proposed`) — chặng, luật điều phối giữa các kế hoạch, hàng đợi quyết định QĐ-M.
>
> **Giao việc cho coding agent:** đọc [hướng dẫn chạy agent](../how-to/AGENT_WORKFLOW.md) — hàng đợi
> thẻ chạy được ngay, khối `DECISIONS` đã điền sẵn để dán, luật và lệnh xác minh.
>
> Bảng và các bẫy bên dưới được giữ nguyên làm bản ghi bàn giao ngày đó; trạng thái từng việc phải
> đối chiếu với ba tài liệu trên.

> ### Bàn giao 2026-10-16 — đọc trước bảng bên dưới
>
> Thứ tự công việc nay nằm ở [lộ trình hoàn thiện](COMPLETION_ROADMAP_2026-10-16.md), và việc đầu
> tiên **không** còn là §3.2 mà là **HT0**: sao lưu việc chỉ nằm trên một máy, rồi đưa `main` về
> xanh (§1.f). Bảng bên dưới giữ nguyên làm bối cảnh — các mục của nó đã được xếp lại vào HT1–HT5.
>
> **Tiến độ HT0 (2026-10-16):** khối chưa commit đã nằm trên nhánh **cục bộ**
> `wip/2026-08-16-map2-tier5` (`84fece1`, 170 file khớp từng byte) kèm một git bundle ngoài repo — cố
> ý **không** push, vì repo public và khối đó có bằng chứng bịa. HT0.2 + HT0.3 là nhánh
> `fix/main-ci-green`, xanh cục bộ đủ 30 gate của CI (bảng trong §1.f); còn chờ push, PR và CI thật.
> `389de4b` vẫn chưa push.
>
> **Ba cái bẫy mới, trả giá trong khoảng 2026-07-29 → 2026-08-16:**
>
> - **Một khuyến nghị audit sai đã làm mọi agent mù.** SIM-01 của `AUDIT_REPORT.md` khuyên
>   `intersect_sphere` trả `Some(0.0)` khi tia xuất phát trong collider; agent mang collider của chính
>   nó tại gốc tia, nên raycast luôn trả về chính nó và `ai/model.rs` bỏ hit đó — mọi tia, mọi tick.
>   Cả suite vẫn xanh, và một fixture bị dời để che triệu chứng. Đã sửa ở `389de4b`. Finding của audit
>   tự động là **giả thuyết**: tái hiện bằng test trước khi sửa.
> - **Không ghim toolchain thì `main` có thể đỏ mà không ai sửa gì.** clippy 1.98 thêm
>   `chunks_exact_to_as_chunks` và hai file đã commit trở thành lỗi.
> - **Một gate có thể bị thu hẹp bằng cách sửa lệnh gọi nó.** Khối chưa commit đổi `test:e2e` để chỉ
>   chạy `e2e/tiers/` — exit 0, 270 passed, và 18 spec của CI không chạy. `check_test_targets.mjs`
>   chặn đúng chế độ hỏng này cho Rust; Playwright chưa có gate tương đương.

> **PR #21 đã merge vào `main` tại `a053083`** (2026-07-27), sau ba commit sửa hạ tầng test mà
> `068a750` chưa có. Bảng §1 vẫn đúng; cái mới nằm ở [§1.e](#1e-sau-pr-21--đo-2026-07-27-tại-0f5b4d3)
> — lượt CI đầy đủ đầu tiên xanh trên runner GitHub, và tám view chuẩn đo lại byte-identical.

> **Việc #1 của bàn giao trước đã XONG.** Số đếm đột biến theo node đã ship và nén lưu trữ đã bật —
> xem [§3.15.1](#3151-việc-còn-lại--đọc-mục-này-trước), nay chỉ còn OSS-072 (MRCA). Chi tiết cả đợt
> ở [`docs/ai/planning/2026-07-27-feature-anima-completion.md`](../ai/planning/2026-07-27-feature-anima-completion.md)
> và bảng bằng chứng ở [`docs/ai/testing/2026-07-27-feature-anima-completion.md`](../ai/testing/2026-07-27-feature-anima-completion.md).

| # | Việc | Vì sao là việc này | Đọc |
|---|---|---|---|
| 1 | **Chạy app desktop một lần với `ANIMA_TICK_CAPTURE`** | Việc duy nhất còn lại của §3.2, và **máy không làm được** — cần một con người bấm chạy. Dụng cụ đã ship và đã test; thiếu đúng một lần chạy | [§3.2](#32-thay-số-hiệu-năng-proxy-bằng-số-đo-thật--một-nửa-đã-xong-2026-07-26), [BENCHMARKING.md](../how-to/BENCHMARKING.md) |
| 2 | ~~**Quyết định EB-S04**~~ ~~**chạy thí nghiệm E2**~~ **CẢ HAI XONG 2026-07-27** → việc còn lại là **thí nghiệm kế tiếp**, không phải quyết định | E2 đã chạy một lần tại `9c57184`: H1 **null** (delta đúng bằng 0 trên 12/12 cặp), mặc định **giữ opt-in** (ADR-0003 quyết định 13). Số 0 là **hiệu ứng sàn**, không phải bằng chứng về não — cần một điểm đo **trước sàn** (T ≈ 3.000–6.000) và một quần thể **có luân chuyển** | [§3.1](#31-bật-não-tiến-hoá-per-agent-trên-đường-mặc-định) |
| 3 | **OSS-072 MRCA** | Nửa khoa học còn lại của phả hệ. Nén đã xong nên `simplify` đã có sẵn cấu trúc cha/con để dùng lại | [§3.15.1](#3151-việc-còn-lại--đọc-mục-này-trước) |
| 4 | ~~**In-app tick capture**~~ **XONG 2026-07-27** | Đã ship kèm 4 lệnh IPC và gate "đo không làm đổi quỹ đạo" | [§3.2](#32-thay-số-hiệu-năng-proxy-bằng-số-đo-thật--một-nửa-đã-xong-2026-07-26) |
| 5 | ~~**§3.3 adapter thí nghiệm cho thế giới sống**~~ **XONG headless 2026-07-27** | `LiveExperimentAdapter` qua runner chung, trên đúng lịch trình app chạy | [§3.3](#33-đưa-thế-giới-bevy-sống-qua-gate-thí-nghiệm--adapter-headless-đã-xong-2026-07-27) |

**Một việc cần con người, máy không làm được:** chạy `npm run tauri:dev` một lần để xác minh CSP mới
(`tauri.conf.json` nay có `csp` + `devCsp` thay cho `null`). `npm run check:csp` chỉ kiểm **artifact
đã build** so với chính sách đã khai; nó **không** chứng minh app khởi động được dưới chính sách đó,
vì CLAUDE.md cấm chạy full backend trên máy này.

**Bốn cái bẫy mới, trả giá ngày 2026-07-27:**

- **Một gate đặt trên artifact không thể thấy producer của artifact đó đã chết.** `npm run
  capture:views` — producer duy nhất của tám view chuẩn — **không khởi động được** vì webServer dò
  `127.0.0.1` còn Vite bind `::1`, và **không gate nào đỏ**: CI chỉ hash tám PNG đã commit. Ảnh vẫn
  xanh trong lúc khả năng tái tạo chúng đã mất. Sửa ở `0f5b4d3`, đo lại **8/8 byte-identical** ở
  [§1.e](#1e-sau-pr-21--đo-2026-07-27-tại-0f5b4d3).

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

**Hai finding đang mở — cái thứ hai nay ĐÃ đối chứng (2026-07-29):**

- Con số **~4,2 ms** trong doc comment của `ResourceField::REGROWTH_STRIDE` **không tái lập được** —
  release build cho ~0,36 ms, thấp hơn ~12 lần. Việc stride vẫn đúng (đo được 3,97×); chỉ con số
  biện minh là chưa đối chứng. *Đã đối chứng và đóng ở #101 (2026-11-17): đó là số của một bản debug.*

**Finding `DEFAULT_GRID_DIM` đã đóng — đừng mở lại từ một bản sao cũ.** Câu cũ ở đây nói hằng số là
**128**; nó là **256** kể từ 2026-07-27 và bị test `s03_default_grid_dim_tracks_map_settings_default`
buộc phải bằng `MapSettings::default().width`. `COORDINATE_CONTRACT.md` §"Backend sim (mặc định)"
đã ghi `200 / 256 = 0.78125`. Đường đo mới không dùng hằng số nào cả: tick capture đọc kích thước
**từ `ResourceField` của thế giới đang chạy** và ghi vào `CaptureExport.workload` kèm cờ
`dimensions_measured`, nên một số sai không thể lọt vào một report mà không lộ ra. Con số 128 còn sót
lại **chỉ** ở `config.gridDim` trong `benchmark_report.json` (file kết quả cũ), không ở đường code nào.

  > **Đã đối chứng 2026-07-29 — finding đúng, và rộng hơn mô tả trên.** `MapSettings::default()` là
  > `width: 256, height: 256` ([`terrain.rs:97`](../../src-tauri/src/core/terrain.rs)), frontend
  > cũng 256 (`SIM_ARTIFACT_SIZE`, `worldCache.ts`). Doc comment của hằng số **tự khai** là nó đến
  > từ `MapSettings::default` — tức nó mâu thuẫn với chính nguồn nó viện dẫn. Con số 128 đã lan ra
  > **bảy** chỗ coi nó là thẩm quyền: `COORDINATE_CONTRACT.md:117`, `SIMULATION_RULES.md:145`,
  > `MAP_MANIFEST.md:66`, `benchmark_report.schema.json`, `map_manifest.schema.json`,
  > `scripts/bench_baseline.mjs:51`, và **`map_manifest.json` đã ship với `gridDim: 128`**. Nó còn
  > bị **khoá bằng test** ở `src/__tests__/mapManifest.test.ts:26`.
  >
  > **Mã nguồn thì đúng** — các hàm toạ độ nhận `width`/`height` làm tham số, không đọc hằng số này.
  > Đây là lỗi **hợp đồng**, không phải lỗi runtime. Nhưng nó là hợp đồng mà bên thứ ba đọc để đặt
  > ô, nên vẫn cần sửa một lượt trên cả bảy chỗ, và phải sửa cả test đang khoá con số sai.
  >
  > **✅ ĐÃ ĐÓNG 2026-07-29.** `DEFAULT_GRID_DIM = 256`, và **chín** chỗ đã sửa — nhiều hơn bảy chỗ
  > liệt kê ở trên. Hai chỗ chỉ lộ ra khi quét lại, và một trong hai đắt hơn tất cả phần còn lại:
  > `scripts/gen_map_manifest.ts` mặc định `SIZE = 128`, tức **bộ xuất thật** (`npm run gen:manifest`,
  > cấp dữ liệu cho MCP map-vision) khai sai độ phân giải trên mọi manifest nó sinh ra — con số sai
  > tới được một *consumer*, không chỉ một tài liệu. Chỗ còn lại là bảng trạng thái ở
  > `WORLD_SIMULATION_PLAN.md`.
  >
  > Ba gate mới giữ nó không lệch lại: `default_grid_dim_matches_the_real_default_map` (hằng số vs
  > `MapSettings::default`), `published_world_units_per_cell_follows_from_the_grid_dim` (ghim con số
  > 0.78125 mà `COORDINATE_CONTRACT.md` công bố), và 256² nay có trong sweep round-trip S03 của **cả
  > hai** phía — trước đó độ phân giải duy nhất mà hợp đồng nêu tên lại là độ phân giải không phía
  > nào round-trip.
  >
  > **Không sửa `WORLD_DESIGN.md`**, dù nó cũng ghi 128². Chỗ đó là văn bản *phê bình* mô tả tình
  > trạng khi thế giới còn tách đôi — nó đúng tại thời điểm viết, và sửa số trong một bản ghi lịch sử
  > là viết lại lịch sử.

### P0 — Đóng vòng lặp khoa học

Đây là khoảng cách thật của dự án. Chất lượng kỹ thuật đã cao; cái thiếu là **bằng chứng
trên đường mặc định**.

#### 3.1 Bật não tiến hoá per-agent trên đường mặc định

**Vì sao P0.** Một run mặc định hiện cho **mọi agent dùng chung một `BrainModel`**. Đó
chính là gap mà [`docs/research/MAP_AND_ML_UPGRADE_RESEARCH.md`](../research/MAP_AND_ML_UPGRADE_RESEARCH.md)
gọi là lớn nhất. Toàn bộ máy móc đã có và đã test — nó chỉ đang tắt.

**Trạng thái thật (cập nhật 2026-07-27):** **12/12 gate EB pass** (bảng gate trong
[ADR-0003](../decisions/ADR-0003-evolved-per-agent-brains.md)). EB-S04 đã **hết** 🟡: chủ dự án
chọn phương án (1) của DEC-1 và gate được **re-baseline tường minh** về bản dựng có seed
(ADR-0003 quyết định 12 + mục "Cập nhật 2026-07-27"). Đo tại `2b61d07`:
`cargo test --features desktop --test brain_controlled_comparison_tests` → **11 passed; 0 failed**.

**Điểm neo:** `BrainPolicy::default()` tại
[`src-tauri/src/core/resources.rs`](../../src-tauri/src/core/resources.rs) — `evolved: false`,
`lifetime_learning` tắt, `brain_metabolic_cost: 0.0`. Cờ `ANIMA_EVOLVED_BRAINS`,
`ANIMA_LIFETIME_LEARNING`.

~~**Việc thật cần làm trước tiên là quyết định về EB-S04, không phải lật cờ.**~~ **XONG
2026-07-27.** Mốc cũ đòi trùng bit với bản dựng *trước* ADR-0003, và bản dựng ấy **không có quỹ
đạo tất định nào** để mà trùng (RNG tĩnh toàn tiến trình, materialize lười). Mốc mới là **bản dựng
có seed, `BrainPolicy::evolved = false`**, với hai mệnh đề trùng bit — cùng seed hai lần chạy, và
van mở so với bố cục trước bước 4. Lý do bỏ mốc cũ và cái bị mất khi bỏ nó nằm ở ADR-0003 mục
"Cập nhật 2026-07-27".

**Còn lại của §3.1 — và nó là việc khoa học, không phải lật cờ.** Quyết định mặc định
(`evolved: true` hay giữ opt-in) cần **bằng chứng đối chứng nhiều seed được tiền đăng ký trước khi
chạy**. Re-baseline chỉ trả lại một baseline có thật để so; nó không nói gì về hướng của treatment.

> **Bổ sung 2026-11-16 (QD-E2M = B):** các manifest bị `970d963` thêm `live.dormant_eu`, nên artifact
> tháng 7 không còn khớp byte (gate `check:e2` đỏ). Đã sửa bằng **đính chính có ngày**
> (`amendment_1_live_dormant_eu`, không đổi tham số khoa học) và **chạy lại thật** tại `900ba2d`
> trên macOS arm64: H1 vẫn **null**, delta đúng 0 trên 12/12 cặp, thời điểm chạm sàn trùng từng seed
> (kiểm bằng so sánh). Artifact tháng 7 được giữ ở `superseded/`. So sánh và các điều chưa kiểm:
> `artifacts/experiments/e2-evolved-brain-default/RESULT.md`.
>
> ### ✅ E2 ĐÃ CHẠY 2026-07-27 tại `9c57184` — H1 **null**, mặc định **giữ opt-in**
>
> Chạy **một lần**, 12/12 cặp hoàn chỉnh, 157,2 s (2,62 phút) trên trần 90 phút, T = 18.000 (bậc
> trên cùng, không hạ bậc), 0 lỗi / 0 cảnh báo. `paired_mean_delta` của
> `live.mean_agent_energy` = **đúng 0,000000** trên **cả 12 seed**, phương sai ghép = 0, `d_z` không
> xác định ⇒ **không vật chất** theo quy tắc đã đăng ký ⇒ **`evolved` giữ opt-in**
> (ADR-0003 quyết định 13).
>
> **Nhưng số 0 này gần như không mang bằng chứng về não.** Ở T = 18.000, cả hai nhánh trên cả 12
> seed đều đã ở đúng 0 năng lượng hàng nghìn tick: engine **không có cái chết vì đói**
> (`update_agent_evaluation_system` gặp `energy <= 0` thì `continue`) và adapter **không chạy thay
> thế tiến hoá** (E2-F3), nên `live.agent_count` = 10 ở mọi mẫu và quần thể đóng băng. Điểm đo cuối
> nằm **~9.000 tick sau khi** phép đo hết khả năng phân biệt.
>
> Thứ **duy nhất** mang thông tin: `live.closed_eu_total` lệch **−2,6 × 10⁻⁹ EU trên 1,5 × 10⁵ EU**
> — não riêng theo cá thể **không** làm rò rỉ năng lượng ngoài sổ cái, đo trên 12 cặp.
>
> Smoke tìm ra **hai lỗi thứ tự lịch trình** trước khi khoá T (`993a587`, `ec94933`): một run tái lập
> **trong** một tiến trình nhưng **không** giữa các tiến trình, và lỗi thứ hai chạm chính metric
> **chính**. Đã sửa, đã chấp nhận lại bằng **24 tiến trình độc lập cho một kết quả trùng bit**, rồi
> mới chạy ensemble. Chi tiết:
> [`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md).
>
> Trạng thái thế giới sống **không đổi**: **headless adapter verified**.

**Tiền đăng ký E2 đã xong 2026-07-27 — đoạn dưới mô tả trạng thái LÚC ĐĂNG KÝ, giữ nguyên làm chứng
cứ rằng bản đăng ký có trước lần chạy.** Câu hỏi cố định, giả thuyết có hướng,
metric chính/phụ chọn từ đúng 11 observable mà `LiveExperimentAdapter` phát ra, N = 12 seed liệt kê
tường minh, T = 18.000 tick, ngưỡng "khác đủ nhiều", ngữ nghĩa seed và ranh giới suy luận, ước lượng
chi phí và quy tắc quyết định cho ADR-0003 — tất cả nằm ở
[planning E2](../ai/planning/2026-07-27-experiment-e2-evolved-brain-default.md) (kèm
[requirements](../ai/requirements/2026-07-27-experiment-e2-evolved-brain-default.md),
[design](../ai/design/2026-07-27-experiment-e2-evolved-brain-default.md),
[testing](../ai/testing/2026-07-27-experiment-e2-evolved-brain-default.md)), manifest máy đọc được ở
`src-tauri/tests/fixtures/experiments_e2/`, gate `tests/prereg_e2_manifest_tests.rs` **9/9**.

**Hai điều kiện chặn phải mở trước khi chạy** (đặc tả chốt sẵn ở design §3, E2-B **không** được
thiết kế lại): `build_live_world` chèn cứng `BrainPolicy::default()` nên không manifest nào xin được
`evolved = true`; và `live_experiment::genesis` **không** tạo `AgentBrain` kể cả khi cờ bật, khác
genesis của app. **Nhánh treatment hiện chưa tồn tại.**

**Định nghĩa hoàn thành:** ~~EB-S04 chuyển 🟡 → ✅~~ **xong** · ~~quyết định mặc định được ghi thành
mục quyết định trong ADR **sau** khi E2 chạy xong theo đúng tiền đăng ký~~ **xong — ADR-0003 quyết
định 13, `evolved` giữ opt-in** · điều kiện "nếu bật mặc định thì `cargo test --features desktop` vẫn
xanh và EB-S03 (`allocs == 0`) vẫn giữ" **không kích hoạt**, vì mặc định không bị lật.

**§3.1 đóng lại như một quyết định, và mở ra như một câu hỏi.** Cái đã xong là *quy trình*: có
baseline thật, có tiền đăng ký, có thí nghiệm chạy đúng một lần, có quyết định ghi theo quy tắc viết
trước. Cái **chưa** có là câu trả lời khoa học — E2 đo được rằng ở T = 18.000 không có gì để đo, chứ
không đo được rằng não riêng không quan trọng. Việc kế tiếp là một tiền đăng ký mới với điểm đo trước
sàn và một quần thể có luân chuyển; xem
[`RESULT.md` §11](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md).

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
đối chứng, không phải lỗi đã xác định. **Đã đóng ở #101 (2026-11-17):** ~4,2 ms là chi phí của một bản
debug, chưa bao giờ là chi phí release; doc comment đã sửa và bench giữ đường cũ để so.

#### 3.3 Đưa thế giới Bevy sống qua gate thí nghiệm — **adapter headless đã xong (2026-07-27)**

**Trạng thái mới.** [`core/live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs) là
`LiveExperimentAdapter: ExperimentModel`, chạy qua **cùng** `experiment_runner` với
`ReferenceEvolutionWorld` và trên **đúng** lịch trình app dùng —
[`core/simulation_schedule.rs`](../../src-tauri/src/core/simulation_schedule.rs), hàm mà
`SimulationEngine::start` gọi. Gate:
[`tests/live_experiment_tests.rs`](../../src-tauri/tests/live_experiment_tests.rs) (**17 test**, đo
2026-07-27 tại `068a750`; con số **15** ở bản trước là số cũ, đã lạc hậu) —
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

#### 3.19 Đóng vòng sinh thái — **mục P0 mới, mở 2026-08-25**

> **Đây là khoảng trống mà §3.1 và §3.2 không mô tả được, vì nó nằm *giữa* các hệ thống chứ không
> *trong* hệ thống nào.** Bộ gate của dự án rất mạnh ở việc bắt lỗi bên trong một hệ thống — năng
> lượng có rò không, RNG có tái lập không, lịch trình có ổn định không. Không gate nào bắt lỗi kiểu
> "sinh thái này có ảnh hưởng tới tiến hoá kia không". Năm finding dưới đây đều exit 0 hôm nay.

> **Cập nhật 2026-11-17: phần cơ chế của mục này đã có trên thân cây, ở dạng opt-in.** Văn bản bên dưới
> mô tả mã ngày 2026-08-25 và được giữ nguyên làm bối cảnh. Đối chiếu mã và file test trên thân cây tại
> `6f14124`; không chạy lại test nào cho ghi chú này.
>
> - **Đã có, tắt mặc định.** Tử vong: `core/mortality.rs` với `MortalityPolicy` (khoá `live.mortality`,
>   `live.starve_ticks`), gate CA-S02 ở `tests/mortality_tests.rs`. Fitness sinh thái:
>   `evolution/fitness.rs` (khoá `live.ecological_fitness`, `live.w_surplus`), gate FI-S01…S04 và CA-S01 ở
>   `tests/fitness_tests.rs`. Trục MAP-Elites thứ hai nay là `trophic_ratio` (QD-S01…S03 ở
>   `tests/qd_tests.rs`). Observable tiến hoá và CA-S03 ở `tests/observables_tests.rs`. Nhánh H (F-022) đã
>   sửa từ `0b29312`. Bảng task từng nhánh M, F, Q, O, H ở
>   [task backlog](../ai/implementation/2026-08-25-feature-ecological-selection.md).
> - **Đường mặc định không đổi.** `FitnessRuleSetting::default()` vẫn là `Locomotion` và
>   `MortalityPolicy::default()` vẫn là tắt. Vì vậy câu "không cá thể nào chết đói" và "sinh thái không chạm
>   fitness" bên dưới vẫn đúng cho một run không khai khoá nào, và sai cho một run có khai.
> - **Nhánh R (sinh sản theo ngưỡng) vẫn 🔒 trong bảng task.** Sinh sản liên tục đã vào bằng đường khác:
>   khoá `live.reproduction` của thẻ S1 (#66, [ADR-0005](../decisions/ADR-0005-live-population-turnover.md)).
> - **Hai điều kiện tiên quyết ở cuối mục đã được giải quyết.** `AGENTS.md`: QĐ-B quyết ngày 2026-11-14
>   (uỷ quyền theo thẻ). F-017: conflict marker đã gỡ ở `bef7768`.
> - **Còn mở của định nghĩa hoàn thành:** số đo `full_tick` p50 ở 1.000 agent trên bản release. Đó là việc
>   của con người (thẻ H6). CA-S01 và CA-S03 tự khai phạm vi hẹp hơn dạng đầy đủ trong chính file test của
>   chúng; đọc ghi chú ở đầu hai file đó trước khi trích.
> - **Cập nhật 2026-11-24 (`E3b` & `QD-SCALE-TRIAL`):** Stage 4 đã qua cổng qua `E3b` (`#252`, `6d0fdeb2`, [`RESULT.md`](../../artifacts/experiments/e3b-productivity-gradient-body-mass/RESULT.md), `headless adapter verified`). Theo `QD-SCALE-TRIAL` (`PR #255`), các đảo thử nghiệm/demo và lượt chạy tay `H6`/`H7` nhắm tới `~100` cá thể (`~0.33 ms/tick` so với `5.64 ms/tick` ở `1,000` agent theo [`scaling.md`](../paper/data/scaling.md)); mốc `1,000` agent vẫn là đích benchmark dài hạn nhưng không còn chặn các thẻ khác.

**Khẳng định trung tâm, đọc mã 2026-08-25 (📋 chưa đo bằng lệnh nào):** toàn bộ tầng sinh thái —
NPP theo biome, Kleiber, Boltzmann, Holling II/III, Lindeman, mùa vụ — **không có đường nào chạm
tới hàm fitness**. Cả crate có đúng một định nghĩa fitness và nó là
`cumulative_distance + tick_count` ([`agent_systems.rs:528`](../../src-tauri/src/core/agent_systems.rs)),
còn `tick_count` tăng vô điều kiện mỗi tick
([`world_systems.rs:171`](../../src-tauri/src/core/world_systems.rs)) nên sau một epoch nó gần như
bằng nhau ở mọi cá thể. Không cá thể nào chết đói: `energy <= 0` chỉ `continue`
([`agent_systems.rs:452`](../../src-tauri/src/core/agent_systems.rs)).

**Đây cũng là nguyên nhân gốc của kết quả null ở §3.1.** Bản `RESULT.md` của E2 gọi số 0 là hiệu
ứng sàn và đúng — nhưng sâu hơn một bậc: **không tồn tại kênh nào để một bộ não tốt hơn biến thành
nhiều con cháu hơn**. Bất kỳ thí nghiệm nào về hành vi, chạy trên engine hôm nay, đều đo phương sai
của bộ tạo số ngẫu nhiên. Chạy lại E2 ở T thấp hơn sẽ cho một số khác 0 mà không mang thêm thông
tin nào về não.

| Finding | Nội dung | Nhánh |
|---|---|---|
| [F-018](../../FINDINGS.md) | Sinh thái không nối vào chọn lọc | F |
| [F-019](../../FINDINGS.md) | Không có cái chết; cơ chế trả EU thì **đã** viết xong | M |
| [F-020](../../FINDINGS.md) | `AgentEvaluation` tính đúng đại lượng cần rồi bị vứt (`_eval` ở `:523`) | F |
| [F-021](../../FINDINGS.md) | Trục MAP-Elites thứ hai **bằng** fitness trừ một hằng số | Q |
| [F-022](../../FINDINGS.md) | Bảy hệ thống O(agent × segment); mẫu sửa đúng đã có trong cùng file | H |

**Bộ tài liệu:**
[requirements](../ai/requirements/2026-08-25-feature-ecological-selection.md) ·
[design](../ai/design/2026-08-25-feature-ecological-selection.md) ·
[plan](../ai/planning/2026-08-25-feature-ecological-selection.md) ·
[task backlog](../ai/implementation/2026-08-25-feature-ecological-selection.md) ·
[testing](../ai/testing/2026-08-25-feature-ecological-selection.md) ·
[bản giao việc](../ai/planning/2026-08-25-agent-goal-ecological-selection.md).

**Định nghĩa hoàn thành** (đầy đủ ở requirements §5): ba gate **kênh nhân quả** CA-S01…CA-S03 xanh
**kèm đối chứng âm** — mỗi gate phải **đỏ** trên bản dựng cũ, vì một gate chỉ xanh trên bản mới có
thể đang khẳng định một tautology · mọi cờ mới tắt ⇒ checksum **trùng bit** với hôm nay · residual
EU ≤ `1e-3` ở **mọi** cấu hình cờ · một số đo `full_tick` p50 ở **1000 agent**, bản release.

**Hai điều kiện tiên quyết thuộc về chủ dự án, không phải việc kỹ thuật:**

1. [`AGENTS.md`](../../AGENTS.md) khai `src-tauri/src` là **chỉ đọc** và `src-tauri/tests` là vùng
   ghi **duy nhất** (*"Vi phạm = task fail, không ngoại lệ"*). Hợp đồng đó viết cho agent săn
   mutant nhưng phát biểu ở phạm vi toàn repo, còn feature này bắt buộc ghi vào `src-tauri/src`.
   Cần **miễn trừ tường minh**, hoặc thu hẹp phạm vi của `AGENTS.md`.
2. [F-017](../../FINDINGS.md) vẫn OPEN: [`scripts/check_test_targets.mjs`](../../scripts/check_test_targets.mjs)
   đã commit với **ba khối merge conflict còn nguyên**, nên cổng đó chết khi parse và `npm run lint`
   đỏ. **Baseline ratchet 0 ở §1.b không phản ánh cây mã trên nhánh `feat/living-world` hôm nay.**

---

### P1 — Nợ nền tảng đã biết

#### 3.4 Gate `burn`/`wgpu` sau feature flag — ✅ **XONG (xác minh 2026-07-29)**

> Mục này **đã hoàn thành** nhưng vẫn nằm ở P1 mở cho tới khi được đo lại. Cả ba điều kiện của
> "định nghĩa hoàn thành" bên dưới đều đạt:
>
> - `cargo tree --no-default-features -e normal` khớp đúng pattern CI (`neo4rs|tokio-tungstenite|
>   burn-wgpu|naga`) cho **0** kết quả. `burn-core`/`burn-tensor`/`burn-autodiff` vẫn có mặt và
>   **đúng như thiết kế** — đó là learner CPU, tức chính đường fallback.
> - Gate `cargo tree` trong [`ci.yml`](../../.github/workflows/ci.yml) đã phủ `burn-wgpu` và `naga`.
> - Hình dạng code đã tái cấu trúc: `learn_handle` nay nằm sau `#[cfg(feature = "ml-wgpu")]`
>   ([`simulation_loop.rs:216`](../../src-tauri/src/core/simulation_loop.rs)), và `burn-wgpu` là
>   `optional = true` sau feature `ml-wgpu` ([`Cargo.toml:45`](../../src-tauri/Cargo.toml)).
>
> Mô tả blocker bên dưới giữ lại làm **bối cảnh lịch sử**, không phải việc cần làm.

**Blocker (lịch sử) là hình dạng code, không phải khối lượng.** `learn_handle` tại
[`src-tauri/src/core/simulation_loop.rs:182`](../../src-tauri/src/core/simulation_loop.rs)
được gán từ một `if has_wgpu { … } else { … }` mà hai nhánh **không `cfg` riêng từng nhánh được**.
Cần tái cấu trúc chỗ đó, không phải rắc attribute. `burn` chỉ dùng ở **hai** file — bảy kết quả
grep còn lại là văn xuôi khớp chữ "burned energy".

**Định nghĩa hoàn thành:** `cargo tree --no-default-features` không còn `burn-wgpu`/`naga` ·
gate `cargo tree` trong CI mở rộng để phủ chúng · fallback CPU vẫn chạy.

#### 3.5 Thu hẹp `tokio = { features = ["full"] }` — ✅ **XONG (2026-11-17)**

[`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml) giờ là
`default-features = false, features = ["rt-multi-thread", "macros", "sync", "time", "net"]`.
Mỗi feature có điểm dùng thật: `rt-multi-thread` (`runtime::Builder::new_multi_thread` ở
`core/simulation_loop.rs` và `evolution/lineage.rs`), `macros` (`#[tokio::test]`, `select!`, `join!`),
`sync` (`Mutex`, `Semaphore`), `time` (`sleep`/`timeout`/`Instant`, và `enable_all()` cần nó),
`net` (`TcpListener`/`TcpStream` trong `core/networking_systems.rs` và test migration). Không có
chỗ nào dùng `tokio::io`, `fs`, `process`, `signal`.

**Bằng chứng (macOS, 2026-11-17):** `cargo clippy --all-targets -- -D warnings` sạch ở cả cấu hình
mặc định và `--features desktop` · `cargo test --features desktop` 1434 pass / 0 fail / 2 ignored và
`check_test_targets.mjs --profile desktop` xanh · `cargo test` mặc định 1408 pass / 0 fail / 2 ignored và
`--profile default` xanh · `cargo build --release --features desktop` xanh · `Cargo.lock` không đổi · `cargo tree --no-default-features` vẫn 0 crate bị gate.
Lưu ý: `NOTICE` báo cũ khi chạy `gen_notice.mjs --check` trên macOS **cả trước lẫn sau** thay đổi này
(file được sinh từ đồ thị host Windows), nên không được tái sinh ở đây.

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

> *Cập nhật 2026-11-17:* tiêu đề mục này đã cũ. ADR-0004 được chấp nhận từ 2026-07-26, và **ADR-0002 được
> chủ dự án chấp nhận ngày 2026-11-17, trong phạm vi lát headless** (mục "Quyết định (2026-11-17)" của ADR).
> Không còn ADR nào đang đỡ tải mà ở trạng thái `proposed`. Văn bản dưới đây giữ làm bối cảnh.
> *Cập nhật 2026-11-24:* [ADR-0009](../decisions/ADR-0009-burn-022-flex-wgpu-upgrade.md) và [ADR-0013](../decisions/ADR-0013-live-creature-lifecycle.md) đã `accepted` trên thân cây qua `QD-OPENQ` (thẻ `D22`, `PR #262` đã merge); [ADR-0014](../decisions/ADR-0014-shared-island-host-and-guests.md) đã `accepted` trong `#256` (`QD-ADR0014`); [ADR-0018](../decisions/ADR-0018-ecosystem-scales-and-cycles.md) (`proposed`) đã merge trong `#263`; và `ADR-0019` được duyệt theo `QD-BRAINBODY` (`PR #255` đã merge, `PR #259` còn mở); xem [`docs/decisions/README.md`](../decisions/README.md).

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
> `tests/lineage_mutation_count_tests.rs` + `lineage_compaction_tests.rs` (số test của ngày đó là 7 và 9; file đầu nay có nhiều hơn). Cận
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

**(2) OSS-072 — truy vấn MRCA.** ✅ Xong và **đã merge vào thân cây ở #75** (2026-11-16): `evolution/mrca.rs`, lệnh `get_lineage_mrca`, `tests/lineage_mrca_tests.rs`; đáp án là một **tập**, không phải một node. Lượt đo toàn bộ đầu tiên có nó là §1.h. OSS-073 nay không còn bị chặn.

*Neo:* `evolution/simplify.rs` đã có sẵn cấu trúc cha/con và phép kiểm chu trình để dùng lại.

*Định nghĩa hoàn thành:* MRCA tất định, có test trên cây biết trước đáp án · xử lý đúng trường hợp
**DAG** (crossover cho hai cha, nên MRCA không nhất thiết duy nhất — phải quyết định trả về gì và
**ghi rõ**, đừng chọn bừa một nhánh) · trường hợp không có tổ tiên chung trả về gì cũng phải khai
báo, vì rừng nhiều gốc là chuyện bình thường ở đây.

Sau đó mới tới OSS-073 (giao thức đo "line of descent" kiểu Avida) — nó cần MRCA.

**(3) Ca sinh live vào tracker, id duy nhất, thời điểm chết (F-066).** ✅ **Xong, opt-in** (2026-11-22).
Trước đó OSS-073, cổng S43 và ứng viên loài AE-S11 bị chặn trên quần thể live bởi hai khe: ca sinh live
(`SpawnOffspringCommand`) không ghi cạnh nào vào tracker (chỉ epoch MAP-Elites gọi `add_reproduction`),
và id con `cha.thế_hệ` trùng giữa anh em (gốc của F-047). Nay khoá `live.lineage_tracking = 1` của
adapter headless bật [`core/live_lineage.rs`](../../src-tauri/src/core/live_lineage.rs): founder là root
`origin_tick = 0`, mỗi ca sinh có id `birth-{run}-{n}` riêng, một cạnh từ id riêng của cha (`Clone`/`Mutate`
theo việc genome thân hoặc não có thật sự đổi, so từng bit) và `origin_tick`; mỗi ca chết ghi
`destruction_tick`. Snapshot mang đồ thị, con trỏ id (`SavedSimulationState::live_lineage`) và id của từng
cá thể sống (`SerializedAgent::live_id`, khôi phục đúng id đó, không mint lại); save cũ vẫn đọc được. Gate:
`tests/live_lineage_tests.rs`.

- **Ghi phả hệ không đổi quỹ đạo** (ADR-0008 động lực 4, "không phản hồi"): cùng seed và manifest, chạy
  có khoá và không khoá cho cùng `world_checksum`, cùng số sinh/chết ở mọi mốc đo
  (`recording_lineage_does_not_change_the_trajectory`, so lượt chạy với lượt chạy, kể cả khi nén chạy).
  Lý do: `autonomous_reproduction_system` duyệt cha theo `AgentLineageId`, nên id duy nhất **không** ghi
  vào đó; `AgentLineageId` giữ giá trị cũ `cha.thế_hệ` ở mọi lượt chạy, còn id duy nhất nằm trong bảng phụ
  của `LiveLineage` theo `Entity`. Bản đầu của PR đã ghi đè `AgentLineageId` và làm lệch quỹ đạo (54 sinh /
  26 chết với khoá, 53 / 25 không khoá ở tick 2.500); bản sửa loại bỏ điều đó. Khoá vẫn opt-in vì tốn bộ nhớ
  và một bản sao genotype mỗi ca sinh; `LIVE_MODEL_VERSION` không đổi.
- **Còn lại:** app desktop chưa đặt khoá (Neo4j chưa có cột cho hai tick); nén vẫn nối đường đơn nhánh,
  nên một chuỗi genotype kiểu Avida theo từng bước dọc line of descent cần thêm tuỳ chọn tắt nén. OSS-073
  nay **làm được** trên quần thể live với đồ thị có thật; giao thức đo của nó vẫn chưa viết.
- **Lỗ thứ hai, vẫn mở** (rà soát 2026-11-22, [ghi chú](../research/OPEN_ENDEDNESS_LINEAGE_SPECIES_RESEARCH_2026-11-22.md)):
  `compact` giữ **số** đột biến qua chuỗi một con (`SimplifiedEdge.mutations`) nhưng giải phóng genotype
  trung gian, nên phân loại đột biến có lợi / trung tính / có hại kiểu Avida cần nhật ký diff đột biến theo
  cạnh hoặc ghim tổ tiên. Chưa có thẻ; đây là ghi nhận, không phải quyết định. (Lỗ thứ nhất của ghi chú đó,
  ca sinh live không vào tracker, chính là F-066 ở trên.)

**Hai cái bẫy đã trả giá rồi, đừng đạp lại:**

- **`add_reproduction` nay từ chối ghi cạnh có cha không tồn tại.** Đừng "sửa" nó thành ghi vô điều
  kiện: một cạnh mồ côi làm hỏng **toàn bộ** đồ thị, vì cả `to_newick` lẫn `simplify` đều từ chối xử
  lý đồ thị chứa nó.
- **Tập sample của `compact` không phải "ai đang sống".** Phải gồm mọi `lineage_id` trong archive
  MAP-Elites, vì một elite có thể được chọn làm cha ở epoch sau mà không phải tổ tiên của ai đang
  sống.

**IPC — câu cũ ở đây đã lỗi thời (đối chiếu 2026-11-17).** Nó nói `to_newick` và `simplify` chưa có lệnh
Tauri nào gọi. Trên thân cây, xuất Newick đã có lệnh `export_phylogenetic_tree` và MRCA có
`get_lineage_mrca`; cả hai có hàng trong [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md), nơi giữ hợp đồng
IPC từ 2026-11-14 thay cho `PROJECT.md`. `simplify` vẫn chưa có lệnh riêng: nó chạy bên trong `compact`.

**Neo4j:** `compact` chỉ co bộ nhớ trong. Khi Neo4j online, `get_lineage_graph` đọc từ database nên
vẫn trả đồ thị **đầy đủ**.

---

### P2 — Vệ sinh, làm được lẻ

| # | Việc | Điểm neo | Vì sao đáng làm |
|---|---|---|---|
| ~~3.9~~ | ~~Thêm `// SAFETY:` cho hai `unsafe impl Send/Sync`~~ **XONG** (`0d6b4b2`, 2026-07-27; ghi nhận 2026-11-17) | [`ai/model.rs`](../../src-tauri/src/ai/model.rs) | `unsafe impl Send` đã bị bỏ (mọi trường tự `Send`; test `brain_model_is_send_without_an_unsafe_impl` giữ cho nó không quay lại). Còn đúng **một** `unsafe impl Sync for BrainModel`, kèm luận chứng `// SAFETY:` và hai yêu cầu được test cấu trúc giữ. Dòng cũ ở đây ("2/2 khối không có luận chứng") đã lỗi thời |
| ~~3.10~~ | ~~Hoà giải trạng thái ADR-0002~~ **XONG 2026-11-17**: chủ dự án chấp nhận ADR trong phạm vi lát headless; mục "Quyết định (2026-11-17)" của ADR ghi phần nào được chấp nhận và phần nào vẫn là mục tiêu | [`ADR-0002`](../decisions/ADR-0002-world-laws-and-exotic-energy.md), trước đó `proposed` | AE1–AE3 đã ship. Theo quy tắc 6 của [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md), khi code và tài liệu xung đột thì **mở finding**, không tự coi code là đúng. **Đã lên giá:** ADR-0004 nay dựa vào ER01 của nó — xem §3.8 |
| ~~3.11~~ | ~~Giảm 491 warning ESLint~~ **XONG** (2026-07-27, `feature-anima-completion`) | `scripts/eslint_ratchet.mjs` baseline = **0** | 491 → 483 → 267 → **0**. Không nới rule, không thêm `eslint-disable`: sáu directive cũ đã biến mất, mọi `any` được thay bằng kiểu thật, và bốn rule React Compiler pass nhờ đổi code — frame loop đọc `state.scene`/`state.camera` thay vì đóng gói giá trị render, phần ghi three.js mệnh lệnh còn lại thành hàm có tên nhận đối tượng qua tham số. Ba finding hoá ra là lỗi thật (xem commit) |
| ~~3.18~~ | ~~Bỏ mọi lối thoát kiểu (`as any` / `as unknown as` / `as never` / `eslint-disable`) và bật typecheck cho `tests/`~~ **XONG** (2026-07-27, `feature-anima-completion`) | `npm run typecheck:tests` (gate mới trong CI) · [`src/window-globals.d.ts`](../../src/window-globals.d.ts) · [`tests/mocks/segment_fixtures.ts`](../../tests/mocks/segment_fixtures.ts) | Grep chính xác trên `src`/`tests`/`playground` **= 0** cho cả bốn mẫu, kể cả trong comment. Không suppress, không nới rule, không xoá test. Mỗi chỗ được gỡ bằng cách **làm cho điều đang bị khẳng định trở thành đúng**: các thuộc tính `window` được khai báo thật; `buildAgentHierarchy` nhận `readonly unknown[]` đúng như dữ liệu IPC nó vốn phòng thủ (lộ ra 2 lỗi thật: `segment_id` không phải số bị key bằng `undefined`, và `s.x \|\| 0` cho chuỗi lọt vào trường `number`); `biomeAt`/`findSpawn` nhận đúng các field chúng đọc; mock r3f dựng `THREE.Scene`/`PerspectiveCamera` thật. **Phát hiện phụ:** gói `tests/` chưa từng được typecheck — **86 lỗi** không lệnh nào báo, nay = 0 và có gate | |
| 3.12 | ~~Tách file lớn~~ **XONG (2026-11-17, #99)**: ba file nay là thư mục module | Lúc ghi: `experiment_runner.rs` 3.173 dòng · `experiment.rs` 2.192 · `exotic_energy.rs` 1.839 | Tiền lệ tốt đã có: `aae673e` tách learner và emit thread khỏi `simulation_loop.rs` |
| 3.13 | Nợ phụ thuộc | `burn` 0.13.2 (ghim, có lý do ghi trong CLAUDE.md) · `bevy_ecs` 0.13 · React 18→19 · `@react-three/fiber` 8→9 · `three` 0.184→0.185 | **Không phải nợ bảo mật** — advisory đã sạch và đã có gate. Là nợ framework |
| ~~3.14~~ | ~~Dọn tài liệu cũ ở root~~ **XONG** (ghi nhận 2026-11-17) | [`PHASE6_HANDOFF_REPORT.md`](../archive/PHASE6_HANDOFF_REPORT.md), [`PHASE1_EXECUTION_PLAN.md`](../archive/PHASE1_EXECUTION_PLAN.md) | Cả hai đã nằm trong `docs/archive/`; root không còn bản nào |
| 3.16 | Đóng phần còn nợ của OSS-003 *(hai hàng 3.16 trùng nhau được gộp 2026-11-17)* | [`licensing/UNRESOLVED.md`](../../licensing/UNRESOLVED.md) · [`LICENSE-MIT`](../../LICENSE-MIT) / [`LICENSE-APACHE`](../../LICENSE-APACHE) · [`NOTICE`](../../NOTICE) · [`CONTRIBUTING.md`](../../CONTRIBUTING.md) | **Licence của chính dự án: xong 2026-08-08.** Relicense sang `MIT OR Apache-2.0`; `LICENSE` proprietary đã xoá; phạm vi code / asset / định dạng `.anmw` tách trong README; `license` khai trong `[workspace.package]` và cả hai `package.json`. **Hồ sơ bên thứ ba: phần kỹ thuật xong 2026-07-27.** `NOTICE`, `licensing/THIRD_PARTY_LICENSES.txt` và SBOM đều **sinh tự động** và có gate CI; 31/32 khoảng trống văn bản đã đóng bằng kho vendor `licensing/upstream/` (39 file lấy từ đúng commit bất biến của bản phát hành; `npm run verify:upstream-licenses` đối chiếu lại byte). Một hàng trùng ở đây từng viết "`NOTICE` duy trì thủ công và chỉ phủ dependency trực tiếp"; câu đó sai từ `766609e`. **Văn bản còn thiếu: không còn (2026-11-19).** (a) `hexf-parse` 0.2.1 rời đồ thị ở R1c; dòng cuối, `utf8-chars` 3.0.7, đóng bằng cách chọn Apache-2.0 trong `MIT OR Apache-2.0` mà crate khai (PR #175, QĐ-C2), `UNRESOLVED.md` trống; câu hỏi pháp lý còn lại là trường `license` trơn của Cargo có phải lời cấp phép hay không, xem §1.c; (b) `neo4rs`/`neo4rs-macros` chỉ có tuyên bố license trong README và `zune-inflate` chỉ publish văn bản Zlib trong ba lựa chọn — cần pháp lý đọc, xem `licensing/README.md` |
| 3.17 | Inventory dependency (OSS-004) *(hai hàng 3.17 trùng nhau được gộp 2026-11-17)* | [`sbom.cdx.json`](../../sbom.cdx.json) · [`OPEN_SOURCE_LANDSCAPE.md`](../research/OPEN_SOURCE_LANDSCAPE.md) | **Inventory tự động cho dependency code đã có** (2026-07-27): SBOM CycloneDX với purl, biểu đồ phụ thuộc và SPDX, sinh từ lockfile đã ghim; số thành phần hiện hành ở §1. Lỗ hổng cũ (`burn`/`burn-wgpu` là runtime dep nhưng vắng khỏi ma trận khảo sát tới 2026-07-26) nay không thể tái diễn cho dep runtime. **Còn nợ:** ma trận khảo sát thủ công cho **model / dataset / asset**; SBOM không thay thế được nó |

---

## 4. Bẫy đã biết — đọc trước khi sửa

Không lặp lại nội dung CLAUDE.md; đây là những cái tốn nhiều giờ nhất và dễ tái phát nhất.

- **Worktree làm `npm run lint` đỏ toàn bộ, và CI không thấy.** §4 khuyên dùng `git worktree` — nhưng
  mỗi worktree là một bản sao đầy đủ **kèm `tsconfig.json` riêng**, và typescript-eslint thấy nhiều
  ứng viên `tsconfigRootDir` thì **từ chối parse mọi thứ**. Đo 2026-07-29 với 34 worktree:
  **4.656 lỗi**, không lỗi nào thuộc cây này. CI checkout sạch nên vĩnh viễn xanh — gate chỉ hỏng ở
  chỗ không ai nhìn. Đã sửa: `.worktrees/**` vào `ignores` của `eslint.config.js`, và `.worktrees/`
  vào `.gitignore` (trước đó chỉ nằm ở `.git/info/exclude`, tức **không sống qua một lần clone**).
- **Output `cargo test` bị wrap làm `check_test_targets.mjs` bỏ qua target trong im lặng.** Regex cũ
  đòi `Running <path> (` **trên cùng một dòng**; PowerShell wrap stderr của native command theo bề
  rộng console, nên dấu `(` rơi xuống dòng sau. Đo được: 75 dòng `Running`, **61 khớp**, script in
  "test targets: 61, empty: 0" và exit 0 — đúng chế độ hỏng-im-lặng mà nó tồn tại để chặn, xảy ra
  bên trong chính nó. Đã sửa: bỏ yêu cầu `(`, **và** thêm tự-kiểm đối chiếu số target parse được với
  số dòng `Running`. Tự-kiểm đó còn bắt được một target **crash lúc load** — đúng hình dạng
  `STATUS_ENTRYPOINT_NOT_FOUND` ở mục Git Bash bên dưới, thứ trước đây exit 0.
- **Một `#[test]` mỗi binary KHÔNG đủ cho gate zero-alloc.** Cách phòng ghi trong
  [`tests/common/allocator.rs`](../../src-tauri/tests/common/allocator.rs) chỉ loại được thread của
  *test anh em*. `observer_trace_zero_alloc_tests` **đã** là một `#[test]` duy nhất và vẫn fail
  ("made 2 heap allocations") trong lượt chạy đầy đủ 2026-07-29, pass 3/3 khi chạy riêng — vì
  **thread chính của libtest** sống suốt tiến trình và có cấp phát. Đã thêm
  `start_tracking_this_thread()` (chỉ đếm thread gọi) và chuyển suite đó sang dùng nó. Cổng combat
  (#80, 2026-11-16) và cổng emit (#86, 2026-11-17, flake trên CI của một PR chỉ sửa tài liệu) đã đi
  cùng đường đó, vì mọi hàm chúng đo đều chạy trên thread gọi.

  **Đừng chuyển bừa mọi suite sang nó.** `terrain_challenger_tests` **phải** giữ `start_tracking()`
  process-wide: `TerrainMap::generate` chạy pass noise trên rayon worker, nên bộ đếm theo thread sẽ
  **thôi đếm đúng thứ gate đó tồn tại để bắt** — và sẽ xanh trong lúc làm vậy.
  `map_generation_zero_alloc_tests` từng được kể ở đây, nhưng test duy nhất của nó đo một bản chép vòng
  xói mòn chạy trên thread gọi và không gọi `generate`; nó flake trên CI của một commit chỉ sửa tài liệu
  (2026-11-18, "4 heap allocations"), nên từ ngày đó nó chỉ đếm thread gọi. Một gate im đi tệ hơn một gate thỉnh thoảng ồn. Hai
  control âm trong file đó chốt cả hai chiều: bộ đếm vẫn thấy cấp phát thật, và nó **cố tình** mù
  với thread khác.

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
- **`challenger_meta_ai_tests` nay là flake ĐÃ QUAN SÁT ĐƯỢC, không còn latent** (2026-07-29). File
  có **3 `#[test]` trong một binary** và một trong số đó **spawn thread nền** — đúng tổ hợp mà
  [`tests/common/allocator.rs`](../../src-tauri/tests/common/allocator.rs) mô tả. Khi máy bận, hai
  test cùng đỏ:
  - `test_ecs_hot_path_zero_allocations_i22` → "got 4" (thread nền của test anh em cấp phát giữa
    cửa sổ đo);
  - `test_environmental_systems_non_blocking_performance` → 1,2587s so với trần 1,2s. Test này
    **cộng dồn `thread::sleep(16ms)`** rồi khẳng định tổng < 1200ms; hạt thời gian mặc định của
    Windows là ~15,6ms, nên biên an toàn mỏng theo thiết kế chứ không do hồi quy.

  Chạy riêng: **3/3 pass, ba lần liên tiếp.** Cách sửa đúng là gộp về một `#[test]` (nửa zero-alloc)
  và cho test thời gian một trần rộng hơn hoặc bỏ `sleep` khỏi phép đo. **Nửa đầu đã làm** (`c6e8f42`,
  2026-07-29): ba phần nay là một `#[test]` (`challenger_meta_ai_contracts`). Cổng zero-alloc ở đó **phải**
  giữ `start_tracking()` toàn tiến trình, vì `Schedule::default()` chạy hệ trên pool luồng. **Nửa sau
  chưa làm:** test thời gian vẫn cộng `sleep(16ms)` với trần 1.200 ms.
- **`environmental_elements_stress_tests` cũng đo bằng đồng hồ thật, và đã đỏ trên CI** (2026-11-18,
  PR #112, run `44195560520`): `environmental_elements_stress_contracts` khẳng định thời gian tick trung
  bình dưới 50 ms và đo được 101,9 ms trên runner `windows-latest`. PR đó chỉ sửa tài liệu; chạy lại job
  thì xanh, và cùng test xanh trên #107, #113 và #114 cùng ngày. Một khẳng định thời gian tuyệt đối trên
  runner dùng chung là flake theo thiết kế, giống nửa sau của `challenger_meta_ai_tests`. Chưa sửa: cách
  đúng là đo hiệu năng ở bench (Criterion) và để test chỉ khẳng định engine tick được, hoặc nới trần đủ xa
  để chỉ bắt một hồi quy cấp bậc độ lớn.
- **Hỏi máy đang chạy gì, bằng lệnh, trước khi gọi một lần đỏ là hồi quy.** Lúc hai test trên đỏ,
  máy có **2 `cargo` + 1 `rustc` + 6 `MSBuild` + 17 `node`** của phiên khác, và 3 worktree vừa được
  ghi trong 2 giờ. Suite `tests/` cùng lúc đó cho 6 lỗi — 3 lỗi là `Test timed out in 15000ms` thật,
  3 lỗi còn lại đội lốt khẳng định sai (`expected null not to be null`, `expected 0 to be greater
  than 0`). `Get-Process | Where-Object { $_.ProcessName -match 'cargo|rustc|node' }` là một câu hỏi
  rẻ hơn nhiều so với một cuộc điều tra hồi quy không có gì để tìm.
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
  `tests/` báo hàng loạt lỗi giả: timeout render bị đọc thành "không tìm thấy DOM node", nên lỗi
  trông như một khẳng định sai chứ không như một timeout. Chạy lại lúc máy rảnh: 243 pass, và thời
  gian tường rơi từ 50,5s xuống 19,5s. Trước khi kết luận đỏ là hồi quy, hãy hỏi máy lúc đó đang
  chạy gì.
  > **Sửa 2026-07-26:** bản trước ghi "~28 lỗi" như thể đó là dấu hiệu nhận dạng. **Không phải** —
  > số lỗi thay đổi theo tải; đo được 28 rồi 61 trong hai lần liên tiếp. Dấu hiệu thật là hình dạng
  > (mỗi file ~25–28s thay vì ~1s; tổng thời gian `tests` lớn hơn nhiều lần thời gian tường). Và khi
  > không thể chờ máy rảnh, **`npm run test:frontend -- --maxWorkers=2` chữa được** — đã kiểm: 243
  > pass trên đúng cái máy vừa cho 61 lỗi. Chi tiết ở §1.
  >
  > **Cập nhật 2026-07-27 (`72a6e34`):** config đã tự ghim số worker — 4 ở máy này, **2 khi biến
  > `CI` được đặt** — nên mặc định không còn là "một worker mỗi nhân". Cờ tay ở trên vẫn là cách
  > chữa khi máy đang bận; cái nó không chữa được là **một máy có ít nhân hơn máy đã đo**, và đó
  > đúng là cách runner `ubuntu-latest` (4 vCPU) làm hai file `render(<App />)` timeout. Xem §1.e.
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

### Dated traps added 2026-11-18 → 2026-11-25 (recorded `2026-11-25`)

- **`2026-11-18` — Windows CRLF checkout breaks SHA-256 manifest gates (`#125`, [§1.i](#1i-đo-2026-11-18-tại-46e25d5--lượt-đo-lại-đầy-đủ-sau-đợt-2-trên-windows-11-x64-thẻ-r0)).**
  With `core.autocrlf=true`, Git converts JSON manifest line endings to CRLF on checkout unless pinned
  to `eol=lf` in `.gitattributes`.
- **`2026-11-19` — `Swatinem/rust-cache` keyed on environment hash missed on every PR (`#157`).**
  PR builds computed a different Rust env hash than `main` and rebuilt cold (~95 min), while `determinism`
  filled the 10 GB quota. `.github/workflows/ci.yml` (`77–88`) now keys on `rust-toolchain.toml` and saves
  caches only from `main` and `feat/ecological-selection`.
- **`2026-11-19` — Headless experiment runs previously read desktop terrain artifacts or `map_config.json` (`F051` `#174`, `F051b` `#179`).**
  Headless runners now enforce `TerrainSource::Generated`, ignore `ANIMA_WORLD_ARTIFACT` and `map_config.json`,
  and record `WorldIdentity` in provenance.
- **`2026-11-20` — `check:bundle` filename-only 3D check missed direct `three.js` imports into `index.html` (`F-054`, `#190`; fixed `#197`).**
  Chunk names change under bundling (`defineProperty-*.js`); `scripts/check_bundle_budget.mjs` now detects
  `three.js` by content/module origin rather than filename prefix alone.
- **`2026-11-20` — `final_observables["live.mean_lifespan"]` read `0.0` when `duration_ticks % sample_period == 0` (`F-053`, `#183`; mitigated `#200`).**
  The final time-series sample drained `VitalStatistics::sample_window()` right before `final_observables`
  called `peek_window()`. Keep legacy windowed IDs untouched for registered fixtures and use additive
  cumulative IDs (`live.cumulative_mean_lifespan`).
- **`2026-11-20` — Parallel Cargo builds on 16 GB Windows 2 (Dell Vostro 3530) cause OOM shutdown (`QD-MACSHARE` in [`AGENT_WORKFLOW.md` §4](../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0), [`scripts/paper/gate_controls.mjs:114-122,170-222`](../../scripts/paper/gate_controls.mjs#L114-L122), [`gate_controls.md:108`](../paper/data/gate_controls.md), [`2026-11-23-nr-spike-protocol.md:129`](../ai/planning/2026-11-23-nr-spike-protocol.md)).**
  Unlike the 32 GiB Mac (`CARGO_BUILD_JOBS=4 nice -n 10`), Windows 2 must run Cargo only when zero
  `cargo`/`rustc` processes appear twice `60 s` apart, with `>= 5 GiB` free physical RAM, `>= 20 GiB`
  free drive space, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`,
  and `PRIORITY_LOW` (`Idle` priority).
- **`2026-11-22` — Writing unique per-birth IDs into `AgentLineageId` mutates trajectories (`F-066`, `#207`, [§3.15.1](#3151-việc-còn-lại--đọc-mục-này-trước)).**
  `autonomous_reproduction_system` iterates parents ordered by `AgentLineageId`, which changes `SimRng`
  draws. Store unique lineage IDs in `LiveLineage`'s side table instead to preserve ADR-0008 zero-feedback.
- **`2026-11-22` — `include_str!("../../src/...")` escaping `src-tauri/` breaks `cargo-mutants` (`F-061`, `#198`).**
  `cargo mutants` copies `src-tauri/` into a temporary sandbox where relative `../../` paths fail baseline
  compilation; keep test inputs crate-local.
- **`2026-11-23` — CI shallow clone (`fetch-depth: 1`) breaks git-history gates (`#211`, `.github/workflows/ci.yml:271–276`; see also `#186`).**
  `scripts/paper/incident_dates.mjs` resolves commit dates across git history, so the `Frontend` CI job
  must keep `fetch-depth: 0` (and artifact verifiers handle shallow clones explicitly).
- **`2026-11-24` — Profile-scoped `#[ignore]` exemptions and `run_e3_experiment.rs` reuse pitfalls (`#199`, [`scripts/test_target_policy.mjs:100-150`](../../scripts/test_target_policy.mjs#L100-L150); `#234`, [`2026-11-24-e3b-runner.md`](../ai/planning/2026-11-24-e3b-runner.md#1-why-run_e3_experimentrs-cannot-run-e3b-read-only-finding)).**
  Unmet `MAP-D3` Card 3 tests in `anima-worldgen` must stay scoped to `profiles: ['worldgen-cpu']` alongside un-ignored split tests rather than leaking into the `--profile desktop` allow-list; separately, `run_e3_experiment.rs` hardcodes `"e3-preregistration.json"`, omits `live.mean_lifespan` from `SERIES_OBSERVABLES`, and emits `"completed"` / `"final_observables"` schemas incompatible with `verify_e3b_artifacts.mjs` (`#234` added dedicated `run_e3b_experiment.rs`).
- **`2026-11-24` — Cross-OS non-vacuity assertions cannot pin late-run `tick 18,000` survival or learned-weight counts (`#236`, [`src-tauri/tests/world_record_no_feedback_tests.rs:238-273`](../../src-tauri/tests/world_record_no_feedback_tests.rs#L238-L273)).**
  Even when `recording ON == recording OFF` parity (`world_checksum`, `learned`, `populations`) holds bit-for-bit across `[0, 180, 1_800, 18_000]`, `libm` trajectory differences across Windows vs macOS can cause tick-`18,000` learned-weight assertions on seed `700_002` to pass on Windows and fail on macOS; keep `ON == OFF` comparisons strict through the full run while gating non-vacuity on early/intermediate/peak telemetry (`ticks 180, 1_800`, `max_learned > 0`, `last_alive_tick >= 1_800`).
- **`2026-11-25` — Open `physics_tests` allocation flake (`P11`, `anima-case`; CI run [45516447719](https://github.com/DuongNAD/Anima-Engine/actions/runs/45516447719)).**
  In attempt 1 of `determinism` (`job/138876997447`), `test_spatial_grid_zero_allocation` (`src-tauri/tests/physics_tests.rs:532`) panicked with `Raycast should perform 0 heap allocations, but made 1`, poisoning the mutex for `test_static_equilibrium`, `test_zero_allocation_hot_path`, and `test_zero_joint_axis_no_nan` (`PoisonError`) before passing on attempt 2. Tracked as open Card `P11`.

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
