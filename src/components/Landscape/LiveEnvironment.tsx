import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import type { EnvironmentalState } from '../../types/generated/EnvironmentalState';
import type { SimulationTickPayload } from '../../types/generated/SimulationTickPayload';
import type { TickSource } from './utils/tickRecording';
import type { World } from './utils/worldGen';
import { surfaceHeight } from './utils/worldSample';
import { isCaptureRequested } from './utils/captureMode';
import {
  computeEntityColor,
  countsFromMapped,
  mapSimulationElements,
  type MappedSimElement,
  type MappedSimEnvironment,
  type SimEnvironmentChannel,
  type SimEnvironmentCounts,
} from './utils/simEnvironment';

// prototype, to be replaced — V1 3D simulation lakes/trees/food renderer from
// `environmental_state` and `get_environmental_elements` (no `useFrame` hook needed because
// environmental elements only update on IPC pulls or tick snapshots).

export interface LiveEnvironmentProps {
  world: World;
  renderSize: number;
  heightRatio: number;
  meshResolution: number;
  tickSource?: TickSource | null;
  channel?: SimEnvironmentChannel | null;
  fetchInitialElements?: (() => Promise<EnvironmentalState | null>) | null;
  onCountsChange?: (counts: SimEnvironmentCounts) => void;
}

const INIT_CAP = 256;
const EMPTY_MAPPED: MappedSimEnvironment = { lakes: [], trees: [], food: [], skipped: 0 };
const ZERO_COUNTS: SimEnvironmentCounts = { lakes: 0, trees: 0, food: 0, total: 0 };
const WATER_LAYER_TAG = { rwLayer: 'water' } as const;
const FLORA_LAYER_TAG = { rwLayer: 'flora' } as const;

function paintGeom(geom: THREE.BufferGeometry, hex: string): THREE.BufferGeometry {
  const c = new THREE.Color(hex);
  const count = geom.getAttribute('position')?.count ?? 0;
  const arr = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    arr[i * 3] = c.r;
    arr[i * 3 + 1] = c.g;
    arr[i * 3 + 2] = c.b;
  }
  geom.setAttribute('color', new THREE.BufferAttribute(arr, 3));
  return geom;
}

function makeTreeGeometry(): THREE.BufferGeometry {
  const parts = [
    paintGeom(new THREE.CylinderGeometry(0.16, 0.24, 1.15, 6, 1).translate(0, 0.575, 0), '#6b4c35'),
    paintGeom(new THREE.ConeGeometry(0.88, 1.35, 7, 1).translate(0, 1.65, 0), '#2f7d32'),
    paintGeom(new THREE.ConeGeometry(0.6, 1.0, 7, 1).translate(0, 2.35, 0), '#3f9142'),
  ];
  const merged = mergeGeometries(parts, false);
  if (merged) {
    parts.forEach((p) => {
      if (p !== merged) p.dispose();
    });
    return merged;
  }
  parts[1].dispose();
  parts[2].dispose();
  return parts[0];
}

function writeMesh(
  mesh: THREE.InstancedMesh | null,
  items: readonly MappedSimElement[],
  capacity: number,
  scratch: { m: THREE.Matrix4; p: THREE.Vector3; q: THREE.Quaternion; s: THREE.Vector3; c: THREE.Color },
): void {
  if (!mesh) return;
  const count = Math.min(items.length, capacity);
  const { m, p, q, s, c } = scratch;
  q.identity();
  if (typeof mesh.setMatrixAt === 'function') {
    for (let i = 0; i < count; i++) {
      const el = items[i];
      p.set(el.renderX, el.renderY, el.renderZ);
      s.set(el.renderRadius, el.heightScale, el.renderRadius);
      mesh.setMatrixAt(i, m.compose(p, q, s));
      if (typeof mesh.setColorAt === 'function') {
        mesh.setColorAt(i, computeEntityColor(el.kind, el.resources, c));
      }
    }
  }
  mesh.count = count;
  mesh.visible = count > 0;
  if (mesh.instanceMatrix) mesh.instanceMatrix.needsUpdate = true;
  if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
}

