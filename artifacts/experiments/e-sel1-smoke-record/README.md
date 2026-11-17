# E-SEL1 smoke record (excluded from every analysis)

Each smoke run of E-SEL1 that is not the final, locked one is kept here, so every choice the smoke made, and every time it stopped, can be audited. Nothing here feeds the E-SEL1 analysis.

None of these files contains a body-mass value. In `--smoke` mode the runner writes only these observables:

- `live.agent_count`
- `live.standing_crop`
- `live.closed_eu_total`
- `live.births`
- `live.deaths`

The driver reads only:

- `live.agent_count`
- `live.standing_crop`
- microseconds per tick
- each run's final checksum

## run-1-stopped/

Smoke run 1 used build `13209396e6fb8a0e696ca9c023e70531df305634` (release, macOS arm64). It was driven by `scripts/esel1_smoke_driver.mjs`, before that script had its manipulation check.

It stopped at step C. In every trial, the abundant and scarce arms ended on the same final checksum, so r = 1. The cause is F-043 in `FINDINGS.md`: `live.food_cap` was inert.

Every value from this run is void. Smoke run 2 starts again from step A on a build with the fix. The preregistration records this in its block `amendment_1_2026-11-17_smoke_run_1_stopped_F043`.

The run directory contains:

| Path | Contents |
|---|---|
| `trials/<key>/smoke/` | One runner invocation's output. `<key>` is `n0-<N0>_fa-<abundant cap>_fs-<scarce cap>`. |
| `smoke-decisions.json` | Every quantity the driver consulted, and each step's decision. |
| `driver-stdout.txt` | The driver's console output. |

## run-2-stopped/

Smoke run 2 used build `7a825cc56135df83d5a513ecf59fe3b2e9f1dd92` (release, macOS arm64), which includes the F-043 fix. It was driven by the in-repo driver, with the manipulation check.

The manipulation check passed: every scarce arm ended on a final checksum different from the abundant arm's.

The run still stopped at step C. Across food caps 25, 17, 13 and 6, r ran from 0.943 to 1.027, against a required 0.4 to 0.6.

Per amendment 1, the registration is closed without an experiment. The preregistration records this in its block `amendment_2_2026-11-17_smoke_run_2_stopped_registration_closed`.

Run 2's console output was not saved to a file. `smoke-decisions.json` holds every quantity the driver consulted.
