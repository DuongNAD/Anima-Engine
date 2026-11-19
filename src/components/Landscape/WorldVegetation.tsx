import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { FloraType } from './utils/worldGen';
import { makeFloraGeometry } from './utils/floraGeometry';
import { loadFloraAssetGeometries, type FloraGeomEntry } from './assets/worldAssets';
import { createFloraWindMaterial } from './materials/floraWindMaterial';
import { VEGETATION_BASE_SIZE } from './utils/floraClearance';
import { sampleMeshHeight } from './utils/worldSample';
import { debugFloraDensityScale, installFrameCounters, type FrameCountersHandle } from './utils/diagnosticsWindow';
import {
  CASTS_SHADOW,
  buildFloraSources,
  floraTint,
  hash01,
  makeImpostorGeometry,
  type FloraSources,
} from './utils/floraChunks';
import { isCaptureRequested } from './utils/captureMode';
import { getQualityTierConfig, type QualityTier } from './utils/qualityTiers';
import { sceneElapsed } from './utils/sceneClock';
import {
  CELL_FAR,
  CELL_MID,
  CELL_NEAR,
  FLORA_CELLS_PER_SIDE,
  frustumPlanesFromViewProjection,
  multiplyMat4,
  selectFloraCells,
  collectCellRanges,
} from './utils/chunkLod';

export interface WorldVegetationProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  /** Mesh resolution the terrain is rendered at — flora snaps to THAT surface, not the data. */
  meshResolution?: number;
  /** Base size (world units) of a flora instance before its per-instance scale. */
  baseSize?: number;
  /** 'low' keeps every other instance and skips the shadow pass (light-GPU mode). Legacy path. */
  quality?: 'high' | 'low';
  /**
   * Quality tier (RW4-1 table). When given, flora is chunked by distance with impostors and the
   * density follows the tier (`vegetationDensity`). Absent, or in capture mode, the legacy
   * all-instances path runs unchanged.
   */
  tier?: QualityTier;
}

const TYPES: FloraType[] = [
  FloraType.Pine,
  FloraType.Round,
  FloraType.Jungle,
  FloraType.Cactus,
  FloraType.Rock,
  FloraType.Acacia,
  FloraType.Palm,
  FloraType.DeadTree,
  FloraType.Bush,
  FloraType.Reed,
  FloraType.Tuft,
  FloraType.Coral,
  FloraType.Kelp,
  FloraType.Seagrass,
];

const TypedInstances: React.FC<{
  world: World;
  type: FloraType;
  geometry: THREE.BufferGeometry;
  renderSize: number;
  heightRatio: number;
  meshResolution: number;
  baseSize: number;
  lowQuality: boolean;
}> = ({ world, type, geometry, renderSize, heightRatio, meshResolution, baseSize, lowQuality }) => {
  const ref = useRef<THREE.InstancedMesh>(null);

  // Indices of flora of this type (low quality keeps every other instance).
  const indices = useMemo(() => {
    const out: number[] = [];
    let seen = 0;
    for (let i = 0; i < world.floraCount; i++) {
      if (world.floraType[i] !== type) continue;
      if (!lowQuality || seen % 2 === 0) out.push(i);
      seen++;
    }
    return out;
  }, [world, type, lowQuality]);

  // Vertical offset so the geometry's lowest point sits exactly on the ground (unit scale).
  const groundLift = useMemo(() => {
    geometry.computeBoundingBox();
    return geometry.boundingBox ? -geometry.boundingBox.min.y : 0;
  }, [geometry]);

  useLayoutEffect(() => {
    const inst = ref.current;
    if (!inst) return;
    const dummy = new THREE.Object3D();
    const tint = new THREE.Color();
    const { size } = world;
    const heightUnits = renderSize * heightRatio;
    for (let k = 0; k < indices.length; k++) {
      const i = indices[k];
      const u = (world.floraX[i] + size / 2) / size;
      const v = (world.floraZ[i] + size / 2) / size;
      const x = (u - 0.5) * renderSize;
      const z = (v - 0.5) * renderSize;
      // Snap to the RENDER MESH surface (coarser than the data) so nothing floats/sinks.
      const y = sampleMeshHeight(world, u, v, meshResolution) * heightUnits;
      const s = world.floraScale[i] * baseSize;
      dummy.position.set(x, y + groundLift * s, z);
      dummy.scale.set(s, s, s);
      dummy.rotation.set(0, (world.floraX[i] * 0.7 + world.floraZ[i] * 0.3) % (Math.PI * 2), 0);
      dummy.updateMatrix();
      if (typeof inst.setMatrixAt === 'function') inst.setMatrixAt(k, dummy.matrix);
      // Subtle per-instance tint (vertex colours carry the two-tone; this multiplies on top)
      // so a forest reads as thousands of slightly different trees, not one repeated prop.
      if (typeof inst.setColorAt === 'function') {
        const h = hash01(world.floraX[i], world.floraZ[i]);
        const h2 = hash01(world.floraZ[i], world.floraX[i]);
        floraTint(type, h, h2, tint);
        inst.setColorAt(k, tint);
      }
    }
    if (inst.instanceMatrix) inst.instanceMatrix.needsUpdate = true;
    if (inst.instanceColor) inst.instanceColor.needsUpdate = true;
    // `type` chooses the tint palette a few lines up, so it belongs here: leaving it out meant a
    // meadow re-rendered as coral kept its greens until something else invalidated the effect.
  }, [indices, world, renderSize, heightRatio, meshResolution, baseSize, groundLift, type]);

  if (indices.length === 0) return null;
  return (
    <instancedMesh
      ref={ref}
      args={[geometry, null, indices.length]}
      name={`flora-${type}`}
      castShadow={!lowQuality && CASTS_SHADOW.has(type)}
      frustumCulled={false}
    >
      <meshStandardMaterial vertexColors roughness={0.9} metalness={0} flatShading />
    </instancedMesh>
  );
};

