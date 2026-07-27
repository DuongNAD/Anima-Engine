import React, { useMemo } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { surfaceHeight } from './utils/worldSample';
import type { CanonicalViewId } from './utils/mapManifest';
import {
  MARKER_DRAW_LIFT,
  MARKER_HEIGHT,
  ROUTE_DRAW_LIFT,
  type MapEvidenceRecord,
} from './utils/mapEvidence';

// ---------------------------------------------------------------------------------------
// The visible half of the navigation and collision evidence.
//
// # Why the scene draws this at all
//
// Independent review rejected the first accepted `navigation.png` and `collision.png` on content: a
// landscape photograph with no route in it does not prove ground is reachable, and a canopy seen from
// above does not show what a walker does when they meet a trunk. Both gates were being satisfied by
// pictures of the right place.
//
// # Why it draws a committed record rather than computing anything
//
// An overlay that computed its own route would be a picture of the overlay's beliefs, which is the
// same problem with an extra step. So the polyline and the collider circles come from
// `artifacts/map_evidence.json` — computed by `scripts/gen_map_evidence.ts`, re-verified against the
// rig's own rules by `tests/frontend/mapEvidence.test.ts`, and injected into the page by the capture
// harness. The image and the machine-checkable record are then the *same claim*: an image showing a
// route the record does not contain is not a state this pipeline can reach.
//
// # Why it cannot appear in the app
//
// It renders only when `WorldShowcase` is in capture mode *and* the harness has injected a record.
// An ordinary visit has neither, so nothing here runs.
// ---------------------------------------------------------------------------------------

/** Global the capture harness writes before the page loads. Absent on every ordinary visit. */
export const MAP_EVIDENCE_GLOBAL = '__animaMapEvidence';

/** Read the injected record, or `null`. Shape-checked, because a malformed global must not draw. */
export function readInjectedEvidence(): MapEvidenceRecord | null {
  if (typeof window === 'undefined') return null;
  const raw = (window as unknown as Record<string, unknown>)[MAP_EVIDENCE_GLOBAL];
  if (!raw || typeof raw !== 'object') return null;
  const rec = raw as Partial<MapEvidenceRecord>;
  if (!Array.isArray(rec.navigation?.route) || !Array.isArray(rec.collision?.colliders)) return null;
  return rec as MapEvidenceRecord;
}

/** Deliberately unnatural colours: a reviewer must never mistake an overlay for scenery. */
const ROUTE_COLOUR = '#ff2fd0';
const START_COLOUR = '#22ff88';
const GOAL_COLOUR = '#ffd21e';
const RING_COLOUR = '#28e0ff';
const FROM_COLOUR = '#ff3b30';
const TO_COLOUR = '#34ff5a';

/** Everything needed to put a marker on the ground. Grouped so it is one dependency, not four. */
export interface OverlayGround {
  world: World;
  renderSize: number;
  heightRatio: number;
  meshResolution: number;
}

export interface CaptureEvidenceOverlayProps extends OverlayGround {
  view: CanonicalViewId;
  evidence: MapEvidenceRecord;
}

/** Height to draw an overlay at: the ground, lifted just clear of it. */
function drawY(g: OverlayGround, x: number, z: number, lift: number): number {
  return surfaceHeight(g.world, x, z, g.renderSize, g.heightRatio, g.meshResolution) + lift;
}

/** The route: a tube a reviewer can follow, with distinct start and goal markers. */
const RouteEvidence: React.FC<{ route: MapEvidenceRecord['navigation']['route']; ground: OverlayGround }> = ({
  route,
  ground,
}) => {
  const tube = useMemo(() => {
    if (route.length < 2) return null;
    // Draped on the terrain rather than a straight chord between endpoints: a route that floated over
    // a valley would be exactly the kind of picture that looks like evidence and is not.
    const pts = route.map(([x, z]) => new THREE.Vector3(x, drawY(ground, x, z, ROUTE_DRAW_LIFT), z));
    const curve = new THREE.CatmullRomCurve3(pts, false, 'catmullrom', 0.1);
    return new THREE.TubeGeometry(curve, Math.max(64, route.length * 4), 0.7, 8, false);
  }, [route, ground]);

  if (!tube || route.length < 2) return null;
  const [sx, sz] = route[0];
  const [gx, gz] = route[route.length - 1];

  return (
    <group>
      <mesh geometry={tube}>
        <meshBasicMaterial color={ROUTE_COLOUR} toneMapped={false} />
      </mesh>
      {/* Endpoints as pillars, not spheres: a sphere resting on uneven ground is ambiguous about
          which patch of terrain it marks. */}
      <mesh position={[sx, drawY(ground, sx, sz, MARKER_DRAW_LIFT), sz]}>
        <cylinderGeometry args={[0.9, 0.9, MARKER_HEIGHT, 12]} />
        <meshBasicMaterial color={START_COLOUR} toneMapped={false} />
      </mesh>
      <mesh position={[gx, drawY(ground, gx, gz, MARKER_DRAW_LIFT), gz]}>
        <cylinderGeometry args={[0.9, 0.9, MARKER_HEIGHT, 12]} />
        <meshBasicMaterial color={GOAL_COLOUR} toneMapped={false} />
      </mesh>
    </group>
  );
};

