---
title: Chỉ mục working set theo feature (docs/ai)
status: active
owner: maintainers
last_reviewed: 2026-11-17
---

# Working set theo feature

`docs/ai/` giữ tài liệu **vòng đời của từng feature**: requirements → design → planning →
implementation → testing → deployment → monitoring, cùng các bản giao việc cho agent. Đây là
**working set**, không phải nguồn sự thật: khi một quyết định được chấp nhận, nguồn chuẩn là
[reference](../reference/README.md) và [ADR](../decisions/README.md) (quy tắc 7 của
[chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md)). Khi thân một tài liệu ở đây mâu
thuẫn với [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md), tài liệu trạng thái thắng.

Trạng thái trong bảng lấy từ front matter của từng file; đối chiếu lần cuối ngày 2026-11-17, khi mười bảy
tài liệu được đổi trạng thái cho khớp với việc đã merge (xem nhật ký cùng ngày trong `TODO.md`). `README.md` trong mỗi thư
mục pha (`requirements/`, `design/`…) là **template thượng nguồn** của quy trình lifecycle, không
phải tài liệu dự án.

## Feature

| Feature | Requirements | Design | Planning | Implementation | Testing | Deployment / Monitoring |
|---|---|---|---|---|---|---|
| **Alternate evolution & World Lab** (07-24) | [`proposed`](requirements/2026-07-24-feature-alternate-evolution-world-lab.md) | [`proposed`](design/2026-07-24-feature-alternate-evolution-world-lab.md) | [`active`](planning/2026-07-24-feature-alternate-evolution-world-lab.md) | [`active`](implementation/2026-07-24-feature-alternate-evolution-world-lab.md) | [`active`](testing/2026-07-24-feature-alternate-evolution-world-lab.md) | [`proposed`](deployment/2026-07-24-feature-alternate-evolution-world-lab.md) / [`proposed`](monitoring/2026-07-24-feature-alternate-evolution-world-lab.md) |
| **Completion & Hardening** (07-27) | [`active`](requirements/2026-07-27-feature-anima-completion.md) | [`active`](design/2026-07-27-feature-anima-completion.md) | [`active`](planning/2026-07-27-feature-anima-completion.md) | [`active`](implementation/2026-07-27-feature-anima-completion.md) | [`active`](testing/2026-07-27-feature-anima-completion.md) | [`active`](deployment/2026-07-27-feature-anima-completion.md) / [`active`](monitoring/2026-07-27-feature-anima-completion.md) |
| **Thí nghiệm E2** — não tiến hoá làm mặc định (07-27) | [`completed`](requirements/2026-07-27-experiment-e2-evolved-brain-default.md) | [`completed`](design/2026-07-27-experiment-e2-evolved-brain-default.md) | [`completed`](planning/2026-07-27-experiment-e2-evolved-brain-default.md) | [`completed`](implementation/2026-07-27-experiment-e2-evolved-brain-default.md) | [`completed`](testing/2026-07-27-experiment-e2-evolved-brain-default.md) | — |
| **Anima Menagerie** (08-04; chỉ có thiết kế, chưa lên kế hoạch, chưa có mã) | — | [`proposed`](design/2026-08-04-feature-anima-menagerie.md) | — | — | — | — |
| **Đóng vòng sinh thái** (08-25) | [`active`](requirements/2026-08-25-feature-ecological-selection.md) | [`active`](design/2026-08-25-feature-ecological-selection.md) | [`active`](planning/2026-08-25-feature-ecological-selection.md) | [`active`](implementation/2026-08-25-feature-ecological-selection.md) | [`active`](testing/2026-08-25-feature-ecological-selection.md) | — |
| **Thế giới sống** (08-25) | [`active`](requirements/2026-08-25-feature-living-world.md) | ba design con, bên dưới | [`active`](planning/2026-08-25-feature-living-world.md) | [`active`](implementation/2026-08-25-feature-living-world-tasks.md) | [`active`](testing/2026-08-25-feature-living-world.md) | — |
| ↳ Từ genome tới mesh | | [`proposed`](design/2026-08-25-feature-genome-to-mesh.md) | | | | |
| ↳ Animation sinh vật | | [`proposed`](design/2026-08-25-feature-creature-animation.md) | | | | |
| ↳ Tính toán phân tán theo thiết bị | | [`proposed`](design/2026-08-25-feature-federated-compute.md) | | | | |
| **Thí nghiệm E3** — gradient năng suất và khối lượng cơ thể (08-26) | — | — | [`proposed`](planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md) | — | — | — |
| **Thí nghiệm E-SEL1** — khan hiếm thức ăn và khối lượng cơ thể; tiền đăng ký đóng băng rồi đóng tại smoke, không có thí nghiệm (11-16 → 11-17) | — | — | [`completed`](planning/2026-11-16-experiment-e-sel1-food-scarcity-body-mass.md) | — | — | — |
| **Thế giới nhìn rõ** — sinh vật và bản đồ đẹp, nhẹ, mượt (11-16; yêu cầu và thiết kế nằm trong §1 và §3 của kế hoạch, không có file riêng) | — | — | [`active`](planning/2026-11-16-feature-readable-world.md) | — | [`completed`](testing/2026-11-16-rw-c3-flora-lod.md) | — |
| **Thế giới thật và thân sinh vật** — bản đồ là thế giới mô phỏng, bán hiện thực; thân liền mọc từ hình thái (11-19) | — | [`proposed`](design/2026-11-19-feature-real-world-and-bodies.md) | — | — | — | — |

