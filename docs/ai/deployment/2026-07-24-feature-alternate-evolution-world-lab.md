---
phase: deployment
feature: alternate-evolution-world-lab
title: Rollout — Alternate Evolution & World Lab
description: Local feature-flag rollout, artifact migration and rollback gates
status: proposed
owner: maintainers
last_reviewed: 2026-07-24
plan: ../planning/2026-07-24-feature-alternate-evolution-world-lab.md
---

# Rollout — Alternate Evolution & World Lab

Đây là feature desktop/local; chưa cần cloud infrastructure hoặc secrets.

## Stages

1. Schema + headless runner, feature flag off.
2. Reference exotic field/budget.
3. Reference pathway/selection experiments.
4. Live Bevy adapter + save migration, opt-in.
5. World Lab UI read-only/compare.
6. Checkpoint fork/species diagnostics.
7. Default-enable chỉ sau AE-S01…15 và map gates liên quan.

## Artifact migration

- Manifest/result/save/snapshot đều versioned.
- Legacy save map thành `exotic_energy=None`, pathway/storage zero.
- Unknown future version fail có thông báo.
- Reader cũ được giữ ít nhất một release sau writer mới.

## Rollback

- Set `exotic_energy=None`/disable feature flag.
- Không xóa schema reader hoặc field migration.
- Replay baseline fixture và AE-S01.
- Nếu result schema lỗi, giữ raw artifact/failure provenance để debug.
- Rollback visual layer không được xóa simulation state.

## Release gates

- Fresh evidence AE-S01…AE-S15 theo phase.
- Existing S/CM regression gates pass.
- Save/load/migration fixtures pass.
- Performance baseline trên hardware mục tiêu.
- Animal Map Vision pass cho thay đổi map/render; hiện blocked vì MCP unavailable.
