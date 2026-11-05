import * as THREE from 'three';

// -------------------------------------------------------------------------------------------------
// Celestial & Atmospheric Mathematical Interfaces & Types
// -------------------------------------------------------------------------------------------------

export interface CelestialState {
  timeOfDay: number;
  sunPosition: THREE.Vector3;
  sunDirection: THREE.Vector3;
  moonPosition: THREE.Vector3;
  moonDirection: THREE.Vector3;
  sunElevation: number; // in radians
  sunAzimuth: number;   // in radians
  sunColor: THREE.Color;
  sunIntensity: number;
  moonColor: THREE.Color;
  moonIntensity: number;
  skyZenithColor: THREE.Color;
  skyHorizonColor: THREE.Color;
  skyGroundColor: THREE.Color;
  fogColor: THREE.Color;
  fogDensity: number;
  ambientColor: THREE.Color;
  ambientIntensity: number;
  hemiSkyColor: THREE.Color;
  hemiGroundColor: THREE.Color;
  hemiIntensity: number;
  starOpacity: number;
  isDay: boolean;
  windSpeed: number;
  windDirection: THREE.Vector2;
}

// -------------------------------------------------------------------------------------------------
// Scientific Atmospheric Climate & Orographic Physics Interfaces
// -------------------------------------------------------------------------------------------------

export interface AtmosphericClimateSample {
  temperature: number;        // Normalized [0, 1] (0 = freezing frost/snowline, 1 = scorching heat)
  moisture: number;           // Normalized [0, 1] (0 = hyper-arid desert, 1 = hyper-humid rainforest)
  lapseRateTemp: number;      // Temperature drop due to elevation lapse rate (°C or normalized)
  orographicMoisture: number; // Moisture gain from windward condensation [0, 1]
  rainShadowDepletion: number;// Moisture loss from leeward adiabatic warming [0, 1]
  windwardFactor: number;     // Dot product (Wind . -Normal_xz) in [-1, 1]
  isSnowLine: boolean;        // True if altitude exceeds snowline or freezing temp
}

/**
 * Computes International Standard Atmosphere (ISA) elevation temperature lapse rate.
 * ISA Standard: -6.5°C per 1000m (-0.0065°C/m).
 * Mapped to procedural terrain world scale where maxElevation (90m) corresponds to alpine peak altitude (~4000m).
 */
export function computeAtmosphereLapseRate(
  height: number,
  seaLevel = 12.0,
  maxElevation = 90.0,
  baseTemperature = 0.65,
): number {
  // Bounded height
  const clampedHeight = Math.max(0, Math.min(maxElevation, height));
  // Normalized elevation above sea level
  const relElevation = Math.max(0, clampedHeight - seaLevel) / (maxElevation - seaLevel);
  // ISA lapse rate factor: -0.75 normalized drop across full mountain height range
  const lapseDrop = relElevation * 0.72;
  const temp = baseTemperature - lapseDrop;
  return Math.max(0.0, Math.min(1.0, temp));
}

/**
 * Computes 2D orographic precipitation and adiabatic rain shadow dynamics.
 * Wind carrying oceanic moisture encounters topographic barrier:
 * - Windward slope (Wind . -Normal_xz > 0): Air ascends, expands adiabatically, cools and condenses moisture.
 * - Leeward slope (Wind . -Normal_xz < 0): Air descends, compresses adiabatically, warms and depletes moisture (Rain Shadow).
 */