## Bản giao việc, bản ghi phiên và đo lường

Bản ghi phiên (`kind: agent-goal`, `kind: review-findings`) là kế hoạch **tại thời điểm đó**; thân
của chúng không được sửa cho khớp hiện tại.

| Ngày | Tài liệu | Loại | Trạng thái |
|---|---|---|---|
| 2026-07-24 | [Overnight goal — AE1–AE2](planning/2026-07-24-claude-overnight-goal-ae1-ae2.md) | agent-goal | `completed` |
| 2026-07-25 | [Overnight goal — AE2.5](planning/2026-07-25-claude-overnight-goal-ae25.md) | agent-goal | `completed` |
| 2026-07-25 | [Overnight goal — AE3](planning/2026-07-25-claude-overnight-goal-ae3.md) | agent-goal | `completed` |
| 2026-07-25 | [Overnight goal — G0–G4 remediation](planning/2026-07-25-claude-overnight-goal-g0-g4-remediation.md) | agent-goal | `completed` |
| 2026-07-27 | [Supervisor findings — Completion pass](planning/2026-07-27-review-anima-completion-supervisor-findings.md) | review-findings | `completed` |
| 2026-08-25 | [Bản giao việc — Đóng vòng sinh thái](planning/2026-08-25-agent-goal-ecological-selection.md) | brief | `active` |
| 2026-08-25 | [Agent goal — Thế giới sống](planning/2026-08-25-agent-goal-living-world.md) | agent-goal | `active` |
| 2026-10-31 | [Brief cho Antigravity — HT-A và HT-B](planning/2026-10-31-antigravity-brief.md) | brief | `active` |
| 2026-10-31 | [Brief cho Antigravity — tách crate con (S0–S5)](planning/2026-10-31-antigravity-brief-crate-split.md) | brief | `active` |
| 2026-11-01 | [Bề mặt hợp nhất giữa hai cây](planning/2026-11-01-merge-surface-measurement.md) | đo | `completed` |
| 2026-11-05 | [Kế hoạch tổng cho swarm Antigravity](planning/2026-11-05-swarm-master-plan.md) — mọi thẻ, chia đợt | kế hoạch thẻ | `active` |
| 2026-11-05 | [Prompt cho swarm Antigravity](planning/2026-11-05-swarm-prompts.md) — lead, luật swarm, verifier, từng thẻ | prompt | `active` |
| 2026-11-15 | [Swarm ba máy qua Google Drive](planning/2026-11-15-three-machine-drive-swarm.md) — giao thức BLOCK D, prompt lead/trạm/tiếp tục, khối DECISIONS Đợt 2 | prompt | `proposed` |
| 2026-11-14 | [Kiểm toán PR mở và nhánh remote ngày 2026-11-14](monitoring/pr-branch-audit-2026-11-14.md) — 10 PR mở, 102 nhánh remote và mọi hunk chưa lên thân cây của PR #35–#37 | monitoring | `completed` |
| 2026-11-14 | [Triage báo cáo AUDIT_REPORT.md trên thân cây ngày 2026-11-14](monitoring/audit-triage-2026-11-14.md) — đối chiếu 2 bản sao trên `main`, xác minh 27 mục và đề xuất 21 thẻ `§6` | monitoring | `completed` |
| 2026-11-16 | [Thế giới nhìn rõ trên ba máy](planning/2026-11-16-readable-world-three-machines.md) — lane, file sở hữu, thẻ RW-A1 … RW-C5 cho `anima-mac`, `anima-case`, `anima-dell` | kế hoạch thẻ | `active` |
| 2026-11-17 | [Chẩn đoán sau E-SEL1](planning/2026-11-17-esel1-scarcity-diagnosis.md) — cái gì giới hạn quần thể con mồi (đồng cỏ không cạn vì trần sinh sản 80, nước giới hạn), yếu tố khan hiếm nào dùng được, QĐ-SEL1b | đo | `completed` |
| 2026-11-17 | [Thẻ S6 — đo thăm dò cho SEL2](planning/2026-11-17-s6-sel2-exploration.md) — nâng trần sinh sản (`live.prey_cap`) và hạ sức chứa (`live.resource_capacity_scale`): không cấu hình nào vừa đưa standing crop vào 0,4–0,6 vừa giữ quần thể ≥ 5 ở mọi seed | đo | `active` |
| 2026-11-18 | [Giao việc cho Gemini trên Windows (vai `agent-win`) — R0 trước, R1c sau](planning/2026-11-18-gemini-worker-win-handoff.md) — prompt R0 để dán, khối đầu ghép vào prompt R1c, khác biệt khi worker là Gemini | prompt | `active` |
| 2026-11-17 | [Giao việc cho `agent-win` — thẻ R1c, rồi R0](planning/2026-11-17-agent-win-handoff-r1c.md) — prompt để dán vào phiên Windows, dòng uỷ quyền đã soạn cho R1c, lý do chọn thẻ | prompt | `active` |
| 2026-11-18 | [Thí nghiệm rẽ luật và thế giới nhiều máy](planning/2026-11-18-research-law-fork-and-multi-machine.md) — đối chiếu hai báo cáo nghiên cứu (câu 6, câu 7) với mã; đề xuất QĐ-AE-C (bốn nhánh đối chứng) và khuyến nghị cho QĐ-M7 | đối chiếu | `active` |
| 2026-11-18 | [Nâng Burn 0.22 và tất định số học](planning/2026-11-18-research-burn-and-determinism.md) — đối chiếu hai báo cáo nghiên cứu với mã; quyết định QĐ-R1c-a (schema 8, learner khởi tạo lại) và QĐ-R1c-b (`burn-flex`); một phương án cho QĐ-M6 | đối chiếu | `active` |
| 2026-11-18 | [Đề xuất làm lại SEL2](planning/2026-11-18-sel2-redesign-proposal.md) — từ hai báo cáo nghiên cứu: tách nguyên nhân chết (S9), khoá phần cây dự trữ (S10), đo thăm dò lần hai (SEL2-E), đối chứng drift (SEL2-D), endpoint không bị sàn che; chủ dự án duyệt 2026-11-18 | đề xuất | `active` |
| 2026-11-17 | [Đánh giá dự án theo mười hai tiêu chí](monitoring/2026-11-17-project-assessment.md) — bảng điểm sau lượt rà soát toàn bộ tài liệu; điểm là nhận định, không phải phép đo | đánh giá | `completed` |
| 2026-11-18 | [Thẻ S9 — tách nguyên nhân chết](planning/2026-11-18-s9-death-causes.md) — `live.deaths_starvation` và `live.deaths_dehydration`; đo lại các ô hệ số 0,5 và 0,7 của lưới S6: khát giết 71–80 % số cá thể chết ở cả bốn ô | đo | `completed` |
| 2026-11-18 | [Thẻ S10 — khoá `live.plant_reserve`](planning/2026-11-18-s10-plant-reserve.md) — phần cây con vật không ăn được (`ρ·r_max`), cho cả gặm cỏ sống lẫn quần thể gộp; khoá vắng trùng quỹ đạo commit trước; tỉ số crop của S6 có sàn ≈ ρ nên đề xuất đọc phần cỏ ăn được; khôi phục checkpoint ở thế giới nền S6 lệch từ trước S10 | thẻ | `completed` |
| 2026-11-18 | [Thẻ SEL2-E — đo thăm dò lần hai và ba phép đo tiếp theo](planning/2026-11-18-sel2e-exploration.md) — kết quả âm: không ô nào khả thi; khi khan hiếm, quần thể dao động bùng nổ và sụp đổ rồi tuyệt chủng vì sinh sản dừng ở mật độ thấp | đo | `completed` |
| 2026-11-18 | [Thẻ SEL2-D — đối chứng drift trung tính khoá bước](planning/2026-11-18-sel2-d-drift-control.md) — `live.drift_control` và `live.drift_reset_ticks` (`LIVE_KEYS.len() = 24`); ghi/phát lại sinh chết từng tick với tách `P_gen`/`P_pay`, luồng RNG riêng, cửa sổ Channon `K > 0`, báo hụt lịch không tự chế EU | thẻ | `completed` |
| 2026-11-18 | [Thẻ R0 — đo lại toàn bộ gate tại `46e25d5` (Windows 11 x64)](testing/remeasure-2026-11-18-46e25d5.md) — lượt đo tuần tự toàn bộ gate frontend, backend, tái lập 6 tiến trình và đếm test trên Windows 11 x64 (`test:frontend` 1/700 đỏ do CRLF) | đo | `completed` |
| 2026-11-18 | [Thẻ R0 — đề xuất mục §1.i cho `STATE_OF_THE_PROJECT.md`](testing/state-1x-proposed-2026-11-18.md) — bản thảo bảng số đo §1.i theo văn phong §1.h từ lượt đo R0 tại `46e25d5`; đã chép vào STATE §1.i | đề xuất | `completed` |
| 2026-11-18 | [Thẻ S11 — khoá `live.grass_steering` (tìm cỏ cho thú ăn cỏ)](planning/2026-11-18-s11-grass-steering.md) — kết quả âm, **mã không merge** (nhánh `dell/s11-grass-steering` tại `95044c4b`): phép thử cơ chế có đối chứng cho lái tới cỏ dương ở tick 1.000–1.500 rồi âm từ 2.000; 100.000 tick `VIABLE = no` ở hệ số 1 / 0,7 / 0,5. Bỏ khát thì con mồi không thiếu ăn (0,89–0,98 mức mục tiêu); khát mới là giới hạn | thẻ | `completed` |
| 2026-11-18 | [Thẻ S12 — khoá `live.lake_layout` (rải hồ nước theo lưới 2-D)](planning/2026-11-18-s12-lake-layout.md) — thêm `live.lake_layout` (`LIVE_KEYS.len() = 25`); giảm khoảng cách cực đại tới hồ (`92,25 -> 52,02`), giảm chết khát 20k tick (`181 -> 77`), đạt `1505/1505` test desktop & mọi gate tĩnh; đo 100k tick giảm tuyệt chủng `scale = 0,7` từ `4/6` xuống `1/6` (`VIABLE = no`) | thẻ | `completed` |
| 2026-11-18 | [Thẻ F045 — khôi phục checkpoint thế giới sống trùng bit với lượt chạy liền mạch](planning/2026-11-18-f045-restore-fidelity.md) — `IndexedInGrid` cho thực thể sau `rebuild_spatial_grid_system`, `SerializedAgent.sex`, và `SavedSimulationState.next_node_id` (`#[serde(default)]`); khôi phục S6 `seed = 3` ở `1 -> 2` và `600 -> 1.200` khớp từng tick với chạy liền mạch | thẻ | `completed` |
| 2026-11-18 | [Thẻ S13 — thăm dò SEL2 200.000 tick trên lưới hồ trải rộng (`live.lake_layout = 1`)](planning/2026-11-18-s13-spread-lakes.md) — đo 96 lượt 200k tick (Stage A `reserve = 0` và Stage B `reserve = 0,3`, `lakes ∈ {8, 16} × scale ∈ {0,6; 0,7; 0,85}`, mù khối lượng): `0/12` ô `VIABLE = yes`; ở Stage A `lakes = 16, scale = 0,7` đạt `0/6` tuyệt chủng & 4/5 điều kiện, chỉ trượt `C2` (`0,337..0,606`) | đo | `completed` |
| 2026-11-19 | [Thẻ S14 — thăm dò hẹp SEL2 tại 16 hồ trải rộng (`scale ∈ {0,63; 0,65; 0,67}`)](planning/2026-11-19-s14-lakes16-scale.md) — đo `24` lượt `200k` tick (`lakes = 16`, `layout = 1`, `reserve = 0`, `prey_cap = 320`, mù khối lượng): `0/3` ô `VIABLE = yes`; `scale = 0,63` (`0/6` tuyệt chủng) đạt `C1/C3/C4/C5` và `4` seed đối chứng sống đạt `C2` (`0,520..0,558`), chỉ `seed 1, 3` trượt (`0,322; 0,299`) vì đối chứng `prey_cap = 80` tắt ở tick `8.545` và `20.787`; `scale = 0,65` và `0,67` tuyệt chủng `seed 6`; cột `c2_ungrazed` (`0,299..0,413` ở `0,63`) cũng `no` | thẻ | `completed` |
| 2026-11-19 | [Thẻ S15 — đo các mẫu đối chứng ứng viên cho tỉ số `g` tại `16` hồ trải rộng (`scale = 0,63`)](planning/2026-11-19-s15-reference-check.md) — tái lập bit-for-bit `6/6` seed `scale = 0,63` của S14 và đo đối chứng `prey_cap ∈ {80, 120, 160}` (`0/6` tuyệt chủng ở `120` và `160`; `2/6` ở `80`, `pooled80 = 113.430,7`): `VIABLE` (`g_80`) `= no` (`4/6`), chẩn đoán `g_120 = 0,522..0,648` (`3/6`, `no`), `g_160 = 0,612..0,707` (`0/6`, `no`), `g_pooled80 = 0,405..0,559` (`6/6`, `yes`), `g_ungrazed = 0,299..0,413` (`2/6`, `no`); quyền chọn đối chứng thuộc chủ dự án trong `SEL2-P` | đo | `completed` |
| 2026-11-19 | [Thẻ F047 — cô lập nguyên nhân lệch khôi phục của desktop app sau F046](planning/2026-11-19-f047-restore-divergence.md) — `LiveExperimentAdapter` (`R3-like` & `S12/S13`) khớp 100% ở mọi mốc; cô lập lệch `R2a`/`R3` của desktop app do sibling trùng `AgentLineageId` bị ghi đè trong `entities_by_lineage` (và `pending_by_lineage`), đối chứng theo `(lineage_id, request_id)` khớp mọi mốc | thẻ | `completed` |
| 2026-11-19 | [Thẻ F048 — sửa va chạm sibling theo `AgentLineageId` khi khôi phục `pending_inference`](planning/2026-11-19-f048-pending-inference-restore.md) — thêm hàm dùng chung `route_restored_pending_inference` và khoá `(lineage_id, request_id)` ở cả `SimulationEngine::start`, `SavedSharedLearningState::validate`, và `snapshot::validate`; chuyển toàn bộ `R2a`/`R3` sang `div == None` (dưới `90s` debug), bổ sung test đĩa 2 sibling `PendingInference` với `shared_learning = Some(...)`, `1523/1523` desktop & mọi gate xanh (`Supersedes #144`) | thẻ | `completed` |
| 2026-11-19 | [Thẻ SEL2-P-draft — bản thảo đăng ký trước thí nghiệm chọn lọc sinh thái SEL2](planning/2026-11-19-sel2-p-preregistration-draft.md) — bản thảo chưa đóng băng (`status: proposed`, mù khối lượng, chưa chạy seed xác nhận): giả thuyết `H1` hai phía trên tích phân thời gian hậu-warmup của `live.trait_mass_mean`, `3` nhánh (`selected` / `scarce_mut0` / `scarce_drift` `SEL2-D`), `N = 20` cặp `200.000` tick (`780001 + 1000*k`), cổng smoke `C1..C5` (`999961`) với ô tham chiếu `g` để trống chờ chủ dự án quyết định | kế hoạch | `proposed` |
| 2026-11-19 | [Thẻ S2 — thêm `live.mean_energy_integral` và khối lượng cơ thể theo vùng Đông/Tây](planning/2026-11-19-s2-observables.md) — ba observable mới nối cuối `LIVE_EXTENDED_OBSERVABLE_IDS` (`Aggregation::Instant`, không nhiễu loạn nhịp lấy mẫu, sống qua ngã rẽ checkpoint qua đĩa, giá trị hữu hạn khi quần thể bằng 0) | thẻ | `completed` |
| 2026-11-19 | [Thẻ V1 — Bản đồ 3D vẽ thực thể mô phỏng và gắn nhãn trang trí (`prototype, to be replaced`)](planning/2026-11-19-v1-real-world-entities.md) — vẽ `lake`/`tree`/`food` từ `environmental_state` và `get_environmental_elements` qua `liveAgentTransform`/`coordinate` và `surfaceHeight`; bỏ qua lớp phủ cỏ vì chưa có IPC `ResourceField`; gắn nhãn + nút ẩn/hiện `WorldFish`/`WorldBirds`/`WorldWildlife`/`WorldCaves`; nâng `WorldShowcase` `130 -> 145 KiB` (`132,9 KiB`) theo `PROTOTYPE-FIRST` | thẻ | `completed` |
| 2026-11-19 | [Thẻ B1 — Thân sinh vật liên tục & LOD 3 tầng (`prototype, to be replaced`)](planning/2026-11-19-b1-continuous-bodies.md) — `SkinnedMesh` từ chuỗi đốt morphology (`ArchetypeMeshCache`), `CREATURE_BODY_SCALE = 0.32` (lựa chọn hiển thị, không cùng tỷ lệ vị trí), LOD 3 mức (`K = 8/12/16`) + per-creature frustum culling (`68` draw calls `<= 70`, `60.4 FPS` trên RTX), bổ sung synthetic `environmental_state` | thẻ | `completed` |

