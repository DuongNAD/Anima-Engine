---
title: Chỉ mục quyết định kiến trúc
status: active
owner: architecture
last_reviewed: 2026-11-25
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
| [`ADR-0009`](ADR-0009-burn-022-flex-wgpu-upgrade.md) | accepted | Nâng Burn lên 0.22.1, dùng `burn-flex` và cô lập stream WGPU liên luồng (`QĐ-C`, `QĐ-R1c-a`, `QĐ-R1c-b`) |
| [`ADR-0010`](ADR-0010-pinned-world-artifact.md) | accepted | Một world artifact ghim, chia ô, là nguồn sự thật cho mô phỏng và bản đồ (thế giới thật trên bản đồ, 2026-11-19) |
| [`ADR-0012`](ADR-0012-ui-free-simulation-core.md) | accepted | Lõi mô phỏng chạy đủ không cần giao diện; giao diện là client chỉ đọc (ghi chỉ qua ADR-0004); giao diện cho máy (CLI, manifest, artifact, IPC) là hạng nhất; tầng ảnh chẩn đoán tách khỏi UI sản phẩm. ADR-0011 để dành cho lựa chọn renderer sau NR-SPIKE |
| [`ADR-0013`](ADR-0013-live-creature-lifecycle.md) | accepted | Live creature lifecycle for V3: chronological age, life stages (juvenile/adult/senescent), maturation gate & cost, costly reproduction cooldown, and senescence mortality (opt-in, closed-EU via `total_cost`) |
| [`ADR-0014`](ADR-0014-shared-island-host-and-guests.md) | accepted | Shared island architecture (`QD-ADR0014` accepted `2026-11-24`: `QD-SHAREDISLAND` amending `QD-SCOPE`; `QD-GENESIS`; `QD-PLAYRULES`): one authoritative host, real-time ecology profile, LAN `Spectate` / organism `Inhabit` intents with `CauseId` + World Record replay, phased genesis, chronicle, and opt-in host LLM narration |
| [`ADR-0015`](ADR-0015-evolving-plants.md) | proposed | Evolving plants (`QD-LIFE`, `QD-GENESIS` `G1`/`G2`/`G3`, `QD-PLAYRULES` #2): hybrid `Individual` trees/shrubs + `Population Field` evolving grass on `ResourceField`, conserved energy/`total_cost`, Red Queen coevolution, and inhabiting individual plants (`P1`–`P8`) |
| [`ADR-0016`](ADR-0016-changing-world-surface.md) | proposed | Changing world surface (`QD-LIFE` extension, `QD-BIO-D3`): dedicated `WorldRecordHeader::geological_era` (never overloading `era_index`), 3-timescale architecture (`DynamicFields` -> `SurfaceDeltaOverlay` -> offline `anima-worldgen` era transitions), physical process laws + `CauseId` triggers, and `AE-S12`/allopatric gates (`W1`–`W7`) |
| [`ADR-0017`](ADR-0017-evolved-communication.md) | proposed | Evolved communication & local perceptual channels (chemical `PheromoneGrid`, acoustic, visual; `total_cost` accounting, predator eavesdropping, offline shuffled-control auditor) |
| [`ADR-0018`](ADR-0018-ecosystem-scales-and-cycles.md) | proposed | Ecosystem open energy flow (opt-in solar LUE in, `total_cost`-routed heat out via `EnergyLedger`), density-dependent cycles (`live.prey_cap` legacy), Kleiber/lifespan allometric pace, and 3 representation tiers (individuals, swarms, population fields; `QD-BIO-D1`, `QD-ECOSCALE`) |
| [`ADR-0019`](ADR-0019-brain-body-interface.md) | accepted | Brain–body interface for variable morphologies: fixed brain shape (ADR-0003 / EB-S02 / Schema 8) + shared 2-channel motor modulation + per-edge heritable `JointCpgGene` (opt-in, growth & homologous-crossover ready, `QD-BIO-D4`; accepted 2026-11-24, `QD-BRAINBODY D1–D5 = A`) |

**Front matter thắng banner trong thân.** ADR-0005, 0006 và 0007 mở đầu thân bằng dòng "Trạng thái: đề xuất,
chờ chủ dự án", viết lúc soạn; mỗi ADR có một mục "Quyết định (2026-11-16)" ở cuối ghi việc chấp nhận, và
front matter của chúng là `accepted`. Ba quyết định đó do phiên lead ra theo uỷ quyền và được **chủ dự án xác
nhận ngày 2026-11-17** ([sổ quyết định](../planning/COMPLETION_ROADMAP_2026-10-31.md#41-quyết-định-sau-2026-11-14-ghi-thêm-2026-11-17)).
Thân ADR là bản ghi bất biến nên dòng banner cũ không bị sửa.

Tạo ADR mới từ [`ADR-0000-template.md`](ADR-0000-template.md). Không viết lại quyết
định accepted để đổi kết luận; tạo ADR mới và dùng `supersedes`/`superseded_by`.