/** Make sure an instanced mesh owns a colour attribute and dynamic buffers sized to its capacity. */
function prepareInstanceBuffers(mesh: THREE.InstancedMesh, capacity: number): void {
  if (!mesh.instanceMatrix) return;
  mesh.instanceMatrix.setUsage?.(THREE.DynamicDrawUsage);
  if (!mesh.instanceColor && typeof THREE.InstancedBufferAttribute === 'function') {
    mesh.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(capacity * 3), 3);
    mesh.instanceColor.setUsage?.(THREE.DynamicDrawUsage);
  }
}

/** Copy `[start, end)` ranges of the source buffers into the mesh; returns the instance count. */
function uploadRanges(
  mesh: THREE.InstancedMesh,
  matrices: Float32Array,
  colors: Float32Array,
  ranges: readonly number[],
): number {
  const mArr = mesh.instanceMatrix?.array as Float32Array | undefined;
  const cArr = mesh.instanceColor?.array as Float32Array | undefined;
  let dst = 0;
  const cap = mArr ? mArr.length / 16 : 0;
  for (let r = 0; r < ranges.length; r += 2) {
    const start = ranges[r];
    const n = Math.min(ranges[r + 1] - start, cap - dst);
    if (n <= 0) break;
    mArr?.set(matrices.subarray(start * 16, (start + n) * 16), dst * 16);
    cArr?.set(colors.subarray(start * 3, (start + n) * 3), dst * 3);
    dst += n;
  }
  mesh.count = dst;
  mesh.visible = dst > 0;
  if (mesh.instanceMatrix) {
    mesh.instanceMatrix.clearUpdateRanges?.();
    if (dst > 0) mesh.instanceMatrix.addUpdateRange?.(0, dst * 16);
    mesh.instanceMatrix.needsUpdate = true;
  }
  if (mesh.instanceColor) {
    mesh.instanceColor.clearUpdateRanges?.();
    if (dst > 0) mesh.instanceColor.addUpdateRange?.(0, dst * 3);
    mesh.instanceColor.needsUpdate = true;
  }
  return dst;
}

/** Re-select only after the camera moved this far (world units) or turned past `TURN_COS`. */
const MOVE_EPS = 5;
const TURN_COS = Math.cos((3 * Math.PI) / 180);
/** Frustum slack covering motion between selections (~3° at 500 units plus `MOVE_EPS`). */
const FRUSTUM_PAD = 40;