Bốn file prompt dạng `.txt` trong `planning/` không có front matter và cổng metadata không đọc chúng:
[`2026-08-25-handoff-prompt.txt`](planning/2026-08-25-handoff-prompt.txt) (thế giới sống),
[`2026-08-25-handoff-prompt-ecological-selection.txt`](planning/2026-08-25-handoff-prompt-ecological-selection.txt),
[`2026-08-25-handoff-prompt-ecological-selection-2.txt`](planning/2026-08-25-handoff-prompt-ecological-selection-2.txt)
và [`2026-08-26-handoff-prompt-verify-gate-fix.txt`](planning/2026-08-26-handoff-prompt-verify-gate-fix.txt).
Chúng là prompt đã dùng một lần, giữ làm bản ghi.

Hai file swarm được đưa từ nhánh `origin/claude/funny-thompson-jj42fe` (`a82b996`) vào thân cây ngày
2026-11-14, nguyên văn (bước MP-0.6 của kế hoạch 13/11). Cách dùng chúng ngày hôm nay:
[hướng dẫn chạy coding agent](../how-to/AGENT_WORKFLOW.md).

## File do agent sở hữu

Bốn file dưới đây được chuyển từ gốc repo vào đây ở `c8ebec2` và **cố ý không có front matter**:
công cụ agent đọc hoặc đồng bộ chúng. Lý do từng file nằm trong `NO_FRONT_MATTER` của
[`scripts/check_docs_metadata.mjs`](../../scripts/check_docs_metadata.mjs).

| File | Nội dung |
|---|---|
| [`AI_Context.md`](AI_Context.md) | Bối cảnh hệ thống và hướng dẫn cho agent, viết ở Phase 0 (2026-06-12). Mục tiêu hiệu năng trong đó là mục tiêu lúc ấy, không phải số đo |
| [`ORIGINAL_REQUEST.md`](ORIGINAL_REQUEST.md) | Nhật ký yêu cầu gốc gửi cho agent, do công cụ đồng bộ |
| [`TEST_INFRA.md`](TEST_INFRA.md) | Ma trận thiết kế bộ E2E — số đếm là **kế hoạch**, không phải kết quả chạy |
| [`TEST_READY.md`](TEST_READY.md) | Ghi chú sẵn sàng của bộ E2E — cùng lưu ý như trên |

## Thêm tài liệu mới

Đặt tên `YYYY-MM-DD-feature-<slug>.md` trong đúng thư mục pha, dùng front matter của
[chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md#metadata-tối-thiểu), rồi thêm một ô
vào bảng trên. `npm run check:docs` đỏ nếu thiếu front matter hoặc nếu tài liệu không có trong chỉ
mục nào.
