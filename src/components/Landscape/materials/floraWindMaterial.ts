import * as THREE from 'three';

// prototype, to be replaced — W3-lite CC0 flora material with optional root-anchored instanced wind sway.

export interface FloraWindMaterialOptions {
  enableWind: boolean;
  smoothShading: boolean;
}

export interface FloraWindMaterialBundle {
  readonly material: THREE.MeshStandardMaterial;
  updateTime(timeSec: number): void;
  dispose(): void;
}

export function createFloraWindMaterial(
  options: FloraWindMaterialOptions,
): FloraWindMaterialBundle {
  const { enableWind, smoothShading } = options;
  const windTimeUniform = { value: 0 };
  const material = new THREE.MeshStandardMaterial({
    vertexColors: true,
    roughness: smoothShading ? 0.82 : 0.9,
    metalness: 0.0,
    flatShading: !smoothShading,
  });

  if (enableWind) {
    material.onBeforeCompile = (shader) => {
      shader.uniforms.uWindTime = windTimeUniform;
      shader.vertexShader =
        'uniform float uWindTime;\n' +
        shader.vertexShader.replace(
          '#include <begin_vertex>',
          `#include <begin_vertex>
          #ifdef USE_INSTANCING
            float wMask = clamp(transformed.y * 0.55, 0.0, 1.0);
            float wPhase = instanceMatrix[3][0] * 0.08 + instanceMatrix[3][2] * 0.06 + uWindTime * 1.45;
            float wFlutter = sin(wPhase) * 0.055 + cos(wPhase * 2.13 + transformed.y) * 0.02;
            transformed.x += wFlutter * wMask * wMask;
            transformed.z += cos(wPhase * 0.87) * 0.04 * wMask * wMask;
          #endif`,
        );
    };
    material.customProgramCacheKey = () => (smoothShading ? 'w3-flora-wind-smooth' : 'w3-flora-wind-flat');
  } else {
    material.customProgramCacheKey = () => (smoothShading ? 'w3-flora-static-smooth' : 'w3-flora-static-flat');
  }

  return {
    material,
    updateTime(timeSec: number) {
      windTimeUniform.value = timeSec;
    },
    dispose() {
      material.dispose();
    },
  };
}