export function computeOrographicRainShadow(
  normal: THREE.Vector3,
  windDirection: THREE.Vector2,
  height: number,
  worldX: number,
  baseMoisture = 0.52,
): {
  moisture: number;
  orographicCondensation: number;
  rainShadowDepletion: number;
  windwardFactor: number;
  adiabaticHeating: number;
} {
  const windDirNorm = windDirection.clone().normalize();
  // Surface horizontal gradient direction (opposite to normal_xz)
  // When normal points against wind (n.x * w.x < 0), slope is facing into the wind (windward)
  const slopeAlignment = -(normal.x * windDirNorm.x + normal.z * windDirNorm.y);

  // Windward condensation: proportional to positive slope alignment and elevation
  const orographicCondensation = slopeAlignment > 0.0
    ? slopeAlignment * 0.35 * Math.min(1.0, height / 35.0)
    : 0.0;

  // Leeward rain shadow moisture depletion & adiabatic heating
  const rainShadowDepletion = slopeAlignment < 0.0
    ? Math.abs(slopeAlignment) * 0.40 * Math.min(1.0, height / 40.0)
    : 0.0;

  // Regional macro advection (Eastward rain shadow as wind moves west-to-east across continental divide)
  const macroAdvection = -(worldX / 200.0) * 0.28;

  const adiabaticHeating = rainShadowDepletion * 0.15;

  let finalMoisture = baseMoisture + orographicCondensation - rainShadowDepletion + macroAdvection;
  finalMoisture = Math.max(0.0, Math.min(1.0, finalMoisture));

  return {
    moisture: finalMoisture,
    orographicCondensation,
    rainShadowDepletion,
    windwardFactor: slopeAlignment,
    adiabaticHeating,
  };
}

/**
 * Computes integrated atmospheric microclimate for any terrain sample.
 */
export function computeAtmosphericMicroclimate(
  height: number,
  normal: THREE.Vector3,
  worldX: number,
  _worldZ: number,
  isWater: boolean,
  windDir = new THREE.Vector2(1.0, 0.2),
  microNoise = 0.0,
): AtmosphericClimateSample {
  const lapseTemp = computeAtmosphereLapseRate(height, 12.0, 90.0, 0.62);
  const oro = computeOrographicRainShadow(normal, windDir, height, worldX, 0.52);

  // Water body humidity bonus
  const waterBonus = isWater ? 0.45 : (height < 14.5 ? 0.20 : 0.0);
  const finalMoisture = Math.max(0.0, Math.min(1.0, oro.moisture + waterBonus + microNoise * 0.15));

  // Latitude temperature variation
  const latFactor = (worldX / 200.0) * 0.15;
  const finalTemp = Math.max(0.0, Math.min(1.0, lapseTemp + latFactor + oro.adiabaticHeating + microNoise * 0.10));

  const isSnowLine = height >= 48.0 || (height > 38.0 && finalTemp < 0.22);

  return {
    temperature: finalTemp,
    moisture: finalMoisture,
    lapseRateTemp: lapseTemp,
    orographicMoisture: oro.orographicCondensation,
    rainShadowDepletion: oro.rainShadowDepletion,
    windwardFactor: oro.windwardFactor,
    isSnowLine,
  };
}

interface KeyframePoint {
  hour: number;
  sunColor: string;
  sunIntensity: number;
  moonIntensity: number;
  zenithColor: string;
  horizonColor: string;
  groundColor: string;
  fogColor: string;
  fogDensity: number;
  ambientColor: string;
  ambientIntensity: number;
  hemiSkyColor: string;
  hemiGroundColor: string;
  hemiIntensity: number;
  starOpacity: number;
}