const ChunkedFlora: React.FC<{
  world: World;
  geoms: ReadonlyArray<FloraGeomEntry>;
  usingCc0Assets: boolean;
  renderSize: number;
  heightRatio: number;
  meshResolution: number;
  baseSize: number;
  tier: QualityTier;
}> = ({
  world,
  geoms,
  usingCc0Assets,
  renderSize,
  heightRatio,
  meshResolution,
  baseSize,
  tier,
}) => {
  const cfg = getQualityTierConfig(tier);
  // Test-only (RW-C4 negative control); `1` in every production build.
  const densityScale = useMemo(() => debugFloraDensityScale(), []);
  const sources = useMemo(
    () =>
      buildFloraSources(world, geoms, {
        renderSize,
        heightRatio,
        meshResolution,
        baseSize,
        density: cfg.vegetationDensity * densityScale,
      }),
    [world, geoms, renderSize, heightRatio, meshResolution, baseSize, cfg.vegetationDensity, densityScale],
  );
  const impostorGeometry = useMemo(() => makeImpostorGeometry(), []);
  useEffect(() => () => impostorGeometry.dispose(), [impostorGeometry]);

  const animatedBundle = useMemo(
    () =>
      createFloraWindMaterial({
        enableWind: tier !== 'low',
        smoothShading: usingCc0Assets,
      }),
    [tier, usingCc0Assets],
  );
  const staticBundle = useMemo(
    () =>
      createFloraWindMaterial({
        enableWind: false,
        smoothShading: usingCc0Assets,
      }),
    [usingCc0Assets],
  );
  useEffect(() => {
    return () => {
      animatedBundle.dispose();
      staticBundle.dispose();
    };
  }, [animatedBundle, staticBundle]);

  const meshes = useRef<Array<THREE.InstancedMesh | null>>([]);
  const impostorMesh = useRef<THREE.InstancedMesh | null>(null);
  const last = useRef<{
    classes: Uint8Array | null;
    pos: [number, number, number];
    fwd: [number, number, number];
    sources: FloraSources | null;
    tier: QualityTier | null;
  }>({ classes: null, pos: [0, 0, 0], fwd: [0, 0, -1], sources: null, tier: null });
  const scratch = useRef<{ classes: Uint8Array; ranges: number[] }>({
    classes: new Uint8Array(FLORA_CELLS_PER_SIDE * FLORA_CELLS_PER_SIDE),
    ranges: [],
  });

  useFrame((state) => {
    animatedBundle.updateTime(sceneElapsed(state.clock));
    const cam = state.camera as THREE.Camera | undefined;
    const proj = cam?.projectionMatrix?.elements;
    const view = cam?.matrixWorldInverse?.elements;
    if (!cam || !proj || !view) return;
    const l = last.current;
    const px = cam.position.x;
    const py = cam.position.y;
    const pz = cam.position.z;
    const fx = -view[2];
    const fy = -view[6];
    const fz = -view[10];
    const fresh = l.sources !== sources || l.tier !== tier || l.classes === null;
    if (!fresh) {
      const moved = Math.hypot(px - l.pos[0], py - l.pos[1], pz - l.pos[2]);
      const turn = fx * l.fwd[0] + fy * l.fwd[1] + fz * l.fwd[2];
      if (moved < MOVE_EPS && turn > TURN_COS) return;
    }
    const planes = frustumPlanesFromViewProjection(multiplyMat4(proj, view));
    const classes = selectFloraCells(
      {
        cellsPerSide: FLORA_CELLS_PER_SIDE,
        renderSize,
        cellYMin: sources.cellYMin,
        cellYMax: sources.cellYMax,
        camX: px,
        camY: py,
        camZ: pz,
        planes,
        nearRadius: cfg.floraNearRadius,
        midRadius: cfg.floraMidRadius,
        frustumPad: FRUSTUM_PAD,
      },
      scratch.current.classes,
    );
    l.pos = [px, py, pz];
    l.fwd = [fx, fy, fz];
    const prev = l.classes;
    let same = !fresh && prev !== null;
    if (same && prev) {
      for (let k = 0; k < classes.length; k++) {
        if (prev[k] !== classes[k]) {
          same = false;
          break;
        }
      }
    }
    if (same) return;
    l.classes = Uint8Array.from(classes);
    l.sources = sources;
    l.tier = tier;

    const ranges = scratch.current.ranges;
    sources.types.forEach((src, k) => {
      const mesh = meshes.current[k];
      if (!mesh) return;
      collectCellRanges(src.cellStart, classes, (c) => c === CELL_NEAR, ranges);
      uploadRanges(mesh, src.matrices, src.colors, ranges);
    });
    const imp = impostorMesh.current;
    if (imp) {
      // MID cells: trees + shrubs (slots 2c, 2c+1); FAR cells: trees only (slot 2c).
      const ss = sources.impostor.slotStart;
      const flat: number[] = [];
      for (let c = 0; c < classes.length; c++) {
        const cl = classes[c];
        if (cl !== CELL_MID && cl !== CELL_FAR) continue;
        const s = ss[2 * c];
        const e = cl === CELL_MID ? ss[2 * c + 2] : ss[2 * c + 1];
        if (e <= s) continue;
        if (flat.length && flat[flat.length - 1] === s) flat[flat.length - 1] = e;
        else flat.push(s, e);
      }
      uploadRanges(imp, sources.impostor.matrices, sources.impostor.colors, flat);
    }
  });

  const shadows = cfg.shadows;
  const geomTag = usingCc0Assets ? 'cc0' : 'proc';
  return (
    <>
      {sources.types.map((src, k) => {
        if (src.count === 0) return null;
        const mat =
          src.type === FloraType.Rock || src.type === FloraType.Coral
            ? staticBundle.material
            : animatedBundle.material;
        return (
          <instancedMesh
            key={`${src.type}-${src.count}-${geomTag}`}
            ref={(m: THREE.InstancedMesh | null) => {
              meshes.current[k] = m;
              if (m && m.instanceColor === null) {
                prepareInstanceBuffers(m, src.count);
                m.count = 0;
                m.visible = false;
              }
            }}
            args={[geoms[k].geometry, mat, src.count]}
            name={`flora-${src.type}`}
            castShadow={shadows && CASTS_SHADOW.has(src.type)}
            frustumCulled={false}
          />
        );
      })}
      {sources.impostor.count > 0 ? (
        <instancedMesh
          key={`impostor-${sources.impostor.count}-${geomTag}`}
          ref={(m: THREE.InstancedMesh | null) => {
            impostorMesh.current = m;
            if (m && m.instanceColor === null) {
              prepareInstanceBuffers(m, sources.impostor.count);
              m.count = 0;
              m.visible = false;
            }
          }}
          args={[impostorGeometry, null, sources.impostor.count]}
          name="flora-impostor"
          castShadow={false}
          frustumCulled={false}
        >
          <meshStandardMaterial vertexColors roughness={0.95} metalness={0} flatShading />
        </instancedMesh>
      ) : null}
    </>
  );
};

