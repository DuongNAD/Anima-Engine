import React, { memo, useCallback, useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { type World } from '../utils/worldGen';
import { sampleMeshHeight } from '../utils/worldSample';
import type { QualityTier } from '../utils/qualityTiers';
import {
  ELEV_SPAN_M,
  FLORA_SECTORS_PER_SIDE,
  SEA_LEVEL_NORM,
  WORLD_EXTENT_M,
  type WorldV2LoadedBundle,
} from './types';
import {
  BAKE_SPECIES_COUNT,
  LOOK_SHAPE_COUNT,
  LOOK_SPECIES,
  LOOK_SPECIES_SIZE,
  chooseLookShape,
  chooseUnderstoryShape,
  makeLookFloraGeometry,
  type FloraHabitat,
} from './floraLook';
import { C01_RENDER_SIZE, worldRenderScale } from './worldScale';
import {
  ATMOSPHERE_FRAGMENT_PARS,
  atmosphereFragmentApply,
  bindAtmosphereUniforms,
} from '../materials/atmosphere';

export interface WorldV2FloraProps {
  bundle: WorldV2LoadedBundle;
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
  tier?: QualityTier;
}

interface SpeciesBatch {
  speciesId: number;
  geometry: THREE.BufferGeometry;
  /** Instance matrices sorted by grid cell (`cellStart`), 16 floats each. */
  matrices: Float32Array;
  colors: Float32Array;
  count: number;
  grid: FloraGrid;
  /** First instance of each cell; `cellStart[cells]` = `count`. */
  cellStart: Uint32Array;
  /** Lowest / highest instance origin Y per cell (Infinity / -Infinity when empty). */
  cellMinY: Float32Array;
  cellMaxY: Float32Array;
}

// prototype, to be replaced — WV2-LOOK r8 flora distance culling.
//
// The material already shrinks every plant to its origin between 0.7 and 1.0 x `FLORA_FAR_UNITS`
// from the camera, so a plant past that radius draws nothing. It was still submitted: one batch per
// species holds every plant on the map, and on the 81.92 km world that was ~14M triangles of
// degenerate geometry a frame. Plants are now sorted into a coarse grid, and each batch submits
// only the cells that can hold a plant inside the radius AND whose padded box meets a slightly
// widened view frustum, measured from the camera that renders the frame. Plants cast no shadows,
// so a plant outside the view or past the radius draws no pixel: the picture is the same, only
// the triangle count and the vertex work change.

/** Plant fade-out distance: 0.35 of C01's map (~14.3 km), the same in metres at every extent. */
const FLORA_FAR_UNITS = C01_RENDER_SIZE * 0.35;
/** Grid cell size in scene units (~2 km). */
const FLORA_CELL_UNITS = 60;
/** Re-pick the cells once the camera has moved this far; the radius test pads by the same amount. */
const FLORA_REPICK_UNITS = FLORA_CELL_UNITS * 0.5;
/** Re-pick once the view has turned this far; the pick frustum is widened by 4x as much. */
const FLORA_REPICK_DEG = 5;
const FLORA_FOV_MARGIN_DEG = 20;
/** Box padding around a cell's plant origins: a 38 m tree is ~1.1 units tall, ~0.8 wide. */
const FLORA_BOX_PAD = 3;

const _pickProj = new THREE.Matrix4();
const _pickFrustum = new THREE.Frustum();
const _pickBox = new THREE.Box3();
const _viewDir = new THREE.Vector3();

/** The render camera's frustum with the vertical field of view widened by FLORA_FOV_MARGIN_DEG. */
function pickFrustum(camera: THREE.PerspectiveCamera): THREE.Frustum {
  const fov = Math.min(170, camera.fov + FLORA_FOV_MARGIN_DEG);
  const top = (camera.near * Math.tan((fov * Math.PI) / 360)) / camera.zoom;
  const halfW = top * camera.aspect;
  _pickProj.makePerspective(-halfW, halfW, top, -top, camera.near, camera.far);
  _pickProj.multiply(camera.matrixWorldInverse);
  return _pickFrustum.setFromProjectionMatrix(_pickProj);
}

interface FloraGrid {
  side: number;
  cellSize: number;
  renderSize: number;
}

/** Sort `matrices`/`colors` (one plant per 16 / 3 floats) into grid cells by instance origin. */
function sortIntoGrid(
  matrices: Float32Array,
  colors: Float32Array,
  grid: FloraGrid,
): Pick<SpeciesBatch, 'matrices' | 'colors' | 'cellStart' | 'cellMinY' | 'cellMaxY'> {
  const n = matrices.length / 16;
  const cells = grid.side * grid.side;
  const cellOf = new Uint32Array(n);
  const cellStart = new Uint32Array(cells + 1);
  const cellMinY = new Float32Array(cells).fill(Infinity);
  const cellMaxY = new Float32Array(cells).fill(-Infinity);
  const half = grid.renderSize / 2;
  for (let i = 0; i < n; i += 1) {
    const x = matrices[i * 16 + 12];
    const y = matrices[i * 16 + 13];
    const z = matrices[i * 16 + 14];
    const cx = Math.min(grid.side - 1, Math.max(0, Math.floor((x + half) / grid.cellSize)));
    const cz = Math.min(grid.side - 1, Math.max(0, Math.floor((z + half) / grid.cellSize)));
    const c = cz * grid.side + cx;
    cellOf[i] = c;
    cellStart[c + 1] += 1;
    if (y < cellMinY[c]) cellMinY[c] = y;
    if (y > cellMaxY[c]) cellMaxY[c] = y;
  }
  for (let c = 0; c < cells; c += 1) cellStart[c + 1] += cellStart[c];
  const fill = cellStart.slice(0, cells);
  const sortedM = new Float32Array(matrices.length);
  const sortedC = new Float32Array(colors.length);
  for (let i = 0; i < n; i += 1) {
    const d = fill[cellOf[i]]++;
    sortedM.set(matrices.subarray(i * 16, i * 16 + 16), d * 16);
    sortedC.set(colors.subarray(i * 3, i * 3 + 3), d * 3);
  }
  return { matrices: sortedM, colors: sortedC, cellStart, cellMinY, cellMaxY };
}

/**
 * Copy into `mesh`'s instance buffers every cell whose nearest possible plant is within
 * `FLORA_FAR_UNITS + FLORA_REPICK_UNITS` of `cam` and whose padded box meets `frustum`;
 * returns the number of plants written.
 */
function writeNearCells(
  mesh: THREE.InstancedMesh,
  batch: SpeciesBatch,
  cam: THREE.Vector3,
  frustum: THREE.Frustum,
): number {
  const { grid } = batch;
  const half = grid.renderSize / 2;
  const reach = FLORA_FAR_UNITS + FLORA_REPICK_UNITS;
  const reach2 = reach * reach;
  const dstM = mesh.instanceMatrix.array as Float32Array;
  const dstC = mesh.instanceColor ? (mesh.instanceColor.array as Float32Array) : null;
  let n = 0;
  for (let cz = 0; cz < grid.side; cz += 1) {
    const z0 = cz * grid.cellSize - half;
    const dz = Math.max(0, z0 - cam.z, cam.z - (z0 + grid.cellSize));
    if (dz > reach) continue;
    for (let cx = 0; cx < grid.side; cx += 1) {
      const c = cz * grid.side + cx;
      const a = batch.cellStart[c];
      const b = batch.cellStart[c + 1];
      if (a === b) continue;
      const x0 = cx * grid.cellSize - half;
      const dx = Math.max(0, x0 - cam.x, cam.x - (x0 + grid.cellSize));
      const dy = Math.max(0, batch.cellMinY[c] - cam.y, cam.y - batch.cellMaxY[c]);
      if (dx * dx + dy * dy + dz * dz > reach2) continue;
      _pickBox.min.set(x0 - FLORA_BOX_PAD, batch.cellMinY[c] - FLORA_BOX_PAD, z0 - FLORA_BOX_PAD);
      _pickBox.max.set(
        x0 + grid.cellSize + FLORA_BOX_PAD,
        batch.cellMaxY[c] + FLORA_BOX_PAD,
        z0 + grid.cellSize + FLORA_BOX_PAD,
      );
      if (!frustum.intersectsBox(_pickBox)) continue;
      dstM.set(batch.matrices.subarray(a * 16, b * 16), n * 16);
      if (dstC) dstC.set(batch.colors.subarray(a * 3, b * 3), n * 3);
      n += b - a;
    }
  }
  return n;
}

/** Per-mesh culling state: the camera pose of the last pick, and a count waiting for upload. */
interface FloraCullState {
  last: THREE.Vector3 | null;
  lastDir: THREE.Vector3 | null;
  pending: number;
}

/**
 * `onBeforeRender` hook of one species batch. three uploads instance buffers before it draws
 * anything, so a pick written here reaches the GPU on the next frame: the new count is applied
 * then, and until then the frame draws the previous, matching, buffer contents.
 */
function cullFloraBatch(
  mesh: THREE.InstancedMesh,
  batch: SpeciesBatch,
  state: FloraCullState,
  camera: THREE.Camera,
): void {
  const persp = camera as THREE.PerspectiveCamera;
  if (!persp.isPerspectiveCamera) return;
  if (state.pending >= 0) {
    mesh.count = state.pending;
    state.pending = -1;
  }
  const cam = camera.position;
  camera.getWorldDirection(_viewDir);
  if (
    state.last &&
    state.lastDir &&
    state.last.distanceToSquared(cam) < FLORA_REPICK_UNITS * FLORA_REPICK_UNITS &&
    state.lastDir.dot(_viewDir) > Math.cos((FLORA_REPICK_DEG * Math.PI) / 180)
  ) {
    return;
  }
  state.last = (state.last ?? new THREE.Vector3()).copy(cam);
  state.lastDir = (state.lastDir ?? new THREE.Vector3()).copy(_viewDir);
  const n = writeNearCells(mesh, batch, cam, pickFrustum(persp));
  mesh.instanceMatrix.clearUpdateRanges();
  mesh.instanceMatrix.addUpdateRange(0, n * 16);
  mesh.instanceMatrix.needsUpdate = true;
  if (mesh.instanceColor) {
    mesh.instanceColor.clearUpdateRanges();
    mesh.instanceColor.addUpdateRange(0, n * 3);
    mesh.instanceColor.needsUpdate = true;
  }
  state.pending = n;
}

/**
 * One species batch. Memoised with a stable ref callback: the scene re-renders ten times a second
 * while the day clock runs, and the r7 inline ref re-uploaded every plant's matrix (and rebuilt the
 * bounding sphere over all of them) on each of those renders.
 */
const FloraBatchMesh = memo(function FloraBatchMesh({
  batch,
  material,
}: {
  batch: SpeciesBatch;
  material: THREE.Material;
}) {
  const stateRef = useRef<FloraCullState>({ last: null, lastDir: null, pending: -1 });
  const attach = useCallback(
    (mesh: THREE.InstancedMesh | null) => {
      if (!mesh || !mesh.instanceMatrix) return;
      if (!mesh.instanceColor || mesh.instanceColor.count !== batch.count) {
        mesh.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(batch.count * 3), 3);
      }
      // Start with every plant (what the batch drew before r8) until the first pick lands.
      mesh.instanceMatrix.array.set(batch.matrices);
      mesh.instanceMatrix.needsUpdate = true;
      (mesh.instanceColor.array as Float32Array).set(batch.colors);
      mesh.instanceColor.needsUpdate = true;
      mesh.count = batch.count;
      const state = stateRef.current;
      state.last = null;
      state.lastDir = null;
      state.pending = -1;
      mesh.onBeforeRender = (_renderer, _scene, camera) => cullFloraBatch(mesh, batch, state, camera);
    },
    [batch],
  );
  return (
    <instancedMesh
      args={[batch.geometry, material, batch.count]}
      castShadow={false}
      receiveShadow={false}
      frustumCulled={false}
      name={`world-v2-flora-s${batch.speciesId}`}
      ref={attach}
    />
  );
});


