import React, { useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { driveTimeUniform } from './utils/sceneClock';
import type { WorldV2LoadedBundle } from './worldV2/types';
import { evaluateDrawnWaterfall } from './worldV2/waterfallFit';

// ---------------------------------------------------------------------------------------
// WorldWaterfalls — foam curtains hung over the steep river drops the generator detected
// (World.waterfall*). Two instanced meshes total: the curtains and the splash pools at
// their feet, so even hundreds of falls cost two draw calls. The curtain shader scrolls
// bright streaks downward so the water visibly FALLS instead of hanging as a static sheet.
// ---------------------------------------------------------------------------------------

export interface WorldWaterfallsProps {
  world: World;
  worldV2?: WorldV2LoadedBundle | null;
  renderSize?: number;
  heightRatio?: number;
  /**
   * WV2-LOOK r8 (World v2 only): the terrain mesh resolution. The mesh samples elevation every
   * ~100-160 m, so a gorge narrower than that is drawn filled in by its walls, and a curtain placed
   * at the data's lip height hung inside the hill (51 of 58 on the 81.92 km world). With this set,
   * each fall keeps its drop but is lifted until its base sits on the drawn surface.
   */
  meshResolution?: number;
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
float flow1=noise(vec2(vUv.x*7.5,vUv.y*3.8+uTime*2.4));
float flow2=noise(vec2(vUv.x*14.0+2.7,vUv.y*6.5+uTime*3.9));
float filaments=smoothstep(0.24,0.86,flow1*0.58+flow2*0.42);
float xDist=abs(vUv.x-0.5)*2.0;
float edgeMask=0.68+0.14*sin(vUv.y*10.0+uTime*2.8)+0.14*(flow1-0.5);
float side=1.0-smoothstep(edgeMask-0.36,edgeMask,xDist);
float vert=smoothstep(0.0,0.18,vUv.y)*smoothstep(1.0,0.82,vUv.y);
float dist=distance(cameraPosition,vWorldPos);
float distFade=1.0-smoothstep(uTerrainSize*0.16,uTerrainSize*0.46,dist);
float sprayBase=smoothstep(0.45,0.0,vUv.y)*0.30;
float alpha=clamp((0.26+0.68*filaments+sprayBase)*side*vert*distFade,0.0,0.92);
if(alpha<0.015)discard;
vec3 col=mix(vec3(0.56,0.78,0.90),vec3(0.95,0.98,1.0),clamp(filaments*0.8+sprayBase*1.4,0.0,1.0));
col=mix(col,uFogColor,smoothstep(uTerrainSize*0.5,uTerrainSize*2.2,dist));
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
float n=noise(vLocalXZ*3.2+vec2(uTime*0.4,-uTime*0.3));
float ripple=0.5+0.5*sin(r*13.0-uTime*4.2+n*4.2);
float outerFade=1.0-smoothstep(0.18,0.78+(n-0.5)*0.14,r);
float core=1.0-smoothstep(0.0,0.48,r);
vec3 viewDir=normalize(cameraPosition-vWorldPos);
float grazing=smoothstep(0.06,0.34,abs(viewDir.y));
float dist=distance(cameraPosition,vWorldPos);
float distFade=1.0-smoothstep(uTerrainSize*0.16,uTerrainSize*0.44,dist);
float alpha=clamp((core*0.48+ripple*0.30)*outerFade*grazing*distFade,0.0,0.68);
if(alpha<0.015)discard;
vec3 col=mix(vec3(0.76,0.90,0.96),vec3(0.97,0.99,1.0),clamp(core+ripple*0.35,0.0,1.0));
col=mix(col,uFogColor,smoothstep(uTerrainSize*0.5,uTerrainSize*2.2,dist));
gl_FragColor=vec4(col,alpha);
}
`;

export const WorldWaterfalls: React.FC<WorldWaterfallsProps> = ({
  world,
  worldV2 = null,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution,
}) => {
  const curtainRef = useRef<THREE.InstancedMesh>(null);
  const foamRef = useRef<THREE.InstancedMesh>(null);

  const count = world.waterfallCount ?? 0;
  const heightUnits = renderSize * heightRatio;

  const curtainGeom = useMemo(() => {
    const geom = new THREE.PlaneGeometry(1, 1, 4, 8);
    const pos = geom.getAttribute('position');
    for (let i = 0; i < pos.count; i++) {
      const x = pos.getX(i);
      const v = pos.getY(i) + 0.5; // 0 = base, 1 = cliff lip
      const widthTaper = 1.04 - v * 0.42;
      const arcOut = Math.sin(v * Math.PI) * 0.26;
      pos.setX(i, x * widthTaper);
      pos.setZ(i, arcOut);
    }
    pos.needsUpdate = true;
    geom.computeVertexNormals();
    return geom;
  }, []);

  const foamGeom = useMemo(() => new THREE.CircleGeometry(1, 16).rotateX(-Math.PI / 2), []);

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

  useFrame((state) => {
    driveTimeUniform(curtainMat, state.clock);
    driveTimeUniform(foamMat, state.clock);
    const fog = state.scene.fog as THREE.Fog | null;
    if (fog && fog.color) {
      (curtainMat.uniforms.uFogColor.value as THREE.Color).copy(fog.color);
      (foamMat.uniforms.uFogColor.value as THREE.Color).copy(fog.color);
    }
  });

  useLayoutEffect(() => {
    const curtain = curtainRef.current;
    const foam = foamRef.current;
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
        const fit = evaluateDrawnWaterfall(world, worldV2, meshResolution, renderSize, heightUnits, i);
        if (!fit.keep) {
          dummy.position.set(0, -1e6, 0);
          dummy.rotation.set(0, 0, 0);
          dummy.scale.set(0, 0, 0);
          dummy.updateMatrix();
          if (typeof curtain.setMatrixAt === 'function') curtain.setMatrixAt(i, dummy.matrix);
          if (typeof foam.setMatrixAt === 'function') foam.setMatrixAt(i, dummy.matrix);
          continue;
        }
        topY = fit.topY;
        dropY = fit.dropY;
        pitchRad = fit.pitchRad;
        spanOffset = 1.25;
      }
      keptCount += 1;

      const wfDischarge = worldV2?.sim.waterfalls[i]?.discharge_q_m3_s ?? 0;
      const qBoost = wfDischarge > 0 ? Math.min(1.6, 1.0 + Math.log2(1 + wfDischarge) * 0.08) : 1.0;
      const width = Math.min(6.0, (1.8 + dropY * 0.42) * qBoost);

      // Curtain: hangs from the lip down along the drawn rock face to the splash pool.
      // Negative X pitch tilts the base (+downstream) outward so the sheet never knives into the cliff.
      dummy.position.set(x + dirX * spanOffset, topY - dropY / 2, z + dirZ * spanOffset);
      dummy.rotation.order = 'YXZ';
      dummy.rotation.set(-pitchRad, Math.PI / 2 - yaw, 0);
      dummy.scale.set(width, dropY + 0.5, 1);
      dummy.updateMatrix();
      if (typeof curtain.setMatrixAt === 'function') curtain.setMatrixAt(i, dummy.matrix);

      // Splash pool at the base — seated cleanly at the drawn lower step.
      const poolPush = spanOffset + Math.sin(pitchRad) * (dropY * 0.48) + width * 0.16;
      dummy.position.set(x + dirX * poolPush, topY - dropY + 0.18, z + dirZ * poolPush);
      dummy.rotation.set(0, 0, 0);
      dummy.scale.set(width * 0.44, 1, width * 0.36);
      dummy.updateMatrix();
      if (typeof foam.setMatrixAt === 'function') foam.setMatrixAt(i, dummy.matrix);
    }
    curtain.userData.keptCount = keptCount;
    curtain.userData.totalCount = count;
    if (curtain.instanceMatrix) curtain.instanceMatrix.needsUpdate = true;
    if (foam.instanceMatrix) foam.instanceMatrix.needsUpdate = true;
  }, [world, worldV2, count, renderSize, heightUnits, meshResolution]);

  useEffect(() => {
    return () => {
      curtainGeom.dispose();
      foamGeom.dispose();
      curtainMat.dispose();
      foamMat.dispose();
    };
  }, [curtainGeom, foamGeom, curtainMat, foamMat]);

  if (count === 0) return null;
  // Both materials go through `args`, not a `material=` prop or a JSX child. `InstancedMesh`'s
  // constructor takes (geometry, material, count) with none of them optional, so the old cast on an
  // `undefined` material was not a loose type on a value that could be absent — it was a lie about
  // a slot three requires. Passing the real material there is shorter *and* honest, and it gave the
  // foam a material this component owns and therefore disposes.
  return (
    <group name="world-waterfalls">
      <instancedMesh
        ref={curtainRef}
        args={[curtainGeom, curtainMat, count]}
        name="waterfall-curtains"
      />
      <instancedMesh ref={foamRef} args={[foamGeom, foamMat, count]} name="waterfall-foam" />
    </group>
  );
};

export default WorldWaterfalls;
