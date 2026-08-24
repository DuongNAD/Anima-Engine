// Web Worker that runs the heavy world generation off the main thread, then transfers the
// resulting TypedArray buffers back zero-copy so the UI never freezes during a fresh build.
import { generateWorld } from './worldGen';
import type { WorldGenOptions } from './worldGen';

/** What the main thread asks for. */
interface WorldGenRequest {
  seed: string | number;
  opts: WorldGenOptions;
}

// `self` here is a `DedicatedWorkerGlobalScope`, but this project's `tsconfig` loads only the DOM
// libs (adding "WebWorker" makes the two collide on `self`, `postMessage` and half the event map),
// so TypeScript sees a `Window`. That is why this used to open with `const ctx: any = self`.
//
// It does not need to. Both calls below are spelled the way that is valid in *both* scopes:
// `addEventListener('message', ...)` rather than assigning `onmessage`, and the options form of
// `postMessage`, whose `{ transfer }` is `StructuredSerializeOptions` in a worker and
// `WindowPostMessageOptions` in a window. No cast, and the payload type is checked.

self.addEventListener('message', (e: MessageEvent<WorldGenRequest>) => {
  const { seed, opts } = e.data;
  const world = generateWorld(seed, opts);
  // Transfer the large ArrayBuffers (zero-copy) — the worker is done with them.
  const transfer = [
    world.elevation.buffer,
    world.moisture.buffer,
    world.temperature.buffer,
    world.flow.buffer,
    world.slope.buffer,
    world.water.buffer,
    world.riverAmt.buffer,
    world.shore.buffer,
    world.biome.buffer,
    world.floraX.buffer,
    world.floraZ.buffer,
    world.floraScale.buffer,
    world.floraType.buffer,
    world.waterfallX.buffer,
    world.waterfallZ.buffer,
    world.waterfallTopE.buffer,
    world.waterfallDrop.buffer,
    world.waterfallYaw.buffer,
    world.caveX.buffer,
    world.caveZ.buffer,
    world.caveE.buffer,
    world.caveYaw.buffer,
  ];
  self.postMessage(world, { transfer });
});
