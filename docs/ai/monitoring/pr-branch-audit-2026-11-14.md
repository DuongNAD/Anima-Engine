---
phase: monitoring
title: Kiểm toán PR mở và nhánh remote — đo ngày 2026-11-14
description: Phân loại toàn bộ 10 PR mở, 102 nhánh remote và liệt kê đủ mọi hunk chưa lên thân cây của PR #35-#37
status: active
owner: maintainers
last_reviewed: 2026-11-14
---

# Kiểm toán PR mở và nhánh remote (`2026-11-14`)

> **Phạm vi và quy tắc:** Tài liệu thực hiện thẻ **D0** (chỉ đọc, không sửa mã nguồn, không đóng/bình luận/sửa bất kỳ PR hay nhánh nào trên GitHub). Mọi số liệu dưới đây được đo trực tiếp ngày `2026-11-14` từ `origin` đã fetch và API GitHub.

## 1. Mốc đo nền và tóm tắt kết quả

- **Thân cây (`TRUNK`):** `origin/feat/ecological-selection` @ `491ec61e149d3661b5c1790ce9335d6e25d4cd2d` (2026-11-14)
- **Nhánh `main`:** `origin/main` @ `a43872c5e1db0b296d1010795d2d568d9340713b` (tương quan thân cây: `109` commit phía sau thân cây, `33` commit phía trước thân cây theo `git rev-list --left-right --count origin/feat/ecological-selection...origin/main` -> `109	33`)
- **Tổng PR đang mở (`gh pr list --repo DuongNAD/Anima-Engine --state open`):** **10** PR (`#30`, `#31`, `#34`, `#35`, `#36`, `#37`, `#38`, `#39`, `#40`, `#41`)
- **Tổng nhánh remote (`git ls-remote --heads origin | wc -l`):** **102** nhánh (tăng so với con số "~60 nhánh" ghi ngày 2026-11-05 trong thẻ D0), tách làm:
  - **1 nhánh `TRUNK`:** chính `feat/ecological-selection` (`491ec61e14`, mốc đối chiếu chuẩn).
  - **81 nhánh `SUPERSEDED`** là tổ tiên nghiêm ngặt của `origin/feat/ecological-selection` (`ahead = 0`, `git merge-base --is-ancestor` thoát `0`).
  - **20 nhánh** có `ahead > 0` hoặc chưa phải tổ tiên của thân cây: **13** nhánh `REBASE` (gồm 9 nhánh đầu PR `#30, #31, #35–#41`, `main`, `feature-runtime-unblock`, `claude/busy-hamilton-mn42mw`, `claude/funny-thompson-jj42fe`), **1** nhánh `STALE-DOC` (`docs/ht0-7-remeasure-main` / PR `#34`), và **6** nhánh `SUPERSEDED` (`feature-anima-menagerie`, `fix/main-ci-green`, `codex/open-source-release`, `codex/simulation-bottlenecks`, `feat/oss-071b-live-tracker`, `wip/2026-08-16-map2-tier5`).
- **5 nhánh `SUPERSEDED` trong nhóm `ahead > 0` ở trên** (`fix/main-ci-green`, `codex/open-source-release`, `codex/simulation-bottlenecks`, `feat/oss-071b-live-tracker`, `wip/2026-08-16-map2-tier5`) được xếp `SUPERSEDED` vì bản thân nhánh đó đã bị thay thế bởi hậu duệ chính thức (`origin/main`, PR `#30`, PR `#35–#37`) và **không mang công việc độc lập riêng để rebase**; phần mã/sửa lỗi còn sống của các hậu duệ đó đã được theo dõi đầy đủ tại các mục `REBASE` tương ứng (`main`/`#38–#40`, `#30`, `#35–#37`).

### Kết quả lệnh lấy mẫu nguyên văn (`2026-11-14`)

```bash
$ git ls-remote --heads origin | wc -l
102

$ git rev-list --left-right --count origin/feat/ecological-selection...origin/main
109	33

$ git rev-list --left-right --count origin/feat/ecological-selection...origin/fix/ht0-5c-sim-semantics
109	19

$ gh pr list --repo DuongNAD/Anima-Engine --state open --limit 50 --json number,headRefName,baseRefName,headRefOid,updatedAt
[{"baseRefName":"fix/ci-gates-green","headRefName":"fix/birth-death-zero-alloc","headRefOid":"2c5423b4df29e8443246287d7ba243ed7f17063f","number":41,"updatedAt":"2026-10-31T19:21:26Z"},{"baseRefName":"fix/declare-simrng-draw-order","headRefName":"fix/ci-gates-green","headRefOid":"78b2e85872dc429a9da9da9ee68c4c508908ea75","number":40,"updatedAt":"2026-10-31T15:29:03Z"},{"baseRefName":"fix/live-world-restores-its-spatial-grid","headRefName":"fix/declare-simrng-draw-order","headRefOid":"03b331f30de37312642d3a2c22eb6c3d1a2b0cd7","number":39,"updatedAt":"2026-10-31T13:08:24Z"},{"baseRefName":"main","headRefName":"fix/live-world-restores-its-spatial-grid","headRefOid":"fa938c5428077efdd26115016a8b3527de37d7ce","number":38,"updatedAt":"2026-10-31T09:10:33Z"},{"baseRefName":"main","headRefName":"fix/ht0-5c-sim-semantics","headRefOid":"c7829689ecff3ef19612caa8e02f0b75f6d10cf3","number":37,"updatedAt":"2026-10-17T13:08:38Z"},{"baseRefName":"main","headRefName":"fix/ht0-5b-landscape-dispose","headRefOid":"a5b7960d6eb6f422608017a67754c3ba57a7ddfd","number":36,"updatedAt":"2026-10-17T13:08:32Z"},{"baseRefName":"main","headRefName":"fix/ht0-5a-sensor-raycast","headRefOid":"3560faa6c06ad96dea3657338364f42637010511","number":35,"updatedAt":"2026-10-17T13:08:27Z"},{"baseRefName":"main","headRefName":"docs/ht0-7-remeasure-main","headRefOid":"f0a33cb4649a417fa82ccc75423ec8edec115d0d","number":34,"updatedAt":"2026-10-17T10:29:05Z"},{"baseRefName":"main","headRefName":"feat/oss-072-mrca","headRefOid":"b14e836c42b7d7711058decaa055048ed2a4baf4","number":31,"updatedAt":"2026-07-29T06:56:43Z"},{"baseRefName":"main","headRefName":"fix/document-the-wgpu-registry-leak","headRefOid":"0f33998fc99af56d81f96c058079fad659c92ad2","number":30,"updatedAt":"2026-07-28T11:41:58Z"}]
```

---

## 2. Phân loại toàn bộ 10 PR đang mở

| PR | Nhánh đầu (`headRefName`) | Đích (`base`) | Head SHA (10 ký tự) | Commit cuối | vs `origin/main` (`-b/+a`) | vs `trunk` (`-b/+a`) | `git cherry` vs `trunk` | Kết luận CI (`statusCheckRollup`) | Phân loại |
|---|---|---|---|---|---|---|---|---|---|
| `#30` | `fix/document-the-wgpu-registry-leak` | `main` | `0f33998fc9` | `2026-07-28 18:41:55 +0700` | `-163/+6` | `-239/+6` | `+6/-0` | Rust (src-tauri): `SUCCESS`, Frontend: `SUCCESS`, GitGuardian Security Checks: `SUCCESS` | `REBASE` |
| `#31` | `feat/oss-072-mrca` | `main` | `b14e836c42` | `2026-07-29 13:56:03 +0700` | `-163/+4` | `-239/+4` | `+4/-0` | Rust (src-tauri): `SUCCESS`, Frontend: `SUCCESS`, GitGuardian Security Checks: `SUCCESS` | `REBASE` |
| `#34` | `docs/ht0-7-remeasure-main` | `main` | `f0a33cb464` | `2026-10-17 17:21:27 +0700` | `-20/+2` | `-109/+15` | `+14/-0` | Rust (src-tauri): `SUCCESS`, Frontend: `SUCCESS`, GitGuardian Security Checks: `SUCCESS` | `STALE-DOC` |
| `#35` | `fix/ht0-5a-sensor-raycast` | `main` | `3560faa6c0` | `2026-10-17 17:40:25 +0700` | `-20/+2` | `-109/+15` | `+14/-0` | Rust (src-tauri): `SUCCESS`, Frontend: `SUCCESS`, GitGuardian Security Checks: `SUCCESS` | `REBASE` |
| `#36` | `fix/ht0-5b-landscape-dispose` | `main` | `a5b7960d6e` | `2026-10-17 17:43:34 +0700` | `-20/+2` | `-109/+15` | `+14/-0` | Rust (src-tauri): `SUCCESS`, Frontend: `SUCCESS`, GitGuardian Security Checks: `SUCCESS` | `REBASE` |
| `#37` | `fix/ht0-5c-sim-semantics` | `main` | `c7829689ec` | `2026-10-17 17:56:39 +0700` | `-20/+6` | `-109/+19` | `+18/-0` | Rust (src-tauri): `SUCCESS`, Frontend: `SUCCESS`, GitGuardian Security Checks: `SUCCESS` | `REBASE` |
| `#38` | `fix/live-world-restores-its-spatial-grid` | `main` | `fa938c5428` | `2026-10-31 16:10:07 +0700` | `-0/+1` | `-109/+34` | `+33/-0` | Rust (src-tauri): `FAILURE`, Frontend: `FAILURE`, GitGuardian Security Checks: `SUCCESS` | `REBASE` |
| `#39` | `fix/declare-simrng-draw-order` | `fix/live-world-restores-its-spatial-grid` | `03b331f30d` | `2026-10-31 19:56:06 +0700` | `-0/+2` | `-109/+35` | `+34/-0` | Rust (src-tauri): `FAILURE`, Frontend: `FAILURE`, GitGuardian Security Checks: `SUCCESS` | `REBASE` |
| `#40` | `fix/ci-gates-green` | `fix/declare-simrng-draw-order` | `78b2e85872` | `2026-10-31 22:28:59 +0700` | `-0/+3` | `-109/+36` | `+35/-0` | Rust (src-tauri): `FAILURE`, Frontend: `FAILURE`, GitGuardian Security Checks: `SUCCESS` | `REBASE` |
| `#41` | `fix/birth-death-zero-alloc` | `fix/ci-gates-green` | `2c5423b4df` | `2026-11-01 02:20:34 +0700` | `-0/+5` | `-109/+38` | `+37/-0` | Rust (src-tauri): `FAILURE`, Frontend: `FAILURE`, GitGuardian Security Checks: `SUCCESS` | `REBASE` |

