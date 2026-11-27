// prototype, to be replaced — WV2-LOOK r11 smooth river centerline capsule SDF.

import { WaterKind, type WorldV2LoadedBundle } from './types';

/** Byte value in `riverMaskMap.r` representing `signedDistM === 0` (shoreline). */
export const RIVER_SDF_ZERO_BYTE = 80;

export interface RiverSdfWorldInput {
  size: number;
  extentM: number;
  waterKind: Uint8Array;
  heightM: Float32Array;
  waterHeightM: Float32Array;
  hydroFlowRgba: Uint8Array;
  normalRg: Uint8Array;
}

const DX8 = [1, 1, 0, -1, -1, -1, 0, 1];
const DY8 = [0, 1, 1, 1, 0, -1, -1, -1];
const D8_INV = DX8.map((_, k) => 1 / Math.hypot(DX8[k], DY8[k]));

interface RiverVertex {
  x: number;
  y: number;
  halfW: number;
  cue: number;
}

function isStandingWater(wk: number): boolean {
  return (
    wk === WaterKind.Ocean ||
    wk === WaterKind.FreshwaterLake ||
    wk === WaterKind.EndorheicLake
  );
}

function computeChannelHalfWidthM(
  texelM: number,
  qByte: number,
  depthM: number,
): number {
  // Leopold-Maddock width scaling (~sqrt(discharge)): small q=1 creeks shrink in shader via flowCue,
  // while keeping a continuous unbroken capsule SDF on the L1 grid.
  const qNorm = Math.min(1, Math.max(1, qByte) / 105);
  const leopoldW = 6 + Math.sqrt(qNorm) * 40 + Math.min(1, Math.max(0, depthM) / 4) * 6;
  return Math.max(texelM * 0.56, leopoldW);
}

function smoothAndSubdividePolyline(
  pts: readonly RiverVertex[],
): RiverVertex[] {
  if (pts.length <= 2) return pts.slice();

  // Preserve original coords for tight valley clamping (<= 1.4 texels drift).
  const orig = pts;
  let cur = pts.map((p) => ({ ...p }));

  for (let iter = 0; iter < 4; iter += 1) {
    const next = cur.map((p) => ({ ...p }));
    for (let k = 1; k < cur.length - 1; k += 1) {
      const lx = 0.25 * cur[k - 1].x + 0.5 * cur[k].x + 0.25 * cur[k + 1].x;
      const ly = 0.25 * cur[k - 1].y + 0.5 * cur[k].y + 0.25 * cur[k + 1].y;
      const ox = orig[k].x;
      const oy = orig[k].y;
      const dx = lx - ox;
      const dy = ly - oy;
      const d = Math.hypot(dx, dy);
      if (d > 1.4) {
        const s = 1.4 / d;
        next[k].x = ox + dx * s;
        next[k].y = oy + dy * s;
      } else {
        next[k].x = lx;
        next[k].y = ly;
      }
      // Enforce monotonic / smooth half-width downstream while preserving confluence continuity.
      next[k].halfW = Math.max(
        cur[k].halfW,
        0.25 * cur[k - 1].halfW + 0.5 * cur[k].halfW + 0.25 * cur[k + 1].halfW,
      );
      next[k].cue = 0.25 * cur[k - 1].cue + 0.5 * cur[k].cue + 0.25 * cur[k + 1].cue;
    }
    cur = next;
  }

  // One Chaikin corner-cutting pass, keeping both endpoints pinned so branches stay joined.
  const out: RiverVertex[] = [{ ...cur[0] }];
  for (let k = 0; k < cur.length - 1; k += 1) {
    const a = cur[k];
    const b = cur[k + 1];
    out.push({
      x: 0.75 * a.x + 0.25 * b.x,
      y: 0.75 * a.y + 0.25 * b.y,
      halfW: 0.75 * a.halfW + 0.25 * b.halfW,
      cue: 0.75 * a.cue + 0.25 * b.cue,
    });
    out.push({
      x: 0.25 * a.x + 0.75 * b.x,
      y: 0.25 * a.y + 0.75 * b.y,
      halfW: 0.25 * a.halfW + 0.75 * b.halfW,
      cue: 0.25 * a.cue + 0.75 * b.cue,
    });
  }
  out.push({ ...cur[cur.length - 1] });
  return out;
}

/**
 * Trace D8 river channels into smoothed polylines and rasterize a continuous capsule signed
 * distance field (SDF) into RGBA8 (`R = clamp(round(distM - halfW + 80), 0..255)`).
 */
