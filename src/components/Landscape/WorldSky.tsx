import React, { useEffect, useRef, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { getWorldLightRig, sunDirectionWithTilt, type SkyParams } from './utils/skyParams';
import {
  ATMOSPHERE_FRAGMENT_PARS,
  atmosphereUniforms,
  setAtmosphereClouds,
  updateAtmosphereSun,
} from './materials/atmosphere';
import { makeSceneRandom, sceneDelta, sceneElapsed } from './utils/sceneClock';

// Gradient sky dome: zenith deepens, horizon pales into haze, and a soft halo wraps the sun.
// A flat single-colour dome is the single biggest "this is a tech demo" tell — the gradient
// alone makes every frame read as atmosphere.
const SKY_VERTEX = /* glsl */ `
varying vec3 vDir;
void main(){
vDir=position;
gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0);
}
`;
const SKY_FRAGMENT = /* glsl */ `
precision highp float;
varying vec3 vDir;
uniform vec3 uZenith;
uniform vec3 uHorizon;
uniform vec3 uSunDir;
uniform vec3 uSunColor;
uniform float uGlow;
uniform vec3 uMoonDir;
uniform float uMoonGlow;
void main(){
vec3 d=normalize(vDir);
float h=clamp(d.y,0.0,1.0);
vec3 col=mix(uHorizon,uZenith,pow(h,0.58));
float dayMix=smoothstep(0.18,0.72,max(uHorizon.r,max(uHorizon.g,uHorizon.b)));
vec3 hazeBelt=mix(uHorizon,vec3(0.77,0.87,0.94),dayMix*0.72);
col=mix(hazeBelt,col,smoothstep(0.0,0.16,abs(d.y)));
float s=max(dot(d,normalize(uSunDir)),0.0);
col+=uSunColor*(pow(s,300.0)*0.9+pow(s,10.0)*0.16*uGlow);
` +
  // Cold, tighter halo around the moon at night.
  `float m=max(dot(d,normalize(uMoonDir)),0.0);
col+=vec3(0.58,0.66,0.8)*(pow(m,700.0)*0.55+pow(m,26.0)*0.05)*uMoonGlow;
gl_FragColor=vec4(col,1.0);
}
`;

// prototype, to be replaced — WV2-LOOK dome: the shared Preetham sky (atmosphere.ts) plus the
// moon's halo. The same function colours the haze and the water's sky reflection, so land, sea
// and sky meet at the horizon without a seam.
const SKY_LOOK_FRAGMENT = /* glsl */ `
precision highp float;
varying vec3 vDir;
uniform vec3 uMoonDir;
uniform float uMoonGlow;
${ATMOSPHERE_FRAGMENT_PARS}
void main(){
vec3 d=normalize(vDir);
vec3 dd=vec3(d.x,max(d.y,0.0),d.z);
vec3 col=uAtmoPhys>=1.0?atmoPreetham(dd)
:(uAtmoPhys<=0.0?atmoSkyColorLegacy(dd):mix(atmoSkyColorLegacy(dd),atmoPreetham(dd),uAtmoPhys));
float m=max(dot(d,normalize(uMoonDir)),0.0);
col+=vec3(0.58,0.66,0.8)*(pow(m,700.0)*0.55+pow(m,26.0)*0.05)*uMoonGlow;
gl_FragColor=vec4(col,1.0);
}
`;

// prototype, to be replaced — WV2-LOOK trade cumulus as lit impostors, one instanced draw.
// A grid of candidate puffs (N^2 over a tile ~5 map extents wide, re-centred on the camera)
// drifts with the wind; each puff asks the cloud field (atmosphere.ts) how thick the cumulus is
// where it now floats, so cells grow over the windward slopes and fade downwind, and the
// terrain and sea shadows come from the same field. A puff is a lumpy sphere impostor: lit
// from its own normal, its flat base cut at the condensation level, softened at the rim.
const CLOUD_VERTEX = /* glsl */ `
uniform float uTile;
uniform float uN;
attribute vec4 aCell;
varying vec2 vUv;
varying vec3 vC;
varying float vR;
varying float vRy;
varying float vSeed;
varying float vNear;
varying float vNearG;
varying float vHaze;
varying vec3 vHazeC;
${ATMOSPHERE_FRAGMENT_PARS}
void main(){
float sp=uTile/uN;
vec2 camM=cameraPosition.xz*uAtmoMPerUnit;
vec2 p=aCell.xy*uTile+(aCell.zw-0.5)*sp*0.9+uAtmoWind*uAtmoTime;
p=camM+mod(p-camM+0.5*uTile,uTile)-0.5*uTile;
gl_Position=vec4(2.0,2.0,2.0,1.0);
vec2 dom=atmoCloudDom(p);
float C=atmoCloudC(p,dom.g,atmoCloudRegion(p));
if(C<0.03)return;
float top=atmoCloudTop(p,C);
vec2 pu=p/uAtmoMPerUnit;
` +
  // Real-size cumulus: ~0.5-1.6 km across over the sea (thin cells), growing to ~3 km and
  // taller where the cell is thick (the windward bank). Thin ones are 1.6-2.4 times wider than
  // tall seen from the side; the centre sits low, so the condensation level cuts a flat base,
  // and in thick cells some puffs ride higher, building the bank upward. Coarser grids (medium,
  // low) grow their clouds a little so the bank does not fall apart into separate puffs.
  `float rM=mix(250.0,2000.0,pow(top,1.5))*(0.75+0.5*fract(aCell.z*7.13))*sqrt(clamp(sp/1000.0,1.0,4.0));
float asp=mix(2.0,1.1,top)*(0.8+0.4*fract(aCell.w*5.31));
vec3 rc=normalize(vec3(pu.x,uAtmoCloudY,pu.y)-cameraPosition);
float ryM=rM*mix(1.0,1.0/asp,1.0-smoothstep(0.08,0.4,-rc.y));
float cy=uAtmoCloudY+(0.3+1.1*top*fract(aCell.z*3.17))*ryM/uAtmoMPerUnitY;
if(top<0.02||dom.r>cy-0.3*ryM/uAtmoMPerUnitY)return;
vec3 c=vec3(pu.x,cy,pu.y);
vR=rM/uAtmoMPerUnit;
vRy=ryM/uAtmoMPerUnit;
` +
  // Per puff, not per pixel: gone when the camera is inside or right next to it, its haze (along
  // the direction to its base), and whether any terrain within its radius reaches its lower
  // half (else no terrain fade lookup).
  `vNear=smoothstep(1.0,2.4,length(c-cameraPosition)/vR);
if(vNear<0.01)return;
vHazeC=atmoSkyColor(normalize(vec3(rc.x,abs(rc.y)*0.2,rc.z)));
vHaze=atmoHazeAmount(cameraPosition,c);
float gm=max(max(atmoCloudDom(p+vec2(rM,0.0)).r,atmoCloudDom(p-vec2(rM,0.0)).r),max(atmoCloudDom(p+vec2(0.0,rM)).r,atmoCloudDom(p-vec2(0.0,rM)).r));
vNearG=step(cy-vR,max(gm,dom.r));
vec4 mv=viewMatrix*vec4(c,1.0);
mv.xy+=position.xy*vec2(vR,vRy);
gl_Position=projectionMatrix*mv;
vUv=position.xy;
vC=c;
vSeed=aCell.z;
}
`;
const CLOUD_FRAGMENT = /* glsl */ `
precision highp float;
varying vec2 vUv;
varying vec3 vC;
varying float vR;
varying float vRy;
varying float vSeed;
varying float vNear;
varying float vNearG;
varying float vHaze;
varying vec3 vHazeC;
${ATMOSPHERE_FRAGMENT_PARS}
void main(){
if(dot(vUv,vUv)>=1.0)discard;
vec3 cr=vec3(viewMatrix[0][0],viewMatrix[1][0],viewMatrix[2][0]);
vec3 cu=vec3(viewMatrix[0][1],viewMatrix[1][1],viewMatrix[2][1]);
vec3 cb=vec3(viewMatrix[0][2],viewMatrix[1][2],viewMatrix[2][2]);
vec3 wp=vC+vUv.x*vR*cr+vUv.y*vRy*cu;
float dM=length(wp-cameraPosition)*uAtmoMPerUnit;
if(dM<300.0)discard;
` +
  // Four overlapping lobes (a central dome and three around it, their layout from the seed)
  // give each card a cauliflower outline; the nearest lobe's sphere is the surface normal.
  // 2-5 lobes per cloud, their offsets and sizes from the seed, so no two clouds share a shape;
  // a smooth union (soft minimum) with the normal blended by the same weights: no seams.
  `vec4 nz=texture2D(uAtmoCloudTex,vUv*0.6+vSeed*3.7);
float wob=0.82+0.3*nz.r+0.12*nz.g;
int nl=2+int(fract(vSeed*13.7)*3.99);
float ws=0.0;
vec2 l=vec2(0.0);
for(int k=0;k<5;k++){
if(k>=nl)break;
float fk=float(k);
vec3 h=fract(vSeed*vec3(17.3+fk*7.1,29.1+fk*3.7,41.7+fk*5.3));
vec2 o=k==0?vec2(0.0,0.05):vec2((h.x-0.5)*0.84,(h.y-0.3)*0.5);
float rad=k==0?0.55:0.3+0.22*h.z;
vec2 dl=(vUv-o)/rad;
float w=exp(-dot(dl,dl)*wob/0.12);
ws+=w;
l+=dl*w;
}
float r2=-0.12*log(max(ws,1e-6));
l/=max(ws,1e-6);
if(r2>=1.0)discard;
vec3 n=normalize(l.x*cr+l.y*cu+sqrt(max(1.0-dot(l,l),0.0))*cb);
float hB=(wp.y-uAtmoCloudY)*uAtmoMPerUnitY;
vec3 rd=normalize(wp-cameraPosition);
` +
  // The flat base and its darker underside only show from below or the side; seen from above
  // the puff stays round (cut the card there and it reads as a half dome).
  `float side=1.0-smoothstep(0.08,0.4,-rd.y);
` +
  // Soft rim, flat base at the condensation level, thinning near the camera.
  `float ryM=vRy*uAtmoMPerUnit;
float a=(1.0-smoothstep(0.45,1.0,r2))*mix(1.0,smoothstep(-0.08*ryM,0.2*ryM,hB),side)*smoothstep(300.0,1100.0,dM);
` +
  // Soft where the card meets the terrain (the coarse height map stands in for a depth
  // buffer), and gone when the camera is inside or right next to the puff.
  `if(vNearG>0.5)a*=smoothstep(0.0,700.0,(wp.y-atmoCloudDom(wp.xz*uAtmoMPerUnit).r)*uAtmoMPerUnitY);
a*=vNear;
vec3 sd=normalize(uAtmoSunDir);
` +
  // Gentle shading (cumulus are not billiard balls): a little sun on the lit side, a grey
  // base darkening toward the condensation level when seen from below or the side.
  `float sunL=clamp(dot(n,sd)*mix(0.25,0.45,side)+mix(0.78,0.58,side),0.0,1.0)*mix(1.0,mix(0.5,1.0,smoothstep(0.0,1.2*ryM,hB)),side);
vec3 col=mix(vec3(0.60,0.64,0.72),vec3(1.0,0.985,0.95),sunL)*mix(0.93,1.04,nz.g);
col+=vec3(1.0,0.95,0.85)*pow(max(dot(rd,sd),0.0),8.0)*0.35*smoothstep(0.3,1.0,r2);
col=mix(col,vHazeC,vHaze);
gl_FragColor=vec4(min(col,vec3(1.0)),a*0.96);
}
`;

/** `n^2` candidate puffs: grid cell (i/n, j/n) plus two random seeds, one instanced quad. */
function buildCloudPuffGeometry(n: number): THREE.InstancedBufferGeometry {
  const g = new THREE.InstancedBufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute([-1, -1, 0, 1, -1, 0, 1, 1, 0, -1, 1, 0], 3));
  g.setIndex([0, 1, 2, 0, 2, 3]);
  const cell = new Float32Array(n * n * 4);
  let s = 12345;
  const rnd = () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
  for (let j = 0; j < n; j += 1) {
    for (let i = 0; i < n; i += 1) {
      const o = (j * n + i) * 4;
      cell[o] = i / n;
      cell[o + 1] = j / n;
      cell[o + 2] = rnd();
      cell[o + 3] = rnd();
    }
  }
  g.setAttribute('aCell', new THREE.InstancedBufferAttribute(cell, 4));
  g.instanceCount = n * n;
  return g;
}

