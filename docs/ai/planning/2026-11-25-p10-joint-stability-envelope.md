---
phase: planning
feature: joint-stability-envelope
title: P10 JOINT-STABILITY-ENVELOPE — dt = 1/60 Gate for Mutation Floor vs Unscaled Juvenile Scaling
description: Gate pinning resolve_joints_system + integrate_physics_system stability at dt = 1/60 across 3,600 ticks at m = 0.05 kg and divergence at s = 0.25 unscaled juvenile mass.
status: proposed
owner: anima-case
last_reviewed: 2026-11-25
---

# `P10` — `JOINT-STABILITY-ENVELOPE` (Gate for `V3-GROWTH` `D-GROWTH-2`)

## Physics Criterion & Setup

- **Solver systems:** `resolve_joints_system` -> `integrate_physics_system` (`src-tauri/src/physics/dynamics.rs:23-174`) at live `TimeStep(1.0 / 60.0)` (`dt = 1/60 s`).
- **Creature chain:** `2`-segment chain spawned via `decode_genotype` (`src-tauri/src/evolution/genotype.rs:114-260`), which assigns unscaled `JointConstraint { stiffness: 10.0, damping: 1.0 }`.
- **Stability threshold (`research3/RESEARCH_REVIEW_2026-11-25_round3.md` §4 item 4):** Forward Euler damping integration requires reduced mass $\mu = \frac{m_1 m_2}{m_1 + m_2} > \frac{c \cdot dt}{2} \approx \frac{1.0}{120} \approx 0.00833\text{ kg}$.
- **Initial perturbation:** Displaces child segment `Position.0.z += 0.05` (`0.05 m` stretch, initial elastic potential energy $E_0 = \frac{1}{2} k (\Delta z)^2 = 0.012500\text{ J}$) because `decode_genotype` initializes segments at exact zero spring error.

## Measured Results (`src-tauri/tests/joint_stability_envelope_tests.rs`)

| Case | Segment Mass (kg) | Reduced Mass $\mu$ (kg) | Threshold $\frac{c \cdot dt}{2}$ (kg) | Outcome (`dt = 1/60`, `3,600` ticks) | Measured KE / Tick |
|---|---|---|---|---|---|
| **Mutation mass floor** (`evolution/mutation.rs:117`) | `0.05` | `0.025` (`> 0.00833`) | `0.00833` | **PASS** (`3,600` ticks, all finite & bounded) | `init_pe = 0.012500`, `max_ke = 0.002075`, `final_ke = 0.000000` |
| **Negative control (`s = 0.25` juvenile, unscaled `k = 10`, `c = 1`)** | `0.05 * 0.25^3 = 0.00078125` | `0.000391` (`< 0.00833`) | `0.00833` | **PASS (diverges as required)** | Exceeds `1e6 * init_pe` at **tick `3`** (`ke = 4.802442e5`) |

- **Gate meaning for `V3-GROWTH` (`D-GROWTH-2`):** Any juvenile mass scaling ($m(s) \propto s^3$) below the floor must co-scale joint damping/stiffness (or enforce a solver sub-step / semi-implicit stabilization) so $\mu > c \cdot dt / 2$ holds throughout development.
