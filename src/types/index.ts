// IPC contract types.
//
// Everything under `./generated/` is produced from the Rust structs by ts-rs and MUST NOT be edited
// by hand — regenerate with `cargo test --lib export_bindings` from `src-tauri/`. CI regenerates and
// diffs, so a Rust struct that changes without its TypeScript changing fails the build.
//
// This file used to hand-maintain all of these. That is how `head_directions` shipped broken: the
// Rust side sends a `HashMap<u32, [f32; 3]>` (a JSON object), a stale `HeadDirectionTelemetry`
// interface here described an array element that the backend has never sent, `App.tsx` was written
// against that array, and the test mocks agreed with `App.tsx` rather than with the backend. Three
// hand-written mirrors all agreeing with each other and none with the producer — a generated type
// makes that a compile error instead of a silently dead feature (G1.4).

// ---- Generated from Rust. Do not hand-edit; see ./generated/ -----------------------------------
export type { SegmentState } from './generated/SegmentState';
export type { SimulationTickPayload } from './generated/SimulationTickPayload';
export type { SimulationStatus } from './generated/SimulationStatus';
export type { ChronicleEvent } from './generated/ChronicleEvent';
export type { EnvironmentalElement } from './generated/EnvironmentalElement';
export type { EnvironmentalState } from './generated/EnvironmentalState';
export type { RaycastTelemetry } from './generated/RaycastTelemetry';
export type { CombatEvent } from './generated/CombatEvent';
export type { HitEntityType } from './generated/HitEntityType';
export type { AgentType } from './generated/AgentType';
export type { PheromoneGridState } from './generated/PheromoneGridState';
export type { EvolutionSettings } from './generated/EvolutionSettings';
export type { EliteIndividualState } from './generated/EliteIndividualState';
export type { MapElitesGridState } from './generated/MapElitesGridState';
export type { EcosystemState } from './generated/EcosystemState';

// ---- Not generated ------------------------------------------------------------------------------
// These have no single Rust struct to derive from: they are either assembled ad hoc by a command, or
// they are frontend-only view models that never cross IPC. Each one that IS an IPC payload is a
// remaining source of exactly the drift described above and should get a Rust struct plus a derive.

// The migration and lineage payloads used to be hand-written here, with `TODO: derive` beside each.
// They now have `ts_rs::TS` derives on their Rust sources, so they are re-exported from the generated
// directory instead — `ipcBindingAuthority.test.ts` fails on a re-declaration, which is what makes
// "re-export" the only option rather than the polite one.
//
// The copy `status: string` was carrying is the reason this mattered: the Rust field really was a
// `String`, while `App.tsx`'s copy declared `'Success' | 'Failed'`. Two mirrors of one struct, each
// wrong in a different direction, and nothing comparing either to the source. Both are now
// `MigrationStatus`, generated from an enum.
export type { MigrationPayload } from './generated/MigrationPayload';
export type { MigrationDirection } from './generated/MigrationDirection';
export type { MigrationStatus } from './generated/MigrationStatus';
export type { LineageNodePayload } from './generated/LineageNodePayload';
export type { LineageLinkPayload } from './generated/LineageLinkPayload';
export type { LineageGraphPayload } from './generated/LineageGraphPayload';

/// IPC. `get_terrain_map`. TODO: derive.
export interface TerrainMapState {
  width: number;
  height: number;
  biomes: number[];
  elevations?: number[];
  moistures?: number[];
  temperatures?: number[];
  bounds: {
    min: { x: number; y: number; z: number };
    max: { x: number; y: number; z: number };
  };
  pois: [number, number][];
}

// Frontend-only view models. Built in the browser from `SegmentState`; they never cross IPC, so
// there is no Rust counterpart and nothing to keep in sync.
export interface RenderSegment {
  segment_id: number;
  x: number;
  y: number;
  z: number;
  yaw: number;
  pitch: number;
  roll: number;
  joint_anchor: [number, number, number] | null;
  children: RenderSegment[];
}

export interface AgentHierarchy {
  agent_id: number;
  energy: number;
  root: RenderSegment;
}

// Aliases kept for existing call sites; identical to the generated `*Payload` forms re-exported
// above. An alias is not a re-declaration — it has no field list of its own to drift.
export type { LineageNodePayload as LineageNode } from './generated/LineageNodePayload';
export type { LineageLinkPayload as LineageLink } from './generated/LineageLinkPayload';
