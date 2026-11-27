---
title: Installing and Running Anima Engine (0.9)
status: active
owner: maintainers
last_reviewed: 2026-11-27
review_cycle: per-release
---

# Installing and Running Anima Engine (0.9)

This guide explains how to download, install, and try an unsigned single-machine `0.9` release candidate (`v0.9.0-rc.1` workflow artifact) or final `v0.9.0` release of **Anima Engine**, or build it locally from source. See [`0.9.0.md`](../release/0.9.0.md) for current release scope (`QD-FOCUS FOCUS-2`, `#314`), verified scientific limits, and open release items.

---

## 1. Downloading an Installer and World v2 Tile Pack

Under `QD-REL09` (`#255`) and `QD-FOCUS FOCUS-2` (`#314`), installer distribution happens in two steps:

- **For `v0.9.0-rc.1` (`MP-5.1` trial sessions):** produced by the merged `.github/workflows/release-build.yml` GitHub Actions workflow (`#233`; verified green on `windows-latest` and `macos-latest` in run `45268921473`) and uploaded as workflow artifacts with a **14-day retention window** (GitHub login required).
- **For final `v0.9.0`:** published to **GitHub Releases** once `MP-5.1` trial feedback is resolved (`QD-REL09` G1/G3). Pre-merge `WV2-6` developer trial builds with locally staged tiles remain internal only (`QD-TRIAL-V3`, `WB-D4 (a)` `#288`/`#295`) and are never published.

1. Sign in to GitHub (required to download workflow run artifacts for `v0.9.0-rc.1`; final `v0.9.0` downloads will live on the repository's **Releases** page).
2. Open the repository's **Actions** tab (`https://github.com/DuongNAD/Anima-Engine/actions`) and select the green **Release Build** workflow run for the target `v0.9.0-rc.1` commit.
3. Scroll to **Artifacts** and download the bundle for your operating system:
   - **`anima-engine-windows-installers`** — contains the Windows installer(s): Windows Installer (`.msi`) and/or NSIS setup executable (`.exe`).
   - **`anima-engine-macos-bundles`** — contains the macOS disk image (`.dmg`) and application bundle (`.app`) when macOS unsigned builds ship under `QD-M4` (`#255`, contingent on `NM-2` checksum parity).
4. **World v2 tile pack (`QD-1126AM`, `FOCUS-2` step 3):** `WV2-BIG-3` (`#332`, `8d58a580`, `MAP-D8` `9/15`) committed the `81.92 km` (`4096² @ 20 m`) `BIG_4096_v2` world (`world_sha256 = 01956959c751458d279a9e675046de5627fb6095e1451138c41d750a7f88d821`, `sim_world_identity = 1d3130054d6939425003817fccf78b284e34632b64b7443e5d6b380e60203e31`, `sim_256.anmw3` = `1,445,892` B, `release_archive_sha256 = 4fdad1df3acd6ad32075365e754fbb56e5c511ff01ebaa47ae14d9267c7d7d0f`); once the owner's release gate passes (`QD-GATES-1125 (4)` `#288`), the single public tile pack `world_v2_1d313005_tiles.tar.gz` (**`190,071,986` B**, `3,817` entries / `3,560` `.ktx2` tiles; **still unpublished, pending the owner's release gate**) is published once on GitHub Releases.
5. Verify the run link, commit SHA, and release archive hash (`4fdad1df3acd6ad32075365e754fbb56e5c511ff01ebaa47ae14d9267c7d7d0f` for `world_v2_1d313005_tiles.tar.gz`) match your release note or test invitation before extracting or opening the archive.

---

## 2. System Requirements

- **Windows:**
  - 64-bit Windows 10 or Windows 11 (`x86_64`).
  - **Microsoft Edge WebView2 Runtime** (Evergreen). Preinstalled on Windows 11 and most updated Windows 10 systems; if missing on startup, install the Evergreen Bootstrapper from Microsoft (`https://developer.microsoft.com/en-us/microsoft-edge/webview2/`).
  - GPU with Direct3D 11 / WebGL2 support (tested on NVIDIA RTX 5060 Ti and Intel Iris Xe; integrated GPUs should use the **Low** quality tier).
- **macOS:**
  - Apple Silicon (`arm64`) / supported macOS matching the `macos-latest` CI runner bundle, using built-in WebKit and Metal / WebGL2.
- **Power & Memory:**
  - Plug laptops into **AC mains power** before launching the desktop simulator and close other heavy GPU/build applications.

---

## 3. Proceeding Past Unsigned-Build Warnings

Under owner decisions `QD-REL09` G2 and `QD-OPENC` (`#255`), `0.9` installers ship **unsigned** (`QD-M8` signing and auto-update decisions are deferred until before `v1.0.0`; see [`0.9.0.md`](../release/0.9.0.md)). Because the binaries have no Authenticode signature or Apple Developer notarization, both operating systems warn on first launch (`QD-M4` covers unsigned macOS if `NM-2` checksums match the Windows class). **Only proceed after confirming you downloaded the archive directly from an expected GitHub Actions run or GitHub Release in this repository.**