/** Per-frame inputs of the WV2-LOOK physical sky and clouds (module state, not React state). */
function applyLookAtmosphere(sunDir: THREE.Vector3, timeSec: number): void {
  updateAtmosphereSun(sunDir, THREE.MathUtils.smoothstep(sunDir.y, -0.08, 0.06));
  atmosphereUniforms.uAtmoTime.value = timeSec;
}

// ---------------------------------------------------------------------------------------
// WorldSky — day/night sky + lighting for the huge world (WorldShowcase).
//
// Same keyframe model as the legacy Sky.tsx (driven by `getSkyParams(timeOfDay)`), but every
// distance is scaled up for the big world: the terrain spans ~RENDER_SIZE (400) units and the
// camera orbits out to ~1200, so the dome / sun-orbit / stars all sit well beyond that.
//
// Returns the current sun direction via `onSun` so the water shader's specular can track it.
// ---------------------------------------------------------------------------------------

export interface WorldSkyProps {
  timeOfDay?: number; // 0..24
  /** Spin speed of the slow dome rotation + cloud drift. */
  speed?: number;
  /** Overall scale; dome/orbit/star radii derive from this (≈ camera far budget). */
  worldScale?: number;
  /**
   * prototype, to be replaced — WV2-LOOK: tilt of the sun's daily arc (radians) so a
   * late-morning sun sits ~45-50 degrees up instead of near the zenith, plus a stronger key
   * light against a dimmer, warmer fill. 0 / false = the legacy rig, unchanged.
   */
  sunTilt?: number;
  look?: boolean;
  /** WV2-LOOK cumulus: candidate puffs per side of the drifting grid (quality tier). */
  cloudGrid?: number;
}

