import { describe, it, expect } from 'vitest';
import {
  frameStaysInsideFootprint,
  solveInwardFraming,
} from '@/components/Landscape/utils/viewFraming';
import {
  CANONICAL_VIEW_CAMERAS,
  CANONICAL_XZ_EXTENT,
} from '@/components/Landscape/utils/mapManifest';

// The framing constraint, asserted of the committed poses rather than of the solver.
//
// Independent review rejected four of the eight first-accepted canonical views — `collision`,
// `water`, `biome_transition`, `ecosystem` — because each showed the hard square world boundary along
// the right edge. The cause was a fixed south-west camera offset: for any subject in the north-east it
// aims the lens at the two nearest edges of a finite terrain plane.
//
// `solveInwardFraming` replaces the offset with a constraint. But a solver that guarantees a property
// and a literal pasted into `mapManifest.ts` that has it are two different claims, and the second is
// the one that gets photographed — so the interesting tests here are the ones that read
// `CANONICAL_VIEW_CAMERAS`.

const HALF = CANONICAL_XZ_EXTENT / 2;

/** The two views whose subject is the horizon; see the note in `viewFraming.ts`. */
const HORIZON_EXEMPT = new Set(['overview', 'lighting']);

describe('canonical poses — the close-ups keep the world edge out of frame', () => {
  it('contains the frame of every non-exempt view inside the world footprint', () => {
    const failures: string[] = [];
    for (const [id, pose] of Object.entries(CANONICAL_VIEW_CAMERAS)) {
      if (HORIZON_EXEMPT.has(id)) continue;
      if (!frameStaysInsideFootprint(pose, HALF)) failures.push(id);
    }
    expect(
      failures,
      'these poses photograph ground outside the terrain plane, which renders as the hard cut edge ' +
        'four canonical views were rejected for. Re-run scripts/derive_view_cameras.ts.',
    ).toEqual([]);
  });

  it('names the exempt views explicitly rather than leaving the rule implicit', () => {
    // If an exemption were silent, a regression that pushed a close-up's horizon into frame would look
    // like the rule had always allowed it. Both exempt views are wide shots whose lateral spread at
    // the distance they reach is provably wider than this world.
    expect([...HORIZON_EXEMPT].sort()).toEqual(['lighting', 'overview']);
    for (const id of HORIZON_EXEMPT) {
      expect(CANONICAL_VIEW_CAMERAS[id as keyof typeof CANONICAL_VIEW_CAMERAS]).toBeDefined();
    }
  });

  it('aims every view at the interior, not away from it', () => {
    // Inwardness: +1 is looking straight at the middle of the map, -1 straight away from it. The old
    // fixed-offset poses had negative inwardness for subjects on the far side.
    for (const [id, pose] of Object.entries(CANONICAL_VIEW_CAMERAS)) {
      if (id === 'overview') continue; // looks at the origin from outside; direction is trivially inward
      const [px, , pz] = pose.position;
      const [tx, , tz] = pose.target;
      const vlen = Math.hypot(tx - px, tz - pz);
      const slen = Math.hypot(tx, tz);
      if (vlen < 1e-9 || slen < 1e-9) continue;
      const inwardness = ((tx - px) / vlen) * (-tx / slen) + ((tz - pz) / vlen) * (-tz / slen);
      expect(inwardness, `${id} inwardness`).toBeGreaterThan(0.5);
    }
  });

  it('keeps every camera over or near the terrain', () => {
    // `overview` has to sit outside the footprint to frame a 1200-unit map; nothing else should.
    for (const [id, pose] of Object.entries(CANONICAL_VIEW_CAMERAS)) {
      const [x, , z] = pose.position;
      const limit = id === 'overview' ? HALF * 1.5 : HALF;
      expect(Math.max(Math.abs(x), Math.abs(z)), `${id} camera distance from centre`).toBeLessThanOrEqual(
        limit,
      );
    }
  });
});