### Bằng chứng lệnh và chi tiết từng PR

#### PR `#30` — `fix/document-the-wgpu-registry-leak` (`REBASE`)
- **Tiêu đề:** fix(perf): default the learner to CPU — the wgpu path leaks unboundedly
- **SHA đầy đủ:** `0f33998fc99af56d81f96c058079fad659c92ad2` · **Base:** `main` · **Cập nhật GitHub:** `2026-07-28T11:41:58Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...0f33998fc9` -> `239	6`; `git rev-list --left-right --count origin/main...0f33998fc9` -> `163	6`; `git cherry -v` vs `trunk` = `+6/-0`, vs `main` = `+6/-0`.
- **Nhận định:** Chứa 4 commit code/perf/fauna/body (`82c1401`, `9e68f38`, `6e20198`, `0f33998`) + `b6abc91` (docs) chưa lên cả thân cây lẫn `main`; riêng `6035608` (CPU default runtime) đã được cherry-pick sang thân cây ở `75dfef4` (nhưng khác patch-id do xung đột 2 hunk doc nên `git cherry` vẫn hiện `+`).

#### PR `#31` — `feat/oss-072-mrca` (`REBASE`)
- **Tiêu đề:** feat(lineage): OSS-072 -- the MRCA of a DAG is a set, not a node
- **SHA đầy đủ:** `b14e836c42b7d7711058decaa055048ed2a4baf4` · **Base:** `main` · **Cập nhật GitHub:** `2026-07-29T06:56:43Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...b14e836c42` -> `239	4`; `git rev-list --left-right --count origin/main...b14e836c42` -> `163	4`; `git cherry -v` vs `trunk` = `+4/-0`, vs `main` = `+4/-0`.
- **Nhận định:** Chứa 4 commit OSS-072 (`a6d06ac` thêm `mrca.rs` + `lineage_mrca_tests.rs` + TS/IPC MRCA; `365edb4`, `1f9751e`, `b14e836` docs) chưa có trên thân cây hay `main`; mới chỉ được thử nghiệm port sang nhánh chưa merge `claude/funny-thompson-jj42fe` ở `3f9f054`.

#### PR `#34` — `docs/ht0-7-remeasure-main` (`STALE-DOC`)
- **Tiêu đề:** docs(state): re-measure main at 1b3e7ef after #33 and ignore lockfile-only zip advisories (HT0.7)
- **SHA đầy đủ:** `f0a33cb4649a417fa82ccc75423ec8edec115d0d` · **Base:** `main` · **Cập nhật GitHub:** `2026-10-17T10:29:05Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...f0a33cb464` -> `109	15`; `git rev-list --left-right --count origin/main...f0a33cb464` -> `20	2`; `git cherry -v` vs `trunk` = `+14/-0`, vs `main` = `+2/-0`.
- **Nhận định:** Chỉ cập nhật số đo `STATE_OF_THE_PROJECT.md` & `COMPLETION_ROADMAP_2026-10-16.md` cho `main` tại mốc cũ `1b3e7ef` (đã tụt 20 commit sau `origin/main` và không tương thích thân cây); phần `zip` ignore ở `.cargo/audit.toml` (`2162295`) đã nằm trong `f73086c` trên thân cây (chỉ còn `deny.toml` chưa mirror).

#### PR `#35` — `fix/ht0-5a-sensor-raycast` (`REBASE`)
- **Tiêu đề:** fix(sensors): guard degenerate ray direction and test self-collider raycast (HT0.5a)
- **SHA đầy đủ:** `3560faa6c06ad96dea3657338364f42637010511` · **Base:** `main` · **Cập nhật GitHub:** `2026-10-17T13:08:27Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...3560faa6c0` -> `109	15`; `git rev-list --left-right --count origin/main...3560faa6c0` -> `20	2`; `git cherry -v` vs `trunk` = `+14/-0`, vs `main` = `+2/-0`.
- **Nhận định:** Dù bản đối ứng HT0.5a đã lên `main` ở `ef9bc3a`, **toàn bộ 5/6 hunk (3 hunk code/test + 2 hunk `deny.toml`)** (`spatial.rs` 2 hunk, `physics_tests.rs` test `sensor_ray_from_agent_centre_sees_past_its_own_collider`, `deny.toml` 2 hunk) **chưa có trên thân cây** (`491ec61`); chỉ `.cargo/audit.toml` (1 hunk) đã được `f73086c` thay thế.

#### PR `#36` — `fix/ht0-5b-landscape-dispose` (`REBASE`)
- **Tiêu đề:** fix(landscape): guard intrinsic dispose calls, clean up geometries/audio, and reuse scratch vectors (HT0.5b)
- **SHA đầy đủ:** `a5b7960d6eb6f422608017a67754c3ba57a7ddfd` · **Base:** `main` · **Cập nhật GitHub:** `2026-10-17T13:08:32Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...a5b7960d6e` -> `109	15`; `git rev-list --left-right --count origin/main...a5b7960d6e` -> `20	2`; `git cherry -v` vs `trunk` = `+14/-0`, vs `main` = `+2/-0`.
- **Nhận định:** Dù 13/14 file `Landscape/*` và `tests/e2e/tauri-mock.ts` đã lên `main` ở `a18c19f` (và 14/17 file khớp từng byte với `origin/main`, riêng `Terrain.tsx` trên `main` dùng GPU `applyShoreWave`), **toàn bộ 15 file frontend/mock + `deny.toml`** (74/75 hunk) **chưa có trên thân cây**; chỉ `.cargo/audit.toml` (1/75 hunk) đã có trên thân cây qua `f73086c`.

#### PR `#37` — `fix/ht0-5c-sim-semantics` (`REBASE`)
- **Tiêu đề:** fix(sim): reachable-only phenotype mass, zero-energy predator stop, joint axis guard, CPG phase wrap, loop FPS throttle (HT0.5c)
- **SHA đầy đủ:** `c7829689ecff3ef19612caa8e02f0b75f6d10cf3` · **Base:** `main` · **Cập nhật GitHub:** `2026-10-17T13:08:38Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...c7829689ec` -> `109	19`; `git rev-list --left-right --count origin/main...c7829689ec` -> `20	6`; `git cherry -v` vs `trunk` = `+18/-0`, vs `main` = `+6/-0`.
- **Nhận định:** **Toàn bộ 29/30 hunk** (`cpg.rs`, `agent_systems.rs`, `simulation_loop.rs`, `genotype.rs`, `dynamics.rs`, `physics_tests.rs`, `deny.toml`) **chưa có trên thân cây** (chỉ `.cargo/audit.toml`, 1/30 hunk, đã có qua `f73086c`); ngoài ra có 4 phần chưa lên ngay cả `origin/main`: test CPG `tick_wraps_phase_into_zero_to_two_pi_across_multi_period_and_negative_steps`, `DevelopedPhenotype` + BFS stack `STACK_CAP = 64`, `agent_systems.rs` (3 hunk) và `simulation_loop.rs` (`Option<&DevelopedPhenotype>`).

#### PR `#38` — `fix/live-world-restores-its-spatial-grid` (`REBASE`)
- **Tiêu đề:** fix(live): restore the spatial grid on snapshot load, so a resumed run is not blind for a tick
- **SHA đầy đủ:** `fa938c5428077efdd26115016a8b3527de37d7ce` · **Base:** `main` · **Cập nhật GitHub:** `2026-10-31T09:10:33Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...fa938c5428` -> `109	34`; `git rev-list --left-right --count origin/main...fa938c5428` -> `0	1`; `git cherry -v` vs `trunk` = `+33/-0`, vs `main` = `+1/-0`.
- **Nhận định:** Chồng `origin/main` (33 commit) + `fa938c5` khôi phục `SpatialHashGrid` khi load snapshot (`109` sau / `34` trước thân cây); là gốc chuỗi T2 / Wave 1. CI trên PR đang `Rust: FAILURE`, `Frontend: FAILURE`.

#### PR `#39` — `fix/declare-simrng-draw-order` (`REBASE`)
- **Tiêu đề:** fix(sched): declare the SimRng draw order, so one seed gives one world in every process
- **SHA đầy đủ:** `03b331f30de37312642d3a2c22eb6c3d1a2b0cd7` · **Base:** `fix/live-world-restores-its-spatial-grid` · **Cập nhật GitHub:** `2026-10-31T13:08:24Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...03b331f30d` -> `109	35`; `git rev-list --left-right --count origin/main...03b331f30d` -> `0	2`; `git cherry -v` vs `trunk` = `+34/-0`, vs `main` = `+2/-0`.
- **Nhận định:** Đứng trên `#38` thêm `03b331f` chuẩn hóa thứ tự rút `SimRng` (cùng thuộc chuỗi T2 / Wave 1); chưa có trên thân cây. CI `Rust: FAILURE`, `Frontend: FAILURE`.

#### PR `#40` — `fix/ci-gates-green` (`REBASE`)
- **Tiêu đề:** chore(ci): clear all eight failing Rust and frontend CI gates
- **SHA đầy đủ:** `78b2e85872dc429a9da9da9ee68c4c508908ea75` · **Base:** `fix/declare-simrng-draw-order` · **Cập nhật GitHub:** `2026-10-31T15:29:03Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...78b2e85872` -> `109	36`; `git rev-list --left-right --count origin/main...78b2e85872` -> `0	3`; `git cherry -v` vs `trunk` = `+35/-0`, vs `main` = `+3/-0`.
- **Nhận định:** Đứng trên `#39` thêm `78b2e85` dọn 8 gate Rust & frontend (đỉnh hiện tại của T2 theo `AGENT_WORKFLOW.md`, chờ T1 B3b xong trước khi chạy T2); chưa có trên thân cây. CI `Rust: FAILURE`, `Frontend: FAILURE`.

#### PR `#41` — `fix/birth-death-zero-alloc` (`REBASE`)
- **Tiêu đề:** fix(core,test): repair allocator test on macOS and eliminate birth/death command allocations
- **SHA đầy đủ:** `2c5423b4df29e8443246287d7ba243ed7f17063f` · **Base:** `fix/ci-gates-green` · **Cập nhật GitHub:** `2026-10-31T19:21:26Z`
- **Lệnh đối chiếu:** `git rev-list --left-right --count origin/feat/ecological-selection...2c5423b4df` -> `109	38`; `git rev-list --left-right --count origin/main...2c5423b4df` -> `0	5`; `git cherry -v` vs `trunk` = `+37/-0`, vs `main` = `+5/-0`.
- **Nhận định:** Đứng trên `#40` thêm `36e18d8` + `2c5423b` (zero-alloc birth/death & barrier allocator test); thuộc Wave 2 (`B2`) sau khi T2 xong; chưa có trên thân cây. CI `Rust: FAILURE`, `Frontend: FAILURE`.

