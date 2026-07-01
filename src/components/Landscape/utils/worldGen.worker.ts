// Web Worker that runs the heavy world generation off the main thread, then transfers the
// resulting TypedArray buffers back zero-copy so the UI never freezes during a fresh build.
import { generateWorld } from './worldGen';
import type { WorldGenOptions } from './worldGen';

const ctx: any = self;

ctx.onmessage = (e: MessageEvent<{ seed: string | number; opts: WorldGenOptions }>) => {
  const { seed, opts } = e.data;
  const world = generateWorld(seed, opts);
  // Transfer the large ArrayBuffers (zero-copy) — the worker is done with them.
  const transfer = [
    world.elevation.buffer,
    world.moisture.buffer,
    world.temperature.buffer,
    world.flow.buffer,
    world.water.buffer,
    world.shore.buffer,
    world.biome.buffer,
    world.floraX.buffer,
    world.floraZ.buffer,
    world.floraScale.buffer,
    world.floraType.buffer,
  ];
  ctx.postMessage(world, transfer);
};