describe('the framing solver', () => {
  it('turns the camera inward for a subject near an edge', () => {
    const sol = solveInwardFraming({
      subject: [80, 5, 0],
      distance: 12,
      pitchDeg: 42,
      footprintHalf: HALF,
    });
    expect(sol).not.toBeNull();
    // Camera further out than the subject, so the interior is behind it.
    expect(sol!.pose.position[0]).toBeGreaterThan(80);
    expect(sol!.inwardness).toBeGreaterThan(0.9);
    expect(frameStaysInsideFootprint(sol!.pose, HALF)).toBe(true);
  });

  it('refuses a subject that cannot be photographed rather than returning the least bad pose', () => {
    // Six units from the edge is where the real `collision` subject sat: to look inward the camera
    // must stand between it and the edge, which is off the mesh, and the cut then crosses the
    // foreground. Returning something here is how the rejected images were produced.
    const sol = solveInwardFraming({
      subject: [HALF - 6, 5, 0],
      distance: 11,
      pitchDeg: 42,
      footprintHalf: HALF,
    });
    expect(sol).toBeNull();
  });

  it('accepts the same subject once the horizon requirement is dropped', () => {
    const sol = solveInwardFraming({
      subject: [HALF - 6, 5, 0],
      distance: 11,
      pitchDeg: 42,
      footprintHalf: HALF,
      requireFrameInside: false,
    });
    expect(sol).not.toBeNull();
    // Turned toward the interior, but only barely, and that is the arithmetic rather than a weak
    // solver: the camera must stay inside the footprint, the subject is six units from its edge, and
    // the horizontal offset is 8.2 units — so every azimuth that looks properly inward puts the camera
    // outside. The best available is close to tangential. This is why the close-up views take a
    // different *subject* instead of relaxing the constraint.
    expect(sol!.inwardness).toBeGreaterThan(0);
    expect(sol!.inwardness).toBeLessThan(0.5);
  });

  it('rejects a pitch that puts the horizon in frame', () => {
    // The scene's vertical half-FOV is 27.5°. At 20° of pitch the frame's top edge is above
    // horizontal, so it contains sky — and with sky comes whatever the terrain plane does when it ends.
    const shallow = solveInwardFraming({
      subject: [0, 5, 0],
      distance: 20,
      pitchDeg: 20,
      footprintHalf: HALF,
    });
    expect(shallow).toBeNull();

    const steep = solveInwardFraming({
      subject: [0, 5, 0],
      distance: 20,
      pitchDeg: 42,
      footprintHalf: HALF,
    });
    expect(steep).not.toBeNull();
  });

  it('is deterministic', () => {
    const req = {
      subject: [-40, 8, 25] as [number, number, number],
      distance: 19,
      pitchDeg: 42,
      footprintHalf: HALF,
    };
    expect(solveInwardFraming(req)).toEqual(solveInwardFraming(req));
  });

  it('handles a subject at the exact middle of the map', () => {
    // Degenerate: every direction is inward, and the outward normal is undefined. It must pick one
    // rather than divide by zero.
    const sol = solveInwardFraming({
      subject: [0, 3, 0],
      distance: 15,
      pitchDeg: 42,
      footprintHalf: HALF,
    });
    expect(sol).not.toBeNull();
    expect(Number.isFinite(sol!.pose.position[0])).toBe(true);
    expect(Number.isFinite(sol!.pose.position[2])).toBe(true);
  });

  it('rejects the old fixed south-west offset for a north-east subject', () => {
    // A regression control on the defect itself. This is the shape of the pose that produced
    // `collision.png`, `water.png`, `biome_transition.png` and `ecosystem.png`.
    const subject: [number, number, number] = [70, 6, 70];
    const bad = {
      position: [subject[0] - 7, subject[1] + 5, subject[2] - 7] as [number, number, number],
      target: subject,
    };
    expect(frameStaysInsideFootprint(bad, HALF)).toBe(false);
  });
});
