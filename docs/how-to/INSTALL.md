---
title: Installing and Running Anima Engine (0.9)
status: active
owner: maintainers
last_reviewed: 2026-11-24
review_cycle: per-release
---

# Installing and Running Anima Engine (0.9)

This guide explains how to download, install, and try an unsigned single-machine `0.9` pre-release build of **Anima Engine** from GitHub Actions workflow artifacts, or build it locally from source. See [`0.9.0.md`](../release/0.9.0.md) for current release scope, verified scientific limits, and open release gates.

---

## 1. Downloading an Installer from Workflow Artifacts

Pre-release installers are produced by the `.github/workflows/release-build.yml` GitHub Actions workflow (added in PR `#233`, pending merge/review) and uploaded only as workflow artifacts with a **14-day retention window** (there are no public signed releases yet).

1. Sign in to GitHub (GitHub requires authentication to download workflow run artifacts).
2. Open the repository's **Actions** tab (`https://github.com/DuongNAD/Anima-Engine/actions`) and select the green **Release Build** workflow run for the target commit or release candidate.
3. Scroll to **Artifacts** and download the bundle for your operating system:
   - **`anima-engine-windows-installers`** — contains the Windows installer(s): Windows Installer (`.msi`) and/or NSIS setup executable (`.exe`).
   - **`anima-engine-macos-bundles`** — contains the macOS disk image (`.dmg`) and application bundle (`.app`).
4. Verify the run link and commit SHA match the release note or test invitation before extracting or opening the archive.

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

`0.9` trial installers are **unsigned** (code-signing and auto-update decision `QD-M8` remains open; see [`0.9.0.md`](../release/0.9.0.md)). Because the binaries have no Authenticode signature or Apple Developer notarization, both operating systems will block first launch by default. **Only proceed after confirming you downloaded the archive directly from an expected GitHub Actions run in this repository.**

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

- **Use Low quality on integrated GPUs:** Select the **Low** graphics tier on integrated graphics (e.g., Intel Iris Xe) and leave **High** for discrete GPUs.
- **Initial 3D map startup delay:** The **Start Simulation** button remains disabled briefly at first launch while the shared `2048²` world is generated; allow startup to finish before clicking repeatedly.
- **Founding population (`ANIMA_FOUNDING_POPULATION`):** Defaults to `10` if unset (`10,000` ceiling). For interactive desktop trials, setting `ANIMA_FOUNDING_POPULATION=200` before launching populates the world visibly without overloading weaker laptops (see [`OWNER_TRIAL_RUN.md`](OWNER_TRIAL_RUN.md)).
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
