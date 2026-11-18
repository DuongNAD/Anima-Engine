---
phase: testing
feature: anima-completion
title: Đề xuất mục §1.i cho STATE_OF_THE_PROJECT.md — đo ngày 2026-11-18 tại 46e25d5
description: Bản thảo mục §1.i theo đúng văn phong §1.h, tổng hợp từ lượt đo cục bộ thẻ R0 trên Windows 11 x64 tại 46e25d5
status: proposed
owner: maintainers
last_reviewed: 2026-11-18
---

# Bản đề xuất chèn vào `docs/planning/STATE_OF_THE_PROJECT.md` (§1.i)

> Bản thảo dưới đây được soạn cho thẻ `R0` (không sửa trực tiếp `STATE_OF_THE_PROJECT.md` do thẻ không có dòng `AUTHORIZATION`). Mọi con số chỉ lấy từ lượt chạy cục bộ ngày `2026-11-18` tại `46e25d5` (báo cáo đầy đủ: [remeasure-2026-11-18-46e25d5.md](remeasure-2026-11-18-46e25d5.md)).

---

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
