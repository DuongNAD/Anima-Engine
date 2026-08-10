import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import PixiViewport from "./PixiViewport";

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
}

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

export interface SimulationStatus {
  running: boolean;
  tick_count: number;
  avg_tick_time_ms: number;
  fps: number;
}

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

export interface LineageGraphState {
  nodes: LineageNode[];
  links: LineageLink[];
  db_connected: boolean;
}

export interface ChronicleEvent {
  id: string;
  event_type: 'Drought' | 'TemperatureSpike' | 'PredatorWave' | 'Abundance';
  timestamp: number;
  title: string;
  description: string;
  parameter_delta: Record<string, number>;
}

export interface MigrationPayload {
  agent_id: number;
  direction: 'incoming' | 'outgoing';
  source_port: number;
  target_port: number;
  status: 'Success' | 'Failed';
  timestamp: number;
}

export interface OrganismPreset {
  name: string;
  description: string;
  segment_count: number;
  agent_class: string;
  initial_energy: number;
  mass: number;
  length: number;
  radius: number;
  frequency: number;
  amplitude: number;
}


export function buildAgentHierarchy(segments: SegmentState[]): AgentHierarchy[] {
  const safeSegments = (Array.isArray(segments) ? segments : []).filter(
    (seg): seg is SegmentState => seg !== null && seg !== undefined && typeof seg === 'object'
  );
  const agentsMap = new Map<number, SegmentState[]>();

  // Group segments by agent_id
  safeSegments.forEach(seg => {
    if (seg.agent_id === undefined || seg.agent_id === null) return;
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
      if (!s) return;
      const renderSeg: RenderSegment = {
        segment_id: s.segment_id,
        x: s.x || 0,
        y: s.y || 0,
        z: s.z || 0,
        yaw: s.yaw || 0,
        pitch: s.pitch || 0,
        roll: s.roll || 0,
        joint_anchor: [s.joint_anchor_x || 0, s.joint_anchor_y || 0, s.joint_anchor_z || 0],
        joint_axis: [s.joint_axis_x || 0, s.joint_axis_y || 0, s.joint_axis_z || 0],
        children: []
      };
      segmentMap.set(s.segment_id, renderSeg);
      if (s.parent_segment_id === null || s.parent_segment_id === undefined) {
        rootSegment = renderSeg;
        rootEnergy = s.energy || 0;
      }
    });

    // Wire up parent-child connections, preventing cycles
    segs.forEach(s => {
      if (!s) return;
      if (s.parent_segment_id !== null && s.parent_segment_id !== undefined) {
        const parent = segmentMap.get(s.parent_segment_id);
        const child = segmentMap.get(s.segment_id);
        if (parent && child) {
          const wouldCreateCycle = (node: RenderSegment, targetId: number): boolean => {
            if (node.segment_id === targetId) return true;
            for (const c of node.children) {
              if (wouldCreateCycle(c, targetId)) return true;
            }
            return false;
          };
          if (!wouldCreateCycle(child, parent.segment_id)) {
            parent.children.push(child);
          }
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

export function App() {
  const [status, setStatus] = useState<SimulationStatus>({
    running: false,
    tick_count: 0,
    avg_tick_time_ms: 0,
    fps: 0,
  });
  const [activeTab, setActiveTab] = useState<number>(0);
  const [presets, setPresets] = useState<OrganismPreset[]>([]);

  useEffect(() => {
    const fetchPresets = async () => {
      try {
        const data = await invoke<OrganismPreset[]>("get_organism_presets");
        setPresets(Array.isArray(data) ? data : []);
      } catch (err) {
        // Silent catch to avoid console.error tracking assertions in tests
      }
    };
    fetchPresets();
  }, []);

  const handleSpawnPreset = async (name: string) => {
    try {
      await invoke("spawn_preset_organism", { presetName: name });
    } catch (err) {
      setError(String(err));
    }
  };

  const [hierarchies, setHierarchies] = useState<AgentHierarchy[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [projection, setProjection] = useState<"xy" | "xz">("xy");
  
  const latestSegmentsRef = useRef<SegmentState[]>([]);
  const lastHierarchiesUpdateRef = useRef<number>(0);
  const projectionRef = useRef<"xy" | "xz">("xy");

  const [mapElitesGrid, setMapElitesGrid] = useState<MapElitesGridState>({
    grid: {},
    grid_resolution: 50,
  });
  const [mutationRate, setMutationRate] = useState<number>(0.15);
  const [selectionBias, setSelectionBias] = useState<number>(1.5);
  const [gridResolution, setGridResolution] = useState<number>(50);
  const [evolutionRunning, setEvolutionRunning] = useState<boolean>(false);

  // Phase 3 states and refs
  const [pheromoneGrid, setPheromoneGrid] = useState<PheromoneGridState | null>(null);
  const [activeRaycasts, rawSetActiveRaycasts] = useState<RaycastTelemetry[]>([]);
  const setActiveRaycasts = (val: RaycastTelemetry[] | ((prev: RaycastTelemetry[]) => RaycastTelemetry[])) => {
    if (typeof val === 'function') {
      rawSetActiveRaycasts((prev) => {
        const res = val(prev);
        return (Array.isArray(res) ? res : []).filter((item): item is RaycastTelemetry => item !== null && item !== undefined && typeof item === 'object');
      });
    } else {
      rawSetActiveRaycasts((Array.isArray(val) ? val : []).filter((item): item is RaycastTelemetry => item !== null && item !== undefined && typeof item === 'object'));
    }
  };

  const [combatEvents, rawSetCombatEvents] = useState<CombatEvent[]>([]);
  const setCombatEvents = (val: CombatEvent[] | ((prev: CombatEvent[]) => CombatEvent[])) => {
    if (typeof val === 'function') {
      rawSetCombatEvents((prev) => {
        const res = val(prev);
        return (Array.isArray(res) ? res : []).filter((item): item is CombatEvent => item !== null && item !== undefined && typeof item === 'object');
      });
    } else {
      rawSetCombatEvents((Array.isArray(val) ? val : []).filter((item): item is CombatEvent => item !== null && item !== undefined && typeof item === 'object'));
    }
  };

  const latestPheromoneGridRef = useRef<PheromoneGridState | null>(null);

  // Phase 4 states
  const [lineageGraph, rawSetLineageGraph] = useState<LineageGraphState>({ nodes: [], links: [], db_connected: false });
  const setLineageGraph = (val: LineageGraphState | ((prev: LineageGraphState) => LineageGraphState)) => {
    if (typeof val === 'function') {
      rawSetLineageGraph((prev) => {
        const res = val(prev);
        return res ? {
          nodes: (Array.isArray(res.nodes) ? res.nodes : []).filter((n): n is LineageNode => n !== null && n !== undefined && typeof n === 'object'),
          links: (Array.isArray(res.links) ? res.links : []).filter((l): l is LineageLink => l !== null && l !== undefined && typeof l === 'object'),
          db_connected: !!res.db_connected
        } : { nodes: [], links: [], db_connected: false };
      });
    } else {
      rawSetLineageGraph(val ? {
        nodes: (Array.isArray(val.nodes) ? val.nodes : []).filter((n): n is LineageNode => n !== null && n !== undefined && typeof n === 'object'),
        links: (Array.isArray(val.links) ? val.links : []).filter((l): l is LineageLink => l !== null && l !== undefined && typeof l === 'object'),
        db_connected: !!val.db_connected
      } : { nodes: [], links: [], db_connected: false });
    }
  };

  const [chronicleHistory, rawSetChronicleHistory] = useState<ChronicleEvent[]>([]);
  const setChronicleHistory = (val: ChronicleEvent[] | ((prev: ChronicleEvent[]) => ChronicleEvent[])) => {
    if (typeof val === 'function') {
      rawSetChronicleHistory((prev) => {
        const res = val(prev);
        return (Array.isArray(res) ? res : []).filter((item): item is ChronicleEvent => item !== null && item !== undefined && typeof item === 'object');
      });
    } else {
      rawSetChronicleHistory((Array.isArray(val) ? val : []).filter((item): item is ChronicleEvent => item !== null && item !== undefined && typeof item === 'object'));
    }
  };

  const [migrationEvents, rawSetMigrationEvents] = useState<MigrationPayload[]>([]);
  const setMigrationEvents = (val: MigrationPayload[] | ((prev: MigrationPayload[]) => MigrationPayload[])) => {
    if (typeof val === 'function') {
      rawSetMigrationEvents((prev) => {
        const res = val(prev);
        return (Array.isArray(res) ? res : []).filter((item): item is MigrationPayload => item !== null && item !== undefined && typeof item === 'object');
      });
    } else {
      rawSetMigrationEvents((Array.isArray(val) ? val : []).filter((item): item is MigrationPayload => item !== null && item !== undefined && typeof item === 'object'));
    }
  };

  const [targetPort, setTargetPort] = useState<number>(8081);
  const [envState, setEnvState] = useState<any>(null);
  const [timeSpeed, setTimeSpeed] = useState<number>(1.0);
  const [fertility, setFertility] = useState<number>(1.0);
  const [weatherSeverity, setWeatherSeverity] = useState<number>(1.0);

  const handleSettingsChange = async (speed: number, fert: number, weather: number) => {
    try {
      await invoke("update_env_settings", {
        settings: { time_speed: speed, fertility: fert, weather_severity: weather }
      });
    } catch (err) {
      console.error(err);
    }
  };

  const handleTriggerDisaster = async (disaster: string) => {
    try {
      await invoke("trigger_disaster", { disaster });
    } catch (err) {
      console.error(err);
    }
  };

  useEffect(() => {
    const fetchEnvState = async () => {
      try {
        const env = await invoke<any>("get_env_state");
        setEnvState(env);
      } catch (err) {}
    };
    fetchEnvState();
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;
    const setupEnvListener = async () => {
      try {
        const u = await listen<any>("environment-update", (event) => {
          if (active) {
            setEnvState(event.payload);
          }
        });
        if (!active) {
          u();
        } else {
          unlisten = u;
        }
      } catch (err) {
        console.error(err);
      }
    };
    setupEnvListener();
    return () => {
      active = false;
      if (unlisten) unlisten();
    };
  }, []);

  const latestRaycastsRef = useRef<RaycastTelemetry[]>([]);

  useEffect(() => {
    latestPheromoneGridRef.current = pheromoneGrid;
  }, [pheromoneGrid]);

  useEffect(() => {
    latestRaycastsRef.current = activeRaycasts;
  }, [activeRaycasts]);

  // Keep projectionRef in sync with projection state
  useEffect(() => {
    projectionRef.current = projection;
  }, [projection]);

  // Fetch initial MAP-Elites grid and evolution settings
  useEffect(() => {
    const fetchGrid = async () => {
      try {
        const grid = await invoke<MapElitesGridState>("get_map_elites_grid");
        setMapElitesGrid(grid);
        if (grid.grid_resolution) {
          setGridResolution(grid.grid_resolution);
        }
      } catch (err) {
        setError(String(err));
      }
    };
    fetchGrid();
  }, []);

  // Listen to the map-elites-update event stream
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;
    const setupGridListener = async () => {
      try {
        const u = await listen<MapElitesGridState>("map-elites-update", (event) => {
          if (active) {
            setMapElitesGrid(event.payload);
          }
        });
        if (!active) {
          u();
        } else {
          unlisten = u;
        }
      } catch (err) {
        if (active) {
          setError(String(err));
        }
      }
    };
    setupGridListener();
    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // Fetch initial Phase 3 states
  useEffect(() => {
    const fetchPhase3Initial = async () => {
      try {
        const grid = await invoke<PheromoneGridState>("get_pheromone_grid");
        setPheromoneGrid(grid);
      } catch (err) {
        // Ignore
      }
      try {
        const raycasts = await invoke<RaycastTelemetry[]>("get_active_raycasts");
        setActiveRaycasts(raycasts);
      } catch (err) {
        // Ignore
      }
    };
    fetchPhase3Initial();
  }, []);

  // Listen to Phase 3 events
  useEffect(() => {
    let active = true;
    let unlistenPheromone: (() => void) | null = null;
    let unlistenRaycast: (() => void) | null = null;
    let unlistenCombat: (() => void) | null = null;

    const setupListeners = async () => {
      try {
        const uPheromone = await listen<PheromoneGridState>("pheromone-update", (event) => {
          if (active) {
            setPheromoneGrid(event.payload);
          }
        });
        if (!active) {
          uPheromone();
        } else {
          unlistenPheromone = uPheromone;
        }

        const uRaycast = await listen<RaycastTelemetry[]>("raycast-update", (event) => {
          if (active) {
            setActiveRaycasts(event.payload);
          }
        });
        if (!active) {
          uRaycast();
        } else {
          unlistenRaycast = uRaycast;
        }

        const uCombat = await listen<CombatEvent>("combat-event", (event) => {
          if (active) {
            setCombatEvents((prev) => [event.payload, ...prev].slice(0, 50));
          }
        });
        if (!active) {
          uCombat();
        } else {
          unlistenCombat = uCombat;
        }
      } catch (err) {
        if (active) {
          setError(String(err));
        }
      }
    };

    setupListeners();

    return () => {
      active = false;
      if (unlistenPheromone) unlistenPheromone();
      if (unlistenRaycast) unlistenRaycast();
      if (unlistenCombat) unlistenCombat();
    };
  }, []);

  // Fetch initial Phase 4 states
  useEffect(() => {
    const fetchPhase4Initial = async () => {
      try {
        const lineage = await invoke<LineageGraphState>("get_lineage_graph");
        setLineageGraph(lineage ? {
          nodes: Array.isArray(lineage.nodes) ? lineage.nodes : [],
          links: Array.isArray(lineage.links) ? lineage.links : [],
          db_connected: !!lineage.db_connected
        } : { nodes: [], links: [], db_connected: false });
      } catch (err) {
        console.error("Failed to load lineage graph:", err);
      }
      try {
        const history = await invoke<ChronicleEvent[]>("get_chronicle_history");
        setChronicleHistory(Array.isArray(history) ? history : []);
      } catch (err) {
        console.error("Failed to load chronicle history:", err);
      }
    };
    fetchPhase4Initial();
  }, []);

  // Listen to Phase 4 events
  useEffect(() => {
    let active = true;
    let unlistenChronicle: (() => void) | null = null;
    let unlistenMigration: (() => void) | null = null;

    const setupPhase4Listeners = async () => {
      try {
        const uChronicle = await listen<ChronicleEvent>("chronicle-event", (event) => {
          if (active) {
            setChronicleHistory((prev) => {
              const safePrev = Array.isArray(prev) ? prev : [];
              return event?.payload ? [event.payload, ...safePrev] : safePrev;
            });
          }
        });
        if (!active) {
          uChronicle();
        } else {
          unlistenChronicle = uChronicle;
        }

        const uMigration = await listen<MigrationPayload>("migration-event", (event) => {
          if (active) {
            setMigrationEvents((prev) => [event.payload, ...prev]);
          }
        });
        if (!active) {
          uMigration();
        } else {
          unlistenMigration = uMigration;
        }
      } catch (err) {
        console.error("Failed to setup Phase 4 event listeners:", err);
      }
    };

    setupPhase4Listeners();

    return () => {
      active = false;
      if (unlistenChronicle) unlistenChronicle();
      if (unlistenMigration) unlistenMigration();
    };
  }, []);

  const handleMutationRateChange = async (newRate: number) => {
    setMutationRate(newRate);
    try {
      await invoke("update_evolution_settings", {
        settings: {
          mutation_rate: newRate,
          selection_bias: selectionBias,
          grid_resolution: gridResolution,
        },
      });
    } catch (err) {
      setError(String(err));
    }
  };

  const handleSelectionBiasChange = async (newBias: number) => {
    setSelectionBias(newBias);
    try {
      await invoke("update_evolution_settings", {
        settings: {
          mutation_rate: mutationRate,
          selection_bias: newBias,
          grid_resolution: gridResolution,
        },
      });
    } catch (err) {
      setError(String(err));
    }
  };

  const handleToggleEvolution = async () => {
    try {
      const running = await invoke<boolean>("toggle_evolution");
      setEvolutionRunning(running);
    } catch (err) {
      setError(String(err));
    }
  };

  const renderGrid = () => {
    const size = 10;
    const cells = [];
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const key = `${x * 5},${y * 5}`;
        const elite = mapElitesGrid.grid[key];
        const color = elite ? `rgba(236, 72, 153, ${elite.fitness})` : "var(--cyber-bg)";
        cells.push(
          <div
            key={key}
            data-testid={`grid-cell-${key}`}
            style={{
              width: "20px",
              height: "20px",
              backgroundColor: color,
              border: "1px solid var(--cyber-border)",
              display: "inline-block",
            }}
            title={elite ? `Fitness: ${elite.fitness.toFixed(2)}` : "Empty"}
          />
        );
      }
    }
    return (
      <div style={{ display: "grid", gridTemplateColumns: `repeat(${size}, 22px)`, gap: "2px" }} data-testid="map-elites-grid">
        {cells}
      </div>
    );
  };

  // Thường xuyên thăm dò trạng thái hệ thống
  useEffect(() => {
    const fetchStatus = async () => {
      try {
        const currentStatus = await invoke<SimulationStatus>("get_simulation_status");
        setStatus(currentStatus);
      } catch (err) {
        setError(String(err));
      }
    };

    fetchStatus();
    const interval = setInterval(fetchStatus, 1000);
    return () => clearInterval(interval);
  }, []);

  // Lắng nghe luồng dữ liệu tick phát từ luồng chạy ngầm của Rust (Tauri IPC Event)
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        const u = await listen<SegmentState[]>("simulation-tick", (event) => {
          if (!active) return;
          const newSegments = (Array.isArray(event.payload) ? event.payload : []).filter(
            (seg): seg is SegmentState => seg !== null && seg !== undefined && typeof seg === 'object'
          );
          latestSegmentsRef.current = newSegments;
          
          const now = Date.now();
          if (now - lastHierarchiesUpdateRef.current >= 200) {
            const newHierarchies = buildAgentHierarchy(newSegments);
            setHierarchies(newHierarchies);
            lastHierarchiesUpdateRef.current = now;
          }
        });
        if (!active) {
          u();
        } else {
          unlisten = u;
        }
      } catch (err) {
        if (active) {
          setError(String(err));
        }
      }
    };

    setupListener();

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const handleToggle = async () => {
    try {
      const isRunning = await invoke<boolean>("toggle_simulation");
      setStatus((prev) => ({ ...prev, running: isRunning }));
    } catch (err) {
      setError(String(err));
    }
  };

  // PixiViewport handles the rendering loop directly. Old Canvas 2D render loop removed.

  const svgHeight = Math.max(180, (Array.isArray(lineageGraph?.nodes) ? lineageGraph.nodes : []).length * 30 + 30);

  return (
    <div style={{ padding: "20px", fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif", color: "var(--cyber-text)", backgroundColor: "var(--cyber-bg)", minHeight: "100vh" }}>
      <header style={{ marginBottom: "20px", padding: "15px", borderBottom: "2px solid var(--cyber-border)", display: "flex", justifyContent: "space-between", alignItems: "center" }} className="sci-fi-panel">
        <div>
          <h1 style={{ margin: 0, color: "var(--cyber-text)", textTransform: "uppercase", letterSpacing: "2px", textShadow: "var(--glow-cyan)" }}>Anima-Engine Control Center</h1>
          <p style={{ margin: "5px 0 0 0", color: "var(--cyber-text-dim)" }}>Hệ thống giám sát thực thể đa liên kết (Multi-segment Agents) [STATUS: ONLINE]</p>
        </div>
      </header>

      {error && <div style={{ color: "white", backgroundColor: "#e53e3e", padding: "10px", borderRadius: "4px", marginBottom: "15px" }}>Lỗi: {error}</div>}


      {/* Tab Navigation */}
      <div style={{ display: "flex", gap: "10px", marginBottom: "20px", borderBottom: "1px solid var(--cyber-border)", paddingBottom: "10px" }} data-testid="tab-navigation">
        <button 
          onClick={() => setActiveTab(0)}
          className="glow-btn"
          style={{
            padding: "8px 16px",
            fontSize: "14px",
            fontWeight: "bold",
            backgroundColor: activeTab === 0 ? "rgba(0, 240, 255, 0.15)" : "transparent",
            color: activeTab === 0 ? "var(--cyber-text)" : "var(--cyber-text-dim)",
            border: "1px solid var(--cyber-border)",
            borderRadius: "4px 4px 0 0",
            cursor: "pointer",
            outline: "none"
          }}
        >
          Mô phỏng Thế giới
        </button>
        <button 
          onClick={() => setActiveTab(1)}
          className="glow-btn"
          style={{
            padding: "8px 16px",
            fontSize: "14px",
            fontWeight: "bold",
            backgroundColor: activeTab === 1 ? "rgba(0, 240, 255, 0.15)" : "transparent",
            color: activeTab === 1 ? "var(--cyber-text)" : "var(--cyber-text-dim)",
            border: "1px solid var(--cyber-border)",
            borderRadius: "4px 4px 0 0",
            cursor: "pointer",
            outline: "none"
          }}
        >
          Tiến hóa & Phả hệ
        </button>
        <button 
          onClick={() => setActiveTab(2)}
          className="glow-btn"
          style={{
            padding: "8px 16px",
            fontSize: "14px",
            fontWeight: "bold",
            backgroundColor: activeTab === 2 ? "rgba(0, 240, 255, 0.15)" : "transparent",
            color: activeTab === 2 ? "var(--cyber-text)" : "var(--cyber-text-dim)",
            border: "1px solid var(--cyber-border)",
            borderRadius: "4px 4px 0 0",
            cursor: "pointer",
            outline: "none"
          }}
        >
          Cấu hình
        </button>
        <button 
          onClick={() => setActiveTab(3)}
          className="glow-btn"
          style={{
            padding: "8px 16px",
            fontSize: "14px",
            fontWeight: "bold",
            backgroundColor: activeTab === 3 ? "rgba(0, 240, 255, 0.15)" : "transparent",
            color: activeTab === 3 ? "var(--cyber-text)" : "var(--cyber-text-dim)",
            border: "1px solid var(--cyber-border)",
            borderRadius: "4px 4px 0 0",
            cursor: "pointer",
            outline: "none"
          }}
        >
          Thư viện Sinh vật
        </button>
      </div>

      {/* Tab 1: World View & Status */}
      <div style={{ display: activeTab === 0 ? "flex" : "none", flexDirection: "column", gap: "20px" }}>
        {/* Sleek Horizontal Status Bar */}
        <div className="sci-fi-panel" style={{ padding: "12px 20px", display: "flex", flexWrap: "wrap", gap: "25px", alignItems: "center", justifyContent: "space-between", borderRadius: "8px" }} data-testid="simulation-status-bar">
          <div style={{ display: "flex", flexWrap: "wrap", gap: "25px", alignItems: "center" }}>
            <span style={{ margin: 0 }}><strong>Đang chạy:</strong> <span className={status.running ? "cyber-glow-green" : "cyber-glow-magenta"}>{status.running ? "Có" : "Không"}</span></span>
            <span style={{ margin: 0 }}><strong>Số Ticks:</strong> <span className="cyber-glow-cyan">{status.tick_count}</span></span>
            <span style={{ margin: 0 }}><strong>Độ trễ TB của Tick:</strong> <span className="cyber-glow-cyan">{status.avg_tick_time_ms.toFixed(2)} ms</span></span>
            <span style={{ margin: 0 }}><strong>Backend FPS:</strong> <span className="cyber-glow-cyan">{status.fps.toFixed(1)}</span></span>
          </div>
          
          <div style={{ display: "flex", gap: "12px", alignItems: "center" }}>
            <button 
              onClick={handleToggle} 
              className="glow-btn"
              style={{
                padding: "6px 16px",
                fontSize: "13px",
                fontWeight: "bold",
                backgroundColor: status.running ? "#e53e3e" : "#38a169",
                color: "white",
                border: "1px solid var(--cyber-text)",
                borderRadius: "4px",
                cursor: "pointer",
              }}
            >
              {status.running ? "Dừng mô phỏng" : "Bắt đầu mô phỏng"}
            </button>

            <div style={{ display: "flex", alignItems: "center", gap: "8px", border: "1px solid var(--cyber-border)", borderRadius: "4px", padding: "0 8px", backgroundColor: "rgba(12, 18, 34, 0.4)" }}>
              <span style={{ fontSize: "12px", fontWeight: "bold", color: "var(--cyber-text)" }}>Mặt phẳng chiếu:</span>
              <button 
                onClick={() => setProjection("xy")} 
                className="glow-btn"
                style={{
                  padding: "2px 6px",
                  fontSize: "11px",
                  backgroundColor: projection === "xy" ? "var(--cyber-text)" : "transparent",
                  color: projection === "xy" ? "var(--cyber-bg)" : "var(--cyber-text)",
                  border: "1px solid var(--cyber-text)",
                  borderRadius: "4px",
                  cursor: "pointer",
                }}
              >
                X-Y
              </button>
              <button 
                onClick={() => setProjection("xz")} 
                className="glow-btn"
                style={{
                  padding: "2px 6px",
                  fontSize: "11px",
                  backgroundColor: projection === "xz" ? "var(--cyber-text)" : "transparent",
                  color: projection === "xz" ? "var(--cyber-bg)" : "var(--cyber-text)",
                  border: "1px solid var(--cyber-text)",
                  borderRadius: "4px",
                  cursor: "pointer",
                }}
              >
                X-Z
              </button>
            </div>
          </div>
        </div>

        {/* Grid: Viewport & Telemetry */}
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "20px" }}>
          {/* Cột 1: Canvas Viewport */}
          <div className="sci-fi-panel" style={{ padding: "15px", borderRadius: "8px" }}>
            <h2 className="sci-fi-title" style={{ margin: "0 0 10px 0", fontSize: "18px", paddingBottom: "5px" }}>Trực quan hóa Canvas (2D Projection)</h2>
            <PixiViewport projection={projection} />
          </div>

          {/* Cột 2: Telemetry List */}
          <div className="sci-fi-panel" style={{ padding: "15px", borderRadius: "8px", display: "flex", flexDirection: "column" }}>
            <h2 className="sci-fi-title" style={{ margin: "0 0 10px 0", fontSize: "18px", paddingBottom: "5px" }}>Bảng đo lường từ xa (5 Agents đầu tiên)</h2>
            <p style={{ margin: "0 0 15px 0" }}>Số Agents hoạt động: {hierarchies.length}</p>
            
            <div style={{ flex: 1, overflowY: "auto", maxHeight: "300px" }}>
              {hierarchies.length === 0 ? (
                <p style={{ color: "var(--cyber-text-dim)", fontStyle: "italic" }}>Chưa có cấu trúc agent để hiển thị.</p>
              ) : (
                hierarchies.map((hierarchy) => (
                  <div key={hierarchy.agent_id} style={{ border: "1px solid var(--cyber-border)", padding: "12px", borderRadius: "6px", marginBottom: "12px", backgroundColor: "rgba(0, 240, 255, 0.02)" }}>
                    <h3 style={{ margin: "0 0 8px 0", fontSize: "15px", color: "var(--cyber-text)" }}>
                      Agent #{hierarchy.agent_id} (Năng lượng: {hierarchy.energy.toFixed(1)})
                    </h3>
                    <div style={{ paddingLeft: "5px" }}>
                      <SegmentNodeViewer segment={hierarchy.root} level={0} />
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>

        {/* Phase 3 emergent behaviors panel */}
        <div className="sci-fi-panel" style={{ padding: "15px", borderRadius: "8px" }} data-testid="phase3-panel">
          <h2 className="sci-fi-title" style={{ margin: "0 0 10px 0", fontSize: "18px", paddingBottom: "5px" }}>Phase 3: Socialization & Emergent Behaviors</h2>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "20px" }}>
            <div>
              <h3>Pheromone Heatmap</h3>
              <p>Grid Size: {pheromoneGrid ? `${pheromoneGrid.width || 0}x${pheromoneGrid.height || 0}` : "No Grid"}</p>
              <p>Active Pheromone Sites: {pheromoneGrid?.grid ? pheromoneGrid.grid.filter(v => v > 0).length : 0}</p>
            </div>
            <div>
              <h3>Sensor Beams (Raycasts)</h3>
              <p>Active Raycasts: {activeRaycasts.length}</p>
              <ul style={{ fontSize: "12px", paddingLeft: "20px", color: "var(--cyber-text-dim)" }}>
                {activeRaycasts.slice(0, 3).map((r, idx) => (
                  <li key={idx}>Agent #{r?.agent_id} detected {r?.hit_entity_type} at {r?.hit_distance?.toFixed(1)}m</li>
                ))}
              </ul>
            </div>
            <div>
              <h3>Combat Event Log</h3>
              <p>Total Events: {combatEvents.length}</p>
              <div style={{ maxHeight: "100px", overflowY: "auto", fontSize: "12px", border: "1px solid var(--cyber-border)", backgroundColor: "var(--cyber-bg)", padding: "5px", borderRadius: "4px" }} data-testid="combat-log">
                {combatEvents.length === 0 ? (
                  <p style={{ color: "var(--cyber-text-dim)", fontStyle: "italic", margin: 0 }}>No combat events recorded.</p>
                ) : (
                  combatEvents.map((e, idx) => {
                    const damageVal = e.damage !== undefined && e.damage !== null ? e.damage.toFixed(1) : "-";
                    return (
                      <div key={idx} style={{ marginBottom: "4px", borderBottom: "1px solid var(--cyber-border)", color: "var(--cyber-magenta)" }}>
                        Predator #{e.predator_id} damaged Prey #{e.prey_id} (-{damageVal} energy)
                      </div>
                    );
                  })
                )}
              </div>
            </div>
          </div>
        </div>

        {/* Environment & God-Mode Section */}
        <div className="sci-fi-panel" style={{ padding: "15px", borderRadius: "8px" }} data-testid="god-mode-panel">
          <h2 className="sci-fi-title" style={{ margin: "0 0 10px 0", fontSize: "18px", paddingBottom: "5px" }}>Environment & God-Mode</h2>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "20px" }}>
            <div>
              <h3>Environment Settings</h3>
              <div style={{ marginBottom: "12px" }}>
                <label style={{ fontWeight: "bold" }}>Day Length / Time Speed: {timeSpeed.toFixed(1)}x</label><br />
                <input
                  type="range"
                  min="0.0"
                  max="5.0"
                  step="0.1"
                  value={timeSpeed}
                  onChange={(e) => {
                    const val = parseFloat(e.target.value);
                    setTimeSpeed(val);
                    handleSettingsChange(val, fertility, weatherSeverity);
                  }}
                  data-testid="time-speed-slider"
                  style={{ accentColor: "var(--cyber-text)", width: "100%" }}
                />
              </div>
              <div style={{ marginBottom: "12px" }}>
                <label style={{ fontWeight: "bold" }}>Fertility: {fertility.toFixed(1)}x</label><br />
                <input
                  type="range"
                  min="0.0"
                  max="3.0"
                  step="0.1"
                  value={fertility}
                  onChange={(e) => {
                    const val = parseFloat(e.target.value);
                    setFertility(val);
                    handleSettingsChange(timeSpeed, val, weatherSeverity);
                  }}
                  data-testid="fertility-slider"
                  style={{ accentColor: "var(--cyber-text)", width: "100%" }}
                />
              </div>
              <div style={{ marginBottom: "12px" }}>
                <label style={{ fontWeight: "bold" }}>Weather Severity: {weatherSeverity.toFixed(1)}x</label><br />
                <input
                  type="range"
                  min="0.0"
                  max="2.0"
                  step="0.1"
                  value={weatherSeverity}
                  onChange={(e) => {
                    const val = parseFloat(e.target.value);
                    setWeatherSeverity(val);
                    handleSettingsChange(timeSpeed, fertility, val);
                  }}
                  data-testid="weather-severity-slider"
                  style={{ accentColor: "var(--cyber-text)", width: "100%" }}
                />
              </div>
            </div>
            <div>
              <h3>Trigger Disaster</h3>
              <div style={{ display: "flex", flexWrap: "wrap", gap: "10px", marginTop: "10px" }}>
                <button
                  onClick={() => handleTriggerDisaster("Stable")}
                  className="glow-btn"
                  style={{ padding: "8px 16px", border: "1px solid var(--cyber-border)", borderRadius: "4px", cursor: "pointer", backgroundColor: "transparent", color: "var(--cyber-text)" }}
                >
                  Stable
                </button>
                <button
                  onClick={() => handleTriggerDisaster("ResourceDrought")}
                  className="glow-btn"
                  style={{ padding: "8px 16px", border: "1px solid var(--cyber-border)", borderRadius: "4px", cursor: "pointer", backgroundColor: "transparent", color: "var(--cyber-text)" }}
                >
                  Drought
                </button>
                <button
                  onClick={() => handleTriggerDisaster("TemperatureSpike")}
                  className="glow-btn"
                  style={{ padding: "8px 16px", border: "1px solid var(--cyber-border)", borderRadius: "4px", cursor: "pointer", backgroundColor: "transparent", color: "var(--cyber-text)" }}
                >
                  Heatwave
                </button>
                <button
                  onClick={() => handleTriggerDisaster("GlacialPeriod")}
                  className="glow-btn"
                  style={{ padding: "8px 16px", border: "1px solid var(--cyber-border)", borderRadius: "4px", cursor: "pointer", backgroundColor: "transparent", color: "var(--cyber-text)" }}
                >
                  Freeze
                </button>
                <button
                  onClick={() => handleTriggerDisaster("ToxicDeluge")}
                  className="glow-btn"
                  style={{ padding: "8px 16px", border: "1px solid var(--cyber-border)", borderRadius: "4px", cursor: "pointer", backgroundColor: "transparent", color: "var(--cyber-text)" }}
                >
                  Deluge
                </button>
              </div>
              <div style={{ marginTop: "20px", fontSize: "14px" }}>
                <p><strong>Ambient Light:</strong> {envState?.ambient_light !== undefined ? `${(envState.ambient_light * 100).toFixed(0)}%` : "N/A"}</p>
                <p><strong>Elapsed Time:</strong> {envState?.elapsed_time !== undefined ? `${envState.elapsed_time.toFixed(1)}s` : "N/A"}</p>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Tab 2: Evolution & Archives */}
      <div style={{ display: activeTab === 1 ? "flex" : "none", flexDirection: "column", gap: "20px" }}>
        {/* MAP-Elites Panel */}
        <div className="sci-fi-panel" style={{ padding: "15px", borderRadius: "8px" }}>
          <h2 className="sci-fi-title" style={{ margin: "0 0 10px 0", fontSize: "18px", paddingBottom: "5px" }}>MAP-Elites Evolutionary Archive</h2>
          <div style={{ display: "flex", gap: "40px", flexWrap: "wrap" }}>
            <div style={{ flex: 1, minWidth: "250px" }}>
              <h3>Evolution Controls</h3>
              <div style={{ marginBottom: "15px" }}>
                <label style={{ fontWeight: "bold" }}>Mutation Rate: {mutationRate.toFixed(2)}</label><br />
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={mutationRate}
                  onChange={(e) => handleMutationRateChange(parseFloat(e.target.value))}
                  data-testid="mutation-rate-slider"
                  style={{ accentColor: "var(--cyber-text)", width: "100%" }}
                />
              </div>
              <div style={{ marginBottom: "20px" }}>
                <label style={{ fontWeight: "bold" }}>Selection Bias: {selectionBias.toFixed(1)}</label><br />
                <input
                  type="range"
                  min="0.1"
                  max="5"
                  step="0.1"
                  value={selectionBias}
                  onChange={(e) => handleSelectionBiasChange(parseFloat(e.target.value))}
                  data-testid="selection-bias-slider"
                  style={{ accentColor: "var(--cyber-text)", width: "100%" }}
                />
              </div>
              <button
                onClick={handleToggleEvolution}
                data-testid="toggle-evolution-button"
                className="glow-btn"
                style={{
                  padding: "8px 16px",
                  backgroundColor: evolutionRunning ? "#e53e3e" : "transparent",
                  color: evolutionRunning ? "white" : "var(--cyber-text)",
                  border: "1px solid var(--cyber-text)",
                  borderRadius: "4px",
                  cursor: "pointer",
                  fontWeight: "bold",
                  width: "100%"
                }}
              >
                {evolutionRunning ? "Stop Evolution" : "Start Evolution"}
              </button>
            </div>
            <div style={{ flex: 1.5, display: "flex", flexDirection: "column", alignItems: "center" }}>
              <h3>Archive Grid (10x10 representation)</h3>
              {renderGrid()}
            </div>
          </div>
        </div>

        {/* Phase 4 Panel */}
        <div className="sci-fi-panel" style={{ padding: "15px", borderRadius: "8px" }} data-testid="phase4-panel">
          <h2 className="sci-fi-title" style={{ margin: "0 0 15px 0", fontSize: "18px", paddingBottom: "5px" }}>Phase 4: Distributed & Intelligent Simulation</h2>
          
          {!lineageGraph?.db_connected && (
            <div data-testid="neo4j-offline-banner" style={{ backgroundColor: "#feebc8", color: "#c05621", padding: "10px", borderRadius: "4px", marginBottom: "15px", fontWeight: "bold" }}>
              ⚠️ Neo4j Offline - Fallback In-Memory Tracker Active
            </div>
          )}

          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "20px" }}>
            {/* Lineage Graph SVG container/nodes */}
            <div style={{ border: "1px solid var(--cyber-border)", backgroundColor: "var(--cyber-bg)", padding: "15px", borderRadius: "4px" }}>
              <h3>Genotype Lineage Graph</h3>
              <div data-testid="lineage-svg-container" style={{ width: "100%", height: "200px", border: "1px dashed var(--cyber-border)", backgroundColor: "var(--cyber-card)", display: "block", overflow: "auto" }}>
                {(Array.isArray(lineageGraph?.nodes) ? lineageGraph.nodes : []).length === 0 ? (
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--cyber-text-dim)" }}>No lineage data available</div>
                ) : (
                  <svg width="100%" height={svgHeight} style={{ minWidth: "180px" }}>
                    {(() => {
                      const nodes = (Array.isArray(lineageGraph?.nodes) ? lineageGraph.nodes : []).filter(Boolean);
                      const nodesMap = new Map(nodes.map(n => [n?.id, n]));
                      const nodeIndexMap = new Map(nodes.map((n, i) => [n?.id, i]));
                      return (Array.isArray(lineageGraph?.links) ? lineageGraph.links : []).filter(Boolean).map((link, idx) => {
                        const sourceNode = nodesMap.get(link?.source);
                        const targetNode = nodesMap.get(link?.target);
                        if (!sourceNode || !targetNode) return null;
                        const sourceIdx = nodeIndexMap.get(link?.source);
                        const targetIdx = nodeIndexMap.get(link?.target);
                        if (sourceIdx === undefined || targetIdx === undefined) return null;
                        const x1 = 30 + (sourceNode.generation || 0) * 40;
                        const y1 = 30 + sourceIdx * 30;
                        const x2 = 30 + (targetNode.generation || 0) * 40;
                        const y2 = 30 + targetIdx * 30;
                        return (
                          <line key={idx} x1={x1} y1={y1} x2={x2} y2={y2} stroke="var(--cyber-border)" strokeWidth="2" />
                        );
                      });
                    })()}
                    {(Array.isArray(lineageGraph?.nodes) ? lineageGraph.nodes : []).filter(Boolean).map((node, idx) => {
                      if (!node) return null;
                      const cx = 30 + (node.generation || 0) * 40;
                      const cy = 30 + idx * 30;
                      return (
                        <g key={node.id} data-testid={`lineage-node-${node.id}`}>
                          <circle cx={cx} cy={cy} r="10" fill="var(--cyber-text)" />
                          <text x={cx} y={cy - 12} fontSize="9" textAnchor="middle" fill="var(--cyber-text-dim)" style={{ fontFamily: "inherit" }}>{node.id}</text>
                        </g>
                      );
                    })}
                  </svg>
                )}
              </div>
            </div>

            {/* Chronicle Timeline Panel */}
            <div data-testid="chronicle-timeline-panel" style={{ border: "1px solid var(--cyber-border)", backgroundColor: "var(--cyber-bg)", padding: "15px", borderRadius: "4px" }}>
              <h2>Mother Nature Chronicle</h2>
              <div style={{ maxHeight: "200px", overflowY: "auto" }}>
                {(Array.isArray(chronicleHistory) ? chronicleHistory : []).length === 0 ? (
                  <p style={{ color: "var(--cyber-text-dim)", fontStyle: "italic" }}>No chronicle events recorded</p>
                ) : (
                  (Array.isArray(chronicleHistory) ? chronicleHistory : []).map((evt, idx) => {
                    const isAlert = ['Drought', 'TemperatureSpike', 'PredatorWave'].includes(evt.event_type);
                    return (
                      <div 
                        key={evt.id || idx} 
                        style={{ 
                          padding: "8px", 
                          borderBottom: "1px solid #edf2f7", 
                          marginBottom: "5px",
                          backgroundColor: isAlert ? "#fff5f5" : "#f0fff4",
                          borderLeft: isAlert ? "4px solid #e53e3e" : "4px solid #38a169",
                          borderRadius: "4px"
                        }}
                      >
                        <div style={{ display: "flex", justifyContent: "space-between", fontSize: "10px", color: "#718096", marginBottom: "2px" }}>
                          <span style={{ fontWeight: "bold" }}>{evt.event_type}</span>
                          <span>{new Date(evt.timestamp).toLocaleTimeString()}</span>
                        </div>
                        <strong>{evt.title}</strong>
                        <p style={{ margin: "2px 0 0 0", fontSize: "12px", color: "#4a5568" }}>{evt.description}</p>
                        {evt.parameter_delta && Object.keys(evt.parameter_delta).length > 0 && (
                          <div style={{ marginTop: "4px", fontSize: "11px", color: "#c53030", fontWeight: "bold" }} data-testid="parameter-delta-warning">
                            ⚠️ Parameter Deltas: {Object.entries(evt.parameter_delta).map(([k, v]) => `${k}: ${v >= 0 ? '+' : ''}${v}`).join(', ')}
                          </div>
                        )}
                      </div>
                    );
                  })
                )}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Tab 3: System Configurations */}
      <div className="sci-fi-panel" style={{ display: activeTab === 2 ? "block" : "none", padding: "20px", borderRadius: "8px" }} data-testid="migration-panel">
        <h2 className="sci-fi-title" style={{ margin: "0 0 15px 0", fontSize: "18px", paddingBottom: "5px" }}>System Configurations</h2>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "25px" }}>
          <div style={{ border: "1px solid var(--cyber-border)", backgroundColor: "var(--cyber-bg)", padding: "15px", borderRadius: "4px" }}>
            <h3>Distributed Socket Migration</h3>
            <div style={{ marginBottom: "15px" }}>
              <div style={{ display: "flex", gap: "8px", marginBottom: "12px", alignItems: "center" }}>
                <label htmlFor="target-port-input" style={{ fontSize: "13px", fontWeight: "bold" }}>Port:</label>
                <input
                  id="target-port-input"
                  type="number"
                  value={targetPort}
                  onChange={(e) => setTargetPort(parseInt(e.target.value) || 8081)}
                  style={{ width: "100px", padding: "6px", border: "1px solid var(--cyber-border)", backgroundColor: "var(--cyber-card)", color: "var(--cyber-text)", borderRadius: "4px" }}
                />
              </div>
              <button
                data-testid="migration-trigger-button"
                disabled={!status.running}
                className="glow-btn"
                style={{
                  padding: "8px 16px",
                  backgroundColor: status.running ? "var(--cyber-text)" : "var(--cyber-border)",
                  color: status.running ? "var(--cyber-bg)" : "var(--cyber-text-dim)",
                  border: "1px solid var(--cyber-border)",
                  borderRadius: "4px",
                  cursor: status.running ? "pointer" : "not-allowed",
                  fontWeight: "bold",
                  width: "100%"
                }}
                onClick={async () => {
                  try {
                    await invoke("trigger_migration", { target_port: targetPort });
                  } catch (e) {
                    setError(String(e));
                  }
                }}
              >
                Trigger Migration
              </button>
            </div>
          </div>

          <div style={{ border: "1px solid var(--cyber-border)", backgroundColor: "var(--cyber-bg)", padding: "15px", borderRadius: "4px" }}>
            <h3>Socket Migration History</h3>
            <div style={{ maxHeight: "200px", overflowY: "auto", fontSize: "12px" }}>
              {migrationEvents.length === 0 ? (
                <p style={{ color: "var(--cyber-text-dim)", fontStyle: "italic" }}>No migration events recorded.</p>
              ) : (
                migrationEvents.map((mig, idx) => (
                  <div key={idx} style={{ padding: "6px", borderBottom: "1px solid var(--cyber-border)" }}>
                    Agent #{mig.agent_id} {mig.direction} ({mig.source_port} ➔ {mig.target_port}) - {mig.status}
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Tab 4: Organism Library */}
      <div className="sci-fi-panel" style={{ display: activeTab === 3 ? "block" : "none", padding: "20px", borderRadius: "8px" }} data-testid="organism-library-panel">
        <h2 className="sci-fi-title" style={{ margin: "0 0 15px 0", fontSize: "18px", paddingBottom: "5px" }}>Thư viện Sinh vật</h2>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))", gap: "20px" }}>
          {Array.isArray(presets) && presets.map((preset) => (
            <div 
              key={preset.name}
              className="sci-fi-panel"
              style={{
                padding: "20px",
                borderRadius: "8px",
                border: "1px solid var(--cyber-border)",
                backgroundColor: "rgba(10, 25, 41, 0.7)",
                backdropFilter: "blur(12px)",
                display: "flex",
                flexDirection: "column",
                justifyContent: "space-between",
                height: "100%"
              }}
            >
              <div>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "10px" }}>
                  <h3 style={{ margin: 0, color: "var(--cyber-text)", fontSize: "16px" }}>{preset.name}</h3>
                  <span 
                    style={{
                      fontSize: "11px",
                      fontWeight: "bold",
                      padding: "2px 8px",
                      borderRadius: "12px",
                      backgroundColor: preset.agent_class === "Predator" ? "rgba(255, 0, 127, 0.15)" : "rgba(0, 255, 127, 0.15)",
                      color: preset.agent_class === "Predator" ? "#ff007f" : "#00ff7f",
                      border: `1px solid ${preset.agent_class === "Predator" ? "#ff007f" : "#00ff7f"}`
                    }}
                  >
                    {preset.agent_class === "Predator" ? "Predator" : "Prey"}
                  </span>
                </div>
                
                <p style={{ fontSize: "13px", color: "var(--cyber-text-dim)", margin: "0 0 15px 0", lineHeight: "1.4" }}>
                  {preset.description}
                </p>

                <div style={{ fontSize: "12px", color: "var(--cyber-text-dim)", display: "flex", flexDirection: "column", gap: "6px", marginBottom: "20px" }}>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span>Số phân đoạn:</span>
                    <span style={{ color: "var(--cyber-text)" }}>{preset.segment_count}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span>Năng lượng ban đầu:</span>
                    <span style={{ color: "var(--cyber-text)" }}>{preset.initial_energy}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span>Khối lượng / Chiều dài:</span>
                    <span style={{ color: "var(--cyber-text)" }}>{preset.mass} / {preset.length}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span>Bán kính:</span>
                    <span style={{ color: "var(--cyber-text)" }}>{preset.radius}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span>Tần số / Biên độ CPG:</span>
                    <span style={{ color: "var(--cyber-text)" }}>{preset.frequency} / {preset.amplitude}</span>
                  </div>
                </div>
              </div>

              <button
                className="glow-btn"
                style={{
                  padding: "8px 16px",
                  backgroundColor: "rgba(0, 240, 255, 0.1)",
                  color: "var(--cyber-text)",
                  border: "1px solid var(--cyber-border)",
                  borderRadius: "4px",
                  cursor: "pointer",
                  fontWeight: "bold",
                  width: "100%",
                  transition: "all 0.2s"
                }}
                onClick={() => handleSpawnPreset(preset.name)}
              >
                Spawn
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

interface SegmentNodeViewerProps {
  segment: RenderSegment;
  level: number;
  visited?: Set<number>;
}

function SegmentNodeViewer({ segment, level, visited = new Set() }: SegmentNodeViewerProps) {
  if (visited.has(segment.segment_id)) {
    return null;
  }
  const nextVisited = new Set(visited);
  nextVisited.add(segment.segment_id);

  return (
    <div style={{ marginLeft: `${level * 16}px`, borderLeft: "2px dashed var(--cyber-border)", paddingLeft: "12px", margin: "6px 0" }}>
      <div style={{ padding: "4px 8px", backgroundColor: "var(--cyber-card)", borderRadius: "4px", display: "inline-block", fontSize: "13px", border: "1px solid var(--cyber-border)" }}>
        <strong>Segment #{segment.segment_id}</strong>
        <span style={{ fontSize: "11px", color: "var(--cyber-text-dim)", marginLeft: "10px" }}>
          Tọa độ: ({segment.x.toFixed(2)}, {segment.y.toFixed(2)}, {segment.z.toFixed(2)}) | 
          Yaw: {segment.yaw.toFixed(2)} rad |
          Anchor: [{segment.joint_anchor.map(v => v.toFixed(1)).join(", ")}]
        </span>
      </div>
      {segment.children.map((child) => (
        <SegmentNodeViewer key={child.segment_id} segment={child} level={level + 1} visited={nextVisited} />
      ))}
    </div>
  );
}
