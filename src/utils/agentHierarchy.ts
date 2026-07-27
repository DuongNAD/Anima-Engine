// The single implementation of the flat-segments → render-tree transform.
//
// It used to exist three times: in `App.tsx` (the copy that actually ships), in the now-deleted
// `hooks/useSimulation.ts`, and in `tests/mocks/mock_ipc_payloads.ts`. The test suite imported the
// mocks copy, so what it verified was a fourth-wall fixture rather than the shipping code — and the
// copies had drifted apart in exactly the way that arrangement invites. The mocks copy had no cycle
// guard and no null tolerance, so `agent_id: null` from a partially-written payload threw, and a
// `parent_segment_id` cycle recursed until the stack blew. Neither was reachable from any test.
//
// `SegmentState` is an IPC payload and comes from the generated bindings. It was declared here by
// hand as well, under a comment claiming these are all frontend-only view models — and it had
// drifted: this copy said `agent_type?: 'predator' | 'prey'`, `hydration?: number` and
// `head_direction?: [number, number, number]`, while the Rust struct has `agent_type: AgentType |
// null` and both of the others required. Two of those differences change what a consumer must
// handle: `null` is not `undefined`, and an optional field is not a required one.
//
// Re-exported rather than merely imported, because `App.tsx` and the frontend suite import the name
// from here. An alias has no field list of its own to drift.
export type { SegmentState } from '../types/generated/SegmentState';
import type { SegmentState } from '../types/generated/SegmentState';

// The rest of this file IS frontend-only: these view models are built in the browser and never
// cross IPC, so there is no Rust counterpart and nothing for ts-rs to keep in sync.

export interface RenderSegment {
  segment_id: number;
  x: number;
  y: number;
  z: number;
  yaw: number;
  pitch: number;
  roll: number;
  joint_anchor: [number, number, number];
  joint_axis: [number, number, number];
  children: RenderSegment[];
}

export interface AgentHierarchy {
  agent_id: number;
  energy: number;
  root: RenderSegment;
}

/**
 * The root segment (the one with no parent) of each agent, keyed by agent id.
 *
 * The telemetry panel needs two fields — `hydration` and `head_direction` — that the render tree
 * deliberately does not carry, so it used to reach for the newest raw segment array held in a ref
 * and search it *during render*. That is the pattern `react-hooks/refs` rejects, and the reason is
 * visible in this exact case: the ref held a newer tick than the throttled `hierarchies` state the
 * panel was iterating, so the row for an agent and the hydration printed inside it could come from
 * two different ticks. Indexing here, from the same array `buildAgentHierarchy` is given, makes the
 * whole panel one snapshot.
 *
 * First match wins, matching the `find` this replaced: a payload with two parentless segments for
 * one agent is malformed, and picking the earlier one is what the panel already showed.
 */
export function indexRootSegments(segments: SegmentState[]): Record<number, SegmentState> {
  const roots: Record<number, SegmentState> = {};
  for (const seg of Array.isArray(segments) ? segments : []) {
    if (!seg || typeof seg !== 'object') continue;
    if (seg.agent_id === undefined || seg.agent_id === null) continue;
    if (seg.parent_segment_id !== null && seg.parent_segment_id !== undefined) continue;
    if (roots[seg.agent_id] === undefined) roots[seg.agent_id] = seg;
  }
  return roots;
}

// ---------------------------------------------------------------------------------------
// Reading a tick payload
//
// `buildAgentHierarchy` was always written for untrusted input — it skipped nulls, non-objects and
// segments with no `agent_id`, and it guards against a `parent_segment_id` cycle — but its parameter
// said `SegmentState[]`, so the only way to reach any of those branches was to hold a cast. That is
// the wrong way round: the tolerance is the feature, and the signature was hiding it.
//
// It is not hypothetical tolerance either. `App.tsx` fills this from the `simulation-tick` event,
// whose `payload` crosses IPC as JSON; the array it hands over is *typed* `SegmentState[]` by a
// predicate that only checks the elements are objects. Whatever the backend serialised is what
// arrives, so every field below is read as `unknown` and coerced, rather than trusted.
// ---------------------------------------------------------------------------------------

