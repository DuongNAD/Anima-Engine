import { describe, it, expect } from 'vitest';
import { buildAgentHierarchy, type RenderSegment, type SegmentState } from '../utils/agentHierarchy';

// These cover the two behaviours the shipping implementation had and its test-fixture twin did not.
// While the suite imported the copy in `tests/mocks/`, neither branch below was reachable from any
// test, so both could have regressed silently.

// The fixture now has to supply every field, because `SegmentState` comes from the generated
// bindings rather than a hand-written copy that had made `hydration`, `head_direction` and
// `agent_type` optional. Filling them in is the point: a fixture that omits what the backend always
// sends is testing a shape nothing produces.
function seg(over: Partial<SegmentState> & Pick<SegmentState, 'agent_id' | 'segment_id'>): SegmentState {
  return {
    parent_segment_id: null,
    x: 0, y: 0, z: 0,
    yaw: 0, pitch: 0, roll: 0,
    joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
    joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
    energy: 0,
    hydration: 0,
    head_direction: [0, 0, 0],
    agent_type: null,
    ...over,
  };
}

describe('buildAgentHierarchy', () => {
  it('nests children under their parent segment', () => {
    const hierarchies = buildAgentHierarchy([
      seg({ agent_id: 7, segment_id: 0, parent_segment_id: null, energy: 42 }),
      seg({ agent_id: 7, segment_id: 1, parent_segment_id: 0 }),
      seg({ agent_id: 7, segment_id: 2, parent_segment_id: 1 }),
    ]);

    expect(hierarchies).toHaveLength(1);
    expect(hierarchies[0].agent_id).toBe(7);
    // Energy comes from the root segment, not from the last one seen.
    expect(hierarchies[0].energy).toBe(42);
    expect(hierarchies[0].root.segment_id).toBe(0);
    expect(hierarchies[0].root.children.map(c => c.segment_id)).toEqual([1]);
    expect(hierarchies[0].root.children[0].children.map(c => c.segment_id)).toEqual([2]);
  });

  it('separates segments belonging to different agents', () => {
    const hierarchies = buildAgentHierarchy([
      seg({ agent_id: 1, segment_id: 0 }),
      seg({ agent_id: 2, segment_id: 0 }),
      seg({ agent_id: 1, segment_id: 1, parent_segment_id: 0 }),
    ]);

    expect(hierarchies.map(h => h.agent_id).sort()).toEqual([1, 2]);
    const agent1 = hierarchies.find(h => h.agent_id === 1)!;
    const agent2 = hierarchies.find(h => h.agent_id === 2)!;
    expect(agent1.root.children).toHaveLength(1);
    expect(agent2.root.children).toHaveLength(0);
  });

  it('drops a parent link that would close a cycle instead of recursing forever', () => {
    // 0 -> 1 -> 2, and then 2 claims 0 as its child. Without the guard, wiring that last edge builds
    // a ring and the next traversal never terminates.
    const hierarchies = buildAgentHierarchy([
      seg({ agent_id: 1, segment_id: 0, parent_segment_id: null }),
      seg({ agent_id: 1, segment_id: 1, parent_segment_id: 0 }),
      seg({ agent_id: 1, segment_id: 2, parent_segment_id: 1 }),
      seg({ agent_id: 1, segment_id: 0, parent_segment_id: 2 }),
    ]);

    expect(hierarchies).toHaveLength(1);
    // Serializing is the assertion: a cycle would throw "Converting circular structure to JSON".
    expect(() => JSON.stringify(hierarchies[0].root)).not.toThrow();

    const seen = new Set<number>();
    // Typed as what it is handed. The local shape this used to declare was not `RenderSegment` and
    // did not describe its own recursion, so both call sites needed a cast to reach it.
    const walk = (node: RenderSegment): void => {
      expect(seen.has(node.segment_id)).toBe(false);
      seen.add(node.segment_id);
      node.children.forEach(walk);
    };
    walk(hierarchies[0].root);
  });

  it('tolerates a malformed payload rather than throwing', () => {
    // Exactly what the tick event can deliver: a partially-written array with holes and an entry
    // whose `agent_id` came back null. No cast — `buildAgentHierarchy` takes `unknown[]` because
    // this is the input it was written for.
    const malformed: readonly unknown[] = [
      null,
      undefined,
      seg({ agent_id: 1, segment_id: 0 }),
      { agent_id: null, segment_id: 9 },
    ];

    const hierarchies = buildAgentHierarchy(malformed);
    expect(hierarchies).toHaveLength(1);
    expect(hierarchies[0].agent_id).toBe(1);
  });

  it('returns an empty list for a non-array payload', () => {
    expect(buildAgentHierarchy(undefined)).toEqual([]);
    expect(buildAgentHierarchy(null)).toEqual([]);
  });

  it('drops a segment whose id is not a number rather than keying the tree on undefined', () => {
    // `RenderSegment.segment_id` is declared `number`. A payload that omits one used to be stored
    // under the key `undefined` and handed on with that field missing.
    const hierarchies = buildAgentHierarchy([
      { agent_id: 5, segment_id: 0, parent_segment_id: null },
      { agent_id: 5, parent_segment_id: 0 },
    ]);

    expect(hierarchies).toHaveLength(1);
    expect(hierarchies[0].root.segment_id).toBe(0);
    expect(hierarchies[0].root.children).toEqual([]);
  });

  it('coerces a non-numeric coordinate instead of copying it into a number field', () => {
    // The render tree feeds these straight into geometry. A string arriving from a hand-edited or
    // half-migrated payload used to pass through `|| 0` untouched.
    const hierarchies = buildAgentHierarchy([
      { agent_id: 6, segment_id: 0, parent_segment_id: null, x: 'nine', y: 3, energy: 'lots' },
    ]);

    expect(hierarchies[0].root.x).toBe(0);
    expect(hierarchies[0].root.y).toBe(3);
    expect(hierarchies[0].energy).toBe(0);
  });

  it('omits an agent whose segments contain no root', () => {
    // Every segment claims a parent, so there is no entry point into the tree.
    const hierarchies = buildAgentHierarchy([
      seg({ agent_id: 3, segment_id: 1, parent_segment_id: 0 }),
      seg({ agent_id: 3, segment_id: 2, parent_segment_id: 1 }),
    ]);
    expect(hierarchies).toEqual([]);
  });
});
