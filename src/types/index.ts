export interface SegmentState {
  agent_id: number;
  segment_id: number;
  parent_segment_id: number | null;
  x: number;
  y: number;
  z: number;
  yaw: number;
  pitch: number;
  roll: number;
  joint_anchor_x: number;
  joint_anchor_y: number;
  joint_anchor_z: number;
  joint_axis_x: number;
  joint_axis_y: number;
  joint_axis_z: number;
  energy: number;
  agent_type?: 'predator' | 'prey';
  hydration?: number;
  head_direction?: [number, number, number];
}

export interface EnvironmentalElement {
  type: 'lake' | 'tree' | string;
  x: number;
  y: number;
  radius: number;
  resources: number;
}

export interface EnvironmentalState {
  elements: EnvironmentalElement[];
}

export interface HeadDirectionTelemetry {
  agent_id: number;
  direction: [number, number, number];
}

export interface SimulationTickPayload {
  segments: SegmentState[];
  environmental_state: EnvironmentalState;
  head_directions: { [key: number]: [number, number, number] };
}

export interface RaycastTelemetry {
  origin: [number, number, number];
  direction: [number, number, number];
  hit_distance: number;
  hit_entity_type: 'Food' | 'Predator' | 'Prey' | 'Obstacle' | 'None';
  agent_id: number;
}

export interface CombatEvent {
  predator_id: number;
  prey_id: number;
  damage: number;
  energy_transferred: number;
}

export interface MigrationPayload {
  agent_id: number;
  direction: 'incoming' | 'outgoing';
  source_port: number;
  target_port: number;
  status: string;
  timestamp: number;
}

export interface SimulationStatus {
  running: boolean;
  tick_count: number;
  avg_tick_time_ms: number;
  fps: number;
}

export interface ChronicleEvent {
  id: string;
  event_type: string;
  timestamp: number;
  title: string;
  description: string;
  parameter_delta?: { [key: string]: number };
}

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

export interface EliteIndividualState {
  fitness: number;
  features: number[];
}

export interface MapElitesGridState {
  grid: { [key: string]: EliteIndividualState };
  grid_resolution: number;
}

export interface EvolutionSettings {
  mutation_rate: number;
  selection_bias: number;
  grid_resolution: number;
}

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

export interface PheromoneGridState {
  grid: number[];
  width: number;
  height: number;
}

export interface LineageNode {
  id: string;
  generation: number;
  parent_id: string | null;
  fitness: number;
  mutations_count: number;
}

export interface LineageLink {
  source: string;
  target: string;
}