export function buildSmoothedRiverMaskData(
  bundle: Pick<
    WorldV2LoadedBundle,
    | 'size'
    | 'extentM'
    | 'waterKind'
    | 'heightM'
    | 'waterHeightM'
    | 'hydroFlowRgba'
    | 'normalRg'
  >,
): Uint8Array {
  const { size, extentM, waterKind, heightM, waterHeightM, hydroFlowRgba, normalRg } = bundle;
  const count = size * size;
  const texelM = extentM / Math.max(1, size);
  const out = new Uint8Array(count * 4);

  const isChannel = new Uint8Array(count);
  const halfWidthArr = new Float32Array(count);
  const cueArr = new Float32Array(count);

  for (let i = 0; i < count; i += 1) {
    const o4 = i * 4;
    out[o4 + 1] = hydroFlowRgba[o4];
    out[o4 + 2] = hydroFlowRgba[o4 + 1];

    const wk = waterKind[i];
    if (isStandingWater(wk)) {
      out[o4] = 255;
      continue;
    }

    const ch = wk === WaterKind.River || wk === WaterKind.Waterfall || wk === WaterKind.DeltaTurbid;
    if (!ch) continue;

    isChannel[i] = 1;
    const q = hydroFlowRgba[o4 + 2];
    const depthM = Math.max(0.25, waterHeightM[i] - heightM[i]);
    halfWidthArr[i] = computeChannelHalfWidthM(texelM, q, depthM);
    const qNorm = Math.min(1, Math.max(0, q) / 110);
    // Base flow-energy cue (0..190); confluences, waterfalls & steep breaks boost into foam range (195..255).
    let cue = Math.round(28 + Math.sqrt(qNorm) * 162);
    if (wk === WaterKind.Waterfall) cue = Math.min(255, cue + 60);
    cueArr[i] = cue;
  }

  // Pick one deterministic D8 downstream neighbor per channel cell.
  const nextCell = new Int32Array(count).fill(-1);
  const inDegree = new Uint16Array(count);

  for (let y = 0; y < size; y += 1) {
    const row = y * size;
    for (let x = 0; x < size; x += 1) {
      const i = row + x;
      if (!isChannel[i]) continue;
      const o4 = i * 4;
      const ux = (hydroFlowRgba[o4] - 128) / 127;
      const uz = (hydroFlowRgba[o4 + 1] - 128) / 127;
      const uLen = Math.hypot(ux, uz);
      const h0 = heightM[i];

      let bestCh = -1;
      let bestChScore = -Infinity;
      let bestStand = -1;
      let bestStandScore = -Infinity;

      for (let d = 0; d < 8; d += 1) {
        const nx = x + DX8[d];
        const ny = y + DY8[d];
        if (nx < 0 || nx >= size || ny < 0 || ny >= size) continue;
        const ni = ny * size + nx;
        const dh = (h0 - heightM[ni]) * D8_INV[d];
        const dot =
          uLen > 0.15 ? ((DX8[d] * ux + DY8[d] * uz) * D8_INV[d]) / uLen : 0;
        const qn = hydroFlowRgba[ni * 4 + 2] / 255;

        if (isChannel[ni]) {
          // Prefer downhill flow alignment and rising downstream discharge.
          const sc = dh * 2.0 + dot * 1.5 + qn * 0.4;
          if (sc > bestChScore && (dh > -1.0 || dot > 0.35 || hydroFlowRgba[ni * 4 + 2] >= hydroFlowRgba[o4 + 2])) {
            bestChScore = sc;
            bestCh = ni;
          }
        } else if (isStandingWater(waterKind[ni])) {
          const sc = dh * 2.0 + dot * 1.5;
          if (sc > bestStandScore) {
            bestStandScore = sc;
            bestStand = ni;
          }
        }
      }

      if (bestCh >= 0 && (bestStand < 0 || bestChScore >= bestStandScore)) {
        nextCell[i] = bestCh;
        inDegree[bestCh] += 1;
      } else if (bestStand >= 0) {
        nextCell[i] = bestStand;
      }
    }
  }

  // Mark confluences & steep slope breaks with an aeration/foam boost in cueArr (alpha channel).
  for (let i = 0; i < count; i += 1) {
    if (!isChannel[i]) continue;
    const nxt = nextCell[i];
    const dropM = nxt >= 0 ? heightM[i] - heightM[nxt] : 0;
    const nx = (normalRg[i * 2] / 255) * 2 - 1;
    const nz = (normalRg[i * 2 + 1] / 255) * 2 - 1;
    const slope = 1 - Math.sqrt(Math.max(0, 1 - nx * nx - nz * nz));
    if (inDegree[i] >= 2 || dropM > 8 || slope > 0.16) {
      cueArr[i] = Math.min(255, Math.max(cueArr[i] + 55, 200));
    }
  }

  const distSdf = new Float32Array(count).fill(175.0);
  const segCue = new Float32Array(count);
  const bestDirX = new Float32Array(count);
  const bestDirZ = new Float32Array(count);
  const hasSegDir = new Uint8Array(count);

  const visitedStart = new Uint8Array(count);
  const polylines: RiverVertex[][] = [];

  const traceFrom = (startIdx: number): void => {
    const pts: RiverVertex[] = [];
    const localSeen = new Set<number>();
    let cur = startIdx;
    while (cur >= 0) {
      const cx = cur % size;
      const cy = Math.floor(cur / size);
      if (!isChannel[cur]) {
        // Connect terminal node onto standing water boundary using previous width/cue.
        const prev = pts[pts.length - 1];
        if (prev) {
          pts.push({ x: cx, y: cy, halfW: prev.halfW, cue: prev.cue });
        }
        break;
      }
      pts.push({
        x: cx,
        y: cy,
        halfW: halfWidthArr[cur],
        cue: cueArr[cur],
      });
      if (localSeen.has(cur)) break;
      localSeen.add(cur);
      visitedStart[cur] = 1;

      const nxt = nextCell[cur];
      if (nxt < 0) break;
      if (isChannel[nxt] && inDegree[nxt] >= 2) {
        // Join at confluence pixel cleanly, then stop so main trunk traces onward once.
        pts.push({
          x: nxt % size,
          y: Math.floor(nxt / size),
          halfW: Math.max(halfWidthArr[cur], halfWidthArr[nxt] * 0.85),
          cue: cueArr[nxt],
        });
        break;
      }
      cur = nxt;
    }
    if (pts.length >= 1) {
      if (inDegree[startIdx] === 0 && pts.length >= 4) {
        pts[0] = { ...pts[0], halfW: pts[0].halfW * 0.68 };
      }
      polylines.push(pts);
    }
  };

  // Start at headwaters and junctions first; then pick up any remaining cycles/fragments.
  for (let i = 0; i < count; i += 1) {
    if (isChannel[i] && (inDegree[i] === 0 || inDegree[i] >= 2)) {
      traceFrom(i);
    }
  }
  for (let i = 0; i < count; i += 1) {
    if (isChannel[i] && !visitedStart[i]) {
      traceFrom(i);
    }
  }

  for (let pIdx = 0; pIdx < polylines.length; pIdx += 1) {
    const rawLine = polylines[pIdx];
    const line = smoothAndSubdividePolyline(rawLine);
    const nPts = line.length;

    const segCount = nPts === 1 ? 1 : nPts - 1;
    for (let s = 0; s < segCount; s += 1) {
      const a = line[s];
      const b = nPts === 1 ? a : line[s + 1];
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const lenSq = dx * dx + dy * dy;
      const segLen = Math.sqrt(lenSq);
      const tx = segLen > 1e-4 ? dx / segLen : 0.7071;
      const tz = segLen > 1e-4 ? dy / segLen : 0.7071;

      const maxW = Math.max(a.halfW, b.halfW);
      const reachTex = Math.ceil((maxW + 44) / texelM);
      const minX = Math.max(0, Math.floor(Math.min(a.x, b.x)) - reachTex);
      const maxX = Math.min(size - 1, Math.ceil(Math.max(a.x, b.x)) + reachTex);
      const minY = Math.max(0, Math.floor(Math.min(a.y, b.y)) - reachTex);
      const maxY = Math.min(size - 1, Math.ceil(Math.max(a.y, b.y)) + reachTex);

      for (let py = minY; py <= maxY; py += 1) {
        const row = py * size;
        for (let px = minX; px <= maxX; px += 1) {
          const i = row + px;
          if (isStandingWater(waterKind[i])) continue;

          let t = 0;
          if (lenSq > 1e-8) {
            t = ((px - a.x) * dx + (py - a.y) * dy) / lenSq;
            if (t < 0) t = 0;
            else if (t > 1) t = 1;
          }
          const qx = a.x + t * dx;
          const qy = a.y + t * dy;
          const dM = Math.hypot(px - qx, py - qy) * texelM;
          const wM = a.halfW + (b.halfW - a.halfW) * t;
          const sdf = dM - wM;

          if (sdf < distSdf[i]) {
            distSdf[i] = sdf;
            segCue[i] = a.cue + (b.cue - a.cue) * t;
            bestDirX[i] = tx;
            bestDirZ[i] = tz;
            hasSegDir[i] = 1;
          }
        }
      }
    }
  }

  for (let i = 0; i < count; i += 1) {
    const o4 = i * 4;
    if (isStandingWater(waterKind[i])) {
      out[o4] = 255;
      out[o4 + 3] = 0;
      continue;
    }
    const qByte = Math.max(0, Math.min(255, Math.round(distSdf[i] + RIVER_SDF_ZERO_BYTE)));
    out[o4] = qByte;
    if (hasSegDir[i]) {
      out[o4 + 1] = Math.max(0, Math.min(255, Math.round(128 + bestDirX[i] * 127)));
      out[o4 + 2] = Math.max(0, Math.min(255, Math.round(128 + bestDirZ[i] * 127)));
      out[o4 + 3] = Math.max(0, Math.min(255, Math.round(segCue[i])));
    }
  }

  return out;
}

const smoothedRiverMaskCache = new WeakMap<WorldV2LoadedBundle, Uint8Array>();

export function getCachedSmoothedRiverMaskData(bundle: WorldV2LoadedBundle): Uint8Array {
  const hit = smoothedRiverMaskCache.get(bundle);
  if (hit) return hit;
  const built = buildSmoothedRiverMaskData(bundle);
  smoothedRiverMaskCache.set(bundle, built);
  return built;
}
