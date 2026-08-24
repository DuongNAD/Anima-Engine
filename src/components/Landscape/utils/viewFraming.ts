// Where to stand to photograph a place in a *bounded* world without photographing its boundary.
//
// # The defect this exists for
//
// The canonical camera poses were derived by putting the camera at `subject - (7, 7)` and looking
// at the subject: a fixed south-west offset. For a subject in the north-east that aims the camera
// north-east, straight at the two nearest edges of a 1200-unit square of terrain, and four of the
// eight canonical views — `collision`, `water`, `biome_transition`, `ecosystem` — came back with the
// hard terrain cut prominent along the right edge. Independent review rejected all four. The images
// were of the right subject and unusable as evidence, because "no visible severe seam or edge
// artifact" is part of the acceptance criterion and a terrain plane stopping mid-air is the most
// severe one available.
//
// A fixed offset cannot fix this — every fixed offset is wrong for the subjects on one side of the
// map. What is needed is a constraint: *the ground the frame covers must be inside the world*.
//
// # The constraint, and why it is checkable rather than a rule of thumb
//
// "Turn it inward" is the right instinct and not sufficient: a wide-enough frame reaches past the
// boundary in the corners even while the view direction points dead at the middle of the map. So
// this projects the frame itself. Take the camera basis, walk a grid of directions across the image
// rectangle, intersect each with the horizontal plane through the subject, and require every hit to
// land inside the world footprint. That is the picture's own ground coverage, so a pose that passes
// cannot show the edge except where terrain relief lifts something distant into view — which is a
// mountain silhouette, not a cut plane.
//
// The camera is placed by searching azimuths around the subject and taking the most inward-facing
// one that satisfies the constraint. Deterministic, and it reports *why* when nothing satisfies it
// instead of quietly returning the least bad pose.

/** A camera pose: where it is and what it looks at. */
export interface FramedPose {
  position: [number, number, number];
  target: [number, number, number];
}

/** What the solver was asked for, and what it found. */
export interface FramingSolution {
  pose: FramedPose;
  /** Azimuth chosen, radians, measured from +x toward +z. */
  azimuth: number;
  /**
   * How inward the view direction points: `1` is straight at the middle of the map, `-1` straight
   * away from it. The solver maximises this subject to the footprint constraint.
   */
  inwardness: number;
  /** Furthest ground distance the frame covers, in the same units as the inputs. */
  groundReach: number;
}

export interface FramingRequest {
  /** Subject position: `[x, y, z]`, y being the ground height there. */
  subject: [number, number, number];
  /** Straight-line distance from camera to subject. */
  distance: number;
  /**
   * Downward angle from the camera to the subject, degrees.
   *
   * Must exceed the vertical half-FOV or the frame contains the horizon, and with the horizon comes
   * whatever the terrain plane does when it ends. 42° against a 55° FOV leaves 14.5°.
   */
  pitchDeg: number;
  /** How far above the subject's own height to aim, so the frame is not all ground. */
  aimAbove?: number;
  /** Vertical field of view of the scene camera, degrees. `WorldShowcase` uses 55. */
  fovDeg?: number;
  /** Viewport aspect ratio (width / height). The capture harness uses 1280×720. */
  aspect?: number;
  /** Half-extent of the world footprint on each horizontal axis. */
  footprintHalf: number;
  /** Keep the camera itself this far inside the footprint. */
  cameraMargin?: number;
  /** Azimuths to try. More is finer; 64 gives 5.6° steps. */
  azimuthSteps?: number;
  /**
   * Require the frame's whole ground coverage to land inside the footprint. Default `true`.
   *
   * Set `false` only for a view whose subject is the horizon itself. The lateral spread of a frame
   * grows with the distance it reaches — at 55° FOV and 16:9 the half-width is 0.93× the ground
   * distance — so a wide shot on a ±100 world is arithmetically unsatisfiable, not merely awkward.
   * Those views (`overview`, `lighting`) get sky in frame by design: a mountain against sky is a
   * silhouette, and a continent surrounded by ocean is what this world is. What the constraint
   * exists to prevent is a *close-up* with the cut terrain plane behind the subject.
   *
   * With it off, the solver still turns the camera inward and still keeps it inside the footprint;
   * it just does not certify the frame's far edge.
   */
  requireFrameInside?: boolean;
}

