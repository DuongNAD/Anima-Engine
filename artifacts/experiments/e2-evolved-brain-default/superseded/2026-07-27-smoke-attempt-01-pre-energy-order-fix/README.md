# Superseded — E2 smoke attempt 01, before the energy-ordering fix

> ⛔ **Not calibration, not evidence about brains, and not the basis of any duration lock.** This
> directory is kept because deleting a failed attempt is how a record stops being a record. Nothing
> here feeds the E2 analysis, and the duration rung was **not** chosen from it.

## What this is

The first excluded-smoke calibration (seed 999983, both arms, T = 18,000), run at commit
`0bcb330` with a dirty tree — dirty only because the run's own untracked outputs were the change.
It completed in 11.44 s and every integrity gate it checked reported green.

## Why it was thrown away

It was not reproducible across processes, and the smoke run is what found that.

Running the same release binary at the same seed in twelve independent processes produced **three**
outcomes:

| processes | world checksum | `live.animals_eu` @ tick 600 |
|--:|---|---|
| 8 | `784036196` | `920.1691818237305` |
| 3 | `784036196` | `920.3547668457031` |
| 1 | `3406435134` | `920.1710510253906` |

Two independent defects, both in `core::simulation_schedule::build_tick_schedule`:

1. **The census snapshot** (fixed in `993a587`). `ecosystem_census_system` recomputes
   `pool.animals` from scratch each tick but declared no order against the systems that move agent
   reserves, so it sampled either this tick's metabolism or last tick's — 0.186 EU apart, exactly
   one tick of decay across ten agents. This is the middle row.
2. **The energy pipeline** (fixed in the commit that supersedes this directory). Bevy reported
   seven unordered conflicting pairs among `herbivore_grazing_system`,
   `resource_field_regrowth_system`, `detect_food_collisions_system`, `combat_system`,
   `metabolic_decay_system` and `resource_field_regrowth_system`. Those orders do not commute, so
   the world checksum **and `live.mean_agent_energy`** — E2's primary observable — moved with a
   per-process hash seed. This is the third row.

Neither was visible to `the_same_seed_and_manifest_give_the_same_live_checksum`, which compares two
runs *inside one process* — precisely the comparison a per-process ordering cannot fail.

## What replaced it

A fresh smoke, from the top, on a build where thirty independent processes produce a bit-identical
checksum and bit-identical values for all eleven observables. The replacement lives in `../../smoke/`
and is the only calibration the duration lock may cite.

## Integrity of these files

`checksums.sha256` in this directory still describes these bytes and still verifies; the files were
moved, not edited. They are retained exactly as written so the defect remains reproducible from the
record rather than only from the commit message.
