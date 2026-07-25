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

/// IPC. Emitted by `migration-event`; assembled inline in `networking_systems.rs` rather than from a
/// named struct, so there is nothing to derive from yet. TODO: give it one.
export interface MigrationPayload {
  agent_id: number;
  direction: 'incoming' | 'outgoing';
  source_port: number;
  target_port: number;
  status: string;
  timestamp: number;
}

/// IPC. `get_lineage_graph` builds this from the Neo4j/in-memory tracker. TODO: derive.
export interface LineageNodePayload {
  id: string;
  generation: number;
  parent_id: string | null;
  fitness: number;
  mutations_count: number;
}

export interface LineageLinkPayload {
  source: string;
  target: string;
}

export interface LineageGraphPayload {
  nodes: LineageNodePayload[];
  links: LineageLinkPayload[];
  db_connected: boolean;
}

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

// Aliases kept for existing call sites; identical to the *Payload forms above.
export type LineageNode = LineageNodePayload;
export type LineageLink = LineageLinkPayload;