// prototype, to be replaced — WV2-LOOK placement rules.
//
// Each baked flora point becomes a small STAND of plants at real-world size (8-38 m trees,
// 3-11 m boulders) instead of one 40-130 m toy tree: the bake's spacing (~70-200 m) is far
// coarser than a real canopy, so a stand fills the gap without inventing new placement data.
// Stand size follows a low-frequency clump noise and the local biome purity, so forest edges
// thin out raggedly instead of stopping on a line; plants on cliff-steep cells are dropped.

/** Stand radius in metres per bake species id (the archetype, not the drawn shape). */
const STAND_RADIUS_M: ReadonlyArray<number> = [34, 30, 30, 18, 14, 30, 22, 16];
/** Plants per stand at `high`; medium/low scale this down. */
const STAND_BASE: ReadonlyArray<number> = [5, 4, 4, 2, 4, 2, 3, 4];

function hash2(x: number, y: number, seed: number): number {
  let h = Math.imul(x | 0, 374761393) ^ Math.imul(y | 0, 668265263) ^ Math.imul(seed, 2246822519);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

function valueNoise(x: number, y: number, seed: number): number {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const fx = x - xi;
  const fy = y - yi;
  const sx = fx * fx * (3 - 2 * fx);
  const sy = fy * fy * (3 - 2 * fy);
  const a = hash2(xi, yi, seed);
  const b = hash2(xi + 1, yi, seed);
  const c = hash2(xi, yi + 1, seed);
  const d = hash2(xi + 1, yi + 1, seed);
  return (a * (1 - sx) + b * sx) * (1 - sy) + (c * (1 - sx) + d * sx) * sy;
}

function seeded(seed: number): () => number {
  let s = seed >>> 0 || 1;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/**
 * Distance to the nearest sea cell, in grid cells, on a G x G grid over the drawn terrain
 * (two-pass chamfer transform, 1 / sqrt(2) weights). Built once per world load.
 */
function seaDistanceGrid(
  world: World,
  meshResolution: number,
  heightUnits: number,
  seaY: number,
  G: number,
): Float32Array {
  const d = new Float32Array(G * G);
  const INF = 1e9;
  for (let y = 0; y < G; y += 1) {
    for (let x = 0; x < G; x += 1) {
      const h = sampleMeshHeight(world, (x + 0.5) / G, (y + 0.5) / G, meshResolution) * heightUnits;
      d[y * G + x] = h < seaY ? 0 : INF;
    }
  }
  const D = Math.SQRT2;
  for (let y = 0; y < G; y += 1) {
    for (let x = 0; x < G; x += 1) {
      const i = y * G + x;
      let v = d[i];
      if (x > 0) v = Math.min(v, d[i - 1] + 1);
      if (y > 0) {
        v = Math.min(v, d[i - G] + 1);
        if (x > 0) v = Math.min(v, d[i - G - 1] + D);
        if (x < G - 1) v = Math.min(v, d[i - G + 1] + D);
      }
      d[i] = v;
    }
  }
  for (let y = G - 1; y >= 0; y -= 1) {
    for (let x = G - 1; x >= 0; x -= 1) {
      const i = y * G + x;
      let v = d[i];
      if (x < G - 1) v = Math.min(v, d[i + 1] + 1);
      if (y < G - 1) {
        v = Math.min(v, d[i + G] + 1);
        if (x < G - 1) v = Math.min(v, d[i + G + 1] + D);
        if (x > 0) v = Math.min(v, d[i + G - 1] + D);
      }
      d[i] = v;
    }
  }
  return d;
}

const SEA_GRID = 512;

function makeFloraLookMaterial(farUnits: number): THREE.MeshStandardMaterial {
  const material = new THREE.MeshStandardMaterial({
    vertexColors: true,
    roughness: 0.92,
    metalness: 0.0,
  });
  const uFloraFar = { value: farUnits };
  material.onBeforeCompile = (shader) => {
    bindAtmosphereUniforms(shader.uniforms);
    shader.uniforms.uFloraFar = uFloraFar;
    // Past ~14 km a plant is under a pixel and the terrain's canopy layer already carries the
    // forest; shrink instances to their origin over the last 30% of the range so the GPU
    // rasterises nothing for them (degenerate triangles), with no visible pop.
    shader.vertexShader =
      'varying vec3 vLkFloraWp;\nuniform float uFloraFar;\n' +
      shader.vertexShader.replace(
        '#include <project_vertex>',
        `
#ifdef USE_INSTANCING
{
vec3 lkOrigin=(modelMatrix*instanceMatrix*vec4(0.0,0.0,0.0,1.0)).xyz;
float lkKeep=1.0-smoothstep(uFloraFar*0.7,uFloraFar,distance(lkOrigin,cameraPosition));
transformed*=lkKeep;
}
#endif
#include <project_vertex>
vec4 lkFloraWp4=vec4(transformed,1.0);
#ifdef USE_INSTANCING
lkFloraWp4=instanceMatrix*lkFloraWp4;
#endif
vLkFloraWp=(modelMatrix*lkFloraWp4).xyz;
`,
      );
    // Rocks in the swash band are wet: darker (CST-04).
    shader.fragmentShader =
      'varying vec3 vLkFloraWp;\n' +
      ATMOSPHERE_FRAGMENT_PARS +
      shader.fragmentShader
        .replace(
          '#include <color_fragment>',
          `
#include <color_fragment>
diffuseColor.rgb*=mix(1.0,0.55,1.0-smoothstep(uAtmoSeaY+0.01,uAtmoSeaY+0.07,vLkFloraWp.y));
diffuseColor.rgb*=mix(0.6,1.0,atmoCloudShadow(vLkFloraWp));
`,
        )
        .replace('#include <fog_fragment>', atmosphereFragmentApply('vLkFloraWp'));
  };
  material.customProgramCacheKey = () => 'wv2-look-flora-v4';
  return material;
}

/**
 * Renders decoded `flora_tiles` (`ANMF v1`, `FLORA_FLAG_DECORATIVE`) as up to 12 `InstancedMesh`
 * batches, one per look shape (<= 12 draw calls total). Strictly decorative — mounted only inside `showDecoration`.
 */
export const WorldV2Flora: React.FC<WorldV2FloraProps> = ({
  bundle,
  world,
  renderSize = 1200,
  heightRatio = 0.14,
  meshResolution = 384,
  tier = 'high',
}) => {
  const heightUnits = renderSize * heightRatio;
  // Low keeps every other stand (was one in four) with lite crowns: a forest, not dots.
  const stride = tier === 'high' ? 1 : 2;
  const standScale = tier === 'high' ? 1 : tier === 'medium' ? 0.8 : 0.65;
  const lite = tier === 'low';
  // WV2-LOOK: at real-world size a tree is under one texel of the 2048 shadow map spread over
  // the whole map, so its cast shadow was aliased noise that cost ~25-35% of the high-tier
  // frame. The canopy layer in the terrain shader carries the dark gaps instead.

  const material = useMemo(() => makeFloraLookMaterial(FLORA_FAR_UNITS), []);
  // Map-edge band skipped below, as a fraction of the map: 0.085 on C01, the same metres elsewhere.
  const edgeSkip = 0.085 * worldRenderScale(bundle.extentM).edgeScale;

  const batches = useMemo<SpeciesBatch[]>(() => {
    const extentM = bundle.extentM || WORLD_EXTENT_M;
    const unitsPerM = renderSize / extentM;
    const unitsPerMY = heightUnits / ELEV_SPAN_M;
    const seaY = SEA_LEVEL_NORM * heightUnits;
    const { size } = bundle;
    const perShape: number[][] = Array.from({ length: LOOK_SHAPE_COUNT }, () => []);
    const perShapeColor: number[][] = Array.from({ length: LOOK_SHAPE_COUNT }, () => []);
    const rnd = seeded(bundle.seed ^ 0x5eed);
    const dummy = new THREE.Object3D();
    const tint = new THREE.Color();
    // r9: habitat of every plant, measured on the drawn terrain (height above sea, distance to
    // the sea, slope, biome), so a bake archetype becomes a plant that fits where it stands.
    const seaDist = seaDistanceGrid(world, meshResolution, heightUnits, seaY, SEA_GRID);
    const mPerSeaCell = extentM / SEA_GRID;
    const env: FloraHabitat = { elevM: 0, distSeaM: 0, slope: 0, biome: 0 };

    let idx = 0;
    for (const sector of bundle.floraSectors) {
      for (const inst of sector.instances) {
        if (stride > 1 && idx % stride !== 0) {
          idx += 1;
          continue;
        }
        idx += 1;
        const bakeId = inst.species_id < BAKE_SPECIES_COUNT ? inst.species_id : 3;
        const u = (sector.sectorX + inst.x_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
        const v = (sector.sectorZ + inst.z_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
        // The terrain's map-edge falloff band (see terrainPbrMaterial) is under water.
        if (Math.min(u, 1 - u, v, 1 - v) < edgeSkip) continue;
        const cx = Math.min(size - 1, Math.max(0, Math.floor(u * size)));
        const cy = Math.min(size - 1, Math.max(0, Math.floor(v * size)));
        const ci = cy * size + cx;

        const nx = (bundle.normalRg[ci * 2] / 255) * 2 - 1;
        const nz = (bundle.normalRg[ci * 2 + 1] / 255) * 2 - 1;
        const ny = Math.sqrt(Math.max(0, 1 - nx * nx - nz * nz));
        const slope = 1 - ny;

        // Biome purity: 1 deep inside a biome, lower on an ecotone.
        const purity = (bundle.biomeWeightsRgba[ci * 4] ?? 255) / 255;
        env.biome = bundle.biomeIdsRgba[ci * 4] ?? 0;
        env.slope = slope;
        const wx0 = (u - 0.5) * renderSize;
        const wz0 = (v - 0.5) * renderSize;
        const clump =
          valueNoise(wx0 * 0.045, wz0 * 0.045, 17) * 0.65 + valueNoise(wx0 * 0.16, wz0 * 0.16, 23) * 0.35;
        // Steep ground thins the stand but no longer removes it: chooseLookShape turns cliff
        // members into talus boulders or nothing.
        const keep = clump * 0.9 + purity * 0.55 - Math.min(slope, 0.3) * 1.2;
        if (keep < 0.42) continue;
        const density = Math.min(1.25, Math.max(0.25, (keep - 0.42) * 2.2));
        const k = Math.max(1, Math.round(STAND_BASE[bakeId] * standScale * density + rnd() * 0.8));
        // Forest stands get one understory plant (tree fern or shrub) at high and medium.
        const understory = !lite && rnd() < 0.6 ? 1 : 0;

        const scale01 = inst.scale_u8 / 255;
        const radius = STAND_RADIUS_M[bakeId] * unitsPerM;
        const yaw0 = (inst.yaw_u8 / 255) * Math.PI * 2;
        // Clumped, not evenly spread: members gather around one or two sub-centres inside the
        // stand, leaving gaps (natural regeneration patches), and the whole stand is nudged.
        const subN = k > 2 && rnd() < 0.6 ? 2 : 1;
        const subs: Array<[number, number]> = [];
        for (let q = 0; q < subN; q += 1) {
          const sa = rnd() * Math.PI * 2;
          const sr = Math.sqrt(rnd()) * radius * 0.75;
          subs.push([wx0 + Math.cos(sa) * sr, wz0 + Math.sin(sa) * sr]);
        }
        for (let m = 0; m < k + understory; m += 1) {
          const a = rnd() * Math.PI * 2;
          const sc = subs[m % subN];
          const r = Math.pow(rnd(), 0.8) * radius * 0.55;
          const px = sc[0] + Math.cos(a) * r;
          const pz = sc[1] + Math.sin(a) * r;
          const pu = px / renderSize + 0.5;
          const pv = pz / renderSize + 0.5;
          if (pu < 0 || pu > 1 || pv < 0 || pv > 1) continue;
          const groundY = sampleMeshHeight(world, pu, pv, meshResolution) * heightUnits;
          env.elevM = (groundY - seaY) / unitsPerMY;
          const gx = Math.min(SEA_GRID - 1, Math.floor(pu * SEA_GRID));
          const gy = Math.min(SEA_GRID - 1, Math.floor(pv * SEA_GRID));
          env.distSeaM = seaDist[gy * SEA_GRID + gx] * mPerSeaCell;
          const pick = rnd();
          const shape = m < k ? chooseLookShape(bakeId, env, pick) : chooseUnderstoryShape(bakeId, env, pick);
          if (shape < 0) continue;
          const isRock = shape === LOOK_SPECIES.Rock;
          // Nothing grows under the sea; mangroves stand in the intertidal zone, boulders may sit
          // in the swash, half buried.
          const floorY = isRock
            ? seaY - 2 * unitsPerMY
            : shape === LOOK_SPECIES.Mangrove
              ? seaY - 1.5 * unitsPerMY
              : seaY + 0.6 * unitsPerMY;
          if (groundY < floorY) continue;
          const sizeSpec = LOOK_SPECIES_SIZE[shape];
          const under = m >= k ? 0.55 : 1;
          const hM =
            (sizeSpec.minM +
              (sizeSpec.maxM - sizeSpec.minM) * Math.min(1, Math.max(0, scale01 * 0.6 + rnd() * 0.55 - 0.1))) *
            under;
          const h = hM * unitsPerM;
          const wJ = 0.72 + rnd() * 0.6;
          const py = groundY - h * (isRock ? 0.3 : 0.03);
          dummy.position.set(px, py, pz);
          dummy.rotation.set((rnd() - 0.5) * 0.08, yaw0 + rnd() * Math.PI * 2, (rnd() - 0.5) * 0.08);
          dummy.scale.set(h * wJ, h * (0.88 + rnd() * 0.3), h * (0.72 + rnd() * 0.6));
          dummy.updateMatrix();
          const e = dummy.matrix.elements;
          const list = perShape[shape];
          for (let j = 0; j < 16; j += 1) list.push(e[j]);

          // Per-plant tint: value +-18%, drift between dry-warm and lush-cool.
          const val = 0.78 + rnd() * 0.4;
          const warm = rnd();
          if (isRock) {
            tint.setRGB(val, val * 0.98, val * (0.92 + warm * 0.1));
          } else if (
            (shape === LOOK_SPECIES.Jungle || shape === LOOK_SPECIES.Broadleaf) &&
            bakeId === 0 &&
            rnd() < 0.035
          ) {
            // A few flowering or flushing crowns in the rainforest canopy (red-orange, yellow).
            if (warm < 0.5) tint.setRGB(2.1, 1.05, 0.62);
            else tint.setRGB(1.95, 1.75, 0.6);
          } else {
            tint.setRGB(val * (0.9 + warm * 0.28), val * (0.98 + warm * 0.06), val * (1.06 - warm * 0.3));
          }
          perShapeColor[shape].push(tint.r, tint.g, tint.b);
        }
      }
    }

    const grid: FloraGrid = {
      side: Math.max(1, Math.ceil(renderSize / FLORA_CELL_UNITS)),
      cellSize: 0,
      renderSize,
    };
    grid.cellSize = renderSize / grid.side;
    const out: SpeciesBatch[] = [];
    for (let shape = 0; shape < LOOK_SHAPE_COUNT; shape += 1) {
      const list = perShape[shape];
      if (list.length === 0) continue;
      out.push({
        speciesId: shape,
        geometry: makeLookFloraGeometry(shape, lite),
        count: list.length / 16,
        grid,
        ...sortIntoGrid(new Float32Array(list), new Float32Array(perShapeColor[shape]), grid),
      });
    }
    return out;
  }, [bundle, world, renderSize, heightUnits, meshResolution, stride, standScale, lite, edgeSkip]);

  useEffect(() => {
    return () => {
      for (const b of batches) b.geometry.dispose();
    };
  }, [batches]);

  useEffect(() => {
    return () => {
      material.dispose();
    };
  }, [material]);

  if (batches.length === 0) return null;

  return (
    <group name="world-v2-flora" userData={{ decorative: true, rwLayer: 'flora' }}>
      {batches.map((batch) => (
        <FloraBatchMesh key={batch.speciesId} batch={batch} material={material} />
      ))}
    </group>
  );
};

export default WorldV2Flora;
