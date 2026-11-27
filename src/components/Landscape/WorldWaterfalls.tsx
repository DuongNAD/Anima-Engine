import React, { useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import type { QualityTier } from './utils/qualityTiers';
import { driveTimeUniform } from './utils/sceneClock';
import type { WorldV2LoadedBundle } from './worldV2/types';
import { evaluateDrawnWaterfall } from './worldV2/waterfallFit';
import { getActiveNearPatchVersion, sampleActiveNearPatchY } from './worldV2/terrainNearPatch';

// ---------------------------------------------------------------------------------------
// WorldWaterfalls — foam curtains hung over the steep river drops the generator detected
// (World.waterfall*). Instanced meshes: curtains, plunge splash pools, and (on high/medium)
// soft rising base mist sprites.
// ---------------------------------------------------------------------------------------

export interface WorldWaterfallsProps {
  world: World;
  worldV2?: WorldV2LoadedBundle | null;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
  tier?: QualityTier;
}

// ShaderMaterial on an InstancedMesh: three auto-defines USE_INSTANCING and declares the
// instanceMatrix attribute for us — we only have to APPLY it in the vertex transform.
const WATERFALL_NOISE_GLSL = /* glsl */ `
float hash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453123);}
float noise(vec2 p){
vec2 i=floor(p),f=fract(p),u=f*f*(3.0-2.0*f);
return mix(mix(hash(i),hash(i+vec2(1.0,0.0)),u.x),mix(hash(i+vec2(0.0,1.0)),hash(i+vec2(1.0,1.0)),u.x),u.y);
}
`;

const CURTAIN_VERTEX = /* glsl */ `
varying vec2 vUv;
varying vec3 vWorldPos;
void main(){
vUv=uv;
vec4 world=modelMatrix*instanceMatrix*vec4(position,1.0);
vWorldPos=world.xyz;
gl_Position=projectionMatrix*viewMatrix*world;
}
`;

const CURTAIN_FRAGMENT = /* glsl */ `
precision highp float;
varying vec2 vUv;
varying vec3 vWorldPos;
uniform float uTime;
uniform float uTerrainSize;
uniform vec3 uFogColor;
${WATERFALL_NOISE_GLSL}
void main(){
float flow1=noise(vec2(vUv.x*8.5,vUv.y*4.2+uTime*2.8));
float flow2=noise(vec2(vUv.x*16.0+2.7,vUv.y*7.2+uTime*4.4));
float filaments=smoothstep(0.20,0.84,flow1*0.56+flow2*0.44);
float xDist=abs(vUv.x-0.5)*2.0;
float edgeMask=0.72+0.12*sin(vUv.y*10.0+uTime*2.8)+0.12*(flow1-0.5);
float side=1.0-smoothstep(edgeMask-0.34,edgeMask,xDist);
float lipThick=smoothstep(0.66,0.92,vUv.y)*0.58;
float vert=smoothstep(0.0,0.12,vUv.y)*smoothstep(1.0,0.90,vUv.y);
float dist=distance(cameraPosition,vWorldPos);
float distFade=1.0-smoothstep(uTerrainSize*0.18,uTerrainSize*0.50,dist);
float sprayBase=smoothstep(0.42,0.0,vUv.y)*0.36;
float alpha=clamp((0.30+0.62*filaments+lipThick+sprayBase)*side*vert*distFade,0.0,0.95);
if(alpha<0.015)discard;
vec3 waterCol=mix(vec3(0.42,0.70,0.84),vec3(0.96,0.99,1.0),clamp(filaments*0.76+lipThick*0.7+sprayBase*1.3,0.0,1.0));
vec3 col=mix(waterCol,uFogColor,smoothstep(uTerrainSize*0.5,uTerrainSize*2.2,dist));
gl_FragColor=vec4(col,alpha);
}
`;

const FOAM_VERTEX = /* glsl */ `
varying vec2 vLocalXZ;
varying vec3 vWorldPos;
void main(){
vLocalXZ=position.xz;
vec4 world=modelMatrix*instanceMatrix*vec4(position,1.0);
vWorldPos=world.xyz;
gl_Position=projectionMatrix*viewMatrix*world;
}
`;

const FOAM_FRAGMENT = /* glsl */ `
precision highp float;
varying vec2 vLocalXZ;
varying vec3 vWorldPos;
uniform float uTime;
uniform float uTerrainSize;
uniform vec3 uFogColor;
${WATERFALL_NOISE_GLSL}
void main(){
float r=length(vLocalXZ);
float n=noise(vLocalXZ*3.6+vec2(uTime*0.45,-uTime*0.35));
float ripple=0.5+0.5*sin(r*14.0-uTime*4.6+n*4.2);
float outerFade=1.0-smoothstep(0.18,0.82+(n-0.5)*0.12,r);
float core=1.0-smoothstep(0.0,0.58,r);
vec3 viewDir=normalize(cameraPosition-vWorldPos);
float grazing=smoothstep(0.04,0.22,abs(viewDir.y));
float dist=distance(cameraPosition,vWorldPos);
float distFade=1.0-smoothstep(uTerrainSize*0.18,uTerrainSize*0.48,dist);
float foam=clamp((core*0.48+ripple*0.34)*outerFade,0.0,1.0);
float alpha=clamp(foam*outerFade*grazing*distFade,0.0,0.72);
if(alpha<0.015)discard;
vec3 poolCol=vec3(0.04,0.18,0.25);
vec3 foamCol=vec3(0.94,0.98,1.0);
vec3 col=mix(poolCol,foamCol,clamp(core*0.85+ripple*0.48,0.0,1.0));
col=mix(col,uFogColor,smoothstep(uTerrainSize*0.5,uTerrainSize*2.2,dist));
gl_FragColor=vec4(col,alpha);
}
`;

const MIST_VERTEX = /* glsl */ `
varying vec2 vUv;
varying vec3 vWorldPos;
void main(){
vUv=uv;
vec4 world=modelMatrix*instanceMatrix*vec4(position,1.0);
vWorldPos=world.xyz;
gl_Position=projectionMatrix*viewMatrix*world;
}
`;

const MIST_FRAGMENT = /* glsl */ `
precision highp float;
varying vec2 vUv;
varying vec3 vWorldPos;
uniform float uTime;
uniform float uTerrainSize;
uniform vec3 uFogColor;
${WATERFALL_NOISE_GLSL}
void main(){
vec2 p=vUv-vec2(0.5,0.38);
float r=length(p*vec2(1.15,1.45));
float rise=noise(vec2(vUv.x*3.4,vUv.y*2.6-uTime*0.55));
float wisp=noise(vec2(vUv.x*6.2+1.7,vUv.y*4.5-uTime*0.85));
float puff=(1.0-smoothstep(0.10,0.52+(rise-0.5)*0.16,r))*smoothstep(0.06,0.34,vUv.y);
float dist=distance(cameraPosition,vWorldPos);
float distFade=1.0-smoothstep(uTerrainSize*0.12,uTerrainSize*0.36,dist);
float alpha=clamp(puff*(0.42+0.58*(rise*0.6+wisp*0.4))*0.34*distFade,0.0,0.36);
if(alpha<0.012)discard;
vec3 col=mix(vec3(0.86,0.93,0.97),uFogColor,smoothstep(uTerrainSize*0.4,uTerrainSize*1.8,dist));
gl_FragColor=vec4(col,alpha);
}
`;

export const WorldWaterfalls: React.FC<WorldWaterfallsProps> = ({
  world,
  worldV2 = null,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution,
  tier = 'high',
}) => {
  const curtainRef = useRef<THREE.InstancedMesh>(null);
  const foamRef = useRef<THREE.InstancedMesh>(null);
  const mistRef = useRef<THREE.InstancedMesh>(null);
  const showMist = tier !== 'low';

  const count = world.waterfallCount ?? 0;
  const heightUnits = renderSize * heightRatio;

  const curtainGeom = useMemo(() => {
    const geom = new THREE.PlaneGeometry(1, 1, 6, 10);
    const pos = geom.getAttribute('position');
    for (let i = 0; i < pos.count; i++) {
      const x = pos.getX(i);
      const v = pos.getY(i) + 0.5; // 0 = plunge base, 1 = cliff lip
      const widthTaper = 1.12 - v * 0.48;
      // Top lip pulls back cleanly onto the upper river channel; mid-drop arches forward.
      const lipBack = v > 0.82 ? -Math.pow((v - 0.82) / 0.18, 1.5) * 0.28 : 0;
      const arcOut = Math.sin(Math.min(1, v * 1.12) * Math.PI) * 0.44 + lipBack;
      pos.setX(i, x * widthTaper);
      pos.setZ(i, arcOut);
    }
    pos.needsUpdate = true;
    geom.computeVertexNormals();
    return geom;
  }, []);

  const foamGeom = useMemo(() => new THREE.CircleGeometry(1, 20).rotateX(-Math.PI / 2), []);

  const mistGeom = useMemo(() => new THREE.PlaneGeometry(1, 1, 1, 1), []);

  const curtainMat = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: CURTAIN_VERTEX,
        fragmentShader: CURTAIN_FRAGMENT,
        uniforms: {
          uTime: { value: 0 },
          uTerrainSize: { value: renderSize },
          uFogColor: { value: new THREE.Color('#c4dfef') },
        },
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
        fog: false,
      }),
    [renderSize],
  );

  const foamMat = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: FOAM_VERTEX,
        fragmentShader: FOAM_FRAGMENT,
        uniforms: {
          uTime: { value: 0 },
          uTerrainSize: { value: renderSize },
          uFogColor: { value: new THREE.Color('#c4dfef') },
        },
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
        fog: false,
      }),
    [renderSize],
  );

  const mistMat = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: MIST_VERTEX,
        fragmentShader: MIST_FRAGMENT,
        uniforms: {
          uTime: { value: 0 },
          uTerrainSize: { value: renderSize },
          uFogColor: { value: new THREE.Color('#c4dfef') },
        },
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
        fog: false,
      }),
    [renderSize],
  );

  const nearVerRef = useRef(-1);

  const placeWaterfalls = React.useCallback(() => {
    const curtain = curtainRef.current;
    const foam = foamRef.current;
    const mist = mistRef.current;
    if (!curtain || !foam || count === 0) return;
    const dummy = new THREE.Object3D();
    const { size } = world;
    const toWorld = renderSize / size;
    let keptCount = 0;
    for (let i = 0; i < count; i++) {
      const x = world.waterfallX[i] * toWorld;
      const z = world.waterfallZ[i] * toWorld;
      const yaw = world.waterfallYaw[i];
      const dirX = Math.cos(yaw);
      const dirZ = Math.sin(yaw);
      let dropY = Math.max(1.2, world.waterfallDrop[i] * heightUnits);
      let topY = world.waterfallTopE[i] * heightUnits;
      let pitchRad = 0;
      let spanOffset = 1.0;

      if (worldV2 && meshResolution) {
        const fit = evaluateDrawnWaterfall(
          world,
          worldV2,
          meshResolution,
          renderSize,
          heightUnits,
          i,
          sampleActiveNearPatchY,
        );
        if (!fit.keep) {
          dummy.position.set(0, -1e6, 0);
          dummy.rotation.set(0, 0, 0);
          dummy.scale.set(0, 0, 0);
          dummy.updateMatrix();
          if (typeof curtain.setMatrixAt === 'function') curtain.setMatrixAt(i, dummy.matrix);
          if (typeof foam.setMatrixAt === 'function') foam.setMatrixAt(i, dummy.matrix);
          if (mist && typeof mist.setMatrixAt === 'function') {
            mist.setMatrixAt(i * 2, dummy.matrix);
            mist.setMatrixAt(i * 2 + 1, dummy.matrix);
          }
          continue;
        }
        topY = fit.topY;
        dropY = fit.dropY;
        pitchRad = fit.pitchRad;
        spanOffset = 1.32;
      }
      keptCount += 1;

      const wfDischarge = worldV2?.sim.waterfalls[i]?.discharge_q_m3_s ?? 0;
      const qBoost = wfDischarge > 0 ? Math.min(1.5, 1.0 + Math.log2(1 + wfDischarge) * 0.08) : 1.0;
      const width = Math.min(4.6, (1.15 + dropY * 0.28) * qBoost);

      dummy.position.set(x + dirX * spanOffset, topY - dropY / 2, z + dirZ * spanOffset);
      dummy.rotation.order = 'YXZ';
      dummy.rotation.set(-pitchRad, Math.PI / 2 - yaw, 0);
      dummy.scale.set(width, dropY + 0.7, 1);
      dummy.updateMatrix();
      if (typeof curtain.setMatrixAt === 'function') curtain.setMatrixAt(i, dummy.matrix);

      const poolPush = spanOffset + Math.sin(pitchRad) * (dropY * 0.45) + width * 0.14;
      const poolY = topY - dropY + 0.14;
      dummy.position.set(x + dirX * poolPush, poolY, z + dirZ * poolPush);
      dummy.rotation.set(0, 0, 0);
      dummy.scale.set(width * 0.54, 1, width * 0.44);
      dummy.updateMatrix();
      if (typeof foam.setMatrixAt === 'function') foam.setMatrixAt(i, dummy.matrix);

      if (mist && typeof mist.setMatrixAt === 'function') {
        const mistH = Math.max(0.8, dropY * 0.52);
        const mistW = width * 1.45;
        dummy.position.set(x + dirX * (poolPush + 0.25), poolY + mistH * 0.54, z + dirZ * (poolPush + 0.25));
        dummy.rotation.order = 'YXZ';
        dummy.rotation.set(0, Math.PI / 2 - yaw, 0);
        dummy.scale.set(mistW, mistH, 1);
        dummy.updateMatrix();
        mist.setMatrixAt(i * 2, dummy.matrix);

        dummy.rotation.set(0, Math.PI - yaw, 0);
        dummy.updateMatrix();
        mist.setMatrixAt(i * 2 + 1, dummy.matrix);
      }
    }
    if (curtain.userData) {
      curtain.userData.keptCount = keptCount;
      curtain.userData.totalCount = count;
    }
    if (curtain.instanceMatrix) curtain.instanceMatrix.needsUpdate = true;
    if (foam.instanceMatrix) foam.instanceMatrix.needsUpdate = true;
    if (mist && mist.instanceMatrix) mist.instanceMatrix.needsUpdate = true;
  }, [world, worldV2, meshResolution, count, renderSize, heightUnits]);

  useFrame((state) => {
    driveTimeUniform(curtainMat, state.clock);
    driveTimeUniform(foamMat, state.clock);
    if (showMist) driveTimeUniform(mistMat, state.clock);
    const fog = state.scene.fog as THREE.Fog | null;
    if (fog && fog.color) {
      (curtainMat.uniforms.uFogColor.value as THREE.Color).copy(fog.color);
      (foamMat.uniforms.uFogColor.value as THREE.Color).copy(fog.color);
      if (showMist) (mistMat.uniforms.uFogColor.value as THREE.Color).copy(fog.color);
    }
    if (worldV2 && tier !== 'low') {
      const ver = getActiveNearPatchVersion();
      if (ver !== nearVerRef.current) {
        nearVerRef.current = ver;
        placeWaterfalls();
      }
    }
  });

  useLayoutEffect(() => {
    placeWaterfalls();
    if (worldV2 && tier !== 'low') {
      nearVerRef.current = getActiveNearPatchVersion();
    }
  }, [placeWaterfalls, worldV2, tier, showMist]);

  useEffect(() => {
    return () => {
      curtainGeom.dispose();
      foamGeom.dispose();
      mistGeom.dispose();
      curtainMat.dispose();
      foamMat.dispose();
      mistMat.dispose();
    };
  }, [curtainGeom, foamGeom, mistGeom, curtainMat, foamMat, mistMat]);

  if (count === 0) return null;
  return (
    <group name="world-waterfalls">
      <instancedMesh
        ref={curtainRef}
        args={[curtainGeom, curtainMat, count]}
        name="waterfall-curtains"
      />
      <instancedMesh ref={foamRef} args={[foamGeom, foamMat, count]} name="waterfall-foam" />
      {showMist ? (
        <instancedMesh ref={mistRef} args={[mistGeom, mistMat, count * 2]} name="waterfall-mist" />
      ) : null}
    </group>
  );
};

export default WorldWaterfalls;
