import * as THREE from 'three';
import type { EnvironmentalElement } from '../../../types/generated/EnvironmentalElement';
import type { EnvironmentalState } from '../../../types/generated/EnvironmentalState';
import { DEFAULT_XZ_BOUNDS, worldXzToCell } from './coordinate';
import { isInsideSimBounds, simScale, simToRender } from './liveAgentTransform';

// prototype, to be replaced — V1 frontend mapping of `environmental_state` entities until
// ANMW v3 / ADR-0010 lands pinned world layers on both Rust and frontend.

export const LAKE_SURFACE_OFFSET = 0.18;
export const SIM_GRID_RESOLUTION = 256;

export const DECORATION_LEGEND_TEXT =
  'Decoration, not simulated: fish, birds, wildlife, caves';

export const DECORATION_LEGEND_TOOLTIP =
  'WorldFish, WorldBirds, WorldWildlife, and WorldCaves are procedural decoration layers, not simulated by the Rust engine.';

export type SimEntityKind = 'lake' | 'tree' | 'food';
export type SimGroundSampler = (renderX: number, renderZ: number) => number;

export interface MappedSimElement {
  kind: SimEntityKind;
  renderX: number;
  renderY: number;
  renderZ: number;
  groundY: number;
  renderRadius: number;
  heightScale: number;
  resources: number;
}

export interface MappedSimEnvironment {
  lakes: MappedSimElement[];
  trees: MappedSimElement[];
  food: MappedSimElement[];
  skipped: number;
}

export interface SimEnvironmentCounts {
  lakes: number;
  trees: number;
  food: number;
  total: number;
}

export interface SimEnvironmentChannel {
  publish: (env: EnvironmentalState | null, isSynthetic?: boolean) => void;
  subscribe: (listener: (env: EnvironmentalState | null) => void) => () => void;
  latest: () => EnvironmentalState | null;
  isSynthetic: () => boolean;
}

const clamp01 = (v: number): number => (Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0);

/** Normalize an `EnvironmentalElement.type` wire string to a supported simulation entity kind. */
export function normalizeSimEntityKind(rawType: string): SimEntityKind | null {
  const s = rawType.trim().toLowerCase();
  return s === 'lake' || s === 'tree' || s === 'food' ? s : null;
}

/** Map raw `EnvironmentalElement` items into render space (`y` is Bevy Z) grounded on `sampleGroundY`. */
export function mapSimulationElements(
  elements: readonly EnvironmentalElement[],
  renderSize: number,
  sampleGroundY: SimGroundSampler,
  entityScaleRatio = 1,
): MappedSimEnvironment {
  const lakes: MappedSimElement[] = [];
  const trees: MappedSimElement[] = [];
  const food: MappedSimElement[] = [];
  let skipped = 0;

  const scale = simScale(renderSize);
  if (!Array.isArray(elements) || !Number.isFinite(scale) || scale <= 0) {
    return { lakes, trees, food, skipped: Array.isArray(elements) ? elements.length : 0 };
  }
  const safeRatio =
    Number.isFinite(entityScaleRatio) && entityScaleRatio > 0 ? entityScaleRatio : 1;
  const sizeScale = safeRatio === 1 ? scale : scale * safeRatio;

  for (let i = 0; i < elements.length; i++) {
    const el = elements[i];
    const kind = el && typeof el.type === 'string' ? normalizeSimEntityKind(el.type) : null;
    if (
      !kind ||
      !isInsideSimBounds(el.x, el.y) ||
      worldXzToCell(el.x, el.y, SIM_GRID_RESOLUTION, SIM_GRID_RESOLUTION, DEFAULT_XZ_BOUNDS) === null
    ) {
      skipped += 1;
      continue;
    }

    const renderX = simToRender(el.x, renderSize);
    const renderZ = simToRender(el.y, renderSize);
    const rawGround = sampleGroundY(renderX, renderZ);
    const groundY = Number.isFinite(rawGround) ? rawGround : 0;
    const resources = Number.isFinite(el.resources) ? Math.max(0, el.resources) : 0;
    const rawRadius = Number.isFinite(el.radius) && el.radius > 0 ? el.radius : 0;

    if (kind === 'lake') {
      const floor = safeRatio === 1 ? 2.0 : 2.0 * safeRatio;
      const renderRadius = Math.max(floor, (rawRadius || 3.0) * sizeScale);
      lakes.push({
        kind,
        renderX,
        renderY: groundY + LAKE_SURFACE_OFFSET,
        renderZ,
        groundY,
        renderRadius,
        heightScale: 1,
        resources,
      });
    } else if (kind === 'tree') {
      const radFloor = safeRatio === 1 ? 1.5 : 1.5 * safeRatio;
      const heightFloor = safeRatio === 1 ? 1.8 : 1.8 * safeRatio;
      const renderRadius = Math.max(radFloor, (rawRadius || 1.2) * sizeScale);
      trees.push({
        kind,
        renderX,
        renderY: groundY,
        renderZ,
        groundY,
        renderRadius,
        heightScale: Math.max(heightFloor, renderRadius * (0.85 + 0.35 * clamp01(resources / 100))),
        resources,
      });
    } else {
      const floor = safeRatio === 1 ? 0.9 : 0.9 * safeRatio;
      const renderRadius = Math.max(floor, (rawRadius || 0.45) * sizeScale);
      food.push({
        kind,
        renderX,
        renderY: groundY,
        renderZ,
        groundY,
        renderRadius,
        heightScale: renderRadius,
        resources,
      });
    }
  }

  return { lakes, trees, food, skipped };
}

export function countsFromMapped(m: MappedSimEnvironment): SimEnvironmentCounts {
  return {
    lakes: m.lakes.length,
    trees: m.trees.length,
    food: m.food.length,
    total: m.lakes.length + m.trees.length + m.food.length,
  };
}

const LAKE_LOW = new THREE.Color('#4a6b7c');
const LAKE_HIGH = new THREE.Color('#2aa8dc');
const FOOD_LOW = new THREE.Color('#b45309');
const FOOD_HIGH = new THREE.Color('#f59e0b');

/** Deterministic resource-modulated color per simulation entity kind (never biome-tinted). */
export function computeEntityColor(
  kind: SimEntityKind,
  resources: number,
  out: THREE.Color,
): THREE.Color {
  if (kind === 'lake') return out.copy(LAKE_LOW).lerp(LAKE_HIGH, 0.25 + 0.75 * clamp01(resources / 100));
  if (kind === 'tree') {
    const shade = 0.8 + 0.25 * clamp01(resources / 100);
    return out.setRGB(shade, shade, shade);
  }
  return out.copy(FOOD_LOW).lerp(FOOD_HIGH, 0.2 + 0.8 * clamp01(resources / 50));
}

export function createSimEnvironmentChannel(): SimEnvironmentChannel {
  let latestEnv: EnvironmentalState | null = null;
  let latestSynthetic = false;
  const listeners = new Set<(env: EnvironmentalState | null) => void>();
  return {
    publish(env, isSynthetic = false) {
      latestEnv = env;
      latestSynthetic = Boolean(isSynthetic);
      listeners.forEach((fn) => fn(env));
    },
    subscribe(fn) {
      listeners.add(fn);
      return () => {
        listeners.delete(fn);
      };
    },
    latest() {
      return latestEnv;
    },
    isSynthetic() {
      return latestSynthetic;
    },
  };
}
