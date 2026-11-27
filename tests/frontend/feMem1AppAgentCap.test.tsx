import { describe, it, expect, vi } from 'vitest';
import { render, screen, act, waitFor } from '@testing-library/react';
import { App } from '../../src/App';
import { emit } from '@tauri-apps/api/event';
import type { SegmentState } from '../../src/utils/agentHierarchy';

vi.setConfig({ testTimeout: 30_000 });

vi.mock('../../src/components/Landscape/utils/worldCache', () => ({
  loadOrGenerateWorld: vi.fn().mockResolvedValue({
    size: 64,
    shape: 'continent',
    elevation: new Float32Array(64 * 64),
    moisture: new Float32Array(64 * 64),
    temperature: new Float32Array(64 * 64),
    flow: new Float32Array(64 * 64),
    slope: new Float32Array(64 * 64),
    water: new Float32Array(64 * 64),
    riverAmt: new Uint8Array(64 * 64),
    shore: new Float32Array(64 * 64),
    biome: new Uint8Array(64 * 64),
    lakeBasins: [],
    seaLevel: 0.35,
    version: 1,
  }),
  getMemoizedWorld: vi.fn().mockReturnValue(null),
}));

const mockGraphicsMethods = {
  clear: vi.fn().mockReturnThis(),
  beginFill: vi.fn().mockReturnThis(),
  drawCircle: vi.fn().mockReturnThis(),
  drawPolygon: vi.fn().mockReturnThis(),
  endFill: vi.fn().mockReturnThis(),
  lineStyle: vi.fn().mockReturnThis(),
  moveTo: vi.fn().mockReturnThis(),
  lineTo: vi.fn().mockReturnThis(),
  drawRect: vi.fn().mockReturnThis(),
};

vi.mock('pixi.js', () => ({
  Application: vi.fn(function () {
    return {
      init: vi.fn().mockResolvedValue(undefined),
      canvas: document.createElement('canvas'),
      stage: { addChild: vi.fn(), removeChild: vi.fn() },
      renderer: {},
      ticker: { add: vi.fn(), remove: vi.fn() },
      destroy: vi.fn(),
    };
  }),
  Graphics: vi.fn(function () {
    return mockGraphicsMethods;
  }),
  Container: vi.fn(function () {
    return { addChild: vi.fn(), removeChild: vi.fn() };
  }),
}));

function buildAgentsPayload(agentCount: number): SegmentState[] {
  const segments: SegmentState[] = [];
  for (let agentId = 1; agentId <= agentCount; agentId += 1) {
    const rootId = (agentId - 1) * 3;
    const base = {
      agent_id: agentId,
      y: 1.0,
      z: Math.floor(agentId / 50) * 1.5,
      yaw: 0,
      pitch: 0,
      roll: 0,
      joint_anchor_x: 0,
      joint_anchor_y: 0,
      joint_anchor_z: 0,
      joint_axis_x: 0,
      joint_axis_y: 1,
      joint_axis_z: 0,
      energy: 100 - (agentId % 20),
      hydration: 75,
      head_direction: [1, 0, 0] as [number, number, number],
      agent_type: agentId % 2 === 0 ? ('predator' as const) : ('prey' as const),
    };
    segments.push(
      {
        ...base,
        segment_id: rootId,
        parent_segment_id: null,
        x: (agentId % 50) * 1.5,
        length: 0.9,
        radius: 0.3,
      },
      {
        ...base,
        segment_id: rootId + 1,
        parent_segment_id: rootId,
        x: (agentId % 50) * 1.5 + 0.5,
        length: 0.8,
        radius: 0.25,
      },
      {
        ...base,
        segment_id: rootId + 2,
        parent_segment_id: rootId + 1,
        x: (agentId % 50) * 1.5 + 1.0,
        length: 0.7,
        radius: 0.2,
      },
    );
  }
  return segments;
}

describe('FE-MEM-1 App telemetry list cap', () => {
  it('renders only the first 5 agents and keeps DOM bounded when 1,000 agents arrive', async () => {
    const { container } = render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 60));
    });

    const payload = buildAgentsPayload(1000);
    await act(async () => {
      await emit('simulation-tick', payload);
    });

    await waitFor(() => {
      expect(screen.getByText('Số Agents hoạt động: 1000')).toBeDefined();
    });

    const renderedAgentHeadings = screen.getAllByRole('heading', {
      level: 3,
      name: /^Agent #\d+/,
    });
    expect(renderedAgentHeadings).toHaveLength(5);
    expect(renderedAgentHeadings[0].textContent).toContain('Agent #1 ');
    expect(renderedAgentHeadings[4].textContent).toContain('Agent #5 ');

    const totalDomNodes = container.querySelectorAll('*').length;
    expect(totalDomNodes).toBeLessThan(500);
  });
});