### Windows (Microsoft Defender SmartScreen)

1. Run the extracted `.exe` (NSIS) or `.msi` installer.
2. When **Windows protected your PC** (Microsoft Defender SmartScreen) appears, click **More info**, confirm the app name, and click **Run anyway**.
3. Third-party antivirus scanners may also quarantine unsigned executables; if blocked during a trial session, record the prompt before adjusting local exclusions.

### macOS (Gatekeeper)

1. Open the extracted `.dmg` (or unzip the artifact) and drag `Anima-Engine.app` to Applications (or a test folder).
2. On first launch, macOS displays a Gatekeeper dialog stating that **Anima-Engine cannot be opened because the developer cannot be verified** (or is damaged/unidentified).
3. Click **Cancel** (never move to Trash), then open it explicitly:
   - Control-click (right-click) `Anima-Engine.app` → **Open** → click **Open** in the confirmation dialog; **or**
   - Go to **System Settings → Privacy & Security**, scroll to the security notice for `Anima-Engine.app`, and click **Open Anyway**.

---

## 4. Known Heavy Load & Runtime Guidance

As noted in [`CLAUDE.md`](../../CLAUDE.md#gotchas) and [`OWNER_TRIAL_RUN.md`](OWNER_TRIAL_RUN.md), the full Bevy/Tauri desktop application is resource-intensive and has previously frozen or shut down constrained dev laptops when run alongside heavy builds:

- **Use Low quality on integrated GPUs:** Select the **Low** graphics tier on integrated graphics (e.g., Intel Iris Xe) and leave **High** for discrete GPUs. The `WV2-6` Intel Iris Xe target is `<= 8 GB` RSS, `<= 4 GB` VRAM, and `<= 60` draw calls ([`2026-11-26-wv2-6-iris-xe-runbook.md`](../ai/planning/2026-11-26-wv2-6-iris-xe-runbook.md), `#289`).
- **World loading & startup delay:** Allow **Start Simulation** to enable before clicking repeatedly. On trunk `8d58a580` (`WV2-BIG-3` `#332`, following `WV2-7` `#279`), desktop simulation boots the `81.92 km` `BIG_4096_v2` world (`sim_world_identity = 1d313005…`, `1,445,892` B; prior provisional `C01` preset is preserved at `src-tauri/crates/anima-worldgen/presets/wv2_c01_40960.toml`, `288` B, SHA-256 `f460fa73…`) and rejects legacy pre-World-v2 and old `C01` saves while the screen still renders the v1 map until `WV2-6` merges; in the target `v0.9.0-rc.1` / `v0.9.0` builds (`QD-FOCUS FOCUS-2` `#314`, `WV2-D2` `#295`), the viewer loads the pre-baked `81.92 km` (`4096² @ 20 m`) World v2 tile pack (`world_v2_1d313005_tiles.tar.gz`, `190,071,986` B, SHA-256 `4fdad1df3acd6ad32075365e754fbb56e5c511ff01ebaa47ae14d9267c7d7d0f`; **still unpublished, pending the owner's release gate**).
- **Founding population (`ANIMA_FOUNDING_POPULATION`):** Defaults to `10` if unset (`10,000` ceiling). Per `QD-SCALE-TRIAL` (`#255`), interactive shared trials/demos target **about `100` animals** (`~0.33 ms/tick` headless vs `5.64 ms/tick` at `1,000` agents); setting `ANIMA_FOUNDING_POPULATION=100` or `200` before launching populates the world visibly without overloading weaker laptops (see [`OWNER_TRIAL_RUN.md`](OWNER_TRIAL_RUN.md)).
- **Headless checks and experiments do not need the desktop app:** Automated Rust tests (`cargo test --features desktop`), Vitest suites, and `anima-lab` headless experiments run without spawning a Tauri window or WebGL renderer.

---

## 5. Building Locally from Source (Contributors)

Contributors building desktop bundles locally must use the npm wrapper scripts rather than bare `tauri` CLI commands (see [`CLAUDE.md`](../../CLAUDE.md#commands) and [How-to Index](README.md)):

```powershell
npm ci
npm run tauri:build
```

- **Never run bare `tauri dev` or `tauri build`:** `src-tauri/tauri.conf.json` has no field for default Cargo features, so `npm run tauri:dev` and `npm run tauri:build` pass `--features desktop` explicitly. Bare commands silently compile `default = []` without Neo4j lineage, cross-shard migration, or the `ml-wgpu` learner.
- On 16 GiB Windows laptops (such as Dell Vostro 3530), avoid running desktop builds in parallel with other `cargo`/`rustc` processes; see [`STATE_OF_THE_PROJECT.md` §4](../planning/STATE_OF_THE_PROJECT.md#4-bẫy-đã-biết--đọc-trước-khi-sửa) and [`RELEASE_CHECKLIST.md`](RELEASE_CHECKLIST.md).