export const LiveEnvironment: React.FC<LiveEnvironmentProps> = ({
  world,
  renderSize,
  heightRatio,
  meshResolution,
  tickSource,
  channel,
  fetchInitialElements,
  onCountsChange,
}) => {
  const lakesRef = useRef<THREE.InstancedMesh>(null);
  const treesRef = useRef<THREE.InstancedMesh>(null);
  const foodRef = useRef<THREE.InstancedMesh>(null);

  const [search] = useState(() => (typeof window === 'undefined' ? '' : window.location.search));
  const captureMode = useMemo(() => isCaptureRequested(search), [search]);

  const sampleGroundY = useCallback(
    (rx: number, rz: number): number =>
      surfaceHeight(world, rx, rz, renderSize, heightRatio, meshResolution),
    [world, renderSize, heightRatio, meshResolution],
  );

  const groundRef = useRef(sampleGroundY);
  const sizeRef = useRef(renderSize);
  const onCountsRef = useRef(onCountsChange);
  const rawRef = useRef<EnvironmentalState | null>(null);
  const mappedRef = useRef<MappedSimEnvironment>(EMPTY_MAPPED);
  const hasTickRef = useRef(false);
  const countsRef = useRef<SimEnvironmentCounts>(ZERO_COUNTS);

  const [capacity, setCapacity] = useState(INIT_CAP);
  const [hasEntities, setHasEntities] = useState(false);
  const [rev, setRev] = useState(0);
  const capRef = useRef(capacity);

  const applyEnv = useCallback((env: EnvironmentalState | null, fromTick = true) => {
    if (fromTick) hasTickRef.current = true;
    rawRef.current = env;
    const mapped = env
      ? mapSimulationElements(env.elements, sizeRef.current, groundRef.current)
      : EMPTY_MAPPED;
    mappedRef.current = mapped;

    const next = countsFromMapped(mapped);
    const prev = countsRef.current;
    if (
      next.lakes !== prev.lakes ||
      next.trees !== prev.trees ||
      next.food !== prev.food ||
      next.total !== prev.total
    ) {
      countsRef.current = next;
      onCountsRef.current?.(next);
    }

    const needed = Math.max(mapped.lakes.length, mapped.trees.length, mapped.food.length);
    if (needed > capRef.current) {
      let c = INIT_CAP;
      while (c < needed) c *= 2;
      setCapacity(c);
    }
    if (next.total > 0) setHasEntities(true);
    setRev((r) => (r + 1) | 0);
  }, []);

  useEffect(() => {
    sizeRef.current = renderSize;
    groundRef.current = sampleGroundY;
    onCountsRef.current = onCountsChange;
    capRef.current = capacity;
    if (rawRef.current) {
      mappedRef.current = mapSimulationElements(rawRef.current.elements, renderSize, sampleGroundY);
    }
  }, [renderSize, sampleGroundY, onCountsChange, capacity]);

  useEffect(() => {
    const hasTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
    if (fetchInitialElements === null || (fetchInitialElements === undefined && (captureMode || !hasTauri))) {
      return;
    }
    let cancelled = false;
    void (fetchInitialElements
      ? fetchInitialElements()
      : import('@tauri-apps/api/core').then(({ invoke }) =>
          invoke<EnvironmentalState>('get_environmental_elements'),
        )
    )
      .then((env) => {
        if (!cancelled && env && !hasTickRef.current) applyEnv(env, false);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [fetchInitialElements, captureMode, applyEnv]);

  useEffect(() => {
    if (tickSource === null) return;
    if (tickSource !== undefined) {
      return tickSource.subscribe((p) => applyEnv(p.environmental_state ?? null));
    }
    if (channel) {
      return channel.subscribe((env) => applyEnv(env));
    }
    if (captureMode || typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
      return;
    }
    let stopFn: (() => void) | undefined;
    let cancelled = false;
    void import('@tauri-apps/api/event')
      .then(({ listen }) =>
        listen<SimulationTickPayload>('simulation-tick', (e) => {
          if (!cancelled) applyEnv(e.payload.environmental_state ?? null);
        }),
      )
      .then((stop) => {
        if (cancelled) stop();
        else stopFn = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      stopFn?.();
    };
  }, [tickSource, channel, captureMode, applyEnv]);

  const scratch = useMemo(
    () => ({
      m: new THREE.Matrix4(),
      p: new THREE.Vector3(),
      q: new THREE.Quaternion(),
      s: new THREE.Vector3(),
      c: new THREE.Color(),
    }),
    [],
  );

  const geoms = useMemo(
    () => ({
      lake: new THREE.CylinderGeometry(1, 1.03, 0.24, 16, 1).translate(0, 0.12, 0),
      tree: makeTreeGeometry(),
      food: new THREE.IcosahedronGeometry(1, 0).translate(0, 1, 0),
    }),
    [],
  );

  const mats = useMemo(
    () => ({
      lake: new THREE.MeshStandardMaterial({
        roughness: 0.22,
        metalness: 0.08,
        polygonOffset: true,
        polygonOffsetFactor: -1,
        polygonOffsetUnits: -1,
      }),
      tree: new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.78, flatShading: true }),
      food: new THREE.MeshStandardMaterial({ roughness: 0.45, flatShading: true }),
    }),
    [],
  );

  useEffect(
    () => () => {
      geoms.lake.dispose();
      geoms.tree.dispose();
      geoms.food.dispose();
      mats.lake.dispose();
      mats.tree.dispose();
      mats.food.dispose();
    },
    [geoms, mats],
  );

  useLayoutEffect(() => {
    if (!hasEntities) return;
    if (rawRef.current) {
      mappedRef.current = mapSimulationElements(
        rawRef.current.elements,
        renderSize,
        sampleGroundY,
      );
    }
    const m = mappedRef.current;
    writeMesh(lakesRef.current, m.lakes, capacity, scratch);
    writeMesh(treesRef.current, m.trees, capacity, scratch);
    writeMesh(foodRef.current, m.food, capacity, scratch);
  }, [hasEntities, capacity, rev, renderSize, sampleGroundY, scratch]);

  if (!hasEntities) return null;

  return (
    <group name="sim-environment">
      <instancedMesh
        name="sim-env-lakes"
        key={`sim-lake-${capacity}`}
        ref={lakesRef}
        args={[geoms.lake, mats.lake, capacity]}
        userData={WATER_LAYER_TAG}
        frustumCulled={false}
        receiveShadow
      />
      <instancedMesh
        name="sim-env-trees"
        key={`sim-tree-${capacity}`}
        ref={treesRef}
        args={[geoms.tree, mats.tree, capacity]}
        userData={FLORA_LAYER_TAG}
        frustumCulled={false}
        castShadow
        receiveShadow
      />
      <instancedMesh
        name="sim-env-food"
        key={`sim-food-${capacity}`}
        ref={foodRef}
        args={[geoms.food, mats.food, capacity]}
        userData={FLORA_LAYER_TAG}
        frustumCulled={false}
        castShadow
        receiveShadow
      />
    </group>
  );
};

export default LiveEnvironment;