export const CELESTIAL_TIMELINE: KeyframePoint[] = [
  // 00:00 Deep Night
  {
    hour: 0.0,
    sunColor: '#4a69bd',
    sunIntensity: 0.0,
    moonIntensity: 0.35,
    zenithColor: '#050b18',
    horizonColor: '#0a1226',
    groundColor: '#030712',
    fogColor: '#0a1226',
    fogDensity: 0.0022,
    ambientColor: '#0c1427',
    ambientIntensity: 0.18,
    hemiSkyColor: '#101935',
    hemiGroundColor: '#050914',
    hemiIntensity: 0.2,
    starOpacity: 1.0,
  },
  // 04:30 Astronomical Dawn
  {
    hour: 4.5,
    sunColor: '#4a69bd',
    sunIntensity: 0.0,
    moonIntensity: 0.28,
    zenithColor: '#0f172a',
    horizonColor: '#1e1b4b',
    groundColor: '#050914',
    fogColor: '#1e1b4b',
    fogDensity: 0.0025,
    ambientColor: '#1e1b4b',
    ambientIntensity: 0.2,
    hemiSkyColor: '#1e1b4b',
    hemiGroundColor: '#0b0f19',
    hemiIntensity: 0.22,
    starOpacity: 0.9,
  },
  // 06:00 Dawn Sunrise (Orange Glow #ff7733)
  {
    hour: 6.0,
    sunColor: '#ff7733',
    sunIntensity: 0.95,
    moonIntensity: 0.05,
    zenithColor: '#0284c7',
    horizonColor: '#fb923c',
    groundColor: '#451a03',
    fogColor: '#fed7aa',
    fogDensity: 0.003,
    ambientColor: '#fed7aa',
    ambientIntensity: 0.38,
    hemiSkyColor: '#38bdf8',
    hemiGroundColor: '#78350f',
    hemiIntensity: 0.4,
    starOpacity: 0.0,
  },
  // 09:00 Mid-Morning Golden Sunlight
  {
    hour: 9.0,
    sunColor: '#fff1e0',
    sunIntensity: 1.3,
    moonIntensity: 0.0,
    zenithColor: '#0284c7',
    horizonColor: '#bae6fd',
    groundColor: '#1e293b',
    fogColor: '#bae6fd',
    fogDensity: 0.0018,
    ambientColor: '#dbeafe',
    ambientIntensity: 0.42,
    hemiSkyColor: '#93c5fd',
    hemiGroundColor: '#3f2e18',
    hemiIntensity: 0.45,
    starOpacity: 0.0,
  },
  // 12:00 Midday Warm White (#fff4e6)
  {
    hour: 12.0,
    sunColor: '#fff4e6',
    sunIntensity: 1.45,
    moonIntensity: 0.0,
    zenithColor: '#0284c7',
    horizonColor: '#e0f2fe',
    groundColor: '#1e293b',
    fogColor: '#bae6fd',
    fogDensity: 0.0015,
    ambientColor: '#dbeafe',
    ambientIntensity: 0.45,
    hemiSkyColor: '#93c5fd',
    hemiGroundColor: '#3f2e18',
    hemiIntensity: 0.45,
    starOpacity: 0.0,
  },
  // 15:30 Late Afternoon
  {
    hour: 15.5,
    sunColor: '#fff1e0',
    sunIntensity: 1.32,
    moonIntensity: 0.0,
    zenithColor: '#0284c7',
    horizonColor: '#bae6fd',
    groundColor: '#1e293b',
    fogColor: '#bae6fd',
    fogDensity: 0.0018,
    ambientColor: '#dbeafe',
    ambientIntensity: 0.42,
    hemiSkyColor: '#93c5fd',
    hemiGroundColor: '#3f2e18',
    hemiIntensity: 0.45,
    starOpacity: 0.0,
  },
  // 18:00 Dusk Sunset (Purple-Red #e64a19)
  {
    hour: 18.0,
    sunColor: '#e64a19',
    sunIntensity: 0.95,
    moonIntensity: 0.05,
    zenithColor: '#312e81',
    horizonColor: '#ea580c',
    groundColor: '#451a03',
    fogColor: '#f43f5e',
    fogDensity: 0.0028,
    ambientColor: '#7c3aed',
    ambientIntensity: 0.32,
    hemiSkyColor: '#6366f1',
    hemiGroundColor: '#451a03',
    hemiIntensity: 0.38,
    starOpacity: 0.15,
  },
  // 19:30 Twilight Indigo
  {
    hour: 19.5,
    sunColor: '#e64a19',
    sunIntensity: 0.15,
    moonIntensity: 0.25,
    zenithColor: '#1e1b4b',
    horizonColor: '#4c1d95',
    groundColor: '#0b0f19',
    fogColor: '#1e1b4b',
    fogDensity: 0.0025,
    ambientColor: '#312e81',
    ambientIntensity: 0.22,
    hemiSkyColor: '#1e1b4b',
    hemiGroundColor: '#0f172a',
    hemiIntensity: 0.25,
    starOpacity: 0.85,
  },
  // 21:00 Nightfall Faint Moon Blue (#4a69bd)
  {
    hour: 21.0,
    sunColor: '#4a69bd',
    sunIntensity: 0.0,
    moonIntensity: 0.35,
    zenithColor: '#050b18',
    horizonColor: '#0a1226',
    groundColor: '#030712',
    fogColor: '#0a1226',
    fogDensity: 0.0022,
    ambientColor: '#0c1427',
    ambientIntensity: 0.18,
    hemiSkyColor: '#101935',
    hemiGroundColor: '#050914',
    hemiIntensity: 0.2,
    starOpacity: 1.0,
  },
  // 24:00 Midnight (wraps to 00:00)
  {
    hour: 24.0,
    sunColor: '#4a69bd',
    sunIntensity: 0.0,
    moonIntensity: 0.35,
    zenithColor: '#050b18',
    horizonColor: '#0a1226',
    groundColor: '#030712',
    fogColor: '#0a1226',
    fogDensity: 0.0022,
    ambientColor: '#0c1427',
    ambientIntensity: 0.18,
    hemiSkyColor: '#101935',
    hemiGroundColor: '#050914',
    hemiIntensity: 0.2,
    starOpacity: 1.0,
  },
];

