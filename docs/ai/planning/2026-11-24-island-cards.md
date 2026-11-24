---
phase: planning
feature: shared-island
title: Shared-Island Implementation Cards (I1–I11, Host Authoritative LAN Island, Phased Genesis, Chronicle & Replay)
description: Ordered implementation cards I1 through I11 for accepted ADR-0014 (QD-ADR0014, 2026-11-24; 23 D-slots decided, ISLAND-D7 deferred to I10), queued pending PR #256 merge and per-card AUTHORIZATION.
status: proposed
owner: anima-case
last_reviewed: 2026-11-24
---

# Shared-Island Implementation Cards (`I1`–`I11`, `QD-ADR0014` Accepted — Queued)

> **Execution gating (`ADR-0014` `accepted` `2026-11-24` via `QD-ADR0014`, owner: `"Duyệt theo khuyến nghị"`):**
> [`ADR-0014`](../../decisions/ADR-0014-shared-island-host-and-guests.md) is **`accepted`** with all `23` Milestone 1 / `G2` / `G3` D-slots **`DECIDED`** (see `Summary of Owner Decisions` of `ADR-0014`; **`ISLAND-D7`** remains **`DEFERRED` to `I10`**). All **`ALLOWED PATHS`** below remain **queued pending `PR #256` merge**: no Rust, TypeScript, or contract edits may begin until `PR #256` merges and the lead assigns each card with explicit per-card `AUTHORIZATION` in [`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md) (`anima-case` Lane W first).
>
> **Owner-confirmed Milestone 1 & Genesis basis (`QD-SHAREDISLAND`, `QD-GENESIS`, DECIDED `2026-11-24`):**
> *(Governing scope note: `QD-SHAREDISLAND` amends `QD-SCOPE` ([`AGENT_WORKFLOW.md:314`](../../how-to/AGENT_WORKFLOW.md#L314), `Inhabit` federation un-paused) and is distinct from `QD-ISLAND` ([`AGENT_WORKFLOW.md:310`](../../how-to/AGENT_WORKFLOW.md#L310), the `2026-11-19` island-rule science question, which remains unchanged).)*
> 1. **One authoritative host simulates**; many guests join over **LAN only** (Internet deferred to `I10`).
> 2. Guests may **watch (`Spectate`)** or **inhabit one animal or individual plant (`Inhabit`)** (grass cells are non-inhabitable populations in Milestone 1 per `D-PLANT-1`).
> 3. Every guest action reaches the host strictly as an intent converted into a declared `CauseId` intervention and recorded in the host's World Record (`WR1`/`WR2`).
> 4. **Design first**, while existing tracks continue (`Map v2` on macOS, `SEL2-P2` on Windows 1; `S7` deferred, not cancelled).
> 5. **Three-phase organism genesis (`QD-GENESIS`):** `G1` fixed `"designed founders"` (`live.founder_genotype = 1.0`, assigned `live.predator_fraction`, plus one fixed grass/shrub/tree genotype once `ADR-0015` lands) for the M1 demo -> `G2` screened founder pool + producers-first + multi-pool robustness check (`I11`) -> `G3` separate time-boxed emergent primordial research ADR (no chemistry origin of life). Phase is stamped in genesis manifest, `world_identity`, and manifest fingerprint; experiments never mix phases.
> 6. **Shared-island play rules (`QD-PLAYRULES`, DECIDED `2026-11-24`):** declared `"real-time ecology profile"` (`~1–2 h` real-time creature lifespan, pinned in manifest; faster-than-real-time play ONLY when zero guests are connected); **no respawn** on inhabited death (carcass decomposes via `V4`, guest enters `Spectate` at death site, may re-inhabit offspring / nearby young lineage member / host-allowed organism, zero carryover); total extinction never auto-revives (chronicle + host opens new `era_index` era); guest has zero extra stats/powers, environment interventions are strictly **host-only**, and host can remove (kick) any guest; stable `guest_id` / `island_id`, interest culling, record bounds, multi-guest-safe from day 1. *(Sexual recombination belongs to `D13` / `ADR-0013` amendment; plant mating to `D14` / `ADR-0015`; evolved channels to `D18` / `ADR-0017`.)*

---

## Execution Order & Lane Assignment (`anima-case` Lane W First)

```text
ADR-0014 accepted (2026-11-24, QD-ADR0014: 23 D-slots decided, ISLAND-D7 deferred to I10) -> merge #256 + per-card AUTHORIZATION
  ├──> I1 (Host policy, CauseId guest band, SharedGuestIntents ingress, genesis-phase guards) [anima-case Lane W]
  │      ├──> I2 (Inhabit actuation under total_cost, disconnect release, WR2 InputGuestIntent, G1 prehistory & replay) [anima-case Lane W]
  │      └──> I3 (Off-tick interest-area projection <=250 whole-island/cull, bincode ~20 Hz cap 16)                        [anima-case Lane W]
  │             └──> I4 (LAN-only anima-island-v1 WebSocket server, WorldIdentity handshake, beacon/PIN)               [anima-case Lane W]
  │                    ├──> I5 (Guest WebSocket client & read-only Tauri IPC bridge + ts-rs + TickSource)             [anima-case Lane W / desktop]
  │                    │      ├──> I6 (Shared-island UI, 3P/Spectate camera, intent marker, treatment/"designed founders" banner) [Frontend lane]
  │                    │      │      ├──> I6b (Sensory 1P camera overlay: animals + plants; deferred D-CAM-2(a))    [Frontend + Lane W]
  │                    │      │      └──> I7 (Multi-machine LAN host+guests soak & offline replay parity)             [Win 1 + Mac + Win 2]
  │                    │      └──> I8 (Deterministic off-tick Island Chronicle + 5-layer taxonomy + V7 panel)          [anima-case Lane W + UI]
  │                    │             └──> I9 (Opt-in host OpenRouter :free / localhost LLM narration via ureq/QD-WV25(c)) [Rust backend + UI]
  │                    ├─ (post-M1, gated on deferred ISLAND-D7 + NM-4) ─> I10 (Internet auth, TLS/WSS or QUIC, NAT/relay)
  │                    └─ (post-M1, D-GENESIS-1(b) >=3 pools + VIAB-2 + ADR-0015) ─> I11 (G2 screened founder pool & multi-pool check)
  │                    (G3 emergent primordial organisms belongs to a separate 2-week time-boxed ADR per D-GENESIS-2)
```

---

## Card `I1` — Guest Policy, Reserved `CauseId` Band & Zero-Alloc Intent Ingress (`QD-SHAREDISLAND`, `QD-GENESIS`)

- **Lane / Owner:** `anima-case` **Lane W** · `QD-SHAREDISLAND` / `QD-GENESIS` / `QD-PLAYRULES` (Peer reviewer: `anima-dell`)
- **START WHEN:** `PR #256` is merged and the lead issues explicit `AUTHORIZATION` for `I1` ([`ADR-0014`](../../decisions/ADR-0014-shared-island-host-and-guests.md) is `accepted` with `ISLAND-D5(a)` — pre-tick ingress throttling preserving `replayable: true` — decided).
- **ALLOWED PATHS (`proposed`):**
  - [`src-tauri/crates/anima-domain/src/causal.rs`](../../../src-tauri/crates/anima-domain/src/causal.rs)
  - [`src-tauri/src/core/observer.rs`](../../../src-tauri/src/core/observer.rs)
  - [`src-tauri/src/core/experiment.rs`](../../../src-tauri/src/core/experiment.rs)
  - [`src-tauri/src/core/experiment/manifest.rs`](../../../src-tauri/src/core/experiment/manifest.rs)
  - [`src-tauri/src/core/world_artifact.rs`](../../../src-tauri/src/core/world_artifact.rs)
  - [`src-tauri/src/core/experiment_runner/ensemble.rs`](../../../src-tauri/src/core/experiment_runner/ensemble.rs)
  - `src-tauri/src/core/island/mod.rs` *(to be created by `I1`)*
  - `src-tauri/src/core/island/intents.rs` *(to be created by `I1`)*
  - [`src-tauri/src/core/mod.rs`](../../../src-tauri/src/core/mod.rs)
  - [`src-tauri/src/core/simulation_schedule.rs`](../../../src-tauri/src/core/simulation_schedule.rs)
  - `src-tauri/tests/island_ingress_tests.rs` *(to be created by `I1`)*
- **Existing Anchors vs. New Symbols:**
  - Anchor [`causal.rs:17-39`](../../../src-tauri/crates/anima-domain/src/causal.rs#L17-L39) (`CauseId`, `CAUSE_BACKGROUND`, `CAUSE_OBSERVER`, `is_reserved_cause`).
  - Anchor [`observer.rs:49-116,433-478,627-645`](../../../src-tauri/src/core/observer.rs#L49-L116) (`ObserverPolicy`, `SharedObserverActions`, `drain_observer_actions_system`).
  - Anchor [`experiment/manifest.rs:103-145`](../../../src-tauri/src/core/experiment/manifest.rs#L103-L145) (`ExperimentManifest`, `validate`, `fingerprint`), [`simulation_schedule.rs:121-136`](../../../src-tauri/src/core/simulation_schedule.rs#L121-L136) (`build_tick_schedule` single pre-sensory drain boundary).
  - New symbols (*to be created by `I1`*): `ISLAND_GUEST_CAUSE_BASE`, stable `IslandId` & `GuestId`, `GuestRole`, `GuestIntent` (`ClaimOrganism`, `ReleaseOrganism`, `CreatureControl`, `PlantControl`), `GuestIntentMessage` (rejecting raw state frames AND rejecting any environment/macro intervention per `QD-PLAYRULES` Rule 5), `HostModerationAction::KickGuest`, `RealTimeEcologyProfile` pinned in manifest, `SharedGuestIntents` bounded queue, `drain_guest_intents_system`.
- **DONE WHEN:**
  - [`ExperimentManifest`](../../../src-tauri/src/core/experiment/manifest.rs#L103) refuses reserved guest `CauseId` claims and refuses island-session flags under `ObserverPolicy::Absent`; flipping island session presence or `"real-time ecology profile"` alters manifest fingerprint without changing legacy `WorldLawSet` runs; genesis phase (`G1`/`G2`/`G3`) is declared in manifest/`WorldIdentity` and reflected in `ExperimentManifest::fingerprint()`, refusing mixed-phase ensembles.
  - `cargo test --features desktop --test island_ingress_tests` passes: zero heap allocations during `drain_guest_intents_system`, environment interventions rejected from `GuestIntentMessage`, and `cargo test --features desktop` + `node scripts/check_test_targets.mjs --profile desktop` + `cargo clippy --features desktop --all-targets -- -D warnings` exit `0`.
- **Mandatory Negative Controls:**
  - `science_boundary_refuses_guest_session_in_absent_manifest`: manifest claiming `ISLAND_GUEST_CAUSE_BASE` or enabling guest ingress under `ObserverPolicy::Absent` — or mixing genesis phases within an experiment — returns `Err`.
  - `guest_cannot_write_state`: forged raw state payload (`SegmentState` / `AgentMigrationData`) OR guest environment intervention submitted to `GuestIntentMessage` decoder is rejected and counted before simulation queues.
  - `island_host_tick_zero_alloc`: triggering heap allocation inside `drain_guest_intents_system` fails the alloc-counter assertion (`allocs > 0`).

---

## Card `I2` — Host `Inhabit` Actuation, Disconnect Release, WR1/WR2 Recording & Replay (`QD-SHAREDISLAND`, `QD-GENESIS`)

- **Lane / Owner:** `anima-case` **Lane W** · `QD-SHAREDISLAND` / `QD-GENESIS` (`G1`) / `QD-PLAYRULES` (Peer reviewer: `anima-dell`)
- **START WHEN:** `I1` is merged (or stacked on `I1` per wave rule), `PR #256` merged, and explicit `AUTHORIZATION` issued (`QD-ADR0014` decided `ISLAND-D1(a)` immediate timeout release, `ISLAND-D2(a)` strict exclusive lock, `D-TIME-1 = 20 minutes`/day, `D-TIME-2 = 90 minutes`/year pinned in real-time ecology profile, `D-START-2(b)` headless prehistory, `D-START-3(a)` persistent island).
- **ALLOWED PATHS (`proposed`):**
  - [`src-tauri/src/core/agent_systems.rs`](../../../src-tauri/src/core/agent_systems.rs)
  - [`src-tauri/src/core/locomotion.rs`](../../../src-tauri/src/core/locomotion.rs)
  - [`src-tauri/src/core/simulation_schedule.rs`](../../../src-tauri/src/core/simulation_schedule.rs)
  - [`src-tauri/src/core/components.rs`](../../../src-tauri/src/core/components.rs)
  - [`src-tauri/src/core/simulation_lod.rs`](../../../src-tauri/src/core/simulation_lod.rs)
  - `src-tauri/src/core/island/inhabit.rs` *(to be created by `I2`)*
  - [`src-tauri/src/core/world_record/types.rs`](../../../src-tauri/src/core/world_record/types.rs)
  - [`src-tauri/src/core/world_record/ring.rs`](../../../src-tauri/src/core/world_record/ring.rs)
  - [`src-tauri/src/core/world_record/system.rs`](../../../src-tauri/src/core/world_record/system.rs)
  - [`src-tauri/src/core/world_record/writer.rs`](../../../src-tauri/src/core/world_record/writer.rs)
  - [`src-tauri/src/core/world_record/chain.rs`](../../../src-tauri/src/core/world_record/chain.rs)
  - [`src-tauri/tests/world_record_no_feedback_tests.rs`](../../../src-tauri/tests/world_record_no_feedback_tests.rs)
  - `src-tauri/tests/island_determinism_tests.rs` *(to be created by `I2`)*
- **Existing Anchors vs. New Symbols:**
  - Anchor [`agent_systems.rs:135,1289-1293,1447,1911-1965`](../../../src-tauri/src/core/agent_systems.rs#L1911-L1965) (`ReclaimAndDespawnAgentCommand`, `total_cost`, `action_resolution_system`, `LastTransitionState`), [`locomotion.rs:190,318`](../../../src-tauri/src/core/locomotion.rs#L190-L318) (`steering_drive_system`), [`simulation_schedule.rs:139-143`](../../../src-tauri/src/core/simulation_schedule.rs#L139-L143), [`components.rs:464-465,501-532`](../../../src-tauri/src/core/components.rs#L464-L532) (`InertiaComponent::target_velocity`, `ActionGates`), [`ADR-0013:40`](../../decisions/ADR-0013-live-creature-lifecycle.md#L40) (`V4` carcass/decomposition).
  - Anchor [`world_record/types.rs:132-184,264`](../../../src-tauri/src/core/world_record/types.rs#L132-L184) (`WorldRecordHeader`, `InputKind`, `keyframe_snapshot_sha256`), [`ring.rs:327-339,643`](../../../src-tauri/src/core/world_record/ring.rs#L327-L339) (`RingEntry`, `spsc_ring`), [`system.rs:40,882-939`](../../../src-tauri/src/core/world_record/system.rs#L882-L939) (`DEFAULT_MAX_INPUTS_PER_SEGMENT`, `record_world_record_tick_system`).
  - New symbols (*to be created by `I2`*): `InhabitedControl` component (overriding `action_resolution_system` `cpg_parameters`/`ActionGates` + `steering_drive_system` `target_velocity`, exempting leased entity from LOD freeze with **zero extra speed/strength/senses/immunity**, plus plant hook stub compatible with `ADR-0015`), `InputKind::GuestIntent`, `Copy` `RingEntry::InputGuestIntent` (change-only coalescing + Claim/Release/Kick/Death boundary markers), stable `island_id` + `GuestSessionRecord` + `G1` `"designed founders"` genesis-phase provenance in `WorldRecordHeader`, deterministic disconnect/kick release handler, inhabited-death handler (no respawn, `V4` carcass, revert to `Spectate` at death site, zero carryover), faster-than-real-time gate (`0` connected guests only), extinction-new-era handler (`era_index`), headless `G1` `prehistory_ticks` era initialization (`D-START-2(b)`), and offline island `WorldRecord` replay harness.
- **DONE WHEN:**
  - `cargo test --features desktop --test island_determinism_tests` passes `host_with_zero_guests_matches_solo_bit_for_bit` (both default & learning worlds at checkpoints `[0, 180, 1_800, 18_000]`), `guest_intent_recorded_and_replayed` (`end_checksum` + `CausalLedger` `CauseId` parity with networking disabled), `disconnect_releases_creature` (covering disconnect, host kick, and inhabited death -> `V4` carcass + `Spectate` + zero carryover), `faster_than_real_time_blocked_when_guests_connected`, and zero tick heap allocations.
  - Full `cargo test --features desktop`, `node scripts/check_test_targets.mjs --profile desktop`, and `cargo clippy --features desktop --all-targets -- -D warnings` pass.
- **Mandatory Negative Controls:**
  - `host_without_guests_matches_solo` fails if a single non-empty `ClaimOrganism` + intent is applied.
  - `guest_intent_recorded_and_replayed` fails `verify_record` or checksum equality when one `RecordedInput` entry or byte is altered.
  - `disconnect_releases_creature` fails if release cleanup is skipped, if an inhabited death respawns/carries over stats, or if an inhabited entity exceeds lineage speed/energy/sensory limits.

---

## Card `I3` — Off-Tick Interest-Area Projection & `IslandSnapshot`/`IslandDelta` Encoder (`QD-SHAREDISLAND`)

- **Lane / Owner:** `anima-case` **Lane W** · `QD-SHAREDISLAND` / `QD-PLAYRULES` (Peer reviewer: `anima-dell`)
- **START WHEN:** `I1` merged, `PR #256` merged & explicit `AUTHORIZATION` issued (`QD-ADR0014` decided `ISLAND-D3(a)` `bincode` quantized deltas at `~20 Hz` & cap `16` guests, `ISLAND-D4(b)` whole-island `<= 250` agents then radius cull, `D-CAM-1(a)`, `D-CAM-3(a)`).
- **ALLOWED PATHS (`proposed`):**
  - [`src-tauri/src/core/emit.rs`](../../../src-tauri/src/core/emit.rs)
  - [`src-tauri/src/core/simulation_state.rs`](../../../src-tauri/src/core/simulation_state.rs)
  - [`src-tauri/src/core/simulation_loop.rs`](../../../src-tauri/src/core/simulation_loop.rs)
  - `src-tauri/src/core/island/stream.rs` *(to be created by `I3`)*
  - `src-tauri/tests/island_stream_tests.rs` *(to be created by `I3`)*
- **Existing Anchors vs. New Symbols:**
  - Anchor [`simulation_state.rs:23-56,459`](../../../src-tauri/src/core/simulation_state.rs#L23-L56) (`SegmentState`, `SimulationTickPayload`, `SavedSimulationState`), [`emit.rs:52-95,115-136,184-273`](../../../src-tauri/src/core/emit.rs#L52-L95) (`refresh_tick_payload`, `PheromoneEmitGate`, `spawn_emit_thread`), [`snapshot.rs:58,74`](../../../src-tauri/src/core/snapshot.rs#L58-L74) (stripped instead of full `SavedSimulationState`).
  - New symbols (*to be created by `I3`*): `IslandJoinSnapshot` (`island_id`, stable `guest_id`s, omitting `SimRng`/brain weights), `IslandSnapshot`, `IslandDelta`, interest-area spatial filter (`ISLAND-D4`), quantized `SegmentDelta` (`bincode` framing per `ISLAND-D3`), slow-consumer drop/coalesce fan-out ring (zero backpressure onto simulation tick).
- **DONE WHEN:**
  - `cargo test --features desktop --test island_stream_tests` passes round-trip fidelity, stripped `SimRng`/weight omission checks, and `host_with_spectate_guests_matches_solo` (`1,800` ticks bit-identical with active off-tick projection).
  - Measured `1,000`-agent serialized stream byte size fits well below `1,209,987 B`/tick baseline JSON reported in `C11`.
- **Mandatory Negative Controls:**
  - `host_with_spectate_guests_matches_solo`: routing spectator camera coordinates into host `SharedLodFocus` diverges `world_checksum` and fails the gate.
  - Serialization unit test confirms `SavedSimulationState` private RNG cursor / brain weights cannot appear in `IslandJoinSnapshot`.

---

## Card `I4` — LAN-Only `anima-island-v1` WebSocket Host Server & Discovery (`QD-SHAREDISLAND`)

- **Lane / Owner:** `anima-case` **Lane W** · `QD-SHAREDISLAND` / `QD-PLAYRULES` (Peer reviewer: `anima-dell`)
- **START WHEN:** `I2` and `I3` merged, `PR #256` merged & explicit `AUTHORIZATION` issued (`QD-ADR0014` decided `D-START-1(a)` `256²` then WV2 `C01` after `WV2-7`, and `ISLAND-D6(a)` `IP:port + PIN` + UDP LAN beacon; `ISLAND-D7` stays deferred to `I10`).
- **ALLOWED PATHS (`proposed`):**
  - [`src-tauri/src/core/networking_systems.rs`](../../../src-tauri/src/core/networking_systems.rs) *(only shared constants/helpers if needed; migration `anima-migration-v1` untouched)*
  - `src-tauri/src/core/island/host_server.rs` *(to be created by `I4`)*
  - `src-tauri/src/core/island/lan_guard.rs` *(to be created by `I4`)*
  - [`src-tauri/Cargo.toml`](../../../src-tauri/Cargo.toml) *(only `[[test]] required-features = ["networking"]` entries if adding `networking`-gated integration tests; zero new crates per `ISLAND-D6(a)`)*
  - `src-tauri/tests/island_transport_tests.rs` *(to be created by `I4`)*
- **Existing Anchors vs. New Symbols:**
  - Anchor [`Cargo.toml:56-58,73-77,242`](../../../src-tauri/Cargo.toml#L56-L77) (`socket2`, `tokio`, `tokio-tungstenite`, `futures-util`, `crossbeam-channel`, `bincode`, `networking` feature), [`world_artifact.rs:122,136`](../../../src-tauri/src/core/world_artifact.rs#L122-L136) (`WorldIdentity`, `sim_world_sha256`).
  - New symbols (*to be created by `I4`*): `run_island_websocket_host` (`anima-island-v1`), `WorldIdentity` (`D-START-1`) + `island_id` + stable `guest_id` + PIN handshake validator, LAN-subnet bind/peer guard, heartbeat keepalive & disconnect/host-kick trigger into `SharedGuestIntents`, optional UDP LAN beacon (`ISLAND-D6(a)`).
- **DONE WHEN:**
  - `cargo test --features desktop --test island_transport_tests` passes loopback multi-guest connect, `WorldIdentity` mismatch rejection, PIN rejection, LAN-only bind enforcement, slow-consumer isolation, idle `Spectate` survival past `5 s` (avoiding [`networking_systems.rs:208`](../../../src-tauri/src/core/networking_systems.rs#L208)), host kick removal, and mid-inhabit disconnect release.
  - Default feature build (`cargo check`) compiles cleanly without `tokio-tungstenite`.
- **Mandatory Negative Controls:**
  - `lan_only_rejects_public_bind`: public/WAN bind or non-LAN peer without Internet gate returns `Err`.
  - `WorldIdentity` mismatch (`sim_world_sha256` altered by 1 bit) or wrong PIN refuses WebSocket upgrade/session.

---

## Card `I5` — Guest WebSocket Client, Read-Only IPC Bridge & Adaptive Playout Buffer (`QD-SHAREDISLAND`)

- **Lane / Owner:** `anima-case` **Lane W / desktop** · `QD-SHAREDISLAND` (Peer reviewer: `anima-dell`)
- **START WHEN:** `I4` merged & explicit `AUTHORIZATION` issued (`QD-ADR0014` decided `D-ANIM-1(a)` adaptive `2–3` server stream ticks playout buffer: `~100–150 ms` at `20 Hz`, `~66–100 ms` at `30 Hz`).
- **ALLOWED PATHS (`proposed`):**
  - `src-tauri/src/core/island/guest_client.rs` *(to be created by `I5`)*
  - `src-tauri/src/commands/island.rs` *(to be created by `I5`)*
  - [`src-tauri/src/commands/mod.rs`](../../../src-tauri/src/commands/mod.rs)
  - [`src-tauri/src/lib.rs`](../../../src-tauri/src/lib.rs)
  - [`docs/reference/IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md)
  - `src/types/generated/` *(regenerated `ts-rs` bindings)*
  - [`src/components/Landscape/agentInterpolation.ts`](../../../src/components/Landscape/agentInterpolation.ts)
  - [`src/components/Landscape/utils/tickRecording.ts`](../../../src/components/Landscape/utils/tickRecording.ts)
  - `src-tauri/tests/island_guest_bridge_tests.rs` & `tests/frontend/islandGuestInterpolation.test.ts` *(to be created by `I5`)*
- **Existing Anchors vs. New Symbols:**
  - Anchor [`lib.rs:63-99`](../../../src-tauri/src/lib.rs#L63-L99) (`generate_handler!`), [`IPC_CONTRACT.md:105-113`](../../reference/IPC_CONTRACT.md#L105-L113), [`agentInterpolation.ts:20,69,85,725-749`](../../../src/components/Landscape/agentInterpolation.ts#L20-L85) (`TICK_RENDER_DELAY_MS = 33`), [`tickRecording.ts:45-53`](../../../src/components/Landscape/utils/tickRecording.ts#L45-L53) (`TickSource`).
  - New symbols (*to be created by `I5`*): `IslandGuestClient`, Tauri commands (`host_shared_island`, `join_shared_island`, `leave_shared_island`, `send_island_guest_intent`, `list_lan_islands`), adaptive jitter playout buffer (`D-ANIM-1`), `'island'` `TickSourceKind` adapter feeding read-only viewer events (`simulation-tick`, `combat-event`, etc.) while keeping local `SimulationEngine` stopped on guests.
- **DONE WHEN:**
  - `cargo test --features desktop --test island_guest_bridge_tests`, `npm run check:docs` (including `check_ipc_reference.mjs`), `npm run check:ipc`, `npm run lint`, and `npm run test:frontend` exit `0`.
- **Mandatory Negative Controls:**
  - Missing `IPC_CONTRACT.md` row or snake_case IPC arg fails `npm run check:docs` / `npm run check:ipc` (exit `1`).
  - Guest node attempting to toggle local simulation loop while connected in `'island'` mode is rejected.

---

## Card `I6` — Shared-Island LAN UI, 3rd-Person/Spectator Camera & Treatment Banner (`QD-SHAREDISLAND`, `QD-GENESIS`)

- **Lane / Owner:** Frontend / UI lane · `QD-SHAREDISLAND` / `QD-GENESIS` (`G1`) (Peer reviewer: cross-machine per `AGENT_WORKFLOW.md`)
- **START WHEN:** `I5` merged & explicit `AUTHORIZATION` issued (`QD-ADR0014` decided `D-ANIM-2(a)` prediction `OFF`, `D-CAM-1(a)` 3P follow/orbit, `D-CAM-3(a)` free spectator, `D-PLANT-1(a)` individual trees & shrubs only).
- **ALLOWED PATHS (`proposed`):**
  - `src/components/Island/` *(to be created by `I6`)*
  - [`src/App.tsx`](../../../src/App.tsx) *(routing `TickSource` & Host/Join island bar; note [`LiveAgents.tsx`](../../../src/components/Landscape/LiveAgents.tsx#L57) already consumes `TickSource`)*
  - [`src/PixiViewport.tsx`](../../../src/PixiViewport.tsx) *(third-person follow/orbit camera `D-CAM-1`, free spectator camera `D-CAM-3`, local intent direction arrow/marker)*
  - [`src/components/Landscape/utils/agentPose.ts`](../../../src/components/Landscape/utils/agentPose.ts) *(read-only pose consumer, preserving `CREATURE_BODY_SCALE = 0.32` at `:26`)*
  - `tests/frontend/islandUi.test.tsx` *(to be created by `I6`)*
- **DONE WHEN:**
  - UI exposes Host LAN session (`IP:port + PIN`), Join LAN Island, `Spectate` free camera, `Inhabit` animal or individual plant (`ADR-0015`) 3P follow/orbit camera (plus death-site `Spectate` & offspring/young-lineage re-claim prompt per `QD-PLAYRULES` Rule 3, host kick/remove control per Rule 5, and new-era trigger on total extinction per Rule 4), immediate local intent arrow (`no physics prediction` per `D-ANIM-2`), persistent `ADR-0004` treatment badge, and active genesis phase badge (`"designed founders"` under `G1`); zero simulation math or physics inside `src/` (`ADR-0012` `U2`).
  - `npm run lint`, `node scripts/eslint_ratchet.mjs`, `npm run test:frontend`, `npm run build`, `npm run check:bundle`, `npm run check:csp`, `npm run check:ipc`, and `npm run check:docs` exit `0`.
- **Mandatory Negative Controls:**
  - Guest UI renders intent marker immediately without mutating cached segment poses locally; static check fails if frontend imports any Rust simulation logic or mutates world state outside `send_island_guest_intent`.

---

## Card `I6b` — Sensory First-Person View Mode (`D-CAM-2`, Animals & Plants, `QD-SHAREDISLAND`)

- **Lane / Owner:** `anima-case` Lane W + Frontend lane · `QD-SHAREDISLAND` (Peer reviewer: cross-machine)
- **START WHEN:** `I6` merged and explicit `AUTHORIZATION` issued (`QD-ADR0014` `D-CAM-2(a)` deferred to optional post-M1 `I6b`; for plant sensory overlays, also after `ADR-0015` (*evolving plants, D14*) plant organism pipeline lands).
- **ALLOWED PATHS (`proposed`):**
  - `src-tauri/src/core/island/stream.rs` *(focused single-organism sensor stream extension)*
  - `src/components/Island/` & `src/PixiViewport.tsx` *(sensory raycast/olfactory/plant-signal overlay)*
  - `src-tauri/tests/island_sensory_view_tests.rs` *(to be created by `I6b`)*
- **Existing Anchors:** [`components.rs:40`](../../../src-tauri/src/core/components.rs#L40) (`RaycastTelemetry`), [`agent_systems.rs:1633-1814`](../../../src-tauri/src/core/agent_systems.rs#L1633-L1814) (`sensory_system`, `OlfactorySensors`, range `10.0`).
- **DONE WHEN:**
  - Host streams sensor data **only** for the guest's single inhabited organism (animals: `RaycastTelemetry` + olfactory signals actually fed into brain inputs; individual plants: light, soil water, nutrients, grazing damage); non-inhabited guests receive zero extra sensory bytes.
  - All Rust (`cargo test --features desktop`) and Frontend (`npm run test:frontend`, `npm run build`) checks exit `0`.
- **Mandatory Negative Control:** Broadcasting `RaycastTelemetry` for all `1,000` agents when `D-CAM-2` is requested by `1` guest fails the per-guest bandwidth assertion test.

---

## Card `I7` — Multi-Machine LAN Soak & Offline Replay Parity Verification (`QD-SHAREDISLAND`)

- **Lane / Owner:** Multi-machine verification · `QD-SHAREDISLAND` (`Windows 1` host + `macOS` & `Windows 2` guests; Peer reviewer: cross-machine)
- **START WHEN:** `I6` merged & explicit `AUTHORIZATION` issued.
- **ALLOWED PATHS (`proposed`):**
  - `scripts/verify_island_lan.mjs` *(or dedicated verification harness + result note under `docs/ai/planning/`, `to be created by I7`)*
- **DONE WHEN:**
  - Real LAN multi-machine run (different OS families, e.g., Windows 1 host + macOS/Windows 2 guests) completes join, concurrent `Spectate` + `Inhabit`, abrupt client kill/disconnect (`disconnect_releases_creature`), host kick, and host restart persistence (`D-START-3`); offline host replay of the saved `WorldRecord` matches every segment `end_checksum` bit-for-bit.
- **Mandatory Negative Control:** Replaying with altered `GuestIntent` input fails `end_checksum` comparison at the exact modified segment.

---

## Card `I8` — Off-Tick Island Chronicle & Five-Layer Classification (`QD-SHAREDISLAND`, `QD-GENESIS`)

- **Lane / Owner:** `anima-case` **Lane W** + Frontend V7 timeline reuse · `QD-SHAREDISLAND` / `QD-GENESIS` / `QD-PLAYRULES` (`MSG_20261124_120000` & `121500`; Peer reviewer: cross-machine)
- **START WHEN:** `I2` + `I6` merged (or after `WR2` + `I2` session metadata for backend chronicle builder) & explicit `AUTHORIZATION` issued (`QD-ADR0014` decided `D-START-2(b)` headless prehistory, `D-CHRON-1` `§6` triggers, `D-CHRON-2(a)` `50`-epoch compaction, `D-CHRON-3` concrete clade threshold & naming scheme submitted in `ADR-0014` for lead approval under delegation).
- **ALLOWED PATHS (`proposed`):**
  - `src-tauri/src/core/island/chronicle.rs` *(to be created by `I8`)*
  - `src-tauri/src/commands/island_chronicle.rs` *(to be created by `I8`)*
  - [`src-tauri/src/commands/mod.rs`](../../../src-tauri/src/commands/mod.rs)
  - [`src-tauri/src/evolution/lineage.rs`](../../../src-tauri/src/evolution/lineage.rs)
  - [`src-tauri/src/lib.rs`](../../../src-tauri/src/lib.rs) & [`docs/reference/IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md)
  - `src/components/Island/` *(arrival chronicle & tree-of-life panel reusing [`2026-11-24-v7-causal-timeline-ui-spec.md`](2026-11-24-v7-causal-timeline-ui-spec.md))*
  - `src-tauri/tests/island_chronicle_tests.rs` & `tests/frontend/islandChronicle.test.tsx` *(to be created by `I8`)*
- **Existing Anchors vs. New Symbols:**
  - Anchor [`evolution/lineage.rs:7,236,305,308,595`](../../../src-tauri/src/evolution/lineage.rs#L236-L308) (`LineageTracker`, `compact`), [`commands/evolution.rs:65-125,159,238`](../../../src-tauri/src/commands/evolution.rs#L65-L125) (`LineageGraphPayload`, `LineageMrcaPayload`, `get_lineage_mrca`), [`live_experiment.rs:116,277`](../../../src-tauri/src/core/live_experiment.rs#L116-L277) (`live.predator_fraction`, `live.lineage_tracking`), [`components.rs:66-69`](../../../src-tauri/src/core/components.rs#L66-L69) (`AgentType { Predator, Prey }`), [`EVOLUTION_EXPERIMENT_CONTRACT.md:308,311`](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md#L308-L311) (`AE-S11`, `AE-S14`), [`STATE_OF_THE_PROJECT.md §3.15`](../../planning/STATE_OF_THE_PROJECT.md#L1547-L1590).
  - New symbols (*to be created by `I8`*): `IslandChronicleEntry` (`tick`, `segment_index`, `cause_id`, `chapter_kind`, `genesis_phase`), off-tick deterministic chronicle builder (zero simulation-tick allocation), importance ranker + followed/pinned guest view settings (off simulation state), total-extinction & new-era chapter recorder (`QD-PLAYRULES` Rule 4), extinct-branch summary compactor (`D-CHRON-2(a)`: `COMPACT_EVERY_EPOCHS = 50`), deterministic clade auto-namer (`<founder_short_id>-<body_plan_code>-C<monotonic_index>` at `>= 5` living / `>= 5%` if `< 100` min `3` over `>= 3` gens) & off-sim nickname sidecar (`D-CHRON-3`), 5-layer classifier (`Lineage`, `Clade`, `"assigned role"`, `Body plan`, `AE-S11`-gated `Species`).
- **DONE WHEN:**
  - Every chapter entry carries valid `(tick, segment_index)` jump links to `WorldRecord`, summarizes prehistory on guest arrival, records total island extinction & new-era openings, labels `G1` runs explicitly as `"designed founders"` and stamps active genesis phase provenance, labels `AgentType` strictly as `"assigned role"` (never evolved diet), never emits `"species"` / `"loài"` without `AE-S11`/`AE-S14`, compacts extinct unpinned branches cleanly, and incurs `0` tick-thread allocations.
  - `cargo test --features desktop --test island_chronicle_tests`, `npm run check:docs`, `npm run check:ipc`, `npm run test:frontend`, and `npm run lint` pass.
- **Mandatory Negative Controls:**
  - `chronicle_entries_cited_and_species_label_gated`: submitting an entry missing `(tick, segment_index)` or tagging a morphological/`AgentType` cluster as `"species"` / `"loài"` when `AE-S11` has not passed fails validation.

---

## Card `I9` — Opt-In Host-Side OpenAI-Compatible LLM Chronicle Narration (`D-LLM-1`, `QD-SHAREDISLAND`)

- **Lane / Owner:** Rust backend + UI lane · `QD-SHAREDISLAND` (Peer reviewer: cross-machine)
- **START WHEN:** `I8` merged & explicit `AUTHORIZATION` issued (`QD-ADR0014` decided `D-LLM-1`: **OpenRouter `:free`** opt-in on the host via provider-agnostic OpenAI-compatible endpoint with API key in env var/OS keychain, plus supported **`localhost`** zero-key alternative; reuse existing `ureq` preferred, new crate follows `QD-WV25 (c)` `cargo deny`/`cargo audit` gate).
- **ALLOWED PATHS (`proposed`):**
  - `src-tauri/src/core/island/llm_narration.rs` *(to be created by `I9`, gated behind explicit opt-in Cargo feature/setting)*
  - `src-tauri/src/commands/island_chronicle.rs` & [`src-tauri/src/lib.rs`](../../../src-tauri/src/lib.rs) & [`docs/reference/IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md)
  - [`src-tauri/Cargo.toml`](../../../src-tauri/Cargo.toml) *(only if feature flag added; prefer existing `ureq` at `:74`; any new HTTP crate follows `QD-WV25 (c)`)*
  - `src/components/Island/` *(`"AI-written summary"` badged view with model ID, provider, date)*
  - `src-tauri/tests/island_llm_narration_tests.rs` & [`docs/paper/AI_USE_LOG.md`](../../paper/AI_USE_LOG.md) *(only if ever used in published experiments/outputs per `QĐ-P3`)*
- **DONE WHEN:**
  - Single provider-agnostic OpenAI-compatible `/chat/completions` client configured by `base_url`, `model_id` (no hard-coded ID), and env/OS-keychain key (empty key valid for `localhost`), strictly off by default on host only.
  - Prompt contains **only** `I8` cited facts (guest display names omitted without explicit per-guest consent; zero API keys or host filesystem paths sent); any returned sentence lacking a verified `(tick, segment_index)` citation from the input list is dropped.
  - Results cached by canonical SHA-256 of the input fact list alongside `(model_id, provider, date)`, executed off-tick asynchronously with fallback to plain facts on error/rate limit, never written to `WorldRecord`.
  - `llm_narration_disposable_and_csp_clean`, `npm run check:csp`, `cargo audit` / license gates, `npm run check:docs`, `npm run test:frontend`, and `cargo test --features desktop --test island_llm_narration_tests` exit `0`.
- **Mandatory Negative Controls:**
  - Uncited LLM response sentence is dropped by parser test.
  - Adding an external domain directly to frontend CSP / `fetch()` fails `npm run check:csp` (exit `1`).
  - Narration on vs. off produces identical `WorldRecord` segment hashes and `world_checksum`.

---

## Card `I10` — Deferred Internet Milestone (`ISLAND-D7`, `QD-SHAREDISLAND`, Post-Milestone-1)

- **Lane / Owner:** Deferred post-M1 (`QD-SHAREDISLAND` Internet milestone) until `I1`–`I8` pass cleanly and owner decides `ISLAND-D7` + `NM-4` / `QĐ-M3`
- **START WHEN:** `I7` green, post-M1 Internet milestone prioritized, and owner resolves deferred `ISLAND-D7` + `NM-4` / `QĐ-M3` (auth model, TLS/WSS vs. embedded peer QUIC, NAT/relay topology, shared evolved checkpoint sharing `D-START-2(c)`).
- **ALLOWED PATHS (`proposed`):** To be specified in a dedicated post-Milestone-1 card spec once `ISLAND-D7` closes (`docs/ai/planning/` + `src-tauri/src/core/island/`).
- **DONE WHEN:** Authenticated encrypted transport (`TLS` or `QUIC`), NAT traversal/relay, rate limiting, and `D-START-2(c)` checkpoint verification pass without breaking LAN or Solo regression gates.
- **Mandatory Negative Control:** Unauthenticated or plaintext remote peer attempting Internet host join is rejected at TLS/auth handshake.

---

## Card `I11` — `G2` Screened Founder Pool, Producers-First & Multi-Pool Robustness Check (`QD-SHAREDISLAND`, `QD-GENESIS`)

- **Lane / Owner:** `anima-case` **Lane W** / experiment-contract lane · `QD-SHAREDISLAND` / `QD-GENESIS` (`G2` upgrade; Peer reviewer: cross-machine)
- **START WHEN:** `I1` & `I2` merged (`G1` baseline operational), `ADR-0015` (*evolving plants, `D14`*) plant founders landed, `VIAB-2` calibration complete, and explicit `AUTHORIZATION` issued (`QD-ADR0014` decided `D-GENESIS-1(b)`: measured closed-EU viability trigger per trophic level, `VIAB-2`-calibrated `N` generations, robustness check across **`>= 3` founder pools**). *(Note: `G3` emergent primordial organisms is time-boxed to **`2 weeks` of agent work** in its own separate opt-in ADR per `D-GENESIS-2`, outside `I11`.)*
- **ALLOWED PATHS (`proposed`):**
  - [`src-tauri/src/core/live_experiment.rs`](../../../src-tauri/src/core/live_experiment.rs) *(genesis phase flag & screened founder pool dispatch; `G1` / legacy paths unchanged when off)*
  - `src-tauri/src/core/island/genesis_pool.rs` *(to be created by `I11`)*
  - [`src-tauri/src/core/resources.rs`](../../../src-tauri/src/core/resources.rs) *(dedicated genesis screening RNG stream constant)*
  - `src-tauri/tests/island_genesis_pool_tests.rs` *(to be created by `I11`)*
  - `docs/ai/planning/` *(multi-pool founder robustness check result note, to be created by `I11`)*
- **Existing Anchors:** [`live_experiment.rs:88,96,112-116,3022`](../../../src-tauri/src/core/live_experiment.rs#L3022), [`resources.rs:947`](../../../src-tauri/src/core/resources.rs#L947) (`LIVE_GENESIS_FOUNDERS`), [`mutation.rs:70,211`](../../../src-tauri/src/evolution/mutation.rs#L70-L211), [`2026-11-23-viab1-results.md`](2026-11-23-viab1-results.md), [`2026-11-24-sel2-p2-preregistration-draft.md`](2026-11-24-sel2-p2-preregistration-draft.md#L209-L216) (`VIAB-2`).
- **DONE WHEN:**
  - Declared genotype distribution samples candidate founders on a dedicated seeded RNG stream (never touching the ecology stream), filters candidates through a closed-EU `N`-generation viability screen, seeds producers (`ADR-0015` plants) first followed by herbivores then predators (`live.predator_fraction` becomes legacy once evolved diets operate), stamps `G2` into the genesis manifest, `WorldIdentity`, and `ExperimentManifest::fingerprint()`, and passes an experiment-contract-style robustness check across multiple distinct founder pools (`>= 3`, `D-GENESIS-1(b)`).
  - `cargo test --features desktop --test island_genesis_pool_tests`, full `cargo test --features desktop`, `node scripts/check_test_targets.mjs --profile desktop`, and `cargo clippy --features desktop --all-targets -- -D warnings` exit `0`.
- **Mandatory Negative Controls:**
  - `science_boundary_rejects_mixed_genesis_phases`: experiment ensemble mixing `G1` (`"designed founders"`) and `G2` (`screened pool`) runs returns `Err`.
  - Producers-first ordering check fails if herbivores or predators spawn before the lower trophic level establishes per `D-GENESIS-1`; robustness report check fails if conclusions rely on a single founder pool only.
