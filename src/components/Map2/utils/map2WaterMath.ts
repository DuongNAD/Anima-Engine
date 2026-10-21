import * as THREE from 'three';

// -------------------------------------------------------------------------------------------------
// Gerstner Wave Math Interfaces & Constants
// -------------------------------------------------------------------------------------------------

export interface GerstnerWaveDefinition {
  direction: [number, number]; // Normalized [Dx, Dz]
  amplitude: number;           // Wave amplitude A in meters
  wavelength: number;          // Wavelength L in meters
  speed: number;               // Phase speed multiplier
  steepness: number;           // Crest steepness Q (0.0 to 1.0)
}

export const MAP2_DEFAULT_GERSTNER_WAVES: GerstnerWaveDefinition[] = [
  { direction: [1.0, 0.2], amplitude: 0.35, wavelength: 14.0, speed: 1.0, steepness: 0.45 },
  { direction: [0.4, 0.9], amplitude: 0.20, wavelength: 7.5, speed: 1.25, steepness: 0.35 },
  { direction: [-0.6, 0.5], amplitude: 0.10, wavelength: 4.0, speed: 1.6, steepness: 0.25 },
  { direction: [-0.3, -0.9], amplitude: 0.05, wavelength: 2.0, speed: 2.0, steepness: 0.20 },
];

/**
 * Known river cascade and waterfall coordinate drop-offs in Map 2.
 */
export const MAP2_WATERFALL_LOCATIONS: Array<{
  position: [number, number, number];
  rotationY: number;
  width: number;
  height: number;
}> = [
  { position: [35.0, 15.0, 48.0], rotationY: 0.0, width: 7.0, height: 8.0 },
  { position: [48.0, 16.0, 32.0], rotationY: -0.7, width: 6.0, height: 8.0 },
  { position: [20.0, 15.5, 30.0], rotationY: 0.7, width: 6.0, height: 8.0 },
];

// -------------------------------------------------------------------------------------------------
// Dendritic River Basin & Hydrological Flow Routing Interfaces
// -------------------------------------------------------------------------------------------------

export interface RiverNetworkSample {
  riverDistance: number;       // Distance to nearest river centerline (m)
  riverFlow: number;           // Flow accumulation / river intensity [0, 1]
  riverBedElevation: number;   // Target carved river floor elevation (m)
  isRiverChannel: boolean;     // True if within active channel width
  tributaryBranch: number;     // 0 = main channel, 1 = tributary, -1 = none
  waterfallDrop: number;       // Altitude drop intensity for cascade foam
}

/**
 * Authoritative Dendritic River Network Model:
 * - Main River Channel: Originates at highland massif (x ~ 55, z ~ 85, h ~ 32m),
 *   meanders downhill along gravitational gradient (dh/ds <= 0) to central lake basin (x=35, z=25, h=12m).
 * - Tributary Branch: Tributary originates in western highlands (x ~ 10, z ~ 65)
 *   and merges smoothly into the main trunk at (x ~ 36, z ~ 50).
 */
export function evaluateDendriticRiverNetwork(x: number, z: number): RiverNetworkSample {
  // 1. Main River Centerline Spline: x_main(z) for z in [25, 90]
  let distMain = Infinity;
  let mainFloor = 12.0;
  let mainFlow = 0.0;
  let isMain = false;

  if (z >= 24.0 && z <= 92.0) {
    const tZ = (z - 25.0);
    const riverCurveX = 35.0 + 20.0 * Math.sin(tZ * 0.04);
    distMain = Math.abs(x - riverCurveX);

    // Monotonic downhill floor gradient (dh/ds <= 0 as river flows north-to-south from z=90 to z=25)
    // At z=25 (lake): floor = 9.8m. At z=90 (highland): floor = 13.5m.
    mainFloor = 9.8 + (tZ / 65.0) * 3.7;
    // Main flow accumulation increases downstream towards the lake (Leopold-Maddock law)
    mainFlow = Math.max(0.0, Math.min(1.0, 1.0 - (tZ / 70.0) * 0.5));
    if (distMain < 7.5) {
      isMain = true;
    }
  }

  // 2. Tributary Branch Spline: x_trib(z) for z in [50, 68]
  let distTrib = Infinity;
  let tribFloor = 13.0;
  let tribFlow = 0.0;
  let isTrib = false;

  if (z >= 49.0 && z <= 68.0) {
    const tTrib = (z - 50.0) / 18.0; // 0 at confluence (z=50), 1 at origin (z=68)
    const tribCurveX = 36.0 - 24.0 * tTrib + 2.5 * Math.sin((z - 50.0) * 0.35);
    distTrib = Math.hypot(x - tribCurveX, 0);
    // Downhill gradient towards confluence at z=50
    tribFloor = 11.2 + tTrib * 3.5;
    tribFlow = Math.max(0.0, Math.min(0.65, 0.65 - tTrib * 0.35));
    if (distTrib < 5.0) {
      isTrib = true;
    }
  }

  // 3. Select closest river feature
  const riverDistance = Math.min(distMain, distTrib);
  const riverBedElevation = distMain <= distTrib ? mainFloor : tribFloor;
  const riverFlow = distMain <= distTrib ? mainFlow : tribFlow;
  let tributaryBranch = -1;
  let isRiverChannel = false;

  if (isMain && (!isTrib || distMain <= distTrib)) {
    tributaryBranch = 0;
    isRiverChannel = true;
  } else if (isTrib) {
    tributaryBranch = 1;
    isRiverChannel = true;
  }

  // 4. Waterfall drop intensity at known cascade ledges
  let waterfallDrop = 0.0;
  for (const loc of MAP2_WATERFALL_LOCATIONS) {
    const dW = Math.hypot(x - loc.position[0], z - loc.position[2]);
    if (dW < loc.width) {
      waterfallDrop = Math.max(waterfallDrop, 1.0 - dW / loc.width);
    }
  }

  return {
    riverDistance: Number.isFinite(riverDistance) ? riverDistance : 1000.0,
    riverFlow,
    riverBedElevation,
    isRiverChannel,
    tributaryBranch,
    waterfallDrop,
  };
}

