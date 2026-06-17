import * as THREE from 'three';

export interface SkyParams {
  skyColor: string;
  sunColor: string;
  sunIntensity: number;
  ambientColor: string;
  ambientIntensity: number;
  hemiSkyColor: string;
  hemiGroundColor: string;
  hemiIntensity: number;
  starOpacity: number;
}

export function getSkyParams(timeOfDay: number): SkyParams {
  const keyframes = [
    { time: 0, skyColor: '#02020a', sunColor: '#0a0d1a', sunIntensity: 0.0, ambientColor: '#050510', ambientIntensity: 0.05, hemiSkyColor: '#050510', hemiGroundColor: '#020205', hemiIntensity: 0.05, starOpacity: 1.0 },
    { time: 4.5, skyColor: '#02020a', sunColor: '#0a0d1a', sunIntensity: 0.0, ambientColor: '#050510', ambientIntensity: 0.05, hemiSkyColor: '#050510', hemiGroundColor: '#020205', hemiIntensity: 0.05, starOpacity: 1.0 },
    { time: 6.0, skyColor: '#f97316', sunColor: '#fdba74', sunIntensity: 0.4, ambientColor: '#fed7aa', ambientIntensity: 0.3, hemiSkyColor: '#fdba74', hemiGroundColor: '#451a03', hemiIntensity: 0.3, starOpacity: 0.0 },
    { time: 8.0, skyColor: '#bae6fd', sunColor: '#fef08a', sunIntensity: 0.9, ambientColor: '#bae6fd', ambientIntensity: 0.6, hemiSkyColor: '#bae6fd', hemiGroundColor: '#1e3a8a', hemiIntensity: 0.6, starOpacity: 0.0 },
    { time: 12.0, skyColor: '#0ea5e9', sunColor: '#ffffff', sunIntensity: 1.2, ambientColor: '#e0f2fe', ambientIntensity: 0.8, hemiSkyColor: '#0ea5e9', hemiGroundColor: '#0f172a', hemiIntensity: 0.8, starOpacity: 0.0 },
    { time: 16.0, skyColor: '#38bdf8', sunColor: '#fef08a', sunIntensity: 0.9, ambientColor: '#bae6fd', ambientIntensity: 0.6, hemiSkyColor: '#38bdf8', hemiGroundColor: '#1e3a8a', hemiIntensity: 0.6, starOpacity: 0.0 },
    { time: 18.0, skyColor: '#f97316', sunColor: '#fdba74', sunIntensity: 0.5, ambientColor: '#ffedd5', ambientIntensity: 0.4, hemiSkyColor: '#f97316', hemiGroundColor: '#431407', hemiIntensity: 0.4, starOpacity: 0.0 },
    { time: 19.5, skyColor: '#1e1b4b', sunColor: '#a5b4fc', sunIntensity: 0.0, ambientColor: '#111827', ambientIntensity: 0.1, hemiSkyColor: '#1e1b4b', hemiGroundColor: '#090514', hemiIntensity: 0.1, starOpacity: 0.8 },
    { time: 24.0, skyColor: '#02020a', sunColor: '#0a0d1a', sunIntensity: 0.0, ambientColor: '#050510', ambientIntensity: 0.05, hemiSkyColor: '#050510', hemiGroundColor: '#020205', hemiIntensity: 0.05, starOpacity: 1.0 },
  ];

  let idx = 0;
  for (let i = 0; i < keyframes.length - 1; i++) {
    if (timeOfDay >= keyframes[i].time && timeOfDay <= keyframes[i + 1].time) {
      idx = i;
      break;
    }
  }

  const kf1 = keyframes[idx];
  const kf2 = keyframes[idx + 1];
  const t = (timeOfDay - kf1.time) / (kf2.time - kf1.time);

  const lerpNum = (a: number, b: number, factor: number) => a + (b - a) * factor;
  const lerpColor = (c1: string, c2: string, factor: number) => {
    const color1 = new THREE.Color(c1);
    const color2 = new THREE.Color(c2);
    color1.lerp(color2, factor);
    return `#${color1.getHexString()}`;
  };

  return {
    skyColor: lerpColor(kf1.skyColor, kf2.skyColor, t),
    sunColor: lerpColor(kf1.sunColor, kf2.sunColor, t),
    sunIntensity: lerpNum(kf1.sunIntensity, kf2.sunIntensity, t),
    ambientColor: lerpColor(kf1.ambientColor, kf2.ambientColor, t),
    ambientIntensity: lerpNum(kf1.ambientIntensity, kf2.ambientIntensity, t),
    hemiSkyColor: lerpColor(kf1.hemiSkyColor, kf2.hemiSkyColor, t),
    hemiGroundColor: lerpColor(kf1.hemiGroundColor, kf2.hemiGroundColor, t),
    hemiIntensity: lerpNum(kf1.hemiIntensity, kf2.hemiIntensity, t),
    starOpacity: lerpNum(kf1.starOpacity, kf2.starOpacity, t),
  };
}
