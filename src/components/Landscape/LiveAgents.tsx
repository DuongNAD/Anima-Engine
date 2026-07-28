import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { surfaceHeight } from './utils/worldSample';
import { isInsideSimBounds, simToRender } from './utils/liveAgentTransform';
import { makeDeer, makeLion, makeRabbit, makeWildcat } from './utils/creatureBodies';
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
// water, flora and wildlife had no idea the simulation existed.
//
// # Why bodies and not spheres
//
// The first version drew every published segment as a sphere, which proved the positions crossed
// the IPC boundary and landed on the terrain. It also told a viewer nothing: a thousand identical
// red balls in a grid is a debug overlay, not a population. Each agent is now one animal — prey read
// as prey, predators as predators, and two prey of different builds as two kinds of animal.
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
 * Instances allocated per body type up front, grown by doubling when a population exceeds it.
 *
 * 1024 covers the 1000-agent benchmark workload spread across four bodies with room to spare, and
 * the growth path means a bigger world costs a remount rather than a dropped animal.
 */
const INITIAL_CAPACITY = 1024;

/** Model scale. The bodies are authored around one render unit; the world spans 1200. */
const BODY_SCALE = 3.2;

/** Lifts a creature clear of the ground so it stands on the terrain rather than in it. */
const GROUND_OFFSET = 0.2;

/**
 * The bodies a live agent can wear, and which role each belongs to.
 *
 * Two per role rather than one: a population where every prey is the same silhouette reads as one
 * species cloned, which is the opposite of what an evolution simulator is trying to show. The choice
 * is by `agent_id`, so an agent keeps its body for its whole life and across a reload.
 */
const BODIES = [
  { id: 'deer', role: 'prey' as const, make: makeDeer, tint: '#cdb894' },
  { id: 'rabbit', role: 'prey' as const, make: makeRabbit, tint: '#d8cdb8' },
  { id: 'lion', role: 'predator' as const, make: makeLion, tint: '#e0b45f' },
  { id: 'wildcat', role: 'predator' as const, make: makeWildcat, tint: '#c9b79c' },
];

/** Round up to the next power of two, so capacity growth is a handful of remounts, not one per tick. */
function nextCapacity(needed: number): number {
  let capacity = INITIAL_CAPACITY;
  while (capacity < needed) capacity *= 2;
  return capacity;
}

/** Which body an agent wears: fixed by its id, so it does not change shape between frames. */
function bodyIndexFor(seg: SegmentState): number {
  const predator = seg.agent_type === 'predator';
  // Two candidates per role, picked by the low bit of a cheap hash of the id.
  const h = Math.imul(seg.agent_id + 1, 2654435761) >>> 0;
  return (predator ? 2 : 0) + (h & 1);
}

export const LiveAgents: React.FC<LiveAgentsProps> = ({
  world,
  renderSize,
  heightRatio,
  meshResolution,
}) => {
  const meshRefs = useRef<(THREE.InstancedMesh | null)[]>([]);
  // The payload lands in a ref rather than state: the emit thread publishes far faster than a React
  // render is worth paying for, and `useFrame` is already the place that reads it.
  const segmentsRef = useRef<SegmentState[]>([]);
  const [capacity, setCapacity] = useState(INITIAL_CAPACITY);
  const [connected, setConnected] = useState(false);

  // Scratch objects, allocated once. `useFrame` runs every frame over every agent, so anything
  // constructed inside it would be garbage at 60 Hz.
  const scratch = useMemo(() => ({ dummy: new THREE.Object3D(), color: new THREE.Color() }), []);
  const geometries = useMemo(() => BODIES.map((b) => b.make()), []);
  const materials = useMemo(
    () => BODIES.map(() => new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.85, metalness: 0, flatShading: true })),
    [],
  );
  const tints = useMemo(() => BODIES.map((b) => new THREE.Color(b.tint)), []);

  useEffect(
    () => () => {
      geometries.forEach((g) => g.dispose());
      materials.forEach((m) => m.dispose());
    },
    [geometries, materials],
  );

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
    const segments = segmentsRef.current;
    const { dummy, color } = scratch;
    const drawn = [0, 0, 0, 0];

    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      // One body per agent, not one per segment: the root carries the agent's position and facing,
      // and drawing the other segments too would stack three animals inside each other.
      if (seg.parent_segment_id !== null && seg.segment_id !== 0) continue;
      // A coordinate outside the simulation's own bounds means this consumer and the publisher
      // disagree about the space. Skipping says so; clamping would draw a tidy lie at the map edge.
      if (!isInsideSimBounds(seg.x, seg.z)) continue;

      const b = bodyIndexFor(seg);
      const mesh = meshRefs.current[b];
      if (!mesh) continue;
      if (drawn[b] >= capacity) continue;

      const rx = simToRender(seg.x, renderSize);
      const rz = simToRender(seg.z, renderSize);
      // Height comes from the rendered terrain, not from the payload's `y`. The simulation's y is a
      // 0..10 band that carries no relationship to this mesh's elevation, so trusting it would bury
      // creatures in hillsides and float them over valleys. Standing them on the surface the viewer
      // can see is the honest composition, and it is the same `surfaceHeight` the camera rig walks on.
      const ry = surfaceHeight(world, rx, rz, renderSize, heightRatio, meshResolution) + GROUND_OFFSET;

      dummy.position.set(rx, ry, rz);
      dummy.rotation.set(0, seg.yaw, 0);
      dummy.scale.setScalar(BODY_SCALE);
      dummy.updateMatrix();
      mesh.setMatrixAt(drawn[b], dummy.matrix);

      // Energy dims a creature toward its silhouette as it starves, so a population at the floor
      // reads as one at a glance rather than looking identical to a thriving one.
      color.copy(tints[b]).multiplyScalar(0.45 + 0.55 * Math.min(1, Math.max(0, seg.energy / 100)));
      mesh.setColorAt(drawn[b], color);
      drawn[b]++;
    }

    let needed = 0;
    for (let b = 0; b < BODIES.length; b++) {
      const mesh = meshRefs.current[b];
      if (!mesh) continue;
      mesh.count = drawn[b];
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
      needed = Math.max(needed, drawn[b]);
    }
    // Grown only when a bucket actually filled: the population is split across four bodies, so the
    // trigger is the busiest bucket rather than the headcount.
    if (needed >= capacity) setCapacity(nextCapacity(needed + 1));
  });

  // Nothing at all until a tick has arrived: empty InstancedMeshes still cost draw calls, and a
  // scene with no backend should be exactly the scene it was before this component existed.
  if (!connected) return null;

  return (
    <group name="live-agents">
      {BODIES.map((b, i) => (
        <instancedMesh
          key={`${b.id}-${capacity}`}
          ref={(m) => {
            meshRefs.current[i] = m;
          }}
          args={[geometries[i], materials[i], capacity]}
          name={`live-${b.id}`}
          frustumCulled={false}
          castShadow
          receiveShadow
        />
      ))}
    </group>
  );
};

export default LiveAgents;