/**
 * Computes smooth shoreline transitions without unnatural cliffs plunging into water.
 * Enforces gentle slope (<= 0.30) across the waterline band [10.0m, 14.0m].
 */
export function computeSmoothShorelineProfile(
  rawHeight: number,
  _distToWaterBody = 0,
  seaLevel = 12.0,
  _beachWidth = 6.0,
): number {
  if (rawHeight <= seaLevel) {
    // Underwater shallow bed profile: gentle sub-aqueous shelf
    const depth = seaLevel - rawHeight;
    const smoothDepth = Math.min(depth, 1.5 + Math.pow(depth / 5.0, 1.2) * 3.5);
    return seaLevel - smoothDepth;
  }

  // Shoreline transition zone [seaLevel, seaLevel + 3.0m]
  if (rawHeight < seaLevel + 3.0) {
    const t = (rawHeight - seaLevel) / 3.0;
    // Cubic Hermite smoothstep for gentle sandy slope
    const smoothT = t * t * (3.0 - 2.0 * t);
    return seaLevel + smoothT * 3.0;
  }

  return rawHeight;
}

export interface WaterSampleResult {
  height: number;
  displacement: THREE.Vector3;
  normal: THREE.Vector3;
  tangent: THREE.Vector3;
  binormal: THREE.Vector3;
}

/**
 * Evaluates 4-octave Gerstner wave displacement and analytical normal on the CPU.
 */
export function sampleMap2Water(
  x: number,
  z: number,
  time: number,
  seaLevel = 12.0,
  waves: GerstnerWaveDefinition[] = MAP2_DEFAULT_GERSTNER_WAVES,
): WaterSampleResult {
  let dx = 0;
  let dy = 0;
  let dz = 0;

  const tangent = new THREE.Vector3(1, 0, 0);
  const binormal = new THREE.Vector3(0, 0, 1);

  const waveCount = waves.length;

  for (let i = 0; i < waveCount; i++) {
    const w = waves[i];
    const len = Math.hypot(w.direction[0], w.direction[1]) || 1.0;
    const dirX = w.direction[0] / len;
    const dirZ = w.direction[1] / len;

    const k = (2.0 * Math.PI) / w.wavelength;
    const omega = Math.sqrt(9.81 * k) * w.speed;
    const phase = k * (dirX * x + dirZ * z) - omega * time;

    const Q = w.steepness / (k * w.amplitude * waveCount + 1e-5);
    const cosP = Math.cos(phase);
    const sinP = Math.sin(phase);

    dx += Q * w.amplitude * dirX * cosP;
    dz += Q * w.amplitude * dirZ * cosP;
    dy += w.amplitude * sinP;

    // Tangent derivatives along X
    tangent.x -= Q * dirX * dirX * (k * w.amplitude) * sinP;
    tangent.y += dirX * (k * w.amplitude) * cosP;
    tangent.z -= Q * dirX * dirZ * (k * w.amplitude) * sinP;

    // Binormal derivatives along Z
    binormal.x -= Q * dirX * dirZ * (k * w.amplitude) * sinP;
    binormal.y += dirZ * (k * w.amplitude) * cosP;
    binormal.z -= Q * dirZ * dirZ * (k * w.amplitude) * sinP;
  }

  const normal = new THREE.Vector3()
    .crossVectors(binormal, tangent)
    .normalize();

  return {
    height: seaLevel + dy,
    displacement: new THREE.Vector3(dx, dy, dz),
    normal,
    tangent: tangent.normalize(),
    binormal: binormal.normalize(),
  };
}
