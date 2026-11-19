import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { surfaceHeight } from './utils/worldSample';
import type { TickSource } from './utils/tickRecording';
import { isCaptureRequested } from './utils/captureMode';
import {
  TickInterpolationBuffer,
  type InterpolatedAgentView,
} from './agentInterpolation';
import {
  CREATURE_BODY_SCALE,
  computeCreatureColor,
  computeFarMarkerScale,
  computeHeadBasis,
} from './utils/agentPose';
import type { QualityTier } from './utils/qualityTiers';
import {
  CREATURE_NEAR_SKINNED_CAP,
  CreatureLodSelector,
  LOD_CULLED,
  LOD_FAR_MARKER,
  LOD_NEAR_SKINNED,
  resolveCreatureQualityTier,
} from './utils/creatureLod';
import { SkinnedCreaturePool } from './utils/creatureSkinMesh';
import type { AgentType } from '../../types/generated/AgentType';
import type { SimulationTickPayload } from '../../types/generated/SimulationTickPayload';

// ---------------------------------------------------------------------------------------
// LiveAgents — articulated multi-segment creatures (RW2-1 .. RW2-7 + B1 prototype, to be replaced).
//
// Three-tier LOD with per-creature frustum culling (`creatureLod.ts`):
//   - Near (`<= creatureFullDetailRadius`, capped at K = 8/14/24): continuous `SkinnedMesh`
//     capsule-chain bodies (`live-agents-skinned`, cached per morphology archetype)
//   - Mid (`<= creatureSimplifiedRadius` + overflow): instanced segment cylinders + joints
//   - Far (`> creatureSimplifiedRadius`): instanced billboard markers (`live-agents-markers`)
//   Plus shared instanced head spheres/eyes (`live-agents-spheres`), predator snouts
//   (`live-agents-snouts`), and contact shadows (`live-agents-shadows`), keeping total
//   creature draw calls within `5 + K` (<= 13/19/29) across low/medium/high tiers.
//
// Plus optional single selection ring + payload-only HUD when the user clicks a creature (RW2-7).
// ---------------------------------------------------------------------------------------

export interface LiveAgentsProps {
  world: World;
  renderSize: number;
  heightRatio: number;
  meshResolution: number;
  tickSource?: TickSource | null;
  nowMs?: () => number;
  qualityTier?: QualityTier;
}

interface SelectedAgentSummary {
  agentId: number;
  agentType: AgentType | null;
  energy: number;
  hydration: number;
  segmentCount: number;
  alive: boolean;
}

interface HudFieldRefs {
  root: HTMLDivElement;
  idValue: HTMLElement;
  typeValue: HTMLElement;
  energyValue: HTMLElement;
  hydrationValue: HTMLElement;
  segmentsValue: HTMLElement;
  statusValue: HTMLElement;
}

/** Instances allocated initially; doubles on overflow to keep remounts rare. */
const INITIAL_CAPACITY = 4096;
const MAX_NEAR_SKINNED_SLOTS = Math.max(
  CREATURE_NEAR_SKINNED_CAP.low,
  CREATURE_NEAR_SKINNED_CAP.medium,
  CREATURE_NEAR_SKINNED_CAP.high,
);
const EYE_COLOR = new THREE.Color('#11161d');
const SELECTION_RING_COLOR = new THREE.Color('#fde047');
const LOCAL_FORWARD = new THREE.Vector3(0, 0, 1);
const FALLBACK_CAM_POS = { x: 0, y: 0, z: 0 } as const;

const defaultNowMs = (): number =>
  typeof performance !== 'undefined' ? performance.now() : Date.now();

function readGlPixelRatio(gl: unknown): number {
  if (!gl || typeof gl !== 'object') return 1;
  const reader = (gl as { getPixelRatio?: () => number }).getPixelRatio;
  if (typeof reader !== 'function') return 1;
  const ratio = reader.call(gl);
  return typeof ratio === 'number' && Number.isFinite(ratio) && ratio > 0 ? ratio : 1;
}

function needsReplayOrRecorderModule(search: string): boolean {
  if (!search) return false;
  const params = new URLSearchParams(search);
  const replay = params.get('replay');
  return (replay !== null && replay.trim() !== '') || params.get('record') === '1';
}

function createDefaultLiveTauriSource(): TickSource {
  return {
    kind: 'tauri',
    screenLabel: null,
    subscribe(listener) {
      if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
        return () => {};
      }
      let unlisten: (() => void) | undefined;
      let cancelled = false;
      void import('@tauri-apps/api/event')
        .then(({ listen }) =>
          listen<SimulationTickPayload>('simulation-tick', (event) => {
            if (!cancelled) listener(event.payload);
          }),
        )
        .then((stop) => {
          if (cancelled) stop();
          else unlisten = stop;
        })
        .catch(() => {});
      return () => {
        cancelled = true;
        unlisten?.();
      };
    },
  };
}

