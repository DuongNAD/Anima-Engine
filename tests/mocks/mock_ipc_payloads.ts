export interface AgentState {
  id: number;
  x: number;
  y: number;
  z: number;
  yaw: number;
  pitch: number;
  roll: number;
  energy: number;
  agent_type?: 'predator' | 'prey';
}

export interface SimulationStatus {
  running: boolean;
  tick_count: number;
  avg_tick_time_ms: number;
  fps: number;
}

export const mockSimulationStatus: SimulationStatus = {
  running: true,
  tick_count: 120,
  avg_tick_time_ms: 1.45,
  fps: 60.2,
};

export const mockAgentStates: AgentState[] = [
  {
    id: 1,
    x: 10.0,
    y: 1.5,
    z: -5.0,
    yaw: 0.0,
    pitch: 0.0,
    roll: 0.0,
    energy: 99.5,
    agent_type: 'predator',
  },
  {
    id: 2,
    x: -12.3,
    y: 0.0,
    z: 8.4,
    yaw: 1.2,
    pitch: -0.1,
    roll: 0.0,
    energy: 85.0,
    agent_type: 'prey',
  },
];

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


export interface AgentHierarchy {
  agent_id: number;
  energy: number;
  root: RenderSegment;
}

export interface RenderSegment {
  segment_id: number;
  x: number;
  y: number;
  z: number;
  yaw: number;
  pitch: number;
  roll: number;
  joint_anchor: [number, number, number];
  joint_axis: [number, number, number];
  children: RenderSegment[];
}

export function buildAgentHierarchy(segments: SegmentState[]): AgentHierarchy[] {
  const agentsMap = new Map<number, SegmentState[]>();

  // Group segments by agent_id
  segments.forEach(seg => {
    if (!agentsMap.has(seg.agent_id)) {
      agentsMap.set(seg.agent_id, []);
    }
    agentsMap.get(seg.agent_id)!.push(seg);
  });

  const hierarchies: AgentHierarchy[] = [];

  agentsMap.forEach((segs, agentId) => {
    const segmentMap = new Map<number, RenderSegment>();
    let rootSegment: RenderSegment | null = null;
    let rootEnergy = 0;

    // Initialize all render segments
    segs.forEach(s => {
      const renderSeg: RenderSegment = {
        segment_id: s.segment_id,
        x: s.x,
        y: s.y,
        z: s.z,
        yaw: s.yaw,
        pitch: s.pitch,
        roll: s.roll,
        joint_anchor: [s.joint_anchor_x, s.joint_anchor_y, s.joint_anchor_z],
        joint_axis: [s.joint_axis_x, s.joint_axis_y, s.joint_axis_z],
        children: []
      };
      segmentMap.set(s.segment_id, renderSeg);
      if (s.parent_segment_id === null) {
        rootSegment = renderSeg;
        rootEnergy = s.energy;
      }
    });

    // Wire up parent-child connections
    segs.forEach(s => {
      if (s.parent_segment_id !== null) {
        const parent = segmentMap.get(s.parent_segment_id);
        const child = segmentMap.get(s.segment_id);
        if (parent && child) {
          parent.children.push(child);
        }
      }
    });

    if (rootSegment) {
      hierarchies.push({
        agent_id: agentId,
        energy: rootEnergy,
        root: rootSegment
      });
    }
  });

  return hierarchies;
}

export const mockSegmentStates: SegmentState[] = [
  {
    agent_id: 1,
    segment_id: 0,
    parent_segment_id: null,
    x: 10.0,
    y: 1.5,
    z: -5.0,
    yaw: 0.1,
    pitch: 0.0,
    roll: 0.0,
    joint_anchor_x: 0,
    joint_anchor_y: 0,
    joint_anchor_z: 0,
    joint_axis_x: 0,
    joint_axis_y: 0,
    joint_axis_z: 0,
    energy: 95.5,
    agent_type: 'predator',
  },
  {
    agent_id: 1,
    segment_id: 1,
    parent_segment_id: 0,
    x: 11.0,
    y: 1.5,
    z: -5.0,
    yaw: 0.2,
    pitch: 0.1,
    roll: 0.0,
    joint_anchor_x: 1.0,
    joint_anchor_y: 0.0,
    joint_anchor_z: 0.0,
    joint_axis_x: 0.0,
    joint_axis_y: 0.0,
    joint_axis_z: 1.0,
    energy: 95.5,
    agent_type: 'predator',
  },
  {
    agent_id: 1,
    segment_id: 2,
    parent_segment_id: 1,
    x: 12.0,
    y: 1.5,
    z: -5.0,
    yaw: 0.3,
    pitch: 0.2,
    roll: 0.0,
    joint_anchor_x: 1.0,
    joint_anchor_y: 0.0,
    joint_anchor_z: 0.0,
    joint_axis_x: 0.0,
    joint_axis_y: 0.0,
    joint_axis_z: 1.0,
    energy: 95.5,
    agent_type: 'predator',
  },
];

export interface EvolutionSettings {
  mutation_rate: number;
  selection_bias: number;
  grid_resolution: number;
}

export interface EliteIndividualState {
  fitness: number;
  features: number[];
}

export interface MapElitesGridState {
  grid: Record<string, EliteIndividualState>;
  grid_resolution: number;
}

export const mockEvolutionSettings: EvolutionSettings = {
  mutation_rate: 0.15,
  selection_bias: 1.5,
  grid_resolution: 50,
};

export const mockMapElitesGridState: MapElitesGridState = {
  grid: {
    "10,20": {
      fitness: 0.85,
      features: [0.2, 0.4]
    },
    "30,40": {
      fitness: 0.92,
      features: [0.6, 0.8]
    }
  },
  grid_resolution: 50
};