/**
 * `value` viewed as the bag of unchecked fields a parsed JSON object actually is.
 *
 * Sound as a type predicate, unlike a claim that some object *is* a `SegmentState`: reading any
 * string key off a non-null object really does yield `unknown`, and `unknown` is what forces the
 * coercions below.
 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/**
 * A numeric field, or `0`.
 *
 * `value || 0` on its own is what this file used to do, and it is right about numbers — it maps
 * `NaN` and `-0` to `0`, which is what the render tree wants — but it passes a string straight
 * through into a field typed `number`. The `typeof` is what closes that.
 */
function numField(value: unknown): number {
  return typeof value === 'number' ? value || 0 : 0;
}

/** An id field, or `null` when the payload did not carry a usable one. */
function idField(value: unknown): number | null {
  return typeof value === 'number' ? value : null;
}

/** No parent link at all — the segment is its agent's root. Absent and explicit-null both count. */
function hasNoParent(value: unknown): boolean {
  return value === null || value === undefined;
}

export function buildAgentHierarchy(
  segments: readonly unknown[] | null | undefined
): AgentHierarchy[] {
  const raw: readonly unknown[] = Array.isArray(segments) ? segments : [];
  const safeSegments = raw.filter(isRecord);
  const agentsMap = new Map<number, Record<string, unknown>[]>();

  // Group segments by agent_id
  safeSegments.forEach(seg => {
    const agentId = idField(seg.agent_id);
    if (agentId === null) return;
    const existing = agentsMap.get(agentId);
    if (existing) existing.push(seg);
    else agentsMap.set(agentId, [seg]);
  });

  const hierarchies: AgentHierarchy[] = [];

  agentsMap.forEach((segs, agentId) => {
    const segmentMap = new Map<number, RenderSegment>();
    let rootSegment: RenderSegment | null = null;
    let rootEnergy = 0;

    // Initialize all render segments
    segs.forEach(s => {
      const segmentId = idField(s.segment_id);
      // A segment with no usable id has no place in a tree keyed by id: it used to be stored under
      // `undefined` and to hand that on as `RenderSegment.segment_id`, which is declared `number`.
      if (segmentId === null) return;
      const renderSeg: RenderSegment = {
        segment_id: segmentId,
        x: numField(s.x),
        y: numField(s.y),
        z: numField(s.z),
        yaw: numField(s.yaw),
        pitch: numField(s.pitch),
        roll: numField(s.roll),
        joint_anchor: [
          numField(s.joint_anchor_x),
          numField(s.joint_anchor_y),
          numField(s.joint_anchor_z),
        ],
        joint_axis: [numField(s.joint_axis_x), numField(s.joint_axis_y), numField(s.joint_axis_z)],
        children: []
      };
      segmentMap.set(segmentId, renderSeg);
      if (hasNoParent(s.parent_segment_id)) {
        rootSegment = renderSeg;
        rootEnergy = numField(s.energy);
      }
    });

    // Wire up parent-child connections, preventing cycles
    segs.forEach(s => {
      const parentId = idField(s.parent_segment_id);
      const segmentId = idField(s.segment_id);
      if (parentId === null || segmentId === null) return;
      const parent = segmentMap.get(parentId);
      const child = segmentMap.get(segmentId);
      if (parent && child) {
        const wouldCreateCycle = (node: RenderSegment, targetId: number): boolean => {
          if (node.segment_id === targetId) return true;
          for (const c of node.children) {
            if (wouldCreateCycle(c, targetId)) return true;
          }
          return false;
        };
        if (!wouldCreateCycle(child, parent.segment_id)) {
          parent.children.push(child);
        }
      }
    });

    if (rootSegment) {
      hierarchies.push({
        agent_id: agentId,
        energy: rootEnergy,
        root: rootSegment
      });
    }
  });

  return hierarchies;
}