/**
 * Evaluates the dynamic 24-hour celestial and lighting state for Map 2.
 */
export function computeCelestialState(
  timeOfDay: number,
  windSpeed = 2.5,
  windDir: [number, number] = [1.0, 0.5],
): CelestialState {
  const normTime = ((timeOfDay % 24) + 24) % 24;

  // 1. Calculate Solar & Lunar Orbital Trajectories
  // Sun travels across east-to-west arc with 23.5 degree inclination
  const solarAngle = ((normTime - 6.0) / 12.0) * Math.PI;
  const inclination = (23.5 * Math.PI) / 180.0;

  const sunX = Math.cos(solarAngle);
  const sunY = Math.sin(solarAngle) * Math.cos(inclination);
  const sunZ = Math.sin(solarAngle) * Math.sin(inclination);

  const sunDirection = new THREE.Vector3(sunX, sunY, sunZ).normalize();
  const sunPosition = sunDirection.clone().multiplyScalar(400);

  const moonDirection = sunDirection.clone().negate();
  const moonPosition = moonDirection.clone().multiplyScalar(400);

  const sunElevation = Math.asin(Math.max(-1, Math.min(1, sunDirection.y)));
  const sunAzimuth = Math.atan2(sunDirection.z, sunDirection.x);
  const isDay = sunElevation > -0.05;

  // 2. Interpolate Color Timeline
  let k1 = CELESTIAL_TIMELINE[0];
  let k2 = CELESTIAL_TIMELINE[CELESTIAL_TIMELINE.length - 1];

  for (let i = 0; i < CELESTIAL_TIMELINE.length - 1; i++) {
    if (normTime >= CELESTIAL_TIMELINE[i].hour && normTime <= CELESTIAL_TIMELINE[i + 1].hour) {
      k1 = CELESTIAL_TIMELINE[i];
      k2 = CELESTIAL_TIMELINE[i + 1];
      break;
    }
  }

  const span = k2.hour - k1.hour || 1;
  const tRaw = (normTime - k1.hour) / span;
  const t = tRaw * tRaw * (3.0 - 2.0 * tRaw);

  const sunColor = new THREE.Color(k1.sunColor).lerp(new THREE.Color(k2.sunColor), t);
  const moonColor = new THREE.Color('#8fa8d6');

  const sunIntensity = THREE.MathUtils.lerp(k1.sunIntensity, k2.sunIntensity, t);
  const moonIntensity = THREE.MathUtils.lerp(k1.moonIntensity, k2.moonIntensity, t);

  const skyZenithColor = new THREE.Color(k1.zenithColor).lerp(new THREE.Color(k2.zenithColor), t);
  const skyHorizonColor = new THREE.Color(k1.horizonColor).lerp(new THREE.Color(k2.horizonColor), t);
  const skyGroundColor = new THREE.Color(k1.groundColor).lerp(new THREE.Color(k2.groundColor), t);

  const fogColor = new THREE.Color(k1.fogColor).lerp(new THREE.Color(k2.fogColor), t);
  const fogDensity = THREE.MathUtils.lerp(k1.fogDensity, k2.fogDensity, t);

  const ambientColor = new THREE.Color(k1.ambientColor).lerp(new THREE.Color(k2.ambientColor), t);
  const ambientIntensity = THREE.MathUtils.lerp(k1.ambientIntensity, k2.ambientIntensity, t);

  const hemiSkyColor = new THREE.Color(k1.hemiSkyColor).lerp(new THREE.Color(k2.hemiSkyColor), t);
  const hemiGroundColor = new THREE.Color(k1.hemiGroundColor).lerp(new THREE.Color(k2.hemiGroundColor), t);
  const hemiIntensity = THREE.MathUtils.lerp(k1.hemiIntensity, k2.hemiIntensity, t);

  const starOpacity = THREE.MathUtils.lerp(k1.starOpacity, k2.starOpacity, t);

  const windDirectionVec = new THREE.Vector2(windDir[0], windDir[1]).normalize();

  return {
    timeOfDay: normTime,
    sunPosition,
    sunDirection,
    moonPosition,
    moonDirection,
    sunElevation,
    sunAzimuth,
    sunColor,
    sunIntensity,
    moonColor,
    moonIntensity,
    skyZenithColor,
    skyHorizonColor,
    skyGroundColor,
    fogColor,
    fogDensity,
    ambientColor,
    ambientIntensity,
    hemiSkyColor,
    hemiGroundColor,
    hemiIntensity,
    starOpacity,
    isDay,
    windSpeed,
    windDirection: windDirectionVec,
  };
}

