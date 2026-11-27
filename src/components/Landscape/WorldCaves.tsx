import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import type { World } from './utils/worldGen';
import { buildCavesGeometry } from './utils/caveGeometry';
import { sampleMeshHeight } from './utils/worldSample';
import { ELEV_MIN_M, ELEV_SPAN_M, type WorldV2LoadedBundle } from './worldV2/types';
import {
  ATMOSPHERE_FRAGMENT_PARS,
  atmosphereFragmentApply,
  bindAtmosphereUniforms,
} from './materials/atmosphere';

// prototype, to be replaced — WV2-LOOK cave dressing (World v2 only).
// Mouths at ~1/3 of the v1 size (17-25 m openings on the 41 km map), sunk into the slope so the
// outer tube is buried, rim tinted to the cliff rock, the black kept to the throat and lifted
// toward rock with distance (a 20 m opening is a dark speck at 5 km, not a black cube), plus the
// shared aerial perspective. The scene graph (meshes, names, counts) is unchanged.
const V2_CAVE_SCALE = 0.34;
const V2_CAVE_SINK = 0.55;

function makeV2CaveMaterial(color: string, vertexColors: boolean): THREE.MeshStandardMaterial {
  const material = new THREE.MeshStandardMaterial({
    color,
    vertexColors,
    roughness: 0.95,
    metalness: 0,
    side: THREE.DoubleSide,
  });
  material.onBeforeCompile = (shader) => {
    bindAtmosphereUniforms(shader.uniforms);
    shader.vertexShader =
      'varying vec3 vLkCaveWp;\n' +
      shader.vertexShader.replace(
        '#include <worldpos_vertex>',
        `
#include <worldpos_vertex>
vLkCaveWp=(modelMatrix*vec4(transformed,1.0)).xyz;
`,
      );
    shader.fragmentShader =
      'varying vec3 vLkCaveWp;\n' +
      ATMOSPHERE_FRAGMENT_PARS +
      shader.fragmentShader
        .replace(
          '#include <color_fragment>',
          `
#include <color_fragment>
{
float lkFar=smoothstep(60.0,420.0,length(vViewPosition));
diffuseColor.rgb=mix(diffuseColor.rgb,vec3(0.085,0.08,0.072),lkFar*0.75);
}
`,
        )
        .replace('#include <fog_fragment>', atmosphereFragmentApply('vLkCaveWp'));
  };
  material.customProgramCacheKey = () => `wv2-look-cave-${vertexColors ? 'v' : 'c'}`;
  return material;
}

export interface WorldCavesProps {
  world: World;
  worldV2?: WorldV2LoadedBundle | null;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
}

function buildCaveNetworkGeometry(
  worldV2: WorldV2LoadedBundle | null | undefined,
  world: World,
  renderSize: number,
  heightUnits: number,
  meshResolution: number,
): THREE.BufferGeometry | null {
  const net = worldV2?.caveNetwork;
  if (!net || net.edges.length === 0 || net.nodes.length === 0) return null;
  const extentM = Math.max(1, worldV2.extentM);
  const parts: THREE.BufferGeometry[] = [];
  const up = new THREE.Vector3(0, 1, 0);

  for (const edge of net.edges) {
    const a = net.nodes[edge.from_idx];
    const b = net.nodes[edge.to_idx];
    if (!a || !b) continue;
    const u0 = Math.min(1, Math.max(0, a.x_m / extentM));
    const v0 = Math.min(1, Math.max(0, a.z_m / extentM));
    const u1 = Math.min(1, Math.max(0, b.x_m / extentM));
    const v1 = Math.min(1, Math.max(0, b.z_m / extentM));
    const g0 = sampleMeshHeight(world, u0, v0, meshResolution) * heightUnits;
    const g1 = sampleMeshHeight(world, u1, v1, meshResolution) * heightUnits;
    const gMid = sampleMeshHeight(world, (u0 + u1) * 0.5, (v0 + v1) * 0.5, meshResolution) * heightUnits;

    const p0 = new THREE.Vector3(
      (u0 - 0.5) * renderSize,
      ((a.y_m - ELEV_MIN_M) / ELEV_SPAN_M) * heightUnits,
      (v0 - 0.5) * renderSize,
    );
    const p1 = new THREE.Vector3(
      (u1 - 0.5) * renderSize,
      ((b.y_m - ELEV_MIN_M) / ELEV_SPAN_M) * heightUnits,
      (v1 - 0.5) * renderSize,
    );
    const delta = new THREE.Vector3().subVectors(p1, p0);
    const len = delta.length();
    if (len < 0.05) continue;
    // WV2-LOOK: the 0.3-unit floor made every tunnel >= 10 m thick; exposed stretches poked out
    // of the slopes as dark tubes. Use the data radius with a small floor and cull any segment
    // that rises above the drawn 512^2 terrain mesh.
    const r0 = Math.max(0.06, (a.radius_m / extentM) * renderSize);
    const r1 = Math.max(0.06, (b.radius_m / extentM) * renderSize);
    const midY = (p0.y + p1.y) * 0.5;
    const rMid = (r0 + r1) * 0.5;
    // Culled 1.4 units (~48 m) below the uncarved terrain mesh so carved riverbeds
    // and waterfall drops (<= 38 m) never expose underground cave pipes.
    if (p0.y + r0 > g0 - 1.4 || p1.y + r1 > g1 - 1.4 || midY + rMid > gMid - 1.4) {
      continue;
    }
    const seg = new THREE.CylinderGeometry(r1, r0, len, 6, 1, true);
    const mid = new THREE.Vector3().addVectors(p0, p1).multiplyScalar(0.5);
    const quat = new THREE.Quaternion().setFromUnitVectors(up, delta.normalize());
    seg.applyQuaternion(quat);
    seg.translate(mid.x, mid.y, mid.z);
    parts.push(seg);
  }
  if (parts.length === 0) return null;
  const merged = mergeGeometries(parts, false);
  for (const p of parts) p.dispose();
  return merged;
}

