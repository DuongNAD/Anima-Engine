---
title: First External User Trial Run (Map, Creature Bodies, Experiment Worlds)
status: active
owner: maintainers
last_reviewed: 2026-11-19
review_cycle: per-release
---

# First External User Trial Run

> *Vietnamese version / Bản tiếng Việt: [OWNER_TRIAL_RUN.vi.md](OWNER_TRIAL_RUN.vi.md).*

Written on 2026-11-19 for the project owner per decision `QD-TRIAL` (trial run on Windows 1). This is a **prototype** preview: the goal is to show a friend where the project is heading and gather their feedback, not a production release.

The sequence has three stages, each gating entry to the next:

1. **Solo run on Windows 1** (~30 minutes, §3), with the lead reading logs alongside. Nobody has yet run the real desktop app with the new map; every prior check ran in a browser against synthetic fixtures.
2. **Guided demo for your friend on Windows 1**, with your friend operating the app (§5).
3. **Installing on your friend's machine** (§6), only if stage 1 finishes with no critical bugs.

## 1. Prerequisites Before Starting

All pull requests below must already be merged into `feat/ecological-selection`. Verify with `gh pr view <number> --json state -q .state`, which must print `MERGED`.

| Item | Owner | Why It Is Needed for the Trial |
|---|---|---|
| B1, continuous creature bodies (PR #153) | Windows 1 | Creatures have continuous bodies with eyes at the front |
| LAND1, land vs water navigation in simulation | Dell | Creatures no longer walk across open sea |
| W2-lite, UI button to open an experiment world | Mac | Launches a world from a registered manifest |
| W3-lite batch 1 (fixing 5 worst visual defects) | Windows 1 | Removes edge green halo, sea speckle, white-box waterfalls, orange-red strata bands, flat snow |
| W3-lite batch 2 (CC0 PBR textures and trees) | Windows 1 | Optional: visibly better look, though it may not land in time |

The lead notifies when prerequisites are met, then pauses the Windows 1 agent during the trial. Never run concurrently with the agent, because the desktop app is heavy and simultaneous Rust builds slow the machine down.

## 2. Building the Binary on Windows 1

Use a dedicated worktree, never the agent's working checkout. Run in PowerShell:

```powershell
cd D:\teamwork_projects\Anima-Engine
git fetch origin
git worktree add D:\teamwork_projects\anima-trial origin/feat/ecological-selection
cd D:\teamwork_projects\anima-trial
npm ci
npm run tauri:build
```

- Always use `npm run tauri:build`, **never** bare `tauri build`. Bare commands omit `--features desktop`, producing a feature-stripped binary silently without failing.
- The initial cold build can take 20–40 minutes. Tauri may download packager tools (NSIS, WiX) on first run.
- Executable: `.\src-tauri\target\release\anima-engine.exe`.
- Installers: under `.\src-tauri\target\release\bundle\` (`nsis\` and `msi\` directories).

Launch from PowerShell so logs remain visible in the console and capture them to send to the lead:

```powershell
$env:ANIMA_FOUNDING_POPULATION = "200"
.\src-tauri\target\release\anima-engine.exe 2>&1 | Tee-Object -FilePath trial-log.txt
```

`ANIMA_FOUNDING_POPULATION` sets the number of founding individuals. Left unset, only `10` spawn and the map looks empty. `200` is a comfortable number for the trial; the upper cap is `10,000`.

## 3. Solo Trial Run (~30 Minutes, with Lead Reading Logs)

Check off each line. If any item fails, take a screenshot and send `trial-log.txt` to the lead.

**Startup**

- [ ] App opens with no error dialog.
- [ ] The `"Bắt đầu mô phỏng"` ("Start simulation") button becomes enabled after a few seconds. It is intentionally disabled until the shared world finishes loading in the backend; on a cold machine, generating the `2048²` world can take a moment.
- [ ] Click `"Bắt đầu mô phỏng"`: creature count and tick counter advance.

**3D Map** (`"🏞️ Landscape Showcase"` button)

- [ ] Map renders; the info badge displays **"Simulated: … lakes · … trees · … food"**. If it reads `"Synthetic fixture"`, the app is showing synthetic data: stop and report to the lead.
- [ ] Creatures stand on land and do not walk on the ocean (requires LAND1).
- [ ] Zoom in close to a creature: continuous body, eyes at the front. Red indicates predators, green indicates prey.
- [ ] Fish, birds, background fauna, and burrows display `"Decoration, not simulated"`; the hide-decorations toggle works.
- [ ] HUD FPS stays at `30` or higher on the `"Cao"` ("High") preset. Record the number.
- [ ] Camera modes (`Quay`, `Bay`, `Đi bộ`, `Trên cao`, `Cine` — Orbit, Fly, Walk, Overview, Cine) and `0.5×–4×` speeds work.

**Sustained Run**

- [ ] 20 minutes at `4×` speed: no freeze, no crash.
- [ ] Memory of `anima-engine.exe` in Task Manager does not grow unboundedly (record at minute 1 and minute 20).
- [ ] Creatures survive: the population does not go extinct within 20 minutes.

**Experiment World** (requires W2-lite)

- [ ] The experiment-world button appears beside the run button and opens an existing manifest.
- [ ] Opening an experiment world runs and the 3D map renders that exact world.

**Save and Reload**

- [ ] Close the app and reopen it: the app prompts to resume or loads from autosave without errors.

If a critical bug occurs (crash, hang, disappearing creatures, black map), **stop here**. Have the lead fix it before showing an external user.

## 4. What to Tell Your Friend (Honest Framing)

Do say:

- This is an early prototype of an artificial-life simulator where creatures evolve over generations.
- Each creature's body shape is constructed from segments, joints, and dimensions in its own "genotype". Colors indicate role (predator vs prey), not habitat region.
- Lakes, trees, and food items on the map belong to the simulation. Fish, birds, background animals, and burrows are decorative only, and the app labels them clearly as such.
- Eyes and breathing motion are decorative. Bodies are rendered smaller than true scale relative to world distances so they can be seen.
- The scientific experiments run separately headlessly outside the UI. One pre-registered experiment is underway (SEL2-P: whether food scarcity alters body size), with island rules queued as a main next question.

Do not say:

- "Autonomous living AI", "new species", or "realistic evolution". The project has neither measured nor claimed those yet.
- That everything visible on the map is simulated: decorative props are not.

## 5. Guided Session (~20 Minutes, Friend at the Controls)

1. **Overview** (Overview camera): a world with snowy peaks, forests, rivers, lakes, and sea.
2. **Inspect a creature closely**: morphology comes from its genotype; each individual differs.
3. **Feeding, drinking, hunting**: leave at `1×` speed and follow a predator.
4. **Evolution**: switch to `4×` for a few minutes; generation and population counters change.
5. **Experiment world**: open a manifest and explain "same laws, same seed, deterministic replay on the same OS family".
6. **Gather feedback** (§7).

## 6. Installing on Your Friend's Machine — Possible, with Caveats

Installation is possible. Reasons not to do it right away:

- **Unsigned installers** (signing decision `QĐ-M8` remains deferred). Windows SmartScreen will show `"Windows protected your PC"`. Your friend must click `"More info"`, then `"Run anyway"`. Some antivirus tools may block unsigned binaries.
- **Not yet benchmarked on low-end GPUs.** The target is `30 FPS` on Low quality on Intel Iris Xe with `1,000` creatures, unmeasured at this time. Machines without a discrete GPU may stutter; advise choosing `"Thấp"` ("Low") and lowering `ANIMA_FOUNDING_POPULATION`.
- **Windows-only build for now.** CI builds installers on Windows only here. macOS builds require code signing and notarization or Gatekeeper blocks launch.
- **No auto-updater or automatic crash reporting.** If the app crashes on their machine, you only receive screenshots and log files they manually send back.

Once stage 3 passes, share the installer under `src-tauri\target\release\bundle\nsis\` with three short instructions:

1. Run the installer. When SmartScreen appears, click `"More info"`, then `"Run anyway"`.
2. Open Anima-Engine, wait for `"Bắt đầu mô phỏng"` to light up, click it, then click `"🏞️ Landscape Showcase"`.
3. If performance feels sluggish, select graphics quality `"Thấp"` ("Low"). If an error occurs, take a screenshot and send it back.

The installed binary defaults to `10` founders unless the environment variable is set. To spawn more, guide them to launch from PowerShell as in §2, or wait for a future in-app founder-count input card.

## 7. Feedback to Collect

- What felt most interesting? What was most confusing?
- Would you want to configure conditions for a world (food scarcity, predator density, small islands) and watch what happens?
- How did the map and creatures look: attractive, rough, or hard to read?
- Did the app run smoothly on your machine (FPS, graphics preset)?

Write down the answers and send them to the lead to feed into the work queue.

## 8. After the Trial

- Notify the lead to resume the Windows 1 agent.
- Send the lead `trial-log.txt`, screenshots, and your friend's feedback.
- Optionally remove the temporary trial worktree: `git worktree remove D:\teamwork_projects\anima-trial`.
