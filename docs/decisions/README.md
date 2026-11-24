---
title: Chỉ mục quyết định kiến trúc
status: active
owner: architecture
last_reviewed: 2026-11-24
---

# Architecture Decision Records

| ADR | Trạng thái | Quyết định |
|---|---|---|
| [`ADR-0001`](ADR-0001-creature-development-lifecycle.md) | accepted | Tách development khỏi spawn và lưu phenotype |
| [`ADR-0002`](ADR-0002-world-laws-and-exotic-energy.md) | accepted | World laws + exotic-energy ledger + World Lab. Chấp nhận 2026-11-17 **trong phạm vi lát headless**; live Bevy, lưu trữ, UI và detector loài vẫn là mục tiêu |
| [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md) | accepted | Não di truyền theo cá thể + mở rộng không gian hành động |
| [`ADR-0004`](ADR-0004-observer-as-declared-intervention.md) | accepted | Người quan sát nhập vai là can thiệp được khai báo |
| [`ADR-0005`](ADR-0005-live-population-turnover.md) | accepted | Vòng đời quần thể (sinh sản/chết) trong adapter thí nghiệm live (QĐ-4, QĐ-5) |
| [`ADR-0006`](ADR-0006-locomotion-actuation.md) | accepted | Vận động của sinh vật: lái ghi `target_velocity` hay ma sát dị hướng (F-042, QĐ-LOC) |
| [`ADR-0007`](ADR-0007-live-ecology-sustainability.md) | accepted | Duy trì quần thể live: sàn lực lái, lái tới nước, thú săn (QĐ-ECO) |
| [`ADR-0008`](ADR-0008-world-past-and-time.md) | accepted | Quá khứ bất biến và bản ghi thế giới (ký ức của thế giới); ba lớp thời gian (QĐ-PAST, QĐ-TIME) |
| [`ADR-0009`](ADR-0009-burn-022-flex-wgpu-upgrade.md) | proposed | Nâng Burn lên 0.22.1, dùng `burn-flex` và cô lập stream WGPU liên luồng (`QĐ-C`, `QĐ-R1c-a`, `QĐ-R1c-b`) |
| [`ADR-0010`](ADR-0010-pinned-world-artifact.md) | accepted | Một world artifact ghim, chia ô, là nguồn sự thật cho mô phỏng và bản đồ (thế giới thật trên bản đồ, 2026-11-19) |
| [`ADR-0012`](ADR-0012-ui-free-simulation-core.md) | accepted | Lõi mô phỏng chạy đủ không cần giao diện; giao diện là client chỉ đọc (ghi chỉ qua ADR-0004); giao diện cho máy (CLI, manifest, artifact, IPC) là hạng nhất; tầng ảnh chẩn đoán tách khỏi UI sản phẩm. ADR-0011 để dành cho lựa chọn renderer sau NR-SPIKE |
| [`ADR-0013`](ADR-0013-live-creature-lifecycle.md) | proposed | Live creature lifecycle for V3: chronological age, life stages (juvenile/adult/senescent), maturation gate & cost, costly reproduction cooldown, and senescence mortality (opt-in, closed-EU via `total_cost`) |
| [`ADR-0015`](ADR-0015-evolving-plants.md) | proposed | Evolving plants (`QD-LIFE`, `QD-GENESIS` `G1`/`G2`/`G3`, `QD-PLAYRULES` #2): hybrid `Individual` trees/shrubs + `Population Field` evolving grass on `ResourceField`, conserved energy/`total_cost`, Red Queen coevolution, and inhabiting individual plants (`P1`–`P8`) |

**Front matter thắng banner trong thân.** ADR-0005, 0006 và 0007 mở đầu thân bằng dòng "Trạng thái: đề xuất,
chờ chủ dự án", viết lúc soạn; mỗi ADR có một mục "Quyết định (2026-11-16)" ở cuối ghi việc chấp nhận, và
front matter của chúng là `accepted`. Ba quyết định đó do phiên lead ra theo uỷ quyền và được **chủ dự án xác
nhận ngày 2026-11-17** ([sổ quyết định](../planning/COMPLETION_ROADMAP_2026-10-31.md#41-quyết-định-sau-2026-11-14-ghi-thêm-2026-11-17)).
Thân ADR là bản ghi bất biến nên dòng banner cũ không bị sửa.

Tạo ADR mới từ [`ADR-0000-template.md`](ADR-0000-template.md). Không viết lại quyết
định accepted để đổi kết luận; tạo ADR mới và dùng `supersedes`/`superseded_by`.