interface CloudDescriptor {
  id: number;
  position: [number, number, number];
  scale: [number, number, number];
  speed: number;
}

/**
 * The dome shader's inputs. Live objects: three reads `.value` at draw time, every frame.
 *
 * A type alias rather than an interface so it keeps the implicit index signature `<shaderMaterial
 * uniforms=...>` needs — an interface would have to declare `[k: string]` explicitly and lose the
 * exact member list that makes `applySkyUniforms` checkable.
 */
type SkyUniforms = {
  uZenith: { value: THREE.Color };
  uHorizon: { value: THREE.Color };
  uSunDir: { value: THREE.Vector3 };
  uSunColor: { value: THREE.Color };
  uGlow: { value: number };
  uMoonDir: { value: THREE.Vector3 };
  uMoonGlow: { value: number };
};

function createSkyUniforms(): SkyUniforms {
  return {
    uZenith: { value: new THREE.Color('#3f7fd0') },
    uHorizon: { value: new THREE.Color('#cfe6f2') },
    uSunDir: { value: new THREE.Vector3(0, 1, 0) },
    uSunColor: { value: new THREE.Color('#fff2cc') },
    uGlow: { value: 1 },
    uMoonDir: { value: new THREE.Vector3(0, -1, 0) },
    uMoonGlow: { value: 0 },
  };
}

