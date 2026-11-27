// prototype, to be replaced — WV2-LOOK r14 unified river polyline network, Leopold-Maddock widths & spatial index.

import { WaterKind, type WorldV2LoadedBundle } from './types';

/** Byte value in `riverMaskMap.r` representing `signedDistM === 0` (shoreline). */
export const RIVER_SDF_ZERO_BYTE = 80;

/** Leopold-Maddock channel width coefficient `W = W_min + K * sqrt(Q_eff)`. */
export const RIVER_WIDTH_COEFF_K = 2.6;
export const RIVER_WIDTH_MIN_M = 1.2;
export const RIVER_WIDTH_MAX_M = 68.0;

export type RiverMaskInput = Pick<
  WorldV2LoadedBundle,
  | 'size'
  | 'extentM'
  | 'waterKind'
  | 'heightM'
  | 'waterHeightM'
  | 'hydroFlowRgba'
  | 'normalRg'
>;

export interface RiverPoint {
  x: number;
  y: number;
  halfW: number;
  q: number;
  alongM: number;
  grade: number;
  localGrade: number;
  strahler: number;
  signedCurv: number;
  bankDropM: number;
  cue: number;
  isWaterfall: boolean;
}

export interface RiverNearestHit {
  distM: number;
  signedSideM: number;
  halfW: number;
  bankDropM: number;
  grade: number;
  localGrade: number;
  strahler: number;
  q: number;
  /** Positive on the INSIDE of a bend (point bar), negative on the OUTSIDE (cut bank). */
  bendSide: number;
  dirX: number;
  dirZ: number;
  projU: number;
  projV: number;
  isWaterfall: boolean;
}

export function isTrunkRiver(halfW: number, strahler: number): boolean {
  return strahler >= 3 || halfW * 2 >= 10.0;
}

export interface RiverWidthBucket {
  label: string;
  beforeCount: number;
  afterCount: number;
}

export interface RiverWidthHistogram {
  totalCells: number;
  beforeMinM: number;
  beforeP50M: number;
  beforeP90M: number;
  beforeMaxM: number;
  afterMinM: number;
  afterP50M: number;
  afterP90M: number;
  afterMaxM: number;
  buckets: RiverWidthBucket[];
}

export interface RiverNetwork {
  size: number;
  extentM: number;
  texelM: number;
  polylines: RiverPoint[][];
  queryNearestRiver: (u: number, v: number, maxDistM?: number) => RiverNearestHit | null;
}

interface RawVertex {
  x: number;
  y: number;
  halfW: number;
  q: number;
  grade: number;
  localGrade: number;
  strahler: number;
  cue: number;
  isWaterfall: boolean;
}

const DX8 = [1, 1, 0, -1, -1, -1, 0, 1];
const DY8 = [0, 1, 1, 1, 0, -1, -1, -1];
const D8_INV = DX8.map((_, k) => 1 / Math.hypot(DX8[k], DY8[k]));

function smoothstep01(edge0: number, edge1: number, x: number): number {
  const t = Math.max(0, Math.min(1, (x - edge0) / Math.max(1e-6, edge1 - edge0)));
  return t * t * (3 - 2 * t);
}

function clamp(x: number, lo: number, hi: number): number {
  return x < lo ? lo : x > hi ? hi : x;
}

function isStandingWater(wk: number): boolean {
  return (
    wk === WaterKind.Ocean ||
    wk === WaterKind.FreshwaterLake ||
    wk === WaterKind.EndorheicLake
  );
}

export function isChannelWater(wk: number): boolean {
  return wk === WaterKind.River || wk === WaterKind.Waterfall || wk === WaterKind.DeltaTurbid;
}

/**
 * Physical channel width (metres) from baked `hydro_flow.b` (`qByte`), accounting for mip averaging.
 * Headwaters (`q=1..14` on L1) map cleanly to `1.2..4.0 m` with no `23 m` floor.
 */
export function computeRiverWidthM(
  qByte: number,
  texelM: number,
  slope: number,
  waterKind: number,
): number {
  const baseMipScale = Math.max(1, texelM / 20);
  // 2x2 mip averaging mixes 1-texel L0 river trunks with 2-3 dry bank texels at L1+; boost
  // reconstruction progressively for main stems (q > 10) while keeping headwaters in 1.2-4.8 m.
  const stemBoost = baseMipScale > 1 ? 0.95 * smoothstep01(10, 62, qByte) : 0;
  const mipScale = baseMipScale + stemBoost;
  const log2Q = (Math.min(255, Math.max(0, qByte) * mipScale) / 255) * 10;
  const qEff = Math.pow(2, log2Q) - 1;
  let w = RIVER_WIDTH_MIN_M + RIVER_WIDTH_COEFF_K * Math.sqrt(Math.max(0, qEff));
  const steepNarrow = qByte < 18 ? 0.40 * smoothstep01(0.10, 0.26, slope) : 0.15 * smoothstep01(0.14, 0.32, slope);
  w *= 1 - steepNarrow;
  if (waterKind === WaterKind.DeltaTurbid) w *= 1.35;
  return clamp(w, RIVER_WIDTH_MIN_M, RIVER_WIDTH_MAX_M);
}

