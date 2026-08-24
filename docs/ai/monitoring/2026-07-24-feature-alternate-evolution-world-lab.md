---
phase: monitoring
feature: alternate-evolution-world-lab
title: Observability — Alternate Evolution & World Lab
description: Biến số, sampling, causal traces, budgets, experiment health và dashboard
status: proposed
owner: simulation-observability
last_reviewed: 2026-07-24
design: ../design/2026-07-24-feature-alternate-evolution-world-lab.md
---

# Observability — Alternate Evolution & World Lab

## Nguyên tắc

Observability là một phần của mô hình, không phải log thêm sau cùng. Mọi chart/layer/inspector phải
đọc `ObservableRegistry` và `ExperimentResult`; renderer không tự tính lại biến sinh thái.

## Cấp quan sát

| Scope | Ví dụ |
|---|---|
| World | total EU/MU, climate, species richness, run checksum |
| Region/chunk | source flux, biomass, trait frequency, migration |
| Cell | MU density/uptake, water, nutrient, local population |
| Organism | genotype, phenotype, EU/MU storage, cost, fitness components |
| Lineage | ancestors, births/deaths, trait distribution, niche |
| Species cluster | evidence state, members, divergence, gene flow |
| Experiment | factors, seeds, effect size, interval, failures, performance |

## Canonical metrics

### Budget and mechanism

- `exotic.mu.initial`
- `exotic.mu.sourced`
- `exotic.mu.field`
- `exotic.mu.organism_storage`
- `exotic.mu.dissipated`
- `exotic.mu.exported`
- `exotic.mu.balance_error`
- `energy.eu.plants|animals|detritus|total|balance_error`
- `exotic.uptake_rate`, `exotic.spend_rate`, `exotic.toxicity`

### Evolution

- pathway trait mean/variance/frequency;
- expressed phenotype cost;
- survival probability và reproductive success theo trait bin;
- lineage birth/death/persistence;
- niche occupancy/divergence;
- morph/ecotype/candidate-species/species count;
- extinction, merge và split event;
- time-to-first-mechanism/selection/divergence signal.

### Experiment quality

- requested/completed/failed run count;
- manifest/model/schema/build fingerprints;
- control/treatment initial checksum parity;
- deterministic replay divergence;
- sample count/gaps/downsample level;
- effect size, interval, quantiles;
- causal coverage: tỷ lệ KPI change có root/mechanism;
- budget alert count.

### Performance

- sim tick/ecology-band duration;
- exotic field update time;
- hot-loop allocation count;
- active/reduced/cohort organism count;
- causal records/second và retained bytes;
- time-series bytes/run;
- snapshot time/size;
- frontend layer/chart render time và dropped frames.

## Sampling policy

| Data | Default cadence | Retention |
|---|---:|---|
| Budget audit | mỗi ecology tick | full violations + downsample normal |
| World aggregates | 1 Hz sim | multiresolution |
| Cell fields | 0.2–1 Hz sim | chunked snapshots/delta |
| Organism runtime | selected/watchlist only | bounded ring buffer |
| Trait/species metrics | mỗi generation/epoch | full |
| Birth/death/reproduction | event-driven | aggregate + sampled evidence |
| Causal effects | threshold/event-driven | parent/root + top-K |
| UI telemetry | 1–5 Hz wall display | ephemeral |

UI phải hiển thị cadence và downsample level.

## Dashboards

### Experiment Builder/Run Health

- manifest/factor diff;
- seed queue, progress, failures;
- build/schema fingerprints;
- cancel/retry/export.

### Energy Flows

- EU Sankey/ledger;
- MU source → field → storage → spend/dissipation;
- balance error và top offending region/system.

### Evolution

- trait distribution/frequency;
- survival/reproduction by trait;
- lineage tree;
- species evidence timeline;
- niche/MAP-Elites projections.

### Compare

- aligned control/treatment series;
- delta/effect size/interval;
- per-seed small multiples;
- failed/outlier runs;
- factor diff.

### Causal Explorer

- selected metric at tick/generation;
- direct parent, root cause, contributing effects;
- before/after/delta/mechanism/confidence;
- jump tới region/entity/lineage.

## Alerts

### Critical

| Condition | Action |
|---|---|
| NaN/Inf hoặc field ngoài bound | Stop run, persist failure snapshot |
| EU/MU balance vượt critical tolerance | Stop or quarantine run; mark ensemble failed |
| Initial checksum mismatch giữa fork pair | Không chạy comparison |
| World-law/snapshot fingerprint mismatch | Reject restore/fork |
| Deterministic replay divergence | Block release claim |

### Warning

| Condition | Action |
|---|---|
| Ensemble N dưới minimum claim | Gắn “exploratory only” |
| Causal coverage của KPI thấp | Hiện “unexplained contribution” |
| Sampling quá thưa cho transient | Warning trong chart/export |
| Species cluster gần threshold | Giữ Candidate, hiện uncertainty |
| Causal/time-series budget gần trần | Tăng aggregation/downsample có ghi metadata |
| Map MCP unavailable | Block visual/ecology completion gate |

## Structured event format

```text
run_id, experiment_id, tick, generation, system, region,
observable_id, before, after, delta, unit,
cause_id, parent_effect_id, mechanism,
state_checksum, schema_version
```

Không log raw genotype/large field mỗi tick. Dùng artifact refs/checksums; organism detail chỉ cho
watchlist hoặc event evidence.

## Health checks

- Manifest validator pass.
- Artifact/law/snapshot fingerprints match.
- Observable registry contains required AE metrics.
- Budget audit runs at expected cadence.
- Sampling queue/backpressure within bounds.
- Result artifact finalize + checksum.
- Replay smoke on one seed.
- Frontend can load catalog/series/trace without unit mismatch.

## Incident/debug workflow

1. Freeze run và ghi failure snapshot.
2. Record last good checksum/tick và system.
3. Inspect budget, causal parents và RNG stream key.
4. Replay một seed headless để reproduce.
5. Minimize manifest/factor set.
6. Add regression test và attach result artifact.
7. Không remove failed seed khỏi ensemble summary.

## Current status

Đây là monitoring/observability contract đề xuất. Current code đã có `CausalLedger`,
`ScenarioResult.series`, final observables và `EcosystemPanel`, nhưng chưa có registry, branch tree,
MU metrics hoặc ensemble summary.
