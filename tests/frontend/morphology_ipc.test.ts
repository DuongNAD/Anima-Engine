import { describe, it, expect } from 'vitest';
import { listen, emit } from '@tauri-apps/api/event';
import { SegmentState, buildAgentHierarchy, mockSegmentStates } from '../mocks/mock_ipc_payloads';

describe('Frontend Morphology Stream IPC Integration', () => {
  it('F3_morphology: should receive flat segment payload and reconstruct correct hierarchy', async () => {
    let receivedPayload: SegmentState[] | null = null;

    // Listen to simulation ticks
    const unlisten = await listen<SegmentState[]>('simulation-tick', (event: any) => {
      receivedPayload = event.payload;
    });

    // Emit event with mockSegmentStates
    await emit('simulation-tick', mockSegmentStates);

    expect(receivedPayload).not.toBeNull();
    expect(receivedPayload!.length).toBe(3);

    // Reconstruct hierarchy
    const hierarchies = buildAgentHierarchy(receivedPayload!);
    expect(hierarchies.length).toBe(1);

    const agent = hierarchies[0];
    expect(agent.agent_id).toBe(1);
    expect(agent.energy).toBe(95.5);

    // Check root segment
    expect(agent.root.segment_id).toBe(0);
    expect(agent.root.children.length).toBe(1);

    // Check second segment (child of root)
    const child1 = agent.root.children[0];
    expect(child1.segment_id).toBe(1);
    expect(child1.joint_anchor).toEqual([1.0, 0.0, 0.0]);
    expect(child1.joint_axis).toEqual([0.0, 0.0, 1.0]);
    expect(child1.children.length).toBe(1);

    // Check third segment (child of second segment)
    const child2 = child1.children[0];
    expect(child2.segment_id).toBe(2);
    expect(child2.joint_anchor).toEqual([1.0, 0.0, 0.0]);
    expect(child2.joint_axis).toEqual([0.0, 0.0, 1.0]);
    expect(child2.children.length).toBe(0);

    unlisten();
  });
});
