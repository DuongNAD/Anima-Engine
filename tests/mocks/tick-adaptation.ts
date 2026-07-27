// The `simulation-tick` mock's payload adaptation, as a named switch.
//
// # What the adaptation is
//
// The backend emits `simulation-tick` with two shapes: a bare `SegmentState[]`, and the whole tick
// object that carries `segments` alongside `environmental_state` and `head_directions`. Some
// consumers subscribe to the first and some to the second, and the mocked `emit` in
// `setup-vitest.ts` guesses which by looking for `segmentsRef.current` in the callback's source.
//
// That guess is ugly and it is what the suites were written against. What matters here is that it
// is *defeatable*: a test whose subject is the object shape needs the object to arrive intact.
//
// # Why a module and not a global
//
// It used to be a property read off `globalThis` through a widening cast, which is both an untyped
// contract between two files and a flag nothing could find by name. A module-scoped variable behind
// two exported functions is the same switch with neither problem — and `setup-vitest.ts` runs once
// per test file, so the state is per-file and cannot leak between them.

let deliverWhole = false;

/**
 * Deliver the whole tick payload to every listener, instead of adapting it per callback.
 *
 * Call it in a `try`/`finally` so one test's choice does not become the next test's environment.
 */
export function setWholeTickPayloadDelivery(on: boolean): void {
  deliverWhole = on;
}

/** Whether `emit` should narrow a tick payload to its `segments` for segment-shaped callbacks. */
export function tickPayloadIsAdapted(): boolean {
  return !deliverWhole;
}