```bash
$ git cherry -v origin/feat/ecological-selection origin/fix/document-the-wgpu-registry-leak
+ b6abc9150926eeaeef15455f545f999d097b2cda docs(perf): the rest of the leak is wgpu's registry, and the GPU path is the default
+ 6035608e59627f53bbbf60f9cabe3f65db931f3e feat(ml): default the learner to CPU, because the GPU path grows without bound
+ 82c1401eabde2ccfa5241d904ad55f35c3bee50e feat(world): the animals were standing still, and there were not many of them
+ 9e68f386483bb601d6efcc0bc56f788e8f69b250 feat(world): six more species, in the biomes that had none
+ 6e201982be8df034a59b6fa807cca550baf2f971 feat(map): give the simulated creatures bodies, and stop planting them in rows
+ 0f33998fc99af56d81f96c058079fad659c92ad2 perf(sim): four bottlenecks removed, and none of them was the bottleneck

$ git cherry -v origin/feat/ecological-selection origin/feat/oss-072-mrca
+ a6d06acbf460e65b9b44eb175537837cb5ccb2bd feat(lineage): OSS-072 -- the MRCA of a DAG is a set, not a node
+ 365edb48c54ac3b4061d25f442c016095f8948c5 docs(state): OSS-072 landed, and the frontend suite was never as green as recorded
+ 1f9751e6ded403d765d727ccf377be0c48fcd0ba docs(state): the red frontend suite is CRLF, not the code -- correcting my own finding
+ b14e836c42b7d7711058decaa055048ed2a4baf4 fix(docs): resolve same-page anchors, because I shipped a broken one

$ git cherry -v origin/feat/ecological-selection origin/fix/ht0-5c-sim-semantics
+ 286e45a81c612a74bb18da5faf3ef587866bb212 fix(ci): remove the conflict markers 4b6258d committed, keeping both halves of the gate
+ 3fbeaf427e828d41036c6c3aadae9d11bd124b57 fix(license): align generated compliance metadata
+ 96ac98b913641450257e6531b0130c5cbc27f002 fix(deps): refresh vulnerable npm transitives
+ 363fd0e530ab24098306e813a6c451c8e341c354 fix(license): the NOTICE generator still called the project proprietary
+ a0c1b097c1d8531d749b7abb31effbb39e11ee76 build(rust): pin the toolchain, and fix the two lints 1.98 added
+ 26e410455daa3ffc2a54b206443658a971f7b330 docs(planning): a completion roadmap, and the measurement it starts from
+ 0d224ec0f65295f27de9421e8b64bf4c9a04aba9 fix(deps): rustls 0.23.42 -> 0.23.46, for four advisories published after main last ran
+ 59a422fcef6cb3841b7ba514e9061be8331542cf docs(state): record the rustls advisories as the second reason main stays red
+ b021e4450bb108d7befdba66f3845781a0428258 fix(bindings): WorkloadContext.ts was edited by hand when four fields were added
+ ac1610664f4b3d3cf5e88de7b517cc6473fccdcd fix(deps): refresh npm transitives for advisories published after the last green run
+ 4cf71ee862797e40f6fc45095b316f636b04e8a4 docs(state): the npm advisories and the hand-edited binding were also hiding behind a red main
+ 4aed9a024116ca54e52786d38b70afbcbc67a62e docs(state): the fix branch, measured locally against every gate ci.yml runs
+ ca188f6457acc895edc0223353c114cef19034ca fix(audit): ignore three lockfile-only zip 0.6.6 advisories from disabled burn backends
+ d3965557c027171468fc24c54ccf10c33f862a6a fix(evolution): count only reachable nodes in total_mass and carry mass in DevelopedPhenotype (HT0.5c-1)
+ 41d30dec9f20f4776b6f86470578e28624d65a66 fix(physics): stop depleted predator movement when energy reaches zero (HT0.5c-2)
+ df5f333679885782f427c189838dbee4ddbb1f71 fix(physics): use try_normalize for joint axes to prevent NaN rotations (HT0.5c-3)
+ 0ced745f5d25d03062c2d25c9bb2cdda64feb942 fix(cpg): wrap oscillator phase into [0, 2pi) with rem_euclid (HT0.5c-4)
+ c7829689ecff3ef19612caa8e02f0b75f6d10cf3 fix(core): throttle reported loop FPS to target frame duration via compute_loop_fps (HT0.5c-5)
```

> **Lưu ý về PR `#30` (`6035608`):** Commit `75dfef4` trên thân cây đã cherry-pick phần thay đổi runtime của `6035608`, nhưng thông điệp commit `75dfef4` xác nhận 2 hunk tài liệu (`STATE_OF_THE_PROJECT.md`, `src-tauri/CLAUDE.md`) đã xung đột và được giải quyết theo văn bản nhánh đích nên `git cherry` vẫn hiển thị `+ 6035608`. Bốn commit mã/dữ liệu fauna/hiệu năng phía sau (`82c1401`, `9e68f38`, `6e20198`, `0f33998`) cùng `b6abc91` chưa nằm trên cả `origin/main` lẫn `origin/feat/ecological-selection`.

---

## 3. Phân tích chi tiết từng hunk chưa lên thân cây của PR `#35`, `#36`, `#37`

### 3.1 Cấu trúc gốc chung và chênh lệch tổng thể

Cả ba PR `#35` (`3560faa6c0`), `#36` (`a5b7960d6e`), `#37` (`c7829689ec`) cùng tách từ nền **`1b3e7ef8ee`** (commit merge PR `#33 fix/main-ci-green` trên `main`, ngày 2026-10-17). Do `origin/feat/ecological-selection` tách trước phần lớn lịch sử `043930c1..1b3e7ef8ee`:

- **Phần nền kế thừa (`043930c1..1b3e7ef8ee` — 13 commit):** Gồm `286e45a`, `3fbeaf4`, `96ac98b`, `363fd0e`, `a0c1b09`, `26e4104`, `0d224ec`, `59a422f`, `b021e44`, `ac16106`, `4cf71ee`, `4aed9a0`, `1b3e7ef`. Toàn bộ **13 commit này đã nằm trên `origin/main`** (và trong chuỗi `#38–#40`), nên ở đây không liệt kê lại từng hunk của nền `#33` mà chỉ giải quyết thông qua đợt hợp nhất T2 (`#38–#40`). Riêng đối chiếu các tệp mà chính mỗi PR chạm vào (`git diff 1b3e7ef8ee..PR` so với `origin/main` và `trunk`):
  - **PR `#35` (4 file):** khác `origin/main` **`235` dòng** (`spatial.rs` khớp hoàn toàn 0 dòng khác với `origin/main`, chỉ khác `physics_tests.rs`, `audit.toml`, `deny.toml`); khác `trunk` **`229` dòng**.
  - **PR `#36` (17 file):** khác `origin/main` **`279` dòng** (**14/17 file khớp từng byte với `origin/main`**; chỉ khác `Terrain.tsx`, `audit.toml`, `deny.toml`); khác `trunk` **`893` dòng**.
  - **PR `#37` (8 file):** khác `origin/main` **`1025` dòng** (cả 8 file có chênh lệch vs `main` do `cpg.rs` test mới, `genotype.rs` `DevelopedPhenotype` + stack BFS, `agent_systems.rs`, `simulation_loop.rs`, `audit.toml`/`deny.toml` và chỉnh sửa 2026-08-16 trên `main`); khác `trunk` **`1256` dòng**.

**Kết luận tổng quát cho các commit riêng của `#35–#37` (`1b3e7ef8ee..PR`):**
- Ngoại trừ `src-tauri/.cargo/audit.toml` (3 ignore `zip 0.6.6` đã có bản đầy đủ hơn trên thân cây ở `f73086c`), **100% các hunk code Rust, test Rust, frontend `Landscape/*`, `tauri-mock.ts`, và `src-tauri/deny.toml` trong `#35`, `#36`, `#37` ĐỀU CHƯA CÓ TRÊN THÂN CÂY (`491ec61`)**.
- So với `origin/main` (`a43872c5e1`), phần lớn đã được gộp thành `ef9bc3a` và `a18c19f`, nhưng **không đồng nhất theo từng byte**: vẫn còn những hunk nằm trên PR `#36–#37` mà **cả `main` lẫn `trunk` đều chưa có** hoặc đã viết khác (`Terrain.tsx`, `genotype.rs`, `cpg.rs` test mới, `agent_systems.rs`, `simulation_loop.rs` `DevelopedPhenotype`, `deny.toml`).

### 3.2 Toàn bộ hunk của PR `#35` (`fix/ht0-5a-sensor-raycast`: `1b3e7ef8ee..3560faa6c0` — 4 file, 6 hunk)