function nextCapacity(needed: number): number {
  let capacity = INITIAL_CAPACITY;
  while (capacity < needed) capacity *= 2;
  return capacity;
}

function assignRaycastSphere(mesh: THREE.InstancedMesh | null, sphere: THREE.Sphere): void {
  if (mesh && typeof mesh === 'object' && 'isInstancedMesh' in mesh) {
    mesh.boundingSphere = sphere;
  }
}

function resetInstancedEmpty(mesh: (THREE.InstancedMesh & { visible?: boolean }) | null): void {
  if (!mesh) return;
  mesh.count = 0;
  mesh.visible = false;
}

interface AgentIdLookups {
  capacity: number;
  segmentAgentIds: Int32Array;
  sphereAgentIds: Int32Array;
  snoutAgentIds: Int32Array;
  markerAgentIds: Int32Array;
}

function createLookups(capacity: number): AgentIdLookups {
  return {
    capacity,
    segmentAgentIds: new Int32Array(capacity),
    sphereAgentIds: new Int32Array(capacity * 3),
    snoutAgentIds: new Int32Array(capacity),
    markerAgentIds: new Int32Array(capacity),
  };
}

function updateSelectedSummaryInto(
  out: SelectedAgentSummary,
  agents: readonly InterpolatedAgentView[],
  agentCount: number,
  agentId: number,
): SelectedAgentSummary {
  for (let i = 0; i < agentCount; i++) {
    const p = agents[i];
    if (p.agentId === agentId) {
      out.agentId = p.agentId;
      out.agentType = p.agentType;
      out.energy = p.energy;
      out.hydration = p.hydration;
      out.segmentCount = p.segmentCount;
      out.alive = true;
      return out;
    }
  }
  if (out.agentId === agentId) {
    out.alive = false;
    return out;
  }
  out.agentId = agentId;
  out.agentType = null;
  out.energy = 0;
  out.hydration = 0;
  out.segmentCount = 0;
  out.alive = false;
  return out;
}

function updateHudDom(els: HudFieldRefs, summary: SelectedAgentSummary): void {
  const idText = String(summary.agentId);
  if (els.idValue.textContent !== idText) els.idValue.textContent = idText;

  const typeText = summary.agentType ?? 'untyped';
  if (els.typeValue.textContent !== typeText) els.typeValue.textContent = typeText;

  const energyText = summary.alive ? summary.energy.toFixed(1) : '0.0';
  if (els.energyValue.textContent !== energyText) els.energyValue.textContent = energyText;

  const hydrationText = summary.alive ? summary.hydration.toFixed(1) : '0.0';
  if (els.hydrationValue.textContent !== hydrationText) {
    els.hydrationValue.textContent = hydrationText;
  }

  const segText = String(summary.segmentCount);
  if (els.segmentsValue.textContent !== segText) els.segmentsValue.textContent = segText;

  const statusText = summary.alive ? 'in latest tick' : 'absent from latest tick';
  if (els.statusValue.textContent !== statusText) els.statusValue.textContent = statusText;
}

function buildSelectionHud(onClose: () => void): HudFieldRefs {
  const root = document.createElement('div');
  root.setAttribute('data-testid', 'rw-selected-agent-hud');
  root.style.position = 'fixed';
  root.style.right = '12px';
  root.style.top = '12px';
  root.style.zIndex = '120';
  root.style.minWidth = '196px';
  root.style.padding = '8px 10px';
  root.style.borderRadius = '6px';
  root.style.background = 'rgba(9, 13, 20, 0.88)';
  root.style.border = '1px solid rgba(253, 224, 71, 0.55)';
  root.style.color = '#f8fafc';
  root.style.fontFamily = 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace';
  root.style.fontSize = '11px';
  root.style.lineHeight = '1.45';
  root.style.pointerEvents = 'auto';

  const header = document.createElement('div');
  header.style.display = 'flex';
  header.style.alignItems = 'center';
  header.style.justifyContent = 'space-between';
  header.style.gap = '10px';
  header.style.marginBottom = '4px';

  const title = document.createElement('strong');
  title.textContent = 'Selected Agent';
  title.style.color = '#fde047';

  const closeBtn = document.createElement('button');
  closeBtn.type = 'button';
  closeBtn.setAttribute('data-testid', 'rw-selected-agent-close');
  closeBtn.textContent = '×';
  closeBtn.style.background = 'transparent';
  closeBtn.style.border = '1px solid rgba(255,255,255,0.25)';
  closeBtn.style.borderRadius = '4px';
  closeBtn.style.color = '#e2e8f0';
  closeBtn.style.cursor = 'pointer';
  closeBtn.style.padding = '0 5px';
  closeBtn.onclick = onClose;

  header.appendChild(title);
  header.appendChild(closeBtn);
  root.appendChild(header);

  const makeRow = (label: string, testId: string): HTMLElement => {
    const row = document.createElement('div');
    row.style.display = 'flex';
    row.style.justifyContent = 'space-between';
    row.style.gap = '12px';
    const keySpan = document.createElement('span');
    keySpan.textContent = label;
    keySpan.style.color = '#94a3b8';
    const valSpan = document.createElement('span');
    valSpan.setAttribute('data-testid', testId);
    valSpan.style.fontWeight = '600';
    row.appendChild(keySpan);
    row.appendChild(valSpan);
    root.appendChild(row);
    return valSpan;
  };

  const idValue = makeRow('agent_id', 'rw-selected-agent-id');
  const typeValue = makeRow('type', 'rw-selected-agent-type');
  const energyValue = makeRow('energy', 'rw-selected-agent-energy');
  const hydrationValue = makeRow('hydration', 'rw-selected-agent-hydration');
  const segmentsValue = makeRow('segments', 'rw-selected-agent-segments');
  const statusValue = makeRow('tick', 'rw-selected-agent-status');

  return { root, idValue, typeValue, energyValue, hydrationValue, segmentsValue, statusValue };
}