function legacyR13WidthM(texelM: number, qByte: number, depthM: number): number {
  const qNorm = Math.min(1, Math.max(1, qByte) / 105);
  const leopoldW = 6 + Math.sqrt(qNorm) * 40 + Math.min(1, Math.max(0, depthM) / 4) * 6;
  return Math.max(texelM * 0.56, leopoldW) * 2;
}

/** Computes exact before (R13) vs after (R14-0) channel width histogram and percentiles. */
export function computeRiverWidthHistogram(bundle: RiverMaskInput): RiverWidthHistogram {
  const { size, extentM, waterKind, heightM, waterHeightM, hydroFlowRgba, normalRg } = bundle;
  const texelM = extentM / Math.max(1, size);
  const count = size * size;
  const beforeArr: number[] = [];
  const afterArr: number[] = [];
  const bounds = [2, 5, 10, 20, 40, Infinity];
  const labels = ['<2 m', '2–5 m', '5–10 m', '10–20 m', '20–40 m', '>=40 m'];
  const beforeCounts = new Int32Array(bounds.length);
  const afterCounts = new Int32Array(bounds.length);

  for (let i = 0; i < count; i += 1) {
    const wk = waterKind[i];
    if (!isChannelWater(wk)) continue;
    const q = hydroFlowRgba[i * 4 + 2];
    const depthM = Math.max(0.25, waterHeightM[i] - heightM[i]);
    const nx = (normalRg[i * 2] / 255) * 2 - 1;
    const nz = (normalRg[i * 2 + 1] / 255) * 2 - 1;
    const slope = 1 - Math.sqrt(Math.max(0, 1 - nx * nx - nz * nz));

    const bw = legacyR13WidthM(texelM, q, depthM);
    const aw = computeRiverWidthM(q, texelM, slope, wk);
    beforeArr.push(bw);
    afterArr.push(aw);

    for (let b = 0; b < bounds.length; b += 1) {
      if (bw < bounds[b]) {
        beforeCounts[b] += 1;
        break;
      }
    }
    for (let b = 0; b < bounds.length; b += 1) {
      if (aw < bounds[b]) {
        afterCounts[b] += 1;
        break;
      }
    }
  }

  beforeArr.sort((a, b) => a - b);
  afterArr.sort((a, b) => a - b);
  const pct = (arr: number[], p: number): number =>
    arr.length === 0 ? 0 : arr[Math.min(arr.length - 1, Math.floor(arr.length * p))];

  return {
    totalCells: afterArr.length,
    beforeMinM: pct(beforeArr, 0),
    beforeP50M: pct(beforeArr, 0.5),
    beforeP90M: pct(beforeArr, 0.9),
    beforeMaxM: pct(beforeArr, 1),
    afterMinM: pct(afterArr, 0),
    afterP50M: pct(afterArr, 0.5),
    afterP90M: pct(afterArr, 0.9),
    afterMaxM: pct(afterArr, 1),
    buckets: labels.map((label, idx) => ({
      label,
      beforeCount: beforeCounts[idx],
      afterCount: afterCounts[idx],
    })),
  };
}

function thinCollinearLowQ(pts: readonly RawVertex[]): RawVertex[] {
  if (pts.length <= 4) return pts.slice();
  const out: RawVertex[] = [pts[0]];
  for (let i = 1; i < pts.length - 1; i += 1) {
    const prev = out[out.length - 1];
    const cur = pts[i];
    const next = pts[i + 1];
    // Preserve high-q stems, waterfalls, and steep slope breaks at full polyline density.
    if (cur.q > 3 || cur.isWaterfall || Math.abs(cur.grade - prev.grade) > 0.04) {
      out.push(cur);
      continue;
    }
    const dx = next.x - prev.x;
    const dy = next.y - prev.y;
    const len = Math.hypot(dx, dy);
    if (len < 1e-4 || len > 3.5) {
      out.push(cur);
      continue;
    }
    const perpErr = Math.abs((cur.x - prev.x) * dy - (cur.y - prev.y) * dx) / len;
    if (perpErr >= 0.14) {
      out.push(cur);
    }
  }
  out.push(pts[pts.length - 1]);
  return out;
}

