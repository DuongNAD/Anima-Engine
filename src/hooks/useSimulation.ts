import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTauriEvent } from './useTauriEvent';
import {
  SimulationStatus,
  AgentHierarchy,
  EnvironmentalState,
  SegmentState,
  MapElitesGridState,
  PheromoneGridState,
  RaycastTelemetry,
  CombatEvent,
  LineageGraphPayload,
  ChronicleEvent,
  MigrationPayload,
  RenderSegment,
  LineageNode,
  LineageLink
} from '../types';

export function buildAgentHierarchy(segments: SegmentState[]): AgentHierarchy[] {
  const safeSegments = (Array.isArray(segments) ? segments : []).filter(
    (seg): seg is SegmentState => seg !== null && seg !== undefined && typeof seg === 'object'
  );
  const agentsMap = new Map<number, SegmentState[]>();

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
        children: []
      };
      segmentMap.set(s.segment_id, renderSeg);
      if (s.parent_segment_id === null || s.parent_segment_id === undefined) {
        rootSegment = renderSeg;
        rootEnergy = s.energy || 0;
      }
    });

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

export function useSimulation() {
  const [status, setStatus] = useState<SimulationStatus>({
    running: false,
    tick_count: 0,
    avg_tick_time_ms: 0,
    fps: 0,
  });
  const [hierarchies, setHierarchies] = useState<AgentHierarchy[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [projection, setProjection] = useState<'xy' | 'xz'>('xy');

  const lastHierarchiesUpdateRef = useRef<number>(0);

  const [filePath, setFilePath] = useState<string>('');
  const [zoom, setZoom] = useState<number>(1.0);
  const [pan, setPan] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [environmentalState, setEnvironmentalState] = useState<EnvironmentalState>({ elements: [] });
  const [avgHydration, setAvgHydration] = useState<number | null>(null);
  const [headDirection, setHeadDirection] = useState<[number, number, number] | null>(null);

  const [mapElitesGrid, setMapElitesGrid] = useState<MapElitesGridState>({
    grid: {},
    grid_resolution: 50,
  });
  const [mutationRate, setMutationRate] = useState<number>(0.15);
  const [selectionBias, setSelectionBias] = useState<number>(1.5);
  const [gridResolution, setGridResolution] = useState<number>(50);
  const [evolutionRunning, setEvolutionRunning] = useState<boolean>(false);

  const [pheromoneGrid, setPheromoneGrid] = useState<PheromoneGridState | null>(null);
  const [activeRaycasts, rawSetActiveRaycasts] = useState<RaycastTelemetry[]>([]);
  const [combatEvents, rawSetCombatEvents] = useState<CombatEvent[]>([]);

  const [lineageGraph, rawSetLineageGraph] = useState<LineageGraphPayload>({ nodes: [], links: [], db_connected: false });
  const [chronicleHistory, rawSetChronicleHistory] = useState<ChronicleEvent[]>([]);
  const [migrationEvents, rawSetMigrationEvents] = useState<MigrationPayload[]>([]);
  const [targetPort, setTargetPort] = useState<number>(8081);

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

  const setLineageGraph = (val: LineageGraphPayload | ((prev: LineageGraphPayload) => LineageGraphPayload)) => {
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

  // Poll status
  useEffect(() => {
    const fetchStatus = async () => {
      try {
        const currentStatus = await invoke<SimulationStatus>('get_simulation_status');
        setStatus(currentStatus);
      } catch (err) {
        setError(String(err));
      }
    };

    fetchStatus();
    const interval = setInterval(fetchStatus, 1000);
    return () => clearInterval(interval);
  }, []);

  // Fetch initial env
  useEffect(() => {
    const fetchEnv = async () => {
      try {
        const env = await invoke<EnvironmentalState>('get_environmental_elements');
        if (env) {
          setEnvironmentalState(env);
        }
      } catch (err) {
        // Ignore
      }
    };
    fetchEnv();
  }, []);

  // Fetch initial grid
  useEffect(() => {
    const fetchGrid = async () => {
      try {
        const grid = await invoke<MapElitesGridState>('get_map_elites_grid');
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

  // Fetch initial Phase 3 states
  useEffect(() => {
    const fetchPhase3Initial = async () => {
      try {
        const grid = await invoke<PheromoneGridState>('get_pheromone_grid');
        setPheromoneGrid(grid);
      } catch (err) {
        // Ignore
      }
      try {
        const raycasts = await invoke<RaycastTelemetry[]>('get_active_raycasts');
        setActiveRaycasts(raycasts);
      } catch (err) {
        // Ignore
      }
    };
    fetchPhase3Initial();
  }, []);

  // Fetch initial Phase 4 states
  useEffect(() => {
    const fetchPhase4Initial = async () => {
      try {
        const lineage = await invoke<LineageGraphPayload>('get_lineage_graph');
        setLineageGraph(lineage ? {
          nodes: Array.isArray(lineage.nodes) ? lineage.nodes : [],
          links: Array.isArray(lineage.links) ? lineage.links : [],
          db_connected: !!lineage.db_connected
        } : { nodes: [], links: [], db_connected: false });
      } catch (err) {
        console.error('Failed to load lineage graph:', err);
      }
      try {
        const history = await invoke<ChronicleEvent[]>('get_chronicle_history');
        setChronicleHistory(Array.isArray(history) ? history : []);
      } catch (err) {
        console.error('Failed to load chronicle history:', err);
      }
    };
    fetchPhase4Initial();
  }, []);

  // Listen to simulation-tick
  useTauriEvent<any>('simulation-tick', (event) => {
    let newSegments: SegmentState[] = [];
    if (event.payload) {
      if (Array.isArray(event.payload)) {
        newSegments = event.payload;
      } else if (event.payload && typeof event.payload === 'object' && Array.isArray(event.payload.segments)) {
        newSegments = event.payload.segments;
      }
    }

    const safeSegments = Array.isArray(newSegments)
      ? newSegments.filter(
          (seg): seg is SegmentState => seg !== null && seg !== undefined && typeof seg === 'object'
        )
      : [];

    const hydrations = Array.isArray(safeSegments)
      ? safeSegments.map(s => s.hydration).filter((h): h is number => typeof h === 'number')
      : [];
    if (hydrations.length > 0) {
      const avg = hydrations.reduce((a, b) => a + b, 0) / hydrations.length;
      setAvgHydration(avg);
    }
    const headDirs = Array.isArray(safeSegments)
      ? safeSegments.map(s => s.head_direction).filter((d): d is [number, number, number] => Array.isArray(d))
      : [];
    if (headDirs.length > 0) {
      setHeadDirection(headDirs[0]);
    } else if (
      event.payload &&
      Array.isArray(event.payload.head_directions) &&
      event.payload.head_directions.length > 0 &&
      event.payload.head_directions[0] &&
      Array.isArray(event.payload.head_directions[0].direction)
    ) {
      setHeadDirection(event.payload.head_directions[0].direction);
    }

    const now = Date.now();
    if (now - lastHierarchiesUpdateRef.current >= 200) {
      const newHierarchies = Array.isArray(safeSegments) ? buildAgentHierarchy(safeSegments) : [];
      setHierarchies(newHierarchies);
      lastHierarchiesUpdateRef.current = now;
    }
  });

  // Listen to map-elites-update
  useTauriEvent<MapElitesGridState>('map-elites-update', (event) => {
    setMapElitesGrid(event.payload);
  });

  // Listen to pheromone-update
  useTauriEvent<PheromoneGridState>('pheromone-update', (event) => {
    setPheromoneGrid(event.payload);
  });

  // Listen to raycast-update
  useTauriEvent<RaycastTelemetry[]>('raycast-update', (event) => {
    setActiveRaycasts(event.payload);
  });

  // Listen to combat-event
  useTauriEvent<CombatEvent>('combat-event', (event) => {
    setCombatEvents((prev) => [event.payload, ...prev].slice(0, 50));
  });

  // Listen to chronicle-event
  useTauriEvent<ChronicleEvent>('chronicle-event', (event) => {
    setChronicleHistory((prev) => {
      const safePrev = Array.isArray(prev) ? prev : [];
      return event?.payload ? [event.payload, ...safePrev] : safePrev;
    });
  });

  // Listen to migration-event
  useTauriEvent<MigrationPayload>('migration-event', (event) => {
    setMigrationEvents((prev) => [event.payload, ...prev]);
  });

  // Control Actions
  const handleToggle = async () => {
    try {
      const isRunning = await invoke<boolean>('toggle_simulation');
      setStatus((prev) => ({ ...prev, running: isRunning }));
    } catch (err) {
      setError(String(err));
    }
  };

  const handleSaveState = async () => {
    if (!filePath || filePath.trim() === '') {
      setError('File path cannot be empty.');
      return;
    }
    try {
      await invoke('save_simulation_state', { file_path: filePath });
    } catch (err) {
      setError(String(err));
    }
  };

  const handleLoadState = async () => {
    if (!filePath || filePath.trim() === '') {
      setError('File path cannot be empty.');
      return;
    }
    try {
      await invoke('load_simulation_state', { file_path: filePath });
    } catch (err) {
      setError(String(err));
    }
  };

  const handleZoom = (amount: number) => {
    setZoom(prev => Math.min(10.0, Math.max(0.1, prev + amount)));
  };

  const handlePan = (dx: number, dy: number) => {
    setPan(prev => ({ x: prev.x + dx, y: prev.y + dy }));
  };

  const handleMutationRateChange = async (newRate: number) => {
    setMutationRate(newRate);
    try {
      await invoke('update_evolution_settings', {
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
      await invoke('update_evolution_settings', {
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
      const running = await invoke<boolean>('toggle_evolution');
      setEvolutionRunning(running);
    } catch (err) {
      setError(String(err));
    }
  };

  return {
    status,
    setStatus,
    hierarchies,
    error,
    setError,
    projection,
    setProjection,
    filePath,
    setFilePath,
    zoom,
    setZoom,
    pan,
    setPan,
    environmentalState,
    avgHydration,
    headDirection,
    mapElitesGrid,
    mutationRate,
    selectionBias,
    gridResolution,
    evolutionRunning,
    pheromoneGrid,
    activeRaycasts,
    combatEvents,
    lineageGraph,
    chronicleHistory,
    migrationEvents,
    targetPort,
    setTargetPort,
    handleToggle,
    handleSaveState,
    handleLoadState,
    handleZoom,
    handlePan,
    handleMutationRateChange,
    handleSelectionBiasChange,
    handleToggleEvolution
  };
}