/** Trunk colliders as flat rings, plus the push-out cases the rig's own resolver produced. */
const CollisionEvidence: React.FC<{
  collision: MapEvidenceRecord['collision'];
  ground: OverlayGround;
}> = ({ collision, ground }) => {
  // One instanced mesh, built imperatively and handed to r3f as an object. 600 separate `<mesh>`
  // elements would be 600 draw calls and the same picture; a ref callback that filled the instance
  // matrix would be a render-phase write to a mutable object, which is the pattern this repository is
  // in the middle of removing everywhere else.
  const rings = useMemo(() => {
    const geometry = new THREE.TorusGeometry(1, 0.09, 6, 24);
    const material = new THREE.MeshBasicMaterial({ color: RING_COLOUR, toneMapped: false });
    const count = Math.max(1, collision.colliders.length);
    const mesh = new THREE.InstancedMesh(geometry, material, count);
    const m = new THREE.Matrix4();
    const scale = new THREE.Vector3();
    collision.colliders.forEach((c, i) => {
      m.makeRotationX(-Math.PI / 2);
      m.scale(scale.set(c.radius, c.radius, c.radius));
      m.setPosition(c.x, drawY(ground, c.x, c.z, 0.35), c.z);
      mesh.setMatrixAt(i, m);
    });
    mesh.count = collision.colliders.length;
    mesh.instanceMatrix.needsUpdate = true;
    mesh.frustumCulled = false;
    return mesh;
  }, [collision, ground]);

  return (
    <group>
      <primitive object={rings} />

      {collision.pushOutCases.map((c) => (
        <group key={c.label}>
          {/* Where the walker was (inside the trunk, or already clear) ... */}
          <mesh position={[c.from.x, drawY(ground, c.from.x, c.from.z, 2.4), c.from.z]}>
            <boxGeometry args={[0.5, 4.8, 0.5]} />
            <meshBasicMaterial color={FROM_COLOUR} toneMapped={false} />
          </mesh>
          {/* ... and where `resolveFloraOverlap` put them. Identical positions for the
              already-clear case, which is the point of including it. */}
          <mesh position={[c.to.x, drawY(ground, c.to.x, c.to.z, 2.4), c.to.z]}>
            <boxGeometry args={[0.5, 4.8, 0.5]} />
            <meshBasicMaterial color={TO_COLOUR} toneMapped={false} />
          </mesh>
        </group>
      ))}
    </group>
  );
};

/**
 * Draw the evidence for `view`, or nothing.
 *
 * Only `navigation` and `collision` have an overlay: those are the two gates a photograph of the
 * right place cannot satisfy. Adding one to `ecosystem` or `water` would obscure the very thing those
 * views exist to show.
 */
export const CaptureEvidenceOverlay: React.FC<CaptureEvidenceOverlayProps> = ({
  view,
  evidence,
  world,
  renderSize,
  heightRatio,
  meshResolution,
}) => {
  const ground = useMemo<OverlayGround>(
    () => ({ world, renderSize, heightRatio, meshResolution }),
    [world, renderSize, heightRatio, meshResolution],
  );
  if (view === 'navigation') return <RouteEvidence route={evidence.navigation.route} ground={ground} />;
  if (view === 'collision') return <CollisionEvidence collision={evidence.collision} ground={ground} />;
  return null;
};

export default CaptureEvidenceOverlay;