export const WorldCaves: React.FC<WorldCavesProps> = ({
  world,
  worldV2 = null,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 384,
}) => {
  const heightUnits = renderSize * heightRatio;

  const isV2 = Boolean(worldV2);
  const geom = useMemo(
    () =>
      buildCavesGeometry(
        world,
        renderSize,
        heightUnits,
        meshResolution,
        isV2 ? { scale: V2_CAVE_SCALE, sink: V2_CAVE_SINK, seatOnDrawnSlope: true } : {},
      ),
    [world, renderSize, heightUnits, meshResolution, isV2],
  );
  const v2Materials = useMemo(
    () =>
      isV2
        ? { mouth: makeV2CaveMaterial('#c4bcb2', true), tunnel: makeV2CaveMaterial('#2b2824', false) }
        : null,
    [isV2],
  );
  useEffect(() => {
    return () => {
      v2Materials?.mouth.dispose();
      v2Materials?.tunnel.dispose();
    };
  }, [v2Materials]);

  const networkGeom = useMemo(
    () => buildCaveNetworkGeometry(worldV2, world, renderSize, heightUnits, meshResolution),
    [worldV2, world, renderSize, heightUnits, meshResolution],
  );

  // Dispose geometries when the world changes or component unmounts.
  useEffect(() => {
    return () => {
      if (geom && typeof geom.dispose === 'function') geom.dispose();
      if (networkGeom && typeof networkGeom.dispose === 'function') networkGeom.dispose();
    };
  }, [geom, networkGeom]);

  const hasCaveSource = isV2 && (world.caveCount > 0 || (worldV2?.caveNetwork?.edges.length ?? 0) > 0);
  if (!geom && !networkGeom && !hasCaveSource) return null;
  return (
    <group name="world-caves-group" userData={{ decorative: true }}>
      {geom && v2Materials ? (
        <mesh
          geometry={geom}
          material={v2Materials.mouth}
          name="world-caves"
          receiveShadow
          frustumCulled={false}
        />
      ) : null}
      {geom && !v2Materials ? (
        <mesh geometry={geom} name="world-caves" receiveShadow frustumCulled={false}>
          <meshStandardMaterial
            vertexColors
            roughness={0.94}
            metalness={0}
            flatShading
            side={THREE.DoubleSide}
          />
        </mesh>
      ) : null}
      {networkGeom && v2Materials ? (
        <mesh
          geometry={networkGeom}
          material={v2Materials.tunnel}
          name="world-cave-network"
          frustumCulled={false}
        />
      ) : null}
      {networkGeom && !v2Materials ? (
        <mesh geometry={networkGeom} name="world-cave-network" frustumCulled={false}>
          <meshStandardMaterial
            color="#0b0c10"
            roughness={0.96}
            metalness={0}
            side={THREE.DoubleSide}
          />
        </mesh>
      ) : null}
    </group>
  );
};

export default WorldCaves;