| File | Hunk (`git diff -U0 1b3e7ef8ee..3560faa`) | Tóm tắt thay đổi | Trạng thái trên `trunk` (`491ec61`) | Đối chiếu `origin/main` (`a43872c`) |
|---|---|---|---|---|
| `src-tauri/.cargo/audit.toml` | `@@ -36,0 +37,17 @@ ignore = [` | Thêm ignore 3 advisory lockfile-only của `zip 0.6.6` (`RUSTSEC-2026-0367`, `-0368`, `-0369` qua optional `burn-candle`/`burn-tch` không biên dịch). | **ĐÃ CÓ TRÊN THÂN CÂY** (bị thay bởi commit `f73086c` đã gồm đủ 3 ID này cộng `-0403`, `-0410`). | Chưa có trên `origin/main`. |
| `src-tauri/deny.toml` | `@@ -106,2 +106,3 @@ allow-git = []` | Cập nhật comment giải thích từ 2 mục `gix` sang 5 mục (gồm `zip 0.6.6`). | **CHƯA CÓ trên thân cây** (`src-tauri/deny.toml` trên thân cây vẫn là 2 mục `gix`). | Chưa có trên `origin/main`. |
| `src-tauri/deny.toml` | `@@ -110,0 +112,3 @@ ignore = [` | Thêm `RUSTSEC-2026-0367`, `-0368`, `-0369` vào danh sách `advisories.ignore` của `cargo-deny`. | **CHƯA CÓ trên thân cây** (`src-tauri/deny.toml` ở `491ec61` chỉ có `-0350` và `-0021`). | Chưa có trên `origin/main`. |
| `src-tauri/src/physics/spatial.rs` | `@@ -328,0 +329,3 @@ fn intersect_sphere(` | Chặn tia suy biến `if a <= 0.0 { return None; }` trong `intersect_sphere` thay vì chia cho 0 / ra `NaN`. | **CHƯA CÓ trên thân cây**. | Đã có bản tương ứng trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/physics/spatial.rs` | `@@ -340,0 +344,3 @@ fn intersect_sphere(` | Ghi chú rõ ràng lý do tia bắt đầu trong lòng hình cầu (`t1 < 0`) không được trả về va chạm (tránh tự chặn tia cảm biến bởi collider của chính sinh vật). | **CHƯA CÓ trên thân cây**. | Đã có bản tương ứng trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/tests/physics_tests.rs` | `@@ -676,0 +677,54 @@ fn test_decoupled_systems_zero_allocation() {` | Thêm test hồi quy `sensor_ray_from_agent_centre_sees_past_its_own_collider` kiểm tra tia phóng từ tâm collider xuyên ra mục tiêu phía trước (+54 dòng). | **CHƯA CÓ trên thân cây** (`src-tauri/tests/physics_tests.rs` trên thân cây kết thúc ở dòng 681). | Đã có trên `origin/main` (dòng 846, qua `ef9bc3a`). |

### 3.3 Toàn bộ hunk của PR `#36` (`fix/ht0-5b-landscape-dispose`: `1b3e7ef8ee..a5b7960d6e` — 17 file, 75 hunk)

| File | Hunk (`git diff -U0 1b3e7ef8ee..a5b7960`) | Tóm tắt thay đổi | Trạng thái trên `trunk` (`491ec61`) | Đối chiếu `origin/main` (`a43872c`) |
|---|---|---|---|---|
| `src-tauri/.cargo/audit.toml` | `@@ -36,0 +37,17 @@ ignore = [` | Thêm ignore 3 advisory `zip 0.6.6` (`RUSTSEC-2026-0367`, `-0368`, `-0369`) trong `.cargo/audit.toml` (+17 dòng). | **ĐÃ CÓ TRÊN THÂN CÂY** (bị thay bởi `f73086c` đã có `-0367..-0369` cùng `-0403`, `-0410`). | Chưa có trên `origin/main`. |
| `src-tauri/deny.toml` | `@@ -106,2 +106,3 @@ allow-git = []` | Cập nhật comment ở `[advisories]` giải thích 5 advisory không biên dịch gồm `zip`. | **CHƯA CÓ trên thân cây** (`491ec61` chỉ ignore `-0350`, `-0021`). | Chưa có trên `origin/main`. |
| `src-tauri/deny.toml` | `@@ -110,0 +112,3 @@ ignore = [` | Thêm `RUSTSEC-2026-0367`, `-0368`, `-0369` vào `advisories.ignore` trong `deny.toml`. | **CHƯA CÓ trên thân cây** (`491ec61` chỉ ignore `-0350`, `-0021`). | Chưa có trên `origin/main`. |
| `src/components/Landscape/CameraControls.tsx` | `@@ -12,0 +13,6 @@ extend({ OrbitControls });` | Khai báo 6 biến `THREE.Vector3` scratch ở cấp module (`scratchForward`, `scratchRight`, `scratchMoveDir`, `scratchGazeDir`, `scratchUpVec`, …) để tránh cấp phát mỗi frame. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/CameraControls.tsx` | `@@ -227,2 +233,2 @@ export const CameraControls: React.FC<CameraControlsProps> = ({` | Tái sử dụng `scratchForward` và `scratchRight` khi tính hướng đi ngang. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/CameraControls.tsx` | `@@ -230 +236 @@ export const CameraControls: React.FC<CameraControlsProps> = ({` | Tái sử dụng `scratchMoveDir.set(0, 0, 0)` thay `new THREE.Vector3()`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/CameraControls.tsx` | `@@ -268 +274 @@ export const CameraControls: React.FC<CameraControlsProps> = ({` | Tái sử dụng `scratchGazeDir` thay `new THREE.Vector3()` khi tính `gazeDir`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/CameraControls.tsx` | `@@ -305,2 +311,2 @@ export const CameraControls: React.FC<CameraControlsProps> = ({` | Tái sử dụng `scratchForward` và `scratchUpVec` cho vector nhìn thẳng và hướng lên. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Minimap.tsx` | `@@ -94,0 +95,5 @@ export const Minimap: React.FC<MinimapProps> = ({` | Kiểm tra an toàn `typeof ctx.createImageData === 'function'` trước khi tạo `imgData` và tô pixel khi không null. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Minimap.tsx` | `@@ -99,3 +104 @@ export const Minimap: React.FC<MinimapProps> = ({` | Gộp điều kiện gọi `ctx.putImageData` bằng guard `if (imgData && typeof ctx.putImageData === 'function')`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/PositionalAudio.tsx` | `@@ -19,0 +20,3 @@ export const PositionalAudio: React.FC<PositionalAudioProps> = ({` | Thêm cleanup `useEffect` gọi `audioManager.destroySpatialSource(id)` khi component unmount. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Terrain.tsx` | `@@ -1,2 +1,2 @@` | Đổi import React (`useEffect` thêm vào, bỏ `useFrame` từ `@react-three/fiber`). | **CHƯA CÓ trên thân cây**. | Khác trên `origin/main` (`a18c19f`): `main` giữ sóng bờ bằng GPU shader `applyShoreWave` thay vì bỏ hẳn `useFrame`. |
| `src/components/Landscape/Terrain.tsx` | `@@ -147,32 +147,9 @@ export const Terrain: React.FC<TerrainProps> = ({ width = 500, height = 500, wet` | Bỏ vòng lặp CPU `useFrame` chỉnh vertex `geomRef0..2` mỗi khung hình; thay bằng `useEffect` cleanup gọi `typeof g.dispose === 'function'`. | **CHƯA CÓ trên thân cây**. | Khác trên `origin/main` (`a18c19f`): `main` chuyển sóng bờ sang shader GPU `applyShoreWave` + `dispose()` có guard. |
| `src/components/Landscape/Terrain.tsx` | `@@ -180,5 +157 @@ export const Terrain: React.FC<TerrainProps> = ({ width = 500, height = 500, wet` | Hoàn tất đóng khối `useEffect` cleanup (bỏ các lời gọi `animateLOD(...)` còn lại). | **CHƯA CÓ trên thân cây**. | Khác trên `origin/main` (`a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -484,0 +485,12 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm `useEffect` unmount gọi `typeof x.dispose === 'function'` cho các `useMemo` geometry/material thảm thực vật (+12 dòng). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -642,0 +655 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -642,0 +655 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -650,0 +664 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -650,0 +664 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -659,0 +674 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -659,0 +674 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -667,0 +683 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -667,0 +683 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -675,0 +692 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -675,0 +692 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -683,0 +701 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -683,0 +701 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -692,0 +711 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -692,0 +711 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -701,0 +721 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -701,0 +721 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -710,0 +731 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -710,0 +731 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -718,0 +740 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -718,0 +740 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -727,0 +750 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -727,0 +750 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -736,0 +760 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -736,0 +760 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -744,0 +769 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -744,0 +769 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -753,0 +779 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -753,0 +779 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -761,0 +788 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -761,0 +788 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -770,0 +798 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -770,0 +798 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Vegetation.tsx` | `@@ -779,0 +808 @@ export const Vegetation: React.FC<VegetationProps> = ({` | Thêm prop `frustumCulled={false}` cho `InstancedMesh` thảm thực vật (`@@ -779,0 +808 @@`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Water.tsx` | `@@ -18,0 +19,3 @@ const RIVER_BLEND_RANGE = 6.5 * HEIGHT_CONV;` | Khai báo scratch vector `scratchSunDir`, `scratchMoonDir`, `scratchLightDir` ngoài vòng render. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Water.tsx` | `@@ -498,0 +502,6 @@ export const Water: React.FC<WaterProps> = ({` | Thêm `useEffect` cleanup hủy geometry mặt nước (`typeof ...dispose === 'function'`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Water.tsx` | `@@ -584 +593 @@ export const Water: React.FC<WaterProps> = ({` | Tái sử dụng `scratchSunDir.set(...)` trong `useFrame`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Water.tsx` | `@@ -591 +600 @@ export const Water: React.FC<WaterProps> = ({` | Tái sử dụng `scratchMoonDir.set(...)` trong `useFrame`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Water.tsx` | `@@ -684,0 +694,3 @@ export const Water: React.FC<WaterProps> = ({` | Lấy tham chiếu `oceanMat`, `riverMat` trước cleanup để tránh cảnh báo ref thay đổi lúc unmount. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/Water.tsx` | `@@ -690,0 +703,6 @@ export const Water: React.FC<WaterProps> = ({` | Guard `typeof oceanMat.dispose === 'function'` và `typeof riverMat.dispose === 'function'` dưới R3F mock. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldBirds.tsx` | `@@ -1 +1 @@` | Import `useEffect` trong `WorldBirds.tsx`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldBirds.tsx` | `@@ -5,0 +6,2 @@ import { sceneElapsed } from './utils/sceneClock';` | Khai báo `const scratchDummy = new THREE.Object3D();` ngoài component. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldBirds.tsx` | `@@ -39,0 +42,2 @@ export const WorldBirds: React.FC<WorldBirdsProps> = ({ renderSize = 400, count` | Thêm `useEffect(() => () => geom.dispose(), [geom]);`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldBirds.tsx` | `@@ -59 +63 @@ export const WorldBirds: React.FC<WorldBirdsProps> = ({ renderSize = 400, count` | Tái sử dụng `scratchDummy` trong vòng lặp `useFrame` thay cho `new THREE.Object3D()`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldCameraRig.tsx` | `@@ -18,0 +19,6 @@ extend({ OrbitControls });` | Khai báo 6 biến scratch (`scratchEuler`, `scratchForward`, `scratchRight`, `scratchWish`, `UP_VECTOR`, …). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldCameraRig.tsx` | `@@ -358 +364,2 @@ export const WorldCameraRig: React.FC<WorldCameraRigProps> = ({` | Dùng `scratchEuler.set(...)` khi cập nhật `camera.quaternion`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldCameraRig.tsx` | `@@ -363,2 +370,2 @@ export const WorldCameraRig: React.FC<WorldCameraRigProps> = ({` | Đưa vector hướng nhìn vào `scratchForward` thay vì cấp phát mới. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldCameraRig.tsx` | `@@ -366,2 +373,2 @@ export const WorldCameraRig: React.FC<WorldCameraRigProps> = ({` | Chiếu phẳng `scratchForward.y = 0` và chuẩn hóa tại chỗ. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldCameraRig.tsx` | `@@ -369 +376 @@ export const WorldCameraRig: React.FC<WorldCameraRigProps> = ({` | Tính `scratchRight.crossVectors(scratchForward, UP_VECTOR).normalize()` không cấp phát. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldCameraRig.tsx` | `@@ -371,7 +378,7 @@ export const WorldCameraRig: React.FC<WorldCameraRigProps> = ({` | Tích lũy hướng di chuyển bàn phím WASD/mũi tên vào `scratchWish`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldCameraRig.tsx` | `@@ -384,2 +391,2 @@ export const WorldCameraRig: React.FC<WorldCameraRigProps> = ({` | Đọc vận tốc mục tiêu `targetVx`/`targetVz` từ `scratchWish`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldCameraRig.tsx` | `@@ -482,2 +489,2 @@ export const WorldCameraRig: React.FC<WorldCameraRigProps> = ({` | Đọc `v.targetX`/`v.targetZ` từ `scratchForward`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldFish.tsx` | `@@ -1 +1 @@` | Import `useEffect` trong `WorldFish.tsx`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldFish.tsx` | `@@ -7,0 +8,2 @@ import { sceneElapsed } from './utils/sceneClock';` | Khai báo `const scratchDummy = new THREE.Object3D();` ngoài component. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldFish.tsx` | `@@ -53,0 +56,3 @@ export const WorldFish: React.FC<WorldFishProps> = ({` | Thêm `useEffect(() => () => geom.dispose(), [geom]);`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldFish.tsx` | `@@ -139 +144 @@ export const WorldFish: React.FC<WorldFishProps> = ({` | Tái sử dụng `scratchDummy` trong `useFrame` thay cho `new THREE.Object3D()`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldMinimap.tsx` | `@@ -127,0 +128,5 @@ export const WorldMinimap: React.FC<WorldMinimapProps> = ({` | Kiểm tra `typeof ctx.createImageData === 'function'` trước khi vẽ pixel vào `img`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldMinimap.tsx` | `@@ -131,3 +136 @@ export const WorldMinimap: React.FC<WorldMinimapProps> = ({` | Gọi `ctx.putImageData` có guard `img && typeof ctx.putImageData === 'function'`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldSky.tsx` | `@@ -225 +225,5 @@ export const WorldSky: React.FC<WorldSkyProps> = ({` | Cập nhật `state.scene.background` tại chỗ (`if (... instanceof THREE.Color) .set(...)`) thay vì tạo `new THREE.Color` mỗi frame. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldVegetation.tsx` | `@@ -1 +1 @@` | Import `useEffect` trong `WorldVegetation.tsx`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldVegetation.tsx` | `@@ -150,0 +151 @@ const TypedInstances: React.FC<{` | Thêm `frustumCulled={false}` cho `InstancedMesh` ở `TypedInstances`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldVegetation.tsx` | `@@ -166,0 +168,7 @@ export const WorldVegetation: React.FC<WorldVegetationProps> = ({` | Thêm `useEffect` dispose các geometry/material của thảm thực vật khi unmount (+7 dòng). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldWildlife.tsx` | `@@ -1 +1 @@` | Import `useEffect` trong `WorldWildlife.tsx`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldWildlife.tsx` | `@@ -32,0 +33,3 @@ function hash01(n: number): number {` | Khai báo `scratchDummy = new THREE.Object3D()`, `scratchTint = new THREE.Color()` (và scratch màu liên quan). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldWildlife.tsx` | `@@ -125,0 +129,9 @@ export const WorldWildlife: React.FC<WorldWildlifeProps> = ({` | Thêm `useEffect` cleanup hủy geometry/material thú hoang dã khi unmount (+9 dòng). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldWildlife.tsx` | `@@ -274 +286 @@ export const WorldWildlife: React.FC<WorldWildlifeProps> = ({` | Tái sử dụng `scratchDummy` trong vòng lặp cập nhật instance. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/WorldWildlife.tsx` | `@@ -295 +307 @@ export const WorldWildlife: React.FC<WorldWildlifeProps> = ({` | Tái sử dụng `scratchTint` thay cho `new THREE.Color()`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/utils/audioManager.ts` | `@@ -27,0 +28 @@ export interface SpatialPanner {` | Thêm method tùy chọn `disconnect?(): void;` vào interface `SpatialPanner`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/utils/audioManager.ts` | `@@ -62,0 +64 @@ export class AudioManager {` | Thêm field `private readonly sources: Map<string, AudioNode[]> = new Map();` theo dõi node âm thanh theo `id`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/utils/audioManager.ts` | `@@ -187,0 +190 @@ export class AudioManager {` | Khởi tạo mảng `nodes: AudioNode[] = []` khi tạo nguồn âm thanh không gian. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/utils/audioManager.ts` | `@@ -206,0 +210 @@ export class AudioManager {` | Đẩy `src, filter` vào danh sách `nodes` của nhánh tạo noise. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/utils/audioManager.ts` | `@@ -217,0 +222 @@ export class AudioManager {` | Đẩy `osc, gain` vào `nodes` của nhánh tạo dao động thứ nhất. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/utils/audioManager.ts` | `@@ -228,0 +234 @@ export class AudioManager {` | Đẩy `osc, gain` vào `nodes` của nhánh tạo dao động thứ hai. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/utils/audioManager.ts` | `@@ -229,0 +236 @@ export class AudioManager {` | Lưu `this.sources.set(id, nodes)` vào bản đồ nguồn đang mở. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `src/components/Landscape/utils/audioManager.ts` | `@@ -275,0 +283,51 @@ export class AudioManager {` | Thêm `destroySpatialSource(id)` và `dispose()` để ngắt toàn bộ node & đóng `AudioContext` an toàn (+51 dòng). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `tests/e2e/tauri-mock.ts` | `@@ -9,0 +10 @@ import type { LegacyImportListing } from '../../src/types/generated/LegacyImport` | Import kiểu sinh tự động `LineageGraphPayload` từ `src/types/generated/LineageGraphPayload`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |
| `tests/e2e/tauri-mock.ts` | `@@ -92 +93 @@ const DETERMINISTIC_REPLIES: Record<string, unknown> = {` | Sửa reply giả cho `get_lineage_graph` thành `{ nodes: [], links: [], db_connected: false } satisfies LineageGraphPayload` thay vì `{ nodes: [], edges: [] }`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `a18c19f`). |

### 3.4 Toàn bộ hunk của PR `#37` (`fix/ht0-5c-sim-semantics`: `1b3e7ef8ee..c7829689ec` — 8 file, 30 hunk)

| File | Hunk (`git diff -U0 1b3e7ef8ee..c782968`) | Tóm tắt thay đổi | Trạng thái trên `trunk` (`491ec61`) | Đối chiếu `origin/main` (`a43872c`) |
|---|---|---|---|---|
| `src-tauri/.cargo/audit.toml` | `@@ -36,0 +37,17 @@ ignore = [` | Thêm ignore 3 advisory `zip 0.6.6` (`RUSTSEC-2026-0367`, `-0368`, `-0369`) trong `.cargo/audit.toml` (+17 dòng). | **ĐÃ CÓ TRÊN THÂN CÂY** (bị thay bởi `f73086c` đã có `-0367..-0369` cùng `-0403`, `-0410`). | Chưa có trên `origin/main`. |
| `src-tauri/deny.toml` | `@@ -106,2 +106,3 @@ allow-git = []` | Cập nhật comment `[advisories]` nhắc đủ 5 lockfile-only advisories (`gix` + `zip`). | **CHƯA CÓ trên thân cây**. | Chưa có trên `origin/main`. |
| `src-tauri/deny.toml` | `@@ -110,0 +112,3 @@ ignore = [` | Thêm `RUSTSEC-2026-0367`, `-0368`, `-0369` vào `advisories.ignore` trong `deny.toml`. | **CHƯA CÓ trên thân cây** (`491ec61` chỉ ignore `-0350`, `-0021`). | Chưa có trên `origin/main`. |
| `src-tauri/src/ai/cpg.rs` | `@@ -22,4 +22,2 @@ impl CpgOscillator {` | Đổi wrap pha CPG từ trừ một lần `self.phase -= 2*pi` sang `(self.phase + two_pi * freq * dt).rem_euclid(two_pi)` để pha luôn thuộc `[0, 2π)` dù bước thời gian lớn hay âm. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/ai/cpg.rs` | `@@ -60,0 +59,34 @@ pub struct TimeStep(pub f32);` | Thêm unit test `tick_wraps_phase_into_zero_to_two_pi_across_multi_period_and_negative_steps` (+34 dòng). | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`** (test này không có ở cả `main` lẫn `trunk`). |
| `src-tauri/src/core/agent_systems.rs` | `@@ -307,0 +308 @@ pub fn check_epoch_completion_system(` | Bổ sung `Option<&crate::evolution::genotype::DevelopedPhenotype>` vào query của `check_epoch_completion_system`. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`** (`DevelopedPhenotype` chưa từng lên `main`). |
| `src-tauri/src/core/agent_systems.rs` | `@@ -341 +342 @@ pub fn check_epoch_completion_system(` | Ràng buộc biến `pheno` trong vòng `for` của `check_epoch_completion_system`. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`**. |
| `src-tauri/src/core/agent_systems.rs` | `@@ -347,2 +348,3 @@ pub fn check_epoch_completion_system(` | Ưu tiên đọc `pheno.total_mass` khi đã phát triển, fallback `genotype.0.total_mass()` thay vì duyệt đồ thị lại. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`**. |
| `src-tauri/src/core/simulation_loop.rs` | `@@ -219,0 +220,17 @@ fn mean_tick_time_ms(total: Duration, measured_ticks: u64) -> f64 {` | Thêm hàm pure `compute_loop_fps(elapsed, target_frame_duration)` chặn `fps` tối đa ở `1 / target_frame_duration` khi tick tính quá nhanh (+17 dòng). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/core/simulation_loop.rs` | `@@ -2136,6 +2153,8 @@ impl SimulationEngine {` | Đổi `prey_q` đọc thêm `Option<&DevelopedPhenotype>` khi tính tổng khối lượng con mồi. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`** (`main` vẫn gọi `g.0.total_mass()` trực tiếp). |
| `src-tauri/src/core/simulation_loop.rs` | `@@ -2143 +2162 @@ impl SimulationEngine {` | Tính `prey_mass_sum` qua `pheno` (`p.total_mass`) khi có `DevelopedPhenotype`, fallback `g.0.total_mass()` via `map_or_else`. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`**. |
| `src-tauri/src/core/simulation_loop.rs` | `@@ -2145,6 +2164,8 @@ impl SimulationEngine {` | Đổi `pred_q` đọc thêm `Option<&DevelopedPhenotype>` khi tính tổng khối lượng thú ăn thịt. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`**. |
| `src-tauri/src/core/simulation_loop.rs` | `@@ -2152 +2173 @@ impl SimulationEngine {` | Tính `pred_mass_sum` qua `pheno` (`p.total_mass`) khi có `DevelopedPhenotype`, fallback `g.0.total_mass()` via `map_or_else`. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`**. |
| `src-tauri/src/core/simulation_loop.rs` | `@@ -2248 +2269 @@ impl SimulationEngine {` | Gọi `compute_loop_fps(elapsed, target_frame_duration)` thay cho `1.0 / elapsed.as_secs_f64()`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/core/simulation_loop.rs` | `@@ -2255,5 +2276 @@ impl SimulationEngine {` | Gán trực tiếp `stat.fps = actual_fps;` (đã kiểm tra hữu hạn trong `compute_loop_fps`). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/core/simulation_loop.rs` | `@@ -2658,0 +2676,17 @@ mod mean_tick_time_tests {` | Thêm test `compute_loop_fps_throttles_to_target_and_scales_down_under_load` (+17 dòng). | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/evolution/genotype.rs` | `@@ -25,0 +26,10 @@ pub struct MorphologyGenotype {` | Định nghĩa component `pub struct DevelopedPhenotype { pub total_mass: f32 }` gắn lên `Agent` sau phát triển phôi (+10 dòng). | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`** (nên xem kèm quy tắc hợp đồng `CREATURE_DEVELOPMENT_CONTRACT.md` trên thân cây — `AGENT_WORKFLOW §5` ghi `DevelopedPhenotype` hiện chưa nằm trên thân cây). |
| `src-tauri/src/evolution/genotype.rs` | `@@ -27 +37,2 @@ impl MorphologyGenotype {` | Sửa docstring `total_mass()`: khối lượng chỉ gồm gốc và các đốt nối tới được từ gốc (tính một lần). | **CHƯA CÓ trên thân cây**. | Đã có docstring tương tự trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/evolution/genotype.rs` | `@@ -29,0 +41,10 @@ impl MorphologyGenotype {` | Giải thích hợp đồng reachability so với `decode_genotype` và lưu ý hiệu năng zero-heap. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`**. |
| `src-tauri/src/evolution/genotype.rs` | `@@ -31 +52,66 @@ impl MorphologyGenotype {` | Thay phép cộng tuyến tính mọi `n.mass` trong `self.nodes` bằng BFS zero-heap trên mảng stack `STACK_CAP = 64` (chỉ fallback heap khi > 64 đốt), lọc đốt mồ côi & chu trình (+66 dòng). | **CHƯA CÓ trên thân cây**. | Khác cài đặt trên `origin/main` (`ef9bc3a` dùng `HashSet` + `VecDeque` cấp phát heap, chưa dùng `STACK_CAP = 64`). |
| `src-tauri/src/evolution/genotype.rs` | `@@ -89,0 +176 @@ pub fn decode_genotype(` | Khởi tạo `let mut developed_mass = root_node.mass;` trong `decode_genotype`. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`**. |
| `src-tauri/src/evolution/genotype.rs` | `@@ -160,0 +248 @@ pub fn decode_genotype(` | Cộng dồn `developed_mass += child_node.mass;` khi sinh từng đốt con nối tới được. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`**. |
| `src-tauri/src/evolution/genotype.rs` | `@@ -213,0 +302,4 @@ pub fn decode_genotype(` | Gắn `world.entity_mut(root_entity).insert(DevelopedPhenotype { total_mass: developed_mass })` ở cuối `decode_genotype`. | **CHƯA CÓ trên thân cây**. | **CHƯA CÓ trên `origin/main`**. |
| `src-tauri/src/evolution/genotype.rs` | `@@ -215,0 +308,110 @@ pub fn decode_genotype(` | Thêm 6 unit test (`total_mass_*`, gồm `total_mass_matches_the_body_decode_genotype_spawns` kiểm tra cả `pheno.total_mass == spawned_mass`, cộng test đồ thị > 64 nút) (+110 dòng). | **CHƯA CÓ trên thân cây**. | Có 6 test `total_mass_*` trên `origin/main` (`ef9bc3a`), nhưng **bản `main` thiếu kiểm tra `DevelopedPhenotype` và nhánh > 64 nút không cần heap-fallback đặc thù này**. |
| `src-tauri/src/physics/dynamics.rs` | `@@ -3 +3,2 @@ use crate::core::ecs::{` | Import `Predator` trong `physics/dynamics.rs`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/physics/dynamics.rs` | `@@ -50 +51,2 @@ pub fn resolve_joints_system(` | Dùng `axis_val.try_normalize().unwrap_or(Vec3::Y)` trong `resolve_joints_system` để tránh `NaN` khi trục khớp suy biến về vector 0. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/physics/dynamics.rs` | `@@ -93,0 +96 @@ pub fn integrate_physics_system(` | Thêm `predator_query: Query<&Predator>` vào `integrate_physics_system`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/src/physics/dynamics.rs` | `@@ -153,2 +156,2 @@ pub fn integrate_physics_system(` | Chặn chuyển động khi cạn năng lượng cho **cả `Predator` lẫn `Prey`** (`is_agent = prey_query.get(e).is_ok() OR predator_query.get(e).is_ok()`) thay vì chỉ chặn `Prey`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/tests/physics_tests.rs` | `@@ -5,2 +5,2 @@ use anima_engine_lib::core::ecs::{` | Import `ParentAgent` trong `src-tauri/tests/physics_tests.rs`. | **CHƯA CÓ trên thân cây**. | Đã có trên `origin/main` (qua `ef9bc3a`). |
| `src-tauri/tests/physics_tests.rs` | `@@ -676,0 +677,163 @@ fn test_decoupled_systems_zero_allocation() {` | Thêm 2 test tích hợp: `test_predator_energy_depletion_stops_movement` và `test_zero_joint_axis_no_nan` (+163 dòng). | **CHƯA CÓ trên thân cây**. | Đã có cả 2 test trên `origin/main` (dòng 679 và 756, qua `ef9bc3a`). |

---

## 4. Toàn bộ 102 nhánh remote (`git ls-remote --heads origin`)

### 4.1 Nhóm A — 20 nhánh có `ahead > 0` so với thân cây (hoặc không phải tổ tiên của thân cây)

| Nhánh (`origin/<name>`) | SHA (10 ký tự) | Commit cuối | vs `main` (`-b/+a`) | vs `trunk` (`-b/+a`) | `cherry` vs `trunk` | Phân loại | Bằng chứng lệnh & ghi chú |
|---|---|---|---|---|---|---|---|
| `claude/funny-thompson-jj42fe` | `a51edef22d` | `2026-11-05 04:24:58 +0000` | `-0/+120` | `-1/+45` | `+42/-0` | `REBASE` | Nhánh thử nghiệm tích hợp trước HT-C trên đỉnh `main` + `#38–#41` (`b1c27df`), port `#31` (`3f9f054`), cập nhật `IPC_CONTRACT.md` (`0c5665a`) và sinh hai file prompt swarm (`a82b996` — nội dung 2 file swarm đã được chép vào thân cây trong `491ec61`, nên `git rev-list` báo `-1/+45`). Nội dung sống đi theo lộ trình T2 / G1 / port `#31` có kiểm soát thay vì gộp thẳng. |
| `claude/busy-hamilton-mn42mw` | `d7b8169401` | `2026-11-05 04:23:15 +0000` | `-0/+5` | `-109/+38` | `+37/-0` | `REBASE` | Đứng trên `origin/main` + 5 commit chưa lên `main` hay thân cây (`0cd4279` tách khối 2026-08-16 khỏi tick mặc định, `4df672d` xóa Research Dashboard, `df05d59` xóa bằng chứng Map 2 bịa & trả `test:e2e`, `600c814` khôi phục docs gốc, `d7b8169` dọn advisory). Cần rà kỹ đối chiếu với kế hoạch T2/HT-C. |
| `fix/birth-death-zero-alloc` | `2c5423b4df` | `2026-11-01 02:20:34 +0700` | `-0/+5` | `-109/+38` | `+37/-0` | `REBASE` | Đầu PR `#41` (`+38` so với thân cây, thuộc Wave 2 `B2`). |
| `fix/ci-gates-green` | `78b2e85872` | `2026-10-31 22:28:59 +0700` | `-0/+3` | `-109/+36` | `+35/-0` | `REBASE` | Đầu PR `#40` (`+36` so với thân cây, đích nhập chuỗi T2 sau T1). |
| `fix/declare-simrng-draw-order` | `03b331f30d` | `2026-10-31 19:56:06 +0700` | `-0/+2` | `-109/+35` | `+34/-0` | `REBASE` | Đầu PR `#39` (`+35` so với thân cây, chuỗi T2). |
| `fix/live-world-restores-its-spatial-grid` | `fa938c5428` | `2026-10-31 16:10:07 +0700` | `-0/+1` | `-109/+34` | `+33/-0` | `REBASE` | Đầu PR `#38` (`+34` so với thân cây, chuỗi T2). |
| `main` | `a43872c5e1` | `2026-10-22 00:33:04 +0700` | `-0/+0` | `-109/+33` | `+32/-0` | `REBASE` | Nhánh `origin/main` hiện tại (`33` commit phía trước thân cây, `109` phía sau); nằm sẵn làm nền của chuỗi `#38–#41`, sẽ hội tụ vào thân cây qua T2. |
| `fix/ht0-5c-sim-semantics` | `c7829689ec` | `2026-10-17 17:56:39 +0700` | `-20/+6` | `-109/+19` | `+18/-0` | `REBASE` | Đầu PR `#37` (6 commit HT0.5c trên `1b3e7ef`, chi tiết hunk ở Mục 3). |
| `docs/ht0-7-remeasure-main` | `f0a33cb464` | `2026-10-17 17:21:27 +0700` | `-20/+2` | `-109/+15` | `+14/-0` | `STALE-DOC` | Đầu PR `#34` (đo lại `main` tại `1b3e7ef` đã lỗi thời). |
| `fix/ht0-5a-sensor-raycast` | `3560faa6c0` | `2026-10-17 17:40:25 +0700` | `-20/+2` | `-109/+15` | `+14/-0` | `REBASE` | Đầu PR `#35` (2 commit HT0.5a trên `1b3e7ef`, chi tiết hunk ở Mục 3). |
| `fix/ht0-5b-landscape-dispose` | `a5b7960d6e` | `2026-10-17 17:43:34 +0700` | `-20/+2` | `-109/+15` | `+14/-0` | `REBASE` | Đầu PR `#36` (2 commit HT0.5b trên `1b3e7ef`, chi tiết hunk ở Mục 3). |
| `fix/main-ci-green` | `4aed9a0241` | `2026-10-16 17:06:59 +0700` | `-21/+0` | `-109/+12` | `+12/-0` | `SUPERSEDED` | Tổ tiên trực tiếp của `origin/main` (`git merge-base --is-ancestor origin/fix/main-ci-green origin/main` = exit `0`, merged ở PR `#33` `1b3e7ef`); toàn bộ chuỗi đã nằm sẵn trong `origin/main` / `#38–#40` (`REBASE`). |
| `feature-runtime-unblock` | `e4e89ab46c` | `2026-07-28 20:32:55 +0700` | `-163/+7` | `-239/+7` | `+6/-1` | `REBASE` | 7 commit = toàn bộ 6 commit của PR `#30` (`git cherry` `+6`) + `e4e89ab` (`feat(lab): headless runner...`) đã được đưa lên thân cây & `main` dưới dạng `df85956` (`git cherry` `-1`). Công việc còn sống trùng hoàn toàn với phần chưa lên thân cây của PR `#30`. |
| `fix/document-the-wgpu-registry-leak` | `0f33998fc9` | `2026-07-28 18:41:55 +0700` | `-163/+6` | `-239/+6` | `+6/-0` | `REBASE` | Đầu PR `#30` (chỉ có 1 commit `6035608` đã lên thân cây qua `75dfef4`, 5 commit còn lại chưa lên thân cây). |
| `feat/oss-072-mrca` | `b14e836c42` | `2026-07-29 13:56:03 +0700` | `-163/+4` | `-239/+4` | `+4/-0` | `REBASE` | Đầu PR `#31` (4 commit OSS-072 chưa lên thân cây hay `main`). |
| `codex/open-source-release` | `819ad2dfc3` | `2026-08-08 12:11:42 +0700` | `-163/+3` | `-239/+3` | `+3/-0` | `SUPERSEDED` | Nhánh PR `#32` đã đóng và được thay thế hoàn toàn bởi `#33 fix/main-ci-green` (`1b3e7ef`) trên `origin/main`: `git cherry origin/main` khớp `819ad2d` (`-`), `2344afd` ứng `043930c` trên cả `main` lẫn thân cây (`491ec61`), `096b16a` tương đương `3fbeaf4` trên `main` / `#38–#40` (`REBASE`). |
| `feature-anima-menagerie` | `a0d5f9f531` | `2026-07-29 16:32:05 +0700` | `-242/+2` | `-318/+2` | `+2/-0` | `SUPERSEDED` | Cả 2 commit (`a0d5f9f` đặt `DEFAULT_GRID_DIM = 256` ở 3 nơi; `2652478` sửa 3 gate `check_test_targets` / frontend) đều đã được làm lại và hợp nhất vào thân cây qua `4b6258d`, `c52b127`, `250e03d`, `bef7768` (chứng minh qua `STATE_OF_THE_PROJECT.md §3.0` và đối chiếu code trên thân cây). |
| `wip/2026-08-16-map2-tier5` | `84fece1f63` | `2026-10-16 16:29:46 +0700` | `-33/+2` | `-109/+2` | `+2/-0` | `SUPERSEDED` | Bản sao lưu working tree ngày 2026-08-16 trên đỉnh `389de4b`; toàn bộ mã khối 2026-08-16 đó đã đi qua chuỗi `main` (`606bf0f..a43872c`, tracked ở `main` / `#38–#40` `REBASE`), còn bản dọn phần bịa/dashboard nằm ở `claude/busy-hamilton-mn42mw` (`REBASE`). Không có lý do rebase nhánh WIP thô này. |
| `codex/simulation-bottlenecks` | `540d83d5c4` | `2026-07-29 01:43:26 +0700` | `-143/+1` | `-219/+1` | `+1/-0` | `SUPERSEDED` | Một commit duy nhất `540d83d` (`perf(sim): four bottlenecks removed...`) có diff trùng commit đỉnh `0f33998` của PR `#30` (`git diff 540d83d^..540d83d` khớp patch với `0f33998^..0f33998`); nhánh con sống được theo dõi qua PR `#30` (`REBASE`). |
| `feat/oss-071b-live-tracker` | `389de4bc0f` | `2026-08-15 10:05:57 +0700` | `-33/+1` | `-109/+1` | `+1/-0` | `SUPERSEDED` | Commit gộp ngày 2026-08-15 (`389de4b`) đã được chẻ thành các PR `#35`, `#36`, `#37` (HT0.5a/b/c) và đưa đối ứng lên `main` (`ef9bc3a`, `a18c19f`); chỉ thêm việc xóa alias `@react-three/fiber` trong `tsconfig.json` (đã giải quyết trên `origin/main`) và sửa conflict marker cũ ở `allocator.rs`. Không có công việc độc lập cần rebase riêng ngoài các hunk đang theo dõi ở PR `#35–#37` (`REBASE`). |

```bash
$ git merge-base --is-ancestor origin/fix/main-ci-green origin/main && echo exit_0
exit_0

$ git cherry -v origin/feat/ecological-selection origin/feature-anima-menagerie
+ 265247870ed6e0778640cadc9a0398483307485b fix(gates): three gates were passing without looking -- make them look
+ a0d5f9f531eadadea339ad248b8107ca2dbf560c fix(contract): DEFAULT_GRID_DIM was 128 and nothing read it, so nothing went red

$ git cherry -v origin/feat/ecological-selection origin/feature-runtime-unblock
+ b6abc9150926eeaeef15455f545f999d097b2cda docs(perf): the rest of the leak is wgpu's registry, and the GPU path is the default
+ 6035608e59627f53bbbf60f9cabe3f65db931f3e feat(ml): default the learner to CPU, because the GPU path grows without bound
+ 82c1401eabde2ccfa5241d904ad55f35c3bee50e feat(world): the animals were standing still, and there were not many of them
+ 9e68f386483bb601d6efcc0bc56f788e8f69b250 feat(world): six more species, in the biomes that had none
+ 6e201982be8df034a59b6fa807cca550baf2f971 feat(map): give the simulated creatures bodies, and stop planting them in rows
+ 0f33998fc99af56d81f96c058079fad659c92ad2 perf(sim): four bottlenecks removed, and none of them was the bottleneck
- e4e89ab46ca52c87ab5df8944028e3594003b585 feat(lab): a headless runner for the experiment manifests, and gates that wait for the engine
```

### 4.2 Nhóm B — Toàn bộ 82 nhánh có `ahead = 0` (`81` `SUPERSEDED` + `1` `TRUNK`)

Mọi nhánh dưới đây đều đạt **cả hai điều kiện**: `git rev-list --count origin/feat/ecological-selection..origin/<nhánh>` = **`0`** và `git merge-base --is-ancestor origin/<nhánh> origin/feat/ecological-selection` thoát **`0`** (riêng `feat/ecological-selection` là chính nhánh thân cây chuẩn `TRUNK`; 81 nhánh còn lại là tổ tiên nghiêm ngặt `SUPERSEDED`, `git cherry` rỗng).

```bash
$ for b in <82 zero-ahead branches>; do git merge-base --is-ancestor "origin/$b" origin/feat/ecological-selection || echo FAIL; done
# (đã kiểm tự động 82/82 nhánh ngày 2026-11-14: 0 FAIL, exit 0 toàn bộ)
```

| STT | Nhánh (`origin/<name>`) | SHA (10 ký tự) | Commit cuối | vs `main` (`-b/+a`) | vs `trunk` (`-b/+a`) | `cherry` vs `trunk` | Phân loại & lệnh hỗ trợ |
|---|---|---|---|---|---|---|---|
| 1 | `codex/anima-lab-live-model` | `af1e469535` | `2026-07-29 04:44:38 +0700` | `-129/+0` | `-205/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 2 | `codex/brain-budget-gate` | `0fee2880c4` | `2026-07-29 04:23:11 +0700` | `-131/+0` | `-207/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 3 | `codex/brain-output-isolation` | `7f0ab992e7` | `2026-07-30 16:23:06 +0700` | `-49/+0` | `-125/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 4 | `codex/build-provenance-identity` | `48798a4bcf` | `2026-07-30 15:06:50 +0700` | `-55/+0` | `-131/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 5 | `codex/checkpoint-learner-state` | `38fd86731a` | `2026-07-30 09:00:30 +0700` | `-83/+0` | `-159/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 6 | `codex/combat-centroid-index` | `fecd8b1d12` | `2026-07-29 02:30:22 +0700` | `-142/+0` | `-218/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 7 | `codex/desktop-release-capture` | `b6328f13d6` | `2026-07-29 19:26:40 +0700` | `-98/+0` | `-174/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 8 | `codex/environmental-stress-gate` | `b821d4e1af` | `2026-07-29 03:54:31 +0700` | `-135/+0` | `-211/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 9 | `codex/experiment-fault-boundaries` | `52fb5cb87e` | `2026-07-29 13:24:37 +0700` | `-115/+0` | `-191/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 10 | `codex/finite-inference-boundary` | `26b5887bd2` | `2026-07-30 17:10:12 +0700` | `-45/+0` | `-121/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 11 | `codex/finite-learning-boundary` | `573b6e1f36` | `2026-07-30 17:38:08 +0700` | `-43/+0` | `-119/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 12 | `codex/honest-confidence-intervals` | `7b522f61b8` | `2026-07-29 18:16:35 +0700` | `-100/+0` | `-176/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 13 | `codex/honest-fork-deltas` | `bbd41b0b7c` | `2026-07-29 01:42:51 +0700` | `-144/+0` | `-220/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 14 | `codex/honest-singleton-statistics` | `9f881a1666` | `2026-07-29 17:37:47 +0700` | `-102/+0` | `-178/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 15 | `codex/hrrl-transition-backpressure` | `7de5c9313c` | `2026-07-29 21:01:15 +0700` | `-96/+0` | `-172/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 16 | `codex/inference-backpressure` | `579d864388` | `2026-07-29 02:51:23 +0700` | `-140/+0` | `-216/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 17 | `codex/inference-transition-alignment` | `7b5f826823` | `2026-07-30 19:14:36 +0700` | `-39/+0` | `-115/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 18 | `codex/json-safe-failure-report` | `f52749b192` | `2026-07-29 01:31:16 +0700` | `-146/+0` | `-222/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 19 | `codex/learner-model-channel` | `b6a3dd76f9` | `2026-07-29 22:27:00 +0700` | `-92/+0` | `-168/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 20 | `codex/learner-seed-coherence` | `7b728e02ee` | `2026-07-29 21:25:54 +0700` | `-94/+0` | `-170/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 21 | `codex/lifetime-transition-correctness` | `4a5b7b1dc8` | `2026-07-30 18:25:12 +0700` | `-41/+0` | `-117/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 22 | `codex/lineage-stress-gate` | `7d8cdf00a9` | `2026-07-29 04:07:58 +0700` | `-133/+0` | `-209/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 23 | `codex/linear-segment-index` | `62b8b2a0ad` | `2026-07-29 19:26:53 +0700` | `-97/+0` | `-173/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 24 | `codex/live-inference-invariant` | `af3811f576` | `2026-07-29 03:34:18 +0700` | `-137/+0` | `-213/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 25 | `codex/migration-backpressure` | `49ca7323f9` | `2026-07-29 05:50:14 +0700` | `-125/+0` | `-201/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 26 | `codex/migration-enqueue-atomicity` | `03b93ded37` | `2026-07-29 05:15:32 +0700` | `-127/+0` | `-203/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 27 | `codex/migration-payload-validation` | `5de4451910` | `2026-07-29 08:00:20 +0700` | `-119/+0` | `-195/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 28 | `codex/migration-transition-boundary` | `a7b269ad03` | `2026-07-30 19:45:52 +0700` | `-37/+0` | `-113/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 29 | `codex/migration-unbounded-tree` | `63893f7cd0` | `2026-07-29 06:26:21 +0700` | `-121/+0` | `-197/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 30 | `codex/networking-test-gate` | `b48a43b328` | `2026-07-29 01:18:07 +0700` | `-148/+0` | `-224/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 31 | `codex/profile-ecology-bottleneck` | `62b8b2a0ad` | `2026-07-29 19:26:53 +0700` | `-97/+0` | `-173/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 32 | `codex/restore-seed-coherence` | `35b1d52f3f` | `2026-07-29 23:12:48 +0700` | `-88/+0` | `-164/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 33 | `codex/resume-identity-coherence` | `362d6afedb` | `2026-07-30 00:09:53 +0700` | `-85/+0` | `-161/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 34 | `codex/runtime-counter-safety` | `902520335e` | `2026-07-30 10:39:31 +0700` | `-77/+0` | `-153/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 35 | `codex/scientific-hardening` | `662a718cc9` | `2026-07-29 00:18:31 +0700` | `-151/+0` | `-227/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 36 | `codex/shared-brain-row-stride` | `93e8ec56dc` | `2026-07-30 16:40:56 +0700` | `-47/+0` | `-123/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 37 | `codex/snapshot-counter-guards` | `f0a1062833` | `2026-07-30 09:35:09 +0700` | `-81/+0` | `-157/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 38 | `codex/snapshot-payload-validation` | `3ed8d3cecf` | `2026-07-29 09:35:08 +0700` | `-117/+0` | `-193/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 39 | `codex/strict-experiment-manifest` | `75602b9367` | `2026-07-29 01:10:38 +0700` | `-150/+0` | `-226/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 40 | `codex/test-typecheck-node-types` | `ef3dff6412` | `2026-07-29 05:58:44 +0700` | `-123/+0` | `-199/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 41 | `codex/unified-build-provenance` | `7d3bffffb3` | `2026-07-30 15:29:26 +0700` | `-53/+0` | `-129/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 42 | `docs/csp-boot-evidence` | `bec98d9a9d` | `2026-07-27 22:02:39 +0700` | `-176/+0` | `-252/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 43 | `docs/open-source-review-2026-07-26` | `66e82e7a95` | `2026-07-26 21:39:22 +0700` | `-271/+0` | `-347/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 44 | `docs/oss-010-status-reconcile` | `7cda83a5d9` | `2026-07-26 23:01:40 +0700` | `-262/+0` | `-338/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 45 | `docs/state-after-pr21` | `94506d4f06` | `2026-07-27 20:59:24 +0700` | `-180/+0` | `-256/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 46 | `docs/tick-capture-baseline` | `44ecc75a05` | `2026-07-27 21:37:52 +0700` | `-178/+0` | `-254/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 47 | `feat/autostart-a-fresh-world` | `f9f12b310b` | `2026-07-28 00:46:49 +0700` | `-167/+0` | `-243/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 48 | `feat/ecological-selection` | `491ec61e14` | `2026-11-14 09:13:27 +0700` | `-33/+109` | `-0/+0` | `+0/-0` | `TRUNK` (nhánh nền đối chiếu chuẩn) |
| 49 | `feat/live-agents-on-the-shared-world` | `37ce90f8e8` | `2026-07-28 00:43:38 +0700` | `-168/+0` | `-244/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 50 | `feat/measurable-thousand-agent-release` | `539c9cc99b` | `2026-07-27 23:56:01 +0700` | `-169/+0` | `-245/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 51 | `feat/oss-010-criterion-benches` | `bb4d555551` | `2026-07-26 22:37:01 +0700` | `-268/+0` | `-344/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 52 | `feat/oss-070-newick-export` | `104f67031e` | `2026-07-26 23:48:06 +0700` | `-257/+0` | `-333/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 53 | `feat/oss-071-lineage-simplify` | `9183eb647c` | `2026-07-27 00:45:41 +0700` | `-250/+0` | `-326/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 54 | `feat/persist-dormant-cohorts` | `750ec32d23` | `2026-07-26 09:50:40 +0700` | `-292/+0` | `-368/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 55 | `feature-anima-completion` | `0f5b4d367a` | `2026-07-27 20:37:01 +0700` | `-182/+0` | `-258/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 56 | `feature-counter-exhaustion` | `5828889ed6` | `2026-07-30 10:03:41 +0700` | `-79/+0` | `-155/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 57 | `feature-density-map` | `e5c645311d` | `2026-07-29 18:16:43 +0700` | `-99/+0` | `-175/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 58 | `feature-evolution-checkpoint-provenance` | `80eb075537` | `2026-07-30 12:07:43 +0700` | `-69/+0` | `-145/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 59 | `feature-experiment-budget-validity` | `89b8197811` | `2026-07-30 16:01:39 +0700` | `-51/+0` | `-127/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 60 | `feature-finite-aggregate-statistics` | `4f1cb83ff4` | `2026-07-29 16:46:18 +0700` | `-104/+0` | `-180/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 61 | `feature-headless-combat-telemetry` | `6d1123d620` | `2026-07-30 11:41:51 +0700` | `-71/+0` | `-147/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 62 | `feature-lab-exit-contract` | `84b1760748` | `2026-07-28 23:21:59 +0700` | `-154/+0` | `-230/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 63 | `feature-lab-manifest-boundary` | `58b2677510` | `2026-07-29 14:47:29 +0700` | `-111/+0` | `-187/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 64 | `feature-lab-stream-output` | `cf881b3bc4` | `2026-07-29 15:03:13 +0700` | `-109/+0` | `-185/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 65 | `feature-manual-migration-backpressure` | `ea29d155db` | `2026-07-30 11:31:19 +0700` | `-73/+0` | `-149/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 66 | `feature-map-gate-alignment` | `65033b556a` | `2026-07-29 14:18:38 +0700` | `-113/+0` | `-189/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 67 | `feature-meta-ai-network-failure` | `8f757974be` | `2026-07-30 12:30:39 +0700` | `-67/+0` | `-143/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 68 | `feature-meta-ai-response-limit` | `a0a551cce2` | `2026-07-30 13:30:49 +0700` | `-61/+0` | `-137/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 69 | `feature-meta-ai-strict-response` | `ef1c28e74b` | `2026-07-30 12:52:37 +0700` | `-65/+0` | `-141/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 70 | `feature-provenance-run-identity` | `f5c82d4d85` | `2026-07-30 14:35:08 +0700` | `-57/+0` | `-133/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 71 | `feature-runtime-stability` | `ce558b22b3` | `2026-07-28 22:59:31 +0700` | `-156/+0` | `-232/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 72 | `feature-save-backpressure` | `de5738d4ae` | `2026-07-30 11:02:44 +0700` | `-75/+0` | `-151/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 73 | `feature-scientific-metric-union` | `fdf4970b48` | `2026-07-29 15:21:47 +0700` | `-107/+0` | `-183/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 74 | `feature-scientific-selection-integrity` | `1c1f8e328f` | `2026-07-28 10:31:20 +0700` | `-163/+0` | `-239/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 75 | `feature-tick-capture-clock-origin` | `80c00b6604` | `2026-07-30 13:11:28 +0700` | `-63/+0` | `-139/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 76 | `feature-unique-observable-contract` | `41c129ee5f` | `2026-07-30 14:09:53 +0700` | `-59/+0` | `-135/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 77 | `feature-zero-alloc-gate-isolation` | `c6e8f429fa` | `2026-07-29 00:18:11 +0700` | `-152/+0` | `-228/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 78 | `feature/runtime-integration` | `df85956004` | `2026-07-28 20:34:11 +0700` | `-161/+0` | `-237/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 79 | `feature/runtime-next-p0` | `391d23d185` | `2026-07-28 22:00:43 +0700` | `-158/+0` | `-234/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 80 | `fix/bound-the-inference-pool` | `c1c28c3556` | `2026-07-28 10:14:42 +0700` | `-164/+0` | `-240/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 81 | `fix/dev-url-binds-one-address` | `0a5f614a63` | `2026-07-27 22:12:39 +0700` | `-175/+0` | `-251/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
| 82 | `platform/g0-g2-shared-world` | `b99bd08f1f` | `2026-07-25 21:11:42 +0700` | `-300/+0` | `-376/+0` | `+0/-0` | `SUPERSEDED` (`merge-base --is-ancestor` = 0) |