export interface RaycastTelemetry {
  origin: [number, number, number];
  direction: [number, number, number];
  hit_distance: number;
  hit_entity_type: 'Food' | 'Predator' | 'Prey' | 'Obstacle' | 'None';
  agent_id: number;
}

export interface PheromoneGridState {
  grid: number[];
  width: number;
  height: number;
}

export interface CombatEvent {
  predator_id: number;
  prey_id: number;
  damage: number;
  energy_transferred: number;
}

export const mockRaycastTelemetry: RaycastTelemetry[] = [
  {
    origin: [10.0, 1.5, -5.0],
    direction: [1.0, 0.0, 0.0],
    hit_distance: 3.5,
    hit_entity_type: 'Prey',
    agent_id: 1
  },
  {
    origin: [-12.3, 0.0, 8.4],
    direction: [0.0, 0.0, 1.0],
    hit_distance: 12.0,
    hit_entity_type: 'Food',
    agent_id: 2
  }
];

export const mockPheromoneGridState: PheromoneGridState = {
  grid: new Array(128 * 128).fill(0.0).map((_, i) => {
    // Put a couple of pheromone spots
    const x = i % 128;
    const y = Math.floor(i / 128);
    if (x === 64 && y === 64) return 1.0;
    if (Math.abs(x - 64) < 5 && Math.abs(y - 64) < 5) return 0.5;
    return 0.0;
  }),
  width: 128,
  height: 128
};

export const mockCombatEvent: CombatEvent = {
  predator_id: 1,
  prey_id: 2,
  damage: 15.0,
  energy_transferred: 10.0
};

// --- Phase 4 Interfaces ---

export interface LineageNode {
  id: string; // Agent ID or unique UUID
  generation: number;
  parent_id: string | null;
  fitness: number;
  mutations_count: number;
}

export interface LineageLink {
  source: string; // Parent ID
  target: string; // Child ID
}

export interface LineageGraphState {
  nodes: LineageNode[];
  links: LineageLink[];
  db_connected: boolean; // Indicates if using real Neo4j vs In-memory fallback
}

export interface ChronicleEvent {
  id: string;
  event_type: 'Drought' | 'TemperatureSpike' | 'PredatorWave' | 'Abundance';
  timestamp: number;
  title: string;
  description: string;
  parameter_delta: {
    metabolic_decay_factor?: number;
    max_food_count?: number;
    predator_spawn_rate?: number;
  };
}

export interface MigrationPayload {
  agent_id: number;
  direction: 'incoming' | 'outgoing';
  source_port: number;
  target_port: number;
  status: 'Success' | 'Failed';
  timestamp: number;
}

// --- Phase 4 Mock Data ---

export const mockLineageGraph: LineageGraphState = {
  nodes: [
    { id: "A-0", generation: 0, parent_id: null, fitness: 0.5, mutations_count: 0 },
    { id: "A-1", generation: 1, parent_id: "A-0", fitness: 0.72, mutations_count: 2 },
    { id: "A-2", generation: 1, parent_id: "A-0", fitness: 0.41, mutations_count: 1 },
    { id: "A-3", generation: 2, parent_id: "A-1", fitness: 0.89, mutations_count: 3 },
  ],
  links: [
    { source: "A-0", target: "A-1" },
    { source: "A-0", target: "A-2" },
    { source: "A-1", target: "A-3" },
  ],
  db_connected: false, // Defaulting mock to fallback state to test warning indicator
};

export const mockChronicleHistory: ChronicleEvent[] = [
  {
    id: "evt-01",
    event_type: "TemperatureSpike",
    timestamp: Date.now() - 10000,
    title: "Meta-AI: Temperature Spike detected",
    description: "Mother Nature has triggered a temperature spike. Metabolic decay rates increased by 1.5x.",
    parameter_delta: { metabolic_decay_factor: 1.5 },
  },
  {
    id: "evt-02",
    event_type: "Drought",
    timestamp: Date.now() - 5000,
    title: "Meta-AI: Drought alert",
    description: "Mother Nature has triggered a drought. Maximum food count reduced by 50%.",
    parameter_delta: { max_food_count: -0.5 },
  },
];

export const mockChronicleEvent: ChronicleEvent = {
  id: "evt-03",
  event_type: "PredatorWave",
  timestamp: Date.now(),
  title: "Meta-AI: Predator Invasion",
  description: "A wave of invasive predators has spawned. Predator spawn rate increased.",
  parameter_delta: { predator_spawn_rate: 2.0 },
};

export const mockMigrationPayload: MigrationPayload = {
  agent_id: 42,
  direction: "outgoing",
  source_port: 8080,
  target_port: 8081,
  status: "Success",
  timestamp: Date.now(),
};

// --- Phase 6 Interfaces ---
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
  head_directions: HeadDirectionTelemetry[];
}

// --- Phase 6 Mock Data ---
export const mockEnvironmentalState: EnvironmentalState = {
  elements: [
    {
      type: 'lake',
      x: 50,
      y: 50,
      radius: 30,
      resources: 100
    },
    {
      type: 'tree',
      x: -50,
      y: -50,
      radius: 10,
      resources: 50
    }
  ]
};

export const mockSimulationTickPayload: SimulationTickPayload = {
  segments: mockSegmentStates.map(seg => ({
    ...seg,
    hydration: 75.0,
    head_direction: [1.0, 0.0, 0.0] as [number, number, number]
  })),
  environmental_state: mockEnvironmentalState,
  head_directions: [
    {
      agent_id: 1,
      direction: [1.0, 0.0, 0.0]
    }
  ]
};




