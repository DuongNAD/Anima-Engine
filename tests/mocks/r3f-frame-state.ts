// A complete r3f frame state, for tests that stub `useFrame` themselves.
//
// # Why this exists
//
// Several suites replace `@react-three/fiber` with their own inline `vi.mock` so they can capture
// the `useFrame` callbacks and drive them by hand. Each one then built the state to call those
// callbacks with, and each built a different, smaller one — usually `{ clock: { getElapsedTime } }`,
// because that was all the component under test happened to read at the time.
//
// That is a stub shaped like the test rather than like r3f, and it fails in the one direction that
// matters: production code moved to reading `state.scene` (the handle r3f documents for imperative
// work, and the one the React Compiler rules leave writable) and three suites went red with
// `Cannot read properties of undefined (reading 'fog')` — a crash the real renderer cannot produce,
// because `RootState.scene` is never undefined.
//
// So the shape lives in one place, typed against the same `RootState` the aliased mock exports, and
// a caller overrides only the fields its assertions are about.
//
// # Why the r3f import is type-only
//
// `tests/vitest.config.ts` aliases `@react-three/fiber` to `react-three-fiber-mock.ts`, so a suite
// that calls `vi.mock('@react-three/fiber', factory)` replaces that module *by resolved path*. A
// runtime import of the mock from here would get the suite's factory instead of the real thing. The
// type import is erased before any of that can happen.
//
// `three` is a different module and is not mocked at all, so it is imported for its values: the
// scene and camera below are real instances rather than literals impersonating them.

import * as THREE from 'three';
import type { MockRenderer, RootState } from './react-three-fiber-mock';

/** What `useFrame` hands its callback. */
export type FrameCallback = (state: RootState, delta: number) => void;

/**
 * A frame state with every field r3f guarantees, overridable per test.
 *
 * The scene and camera are real three instances, for the same reason the aliased mock's are: a
 * literal only ever carries the members production happened to read on the day it was written, and
 * this file exists because that failed. Real instances cannot go out of date.
 */
export function makeFrameState(overrides: Partial<RootState> = {}): RootState {
  const gl: MockRenderer = {
    setSize: () => {},
    domElement: document.createElement('canvas'),
    render: () => {},
    getContext: () => {
      throw new Error(
        'the r3f frame state has no WebGL context: nothing running under jsdom should reach one',
      );
    },
    shadowMap: { enabled: false, needsUpdate: false },
  };
  return {
    clock: { getElapsedTime: () => 0, elapsedTime: 0 },
    scene: new THREE.Scene(),
    camera: new THREE.PerspectiveCamera(),
    gl,
    size: { width: 800, height: 600 },
    setFrameloop: () => {},
    setDpr: () => {},
    ...overrides,
  };
}

/** A frame state whose clock reads `elapsed`. The common single-field override. */
export function frameStateAt(elapsed: number): RootState {
  return makeFrameState({ clock: { getElapsedTime: () => elapsed, elapsedTime: elapsed } });
}
