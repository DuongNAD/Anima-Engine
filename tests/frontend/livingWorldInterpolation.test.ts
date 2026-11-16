import { describe, expect, it } from 'vitest';
import {
  lerpAngle,
  lerpVec3,
  lerpVec3Into,
  packAgentSegmentKey,
  slerpQuatTupleInto,
} from '@/components/Landscape/agentInterpolation';

// M0 — test ĐỎ khoá hợp đồng living-world (2026-08-25, nhánh `feat/living-world`).
// N1 — nội suy tick ↔ frame (tasks §2, N1-1): giữ hai payload, lerp vị trí,
// slerp xoay theo alpha; giống nhau ở 30/60/144 Hz (AN-S02).
//
// Đỏ vì module `agentInterpolation.ts` CHƯA tồn tại — LiveAgents.tsx đang vẽ
// thẳng payload mới nhất nên giật ở tick rate thấp. Khi N1 landed (hàm thuần
// được tách ra để test được), các test này chuyển xanh.

describe('N1 agentInterpolation (đỏ cho tới khi N1-1 tách hàm thuần)', () => {
  it('lerpVec3 nội suy tuyến tính giữa hai payload tại alpha = 0.5', () => {
    const a = { x: 0, y: 0, z: 0 };
    const b = { x: 10, y: -4, z: 6 };
    expect(lerpVec3(a, b, 0.5)).toEqual({ x: 5, y: -2, z: 3 });
  });

  it('lerpVec3 tại alpha = 0 và alpha = 1 trả về đúng hai đầu', () => {
    const a = { x: 1, y: 2, z: 3 };
    const b = { x: -3, y: -2, z: -1 };
    expect(lerpVec3(a, b, 0)).toEqual(a);
    expect(lerpVec3(a, b, 1)).toEqual(b);
  });

  it('lerpAngle đi theo cung ngắn, không quay vòng dài', () => {
    // 350° -> 10° phải đi +20° qua 0, không phải -340°.
    expect(lerpAngle(350, 10, 0.5)).toBeCloseTo(0, 5);
    expect(lerpAngle(350, 10, 0)).toBeCloseTo(350, 5);
    expect(lerpAngle(350, 10, 1)).toBeCloseTo(10, 5);
  });

  it('alpha kẹp vào [0, 1] — không ngoại suy quá 1 tick (N1-2)', () => {
    const a = { x: 0, y: 0, z: 0 };
    const b = { x: 10, y: 0, z: 0 };
    expect(lerpVec3(a, b, -0.5)).toEqual(a);
    expect(lerpVec3(a, b, 1.5)).toEqual(b);
  });

  it('lerpVec3Into mutates target in-place without allocating and clamps alpha (RW3-3)', () => {
    const a = { x: 2, y: 4, z: -6 };
    const b = { x: 10, y: -4, z: 2 };
    const out = { x: 0, y: 0, z: 0 };
    const ret = lerpVec3Into(a, b, 0.25, out);
    expect(ret).toBe(out);
    expect(out).toEqual({ x: 4, y: 2, z: -4 });
    lerpVec3Into(a, b, 2.0, out);
    expect(out).toEqual(b);
  });

  it('slerpQuatTupleInto takes shortest arc and keeps unit length (RW3-1)', () => {
    const qA: [number, number, number, number] = [0, 0, 0, 1];
    // Antithetic quaternion sign flip for 90-degree Y rotation: dot < 0 must flip to shortest arc.
    const halfPiHalf = Math.sin(Math.PI / 4);
    const cosPiHalf = Math.cos(Math.PI / 4);
    const qBNeg: [number, number, number, number] = [0, -halfPiHalf, 0, -cosPiHalf];
    const out: [number, number, number, number] = [0, 0, 0, 1];
    slerpQuatTupleInto(qA, qBNeg, 0.5, out);

    // Midpoint is 45 degrees around +Y; qw stays positive and norm stays unit.
    const expectedSin = Math.sin(Math.PI / 8);
    const expectedCos = Math.cos(Math.PI / 8);
    expect(out[1]).toBeCloseTo(expectedSin, 5);
    expect(out[3]).toBeCloseTo(expectedCos, 5);
    expect(Math.hypot(out[0], out[1], out[2], out[3])).toBeCloseTo(1, 6);
  });

  it('packAgentSegmentKey maps distinct (agent_id, segment_id) pairs uniquely (RW3-3)', () => {
    const k1 = packAgentSegmentKey(1, 2);
    const k2 = packAgentSegmentKey(1, 3);
    const k3 = packAgentSegmentKey(2, 2);
    expect(k1).not.toBe(k2);
    expect(k1).not.toBe(k3);
    expect(k2).not.toBe(k3);
  });
});