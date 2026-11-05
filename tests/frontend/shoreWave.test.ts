import { describe, it, expect } from 'vitest';
import {
  applyShoreWave,
  SHORE_WAVE_AMPLITUDE,
  SHORE_WAVE_FREQUENCY,
  type PatchableShader,
  type ShoreWaveUniforms,
} from '../../src/components/Landscape/utils/shoreWave';

// `applyShoreWave` patches three's MeshStandardMaterial vertex shader. Its doc comment says it is pure
// so it can be asserted without a GL context; this file is the assertion. Under jsdom the R3F mock
// never compiles a program, so nothing else in the suite reaches this code.

/** The three chunk anchors the patch hooks, in the order they occur in three's standard shader. */
const STOCK_VERTEX_SHADER = [
  '#include <common>',
  'void main() {',
  '#include <beginnormal_vertex>',
  '#include <begin_vertex>',
  '}',
].join('\n');

function freshUniforms(): ShoreWaveUniforms {
  // Only `.value` is read; the Vector2 type is satisfied structurally.
  return {
    uTime: { value: 1.25 },
    uGridOrigin: { value: { x: 250, y: 250 } as unknown as ShoreWaveUniforms['uGridOrigin']['value'] },
  };
}

function patched(): { shader: PatchableShader; uniforms: ShoreWaveUniforms } {
  const shader: PatchableShader = { vertexShader: STOCK_VERTEX_SHADER, uniforms: {} };
  const uniforms = freshUniforms();
  applyShoreWave(shader, uniforms);
  return { shader, uniforms };
}

/** Pull the numeric literal that multiplies `name`'s right-hand side out of the generated GLSL. */
function coefficient(glsl: string, name: string): number {
  const m = new RegExp(`float ${name} = (-?)([0-9.]+) \\*`).exec(glsl);
  expect(m, `generated shader must define ${name} as <literal> * ...`).not.toBeNull();
  return Number(`${m![1]}${m![2]}`);
}

describe('applyShoreWave', () => {
  it('shares the uniform objects, so a per-frame uTime write reaches the compiled program', () => {
    const { shader, uniforms } = patched();
    expect(shader.uniforms.uTime).toBe(uniforms.uTime);
    expect(shader.uniforms.uGridOrigin).toBe(uniforms.uGridOrigin);
    uniforms.uTime.value = 9;
    expect((shader.uniforms.uTime as { value: number }).value).toBe(9);
  });

  it('declares its attribute and uniforms after #include <common> and keeps every anchor', () => {
    const { shader } = patched();
    const src = shader.vertexShader;
    for (const anchor of ['#include <common>', '#include <beginnormal_vertex>', '#include <begin_vertex>']) {
      expect(src).toContain(anchor);
    }
    const afterCommon = src.slice(src.indexOf('#include <common>'), src.indexOf('void main()'));
    expect(afterCommon).toContain('attribute float aShore;');
    expect(afterCommon).toContain('uniform float uTime;');
    expect(afterCommon).toContain('uniform vec2 uGridOrigin;');
  });

  it('relights the normal before displacing the position, both weighted by aShore', () => {
    const { shader } = patched();
    const src = shader.vertexShader;
    const normalAt = src.indexOf('objectNormal = normalize(mix(objectNormal');
    const displaceAt = src.indexOf('transformed.y += aShore * shoreWave;');
    expect(normalAt).toBeGreaterThan(src.indexOf('#include <beginnormal_vertex>'));
    expect(displaceAt).toBeGreaterThan(src.indexOf('#include <begin_vertex>'));
    expect(normalAt).toBeLessThan(displaceAt);
    expect(src).toMatch(/mix\(objectNormal, normalize\(vec3\(-shoreDx, 1\.0, -shoreDz\)\), aShore\)/);
  });

  it('bakes the wave constants into the shader, with the 1.5x phase rate on X', () => {
    const src = patched().shader.vertexShader;
    expect(coefficient(src, 'shoreWave')).toBeCloseTo(SHORE_WAVE_AMPLITUDE, 4);
    expect(src).toContain(`shoreG.x * ${SHORE_WAVE_FREQUENCY.toFixed(4)} + uTime * 1.5`);
    expect(src).toContain(`shoreG.y * ${SHORE_WAVE_FREQUENCY.toFixed(4)} + uTime;`);
  });

  it("the closed-form slopes it emits are the wave's own derivatives (checked by finite differences)", () => {
    const src = patched().shader.vertexShader;
    const a = coefficient(src, 'shoreWave');
    const dxCoef = coefficient(src, 'shoreDx');
    const dzCoef = coefficient(src, 'shoreDz');
    const f = SHORE_WAVE_FREQUENCY;
    // The emitted forms: wave = a·sin(px)·cos(pz), dx = dxCoef·cos(px)·cos(pz), dz = dzCoef·sin(px)·sin(pz),
    // with px = f·x + 1.5t and pz = f·z + t.
    const wave = (x: number, z: number, t: number) => a * Math.sin(f * x + 1.5 * t) * Math.cos(f * z + t);
    const h = 1e-4;
    for (const [x, z, t] of [[0, 0, 0], [12.5, -40, 3.1], [-250, 250, 17.75], [3, 7, 0.4]]) {
      const px = f * x + 1.5 * t;
      const pz = f * z + t;
      const fdX = (wave(x + h, z, t) - wave(x - h, z, t)) / (2 * h);
      const fdZ = (wave(x, z + h, t) - wave(x, z - h, t)) / (2 * h);
      // The literals are rounded to four decimals, so compare at that precision.
      expect(dxCoef * Math.cos(px) * Math.cos(pz)).toBeCloseTo(fdX, 3);
      expect(dzCoef * Math.sin(px) * Math.sin(pz)).toBeCloseTo(fdZ, 3);
    }
  });
});
