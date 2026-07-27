import React, { useEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { sampleElevation } from './utils/worldSample';
import {
  buildColorTexture,
  buildDetailTexture,
  buildNormalTexture,
  buildRiverMaskTexture,
  buildRoughnessTexture,
} from './utils/worldTerrainTextures';
import { sceneElapsed } from './utils/sceneClock';

export interface WorldTerrainProps {
  world: World;
  /** World-space width the terrain is rendered at, regardless of data resolution. */
  renderSize?: number;
  /** Peak height as a fraction of renderSize. */
  heightRatio?: number;
  /** Mesh resolution (segments per side). Decoupled from the data resolution. */
  meshResolution?: number;
}

/**
 * Renders the huge SoA world as a single height-displaced mesh. The mesh is built at
 * `meshResolution` (e.g. 384) while COLOUR comes from a full data-resolution texture and the
 * fine relief from a residual normal map — so a 2048^2 world reads at full detail without a
 * 4M-vertex mesh. The texture palette is BIOME_RGB, the exact colours the minimap paints, so
 * the 3D world and the minimap always match. The bakes themselves live in
 * `utils/worldTerrainTextures`, shared with `WorldTerrainLod`.
 */
export const WorldTerrain: React.FC<WorldTerrainProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 256,
}) => {
  const geometry = useMemo(() => {
    const res = meshResolution;
    const verts = (res + 1) * (res + 1);
    const positions = new Float32Array(verts * 3);
    const uvs = new Float32Array(verts * 2);
    const heightUnits = renderSize * heightRatio;

    for (let gy = 0; gy <= res; gy++) {
      for (let gx = 0; gx <= res; gx++) {
        const u = gx / res;
        const v = gy / res;
        const e = sampleElevation(world, u, v);
        const i = gy * (res + 1) + gx;

        positions[i * 3] = (u - 0.5) * renderSize; // X
        positions[i * 3 + 1] = e * heightUnits; // Y (up)
        positions[i * 3 + 2] = (v - 0.5) * renderSize; // Z
        uvs[i * 2] = u;
        uvs[i * 2 + 1] = v;
      }
    }

    const indices = new Uint32Array(res * res * 6);
    let o = 0;
    for (let gy = 0; gy < res; gy++) {
      for (let gx = 0; gx < res; gx++) {
        const a = gy * (res + 1) + gx;
        const b = a + 1;
        const c = a + (res + 1);
        const d = c + 1;
        // CCW from above so computeVertexNormals() yields +Y normals (no back-face culling).
        indices[o++] = a;
        indices[o++] = c;
        indices[o++] = b;
        indices[o++] = b;
        indices[o++] = c;
        indices[o++] = d;
      }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geom.setAttribute('uv', new THREE.BufferAttribute(uvs, 2));
    geom.setIndex(new THREE.BufferAttribute(indices, 1));
    geom.computeVertexNormals();
    return geom;
  }, [world, renderSize, heightRatio, meshResolution]);

  const colorMap = useMemo(() => buildColorTexture(world), [world]);
  const normalMap = useMemo(
    () => buildNormalTexture(world, renderSize, heightRatio, meshResolution),
    [world, renderSize, heightRatio, meshResolution],
  );
  const roughnessMap = useMemo(() => buildRoughnessTexture(world), [world]);
  const detailMap = useMemo(() => buildDetailTexture(), []);

  // River-ribbon mask (feathered), so the shader knows where flowing water lies.
  const riverMaskMap = useMemo(() => buildRiverMaskTexture(world), [world]);

  // Inject (1) the high-repeat detail multiply and (2) a two-layer counter-scrolling shimmer
  // over the river ribbons, right after the base map sample. Standard onBeforeCompile patch;
  // the shader ref lets useFrame drive uTime so the rivers visibly FLOW.
  const shaderRef = useRef<{ uniforms: Record<string, { value: unknown }> } | null>(null);
  const onBeforeCompile = useMemo(
    () => (shader: { uniforms: Record<string, { value: unknown }>; fragmentShader: string }) => {
      shader.uniforms.uDetail = { value: detailMap };
      shader.uniforms.uRiverMask = { value: riverMaskMap };
      shader.uniforms.uTime = { value: 0 };
      shader.fragmentShader =
        'uniform sampler2D uDetail;\nuniform sampler2D uRiverMask;\nuniform float uTime;\n' +
        shader.fragmentShader.replace(
          '#include <map_fragment>',
          `#include <map_fragment>
          {
            float dtl = texture2D(uDetail, vMapUv * 220.0).r * 2.0;
            diffuseColor.rgb *= mix(1.0, dtl, 0.34);
            float riv = texture2D(uRiverMask, vMapUv).r;
            if (riv > 0.01) {
              float flow1 = texture2D(uDetail, vMapUv * 260.0 + vec2(uTime * 0.025, uTime * 0.045)).r * 2.0;
              float flow2 = texture2D(uDetail, vMapUv * 140.0 - vec2(uTime * 0.018, uTime * 0.03)).r * 2.0;
              diffuseColor.rgb *= mix(1.0, flow1 * 0.55 + flow2 * 0.45, riv * 0.3);
            }
          }`,
        );
      shaderRef.current = shader;
    },
    [detailMap, riverMaskMap],
  );

  useFrame((state) => {
    const sh = shaderRef.current;
    if (sh && sh.uniforms.uTime) sh.uniforms.uTime.value = sceneElapsed(state.clock);
  });

  useEffect(() => {
    return () => {
      colorMap.dispose();
      normalMap.dispose();
      roughnessMap.dispose();
      detailMap.dispose();
      riverMaskMap.dispose();
    };
  }, [colorMap, normalMap, roughnessMap, detailMap, riverMaskMap]);

  return (
    <mesh geometry={geometry} name="world-terrain" receiveShadow>
      <meshStandardMaterial
        map={colorMap}
        normalMap={normalMap}
        normalScale={new THREE.Vector2(1, 1)}
        roughnessMap={roughnessMap}
        roughness={1.0}
        metalness={0.0}
        onBeforeCompile={onBeforeCompile}
      />
    </mesh>
  );
};

export default WorldTerrain;
