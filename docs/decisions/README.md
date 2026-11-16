---
title: Chỉ mục quyết định kiến trúc
status: active
owner: architecture
last_reviewed: 2026-07-24
---

# Architecture Decision Records

| ADR | Trạng thái | Quyết định |
|---|---|---|
| [`ADR-0001`](ADR-0001-creature-development-lifecycle.md) | accepted | Tách development khỏi spawn và lưu phenotype |
| [`ADR-0002`](ADR-0002-world-laws-and-exotic-energy.md) | proposed | World laws + exotic-energy ledger + World Lab |
| [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md) | accepted | Não di truyền theo cá thể + mở rộng không gian hành động |
| [`ADR-0004`](ADR-0004-observer-as-declared-intervention.md) | accepted | Người quan sát nhập vai là can thiệp được khai báo |
| [`ADR-0005`](ADR-0005-live-population-turnover.md) | accepted | Vòng đời quần thể (sinh sản/chết) trong adapter thí nghiệm live (QĐ-4, QĐ-5) |
| [`ADR-0006`](ADR-0006-locomotion-actuation.md) | accepted | Vận động của sinh vật: lái ghi `target_velocity` hay ma sát dị hướng (F-042, QĐ-LOC) |
| [`ADR-0007`](ADR-0007-live-ecology-sustainability.md) | accepted | Duy trì quần thể live: sàn lực lái, lái tới nước, thú săn (QĐ-ECO) |

Tạo ADR mới từ [`ADR-0000-template.md`](ADR-0000-template.md). Không viết lại quyết
định accepted để đổi kết luận; tạo ADR mới và dùng `supersedes`/`superseded_by`.
