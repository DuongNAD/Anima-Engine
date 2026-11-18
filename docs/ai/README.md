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
| 2026-11-17 | [Giao việc cho `agent-win` — thẻ R1c, rồi R0](planning/2026-11-17-agent-win-handoff-r1c.md) — prompt để dán vào phiên Windows, dòng uỷ quyền đã soạn cho R1c, lý do chọn thẻ | prompt | `active` |
| 2026-11-17 | [Đánh giá dự án theo mười hai tiêu chí](monitoring/2026-11-17-project-assessment.md) — bảng điểm sau lượt rà soát toàn bộ tài liệu; điểm là nhận định, không phải phép đo | đánh giá | `completed` |
| 2026-11-18 | [Thẻ S9 — tách nguyên nhân chết](planning/2026-11-18-s9-death-causes.md) — `live.deaths_starvation` và `live.deaths_dehydration`; đo lại các ô hệ số 0,5 và 0,7 của lưới S6: khát giết 71–80 % số cá thể chết ở cả bốn ô | đo | `completed` |

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