/** One frame of the day/night palette, as the dome's shader wants it. */
interface SkyFrame {
  params: SkyParams;
  sunPosition: [number, number, number];
  moonPosition: [number, number, number];
  moonGlow: number;
}

/**
 * Pack a frame's palette and orbit into the dome's uniforms.
 *
 * A named operation over the uniform object rather than eight assignments inside `useFrame`: the
 * object is built by `useMemo` and therefore belongs to render, and a frame callback writing
 * straight into it is the "modified after render completes" hazard `react-hooks/immutability`
 * names. Gathering the whole update behind one call keeps the writes in one place — and makes the
 * packing itself something that can be called without a renderer.
 *
 * `scratch` and `haze` are caller-owned colours so this allocates nothing per frame.
 */
function applySkyUniforms(
  u: SkyUniforms,
  frame: SkyFrame,
  scratch: THREE.Color,
  haze: THREE.Color,
): void {
  // The horizon pales toward haze by day but stays dark at night, scaled by the sky's luminance.
  scratch.set(frame.params.skyColor);
  const lum = (scratch.r + scratch.g + scratch.b) / 3;
  u.uZenith.value.copy(scratch).multiplyScalar(0.62);
  u.uHorizon.value.copy(scratch).lerp(haze, 0.15 + 0.4 * Math.min(1, lum * 2.2));
  u.uSunDir.value.set(frame.sunPosition[0], frame.sunPosition[1], frame.sunPosition[2]).normalize();
  u.uSunColor.value.set(frame.params.sunColor);
  u.uGlow.value = frame.params.sunIntensity;
  u.uMoonDir.value
    .set(frame.moonPosition[0], frame.moonPosition[1], frame.moonPosition[2])
    .normalize();
  u.uMoonGlow.value = frame.moonGlow;
  // WV2-LOOK aerial perspective reads the dome's own horizon so distant land melts into it.
  atmosphereUniforms.uAtmoHorizon.value.copy(u.uHorizon.value);
  atmosphereUniforms.uAtmoZenith.value.copy(u.uZenith.value);
  atmosphereUniforms.uAtmoSunDir.value.copy(u.uSunDir.value);
  atmosphereUniforms.uAtmoSunColor.value.copy(u.uSunColor.value);
  atmosphereUniforms.uAtmoGlow.value = u.uGlow.value;
}