function smoothAndSubdividePolyline(
  pts: readonly RawVertex[],
  onLaplacian?: (smoothed: readonly RawVertex[]) => void,
): RawVertex[] {
  if (pts.length <= 2) {
    onLaplacian?.(pts);
    return pts.slice();
  }

  const orig = pts;
  let cur = pts.map((p) => ({ ...p }));
  let maxHalfW = 0;
  let maxOrd = 1;
  for (let k = 0; k < cur.length; k += 1) {
    if (cur[k].halfW > maxHalfW) maxHalfW = cur[k].halfW;
    if (cur[k].strahler > maxOrd) maxOrd = cur[k].strahler;
  }

  // Pull centerline gently on trunk/curved channels so S-bends arc cleanly without zig-zag corners.
  const lapIters = isTrunkRiver(maxHalfW, maxOrd) || maxHalfW >= 3.5 ? 6 : 4;
  const maxDisp = lapIters === 6 ? 1.45 : 1.2;
  for (let iter = 0; iter < lapIters; iter += 1) {
    const next = cur.map((p) => ({ ...p }));
    for (let k = 1; k < cur.length - 1; k += 1) {
      const lx = 0.25 * cur[k - 1].x + 0.5 * cur[k].x + 0.25 * cur[k + 1].x;
      const ly = 0.25 * cur[k - 1].y + 0.5 * cur[k].y + 0.25 * cur[k + 1].y;
      const ox = orig[k].x;
      const oy = orig[k].y;
      const dx = lx - ox;
      const dy = ly - oy;
      const d = Math.hypot(dx, dy);
      if (d > maxDisp) {
        const s = maxDisp / d;
        next[k].x = ox + dx * s;
        next[k].y = oy + dy * s;
      } else {
        next[k].x = lx;
        next[k].y = ly;
      }
      next[k].halfW = Math.max(
        cur[k].halfW * (cur[k].grade > 0.12 ? 1.0 : 0.96),
        0.25 * cur[k - 1].halfW + 0.5 * cur[k].halfW + 0.25 * cur[k + 1].halfW,
      );
      next[k].grade = 0.25 * cur[k - 1].grade + 0.5 * cur[k].grade + 0.25 * cur[k + 1].grade;
      next[k].cue = 0.25 * cur[k - 1].cue + 0.5 * cur[k].cue + 0.25 * cur[k + 1].cue;
    }
    cur = next;
  }
  onLaplacian?.(cur);

  // Thin collinear low-q reaches before Chaikin corner cutting so triangle counts stay modest.
  let stage = thinCollinearLowQ(cur);
  // Fluidly curve wide sinuous rivers (2nd Chaikin pass on W >= 11 m keeps segment chord <= ~12-18 m).
  const passes = maxHalfW >= 5.5 ? 2 : 1;
  for (let pass = 0; pass < passes; pass += 1) {
    const sub: RawVertex[] = [{ ...stage[0] }];
    for (let k = 0; k < stage.length - 1; k += 1) {
      const a = stage[k];
      const b = stage[k + 1];
      sub.push({
        x: 0.75 * a.x + 0.25 * b.x,
        y: 0.75 * a.y + 0.25 * b.y,
        halfW: 0.75 * a.halfW + 0.25 * b.halfW,
        q: 0.75 * a.q + 0.25 * b.q,
        grade: 0.75 * a.grade + 0.25 * b.grade,
        localGrade: 0.75 * a.localGrade + 0.25 * b.localGrade,
        strahler: Math.max(a.strahler, b.strahler),
        cue: 0.75 * a.cue + 0.25 * b.cue,
        isWaterfall: a.isWaterfall || b.isWaterfall,
      });
      sub.push({
        x: 0.25 * a.x + 0.75 * b.x,
        y: 0.25 * a.y + 0.75 * b.y,
        halfW: 0.25 * a.halfW + 0.75 * b.halfW,
        q: 0.25 * a.q + 0.75 * b.q,
        grade: 0.25 * a.grade + 0.75 * b.grade,
        localGrade: 0.25 * a.localGrade + 0.75 * b.localGrade,
        strahler: Math.max(a.strahler, b.strahler),
        cue: 0.25 * a.cue + 0.75 * b.cue,
        isWaterfall: a.isWaterfall || b.isWaterfall,
      });
    }
    sub.push({ ...stage[stage.length - 1] });
    stage = sub;
  }
  return stage;
}