/**
 * Builds a deterministic procedural starfield BufferGeometry on a celestial dome.
 */
export function buildMap2StarfieldGeometry(
  count = 2000,
  radius = 1150,
): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry();
  const positions = new Float32Array(count * 3);
  const sizes = new Float32Array(count);
  const phases = new Float32Array(count);
  const twinkleSpeeds = new Float32Array(count);
  const colors = new Float32Array(count * 3);

  let seed = 123456789;
  const lcg = () => {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed / 4294967296;
  };

  const starPalette = [
    new THREE.Color('#ffffff'),
    new THREE.Color('#dbeafe'),
    new THREE.Color('#fef08a'),
    new THREE.Color('#93c5fd'),
    new THREE.Color('#fed7aa'),
  ];

  for (let i = 0; i < count; i++) {
    const y = lcg() * 0.95 + 0.05;
    const phi = Math.acos(y);
    const theta = lcg() * Math.PI * 2.0;

    const r = radius * (0.98 + lcg() * 0.04);
    const x = r * Math.sin(phi) * Math.cos(theta);
    const z = r * Math.sin(phi) * Math.sin(theta);
    const posY = r * Math.cos(phi);

    positions[i * 3] = x;
    positions[i * 3 + 1] = posY;
    positions[i * 3 + 2] = z;

    sizes[i] = 1.2 + lcg() * 2.8;
    phases[i] = lcg() * Math.PI * 2.0;
    twinkleSpeeds[i] = 1.2 + lcg() * 3.8;

    const col = starPalette[Math.floor(lcg() * starPalette.length)];
    colors[i * 3] = col.r;
    colors[i * 3 + 1] = col.g;
    colors[i * 3 + 2] = col.b;
  }

  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute('aSize', new THREE.BufferAttribute(sizes, 1));
  geometry.setAttribute('aPhase', new THREE.BufferAttribute(phases, 1));
  geometry.setAttribute('aTwinkleSpeed', new THREE.BufferAttribute(twinkleSpeeds, 1));
  geometry.setAttribute('aStarColor', new THREE.BufferAttribute(colors, 3));

  return geometry;
}