/** Nudge the key light toward a warm late-morning white (WV2-LOOK only). */
function warmSun(hex: string): string {
  const c = new THREE.Color(hex).lerp(new THREE.Color('#ffe2b8'), 0.45);
  return `#${c.getHexString()}`;
}

export const WorldSky: React.FC<WorldSkyProps> = ({
  timeOfDay = 12,
  speed = 1.0,
  worldScale = 400,
  sunTilt = 0,
  look = false,
  cloudGrid = 210,
}) => {
  const cloudsRef = useRef<THREE.Group>(null);
  const tmpColor = useMemo(() => new THREE.Color(), []);
  const HAZE = useMemo(() => new THREE.Color('#eef6fb'), []);
  const skyUniforms = useMemo(() => createSkyUniforms(), []);
  // WV2-LOOK dome: the shared atmosphere set plus the moon terms of the legacy block.
  const lookSkyUniforms = useMemo(
    () => ({ ...atmosphereUniforms, uMoonDir: skyUniforms.uMoonDir, uMoonGlow: skyUniforms.uMoonGlow }),
    [skyUniforms],
  );
  // The puff tile spans ~5 map extents around the camera (scale-agnostic: extent in metres is
  // the render size times the metres per unit that `configureAtmosphere` set).
  const cloudUniforms = useMemo<Record<string, THREE.IUniform>>(
    () => ({
      ...atmosphereUniforms,
      uTile: { value: 5 * worldScale * atmosphereUniforms.uAtmoMPerUnit.value },
      uN: { value: cloudGrid },
    }),
    [worldScale, cloudGrid],
  );
  const cloudPuffs = useMemo(() => buildCloudPuffGeometry(cloudGrid), [cloudGrid]);
  useEffect(() => () => cloudPuffs.dispose(), [cloudPuffs]);
  useEffect(() => {
    setAtmosphereClouds(look);
    return () => setAtmosphereClouds(false);
  }, [look]);

  const DOME_R = worldScale * 6.5; // 2600 @400 — beyond camera maxDistance (1200)
  const ORBIT_R = worldScale * 4.5; // 1800
  const STAR_R = worldScale * 6.0; // 2400
  const CLOUD_Y = worldScale * 0.8; // 320
  const CLOUD_SPREAD = worldScale * 2.4; // 960

  // Sun / moon orbit (same tilt convention as legacy Sky).
  const safeTime = Number.isFinite(timeOfDay) ? ((timeOfDay % 24) + 24) % 24 : 0;
  const angle = ((safeTime - 6) / 24) * Math.PI * 2;
  const sunDirUnit = useMemo(() => {
    if (sunTilt !== 0) {
      const d = sunDirectionWithTilt(safeTime, sunTilt);
      return new THREE.Vector3(d[0], d[1], d[2]);
    }
    return new THREE.Vector3(Math.cos(angle), Math.sin(angle), Math.sin(angle) * 0.1).normalize();
  }, [angle, safeTime, sunTilt]);
  const sunPosition: [number, number, number] = [
    sunDirUnit.x * ORBIT_R,
    sunDirUnit.y * ORBIT_R,
    sunDirUnit.z * ORBIT_R,
  ];
  const moonAngle = angle + Math.PI;
  const moonDirY = Math.sin(moonAngle);
  const moonPosition: [number, number, number] = [
    Math.cos(moonAngle) * ORBIT_R,
    moonDirY * ORBIT_R,
    Math.sin(moonAngle) * 0.1 * ORBIT_R,
  ];

  const lightRig = getWorldLightRig(safeTime);
  const { params, showMoon } = lightRig;
  // WV2-LOOK key/fill balance: sun carries the modelling, sky fill is dimmer and the ground
  // bounce is warm earth instead of near-black navy. Exposure is raised in WorldShowcase.
  const sunLightIntensity = look ? lightRig.sunIntensity * 2.1 : lightRig.sunIntensity;
  const hemiLightIntensity = look ? lightRig.hemiIntensity * 1.8 : lightRig.hemiIntensity;
  const ambientLightIntensity = look ? lightRig.ambientIntensity * 0.5 : lightRig.ambientIntensity;
  // The legacy sky colour has almost no red, which turned every shadow cyan; skylight is blue
  // but broad-band.
  const hemiSky = look ? '#a9c8e6' : params.hemiSkyColor;
  const hemiGround = look ? '#5a4a36' : params.hemiGroundColor;
  const sunLightColor = look ? warmSun(params.sunColor) : params.sunColor;
  const showStars = params.starOpacity > 0;

  const starPositions = useMemo(() => {
    const count = 700;
    const positions = new Float32Array(count * 3);
    // Seeded under capture, `Math.random` otherwise. 700 stars scattered afresh on every page load
    // are 700 pixels that differ between two shots of the same view — invisible to a reviewer and
    // fatal to a byte-comparison reproducibility gate. See `sceneClock.ts`.
    const rand = makeSceneRandom('sky.stars');
    for (let i = 0; i < count; i++) {
      const u = rand();
      const v = rand();
      const theta = u * 2.0 * Math.PI;
      const phi = Math.acos(2.0 * v - 1.0);
      positions[i * 3] = STAR_R * Math.sin(phi) * Math.cos(theta);
      positions[i * 3 + 1] = Math.abs(STAR_R * Math.sin(phi) * Math.sin(theta)); // keep above horizon
      positions[i * 3 + 2] = STAR_R * Math.cos(phi);
    }
    return positions;
  }, [STAR_R]);

  const clouds = useMemo<CloudDescriptor[]>(() => {
    const s = worldScale / 400;
    const base: CloudDescriptor[] = [
      { id: 1, position: [-150, 60, -50], scale: [25, 6, 15], speed: 1.5 },
      { id: 2, position: [-80, 75, -120], scale: [35, 8, 20], speed: 1.0 },
      { id: 3, position: [20, 65, 80], scale: [20, 5, 12], speed: 2.0 },
      { id: 4, position: [90, 80, -20], scale: [40, 10, 25], speed: 0.8 },
      { id: 5, position: [-200, 70, 40], scale: [30, 7, 18], speed: 1.2 },
      { id: 6, position: [160, 85, -80], scale: [28, 6, 16], speed: 1.6 },
      { id: 7, position: [-300, 90, 180], scale: [44, 9, 26], speed: 1.1 },
      { id: 8, position: [260, 78, 140], scale: [32, 7, 20], speed: 1.4 },
      { id: 9, position: [40, 68, 200], scale: [26, 6, 15], speed: 1.3 },
      { id: 10, position: [-120, 82, 260], scale: [38, 9, 22], speed: 0.9 },
      { id: 11, position: [220, 72, 60], scale: [22, 5, 13], speed: 1.7 },
      { id: 12, position: [-260, 66, -180], scale: [30, 7, 17], speed: 1.2 },
      { id: 13, position: [120, 88, -220], scale: [46, 10, 26], speed: 0.7 },
      { id: 14, position: [-40, 76, -280], scale: [24, 6, 14], speed: 1.5 },
    ];
    // Modest cumulus puffs high above the terrain — NOT map-sized slabs: at worldScale 1000
    // a x4 multiplier made 400-unit blobs that smeared into streaks across the whole sky.
    return base.map((c) => ({
      ...c,
      position: [c.position[0] * 4.5 * s, CLOUD_Y * 1.35 + c.position[1] * s, c.position[2] * 4.5 * s],
      scale: [c.scale[0] * 1.8 * s, c.scale[1] * 1.4 * s, c.scale[2] * 1.8 * s],
    }));
  }, [worldScale, CLOUD_Y]);

  useFrame((state, rawDelta) => {
    // Fixed under capture. Cloud drift below integrates this, and `speed = 0` happens to zero the
    // product today — a coincidence of the canonical defaults, not a property worth relying on.
    const delta = sceneDelta(rawDelta);
    const safeDelta = Math.min(delta, 0.1);
    // Background follows the sky colour so night actually goes dark beyond the dome. Reached
    // through `state.scene`, the handle r3f gives the frame loop for imperative work.
    if (state.scene.background instanceof THREE.Color) {
      state.scene.background.set(params.skyColor);
    } else {
      state.scene.background = new THREE.Color(params.skyColor);
    }

    // Drive the gradient + halo from the day/night palette.
    applySkyUniforms(
      skyUniforms,
      { params, sunPosition, moonPosition, moonGlow: showMoon ? moonDirY : 0 },
      tmpColor,
      HAZE,
    );
    if (look) applyLookAtmosphere(skyUniforms.uSunDir.value, sceneElapsed(state.clock));

    if (cloudsRef.current) {
      const children = cloudsRef.current.children;
      for (let i = 0; i < children.length; i++) {
        const child = children[i] as THREE.Object3D;
        child.position.x += safeDelta * (clouds[i]?.speed ?? 1) * 8.0 * speed;
        if (child.position.x > CLOUD_SPREAD) child.position.x = -CLOUD_SPREAD;
      }
    }
  });

  return (
    <group name="world-sky-group">
      {/* Sky dome — gradient shader (zenith -> horizon haze) with a soft sun halo; on the
          WV2-LOOK path the shared physical sky instead. */}
      <mesh name="world-sky-dome">
        <sphereGeometry args={[DOME_R, 32, 24]} />
        {look ? (
          <shaderMaterial
            key="look"
            vertexShader={SKY_VERTEX}
            fragmentShader={SKY_LOOK_FRAGMENT}
            uniforms={lookSkyUniforms}
            side={THREE.BackSide}
            fog={false}
          />
        ) : (
          <shaderMaterial
            key="legacy"
            vertexShader={SKY_VERTEX}
            fragmentShader={SKY_FRAGMENT}
            uniforms={skyUniforms}
            side={THREE.BackSide}
            fog={false}
          />
        )}
      </mesh>
      {look ? (
        <mesh name="world-cloud-layer" geometry={cloudPuffs} frustumCulled={false} renderOrder={3}>
          <shaderMaterial
            vertexShader={CLOUD_VERTEX}
            fragmentShader={CLOUD_FRAGMENT}
            uniforms={cloudUniforms}
            transparent
            depthWrite={false}
            side={THREE.DoubleSide}
            fog={false}
          />
        </mesh>
      ) : null}

      <directionalLight
        name="world-sun-light"
        position={sunPosition}
        color={sunLightColor}
        intensity={sunLightIntensity}
        castShadow
        shadow-mapSize-width={2048}
        shadow-mapSize-height={2048}
        shadow-camera-far={ORBIT_R * 2}
        shadow-camera-left={-worldScale * 0.7}
        shadow-camera-right={worldScale * 0.7}
        shadow-camera-top={worldScale * 0.7}
        shadow-camera-bottom={-worldScale * 0.7}
      />

      {showMoon && (
        <directionalLight
          name="world-moon-light"
          position={moonPosition}
          color="#dbeafe"
          intensity={lightRig.moonIntensity}
        />
      )}

      {/* Ambient + hemisphere are kept low so the summed irradiance stays near 1.0 — otherwise
          the light biome colours (sand, rock, snow) blow out to white and stop matching the
          minimap. The sun provides most of the directional shading. */}
      <hemisphereLight
        name="world-hemi-light"
        color={hemiSky}
        groundColor={hemiGround}
        intensity={hemiLightIntensity}
      />
      <ambientLight name="world-ambient-light" color={params.ambientColor} intensity={ambientLightIntensity} />

      {showMoon && (
        <mesh name="world-moon-mesh" position={moonPosition}>
          <sphereGeometry args={[worldScale * 0.09, 16, 16]} />
          <meshBasicMaterial color="#fffdf0" fog={false} />
        </mesh>
      )}

      {/* Sun disc */}
      {params.sunIntensity > 0.05 && (
        <mesh name="world-sun-mesh" position={sunPosition}>
          <sphereGeometry args={[worldScale * 0.11, 16, 16]} />
          <meshBasicMaterial color={params.sunColor} fog={false} />
        </mesh>
      )}

      {showStars && (
        <points name="world-stars">
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              count={starPositions.length / 3}
              array={starPositions}
              itemSize={3}
            />
          </bufferGeometry>
          <pointsMaterial size={worldScale * 0.006} color="#ffffff" transparent opacity={params.starOpacity} sizeAttenuation fog={false} />
        </points>
      )}

      {/* Puffy low-poly clouds: overlapping flat-shaded ellipsoids read as soft cumulus from
          any angle (the old axis-aligned boxes looked like slabs at the horizon). */}
      <group ref={cloudsRef} name="world-clouds-group" visible={!look}>
        {clouds.map((cloud) => (
          <group key={cloud.id} position={cloud.position}>
            {/* The core puff casts a real shadow — drifting cloud shadows sweep the land. */}
            <mesh castShadow scale={[cloud.scale[0] * 0.55, cloud.scale[1] * 0.8, cloud.scale[2] * 0.6]}>
              <sphereGeometry args={[1, 7, 6]} />
              <meshStandardMaterial color="#ffffff" transparent opacity={0.5} roughness={1} fog={false} flatShading />
            </mesh>
            <mesh
              position={[-cloud.scale[0] * 0.38, -cloud.scale[1] * 0.15, cloud.scale[2] * 0.12]}
              scale={[cloud.scale[0] * 0.38, cloud.scale[1] * 0.55, cloud.scale[2] * 0.42]}
            >
              <sphereGeometry args={[1, 7, 6]} />
              <meshStandardMaterial color="#f4f8ff" transparent opacity={0.42} roughness={1} fog={false} flatShading />
            </mesh>
            <mesh
              position={[cloud.scale[0] * 0.36, -cloud.scale[1] * 0.1, -cloud.scale[2] * 0.1]}
              scale={[cloud.scale[0] * 0.35, cloud.scale[1] * 0.5, cloud.scale[2] * 0.4]}
            >
              <sphereGeometry args={[1, 7, 6]} />
              <meshStandardMaterial color="#f8fbff" transparent opacity={0.46} roughness={1} fog={false} flatShading />
            </mesh>
          </group>
        ))}
      </group>
    </group>
  );
};

export default WorldSky;
