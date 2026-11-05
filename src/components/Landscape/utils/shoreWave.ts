import type * as THREE from 'three';

// Shoreline ripple. Same wave the terrain has always had — amplitude, frequency and the 1.5x phase
// rate on X are carried over unchanged — but evaluated per vertex on the GPU.
//
// It used to run on the CPU in a `useFrame`: for each of the three LOD geometries it walked the
// whole grid (250,000 cells at LOD 0), wrote into the position buffer and then called
// `computeVertexNormals()` on a quarter-million vertices, every frame. That cost is why it was
// deleted. None of it was needed — the wave is a closed-form function of position and time, so the
// GPU can evaluate it in the vertex shader and the CPU's only per-frame job is writing one float.
export const SHORE_WAVE_AMPLITUDE = 0.3;
export const SHORE_WAVE_FREQUENCY = 0.15;

const glsl = (n: number) => n.toFixed(4);

/** The uniforms the shoreline wave drives. Only `uTime` changes per frame. */
export interface ShoreWaveUniforms {
  uTime: { value: number };
  uGridOrigin: { value: THREE.Vector2 };
}

/** The slice of three's `onBeforeCompile` shader argument this patch touches. */
export interface PatchableShader {
  vertexShader: string;
  uniforms: Record<string, unknown>;
}

/**
 * Rewrites a MeshStandardMaterial vertex shader to displace and relight the shoreline.
 *
 * Pure, and in its own module, so it can be asserted without a GL context: under jsdom every material
 * is a plain DOM element, so a test that renders `<Terrain>` can never reach the compiled program.
 * `tests/frontend/shoreWave.test.ts` is that test. The previous version of this feature had no test
 * of any kind, which is how deleting it registered as a pure performance win.
 */
export function applyShoreWave(shader: PatchableShader, uniforms: ShoreWaveUniforms): void {
  shader.uniforms.uTime = uniforms.uTime;
  shader.uniforms.uGridOrigin = uniforms.uGridOrigin;
  shader.vertexShader = shader.vertexShader
    .replace(
      '#include <common>',
      `#include <common>
      attribute float aShore;
      uniform float uTime;
      uniform vec2 uGridOrigin;`,
    )
    .replace(
      '#include <beginnormal_vertex>',
      `#include <beginnormal_vertex>
      vec2 shoreG = vec2(position.x + uGridOrigin.x, position.z + uGridOrigin.y);
      float shorePhaseX = shoreG.x * ${glsl(SHORE_WAVE_FREQUENCY)} + uTime * 1.5;
      float shorePhaseZ = shoreG.y * ${glsl(SHORE_WAVE_FREQUENCY)} + uTime;
      float shoreWave = ${glsl(SHORE_WAVE_AMPLITUDE)} * sin(shorePhaseX) * cos(shorePhaseZ);
      // The displaced normal, in closed form. This is what computeVertexNormals() bought, and why
      // deleting the animation was a visible loss rather than a free one: at this amplitude the eye
      // reads the moving light, not 0.3 units of displacement.
      float shoreDx = ${glsl(SHORE_WAVE_AMPLITUDE * SHORE_WAVE_FREQUENCY)} * cos(shorePhaseX) * cos(shorePhaseZ);
      float shoreDz = -${glsl(SHORE_WAVE_AMPLITUDE * SHORE_WAVE_FREQUENCY)} * sin(shorePhaseX) * sin(shorePhaseZ);
      objectNormal = normalize(mix(objectNormal, normalize(vec3(-shoreDx, 1.0, -shoreDz)), aShore));`,
    )
    .replace(
      '#include <begin_vertex>',
      `#include <begin_vertex>
      transformed.y += aShore * shoreWave;`,
    );
}