/** Sample density across the image rectangle when projecting the frame onto the ground. */
const FRAME_SAMPLES = 7;

/**
 * Where the frame's ground coverage lands, or `null` if any sample misses the ground plane.
 *
 * A miss means that direction points at or above the horizon, which is itself a failure: the
 * horizon is where the terrain plane's edge lives.
 */
function frameGroundHits(
  pos: [number, number, number],
  target: [number, number, number],
  planeY: number,
  fovDeg: number,
  aspect: number,
): Array<[number, number]> | null {
  const fx = target[0] - pos[0];
  const fy = target[1] - pos[1];
  const fz = target[2] - pos[2];
  const flen = Math.hypot(fx, fy, fz);
  if (flen < 1e-9) return null;
  const f: [number, number, number] = [fx / flen, fy / flen, fz / flen];

  // right = normalize(f × worldUp) with worldUp = (0, 1, 0), which reduces to (-f.z, 0, f.x); then
  // up' = right × f. Degenerate only when f is vertical, which `pitchDeg < 90` excludes.
  const clen = Math.hypot(f[2], f[0]);
  if (clen < 1e-9) return null;
  const r: [number, number, number] = [-f[2] / clen, 0, f[0] / clen];
  const u: [number, number, number] = [
    r[1] * f[2] - r[2] * f[1],
    r[2] * f[0] - r[0] * f[2],
    r[0] * f[1] - r[1] * f[0],
  ];

  const tanV = Math.tan((fovDeg * Math.PI) / 360);
  const tanH = tanV * aspect;

  const hits: Array<[number, number]> = [];
  for (let i = 0; i < FRAME_SAMPLES; i++) {
    const sv = (i / (FRAME_SAMPLES - 1)) * 2 - 1;
    for (let j = 0; j < FRAME_SAMPLES; j++) {
      const su = (j / (FRAME_SAMPLES - 1)) * 2 - 1;
      const dx = f[0] + r[0] * su * tanH + u[0] * sv * tanV;
      const dy = f[1] + r[1] * su * tanH + u[1] * sv * tanV;
      const dz = f[2] + r[2] * su * tanH + u[2] * sv * tanV;
      // Above the horizon, or parallel to the ground: no ground hit, so the frame contains sky and
      // therefore the far edge of the world.
      if (dy >= -1e-6) return null;
      const t = (planeY - pos[1]) / dy;
      if (t <= 0) return null;
      hits.push([pos[0] + dx * t, pos[2] + dz * t]);
    }
  }
  return hits;
}

/**
 * Place a camera so that `subject` is centred and the whole frame lands inside the world.
 *
 * Returns `null` when no azimuth satisfies the constraint at the requested distance and pitch —
 * which is information, not an error to swallow. The caller either moves closer, pitches down
 * harder, or documents why this particular view is allowed to see the horizon.
 */