function enrichPolylineMetrics(raw: readonly RawVertex[], texelM: number): RiverPoint[] {
  const n = raw.length;
  const out: RiverPoint[] = new Array(n);
  const cumAlong = new Float64Array(n);
  for (let k = 1; k < n; k += 1) {
    cumAlong[k] = cumAlong[k - 1] + Math.hypot(raw[k].x - raw[k - 1].x, raw[k].y - raw[k - 1].y) * texelM;
  }
  const curvArcM = 140;
  for (let k = 0; k < n; k += 1) {
    const alongM = cumAlong[k];
    let iIn0 = k;
    while (iIn0 > 0 && alongM - cumAlong[iIn0] < curvArcM) iIn0 -= 1;
    let iOut1 = k;
    while (iOut1 < n - 1 && cumAlong[iOut1] - alongM < curvArcM) iOut1 += 1;

    const inDx = raw[k].x - raw[iIn0].x;
    const inDz = raw[k].y - raw[iIn0].y;
    const outDx = raw[iOut1].x - raw[k].x;
    const outDz = raw[iOut1].y - raw[k].y;
    const inLen = Math.hypot(inDx, inDz);
    const outLen = Math.hypot(outDx, outDz);
    let signedCurv = 0;
    if (inLen > 0.25 && outLen > 0.25) {
      const tx0 = inDx / inLen;
      const tz0 = inDz / inLen;
      const tx1 = outDx / outLen;
      const tz1 = outDz / outLen;
      // Right-hand turn (tangent turns toward right-normal (-tz, tx)) => positive cross(T_in, T_out).
      const cross = tx0 * tz1 - tz0 * tx1;
      const turn = Math.asin(clamp(cross, -1, 1));
      signedCurv = clamp(turn / 0.58, -1, 1);
    }
    // Smooth macro grade across +/-2 vertices while preserving unsmoothed localGrade for step foam.
    let gSum = 0;
    let gCnt = 0;
    for (let gk = Math.max(0, k - 2); gk <= Math.min(n - 1, k + 2); gk += 1) {
      gSum += raw[gk].grade;
      gCnt += 1;
    }
    const smoothGrade = gCnt > 0 ? gSum / gCnt : raw[k].grade;
    const halfW = raw[k].halfW;
    const bankDropM = clamp(1.0 + Math.sqrt(Math.max(0.5, halfW)) * 0.42, 1.0, 3.0);
    out[k] = {
      x: raw[k].x,
      y: raw[k].y,
      halfW,
      q: raw[k].q,
      alongM,
      grade: smoothGrade,
      localGrade: raw[k].localGrade,
      strahler: raw[k].strahler,
      signedCurv,
      bankDropM,
      cue: raw[k].cue,
      isWaterfall: raw[k].isWaterfall,
    };
  }
  return out;
}

const SPATIAL_GRID_SIDE = 256;

/**
 * Extracts continuous smoothed river polylines from D8 channel cells with discharge-scaled widths,
 * longitudinal slope grades, meander signed curvature, and an O(1) spatial segment lookup index.
 */
