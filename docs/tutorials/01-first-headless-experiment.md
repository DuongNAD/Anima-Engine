---
title: Bài 01 — Từ clone tới thí nghiệm headless đầu tiên
status: active
owner: maintainers
last_reviewed: 2026-11-22
review_cycle: per-release
---

# Bài 01 — Từ clone tới thí nghiệm headless đầu tiên

Bài hướng dẫn này đưa bạn đi từ một bản clone sạch của repository đến lúc tự chạy xong lượt thử **E2 smoke** ở chế độ headless, kiểm chứng toàn bộ digest độc lập bằng `npm run check:e2`, và đọc đúng ranh giới suy luận của kết quả.

> Trạng thái đo hiện tại của toàn dự án (số test, clippy, E2E, xác minh gần nhất) được giữ tập trung tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền) (lượt đo mới nhất tại [§1.i](../planning/STATE_OF_THE_PROJECT.md#1i-đo-2026-11-18-tại-46e25d5--lượt-đo-lại-đầy-đủ-sau-đợt-2-trên-windows-11-x64-thẻ-r0)). Tài liệu này không sao chép các con số tổng của dự án để tránh lệch theo thời gian.

---

## Điều kiện tiên quyết và lưu ý đa nền tảng

Trước khi bắt đầu, chuẩn bị:

- **Git**, **Node.js `22`** (kèm `npm`), và **Rust qua `rustup`** (file [`rust-toolchain.toml`](../../rust-toolchain.toml) tự ghim channel `1.98.1` cùng `rustfmt` và `clippy`).
- **Trình biên dịch hệ thống:**
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`).
  - **Windows:** MSVC Build Tools (xem bảng quy đổi PowerShell tại [How-to §Trên macOS và Linux](../how-to/README.md#trên-macos-và-linux)).
  - **Linux:** vì `tauri` là dependency trực tiếp của crate `anima-engine`, ngay cả build headless vẫn cần các thư viện dev hệ thống (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libsoup-3.0-dev`).

Các khối lệnh bên dưới dùng cú pháp POSIX (`bash`/`zsh`) chạy từ **thư mục gốc của repository**; ghi chú cho Windows PowerShell được tách riêng ngoài khối tự động kiểm thử.

---

## Bước 1 — Clone repository

Clone mã nguồn và đi vào thư mục gốc của dự án (nhánh phát triển hiện tại là `feat/ecological-selection`, xem [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```bash
git clone https://github.com/DuongNAD/Anima-Engine.git
cd Anima-Engine
git checkout feat/ecological-selection
```

---

## Bước 2 — Cài dependency JavaScript

Cài các gói Node phục vụ script kiểm chứng và frontend từ `package-lock.json` bằng `npm ci` (nếu làm việc trên môi trường cho phép cập nhật lockfile mới có thể dùng `npm install`, nhưng `npm ci` giữ cây Git sạch):

<!-- doc-test -->
```bash
npm ci
```

Kết quả quan sát được (rút gọn):

```text
added 290 packages, and audited 291 packages in 2s
...
```

---

## Bước 3 — Chạy một target kiểm thử backend nhanh

Trước khi chờ biên dịch bản `--release`, hãy chạy một target Rust nhỏ và không step tick là `e2_seam_tests`. Target này dùng `default` features (bỏ qua `wgpu`, `neo4rs`, `tokio-tungstenite`) để kiểm tra nhanh giao diện nạp manifest E2, cách ly `AgentBrain`, đồng nhất luồng RNG sinh thái (`SimRng`), đồng nhất `WorldIdentity` và lệnh cấm `cargo run`:

<!-- doc-test -->
```bash
cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests
```

Kết quả quan sát được trên macOS arm64 (rút gọn phần biên dịch crate):

```text
   Compiling anima-engine v0.1.0 (.../src-tauri)
...
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 13s
     Running tests/e2_seam_tests.rs (src-tauri/target/debug/deps/e2_seam_tests-...)

running 9 tests
test the_live_world_honours_exactly_one_new_initial_condition_key ... ok
test absent_means_false_so_every_existing_live_manifest_keeps_its_behaviour ... ok
test one_means_true_and_nothing_else_is_accepted ... ok
test the_preregistered_reproduction_never_launches_the_app ... ok
test the_treatment_builds_founders_with_brains_and_the_control_builds_none ... ok
test the_founding_population_differs_only_by_the_brain ... ok
test both_arms_observe_the_same_world_identity ... ok
test the_committed_e2_manifests_build_the_arms_they_declare ... ok
test founder_brains_leave_the_ecology_stream_exactly_where_the_control_left_it ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
```

> Khi cần chạy toàn bộ bộ kiểm thử đầy đủ thay vì một target làm quen, tham khảo [`STATE_OF_THE_PROJECT.md` §5](../planning/STATE_OF_THE_PROJECT.md#5-lệnh-xác-minh-đầy-đủ).

---

## Bước 4 — Build experiment runner ở chế độ release

Binary thí nghiệm E2 là example `run_e2_brain_experiment`. Biên dịch ở chế độ `--release --features desktop`:

<!-- doc-test -->
```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_e2_brain_experiment
```

Kết quả quan sát được (rút gọn):

```text
   Compiling anima-engine v0.1.0 (.../src-tauri)
...
    Finished `release` profile [optimized] target(s) in 7m 43s
```

**Hai điều tuyệt đối KHÔNG làm ở bước này:**

1. **Không dùng `cargo run`:** hợp đồng E2 ([`e2-preregistration.json`](../../src-tauri/tests/fixtures/experiments_e2/e2-preregistration.json)), `e2_seam_tests` và `scripts/verify_e2_artifacts.mjs` đều cấm `cargo run`. Bạn phải build trước rồi gọi trực tiếp file nhị phân đã biên dịch để `provenance.json` ghim đúng `"profile": "release"` và `binary_sha256` của file thực thi.
2. **Không dùng `npm run tauri:dev` hay mở app desktop:** `run_e2_brain_experiment` chạy thuần headless trong một tiến trình duy nhất thông qua `LiveExperimentAdapter` trên `simulation_schedule::build_tick_schedule` — không tạo cửa sổ, không Tauri handle, không GPU device, không renderer và không mở WebSocket.

---

## Bước 5 — Chạy E2 smoke manifest

Gọi trực tiếp binary vừa build với cờ `--smoke`, trỏ vào bộ manifest đã đóng băng tại [`src-tauri/tests/fixtures/experiments_e2/`](../../src-tauri/tests/fixtures/experiments_e2) và xuất kết quả ra thư mục gitignored `src-tauri/target/tutorial-e2-smoke`:

<!-- doc-test -->
```bash
./src-tauri/target/release/examples/run_e2_brain_experiment --smoke \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/tutorial-e2-smoke
```

> **Trên Windows PowerShell**, đường dẫn file thực thi có đuôi `.exe`:
>
> ```powershell
> .\src-tauri\target\release\examples\run_e2_brain_experiment.exe --smoke `
>   --manifest-dir src-tauri\tests\fixtures\experiments_e2 `
>   --out src-tauri\target\tutorial-e2-smoke
> ```

Kết quả quan sát được:

```text
E2 Smoke: 1 seeds x 2 arms x 18000 ticks, sample period 600
  integrity: control brains 0 / treatment brains 10 of 10 founders; ecology stream identical: true; world identity identical: true
  finished in 4.4s: 1 complete pairs of 1
  artifacts written to src-tauri/target/tutorial-e2-smoke/smoke
```

**Vì sao xuất vào `src-tauri/target/tutorial-e2-smoke` thay vì `artifacts/experiments/e2-evolved-brain-default`?**

Thư mục `artifacts/experiments/e2-evolved-brain-default/` đã được commit làm bằng chứng tham chiếu (xem [`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md)). Nếu truyền `--out artifacts/experiments/e2-evolved-brain-default`, lệnh `--smoke` sẽ ghi đè `smoke/provenance.json`, `smoke/summary.md` và `smoke/checksums.sha256` bằng timestamp và đường dẫn máy của bạn, làm bẩn `git status`. Thư mục `src-tauri/target/` đã nằm trong `.gitignore` nên giữ repository sạch sẽ.

---

## Bước 6 — Kiểm chứng digest bằng `npm run check:e2`

Chạy trình kiểm tra Node độc lập (`scripts/verify_e2_artifacts.mjs`) cho cả thư mục smoke bạn vừa sinh ra lẫn bộ bằng chứng đã commit trong repo:

<!-- doc-test -->
```bash
npm run check:e2 -- src-tauri/target/tutorial-e2-smoke
npm run check:e2
```

Kết quả quan sát được (rút gọn đường dẫn máy cục bộ bằng `…`):

```text
> anima-engine-frontend@0.1.0 check:e2
> node scripts/verify_e2_artifacts.mjs src-tauri/target/tutorial-e2-smoke

note  analysis: no ensemble has been run yet (no paired-report.json at the artifact root)
note  smoke: recomputed 27 per-seed deltas
note  analysis: no execution-lock.json — the duration rung was not locked from a smoke calibration

OK — every E2 artifact in …/src-tauri/target/tutorial-e2-smoke verifies against node:crypto and the preregistration.

> anima-engine-frontend@0.1.0 check:e2
> node scripts/verify_e2_artifacts.mjs

note  analysis: recomputed 264 per-seed deltas
note  smoke: recomputed 22 per-seed deltas

OK — every E2 artifact in …/artifacts/experiments/e2-evolved-brain-default verifies against node:crypto and the preregistration.
```

Lưu ý hai thông báo hợp lệ ở lượt chạy smoke mới:

- `note  analysis: no ensemble has been run yet` và `no execution-lock.json` xuất hiện vì ở đây ta chỉ mới chạy `--smoke`, chưa chạy `--ensemble` 12 seed.
- Script dùng `node:crypto` tính lại SHA-256 độc lập với `core::sha256` của Rust, kiểm tra không có file nào bị sót ngoài `checksums.sha256`, đối chiếu từng byte của `manifests/*` với `HEAD:src-tauri/tests/fixtures/experiments_e2/*`, kiểm tra các cổng tiền đăng ký (`E2-G1`, `E2-G3`, `E2-G4`, `E2-G6`, `profile === "release"`, `seed_order === [999983]`), và tính lại phép trừ `treatment_final - control_final === delta` cho toàn bộ 27 dòng trong `per-seed-deltas.csv`. *(Lượt chạy gốc trong repo ở 2026-11-16 ghi nhận 22 observable; trên thân cây hiện tại bộ registry đã có 27 observable nên lượt smoke mới ghi `27 per-seed deltas`.)*

---

## Bước 7 — Đọc `smoke/summary.md` và danh sách file đầu ra

In ra báo cáo tóm tắt Markdown mà runner vừa sinh:

<!-- doc-test -->
```bash
cat src-tauri/target/tutorial-e2-smoke/smoke/summary.md
```

Kết quả quan sát được (rút gọn phần giữa bảng 27 observable):

```text
# E2 — per-agent evolved brain vs shared BrainModel

> ⛔ **Calibration only, seed 999983, EXCLUDED from every analysis, table, statistic and claim.** These numbers exist to time the harness and prove it runs. They are not evidence about brains and may not be quoted as any part of the E2 result.

Model: `core::live_experiment::LiveExperimentAdapter` — **headless adapter verified**, which is the only status claim permitted. Harness: `core::experiment_runner::run_paired_ensemble_with_control`.

- seeds run: `[999983]`
- duration: **18000 ticks** (committed 18000, ladder [18000, 12000, 6000]), sample period 600
- complete pairs: **1 of 1** (decision needs 10)
- wall clock: **4.4 s** (0.07 min) against a registered ceiling of 90 min
- declared factors: `["initial_conditions"]`

## Integrity (gates E2-G1, E2-G3, E2-G6)

| check | result |
|---|---|
| control founders carrying `AgentBrain` | 0 of 10 |
| treatment founders carrying `AgentBrain` | 10 of 10 |
| ecology stream identical after genesis | yes |
| world identity identical across arms | yes |

## Every observable the run produced
...
| `live.closed_eu_total` | harness integrity | 1 | 155971.799862 | 155971.799862 | 1.106e-9 | 1.106e-9 | n/a | n/a | n/a | n/a | +0.00% | no |
...
| `live.mean_agent_energy` | primary | 1 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | n/a | no |
| `live.mean_energy_integral` | reported | 1 | 281958.324842 | 292190.542737 | 10232.217895 | 10232.217895 | n/a | n/a | n/a | n/a | +3.63% | no |
...
## The primary metric, against the three registered conditions

`live.mean_agent_energy`, paired per seed, hypothesised **negative** (H1).

| condition | required | measured | met |
|---|---|---|:--:|
| 95% CI excludes 0 | CI must not contain 0 | undefined | no |
| \|d_z\| ≥ 0.8 | large by convention | n/a | no |
| \|delta\| ≥ 5% of \|control mean\| | relative shift | undefined | yes |

**Material: no.**
...
## What this cannot establish
...
```

> **Lưu ý:** Thư mục `smoke/` **không có file `summary.json`**. Bản tóm tắt cho người đọc là `summary.md`, còn dữ liệu máy đọc nằm ở các file JSON và CSV bên dưới.

---

## Giải thích ý nghĩa từng file đầu ra

Trong `src-tauri/target/tutorial-e2-smoke/smoke/` có đúng 10 file:

| File | Ý nghĩa | Có trùng bit-for-bit giữa 2 máy/2 lượt chạy? |
|---|---|---|
| `summary.md` | Tóm tắt Markdown gồm cảnh báo loại trừ seed `999983`, bảng kiểm toàn vẹn, bảng 27 observable, 3 điều kiện materiality cho chỉ số chính `live.mean_agent_energy` và mục *What this cannot establish*. | Khác (chứa `wall clock: X.X s`). |
| `effects.json` | Chỉ số tổng hợp từng observable (`kind: "e2-smoke-effects-EXCLUDED-FROM-ANALYSIS"`), quy tắc materiality (`95% CI`, `|d_z| >= 0.8`, `|delta| >= 5%`), `n_complete_pairs: 1`, `min_complete_pairs_for_a_decision: 10`. | **Trùng** trên cùng commit và cùng OS/kiến trúc. |
| `paired-report.json` | Cấu trúc `PairedEnsembleReport` đầy đủ gồm fingerprint của manifest, world law, registry, các `RunResult` theo cặp và `build_id`. | **Trùng** trên cùng commit và cùng OS/kiến trúc. |
| `per-seed-deltas.csv` | Bảng 27 dòng (kèm 1 dòng tiêu đề): `seed,observable,control_final,treatment_final,delta` cho phép kiểm toán số học từng cặp seed. | **Trùng** trên cùng commit và cùng OS/kiến trúc. |
| `runs.csv` | Trạng thái và giá trị cuối của từng arm (`999983,control,completed` và `999983,treatment,completed`) kèm `final_checksum` (`1892961356` và `1257130731` trên macOS arm64 tại commit này). | **Trùng** trên cùng commit và cùng OS/kiến trúc. |
| `provenance.json` | Metadata môi trường (`build.*`, `host.*`, `timing.*`), kiểm chứng cổng `E2-G1`/`E2-G3`/`E2-G6`, `model_actually_run` (`live-bevy-world/2`), `model_status` và danh sách `what_this_run_does_not_claim`. | Khác (chứa `binary_path`, `binary_sha256`, `started_utc`/`ended_utc`, `ensemble_wall_clock_seconds`). |
| `manifests/*.json` (3 file) | Bản sao nguyên văn `e2-preregistration.json`, `e2-smoke-control-shared-brain.json` và `e2-smoke-treatment-evolved-brain.json` dùng trong lượt chạy. | **Trùng** (bắt buộc khớp byte với `HEAD`). |
| `checksums.sha256` | Danh sách SHA-256 của 9 file trên, là đầu vào cho `npm run check:e2`. | Chỉ thay đổi ở 2 dòng `provenance.json` và `summary.md` giữa 2 lượt chạy cùng commit/máy. |

Hai chi tiết dễ gây hiểu nhầm khi mới đọc số liệu smoke:

- **Vì sao `live.mean_agent_energy` bằng `0.000000` ở cả hai arm và cột `met` cho điều kiện `|delta| >= 5%` lại hiện `yes`?** Trong `LiveExperimentAdapter`, các agent tiêu hết năng lượng về sàn `0.0` trước tick `18000` (vì không có cái chết do đói và không có thay thế tiến hóa; xem [`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md)). Khi cả `control_mean` lẫn `delta` đều bằng `0.0`, phép so sánh `0.0 >= 0.05 * 0.0` trả `true` một cách hình thức dù `relative_shift` là `undefined`. Tuy nhiên kết luận vẫn là **`Material: no`** vì hai điều kiện còn lại (`95% CI` và `|d_z|`) đều `no`.
- **Ý nghĩa của `live.closed_eu_total`:** cả hai arm giữ tổng năng lượng đóng xấp xỉ `155971.799862` EU với sai lệch `1.106e-9` EU (nhiễu cộng dấu phẩy động, nằm rất xa bên trong ngưỡng bảo toàn closed-EU theo [Hợp đồng sổ cái năng lượng](../reference/ENERGY_LEDGER_CONTRACT.md#6-cổng-g11)).

---

## Ranh giới suy luận: kết quả smoke chứng minh gì và KHÔNG chứng minh gì?

### Điều lượt chạy smoke CHỨNG MINH được

1. **Harness headless biên dịch và chạy thông suốt:** `LiveExperimentAdapter` chạy trọn vẹn 18.000 tick cho cả hai arm `control` và `treatment` ở seed `999983` trong cấu hình `--release` không panic, không warning.
2. **Cách ly nhân quả và bảo toàn năng lượng giữ vững:**
   - **E2-G1:** `0/10` cá thể sáng lập ở `control` mang `AgentBrain`; `10/10` cá thể ở `treatment` mang `AgentBrain`.
   - **E2-G3:** luồng `SimRng` sinh thái khớp từng bước ngay sau genesis.
   - **E2-G4:** hai arm chỉ khác nhau ở khóa `initial_conditions` (`live.evolved_brains`).
   - **E2-G6:** hai arm dựng cùng `WorldIdentity` (`seed = 1337`, `256 × 256`, `checksum = 1212187560`).
   - **Closed-EU:** `live.closed_eu_total` bảo toàn ở mức `~1e-9` EU.
3. **Ước lượng ngân sách thời gian:** 1 cặp hết ~`4.4 s` trên Apple Silicon (dùng để khóa duration rung dưới trần 90 phút trước khi chạy `--ensemble`).
4. **Tính tái lập và toàn vẹn artifact:** mọi file trong `smoke/` khớp `checksums.sha256` qua `node:crypto`.

### Điều lượt chạy smoke KHÔNG chứng minh

1. **Không phải kết quả khoa học về não tiến hóa:** seed `999983` được đăng ký riêng cho hiệu chuẩn và **bị loại trừ khỏi mọi phân tích** (`kind: "e2-smoke-effects-EXCLUDED-FROM-ANALYSIS"`). Cỡ mẫu `n = 1` cặp (yêu cầu tối thiểu `10` cặp để kết luận), nên `SD`, `SE`, `95% CI` và `d_z` đều vô nghĩa (`n/a` / `null`).
2. **Không chứng minh chọn lọc hay thích nghi tiến hóa:** như `provenance.json` và `summary.md` ghi rõ, `LiveExperimentAdapter` chỉ ghi nhận các cá thể sáng lập và **không thực hiện vòng thay thế tiến hóa** qua nhiều thế hệ.
3. **Không khái quát hóa qua nhiều bản đồ:** toàn bộ lượt chạy chỉ dùng duy nhất một thiết kế thế giới (`seed = 1337`, `256 × 256`).
4. **Không chứng minh ứng dụng desktop hay GPU sẵn sàng:** trạng thái dừng đúng ở **`headless adapter verified`**, không đồng nghĩa với *experiment-ready* cho toàn sản phẩm hay kiểm chứng Tauri/WGPU/WebView.
5. **Không khẳng định bit-identical giữa các hệ điều hành khác nhau:** theo [Hợp đồng tất định](../reference/DETERMINISM_CONTRACT.md#1-lời-hứa-và-giới-hạn-của-nó), lời hứa tất định áp dụng cho *cùng manifest + cùng build trên cùng nền tảng*; `final_checksum` có thể khác giữa Windows và macOS do tối ưu dấu phẩy động khác target.

---

## Đọc tiếp các hợp đồng chuẩn

- [Chỉ mục `docs/reference/`](../reference/README.md) — điểm tra cứu mọi hợp đồng đang có hiệu lực.
- [Hợp đồng chế độ tiến hóa và thí nghiệm thế giới](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)* — [thuật ngữ chuẩn](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md#thuật-ngữ-chuẩn), [sự thật code hiện tại](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md#sự-thật-code-hiện-tại) và [cổng chấp nhận](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md#cổng-chấp-nhận).
- [Hợp đồng tất định](../reference/DETERMINISM_CONTRACT.md#1-lời-hứa-và-giới-hạn-của-nó) — phạm vi tái lập và nguồn rò rỉ.
- [Hợp đồng sổ cái năng lượng](../reference/ENERGY_LEDGER_CONTRACT.md#6-cổng-g11) — quy tắc closed-EU và cổng G1.1.
- [Lộ trình mô phỏng thế giới §3.2](../../WORLD_SIMULATION_PLAN.md#32-world-laws-và-experiment-manifest) — cấu trúc `ExperimentManifest` và `WorldLawSet`.
- [ADR-0003 — Não di truyền theo cá thể](../decisions/ADR-0003-evolved-per-agent-brains.md) — bối cảnh kiến trúc đằng sau thí nghiệm E2.
- [Ghi nhận kết quả E2 (2026-11-16)](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md) và [hồ sơ kiểm thử E2](../ai/testing/2026-07-27-experiment-e2-evolved-brain-default.md).
