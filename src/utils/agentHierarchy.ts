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

export function buildAgentHierarchy(segments: SegmentState[]): AgentHierarchy[] {
  const safeSegments = (Array.isArray(segments) ? segments : []).filter(
    (seg): seg is SegmentState => seg !== null && seg !== undefined && typeof seg === 'object'
  );
  const agentsMap = new Map<number, SegmentState[]>();

  // Group segments by agent_id
  safeSegments.forEach(seg => {
    if (seg.agent_id === undefined || seg.agent_id === null) return;
    if (!agentsMap.has(seg.agent_id)) {
      agentsMap.set(seg.agent_id, []);
    }
    agentsMap.get(seg.agent_id)!.push(seg);
  });

  const hierarchies: AgentHierarchy[] = [];

  agentsMap.forEach((segs, agentId) => {
    const segmentMap = new Map<number, RenderSegment>();
    let rootSegment: RenderSegment | null = null;
    let rootEnergy = 0;

    // Initialize all render segments
    segs.forEach(s => {
      if (!s) return;
      const renderSeg: RenderSegment = {
        segment_id: s.segment_id,
        x: s.x || 0,
        y: s.y || 0,
        z: s.z || 0,
        yaw: s.yaw || 0,
        pitch: s.pitch || 0,
        roll: s.roll || 0,
        joint_anchor: [s.joint_anchor_x || 0, s.joint_anchor_y || 0, s.joint_anchor_z || 0],
        joint_axis: [s.joint_axis_x || 0, s.joint_axis_y || 0, s.joint_axis_z || 0],
        children: []
      };
      segmentMap.set(s.segment_id, renderSeg);
      if (s.parent_segment_id === null || s.parent_segment_id === undefined) {
        rootSegment = renderSeg;
        rootEnergy = s.energy || 0;
      }
    });

    // Wire up parent-child connections, preventing cycles
    segs.forEach(s => {
      if (!s) return;
      if (s.parent_segment_id !== null && s.parent_segment_id !== undefined) {
        const parent = segmentMap.get(s.parent_segment_id);
        const child = segmentMap.get(s.segment_id);
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
