import React, { useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { sampleMap2Terrain } from './utils/map2TerrainGen';
import { Map2Biome } from './utils/map2BiomeConfig';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';

// -------------------------------------------------------------------------------------------------
// Map 2 Fantasy Crystals & Bioluminescent Cave Architecture
// -------------------------------------------------------------------------------------------------

export interface Map2FantasyCrystalsProps {
  size?: number;
  waterLevel?: number;
  pulseSpeed?: number;
  showPointLights?: boolean;
}

interface CrystalPlacement {
  x: number;
  y: number;
  z: number;
  scale: number;
  rotX: number;
  rotY: number;
  rotZ: number;
  color: THREE.Color;
}

function hash2D(x: number, z: number, seed = 0): number {
  let h = Math.imul((x * 374761393) | 0, (z * 668265263) | 0) + Math.imul(seed, 1274126177);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

function paint(geom: THREE.BufferGeometry, hex: string): THREE.BufferGeometry {
  const c = new THREE.Color(hex);
  const count = geom.attributes.position.count;
  const colors = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    colors[i * 3] = c.r;
    colors[i * 3 + 1] = c.g;
    colors[i * 3 + 2] = c.b;
  }
  geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  if (!geom.attributes.normal) geom.computeVertexNormals();
  return geom;
}

/**
 * Creates faceted tall hexagonal crystal spire geometry.
 */
function makeCrystalSpireGeometry(): THREE.BufferGeometry {
  const parts: THREE.BufferGeometry[] = [];
  // Central hexagonal prism
  parts.push(paint(new THREE.CylinderGeometry(0.04, 0.22, 1.8, 6).translate(0, 0.9, 0), '#a855f7'));
  // Pointed pyramid tip
  parts.push(paint(new THREE.ConeGeometry(0.22, 0.5, 6).translate(0, 1.95, 0), '#c084fc'));
  // Base shards
  parts.push(
    paint(new THREE.CylinderGeometry(0.02, 0.12, 0.9, 5).rotateZ(0.3).translate(0.24, 0.45, 0.1), '#38bdf8'),
  );
  parts.push(
    paint(new THREE.CylinderGeometry(0.02, 0.1, 0.75, 5).rotateZ(-0.35).translate(-0.2, 0.38, -0.1), '#ec4899'),
  );

  const merged = mergeGeometries(parts, false) ?? parts[0];
  parts.forEach((p) => {
    if (p !== merged) p.dispose();
  });
  merged.computeVertexNormals();
  return merged;
}

/**
 * Creates faceted stalactite / stalagmite cluster geometry.
 */
function makeStalactiteGeometry(): THREE.BufferGeometry {
  const parts: THREE.BufferGeometry[] = [];
  parts.push(paint(new THREE.ConeGeometry(0.28, 2.2, 7).rotateX(Math.PI).translate(0, 1.1, 0), '#818cf8'));
  parts.push(
    paint(
      new THREE.ConeGeometry(0.16, 1.4, 6).rotateX(Math.PI).rotateZ(0.2).translate(0.18, 0.9, 0.08),
      '#06b6d4',
    ),
  );
  parts.push(
    paint(
      new THREE.ConeGeometry(0.14, 1.2, 5).rotateX(Math.PI).rotateZ(-0.25).translate(-0.16, 0.8, -0.06),
      '#a855f7',
    ),
  );

  const merged = mergeGeometries(parts, false) ?? parts[0];
  parts.forEach((p) => {
    if (p !== merged) p.dispose();
  });
  merged.computeVertexNormals();
  return merged;
}

/**
 * Creates a pulsating emissive crystal material with chromatic breathing shifts.
 */
function createPulsingCrystalMaterial(
  uniforms: {
    uTime: { value: number };
    uGlowColor1: { value: THREE.Color };
    uGlowColor2: { value: THREE.Color };
    uPulseSpeed: { value: number };
  },
): THREE.MeshStandardMaterial {
  const mat = new THREE.MeshStandardMaterial({
    vertexColors: true,
    roughness: 0.25,
    metalness: 0.75,
    emissive: new THREE.Color('#3b0764'),
    emissiveIntensity: 0.8,
    flatShading: true,
  });

  mat.onBeforeCompile = (shader) => {
    shader.uniforms.uTime = uniforms.uTime;
    shader.uniforms.uGlowColor1 = uniforms.uGlowColor1;
    shader.uniforms.uGlowColor2 = uniforms.uGlowColor2;
    shader.uniforms.uPulseSpeed = uniforms.uPulseSpeed;

    shader.vertexShader = `
      varying vec3 vCrystalWorldPos;
      ${shader.vertexShader}
    `.replace(
      '#include <worldpos_vertex>',
      `#include <worldpos_vertex>
      vCrystalWorldPos = (modelMatrix * vec4(transformed, 1.0)).xyz;
      `,
    );

    shader.fragmentShader = `
      uniform float uTime;
      uniform vec3 uGlowColor1;
      uniform vec3 uGlowColor2;
      uniform float uPulseSpeed;
      varying vec3 vCrystalWorldPos;
      ${shader.fragmentShader}
    `.replace(
      '#include <emissivemap_fragment>',
      `#include <emissivemap_fragment>
      float pulse = 0.65 + 0.35 * sin(uTime * uPulseSpeed * 2.0 + vCrystalWorldPos.x * 0.3 + vCrystalWorldPos.z * 0.3);
      vec3 glowColor = mix(uGlowColor1, uGlowColor2, 0.5 + 0.5 * sin(uTime * uPulseSpeed * 1.1 + vCrystalWorldPos.y * 0.25));
      totalEmissiveRadiance += glowColor * pulse * 1.8;
      `,
    );
  };

  return mat;
}

export const Map2FantasyCrystals: React.FC<Map2FantasyCrystalsProps> = ({
  size = 200,
  waterLevel = 12.0,
  pulseSpeed = 1.0,
  showPointLights = true,
}) => {
  const spireMeshRef = useRef<THREE.InstancedMesh>(null);
  const stalactiteMeshRef = useRef<THREE.InstancedMesh>(null);
  const pointLightGroupRef = useRef<THREE.Group>(null);

  // Shared crystal uniforms created once via ref
  const crystalUniformsRef = useRef({
    uTime: { value: 0 },
    uGlowColor1: { value: new THREE.Color('#a855f7') }, // Amethyst
    uGlowColor2: { value: new THREE.Color('#00f0ff') }, // Electric Cyan
    uPulseSpeed: { value: pulseSpeed },
  });

  // Geometries, materials, and uniforms created once
  const { spireGeom, stalactiteGeom, crystalMat } = useMemo(() => {
    const sGeom = makeCrystalSpireGeometry();
    const stGeom = makeStalactiteGeometry();
    const cMat = createPulsingCrystalMaterial(crystalUniformsRef.current);
    return { spireGeom: sGeom, stalactiteGeom: stGeom, crystalMat: cMat };
  }, []);

  useEffect(() => {
    crystalUniformsRef.current.uPulseSpeed.value = pulseSpeed;
  }, [pulseSpeed]);

  useEffect(() => {
    return () => {
      spireGeom.dispose();
      stalactiteGeom.dispose();
      crystalMat.dispose();
    };
  }, [spireGeom, stalactiteGeom, crystalMat]);

  // Generate crystal placements in cave zones, mystical ravines, and subterranean grottoes
  const { spires, stalactites, lightPositions } = useMemo(() => {
    const spiresList: CrystalPlacement[] = [];
    const stalactitesList: CrystalPlacement[] = [];
    const lights: Array<{ position: THREE.Vector3; color: string }> = [];

    const halfSize = size * 0.45;
    const gridRes = 70;
    const step = (halfSize * 2) / gridRes;
    const dummyColor = new THREE.Color();

    for (let gx = 0; gx < gridRes; gx++) {
      for (let gz = 0; gz < gridRes; gz++) {
        const jx = (hash2D(gx, gz, 11) - 0.5) * step * 0.9;
        const jz = (hash2D(gx, gz, 12) - 0.5) * step * 0.9;
        const wx = -halfSize + (gx + 0.5) * step + jx;
        const wz = -halfSize + (gz + 0.5) * step + jz;

        // Skip spawn clearance
        if (Math.sqrt(wx * wx + wz * wz) < 12.0) continue;

        const sample = sampleMap2Terrain(wx, wz);
        const { height, biomeId, caveIntensity } = sample;

        const isCaveBiome = biomeId === Map2Biome.BIOLUMINESCENT_CAVES;
        const isGrotto = Math.hypot(wx - (-60), wz - (-60)) < 16.0;
        const isSubterranean = caveIntensity > 0.75 && height < 22.0;

        if (!isCaveBiome && !isGrotto && !isSubterranean) continue;

        const rType = hash2D(gx, gz, 14);
        const rScale = hash2D(gx, gz, 15);
        const rHue = hash2D(gx, gz, 16);

        if (rHue < 0.4) dummyColor.setRGB(0.9, 0.4, 1.4); // Amethyst
        else if (rHue < 0.75) dummyColor.setRGB(0.2, 0.9, 1.3); // Cyan
        else dummyColor.setRGB(1.2, 0.5, 0.9); // Magenta

        const rotY = hash2D(gx, gz, 17) * Math.PI * 2;
        const rotX = (hash2D(gx, gz, 18) - 0.5) * 0.3;
        const rotZ = (hash2D(gx, gz, 19) - 0.5) * 0.3;

        if (rType < 0.65) {
          // Upright crystal spire
          const scale = 1.4 + rScale * 1.2;
          spiresList.push({
            x: wx,
            y: height,
            z: wz,
            scale,
            rotX,
            rotY,
            rotZ,
            color: dummyColor.clone(),
          });

          // Strategic point light
          if (lights.length < 6 && isCaveBiome && hash2D(gx, gz, 20) > 0.8) {
            lights.push({
              position: new THREE.Vector3(wx, height + 2.5, wz),
              color: rHue < 0.5 ? '#a855f7' : '#00f0ff',
            });
          }
        } else {
          // Hanging / ceiling stalactite formation (or grounded crystalline shard)
          const scale = 1.2 + rScale * 1.0;
          stalactitesList.push({
            x: wx,
            y: height + (isCaveBiome ? 5.0 : 0.0),
            z: wz,
            scale,
            rotX: rotX + (isCaveBiome ? 0 : Math.PI), // Invert if on ground
            rotY,
            rotZ,
            color: dummyColor.clone(),
          });
        }
      }
    }

    return { spires: spiresList, stalactites: stalactitesList, lightPositions: lights };
  }, [size, waterLevel]);

  // Update instance matrices
  useLayoutEffect(() => {
    const dummy = new THREE.Object3D();

    const spireMesh = spireMeshRef.current;
    if (spireMesh && spires.length > 0) {
      for (let i = 0; i < spires.length; i++) {
        const p = spires[i];
        dummy.position.set(p.x, p.y, p.z);
        dummy.rotation.set(p.rotX, p.rotY, p.rotZ);
        dummy.scale.set(p.scale, p.scale, p.scale);
        dummy.updateMatrix();

        spireMesh.setMatrixAt(i, dummy.matrix);
        spireMesh.setColorAt(i, p.color);
      }
      if (spireMesh.instanceMatrix) spireMesh.instanceMatrix.needsUpdate = true;
      if (spireMesh.instanceColor) spireMesh.instanceColor.needsUpdate = true;
    }

    const stalactiteMesh = stalactiteMeshRef.current;
    if (stalactiteMesh && stalactites.length > 0) {
      for (let i = 0; i < stalactites.length; i++) {
        const p = stalactites[i];
        dummy.position.set(p.x, p.y, p.z);
        dummy.rotation.set(p.rotX, p.rotY, p.rotZ);
        dummy.scale.set(p.scale, p.scale, p.scale);
        dummy.updateMatrix();

        stalactiteMesh.setMatrixAt(i, dummy.matrix);
        stalactiteMesh.setColorAt(i, p.color);
      }
      if (stalactiteMesh.instanceMatrix) stalactiteMesh.instanceMatrix.needsUpdate = true;
      if (stalactiteMesh.instanceColor) stalactiteMesh.instanceColor.needsUpdate = true;
    }
  }, [spires, stalactites]);

  useFrame((state) => {
    const t = state.clock.getElapsedTime();
    crystalUniformsRef.current.uTime.value = t;

    // Animate point light intensities
    if (pointLightGroupRef.current) {
      const lights = pointLightGroupRef.current.children;
      for (let i = 0; i < lights.length; i++) {
        const light = lights[i] as THREE.PointLight;
        if (light.isPointLight) {
          light.intensity = 2.5 + 1.2 * Math.sin(t * 2.2 + i * 1.4);
        }
      }
    }
  });

  return (
    <group name="map2-fantasy-crystals-root">
      {spires.length > 0 && (
        <instancedMesh
          ref={spireMeshRef}
          args={[spireGeom, crystalMat, spires.length]}
          name="map2-crystal-spires"
          castShadow
          receiveShadow
          frustumCulled={false}
        />
      )}

      {stalactites.length > 0 && (
        <instancedMesh
          ref={stalactiteMeshRef}
          args={[stalactiteGeom, crystalMat, stalactites.length]}
          name="map2-crystal-stalactites"
          castShadow
          receiveShadow
          frustumCulled={false}
        />
      )}

      {showPointLights && lightPositions.length > 0 && (
        <group ref={pointLightGroupRef} name="map2-crystal-point-lights">
          {lightPositions.map((l, idx) => (
            <pointLight
              key={`crystal-light-${idx}`}
              position={[l.position.x, l.position.y, l.position.z]}
              color={l.color}
              intensity={3.0}
              distance={25}
              decay={2}
            />
          ))}
        </group>
      )}
    </group>
  );
};

export default Map2FantasyCrystals;