export function extractRiverNetwork(bundle: RiverMaskInput): RiverNetwork {
  const { size, extentM, waterKind, heightM, hydroFlowRgba, normalRg } = bundle;
  const count = size * size;
  const texelM = extentM / Math.max(1, size);

  const isChannel = new Uint8Array(count);
  const halfWidthArr = new Float32Array(count);
  const qArr = new Uint8Array(count);
  const slopeArr = new Float32Array(count);
  const cueArr = new Float32Array(count);

  for (let i = 0; i < count; i += 1) {
    const wk = waterKind[i];
    if (!isChannelWater(wk)) continue;
    isChannel[i] = 1;
    const o4 = i * 4;
    const q = hydroFlowRgba[o4 + 2];
    qArr[i] = q;
    const nx = (normalRg[i * 2] / 255) * 2 - 1;
    const nz = (normalRg[i * 2 + 1] / 255) * 2 - 1;
    const slope = 1 - Math.sqrt(Math.max(0, 1 - nx * nx - nz * nz));
    slopeArr[i] = slope;
    halfWidthArr[i] = computeRiverWidthM(q, texelM, slope, wk) * 0.5;
  }

  const nextCell = new Int32Array(count).fill(-1);
  const inDegree = new Uint16Array(count);
  const gradeArr = new Float32Array(count);
  const localGradeArr = new Float32Array(count);
  const wfArr = new Uint8Array(count);

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
          const sc = dh * 2.0 + dot * 1.5 + qn * 0.4;
          if (
            sc > bestChScore &&
            (dh > -1.0 || dot > 0.35 || hydroFlowRgba[ni * 4 + 2] >= hydroFlowRgba[o4 + 2])
          ) {
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

  // Exact topological Strahler order + dominant upstream parent continuity on channel DAG
  const remIn = new Int32Array(inDegree);
  const maxOrd = new Uint8Array(count);
  const cntMax = new Uint8Array(count);
  const strahlerArr = new Uint8Array(count);
  const upLen = new Uint32Array(count);
  const mainParent = new Int32Array(count).fill(-1);
  const topoQ: number[] = [];
  for (let i = 0; i < count; i += 1) {
    if (isChannel[i] && remIn[i] === 0) topoQ.push(i);
  }
  for (let qi = 0; qi < topoQ.length; qi += 1) {
    const u = topoQ[qi];
    const ord = maxOrd[u] === 0 ? 1 : cntMax[u] >= 2 ? maxOrd[u] + 1 : maxOrd[u];
    strahlerArr[u] = ord;
    const lenU = upLen[u] + 1;
    const v = nextCell[u];
    if (v >= 0 && isChannel[v]) {
      if (ord > maxOrd[v]) {
        maxOrd[v] = ord;
        cntMax[v] = 1;
      } else if (ord === maxOrd[v]) {
        cntMax[v] += 1;
      }
      const prevP = mainParent[v];
      const better =
        prevP < 0 ||
        ord > strahlerArr[prevP] ||
        (ord === strahlerArr[prevP] && qArr[u] > qArr[prevP]) ||
        (ord === strahlerArr[prevP] && qArr[u] === qArr[prevP] && lenU > upLen[prevP]);
      if (better) {
        mainParent[v] = u;
        upLen[v] = lenU;
      }
      remIn[v] -= 1;
      if (remIn[v] === 0) topoQ.push(v);
    }
  }
  for (let i = 0; i < count; i += 1) {
    if (isChannel[i] && strahlerArr[i] === 0) {
      strahlerArr[i] = Math.max(1, maxOrd[i]);
    }
  }

  // Longitudinal grade + local step grade + waterfall flagging + trunk/rapids cue
  for (let i = 0; i < count; i += 1) {
    if (!isChannel[i]) continue;
    const wk = waterKind[i];
    const q = qArr[i];
    const nxt = nextCell[i];
    let longSlope = 0;
    if (wk === WaterKind.Waterfall) wfArr[i] = 1;
    if (nxt >= 0) {
      const ax = i % size;
      const ay = Math.floor(i / size);
      const bx = nxt % size;
      const by = Math.floor(nxt / size);
      const stepM = Math.max(texelM * 0.5, Math.hypot(bx - ax, by - ay) * texelM);
      longSlope = Math.max(0, (heightM[i] - heightM[nxt]) / stepM);
      if (waterKind[nxt] === WaterKind.Waterfall) {
        wfArr[i] = 1;
        if (isChannel[nxt]) wfArr[nxt] = 1;
      }
    }
    localGradeArr[i] = Math.max(slopeArr[i] * 0.65, longSlope);
    const grade = Math.max(slopeArr[i], longSlope * 0.85);
    gradeArr[i] = grade;
    const isTrunk = isTrunkRiver(halfWidthArr[i], strahlerArr[i]);
    let cue = isTrunk
      ? Math.round(138 + Math.min(1, q / 110) * 60)
      : Math.round(22 + Math.min(1, q / 35) * 86);
    if (wk === WaterKind.Waterfall || inDegree[i] >= 2 || longSlope > 0.12 || slopeArr[i] > 0.16) {
      cue = Math.min(255, Math.max(cue + 55, 205));
    }
    cueArr[i] = cue;
  }

  const visitedStart = new Uint8Array(count);
  const traced: Array<{
    pts: RawVertex[];
    rawCells: number[];
    joinCell: number;
    maxOrd: number;
  }> = [];

  const traceFrom = (startIdx: number): void => {
    const pts: RawVertex[] = [];
    const rawCells: number[] = [];
    const localSeen = new Set<number>();
    let joinCell = -1;
    let maxOrd = 1;
    let cur = startIdx;
    while (cur >= 0) {
      const cx = cur % size;
      const cy = Math.floor(cur / size);
      if (!isChannel[cur]) {
        const prev = pts[pts.length - 1];
        if (prev) {
          pts.push({
            x: cx,
            y: cy,
            halfW: prev.halfW,
            q: prev.q,
            grade: prev.grade * 0.5,
            localGrade: prev.localGrade * 0.5,
            strahler: prev.strahler,
            cue: prev.cue,
            isWaterfall: false,
          });
          rawCells.push(cur);
        }
        break;
      }
      if (strahlerArr[cur] > maxOrd) maxOrd = strahlerArr[cur];
      pts.push({
        x: cx,
        y: cy,
        halfW: halfWidthArr[cur],
        q: qArr[cur],
        grade: gradeArr[cur],
        localGrade: localGradeArr[cur],
        strahler: strahlerArr[cur],
        cue: cueArr[cur],
        isWaterfall: wfArr[cur] === 1,
      });
      rawCells.push(cur);
      if (localSeen.has(cur)) break;
      localSeen.add(cur);
      visitedStart[cur] = 1;

      const nxt = nextCell[cur];
      if (nxt < 0) break;
      // Continue dominant trunks unbroken through confluences; only terminate joining minor tributaries.
      if (
        isChannel[nxt] &&
        ((mainParent[nxt] >= 0 && mainParent[nxt] !== cur) || visitedStart[nxt])
      ) {
        if (strahlerArr[nxt] > maxOrd) maxOrd = strahlerArr[nxt];
        pts.push({
          x: nxt % size,
          y: Math.floor(nxt / size),
          halfW: Math.max(halfWidthArr[cur], halfWidthArr[nxt] * 0.88),
          q: Math.max(qArr[cur], qArr[nxt]),
          grade: gradeArr[nxt],
          localGrade: localGradeArr[nxt],
          strahler: strahlerArr[nxt],
          cue: cueArr[nxt],
          isWaterfall: wfArr[nxt] === 1,
        });
        rawCells.push(nxt);
        joinCell = nxt;
        break;
      }
      cur = nxt;
    }
    if (pts.length >= 2) {
      // Prune 1-pixel dead-end raster wart stubs beside channels so they don't render as floating sticks.
      if (pts.length <= 2 && inDegree[startIdx] === 0 && joinCell >= 0) {
        return;
      }
      if (inDegree[startIdx] === 0 && pts.length >= 4) {
        pts[0] = { ...pts[0], halfW: Math.max(RIVER_WIDTH_MIN_M * 0.5, pts[0].halfW * 0.85) };
      }
      traced.push({ pts, rawCells, joinCell, maxOrd });
    }
  };

  for (let i = 0; i < count; i += 1) {
    if (isChannel[i] && mainParent[i] < 0) {
      traceFrom(i);
    }
  }
  for (let i = 0; i < count; i += 1) {
    if (isChannel[i] && !visitedStart[i]) {
      traceFrom(i);
    }
  }

  traced.sort((a, b) => b.maxOrd - a.maxOrd || b.pts.length - a.pts.length);
  const smoothedCellPos = new Map<number, { x: number; y: number }>();
  const polylines: RiverPoint[][] = [];
  for (let i = 0; i < traced.length; i += 1) {
    const item = traced[i];
    if (item.joinCell >= 0) {
      const snap = smoothedCellPos.get(item.joinCell);
      if (snap) {
        const lastIdx = item.pts.length - 1;
        item.pts[lastIdx] = { ...item.pts[lastIdx], x: snap.x, y: snap.y };
      }
    }
    const smoothed = smoothAndSubdividePolyline(item.pts, (lap) => {
      for (let k = 0; k < lap.length && k < item.rawCells.length; k += 1) {
        const cellIdx = item.rawCells[k];
        if (!smoothedCellPos.has(cellIdx)) {
          smoothedCellPos.set(cellIdx, { x: lap[k].x, y: lap[k].y });
        }
      }
    });
    if (smoothed.length >= 2) {
      polylines.push(enrichPolylineMetrics(smoothed, texelM));
    }
  }

  // Build spatial segment lookup grid (256x256)
  let totalSegs = 0;
  for (let p = 0; p < polylines.length; p += 1) {
    totalSegs += Math.max(0, polylines[p].length - 1);
  }
  const segAx = new Float32Array(totalSegs);
  const segAy = new Float32Array(totalSegs);
  const segBx = new Float32Array(totalSegs);
  const segBy = new Float32Array(totalSegs);
  const segAhw = new Float32Array(totalSegs);
  const segBhw = new Float32Array(totalSegs);
  const segAg = new Float32Array(totalSegs);
  const segBg = new Float32Array(totalSegs);
  const segAlg = new Float32Array(totalSegs);
  const segBlg = new Float32Array(totalSegs);
  const segOrd = new Uint8Array(totalSegs);
  const segAc = new Float32Array(totalSegs);
  const segBc = new Float32Array(totalSegs);
  const segAq = new Float32Array(totalSegs);
  const segBq = new Float32Array(totalSegs);
  const segWf = new Uint8Array(totalSegs);

  const gSide = SPATIAL_GRID_SIDE;
  const gCells = gSide * gSide;
  const cellCounts = new Uint32Array(gCells);
  const texPerCell = size / gSide;

  let sCursor = 0;
  for (let p = 0; p < polylines.length; p += 1) {
    const line = polylines[p];
    for (let k = 0; k < line.length - 1; k += 1) {
      const a = line[k];
      const b = line[k + 1];
      segAx[sCursor] = a.x;
      segAy[sCursor] = a.y;
      segBx[sCursor] = b.x;
      segBy[sCursor] = b.y;
      segAhw[sCursor] = a.halfW;
      segBhw[sCursor] = b.halfW;
      segAg[sCursor] = a.grade;
      segBg[sCursor] = b.grade;
      segAlg[sCursor] = a.localGrade;
      segBlg[sCursor] = b.localGrade;
      segOrd[sCursor] = Math.max(a.strahler, b.strahler);
      segAc[sCursor] = a.signedCurv;
      segBc[sCursor] = b.signedCurv;
      segAq[sCursor] = a.q;
      segBq[sCursor] = b.q;
      segWf[sCursor] = a.isWaterfall || b.isWaterfall ? 1 : 0;

      const x0 = clamp(Math.floor(Math.min(a.x, b.x) / texPerCell), 0, gSide - 1);
      const x1 = clamp(Math.floor(Math.max(a.x, b.x) / texPerCell), 0, gSide - 1);
      const y0 = clamp(Math.floor(Math.min(a.y, b.y) / texPerCell), 0, gSide - 1);
      const y1 = clamp(Math.floor(Math.max(a.y, b.y) / texPerCell), 0, gSide - 1);
      for (let gy = y0; gy <= y1; gy += 1) {
        const row = gy * gSide;
        for (let gx = x0; gx <= x1; gx += 1) {
          cellCounts[row + gx] += 1;
        }
      }
      sCursor += 1;
    }
  }

  const cellStart = new Uint32Array(gCells + 1);
  for (let c = 0; c < gCells; c += 1) cellStart[c + 1] = cellStart[c] + cellCounts[c];
  const cellFill = cellStart.slice(0, gCells);
  const cellSegs = new Uint32Array(cellStart[gCells]);

  for (let s = 0; s < totalSegs; s += 1) {
    const x0 = clamp(Math.floor(Math.min(segAx[s], segBx[s]) / texPerCell), 0, gSide - 1);
    const x1 = clamp(Math.floor(Math.max(segAx[s], segBx[s]) / texPerCell), 0, gSide - 1);
    const y0 = clamp(Math.floor(Math.min(segAy[s], segBy[s]) / texPerCell), 0, gSide - 1);
    const y1 = clamp(Math.floor(Math.max(segAy[s], segBy[s]) / texPerCell), 0, gSide - 1);
    for (let gy = y0; gy <= y1; gy += 1) {
      const row = gy * gSide;
      for (let gx = x0; gx <= x1; gx += 1) {
        cellSegs[cellFill[row + gx]++] = s;
      }
    }
  }

  const queryNearestRiver = (u: number, v: number, maxDistM = 120): RiverNearestHit | null => {
    const px = clamp(u, 0, 1) * (size - 1);
    const pz = clamp(v, 0, 1) * (size - 1);
    const radCells = Math.max(1, Math.ceil(maxDistM / (extentM / gSide)));
    const cgx = clamp(Math.floor(px / texPerCell), 0, gSide - 1);
    const cgy = clamp(Math.floor(pz / texPerCell), 0, gSide - 1);
    const gx0 = Math.max(0, cgx - radCells);
    const gx1 = Math.min(gSide - 1, cgx + radCells);
    const gy0 = Math.max(0, cgy - radCells);
    const gy1 = Math.min(gSide - 1, cgy + radCells);

    let bestDistSqM = maxDistM * maxDistM;
    let bestHit: RiverNearestHit | null = null;
    const texM2 = texelM * texelM;

    for (let gy = gy0; gy <= gy1; gy += 1) {
      const row = gy * gSide;
      for (let gx = gx0; gx <= gx1; gx += 1) {
        const c = row + gx;
        const start = cellStart[c];
        const end = cellStart[c + 1];
        for (let k = start; k < end; k += 1) {
          const s = cellSegs[k];
          const ax = segAx[s];
          const ay = segAy[s];
          const dx = segBx[s] - ax;
          const dy = segBy[s] - ay;
          const lenSq = dx * dx + dy * dy;
          let t = 0;
          if (lenSq > 1e-8) {
            t = clamp(((px - ax) * dx + (pz - ay) * dy) / lenSq, 0, 1);
          }
          const qx = ax + dx * t;
          const qy = ay + dy * t;
          const ox = px - qx;
          const oz = pz - qy;
          const dSqM = (ox * ox + oz * oz) * texM2;
          if (dSqM < bestDistSqM) {
            bestDistSqM = dSqM;
            const segLen = Math.sqrt(lenSq);
            const dirX = segLen > 1e-4 ? dx / segLen : 0.7071;
            const dirZ = segLen > 1e-4 ? dy / segLen : 0.7071;
            const rx = -dirZ;
            const rz = dirX;
            const distM = Math.sqrt(dSqM);
            const signedSideM = (ox * rx + oz * rz) * texelM;
            const halfW = segAhw[s] + (segBhw[s] - segAhw[s]) * t;
            const curv = segAc[s] + (segBc[s] - segAc[s]) * t;
            const sideNorm = distM > 1e-4 ? clamp(signedSideM / distM, -1, 1) : 0;
            bestHit = {
              distM,
              signedSideM,
              halfW,
              bankDropM: clamp(1.0 + Math.sqrt(Math.max(0.5, halfW)) * 0.42, 1.0, 3.0),
              grade: segAg[s] + (segBg[s] - segAg[s]) * t,
              localGrade: segAlg[s] + (segBlg[s] - segAlg[s]) * t,
              strahler: segOrd[s],
              q: segAq[s] + (segBq[s] - segAq[s]) * t,
              bendSide: sideNorm * curv,
              dirX,
              dirZ,
              projU: qx / Math.max(1, size - 1),
              projV: qy / Math.max(1, size - 1),
              isWaterfall: segWf[s] === 1,
            };
          }
        }
      }
    }
    return bestHit;
  };

  return { size, extentM, texelM, polylines, queryNearestRiver };
}

const riverNetworkCache = new WeakMap<RiverMaskInput, RiverNetwork>();

export function getCachedRiverNetwork(bundle: RiverMaskInput): RiverNetwork {
  const hit = riverNetworkCache.get(bundle);
  if (hit) return hit;
  const built = extractRiverNetwork(bundle);
  riverNetworkCache.set(bundle, built);
  return built;
}

/**
 * Rasterizes the unified smoothed river network into RGBA8 `riverMaskMap`:
 * - `R`: `clamp(round(distM - halfW + 80), 0..255)` (standing water = `255`).
 * - `G`: encoded `bendInside` (`clamp(round(128 + bendSide * 127), 0..255)`, `>128` inside point bar).
 * - `B`: encoded `halfW` in metres (`clamp(round(halfW * 4), 0..255)`, `0..63.75 m`).
 * - `A`: flow energy / rapids cue (`0..190` from Q; boosted `>=200` at confluences/waterfalls/steep chutes).
 */
export function buildSmoothedRiverMaskData(bundle: RiverMaskInput): Uint8Array {
  const { size, extentM, waterKind, hydroFlowRgba } = bundle;
  const count = size * size;
  const texelM = extentM / Math.max(1, size);
  const net = getCachedRiverNetwork(bundle);
  const out = new Uint8Array(count * 4);

  const distSdf = new Float32Array(count).fill(175.0);
  const segHalfW = new Float32Array(count);
  const segBend = new Float32Array(count);
  const segCue = new Float32Array(count);
  const hasSeg = new Uint8Array(count);

  for (let pIdx = 0; pIdx < net.polylines.length; pIdx += 1) {
    const line = net.polylines[pIdx];
    for (let s = 0; s < line.length - 1; s += 1) {
      const a = line[s];
      const b = line[s + 1];
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const lenSq = dx * dx + dy * dy;
      const segLen = Math.sqrt(lenSq);
      const tx = segLen > 1e-4 ? dx / segLen : 0.7071;
      const tz = segLen > 1e-4 ? dy / segLen : 0.7071;
      const rx = -tz;
      const rz = tx;

      const maxW = Math.max(a.halfW, b.halfW);
      const isTrunk = isTrunkRiver(maxW, Math.max(a.strahler, b.strahler));
      const reachTex = Math.ceil((maxW + (isTrunk ? 76 : 36)) / texelM);
      const minX = Math.max(0, Math.floor(Math.min(a.x, b.x)) - reachTex);
      const maxX = Math.min(size - 1, Math.ceil(Math.max(a.x, b.x)) + reachTex);
      const minY = Math.max(0, Math.floor(Math.min(a.y, b.y)) - reachTex);
      const maxY = Math.min(size - 1, Math.ceil(Math.max(a.y, b.y)) + reachTex);

      for (let py = minY; py <= maxY; py += 1) {
        const row = py * size;
        for (let px = minX; px <= maxX; px += 1) {
          const i = row + px;
          if (isStandingWater(waterKind[i])) continue;
          const t = lenSq > 1e-8 ? clamp(((px - a.x) * dx + (py - a.y) * dy) / lenSq, 0, 1) : 0;
          const qx = a.x + t * dx;
          const qy = a.y + t * dy;
          const ox = px - qx;
          const oy = py - qy;
          const dM = Math.hypot(ox, oy) * texelM;
          const wM = a.halfW + (b.halfW - a.halfW) * t;
          const sdf = dM - wM;
          if (sdf < distSdf[i]) {
            distSdf[i] = sdf;
            segHalfW[i] = wM;
            const curv = a.signedCurv + (b.signedCurv - a.signedCurv) * t;
            const sideNorm = dM > 1e-4 ? clamp(((ox * rx + oy * rz) * texelM) / dM, -1, 1) : 0;
            segBend[i] = sideNorm * curv;
            segCue[i] = a.cue + (b.cue - a.cue) * t;
            hasSeg[i] = 1;
          }
        }
      }
    }
  }

  for (let i = 0; i < count; i += 1) {
    const o4 = i * 4;
    if (isStandingWater(waterKind[i])) {
      out[o4] = 255;
      out[o4 + 1] = 128;
      out[o4 + 2] = 0;
      out[o4 + 3] = 0;
      continue;
    }
    out[o4] = clamp(Math.round(distSdf[i] + RIVER_SDF_ZERO_BYTE), 0, 255);
    if (hasSeg[i]) {
      out[o4 + 1] = clamp(Math.round(128 + segBend[i] * 127), 0, 255);
      out[o4 + 2] = clamp(Math.round(segHalfW[i] * 4), 0, 255);
      out[o4 + 3] = clamp(Math.round(segCue[i]), 0, 255);
    } else {
      out[o4 + 1] = 128;
      out[o4 + 2] = 0;
      out[o4 + 3] = hydroFlowRgba[o4 + 3];
    }
  }

  return out;
}

const smoothedRiverMaskCache = new WeakMap<RiverMaskInput, Uint8Array>();

export function getCachedSmoothedRiverMaskData(bundle: RiverMaskInput): Uint8Array {
  const hit = smoothedRiverMaskCache.get(bundle);
  if (hit) return hit;
  const built = buildSmoothedRiverMaskData(bundle);
  smoothedRiverMaskCache.set(bundle, built);
  return built;
}