const FrameDiagnostics: React.FC<{ renderSize: number; heightRatio: number }> = ({
  renderSize,
  heightRatio,
}) => {
  const { gl, scene } = useThree();
  const handleRef = useRef<FrameCountersHandle | null>(null);

  useLayoutEffect(() => {
    const handle = installFrameCounters(gl, scene, { renderSize, heightRatio });
    handleRef.current = handle;
    return () => {
      handleRef.current = null;
      handle.dispose();
    };
  }, [gl, scene, renderSize, heightRatio]);

  useFrame((state) => {
    handleRef.current?.applyPinnedPose(state.camera);
  });

  return null;
};

/** Renders the world's flora (SoA) as instanced low-poly meshes, one InstancedMesh per type. */
export const WorldVegetation: React.FC<WorldVegetationProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 256,
  baseSize = VEGETATION_BASE_SIZE,
  quality = 'high',
  tier,
}) => {
  const fallbackGeoms = useMemo(
    () => TYPES.map((type) => ({ type, geometry: makeFloraGeometry(type) })),
    [],
  );

  useEffect(() => {
    return () => {
      fallbackGeoms.forEach((d) => d.geometry.dispose());
    };
  }, [fallbackGeoms]);

  // Capture mode (`?capture=1`) keeps the full legacy drawing path so the canonical views do not
  // depend on camera-driven selection (RW4-4); so does a caller that passes no `tier`.
  const captureMode = typeof window !== 'undefined' && isCaptureRequested(window.location.search);
  const chunked = tier !== undefined && !captureMode;
  const [cc0State, setCc0State] = useState<{ tier: QualityTier; geoms: FloraGeomEntry[] } | null>(
    null,
  );

  useEffect(() => {
    if (!chunked || tier === undefined || tier === 'low') return;
    let cancelled = false;
    void loadFloraAssetGeometries(TYPES, tier).then((loaded) => {
      if (!cancelled && loaded && loaded.length === TYPES.length) {
        setCc0State({ tier, geoms: loaded });
      }
    });
    return () => {
      cancelled = true;
    };
  }, [chunked, tier]);

  const usingCc0Assets = Boolean(
    chunked && tier !== undefined && tier !== 'low' && cc0State && cc0State.tier === tier,
  );
  const activeGeoms = usingCc0Assets && cc0State ? cc0State.geoms : fallbackGeoms;

  return (
    <group name="world-vegetation">
      <FrameDiagnostics renderSize={renderSize} heightRatio={heightRatio} />
      {chunked ? (
        <ChunkedFlora
          world={world}
          geoms={activeGeoms}
          usingCc0Assets={usingCc0Assets}
          renderSize={renderSize}
          heightRatio={heightRatio}
          meshResolution={meshResolution}
          baseSize={baseSize}
          tier={tier}
        />
      ) : (
        fallbackGeoms.map((d) => (
          <TypedInstances
            key={d.type}
            world={world}
            type={d.type}
            geometry={d.geometry}
            renderSize={renderSize}
            heightRatio={heightRatio}
            meshResolution={meshResolution}
            baseSize={baseSize}
            lowQuality={quality === 'low'}
          />
        ))
      )}
    </group>
  );
};

export default WorldVegetation;
