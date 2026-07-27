// A 2D canvas context double, for the suites that assert on what the app drew.
//
// jsdom implements `<canvas>` but not its rendering contexts, so `getContext('2d')` returns `null`
// and every draw call in `PixiViewport`'s Canvas-2D fallback is skipped. Three suites therefore
// stub `getContext` with a bag of spies — and each declared it `let mockCtx: any`, then handed it
// over as `mockReturnValue(mockCtx as any)`.
//
// Both `any`s were load-bearing in the wrong direction: the first meant a test could assert on a
// method the stub does not have and pass vacuously (a `vi.fn()` that was never created is
// `undefined`, and `expect(undefined).not.toHaveBeenCalled()` throws — but `expect(ctx.arcTo)` used
// as a value does not), and the second meant nothing checked that the stub resembled a context at
// all. Naming the members here makes the stub one thing, shared, and checkable.

import { vi } from 'vitest';
import type { Mock } from 'vitest';

/** The 2D context methods and state the app's fallback renderer touches. */
export interface Canvas2DStub {
  clearRect: Mock;
  beginPath: Mock;
  arc: Mock;
  fill: Mock;
  stroke: Mock;
  moveTo: Mock;
  lineTo: Mock;
  closePath: Mock;
  fillText: Mock;
  fillRect: Mock;
  fillStyle: string;
  strokeStyle: string;
  lineWidth: number;
  font: string;
  textAlign: string;
  textBaseline: string;
}

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
    textAlign: '',
    textBaseline: '',
  };
}

/**
 * Make every `<canvas>` in the test hand back `stub` from `getContext`.
 *
 * One widening, in one place, for the same reason `react-three-fiber-mock.ts` documents its own: a
 * stub standing in for a class jsdom cannot host is not a subtype of it and never will be, and the
 * choice is where the claim is made, not whether. Here it is made once, against a named interface,
 * instead of three times against `any`.
 */
export function stubCanvas2D(stub: Canvas2DStub): void {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(
    stub as unknown as CanvasRenderingContext2D,
  );
}