export function solveInwardFraming(req: FramingRequest): FramingSolution | null {
  const {
    subject,
    distance,
    pitchDeg,
    aimAbove = 0,
    fovDeg = 55,
    aspect = 1280 / 720,
    footprintHalf,
    cameraMargin = 4,
    azimuthSteps = 64,
    requireFrameInside = true,
  } = req;

  const [sx, sy, sz] = subject;
  const target: [number, number, number] = [sx, sy + aimAbove, sz];
  const pitch = (pitchDeg * Math.PI) / 180;
  const dh = distance * Math.cos(pitch);
  const dv = distance * Math.sin(pitch);
  const camLimit = footprintHalf - cameraMargin;

  // Inward is "toward the middle of the map". At the exact middle every direction is inward, so the
  // degenerate case picks +x rather than dividing by zero.
  const sr = Math.hypot(sx, sz);
  const inX = sr > 1e-9 ? -sx / sr : 1;
  const inZ = sr > 1e-9 ? -sz / sr : 0;

  let best: FramingSolution | null = null;
  for (let k = 0; k < azimuthSteps; k++) {
    const az = (k / azimuthSteps) * Math.PI * 2;
    // Camera sits at azimuth `az` from the subject; it therefore *looks* along `-az`.
    const px = sx + Math.cos(az) * dh;
    const pz = sz + Math.sin(az) * dh;
    if (Math.abs(px) > camLimit || Math.abs(pz) > camLimit) continue;

    const pos: [number, number, number] = [px, sy + dv, pz];
    const hits = frameGroundHits(pos, target, sy, fovDeg, aspect);
    if (requireFrameInside && !hits) continue;

    let reach = 0;
    let inside = true;
    for (const [hx, hz] of hits ?? []) {
      if (Math.abs(hx) > footprintHalf || Math.abs(hz) > footprintHalf) {
        inside = false;
        if (requireFrameInside) break;
      }
      reach = Math.max(reach, Math.hypot(hx - px, hz - pz));
    }
    if (requireFrameInside && !inside) continue;

    const inwardness = -Math.cos(az) * inX - Math.sin(az) * inZ;
    if (!best || inwardness > best.inwardness) {
      best = { pose: { position: pos, target }, azimuth: az, inwardness, groundReach: reach };
    }
  }
  return best;
}

/**
 * Is `point` inside what this pose's camera actually shows?
 *
 * An exact frustum test, not an approximation of one: project the point onto the camera basis and
 * require its normalised image coordinates to be within the rectangle. Used to bind the navigation
 * evidence to the navigation image — the published route's goal was previously chosen by graph
 * distance alone, so the route ran off the bottom-right corner and the goal marker fell outside the
 * photograph. A reviewer could see a path and not where it ended, which is most of what a
 * reachability claim is.
 *
 * @param margin - Fraction of the half-extent to keep clear of each edge. The markers are pillars
 *   with real width and height; a point exactly on the frame boundary is half a marker outside it.
 */
export function isPointInFrame(
  pose: FramedPose,
  point: [number, number, number],
  fovDeg = 55,
  aspect = 1280 / 720,
  margin = 0.12,
): boolean {
  const [px, py, pz] = pose.position;
  const fx = pose.target[0] - px;
  const fy = pose.target[1] - py;
  const fz = pose.target[2] - pz;
  const flen = Math.hypot(fx, fy, fz);
  if (flen < 1e-9) return false;
  const f: [number, number, number] = [fx / flen, fy / flen, fz / flen];

  const clen = Math.hypot(f[2], f[0]);
  if (clen < 1e-9) return false;
  const r: [number, number, number] = [-f[2] / clen, 0, f[0] / clen];
  const u: [number, number, number] = [
    r[1] * f[2] - r[2] * f[1],
    r[2] * f[0] - r[0] * f[2],
    r[0] * f[1] - r[1] * f[0],
  ];

  const v: [number, number, number] = [point[0] - px, point[1] - py, point[2] - pz];
  const depth = v[0] * f[0] + v[1] * f[1] + v[2] * f[2];
  if (depth <= 1e-6) return false; // behind the camera

  const tanV = Math.tan((fovDeg * Math.PI) / 360);
  const tanH = tanV * aspect;
  const nx = (v[0] * r[0] + v[1] * r[1] + v[2] * r[2]) / (depth * tanH);
  const ny = (v[0] * u[0] + v[1] * u[1] + v[2] * u[2]) / (depth * tanV);
  const limit = 1 - margin;
  return Math.abs(nx) <= limit && Math.abs(ny) <= limit;
}

/**
 * Does this pose's frame land entirely inside the world footprint?
 *
 * The same predicate `solveInwardFraming` enforces, exported so a test can assert it of the
 * *committed* poses. A derivation that guarantees a property and a committed literal that has it are
 * different claims, and only the second one is what gets photographed.
 */
export function frameStaysInsideFootprint(
  pose: FramedPose,
  footprintHalf: number,
  fovDeg = 55,
  aspect = 1280 / 720,
): boolean {
  const hits = frameGroundHits(pose.position, pose.target, pose.target[1], fovDeg, aspect);
  if (!hits) return false;
  return hits.every(([x, z]) => Math.abs(x) <= footprintHalf && Math.abs(z) <= footprintHalf);
}
