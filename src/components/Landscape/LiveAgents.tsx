import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { surfaceHeight } from './utils/worldSample';
import { isInsideSimBounds, simToRender } from './utils/liveAgentTransform';
import type { SegmentState } from '../../types/generated/SegmentState';
import type { SimulationTickPayload } from '../../types/generated/SimulationTickPayload';

// ---------------------------------------------------------------------------------------
// LiveAgents — the running simulation's creatures, drawn on the world they actually live in.
//
// # What this closes
//
// The backend has always simulated inside the shared world: `init_world` loads the same World
// Artifact the frontend generates and downsamples it. But the only view of the population was the
// dashboard's 2D canvas — a black grid with dots — while the 3D scene that draws the terrain,
// water, flora and wildlife had no idea the simulation existed. Two renderings of one world, and
// the good one was empty.
//
// # Why this renders nothing outside the desktop app
//
// It listens on a Tauri event. `landscape.html` opened in an ordinary browser has no transport, so
// this mounts, finds none, and draws nothing — which is what keeps the canonical view capture
// byte-identical: that harness drives a plain Chromium, so no agent can appear in an image whose
// SHA-256 is a gate. The same property makes the standalone showcase keep working with no backend.
// ---------------------------------------------------------------------------------------

export interface LiveAgentsProps {
  world: World;
  renderSize: number;
  heightRatio: number;
  meshResolution: number;
}

/**
 * Instances allocated up front. Three segments per creature at the moment, so this holds a founding
 * population of ~1300 before it has to grow — comfortably past the 10 a default genesis creates and
 * past the 1000 the benchmark asks for at three segments each... which it is not, so it grows.
 */
const INITIAL_CAPACITY = 4096;

/** Radius of a drawn segment, in render units. Small enough to read as a creature at map scale. */
const SEGMENT_RADIUS = 1.6;

/** Lifts a creature clear of the ground so it reads as standing on the terrain, not embedded in it. */
const GROUND_OFFSET = SEGMENT_RADIUS * 0.9;

const PREY_COLOR = new THREE.Color('#5ad67d');
const PREDATOR_COLOR = new THREE.Color('#e2554f');
/** Neither prey nor predator: a creature whose role the payload did not carry. */
const UNTYPED_COLOR = new THREE.Color('#cfd6dd');

/** Round up to the next power of two, so capacity growth is a handful of remounts, not one per tick. */
function nextCapacity(needed: number): number {
  let capacity = INITIAL_CAPACITY;
  while (capacity < needed) capacity *= 2;
  return capacity;
}

export const LiveAgents: React.FC<LiveAgentsProps> = ({
  world,
  renderSize,
  heightRatio,
  meshResolution,
}) => {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  // The payload lands in a ref rather than state: the emit thread publishes far faster than a React
  // render is worth paying for, and `useFrame` is already the place that reads it.
  const segmentsRef = useRef<SegmentState[]>([]);
  const [capacity, setCapacity] = useState(INITIAL_CAPACITY);
  const [connected, setConnected] = useState(false);

  // Scratch objects, allocated once. `useFrame` runs every frame over every segment, so anything
  // constructed inside it would be garbage at 60 Hz.
  const scratch = useMemo(() => ({ dummy: new THREE.Object3D(), color: new THREE.Color() }), []);
  const geometry = useMemo(() => new THREE.SphereGeometry(SEGMENT_RADIUS, 10, 8), []);
  const material = useMemo(
    () => new THREE.MeshStandardMaterial({ roughness: 0.55, metalness: 0.05 }),
    [],
  );

  useEffect(() => () => geometry.dispose(), [geometry]);
  useEffect(() => () => material.dispose(), [material]);

  useEffect(() => {
    // No Tauri transport means no simulation to draw. Checked rather than caught: an ordinary
    // browser load must not depend on an import rejecting.
    if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) return;

    let unlisten: (() => void) | undefined;
    let cancelled = false;

    void import('@tauri-apps/api/event')
      .then(({ listen }) =>
        listen<SimulationTickPayload>('simulation-tick', (event) => {
          segmentsRef.current = event.payload.segments;
          if (event.payload.segments.length > 0) setConnected(true);
        }),
      )
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {
        // A transport that is present but refuses to subscribe leaves the scene as it was: empty.
        // Nothing is logged, because `console_hygiene.spec.ts` holds this scene to zero Anima-owned
        // console output and a backend that is simply not running is not a defect to report.
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useFrame(() => {
    const mesh = meshRef.current;
    if (!mesh) return;
    const segments = segmentsRef.current;

    if (segments.length > capacity) {
      setCapacity(nextCapacity(segments.length));
      return;
    }

    const { dummy, color } = scratch;
    let drawn = 0;

    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      // A coordinate outside the simulation's own bounds means this consumer and the publisher
      // disagree about the space. Skipping says so; clamping would draw a tidy lie at the map edge.
      if (!isInsideSimBounds(seg.x, seg.z)) continue;

      const rx = simToRender(seg.x, renderSize);
      const rz = simToRender(seg.z, renderSize);
      // Height comes from the rendered terrain, not from the payload's `y`. The simulation's y is a
      // 0..10 band that carries no relationship to this mesh's elevation, so trusting it would bury
      // creatures in hillsides and float them over valleys. Standing them on the surface the viewer
      // can see is the honest composition, and it is the same `surfaceHeight` the camera rig walks on.
      const ry = surfaceHeight(world, rx, rz, renderSize, heightRatio, meshResolution) + GROUND_OFFSET;

      dummy.position.set(rx, ry, rz);
      dummy.rotation.set(0, seg.yaw, 0);
      dummy.updateMatrix();
      mesh.setMatrixAt(drawn, dummy.matrix);

      const source =
        seg.agent_type === 'prey'
          ? PREY_COLOR
          : seg.agent_type === 'predator'
            ? PREDATOR_COLOR
            : UNTYPED_COLOR;
      // Energy dims a creature toward its silhouette as it starves, so a population at the floor
      // reads as one at a glance rather than looking identical to a thriving one.
      color.copy(source).multiplyScalar(0.45 + 0.55 * Math.min(1, Math.max(0, seg.energy / 100)));
      mesh.setColorAt(drawn, color);
      drawn++;
    }

    mesh.count = drawn;
    mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  });

  // Nothing at all until a tick has arrived: an empty InstancedMesh still costs a draw call, and a
  // scene with no backend should be exactly the scene it was before this component existed.
  if (!connected) return null;

  return (
    <instancedMesh
      key={capacity}
      ref={meshRef}
      args={[geometry, material, capacity]}
      frustumCulled={false}
      castShadow
      receiveShadow
    />
  );
};

export default LiveAgents;