export const LiveAgents: React.FC<LiveAgentsProps> = ({
  world,
  renderSize,
  heightRatio,
  meshResolution,
  tickSource,
  nowMs = defaultNowMs,
  qualityTier,
}) => {
  const segmentsMeshRef = useRef<THREE.InstancedMesh>(null);
  const spheresMeshRef = useRef<THREE.InstancedMesh>(null);
  const snoutsMeshRef = useRef<THREE.InstancedMesh>(null);
  const shadowsMeshRef = useRef<THREE.InstancedMesh>(null);
  const markersMeshRef = useRef<THREE.InstancedMesh>(null);
  const skinnedGroupRef = useRef<THREE.Group>(null);
  const selectionRingRef = useRef<THREE.Mesh>(null);

  const bufferRef = useRef<TickInterpolationBuffer | null>(null);
  const lodSelectorRef = useRef<CreatureLodSelector | null>(null);
  const skinPoolRef = useRef<SkinnedCreaturePool | null>(null);

  const [capacity, setCapacity] = useState(INITIAL_CAPACITY);
  const [connected, setConnected] = useState(false);
  const [selectedAgentId, setSelectedAgentId] = useState<number | null>(null);
  const selectedAgentIdRef = useRef<number | null>(null);
  const selectedSummaryRef = useRef<SelectedAgentSummary | null>(null);
  const hudRefs = useRef<HudFieldRefs | null>(null);

  const [search] = useState(() => (typeof window === 'undefined' ? '' : window.location.search));
  const captureMode = useMemo(() => isCaptureRequested(search), [search]);
  const [lazyUrlState, setLazyUrlState] = useState<{
    search: string;
    source: TickSource | null;
  } | null>(null);

  useEffect(() => {
    if (tickSource !== undefined || !needsReplayOrRecorderModule(search)) return;
    let cancelled = false;
    void import('./utils/tickRecording').then((mod) => {
      if (!cancelled) {
        setLazyUrlState({ search, source: mod.resolveTickSourceFromSearch(search) });
      }
    });
    return () => {
      cancelled = true;
    };
  }, [tickSource, search]);

  const activeSource = useMemo(() => {
    if (tickSource !== undefined) return tickSource;
    if (needsReplayOrRecorderModule(search)) {
      return lazyUrlState && lazyUrlState.search === search ? lazyUrlState.source : null;
    }
    if (captureMode) return null;
    return createDefaultLiveTauriSource();
  }, [tickSource, search, lazyUrlState, captureMode]);

  const sphereCapacity = capacity * 3;

  const ensureBuffer = useCallback((): TickInterpolationBuffer => {
    if (!bufferRef.current) {
      bufferRef.current = new TickInterpolationBuffer();
    }
    return bufferRef.current;
  }, []);

  const ensureSummary = useCallback((): SelectedAgentSummary => {
    if (!selectedSummaryRef.current) {
      selectedSummaryRef.current = {
        agentId: 0,
        agentType: null,
        energy: 0,
        hydration: 0,
        segmentCount: 0,
        alive: false,
      };
    }
    return selectedSummaryRef.current;
  }, []);

  // Lookup tables held in a ref (resized when capacity grows, never inside useFrame steady-state)
  // so raycast clicks map instanceId -> agentId with zero per-frame allocations.
  const lookupsRef = useRef<AgentIdLookups | null>(null);
  useLayoutEffect(() => {
    ensureBuffer();
    ensureSummary();
    if (!lookupsRef.current || lookupsRef.current.capacity < capacity) {
      lookupsRef.current = createLookups(capacity);
    }
  }, [capacity, ensureBuffer, ensureSummary]);

  // Scratch math objects allocated once and reused across frames.
  const scratch = useMemo(
    () => ({
      matrix: new THREE.Matrix4(),
      color: new THREE.Color(),
      partColor: new THREE.Color(),
      pos: new THREE.Vector3(),
      quat: new THREE.Quaternion(),
      rotZ45: new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), Math.PI / 4),
      scale: new THREE.Vector3(),
      forward: new THREE.Vector3(),
      right: new THREE.Vector3(),
      up: new THREE.Vector3(),
    }),
    [],
  );

  const geometries = useMemo(() => {
    const segment = new THREE.CylinderGeometry(0.92, 1.0, 1, 6, 1);
    segment.rotateX(Math.PI / 2);

    const sphere = new THREE.SphereGeometry(1, 5, 4);

    const snout = new THREE.ConeGeometry(1, 1, 5, 1);
    snout.rotateX(Math.PI / 2);

    const shadow = new THREE.CircleGeometry(1, 8);
    shadow.rotateX(-Math.PI / 2);

    const marker = new THREE.PlaneGeometry(1, 1);

    const ring = new THREE.RingGeometry(0.76, 1.0, 24);
    ring.rotateX(-Math.PI / 2);

    return { segment, sphere, snout, shadow, marker, ring };
  }, []);

  const materials = useMemo(() => {
    const body = new THREE.MeshStandardMaterial({
      roughness: 0.58,
      metalness: 0.06,
      flatShading: true,
    });
    const feature = new THREE.MeshStandardMaterial({
      roughness: 0.5,
      metalness: 0.05,
      flatShading: true,
    });
    const snout = new THREE.MeshStandardMaterial({
      roughness: 0.48,
      metalness: 0.08,
      flatShading: true,
    });
    const shadow = new THREE.MeshBasicMaterial({
      color: '#070b10',
      transparent: true,
      opacity: 0.38,
      depthWrite: false,
      polygonOffset: true,
      polygonOffsetFactor: -2,
      polygonOffsetUnits: -2,
    });
    const marker = new THREE.MeshBasicMaterial({
      side: THREE.DoubleSide,
      toneMapped: false,
    });
    const ring = new THREE.MeshBasicMaterial({
      color: SELECTION_RING_COLOR,
      side: THREE.DoubleSide,
      transparent: true,
      opacity: 0.92,
      depthWrite: false,
    });
    return { body, feature, snout, shadow, marker, ring };
  }, []);

  const worldBoundsSphere = useMemo(
    () => new THREE.Sphere(new THREE.Vector3(0, 0, 0), Math.max(renderSize, 200) * 2),
    [renderSize],
  );

  useEffect(() => {
    return () => {
      geometries.segment.dispose();
      geometries.sphere.dispose();
      geometries.snout.dispose();
      geometries.shadow.dispose();
      geometries.marker.dispose();
      geometries.ring.dispose();
    };
  }, [geometries]);

  useEffect(() => {
    return () => {
      materials.body.dispose();
      materials.feature.dispose();
      materials.snout.dispose();
      materials.shadow.dispose();
      materials.marker.dispose();
      materials.ring.dispose();
    };
  }, [materials]);

  useLayoutEffect(() => {
    if (!lodSelectorRef.current) {
      lodSelectorRef.current = new CreatureLodSelector();
    }
    if (!skinPoolRef.current) {
      skinPoolRef.current = new SkinnedCreaturePool(MAX_NEAR_SKINNED_SLOTS);
    }
    return () => {
      skinPoolRef.current?.dispose();
      skinPoolRef.current = null;
    };
  }, []);

  useLayoutEffect(() => {
    if (!connected) return;
    resetInstancedEmpty(segmentsMeshRef.current);
    resetInstancedEmpty(spheresMeshRef.current);
    resetInstancedEmpty(snoutsMeshRef.current);
    resetInstancedEmpty(shadowsMeshRef.current);
    resetInstancedEmpty(markersMeshRef.current);
    assignRaycastSphere(segmentsMeshRef.current, worldBoundsSphere);
    assignRaycastSphere(spheresMeshRef.current, worldBoundsSphere);
    assignRaycastSphere(snoutsMeshRef.current, worldBoundsSphere);
    assignRaycastSphere(markersMeshRef.current, worldBoundsSphere);
    skinPoolRef.current?.bindGroup(skinnedGroupRef.current);
  }, [connected, capacity, worldBoundsSphere]);

  const sampleGroundY = useCallback(
    (rx: number, rz: number): number =>
      surfaceHeight(world, rx, rz, renderSize, heightRatio, meshResolution),
    [world, renderSize, heightRatio, meshResolution],
  );

  const renderSizeRef = useRef(renderSize);
  const sampleGroundYRef = useRef(sampleGroundY);
  const nowMsRef = useRef(nowMs);
  const qualityTierRef = useRef(qualityTier);
  const searchRef = useRef(search);

  // Sync world/clock/quality refs and rebuild cached buffered snapshots whenever terrain parameters change.
  useEffect(() => {
    renderSizeRef.current = renderSize;
    sampleGroundYRef.current = sampleGroundY;
    nowMsRef.current = nowMs;
    qualityTierRef.current = qualityTier;
    searchRef.current = search;
    ensureBuffer().rebuildForWorld(renderSize, sampleGroundY, CREATURE_BODY_SCALE);
  }, [renderSize, sampleGroundY, nowMs, qualityTier, search, ensureBuffer]);

  useEffect(() => {
    selectedAgentIdRef.current = selectedAgentId;
    if (selectedAgentId === null || captureMode || typeof document === 'undefined') {
      hudRefs.current = null;
      return;
    }

    const showcaseEl = document.querySelector('[data-testid="world-showcase"]');
    const host = showcaseEl instanceof HTMLElement ? showcaseEl : document.body;
    const latest = ensureBuffer().sampleLatest();
    const summary = updateSelectedSummaryInto(
      ensureSummary(),
      latest.agents,
      latest.agentCount,
      selectedAgentId,
    );

    const els = buildSelectionHud(() => {
      selectedAgentIdRef.current = null;
      setSelectedAgentId(null);
    });
    updateHudDom(els, summary);
    hudRefs.current = els;
    host.appendChild(els.root);

    return () => {
      if (hudRefs.current === els) hudRefs.current = null;
      els.root.remove();
    };
  }, [selectedAgentId, captureMode, ensureBuffer, ensureSummary]);

  useEffect(() => {
    const buffer = ensureBuffer();
    buffer.clear();
    if (!activeSource) return;
    const stop = activeSource.subscribe((payload) => {
      buffer.push(
        payload,
        nowMsRef.current(),
        renderSizeRef.current,
        sampleGroundYRef.current,
        CREATURE_BODY_SCALE,
      );
      if (payload.segments.length > 0 || buffer.hasActiveAgents()) {
        setConnected(true);
      }
    });
    return () => {
      stop();
      buffer.clear();
    };
  }, [activeSource, ensureBuffer]);

  useEffect(() => {
    if (!activeSource?.mountOverlay || typeof document === 'undefined') return;
    const showcaseEl = document.querySelector('[data-testid="world-showcase"]');
    const host = showcaseEl instanceof HTMLElement ? showcaseEl : document.body;
    return activeSource.mountOverlay(host);
  }, [activeSource]);

  const selectByLookup = useCallback(
    (lookup: Int32Array | undefined, e: THREE.Intersection & { stopPropagation(): void }) => {
      if (!lookup || typeof e.instanceId !== 'number') return;
      const agentId = lookup[e.instanceId];
      if (!agentId) return;
      e.stopPropagation();
      selectedAgentIdRef.current = agentId;
      setSelectedAgentId(agentId);
    },
    [],
  );

  const handleSegmentClick = useCallback(
    (e: THREE.Intersection & { stopPropagation(): void }) =>
      selectByLookup(lookupsRef.current?.segmentAgentIds, e),
    [selectByLookup],
  );
  const handleSphereClick = useCallback(
    (e: THREE.Intersection & { stopPropagation(): void }) =>
      selectByLookup(lookupsRef.current?.sphereAgentIds, e),
    [selectByLookup],
  );
  const handleSnoutClick = useCallback(
    (e: THREE.Intersection & { stopPropagation(): void }) =>
      selectByLookup(lookupsRef.current?.snoutAgentIds, e),
    [selectByLookup],
  );
  const handleMarkerClick = useCallback(
    (e: THREE.Intersection & { stopPropagation(): void }) =>
      selectByLookup(lookupsRef.current?.markerAgentIds, e),
    [selectByLookup],
  );
  const handleSkinnedClick = useCallback(
    (e: THREE.Intersection & { stopPropagation(): void }) => {
      const agentId = e.object?.userData?.agentId;
      if (typeof agentId !== 'number' || !agentId) return;
      e.stopPropagation();
      selectedAgentIdRef.current = agentId;
      setSelectedAgentId(agentId);
    },
    [],
  );

  useFrame((state) => {
    const segMesh = segmentsMeshRef.current;
    const sphereMesh = spheresMeshRef.current;
    const snoutMesh = snoutsMeshRef.current;
    const shadowMesh = shadowsMeshRef.current;
    const markerMesh = markersMeshRef.current;
    const skinPool = skinPoolRef.current;
    const lodSelector = lodSelectorRef.current;
    if (!segMesh || !sphereMesh || !snoutMesh || !shadowMesh || !markerMesh) return;
    if (!skinPool || !lodSelector) return;

    const buffer = bufferRef.current;
    const lookups = lookupsRef.current;
    if (!buffer || !buffer.hasSnapshots() || !lookups) return;

    const nowMsVal = nowMsRef.current();
    const frame = captureMode
      ? buffer.sampleLatest()
      : buffer.sample(nowMsVal, renderSize, sampleGroundY);

    if (frame.totalSegments > capacity) {
      setCapacity(nextCapacity(frame.totalSegments));
      return;
    }

    const camPos = state.camera?.position ?? FALLBACK_CAM_POS;
    const camQuat = state.camera?.quaternion;
    const camFov =
      state.camera && 'fov' in state.camera && typeof state.camera.fov === 'number'
        ? state.camera.fov
        : 55;
    const viewportHeight = state.size?.height ?? 768;
    const realShadowsEnabled = Boolean(state.gl?.shadowMap?.enabled);
    const activeTier = resolveCreatureQualityTier(
      qualityTierRef.current,
      searchRef.current,
      realShadowsEnabled,
      readGlPixelRatio(state.gl),
    );
    const timeSec = nowMsVal * 0.001;

    const agents = frame.agents;
    const agentCount = frame.agentCount;
    const lod = lodSelector.select(agents, agentCount, state.camera, activeTier, renderSize);
    skinPool.bindGroup(skinnedGroupRef.current);

    const { matrix, color, partColor, pos, quat, rotZ45, scale, forward, right, up } = scratch;
    let nearSlot = 0;
    let segDrawn = 0;
    let sphereDrawn = 0;
    let snoutDrawn = 0;
    let shadowDrawn = 0;
    let markerDrawn = 0;

    const currentSelectedId = selectedAgentIdRef.current;
    let selectedPose: InterpolatedAgentView | null = null;

    for (let pIdx = 0; pIdx < agentCount; pIdx++) {
      const pose = agents[pIdx];
      if (currentSelectedId !== null && pose.agentId === currentSelectedId) {
        selectedPose = pose;
      }

      const lodTier = lod.tiers[pIdx];
      if (lodTier === LOD_CULLED) {
        continue;
      }

      computeCreatureColor(pose.agentType, pose.agentId, pose.energy, color);

      if (lodTier === LOD_FAR_MARKER) {
        const dx = camPos.x - pose.rootPosition[0];
        const dy = camPos.y - pose.rootPosition[1];
        const dz = camPos.z - pose.rootPosition[2];
        const dist = Math.hypot(dx, dy, dz);
        const markerSize = computeFarMarkerScale(dist, viewportHeight, camFov, 6.4);

        pos.set(pose.rootPosition[0], pose.groundY + Math.max(2.5, markerSize * 0.45), pose.rootPosition[2]);
        if (camQuat) quat.copy(camQuat);
        else quat.identity();

        if (pose.agentType === 'predator') {
          quat.multiply(rotZ45);
          scale.set(markerSize * 1.15, markerSize * 1.15, 1);
        } else {
          scale.set(markerSize * 1.25, markerSize * 0.82, 1);
        }
        matrix.compose(pos, quat, scale);
        markerMesh.setMatrixAt(markerDrawn, matrix);

        // Keep far markers legible even for low-energy creatures while preserving species hue.
        computeCreatureColor(pose.agentType, pose.agentId, Math.max(60, pose.energy), partColor);
        markerMesh.setColorAt(markerDrawn, partColor);
        lookups.markerAgentIds[markerDrawn] = pose.agentId;
        markerDrawn++;
        continue;
      }

      const useSkinned =
        lodTier === LOD_NEAR_SKINNED &&
        nearSlot < skinPool.maxSlots() &&
        skinPool.updateSlot(nearSlot, pose, timeSec, scratch);

      if (useSkinned) {
        nearSlot++;
      } else {
        // Mid mode: articulated cylinders sized by (radius, length) + joint spheres.
        const segCount = pose.segmentCount;
        for (let sIdx = 0; sIdx < segCount; sIdx++) {
          const segPose = pose.segments[sIdx];
          pos.set(
            segPose.renderPosition[0],
            segPose.renderPosition[1],
            segPose.renderPosition[2],
          );
          quat.set(
            segPose.quaternion[0],
            segPose.quaternion[1],
            segPose.quaternion[2],
            segPose.quaternion[3],
          );
          scale.set(segPose.renderRadius, segPose.renderRadius, segPose.renderLength);
          matrix.compose(pos, quat, scale);
          segMesh.setMatrixAt(segDrawn, matrix);
          segMesh.setColorAt(segDrawn, color);
          lookups.segmentAgentIds[segDrawn] = pose.agentId;
          segDrawn++;

          if (segPose.hasJoint && sphereDrawn < sphereCapacity) {
            pos.set(
              segPose.jointRenderPosition[0],
              segPose.jointRenderPosition[1],
              segPose.jointRenderPosition[2],
            );
            quat.identity();
            const jr = segPose.jointRenderRadius;
            scale.set(jr, jr, jr);
            matrix.compose(pos, quat, scale);
            sphereMesh.setMatrixAt(sphereDrawn, matrix);
            partColor.copy(color).multiplyScalar(0.88);
            sphereMesh.setColorAt(sphereDrawn, partColor);
            lookups.sphereAgentIds[sphereDrawn] = pose.agentId;
            sphereDrawn++;
          }
        }
      }

      forward.set(pose.headDirection[0], pose.headDirection[1], pose.headDirection[2]);
      if (forward.lengthSq() <= 1e-8) forward.set(0, 0, 1);
      else forward.normalize();
      computeHeadBasis(forward, right, up);
      quat.setFromUnitVectors(LOCAL_FORWARD, forward);

      const rRx = pose.rootPosition[0];
      const rRy = pose.rootPosition[1];
      const rRz = pose.rootPosition[2];
      const rLen = pose.rootRenderLength;
      const rRad = pose.rootRenderRadius;
      const eyeRadius = Math.max(rRad * 0.28, 0.22);

      if (pose.agentType === 'predator') {
        const snoutLen = useSkinned ? Math.max(rLen * 0.65, rRad * 2.1) : Math.max(rLen * 0.92, rRad * 2.9);
        const snoutRad = useSkinned ? rRad * 0.82 : rRad * 1.06;
        pos.set(
          rRx + forward.x * (rLen * 0.44 + snoutLen * 0.42),
          rRy + forward.y * (rLen * 0.44 + snoutLen * 0.42),
          rRz + forward.z * (rLen * 0.44 + snoutLen * 0.42),
        );
        scale.set(snoutRad, snoutRad * 0.85, snoutLen);
        matrix.compose(pos, quat, scale);
        snoutMesh.setMatrixAt(snoutDrawn, matrix);
        partColor.copy(color).multiplyScalar(1.08);
        snoutMesh.setColorAt(snoutDrawn, partColor);
        lookups.snoutAgentIds[snoutDrawn] = pose.agentId;
        snoutDrawn++;

        const eyeFwd = rLen * 0.48 + snoutLen * 0.14;
        const eyeUp = rRad * 0.56;
        const eyeSide = rRad * 0.64;
        for (let sideSign = -1; sideSign < 2; sideSign += 2) {
          if (sphereDrawn >= sphereCapacity) break;
          pos.set(
            rRx + forward.x * eyeFwd + up.x * eyeUp + right.x * eyeSide * sideSign,
            rRy + forward.y * eyeFwd + up.y * eyeUp + right.y * eyeSide * sideSign,
            rRz + forward.z * eyeFwd + up.z * eyeUp + right.z * eyeSide * sideSign,
          );
          scale.set(eyeRadius, eyeRadius, eyeRadius);
          matrix.compose(pos, quat, scale);
          sphereMesh.setMatrixAt(sphereDrawn, matrix);
          sphereMesh.setColorAt(sphereDrawn, EYE_COLOR);
          lookups.sphereAgentIds[sphereDrawn] = pose.agentId;
          sphereDrawn++;
        }
      } else {
        const headFwd = rLen * 0.48;
        const headUp = rRad * 0.12;
        if (!useSkinned && sphereDrawn < sphereCapacity) {
          pos.set(
            rRx + forward.x * headFwd + up.x * headUp,
            rRy + forward.y * headFwd + up.y * headUp,
            rRz + forward.z * headFwd + up.z * headUp,
          );
          scale.set(rRad * 1.18, rRad * 1.1, rRad * 1.24);
          matrix.compose(pos, quat, scale);
          sphereMesh.setMatrixAt(sphereDrawn, matrix);
          partColor.copy(color).multiplyScalar(1.06);
          sphereMesh.setColorAt(sphereDrawn, partColor);
          lookups.sphereAgentIds[sphereDrawn] = pose.agentId;
          sphereDrawn++;
        }

        const eyeFwd = useSkinned ? rLen * 0.5 + rRad * 0.25 : headFwd + rRad * 0.72;
        const eyeUp = useSkinned ? rRad * 0.42 : headUp + rRad * 0.45;
        const eyeSide = useSkinned ? rRad * 0.52 : rRad * 0.62;
        const activeEyeRadius = useSkinned ? Math.max(rRad * 0.24, 0.2) : eyeRadius;
        for (let sideSign = -1; sideSign < 2; sideSign += 2) {
          if (sphereDrawn >= sphereCapacity) break;
          pos.set(
            rRx + forward.x * eyeFwd + up.x * eyeUp + right.x * eyeSide * sideSign,
            rRy + forward.y * eyeFwd + up.y * eyeUp + right.y * eyeSide * sideSign,
            rRz + forward.z * eyeFwd + up.z * eyeUp + right.z * eyeSide * sideSign,
          );
          scale.set(activeEyeRadius, activeEyeRadius, activeEyeRadius);
          matrix.compose(pos, quat, scale);
          sphereMesh.setMatrixAt(sphereDrawn, matrix);
          sphereMesh.setColorAt(sphereDrawn, EYE_COLOR);
          lookups.sphereAgentIds[sphereDrawn] = pose.agentId;
          sphereDrawn++;
        }
      }

      if (!realShadowsEnabled) {
        pos.set(rRx, pose.groundY + 0.08, rRz);
        const yawAngle = Math.atan2(forward.x, forward.z);
        quat.setFromAxisAngle(up.set(0, 1, 0), yawAngle);
        const shadowW = rRad * 1.65;
        const shadowL = Math.max(rLen * 0.85, rRad * 1.85);
        scale.set(shadowW, 1, shadowL);
        matrix.compose(pos, quat, scale);
        shadowMesh.setMatrixAt(shadowDrawn, matrix);
        shadowDrawn++;
      }
    }

    skinPool.hideFrom(nearSlot);

    segMesh.count = segDrawn;
    segMesh.visible = segDrawn > 0;
    if (segMesh.instanceMatrix) segMesh.instanceMatrix.needsUpdate = true;
    if (segMesh.instanceColor) segMesh.instanceColor.needsUpdate = true;

    sphereMesh.count = sphereDrawn;
    sphereMesh.visible = sphereDrawn > 0;
    if (sphereMesh.instanceMatrix) sphereMesh.instanceMatrix.needsUpdate = true;
    if (sphereMesh.instanceColor) sphereMesh.instanceColor.needsUpdate = true;

    snoutMesh.count = snoutDrawn;
    snoutMesh.visible = snoutDrawn > 0;
    if (snoutMesh.instanceMatrix) snoutMesh.instanceMatrix.needsUpdate = true;
    if (snoutMesh.instanceColor) snoutMesh.instanceColor.needsUpdate = true;

    shadowMesh.count = shadowDrawn;
    shadowMesh.visible = shadowDrawn > 0;
    if (shadowMesh.instanceMatrix) shadowMesh.instanceMatrix.needsUpdate = true;

    markerMesh.count = markerDrawn;
    markerMesh.visible = markerDrawn > 0;
    if (markerMesh.instanceMatrix) markerMesh.instanceMatrix.needsUpdate = true;
    if (markerMesh.instanceColor) markerMesh.instanceColor.needsUpdate = true;

    // Selection ring & payload-only HUD sync (RW2-7).
    const ringMesh = selectionRingRef.current;
    if (currentSelectedId !== null) {
      const summaryOut = selectedSummaryRef.current;
      if (summaryOut) {
        updateSelectedSummaryInto(summaryOut, agents, agentCount, currentSelectedId);
        if (hudRefs.current) updateHudDom(hudRefs.current, summaryOut);
      }
      if (ringMesh) {
        if (selectedPose) {
          const ringRadius = Math.max(
            selectedPose.rootRenderRadius * 2.4,
            selectedPose.rootRenderLength * 0.85,
            2.5,
          );
          ringMesh.position.set(
            selectedPose.rootPosition[0],
            selectedPose.groundY + 0.18,
            selectedPose.rootPosition[2],
          );
          ringMesh.scale.set(ringRadius, 1, ringRadius);
          ringMesh.visible = true;
        } else {
          ringMesh.visible = false;
        }
      }
    } else if (ringMesh) {
      ringMesh.visible = false;
    }
  });

  if (!connected) return null;

  return (
    <group name="live-agents">
      <group
        name="live-agents-skinned"
        ref={skinnedGroupRef}
        onClick={handleSkinnedClick}
      />
      <instancedMesh
        name="live-agents-segments"
        key={`seg-${capacity}`}
        ref={segmentsMeshRef}
        args={[geometries.segment, materials.body, capacity]}
        frustumCulled={false}
        castShadow
        receiveShadow
        onClick={handleSegmentClick}
      />
      <instancedMesh
        name="live-agents-spheres"
        key={`sph-${capacity}`}
        ref={spheresMeshRef}
        args={[geometries.sphere, materials.feature, sphereCapacity]}
        frustumCulled={false}
        onClick={handleSphereClick}
      />
      <instancedMesh
        name="live-agents-snouts"
        key={`snt-${capacity}`}
        ref={snoutsMeshRef}
        args={[geometries.snout, materials.snout, capacity]}
        frustumCulled={false}
        onClick={handleSnoutClick}
      />
      <instancedMesh
        name="live-agents-shadows"
        key={`shd-${capacity}`}
        ref={shadowsMeshRef}
        args={[geometries.shadow, materials.shadow, capacity]}
        frustumCulled={false}
      />
      <instancedMesh
        name="live-agents-markers"
        key={`mrk-${capacity}`}
        ref={markersMeshRef}
        args={[geometries.marker, materials.marker, capacity]}
        frustumCulled={false}
        onClick={handleMarkerClick}
      />
      {selectedAgentId !== null ? (
        <mesh
          name="live-agents-selection-ring"
          ref={selectionRingRef}
          geometry={geometries.ring}
          material={materials.ring}
          frustumCulled={false}
        />
      ) : null}
    </group>
  );
};

export default LiveAgents;
