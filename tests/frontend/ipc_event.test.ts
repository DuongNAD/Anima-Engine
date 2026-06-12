import { describe, it, expect } from 'vitest';
import { listen, emit } from '@tauri-apps/api/event';
import { SegmentState, mockSegmentStates } from '../mocks/mock_ipc_payloads';

describe('Tauri IPC Events - simulation-tick event stream', () => {
  it('F3: should successfully register listener and receive simulation-tick event payload', async () => {
    let receivedPayload: SegmentState[] | null = null;

    const unlisten = await listen<SegmentState[]>('simulation-tick', (event: any) => {
      receivedPayload = event.payload;
    });

    await emit('simulation-tick', mockSegmentStates);

    expect(receivedPayload).not.toBeNull();
    expect(Array.isArray(receivedPayload)).toBe(true);
    expect(receivedPayload!.length).toBe(3);
    expect(receivedPayload![0].agent_id).toBe(1);
    expect(receivedPayload![0].segment_id).toBe(0);
    expect(receivedPayload![0].x).toBe(10.0);
    expect(receivedPayload![0].energy).toBe(95.5);
    expect(receivedPayload![1].agent_id).toBe(1);
    expect(receivedPayload![1].segment_id).toBe(1);
    expect(receivedPayload![1].x).toBe(11.0);
    expect(receivedPayload![1].energy).toBe(95.5);
    expect(receivedPayload![2].agent_id).toBe(1);
    expect(receivedPayload![2].segment_id).toBe(2);
    expect(receivedPayload![2].x).toBe(12.0);
    expect(receivedPayload![2].energy).toBe(95.5);

    unlisten();
  });
});
