// A 2D canvas context double, for the suites that assert on what the app drew.
//
// jsdom implements `<canvas>` but not its rendering contexts, so `getContext('2d')` returns `null`
// and every draw call in `PixiViewport`'s Canvas-2D fallback is skipped. Three suites therefore
// stub `getContext` with a bag of spies — and each declared the bag loosely, then widened it again
// on the way into `mockReturnValue`.
//
// Both widenings were load-bearing in the wrong direction: the first meant a test could assert on a
// method the stub does not have and pass vacuously (a `vi.fn()` nobody created is
// `undefined`, and `expect(undefined).not.toHaveBeenCalled()` throws — but `expect(ctx.arcTo)` used
// as a value does not), and the second meant nothing checked that the stub resembled a context at
// all. Naming the members here makes the stub one thing, shared, and checkable.
//
// # Why the members are picked from the real interface
//
// `Pick<CanvasRenderingContext2D, …>` rather than a hand-written list of `Mock`s. A hand-written
// list is a second description of the same API, and it drifted the moment `textAlign` was declared
// `string`: the DOM's is `CanvasTextAlign`, so the stub's `''` was not a value a real context can
// hold. Picking also makes the widening in `stubCanvas2D` a *checked* one — a real context is one of
// these — which is what removes the cast there rather than relocating it.

import { vi } from 'vitest';

/** The 2D context methods and state the app's fallback renderer touches. */
export type Canvas2DStub = Pick<
  CanvasRenderingContext2D,
  | 'clearRect'
  | 'beginPath'
  | 'arc'
  | 'fill'
  | 'stroke'
  | 'moveTo'
  | 'lineTo'
  | 'closePath'
  | 'fillText'
  | 'fillRect'
  | 'fillStyle'
  | 'strokeStyle'
  | 'lineWidth'
  | 'font'
  | 'textAlign'
  | 'textBaseline'
>;

/** A fresh stub. Per call, so a test that inspects one does not see another's calls. */
export function makeCanvas2DStub(): Canvas2DStub {
  return {
    clearRect: vi.fn(),
    beginPath: vi.fn(),
    arc: vi.fn(),
    fill: vi.fn(),
    stroke: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    closePath: vi.fn(),
    fillText: vi.fn(),
    fillRect: vi.fn(),
    fillStyle: '',
    strokeStyle: '',
    lineWidth: 1,
    font: '',
    textAlign: 'left',
    textBaseline: 'alphabetic',
  };
}

/**
 * Make every `<canvas>` in the test hand back `stub` from `getContext`.
 *
 * The view on the next line names the one overload being replaced. `HTMLCanvasElement` is assignable
 * to it, because its `'2d'` overload returns a `CanvasRenderingContext2D` and that is a
 * `Canvas2DStub` — so this is a widening the compiler checks rather than a claim it takes on faith.
 *
 * Spying on `HTMLCanvasElement.prototype` directly did not work and could not: `getContext` is
 * overloaded, `vi.spyOn` resolves it to the last signature (`'webgpu'`), and the stub was therefore
 * being cast to `GPUCanvasContext`. That cast type-errored where nothing was checking, which is the
 * whole argument against writing one.
 */
export function stubCanvas2D(stub: Canvas2DStub): void {
  const canvasProto: { getContext(contextId: '2d'): Canvas2DStub | null } =
    HTMLCanvasElement.prototype;
  vi.spyOn(canvasProto, 'getContext').mockReturnValue(stub);
}
