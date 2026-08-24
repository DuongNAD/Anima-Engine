// Complete `SegmentState` fixtures, built from the generated binding.
//
// # Why a builder rather than object literals
//
// `SegmentState` has nineteen required fields, and a test is usually about two of them: an agent id
// and a position. Writing the other seventeen out per fixture is what made every suite here write a
// five-field literal instead — which type-checks nowhere, and which is why several of them were
// handing `PixiViewport` a shape the backend cannot produce while claiming it was a tick payload.
//
// A fixture that omits what the backend always sends is testing a shape nothing produces. The
// builder is what makes "complete" free: name the fields the test is about, get the rest.
//
// # Why the generated type
//
// `src/types/generated/SegmentState` is what `ts-rs` derives from the Rust struct, so a field that
// changes on the backend fails here. The hand-written copy this replaces had already drifted — it
// made `hydration`, `head_direction` and `agent_type` optional, and `agent_type` a two-value string
// union rather than `AgentType | null` — and a drifted mock agrees with its consumer instead of with
// the producer, which is exactly the arrangement that lets a whole feature be silently dead.

import type { SegmentState } from '../../src/types/generated/SegmentState';

/**
 * A segment at rest at the origin, with every field the backend always sends.
 *
 * The values are deliberately neutral: a test that cares about one of them sets it, and a test that
 * does not should not be reading meaning into it.
 */
const AT_REST: SegmentState = {
  agent_id: 0,
  segment_id: 0,
  parent_segment_id: null,
  x: 0,
  y: 0,
  z: 0,
  yaw: 0,
  pitch: 0,
  roll: 0,
  joint_anchor_x: 0,
  joint_anchor_y: 0,
  joint_anchor_z: 0,
  joint_axis_x: 0,
  joint_axis_y: 0,
  joint_axis_z: 0,
  energy: 0,
  hydration: 0,
  head_direction: [0, 0, 0],
  agent_type: null,
};

/** A complete segment carrying the fields a test is about. */
export function segment(over: Partial<SegmentState> = {}): SegmentState {
  return { ...AT_REST, ...over };
}

/** A whole agent's worth of segments, each already complete. */
export function segments(...over: Array<Partial<SegmentState>>): SegmentState[] {
  return over.map(segment);
}
