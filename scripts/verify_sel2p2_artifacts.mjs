// Independent verifier for Experiment SEL2-P2:
//   node scripts/verify_sel2p2_artifacts.mjs [artifact-dir] [--prereg <path>] [--skip-committed-manifests]
//   node scripts/verify_sel2p2_artifacts.mjs --self-test
//
// Defaults to `artifacts/experiments/sel2-p2/confirmation`.
// Exits 0 without reading/creating results if confirmation artifacts do not yet exist (pre-SEL2P2-run).
// Passing an explicit path that does not exist, or an existing directory that is empty/incomplete, FAILS (exit 1).

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const committedManifestDir = 'src-tauri/tests/fixtures/experiments_sel2p2';
const DEFAULT_CONFIRM_DIR = 'artifacts/experiments/sel2-p2/confirmation';
const PREREG_FILE = 'sel2p2-preregistration.json';

const ARM_ORDER = ['selected', 'scarce_mut0', 'scarce_drift'];

const CONTRAST_DEFS = [
  {
    id: 'H1',
    name: 'selected / scarce_drift',
    controlArm: 'scarce_drift',
    treatmentArm: 'selected',
    usesDrift: true,
  },
  {
    id: 'H2',
    name: 'selected / scarce_mut0',
    controlArm: 'scarce_mut0',
    treatmentArm: 'selected',
    usesDrift: false,
  },
  {
    id: 'H3',
    name: 'scarce_drift / scarce_mut0',
    controlArm: 'scarce_mut0',
    treatmentArm: 'scarce_drift',
    usesDrift: true,
  },
];

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');
const sha256NormalizedLf = (buf) =>
  sha256(Buffer.from(buf.toString('utf8').replace(/\r\n/g, '\n'), 'utf8'));

const isSafeGitCommit = (ref) => typeof ref === 'string' && /^[0-9a-fA-F]{7,40}$/.test(ref);

// ---- Pure-JS exact special functions (Lanczos logGamma, Lentz betainc, erfc, normInv, t) ------

function logGamma(z) {
  const g = 7;
  const p = [
    0.99999999999980993,
    676.5203681218851,
    -1259.1392167224028,
    771.32342877765313,
    -176.61502916214059,
    12.507343278686905,
    -0.13857109526572012,
    9.9843695780195716e-6,
    1.5056327351493116e-7,
  ];
  if (z < 0.5) {
    return Math.log(Math.PI / Math.sin(Math.PI * z)) - logGamma(1 - z);
  }
  const zm1 = z - 1;
  let x = p[0];
  for (let i = 1; i < p.length; i += 1) {
    x += p[i] / (zm1 + i);
  }
  const t = zm1 + g + 0.5;
  return 0.5 * Math.log(2 * Math.PI) + (zm1 + 0.5) * Math.log(t) - t + Math.log(x);
}

function betacf(a, b, x) {
  const MAXIT = 300;
  const EPS = 3e-15;
  const FPMIN = 1e-300;
  const qab = a + b;
  const qap = a + 1;
  const qam = a - 1;
  let c = 1;
  let d = 1 - (qab * x) / qap;
  if (Math.abs(d) < FPMIN) d = FPMIN;
  d = 1 / d;
  let h = d;
  for (let m = 1; m <= MAXIT; m += 1) {
    const m2 = 2 * m;
    let aa = (m * (b - m) * x) / ((qam + m2) * (a + m2));
    d = 1 + aa * d;
    if (Math.abs(d) < FPMIN) d = FPMIN;
    c = 1 + aa / c;
    if (Math.abs(c) < FPMIN) c = FPMIN;
    d = 1 / d;
    h *= d * c;
    aa = (-(a + m) * (qab + m) * x) / ((a + m2) * (qap + m2));
    d = 1 + aa * d;
    if (Math.abs(d) < FPMIN) d = FPMIN;
    c = 1 + aa / c;
    if (Math.abs(c) < FPMIN) c = FPMIN;
    d = 1 / d;
    const del = d * c;
    h *= del;
    if (Math.abs(del - 1) <= EPS) break;
  }
  return h;
}

export function betaInc(a, b, x) {
  if (x <= 0) return 0;
  if (x >= 1) return 1;
  const bt = Math.exp(
    logGamma(a + b) - logGamma(a) - logGamma(b) + a * Math.log(x) + b * Math.log(1 - x),
  );
  if (x < (a + 1) / (a + b + 2)) {
    return (bt * betacf(a, b, x)) / a;
  }
  return 1 - (bt * betacf(b, a, 1 - x)) / b;
}

export function studentTCdf(t, df) {
  if (!Number.isFinite(df) || df < 1) return NaN;
  if (t === 0) return 0.5;
  if (t === Infinity) return 1;
  if (t === -Infinity) return 0;
  const x = df / (df + t * t);
  const tail = 0.5 * betaInc(0.5 * df, 0.5, x);
  return t >= 0 ? 1 - tail : tail;
}

export function studentTTwoSidedP(t, df) {
  if (!Number.isFinite(df) || df < 1) return NaN;
  if (!Number.isFinite(t)) return Math.abs(t) === Infinity ? 0 : NaN;
  if (t === 0) return 1;
  const x = df / (df + t * t);
  return Math.min(1, Math.max(0, betaInc(0.5 * df, 0.5, x)));
}

export function studentTInv(prob, df) {
  if (!Number.isFinite(df) || df < 1 || !(prob > 0 && prob < 1)) return NaN;
  if (prob === 0.5) return 0;
  if (prob < 0.5) return -studentTInv(1 - prob, df);
  let lo = 0;
  let hi = 1;
  while (studentTCdf(hi, df) < prob && hi < 1e9) {
    hi *= 2;
  }
  for (let i = 0; i < 100; i += 1) {
    const mid = 0.5 * (lo + hi);
    if (studentTCdf(mid, df) < prob) lo = mid;
    else hi = mid;
    if (hi - lo <= 1e-13 * Math.max(1, mid)) break;
  }
  return 0.5 * (lo + hi);
}

export function erfcApprox(x) {
  const z = Math.abs(x);
  const t = 1 / (1 + 0.5 * z);
  const r = t * Math.exp(
    -z * z -
      1.26551223 +
      t * (1.00002368 +
        t * (0.37409196 +
          t * (0.09678418 +
            t * (-0.18628806 +
              t * (0.27886807 +
                t * (-1.13520398 +
                  t * (1.48851587 +
                    t * (-0.82215223 + t * 0.17087277)))))))),
  );
  return x >= 0 ? r : 2 - r;
}

export const normCdf = (z) => 0.5 * erfcApprox(-z / Math.SQRT2);

export function normInv(p) {
  if (!(p > 0 && p < 1)) {
    if (p === 0) return -Infinity;
    if (p === 1) return Infinity;
    return NaN;
  }
  const a = [
    -3.969683028665376e1, 2.209460984245205e2, -2.759285104469687e2,
    1.38357751867269e2, -3.066479806614716e1, 2.506628277459239,
  ];
  const b = [
    -5.447609879822406e1, 1.615858368580409e2, -1.556989798598866e2,
    6.680131188771972e1, -1.328068155288572e1,
  ];
  const c = [
    -7.784894002430293e-3, -3.223964580411365e-1, -2.400758277161838,
    -2.549732539343734, 4.374664141464968, 2.938163982698783,
  ];
  const d = [
    7.784695709041462e-3, 3.224671290700398e-1,
    2.445134137142996, 3.754408661907416,
  ];
  const plow = 0.02425;
  const phigh = 1 - plow;
  let x;
  if (p < plow) {
    const q = Math.sqrt(-2 * Math.log(p));
    x = (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) /
      ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1);
  } else if (p <= phigh) {
    const q = p - 0.5;
    const r = q * q;
    x = (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q /
      (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1);
  } else {
    const q = Math.sqrt(-2 * Math.log(1 - p));
    x = -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) /
      ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1);
  }
  const invSqrt2Pi = 1 / Math.sqrt(2 * Math.PI);
  for (let i = 0; i < 2; i += 1) {
    const err = normCdf(x) - p;
    const pdf = invSqrt2Pi * Math.exp(-0.5 * x * x);
    if (pdf > 0) x -= err / pdf;
  }
  return x;
}

// ---- Basic statistical helpers ----------------------------------------------------------------

export function mean(xs) {
  if (xs.length === 0) return null;
  return xs.reduce((a, b) => a + b, 0) / xs.length;
}

export function median(xs) {
  if (xs.length === 0) return null;
  const sorted = [...xs].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[mid] : 0.5 * (sorted[mid - 1] + sorted[mid]);
}

export function sampleSd(xs) {
  if (xs.length < 2) return null;
  const m = mean(xs);
  const ss = xs.reduce((acc, x) => acc + (x - m) ** 2, 0);
  return Math.sqrt(ss / (xs.length - 1));
}

export function ranksWithTies(xs) {
  const indexed = xs.map((v, idx) => ({ v, idx })).sort((a, b) => a.v - b.v);
  const ranks = new Array(xs.length);
  const tieCounts = [];
  let i = 0;
  while (i < indexed.length) {
    let j = i + 1;
    while (j < indexed.length && indexed[j].v === indexed[i].v) j += 1;
    const avgRank = 0.5 * (i + 1 + j);
    for (let k = i; k < j; k += 1) {
      ranks[indexed[k].idx] = avgRank;
    }
    if (j - i > 1) tieCounts.push(j - i);
    i = j;
  }
  return { ranks, tieCounts };
}

export function wilcoxonSignedRank(diffs, minNonZeroR7 = 7) {
  const nz = diffs.filter((d) => Number.isFinite(d) && d !== 0);
  const nNz = nz.length;
  const pMin = nNz > 0 ? Math.min(1, 2 / Math.pow(2, nNz)) : 1;
  if (nNz === 0) {
    return {
      n_non_zero: 0,
      w_plus: 0,
      w_minus: 0,
      r_c: null,
      p_value: 1,
      p_min: 1,
      meets_r7: false,
      method: 'degenerate_all_zero',
    };
  }
  const absVals = nz.map((d) => Math.abs(d));
  const { ranks, tieCounts } = ranksWithTies(absVals);
  let wPlus = 0;
  let wMinus = 0;
  for (let i = 0; i < nNz; i += 1) {
    if (nz[i] > 0) wPlus += ranks[i];
    else wMinus += ranks[i];
  }
  const totalRank = wPlus + wMinus;
  const rc = totalRank > 0 ? (wPlus - wMinus) / totalRank : 0;

  let pVal;
  let method;
  if (nNz <= 20 && tieCounts.length === 0) {
    const maxSum = (nNz * (nNz + 1)) / 2;
    let dp = new Float64Array(maxSum + 1);
    dp[0] = 1;
    for (let k = 1; k <= nNz; k += 1) {
      const next = new Float64Array(maxSum + 1);
      for (let s = 0; s <= maxSum; s += 1) {
        if (dp[s] > 0) {
          next[s] += dp[s];
          next[s + k] += dp[s];
        }
      }
      dp = next;
    }
    const wObs = Math.round(Math.min(wPlus, wMinus));
    let tailCount = 0;
    for (let s = 0; s <= wObs; s += 1) tailCount += dp[s];
    pVal = Math.min(1, (2 * tailCount) / Math.pow(2, nNz));
    method = 'exact_dp';
  } else {
    const mu = (nNz * (nNz + 1)) / 4;
    const tieAdj = tieCounts.reduce((acc, t) => acc + t * (t * t - 1), 0) / 48;
    const variance = (nNz * (nNz + 1) * (2 * nNz + 1)) / 24 - tieAdj;
    if (variance <= 0) {
      pVal = 1;
    } else {
      const sd = Math.sqrt(variance);
      const wStat = Math.min(wPlus, wMinus);
      const z = Math.abs(wStat - mu) / sd;
      pVal = Math.min(1, 2 * normCdf(-z));
    }
    method = 'normal_approx';
  }

  return {
    n_non_zero: nNz,
    w_plus: wPlus,
    w_minus: wMinus,
    r_c: rc,
    p_value: pVal,
    p_min: pMin,
    meets_r7: nNz >= minNonZeroR7,
    method,
  };
}

export function hodgesLehmannPseudomedian(xs) {
  const n = xs.length;
  if (n === 0) return null;
  if (n === 1) return xs[0];
  const walsh = new Float64Array((n * (n + 1)) / 2);
  let idx = 0;
  for (let i = 0; i < n; i += 1) {
    for (let j = i; j < n; j += 1) {
      walsh[idx] = 0.5 * (xs[i] + xs[j]);
      idx += 1;
    }
  }
  walsh.sort();
  const mid = Math.floor(walsh.length / 2);
  return walsh.length % 2 === 1 ? walsh[mid] : 0.5 * (walsh[mid - 1] + walsh[mid]);
}

function makeSplitMix64(seedBigInt) {
  let state = BigInt.asUintN(64, seedBigInt);
  return () => {
    state = BigInt.asUintN(64, state + 0x9e3779b97f4a7c15n);
    let z = state;
    z = BigInt.asUintN(64, (z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
    z = BigInt.asUintN(64, (z ^ (z >> 27n)) * 0x94d049bb133111ebn);
    z = BigInt.asUintN(64, z ^ (z >> 31n));
    return Number(z & 0x1fffffffffffffn) / 9007199254740992;
  };
}

export function hodgesLehmannBcaCi(xs, { nBoot = 10000, alpha = 0.05, seed = 0x5e120002n } = {}) {
  const n = xs.length;
  const est = hodgesLehmannPseudomedian(xs);
  if (n < 2) {
    return { estimate: est, ci_low: null, ci_high: null, n_boot: 0 };
  }
  const rng = makeSplitMix64(seed);
  const boots = new Float64Array(nBoot);
  const sample = new Array(n);
  let ltCount = 0;
  let eqCount = 0;
  for (let b = 0; b < nBoot; b += 1) {
    for (let i = 0; i < n; i += 1) {
      sample[i] = xs[Math.floor(rng() * n)];
    }
    const hl = hodgesLehmannPseudomedian(sample);
    boots[b] = hl;
    if (hl < est) ltCount += 1;
    else if (hl === est) eqCount += 1;
  }
  boots.sort();

  const jack = new Array(n);
  for (let i = 0; i < n; i += 1) {
    const leaveOne = xs.filter((_, j) => j !== i);
    jack[i] = hodgesLehmannPseudomedian(leaveOne);
  }
  const jackMean = mean(jack);
  let num = 0;
  let den = 0;
  for (let i = 0; i < n; i += 1) {
    const d = jackMean - jack[i];
    num += d * d * d;
    den += d * d;
  }
  const a = den > 0 ? num / (6 * Math.pow(den, 1.5)) : 0;
  const prop = Math.min(1 - 1 / (2 * nBoot), Math.max(1 / (2 * nBoot), (ltCount + 0.5 * eqCount) / nBoot));
  const z0 = normInv(prop);

  const bcaQuantile = (alphaTail) => {
    const zAlpha = normInv(alphaTail);
    const denom = 1 - a * (z0 + zAlpha);
    const u = denom === 0 ? (z0 + zAlpha >= 0 ? 1 : 0) : normCdf(z0 + (z0 + zAlpha) / denom);
    const clamped = Math.min(1, Math.max(0, u));
    const idx = Math.min(nBoot - 1, Math.max(0, Math.round(clamped * (nBoot - 1))));
    return boots[idx];
  };

  return {
    estimate: est,
    ci_low: bcaQuantile(alpha / 2),
    ci_high: bcaQuantile(1 - alpha / 2),
    n_boot: nBoot,
  };
}

export function holmAdjust(pValues) {
  const m = pValues.length;
  const valid = pValues
    .map((p, idx) => ({ p, idx }))
    .filter((entry) => typeof entry.p === 'number' && Number.isFinite(entry.p))
    .sort((a, b) => a.p - b.p);
  const adjusted = new Array(m).fill(null);
  let runningMax = 0;
  for (let rank = 0; rank < valid.length; rank += 1) {
    const rawAdj = (m - rank) * valid[rank].p;
    runningMax = Math.min(1, Math.max(runningMax, rawAdj));
    adjusted[valid[rank].idx] = runningMax;
  }
  return adjusted;
}

export function pairedStats(control, treatment, nRequested = control.length) {
  if (control.length !== treatment.length) {
    throw new Error('control and treatment vectors must have equal length');
  }
  const n = control.length;
  const deltas = control.map((c, i) => treatment[i] - c);
  const out = {
    n_requested: nRequested,
    n_complete_pairs: n,
    control_mean: null,
    treatment_mean: null,
    paired_mean_delta: null,
    paired_sd: null,
    paired_se: null,
    ci95_low: null,
    ci95_high: null,
    paired_dz: null,
    median_of_per_seed_deltas: median(deltas),
  };
  if (n === 0) return out;
  out.control_mean = mean(control);
  out.treatment_mean = mean(treatment);
  out.paired_mean_delta = mean(deltas);
  if (n < 2) return out;
  const sd = sampleSd(deltas);
  const sdTol = 1e-12 * Math.max(1, Math.abs(out.paired_mean_delta));
  if (sd < sdTol) {
    out.paired_sd = 0;
    out.paired_se = 0;
    out.ci95_low = out.paired_mean_delta;
    out.ci95_high = out.paired_mean_delta;
    return out;
  }
  const se = sd / Math.sqrt(n);
  const half = studentTInv(0.975, n - 1) * se;
  out.paired_sd = sd;
  out.paired_se = se;
  out.ci95_low = out.paired_mean_delta - half;
  out.ci95_high = out.paired_mean_delta + half;
  out.paired_dz = out.paired_mean_delta / sd;
  return out;
}

// ---- SEL2-P2 Primary Endpoint, Lee, Lachin Worst-Rank, Smoke Gate & Closed R10 ----------------

export function evaluatePairedLogMeanRatio(
  meanA,
  meanB,
  { sesoi = Math.log(1.1), minNonZeroR7 = 7, nRequested = meanA.length } = {},
) {
  if (meanA.length !== meanB.length) {
    throw new Error('meanA and meanB must have equal length');
  }
  const n = meanA.length;
  const diffs = [];
  for (let i = 0; i < n; i += 1) {
    if (!(meanA[i] > 0 && meanB[i] > 0 && Number.isFinite(meanA[i]) && Number.isFinite(meanB[i]))) {
      throw new Error(`meanA/meanB must be strictly positive finite numbers at index ${i}`);
    }
    diffs.push(Math.log(meanB[i] / meanA[i]));
  }
  const wilcoxon = wilcoxonSignedRank(diffs, minNonZeroR7);
  const out = {
    n_requested: nRequested,
    n_complete_pairs: n,
    pi_ll: nRequested > 0 ? n / nRequested : 0,
    mean_D: null,
    geom_mean_ratio: null,
    sd_D: null,
    se_D: null,
    df: n >= 2 ? n - 1 : null,
    t_stat: null,
    p_two_sided: null,
    ci95_low: null,
    ci95_high: null,
    ci90_tost_low: null,
    ci90_tost_high: null,
    tost_p1: null,
    tost_p2: null,
    tost_equivalent: false,
    paired_dz: null,
    mde_0_5_sd: null,
    mde_holm1_80: null,
    concordance_pos: n > 0 ? diffs.filter((d) => d > 0).length / n : null,
    concordance_neg: n > 0 ? diffs.filter((d) => d < 0).length / n : null,
    median_D: median(diffs),
    hl_pseudomedian: hodgesLehmannPseudomedian(diffs),
    wilcoxon,
    diffs,
  };
  if (n === 0) return out;
  const m = mean(diffs);
  out.mean_D = m;
  out.geom_mean_ratio = Math.exp(m);
  if (n < 2) return out;

  const sd = sampleSd(diffs);
  const zeroTol = 1e-12 * Math.max(1, Math.abs(m));
  if (sd < zeroTol) {
    out.sd_D = 0;
    out.se_D = 0;
    out.t_stat = m === 0 ? 0 : Math.sign(m) * Infinity;
    out.p_two_sided = m === 0 ? 1 : 0;
    out.ci95_low = m;
    out.ci95_high = m;
    out.ci90_tost_low = m;
    out.ci90_tost_high = m;
    out.tost_p1 = m > -sesoi ? 0 : 1;
    out.tost_p2 = m < sesoi ? 0 : 1;
    out.tost_equivalent = Math.abs(m) < sesoi;
    out.mde_0_5_sd = 0;
    out.mde_holm1_80 = 0;
    return out;
  }

  const df = n - 1;
  const se = sd / Math.sqrt(n);
  const tStat = m / se;
  const tCrit95 = studentTInv(0.975, df);
  const tCrit90 = studentTInv(0.95, df);
  const pTwo = studentTTwoSidedP(tStat, df);
  const ci95Low = m - tCrit95 * se;
  const ci95High = m + tCrit95 * se;
  const ci90Low = m - tCrit90 * se;
  const ci90High = m + tCrit90 * se;

  const tLowerBound = (m + sesoi) / se;
  const tUpperBound = (sesoi - m) / se;
  const tostP1 = 1 - studentTCdf(tLowerBound, df);
  const tostP2 = 1 - studentTCdf(tUpperBound, df);

  out.sd_D = sd;
  out.se_D = se;
  out.t_stat = tStat;
  out.p_two_sided = pTwo;
  out.ci95_low = ci95Low;
  out.ci95_high = ci95High;
  out.ci90_tost_low = ci90Low;
  out.ci90_tost_high = ci90High;
  out.tost_p1 = tostP1;
  out.tost_p2 = tostP2;
  out.tost_equivalent = ci90Low > -sesoi && ci90High < sesoi;
  out.paired_dz = m / sd;
  out.mde_0_5_sd = 0.5 * sd;
  out.mde_holm1_80 = 0.4818 * sd;
  return out;
}

export function leeTrimmingBoundsLogMean(survivedMeanA, survivedMeanB) {
  if (survivedMeanA.length === 0 || survivedMeanB.length === 0) {
    return {
      n_a: survivedMeanA.length,
      n_b: survivedMeanB.length,
      k: 0,
      lee_lo: null,
      lee_hi: null,
      width: null,
    };
  }
  const ya = survivedMeanA.map((v) => Math.log(v)).sort((a, b) => a - b);
  const yb = survivedMeanB.map((v) => Math.log(v)).sort((a, b) => a - b);
  const na = ya.length;
  const nb = yb.length;
  const k = Math.min(na, nb);
  const leeLo = mean(yb.slice(0, k)) - mean(ya.slice(na - k));
  const leeHi = mean(yb.slice(nb - k)) - mean(ya.slice(0, k));
  return {
    n_a: na,
    n_b: nb,
    k,
    lee_lo: leeLo,
    lee_hi: leeHi,
    width: leeHi - leeLo,
  };
}

/**
 * Lachin (1999) worst-rank paired test (QD-SEL2P2-D6=(a) R3 fallback when p_ext > 5% or pi_ll < 0.85):
 * Extinct/invalid observations sit strictly below all surviving valid observations, ordered by
 * survivalTicks ascending; surviving valid observations are ordered by post-warmup meanInt ascending.
 * Mid-ranks over the combined 2N pool feed a two-sided paired Wilcoxon signed-rank test on R_B - R_A.
 */
export function lachinWorstRankPaired(obsA, obsB, minNonZeroR7 = 7) {
  if (obsA.length !== obsB.length) {
    throw new Error('lachinWorstRankPaired requires equal-length paired arrays');
  }
  const n = obsA.length;
  const pooled = [];
  const encode = (o, side, i) => {
    const validSurv = Boolean(o.validMass) && Number.isFinite(o.meanInt) && o.meanInt > 0;
    const score = validSurv
      ? { tier: 1, key: o.meanInt }
      : { tier: 0, key: Number.isFinite(o.survivalTicks) ? o.survivalTicks : 0 };
    pooled.push({ ...score, side, i });
  };
  for (let i = 0; i < n; i += 1) {
    encode(obsA[i], 'A', i);
    encode(obsB[i], 'B', i);
  }
  pooled.sort((x, y) => (x.tier !== y.tier ? x.tier - y.tier : x.key - y.key));

  const rankA = new Array(n);
  const rankB = new Array(n);
  let i = 0;
  while (i < pooled.length) {
    let j = i + 1;
    while (j < pooled.length && pooled[j].tier === pooled[i].tier && pooled[j].key === pooled[i].key) {
      j += 1;
    }
    const midRank = 0.5 * (i + 1 + j);
    for (let k = i; k < j; k += 1) {
      if (pooled[k].side === 'A') rankA[pooled[k].i] = midRank;
      else rankB[pooled[k].i] = midRank;
    }
    i = j;
  }

  const rankDiffs = rankA.map((ra, idx) => rankB[idx] - ra);
  const wilcoxon = wilcoxonSignedRank(rankDiffs, minNonZeroR7);
  return {
    n_pairs: n,
    mean_rank_diff: mean(rankDiffs),
    median_rank_diff: median(rankDiffs),
    p_two_sided: wilcoxon.p_value,
    r_c: wilcoxon.r_c,
    wilcoxon,
  };
}

export function evaluatePooledBaseline(runs, minSurviving = 6) {
  const surviving = runs.filter(
    (r) =>
      r.completed &&
      (r.extinct_tick === null || r.extinct_tick === undefined) &&
      Number.isFinite(r.crop_second_half),
  );
  const sufficient = surviving.length >= minSurviving;
  const pooledCrop =
    sufficient && surviving.length > 0
      ? surviving.reduce((acc, r) => acc + r.crop_second_half, 0) / surviving.length
      : null;
  return {
    n_requested: runs.length,
    n_surviving: surviving.length,
    min_required: minSurviving,
    surviving_seeds: surviving.map((r) => r.seed),
    pooled_crop_second_half: pooledCrop,
    sufficient,
  };
}

export function evaluateSmokeGate(
  durationTicks,
  baseline,
  selected,
  scarceMut0,
  scarceDrift,
  smokeGate,
) {
  const byArm = { selected, scarce_mut0: scarceMut0, scarce_drift: scarceDrift };
  const c1MinAgents = Number(smokeGate.c1_min_agents);
  const [c2Lo, c2Hi] = smokeGate.c2_g_band.map(Number);
  const c3MinGens = Number(smokeGate.c3_min_generations);
  const c4MinShare = Number(smokeGate.c4_min_starvation_share);

  const c1Arms = smokeGate.c1_applies_to_arms ?? ['selected', 'scarce_mut0'];
  const c2Arms = smokeGate.c2_applies_to_arms ?? ['selected', 'scarce_mut0'];
  const c3Arms = smokeGate.c3_applies_to_arms ?? ['selected', 'scarce_mut0'];
  const c4Arms = smokeGate.c4_applies_to_arms ?? ['selected', 'scarce_mut0'];
  const c5Arms = smokeGate.c5_applies_to_arms ?? ['selected', 'scarce_mut0'];

  const survived = (x) =>
    Boolean(x) && x.completed && (x.extinct_tick === null || x.extinct_tick === undefined);
  const c1Arm = (x) => survived(x) && Number.isFinite(x.min_agents) && x.min_agents >= c1MinAgents;
  const c1 = c1Arms.every((arm) => c1Arm(byArm[arm]));

  const refCrop =
    baseline.sufficient &&
    Number.isFinite(baseline.pooled_crop_second_half) &&
    baseline.pooled_crop_second_half > 0
      ? baseline.pooled_crop_second_half
      : null;
  const gOf = (x) =>
    refCrop !== null && x && Number.isFinite(x.crop_second_half) ? x.crop_second_half / refCrop : null;
  const c2 = Boolean(baseline.sufficient) && c2Arms.every((arm) => {
    const g = gOf(byArm[arm]);
    return g !== null && g >= c2Lo && g <= c2Hi;
  });

  const gensOf = (x) => {
    if (!x || !(x.total_deaths > 0) || !Number.isFinite(x.mean_lifespan) || x.mean_lifespan <= 0) {
      return null;
    }
    if (!Number.isFinite(x.mean_agents) || x.mean_agents <= 0) return null;
    return Math.min(durationTicks / x.mean_lifespan, x.total_births / Math.max(1.0, x.mean_agents));
  };
  const c3 = c3Arms.every((arm) => {
    const gens = gensOf(byArm[arm]);
    return gens !== null && gens >= c3MinGens;
  });

  const starvShare = (x) => {
    if (!x || !(x.total_deaths > 0) || x.cause_partition_ok !== true) return null;
    if (!Number.isFinite(x.total_starvation) || !Number.isFinite(x.total_dehydration)) return null;
    return x.total_starvation / x.total_deaths;
  };
  const c4 = c4Arms.every((arm) => {
    const sh = starvShare(byArm[arm]);
    return sh !== null && sh > c4MinShare;
  });

  const c5 = c5Arms.every((arm) => Boolean(byArm[arm]) && byArm[arm].births_last_quarter > 0 && byArm[arm].deaths_last_quarter > 0);
  const driftValid = survived(scarceDrift) && scarceDrift.drift_valid === true && scarceDrift.has_shortfall === false;

  return {
    c1,
    c2,
    c3,
    c4,
    c5,
    drift_valid: driftValid,
    gate_pass: c1 && c2 && c3 && c4 && c5 && driftValid,
    g_selected: gOf(selected),
    g_scarce_mut0: gOf(scarceMut0),
    g_scarce_drift: gOf(scarceDrift),
    generations_selected: gensOf(selected),
    generations_scarce_mut0: gensOf(scarceMut0),
    starvation_share_selected: starvShare(selected),
    starvation_share_scarce_mut0: starvShare(scarceMut0),
  };
}

/**
 * Closed 3-label R10 decision mapper for two-sided H1..H3 under QD-SEL2P2-D3=(a), D4=(b), D6=(a).
 * Returns exclusively labels in {supports, equivalent_to_zero, insufficient_evidence}.
 */
export function classifySel2p2Decision({
  smokeAndHostValid,
  failedRunsZero,
  driftAndShortfallValid,
  primaryMode,
  nCompletePairs,
  nRequested,
  minPiLl = 0.85,
  minPairsR7 = 7,
  pHolm,
  fwerAlpha = 0.05,
  meanD,
  tostEquivalent,
  sesoi = Math.log(1.1),
  lachinMeanRankDiff = null,
  saceLo = null,
  saceHi = null,
}) {
  if (!smokeAndHostValid) {
    return { label: 'insufficient_evidence', direction_tag: 'harness_invalid' };
  }
  if (!failedRunsZero || !driftAndShortfallValid) {
    return { label: 'insufficient_evidence', direction_tag: 'attrition_below_floor_or_drift_invalid' };
  }

  const holmSig = typeof pHolm === 'number' && Number.isFinite(pHolm) && pHolm < fwerAlpha;

  if (primaryMode === 'lachin_worst_rank') {
    if (!holmSig) {
      return { label: 'insufficient_evidence', direction_tag: 'inconclusive_wide_ci' };
    }
    const signPos =
      typeof lachinMeanRankDiff === 'number' &&
      lachinMeanRankDiff > 0 &&
      typeof saceLo === 'number' &&
      saceLo >= sesoi;
    const signNeg =
      typeof lachinMeanRankDiff === 'number' &&
      lachinMeanRankDiff < 0 &&
      typeof saceHi === 'number' &&
      saceHi <= -sesoi;
    if (signPos) {
      return { label: 'supports', direction_tag: 'lachin_two_sided_positive' };
    }
    if (signNeg) {
      return { label: 'supports', direction_tag: 'lachin_two_sided_negative' };
    }
    return { label: 'insufficient_evidence', direction_tag: 'attrition_sace_straddles_or_inconclusive' };
  }

  const piLl = nRequested > 0 ? nCompletePairs / nRequested : 0;
  if (nCompletePairs < minPairsR7 || piLl < minPiLl || typeof meanD !== 'number' || !Number.isFinite(meanD)) {
    return { label: 'insufficient_evidence', direction_tag: 'attrition_below_floor_or_drift_invalid' };
  }

  const effBucket =
    Math.abs(meanD) < sesoi
      ? 'lt_sesoi'
      : meanD >= sesoi
        ? 'pos_ge_sesoi'
        : 'neg_ge_sesoi';

  if (effBucket !== 'lt_sesoi' && tostEquivalent) {
    return { label: 'insufficient_evidence', direction_tag: 'ci_inconsistent_impossible' };
  }

  if (holmSig) {
    if (effBucket === 'pos_ge_sesoi') {
      return { label: 'supports', direction_tag: 'two_sided_positive' };
    }
    if (effBucket === 'neg_ge_sesoi') {
      return { label: 'supports', direction_tag: 'two_sided_negative' };
    }
    if (tostEquivalent) {
      return { label: 'equivalent_to_zero', direction_tag: 'trivial_below_sesoi_equivalent' };
    }
    return { label: 'insufficient_evidence', direction_tag: 'inconclusive_straddles_sesoi' };
  }

  if (tostEquivalent && effBucket === 'lt_sesoi') {
    return { label: 'equivalent_to_zero', direction_tag: 'equivalent_to_zero_powered_null' };
  }
  return { label: 'insufficient_evidence', direction_tag: 'inconclusive_wide_ci' };
}

// ---- Repository / Manifest / Provenance / CSV Helpers -----------------------------------------

function gitCommitHasBlob(ref, relPath) {
  if (!isSafeGitCommit(ref)) return false;
  try {
    execFileSync('git', ['cat-file', '-e', `${ref}:${relPath}`], {
      cwd: repoRoot,
      stdio: ['ignore', 'ignore', 'ignore'],
    });
    return true;
  } catch {
    return false;
  }
}

function readCommittedBytesAt(name, ref = 'HEAD') {
  if (ref !== 'HEAD' && !isSafeGitCommit(ref)) return null;
  try {
    return execFileSync('git', ['show', `${ref}:${committedManifestDir}/${name}`], {
      cwd: repoRoot,
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch {
    if (ref === 'HEAD') {
      const wt = join(repoRoot, committedManifestDir, name);
      return existsSync(wt) ? readFileSync(wt) : null;
    }
    return null;
  }
}

export function loadCommittedPrereg(preregOverridePath = null, gitRef = 'HEAD') {
  if (preregOverridePath) {
    return JSON.parse(readFileSync(preregOverridePath, 'utf8'));
  }
  const bytes = readCommittedBytesAt(PREREG_FILE, gitRef);
  if (!bytes) {
    throw new Error(`cannot read ${committedManifestDir}/${PREREG_FILE} from ${gitRef}`);
  }
  return JSON.parse(bytes.toString('utf8'));
}

export function isAllowedDirtyPath(entry) {
  if (typeof entry !== 'string' || entry.trim().length === 0) return false;
  const parts = entry.includes(' -> ') ? entry.split(' -> ') : [entry];
  for (const raw of parts) {
    const p = raw.trim().replace(/\\/g, '/').replace(/^"|"$/g, '');
    if (
      p === 'artifacts' ||
      p.startsWith('artifacts/') ||
      p === 'src-tauri/examples' ||
      p.startsWith('src-tauri/examples/') ||
      p === 'scripts' ||
      p.startsWith('scripts/') ||
      p === 'package.json' ||
      p.startsWith('docs/')
    ) {
      continue;
    }
    return false;
  }
  return true;
}

export function parseCsv(text) {
  const lines = text.replace(/\r\n/g, '\n').split('\n').filter((l) => l.length > 0);
  if (lines.length === 0) return [];
  const headers = lines[0].split(',').map((h) => h.trim());
  return lines.slice(1).map((line) => {
    const vals = line.split(',');
    const row = {};
    for (let i = 0; i < headers.length; i += 1) {
      row[headers[i]] = vals[i] ?? '';
    }
    return row;
  });
}

function parseBoolField(v) {
  if (typeof v === 'boolean') return v;
  const s = String(v).trim().toLowerCase();
  if (s === 'true' || s === '1') return true;
  if (s === 'false' || s === '0') return false;
  return null;
}

function summarizeRunRules(runMeta, runSeries, finalRec, prereg) {
  const durationTicks = Number(prereg.duration.duration_ticks);
  const warmupTicks = Number(prereg.duration.warmup_ticks);
  const tailCutoff = Number(prereg.duration.tail20_threshold_tick ?? 160000);

  const ticks = (runSeries ?? [])
    .map((r) => ({ tick: Number(r.tick), obs: r }))
    .sort((a, b) => a.tick - b.tick);

  let firstExtinctTick = null;
  for (const s of ticks) {
    if (Number(s.obs['live.agent_count']) === 0) {
      firstExtinctTick = s.tick;
      break;
    }
  }
  const finalObs = finalRec?.observables ?? {};
  const finalAgents = Number(finalObs['live.agent_count']);
  if (firstExtinctTick === null && finalAgents === 0) {
    firstExtinctTick = durationTicks;
  }
  const extinct = firstExtinctTick !== null;
  const survivalTicks = firstExtinctTick !== null ? firstExtinctTick : durationTicks;

  const postWarmupSamples = ticks.filter((s) => s.tick > warmupTicks && s.tick <= durationTicks);
  const validPostWarmupMass =
    !extinct &&
    runMeta.status === 'Completed' &&
    postWarmupSamples.length > 0 &&
    postWarmupSamples.every((s) => Number(s.obs['live.agent_count']) >= 1);

  const massMeanInt = validPostWarmupMass
    ? mean(postWarmupSamples.map((s) => Number(s.obs['live.trait_mass_mean'])))
    : null;
  const tailSamples = ticks.filter((s) => s.tick > tailCutoff && Number(s.obs['live.agent_count']) >= 1);
  const massTail20Mean =
    !extinct && tailSamples.length > 0
      ? mean(tailSamples.map((s) => Number(s.obs['live.trait_mass_mean'])))
      : null;

  // F-053: ALWAYS read live.mean_lifespan from series.csv (never final_observables which drained).
  const finalTickSeries = ticks.find((s) => s.tick === durationTicks)?.obs ?? null;
  const lifespanAtT = finalTickSeries ? Number(finalTickSeries['live.mean_lifespan']) : null;
  const birthsSum = ticks.reduce((acc, s) => acc + Number(s.obs['live.births']), 0);
  const deathsSum = ticks.reduce((acc, s) => acc + Number(s.obs['live.deaths']), 0);

  const energyAtT = finalTickSeries ? Number(finalTickSeries['live.mean_energy_integral']) : null;
  const warmSample = ticks.find((s) => s.tick === warmupTicks)?.obs ?? null;
  const energyAtWarm = warmSample ? Number(warmSample['live.mean_energy_integral']) : null;
  const energyPostMean =
    Number.isFinite(energyAtT) && Number.isFinite(energyAtWarm) && durationTicks > warmupTicks
      ? (energyAtT - energyAtWarm) / (durationTicks - warmupTicks)
      : null;

  const isDriftArm = runMeta.arm === 'scarce_drift';
  const driftValid = isDriftArm ? parseBoolField(runMeta.drift_valid) === true : true;
  const hasShortfall = isDriftArm ? parseBoolField(runMeta.has_shortfall) === true : false;

  return {
    seed: Number(runMeta.seed),
    arm: runMeta.arm,
    completed: runMeta.status === 'Completed',
    extinct,
    firstExtinctTick,
    survivalTicks,
    validMass: validPostWarmupMass && Number.isFinite(massMeanInt) && massMeanInt > 0,
    meanInt: massMeanInt,
    massTail20Mean,
    varFinal: Number(finalObs['live.trait_mass_var']),
    energyPostMean,
    finalAgents,
    closedEuTotal: Number(finalObs['live.closed_eu_total']),
    birthsSum,
    deathsSum,
    lifespanAtT,
    cumulativeMeanLifespan: Number(finalObs['live.cumulative_mean_lifespan'] ?? 0),
    driftValid,
    hasShortfall,
  };
}

// ---- Full SEL2-P2 Dataset Evaluation (Independent Recomputation) ------------------------------

export function evaluateSel2p2Dataset({ runs, series, finalObservables, prereg, smokeAndHostValid = true }) {
  const executionOrder = prereg.seeds.execution_order.map(Number);
  const nRequested = executionOrder.length;
  const sesoi = Number(prereg.thresholds.sesoi_log_mean_ratio);
  const fwerAlpha = Number(prereg.thresholds.fwer_alpha);
  const maxExtRate = Number(prereg.thresholds.max_arm_extinction_rate_for_complete_pairs);
  const minPiLl = Number(prereg.thresholds.min_pi_ll);
  const minPairsR7 = Number(prereg.thresholds.min_non_zero_pairs_r7);

  const seriesMap = new Map();
  for (const row of series) {
    const key = `${Number(row.seed)}::${row.arm}`;
    if (!seriesMap.has(key)) seriesMap.set(key, []);
    seriesMap.get(key).push(row);
  }
  const finalMap = new Map();
  for (const f of finalObservables) {
    finalMap.set(`${Number(f.seed)}::${f.arm}`, f);
  }
  const runMap = new Map();
  let failedRunsCount = 0;
  for (const r of runs) {
    const key = `${Number(r.seed)}::${r.arm}`;
    runMap.set(key, r);
    if (r.status !== 'Completed') failedRunsCount += 1;
  }

  const summaries = new Map();
  for (const seed of executionOrder) {
    for (const arm of ARM_ORDER) {
      const key = `${seed}::${arm}`;
      const r = runMap.get(key);
      const f = finalMap.get(key);
      if (r && f) {
        summaries.set(key, summarizeRunRules(r, seriesMap.get(key), f, prereg));
      }
    }
  }

  const failedRunsZero = failedRunsCount === 0 && summaries.size === nRequested * ARM_ORDER.length;

  const rawEntries = CONTRAST_DEFS.map((c) => {
    const meanA = [];
    const meanB = [];
    const obsA = [];
    const obsB = [];
    const survValidA = [];
    const survValidB = [];
    const survAllA = [];
    const survAllB = [];
    const tailA = [];
    const tailB = [];
    const energyA = [];
    const energyB = [];
    let extinctCountA = 0;
    let extinctCountB = 0;
    let driftShortfallOk = true;

    for (const seed of executionOrder) {
      const sa = summaries.get(`${seed}::${c.controlArm}`);
      const sb = summaries.get(`${seed}::${c.treatmentArm}`);
      if (!sa || !sb) continue;

      if (sa.extinct) extinctCountA += 1;
      if (sb.extinct) extinctCountB += 1;
      if (!sa.driftValid || sa.hasShortfall || !sb.driftValid || sb.hasShortfall) {
        driftShortfallOk = false;
      }

      obsA.push(sa);
      obsB.push(sb);
      survAllA.push(sa.survivalTicks);
      survAllB.push(sb.survivalTicks);

      if (sa.validMass) survValidA.push(sa.meanInt);
      if (sb.validMass) survValidB.push(sb.meanInt);

      if (sa.validMass && sb.validMass) {
        meanA.push(sa.meanInt);
        meanB.push(sb.meanInt);
      }
      if (sa.massTail20Mean !== null && sb.massTail20Mean !== null) {
        tailA.push(sa.massTail20Mean);
        tailB.push(sb.massTail20Mean);
      }
      if (sa.energyPostMean !== null && sb.energyPostMean !== null) {
        energyA.push(sa.energyPostMean);
        energyB.push(sb.energyPostMean);
      }
    }

    const pExtA = nRequested > 0 ? extinctCountA / nRequested : 1;
    const pExtB = nRequested > 0 ? extinctCountB / nRequested : 1;
    const completePairsStats = evaluatePairedLogMeanRatio(meanA, meanB, {
      sesoi,
      minNonZeroR7: minPairsR7,
      nRequested,
    });
    const lachin = lachinWorstRankPaired(obsA, obsB, minPairsR7);
    const leeBounds = leeTrimmingBoundsLogMean(survValidA, survValidB);
    const rmst = {
      ...pairedStats(survAllA, survAllB, nRequested),
      wilcoxon: wilcoxonSignedRank(survAllA.map((a, i) => survAllB[i] - a), minPairsR7),
    };

    const completePairEligible =
      pExtA <= maxExtRate &&
      pExtB <= maxExtRate &&
      completePairsStats.pi_ll >= minPiLl &&
      completePairsStats.n_complete_pairs >= minPairsR7;
    const primaryMode = completePairEligible ? 'complete_pairs' : 'lachin_worst_rank';
    const rawPrimaryP = primaryMode === 'complete_pairs' ? completePairsStats.p_two_sided : lachin.p_two_sided;

    return {
      def: c,
      primaryMode,
      rawPrimaryP,
      pExtA,
      pExtB,
      driftShortfallOk,
      completePairsStats,
      lachin,
      leeBounds,
      rmst,
      tail20: tailA.length > 0 && tailA.length === tailB.length
        ? evaluatePairedLogMeanRatio(tailA, tailB, { sesoi, minNonZeroR7: minPairsR7, nRequested })
        : null,
      energyIntegral: pairedStats(energyA, energyB, nRequested),
    };
  });

  const holmP = holmAdjust(rawEntries.map((e) => e.rawPrimaryP));
  const contrasts = {};
  for (let i = 0; i < rawEntries.length; i += 1) {
    const e = rawEntries[i];
    const pHolm = holmP[i];
    const r10 = classifySel2p2Decision({
      smokeAndHostValid,
      failedRunsZero,
      driftAndShortfallValid: e.driftShortfallOk,
      primaryMode: e.primaryMode,
      nCompletePairs: e.completePairsStats.n_complete_pairs,
      nRequested,
      minPiLl,
      minPairsR7,
      pHolm,
      fwerAlpha,
      meanD: e.completePairsStats.mean_D,
      tostEquivalent: e.completePairsStats.tost_equivalent,
      sesoi,
      lachinMeanRankDiff: e.lachin.mean_rank_diff,
      saceLo: e.leeBounds.lee_lo,
      saceHi: e.leeBounds.lee_hi,
    });
    contrasts[e.def.id] = {
      id: e.def.id,
      name: e.def.name,
      primary_mode: e.primaryMode,
      p_ext_control: e.pExtA,
      p_ext_treatment: e.pExtB,
      drift_and_shortfall_valid: e.driftShortfallOk,
      p_holm: pHolm,
      decision: r10.label,
      direction_tag: r10.direction_tag,
      complete_pairs: e.completePairsStats,
      lachin_worst_rank: e.lachin,
      lee_bounds: e.leeBounds,
      rmst: e.rmst,
      tail20: e.tail20,
      energy_integral: e.energyIntegral,
    };
  }

  return {
    primary_decision: contrasts.H1.decision,
    primary_direction_tag: contrasts.H1.direction_tag,
    primary_mode: contrasts.H1.primary_mode,
    failed_runs_count: failedRunsCount,
    contrasts,
  };
}

// ---- Provenance, Checksum & Blinding Verification ---------------------------------------------

function approxEq(a, b, tol = 1e-8) {
  if (a === null && b === null) return true;
  if (typeof a !== 'number' || typeof b !== 'number') return false;
  if (!Number.isFinite(a) || !Number.isFinite(b)) return false;
  return Math.abs(a - b) <= tol * Math.max(1, Math.abs(a), Math.abs(b));
}

function diffPreregAgainstHeadAllowingAddedDefaultZeros(artifactPrereg, headPrereg) {
  const diffs = [];
  const walk = (a, b, path) => {
    if (diffs.length >= 16) return;
    if (path === 'default_keys_left_absent') {
      const keys = new Set([...Object.keys(a ?? {}), ...Object.keys(b ?? {})]);
      for (const k of keys) {
        if (!Object.hasOwn(a ?? {}, k)) {
          if (b[k] !== 0 && b[k] !== 0.0) {
            diffs.push(`${path}.${k} added with non-zero default ${b[k]}`);
          }
        } else if (!Object.hasOwn(b ?? {}, k)) {
          diffs.push(`${path}.${k} missing in HEAD`);
        } else if (a[k] !== b[k]) {
          diffs.push(`${path}.${k}: ${a[k]} vs ${b[k]}`);
        }
      }
      return;
    }
    if (Array.isArray(a) && Array.isArray(b)) {
      if (a.length !== b.length) {
        diffs.push(`${path}: array length ${a.length} vs ${b.length}`);
        return;
      }
      for (let i = 0; i < a.length; i += 1) walk(a[i], b[i], `${path}[${i}]`);
      return;
    }
    if (a && b && typeof a === 'object' && typeof b === 'object' && !Array.isArray(a) && !Array.isArray(b)) {
      const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
      for (const k of keys) {
        if (!Object.hasOwn(a, k) || !Object.hasOwn(b, k)) {
          diffs.push(`${path ? `${path}.` : ''}${k}: missing on one side`);
        } else {
          walk(a[k], b[k], path ? `${path}.${k}` : k);
        }
      }
      return;
    }
    if (a !== b) diffs.push(`${path}: ${JSON.stringify(a)} vs ${JSON.stringify(b)}`);
  };
  walk(artifactPrereg, headPrereg, '');
  return diffs;
}

function verifyChecksumManifest(dir, requiredEntries, fail, labelPrefix) {
  const checksumPath = join(dir, 'checksums.sha256');
  const covered = new Map();
  if (!existsSync(checksumPath)) {
    fail(`${labelPrefix}: missing checksums.sha256`);
    return covered;
  }
  const lines = readFileSync(checksumPath, 'utf8')
    .replace(/\r\n/g, '\n')
    .split('\n')
    .filter((l) => l.length > 0);
  if (lines.length === 0) {
    fail(`${labelPrefix}: checksums.sha256 is empty`);
    return covered;
  }
  for (const line of lines) {
    const m = /^([0-9a-f]{64}) {2}(.+)$/.exec(line);
    if (!m) {
      fail(`${labelPrefix}: malformed checksums.sha256 line '${line}'`);
      continue;
    }
    const [, expectedHex, rel] = m;
    covered.set(rel, expectedHex);
    const target = join(dir, rel);
    if (!existsSync(target)) {
      fail(`${labelPrefix}: checksums.sha256 references missing file ${rel}`);
    } else {
      const actualHex = sha256(readFileSync(target));
      if (actualHex !== expectedHex) {
        fail(`${labelPrefix}: SHA-256 mismatch for ${rel}`);
      }
    }
  }
  for (const req of requiredEntries) {
    if (!covered.has(req)) {
      fail(`${labelPrefix}: checksums.sha256 omits ${req}`);
    }
  }
  return covered;
}

export function assertDirContainsNoTraitMass(dir, fail, label) {
  const walk = (cur) => {
    for (const ent of readdirSync(cur, { withFileTypes: true })) {
      const full = join(cur, ent.name);
      if (ent.isDirectory()) walk(full);
      else if (statSync(full).isFile()) {
        const text = readFileSync(full, 'utf8');
        if (text.toLowerCase().includes('trait_mass')) {
          fail(`${label}: forbidden 'trait_mass' substring found in ${relative(dir, full)}`);
        }
      }
    }
  };
  walk(dir);
}

export function verifyArtifactDir(dir, opts = {}) {
  const failures = [];
  const notes = [];
  const fail = (msg) => failures.push(msg);

  if (!existsSync(dir)) {
    fail(`artifact directory does not exist: ${dir}`);
    return { ok: false, failures, notes, usedRecordedFallback: false, recordedCommit: null };
  }

  const requiredFiles = ['provenance.json', 'runs.csv', 'series.csv', 'final-observables.json', 'effects.json'];
  for (const req of requiredFiles) {
    if (!existsSync(join(dir, req))) fail(`missing required file: ${req}`);
  }
  verifyChecksumManifest(dir, requiredFiles, fail, 'confirmation');

  let prov = null;
  if (existsSync(join(dir, 'provenance.json'))) {
    try {
      prov = JSON.parse(readFileSync(join(dir, 'provenance.json'), 'utf8'));
    } catch (e) {
      fail(`invalid provenance.json: ${e.message}`);
    }
  }

  const recordedCommit = prov?.build?.git_commit ?? null;
  const preregRel = `${committedManifestDir}/${PREREG_FILE}`;
  let manifestGitRef = null;
  if (!(opts.preregOverride || opts.preregPath)) {
    if (recordedCommit && gitCommitHasBlob(recordedCommit, preregRel)) {
      manifestGitRef = recordedCommit;
    } else {
      notes.push(`commit '${recordedCommit ?? '<missing>'}' unavailable locally; comparing against working-tree manifests`);
    }
  } else {
    manifestGitRef = 'HEAD';
  }
  const useFallback = !(opts.preregOverride || opts.preregPath) && manifestGitRef === null;

  let prereg = opts.preregOverride ?? null;
  if (!prereg) {
    if (opts.preregPath) {
      prereg = loadCommittedPrereg(opts.preregPath, 'HEAD');
    } else if (!useFallback) {
      prereg = loadCommittedPrereg(null, manifestGitRef);
    } else {
      const pPath = join(dir, 'manifests', PREREG_FILE);
      if (!existsSync(pPath)) {
        fail(`missing manifest copy: manifests/${PREREG_FILE}`);
        return { ok: false, failures, notes, usedRecordedFallback: true, recordedCommit };
      }
      prereg = JSON.parse(readFileSync(pPath, 'utf8'));
    }
  }

  const manifestDir = join(dir, 'manifests');
  if (!existsSync(manifestDir)) {
    fail('missing manifests/ directory');
  } else if (!opts.skipCommittedManifestCheck) {
    const expectedManifests = [
      PREREG_FILE,
      prereg.manifests.selected,
      prereg.manifests.scarce_mut0,
      prereg.manifests.scarce_drift,
    ];
    const headPrereg = useFallback ? JSON.parse(readFileSync(join(repoRoot, preregRel), 'utf8')) : null;
    for (const mf of expectedManifests) {
      const copyPath = join(manifestDir, mf);
      if (!existsSync(copyPath)) {
        fail(`missing manifest copy: manifests/${mf}`);
        continue;
      }
      if (useFallback) {
        const copyBytes = readFileSync(copyPath);
        const recordedSha = prov?.manifests?.sha256_as_committed?.[mf];
        if (typeof recordedSha === 'string' && sha256NormalizedLf(copyBytes) !== recordedSha) {
          fail(`manifests/${mf} SHA-256 differs from provenance.manifests.sha256_as_committed`);
        }
        if (mf === PREREG_FILE && headPrereg) {
          const diffs = diffPreregAgainstHeadAllowingAddedDefaultZeros(prereg, headPrereg);
          if (diffs.length > 0) fail(`manifests/${PREREG_FILE} differs from HEAD: ${diffs.join('; ')}`);
        } else if (mf !== PREREG_FILE) {
          const wtBytes = readFileSync(join(repoRoot, committedManifestDir, mf));
          if (sha256NormalizedLf(copyBytes) !== sha256NormalizedLf(wtBytes)) {
            fail(`manifests/${mf} differs from working-tree manifest in repo`);
          }
        }
      } else {
        const committed = readCommittedBytesAt(mf, manifestGitRef);
        if (!committed || sha256NormalizedLf(readFileSync(copyPath)) !== sha256NormalizedLf(committed)) {
          fail(`manifests/${mf} differs from committed fixture at ${manifestGitRef}`);
        }
      }
    }
  }

  if (!prov) {
    return { ok: false, failures, notes, usedRecordedFallback: useFallback, recordedCommit };
  }

  if (prov.stopped_early !== false) fail(`provenance.stopped_early must be false, got ${prov.stopped_early}`);
  if (prov.build?.profile !== 'release') fail(`provenance.build.profile must be 'release', got ${prov.build?.profile}`);
  if (!Array.isArray(prov.build?.git_dirty_paths)) {
    fail('provenance.build.git_dirty_paths must be an array');
  } else {
    for (const dp of prov.build.git_dirty_paths) {
      if (!isAllowedDirtyPath(dp)) fail(`disallowed dirty working tree path: ${dp}`);
    }
  }
  if (prov.terrain_source !== 'generated') fail(`provenance.terrain_source != 'generated'`);
  if (
    prov.world_artifact_prerun_check?.anima_world_artifact_unset !== true ||
    prov.world_artifact_prerun_check?.temp_artifact_absent !== true
  ) {
    fail('provenance.world_artifact_prerun_check did not confirm unset env + absent temp artifact');
  }

  const wid = prov.world_identity;
  const sameWid = (a, b) =>
    a && b &&
    Number(a.seed) === Number(b.seed) &&
    Number(a.generator_version) === Number(b.generator_version) &&
    Number(a.width) === Number(b.width) &&
    Number(a.height) === Number(b.height) &&
    Number(a.checksum) === Number(b.checksum);
  if (!wid || typeof wid.checksum !== 'number') {
    fail('provenance.world_identity missing');
  }

  const sr = prov.smoke_reference;
  let smokeAndHostValid = true;
  if (sr?.smoke_dir) {
    const smokeDir = isAbsolute(sr.smoke_dir) ? sr.smoke_dir : join(repoRoot, sr.smoke_dir);
    if (!existsSync(smokeDir)) {
      fail(`referenced smoke_dir does not exist: ${sr.smoke_dir}`);
      smokeAndHostValid = false;
    } else {
      if (sr.gate_pass !== true) {
        fail('smoke_reference.gate_pass is not true');
        smokeAndHostValid = false;
      }
      verifyChecksumManifest(smokeDir, ['provenance.json', 'smoke-report.json'], fail, 'smoke_reference');
      assertDirContainsNoTraitMass(smokeDir, fail, 'smoke_reference');
    }
  } else if (sr && sr.gate_pass !== true) {
    fail('smoke_reference.gate_pass is not true');
    smokeAndHostValid = false;
  }

  if (failures.length > 0 && !existsSync(join(dir, 'effects.json'))) {
    return { ok: false, failures, notes, usedRecordedFallback: useFallback, recordedCommit };
  }

  const runs = parseCsv(readFileSync(join(dir, 'runs.csv'), 'utf8'));
  const series = parseCsv(readFileSync(join(dir, 'series.csv'), 'utf8'));
  const finals = JSON.parse(readFileSync(join(dir, 'final-observables.json'), 'utf8'));
  const effects = JSON.parse(readFileSync(join(dir, 'effects.json'), 'utf8'));

  const expectedOrder = prereg.seeds.execution_order.map(Number);
  const smokeSeed = Number(prereg.seeds.smoke_seed);
  if (runs.length !== expectedOrder.length * ARM_ORDER.length) {
    fail(`runs.csv has ${runs.length} rows, expected ${expectedOrder.length * ARM_ORDER.length}`);
  }
  for (const r of runs) {
    if (Number(r.seed) === smokeSeed) fail(`smoke_seed ${smokeSeed} leaked into runs.csv`);
    if (r.terrain_source !== 'generated') fail(`runs.csv terrain_source != generated at seed=${r.seed}`);
    if (wid && !sameWid(wid, {
      seed: r.world_seed,
      generator_version: r.world_gen_version,
      width: r.world_width,
      height: r.world_height,
      checksum: r.world_checksum,
    })) {
      fail(`runs.csv world_identity mismatch at seed=${r.seed} arm=${r.arm}`);
    }
  }
  for (const f of finals) {
    if (Number(f.seed) === smokeSeed) fail(`smoke_seed ${smokeSeed} leaked into final-observables.json`);
    if (f.terrain_source !== 'generated' || (wid && !sameWid(wid, f.world_identity))) {
      fail(`final-observables.json world_identity mismatch at seed=${f.seed} arm=${f.arm}`);
    }
    const eu = Number(f.observables?.['live.closed_eu_total']);
    if (!Number.isFinite(eu)) fail(`non-finite live.closed_eu_total at seed=${f.seed} arm=${f.arm}`);
  }

  if (effects.s3_lifespan_source && effects.s3_lifespan_source !== 'series.csv') {
    fail(`F-053 violation: s3_lifespan_source must be 'series.csv', got ${effects.s3_lifespan_source}`);
  }

  const recomputed = evaluateSel2p2Dataset({
    runs,
    series,
    finalObservables: finals,
    prereg,
    smokeAndHostValid,
  });
  if (effects.primary_decision !== recomputed.primary_decision) {
    fail(`primary_decision mismatch: expected '${recomputed.primary_decision}', got '${effects.primary_decision}'`);
  }
  if (effects.primary_direction_tag !== recomputed.primary_direction_tag) {
    fail(`primary_direction_tag mismatch: expected '${recomputed.primary_direction_tag}', got '${effects.primary_direction_tag}'`);
  }
  if (effects.primary_mode && effects.primary_mode !== recomputed.primary_mode) {
    fail(`primary_mode mismatch: expected '${recomputed.primary_mode}', got '${effects.primary_mode}'`);
  }

  for (const cid of ['H1', 'H2', 'H3']) {
    const obs = effects.contrasts?.[cid];
    const exp = recomputed.contrasts[cid];
    if (!obs) {
      fail(`effects.json missing contrast '${cid}'`);
      continue;
    }
    if (obs.decision !== exp.decision) fail(`${cid}.decision expected '${exp.decision}', got '${obs.decision}'`);
    if (obs.direction_tag !== exp.direction_tag) {
      fail(`${cid}.direction_tag expected '${exp.direction_tag}', got '${obs.direction_tag}'`);
    }
    if (obs.primary_mode !== exp.primary_mode) {
      fail(`${cid}.primary_mode expected '${exp.primary_mode}', got '${obs.primary_mode}'`);
    }
    if (!approxEq(obs.p_holm, exp.p_holm)) fail(`${cid}.p_holm expected ${exp.p_holm}, got ${obs.p_holm}`);
    for (const k of ['n_complete_pairs', 'mean_D', 'sd_D', 'se_D', 'p_two_sided', 'ci95_low', 'ci95_high', 'ci90_tost_low', 'ci90_tost_high', 'paired_dz']) {
      if (!approxEq(obs.complete_pairs?.[k], exp.complete_pairs?.[k])) {
        fail(`${cid}.complete_pairs.${k} expected ${exp.complete_pairs?.[k]}, got ${obs.complete_pairs?.[k]}`);
      }
    }
    if (Boolean(obs.complete_pairs?.tost_equivalent) !== Boolean(exp.complete_pairs?.tost_equivalent)) {
      fail(`${cid}.complete_pairs.tost_equivalent mismatch`);
    }
    if (!approxEq(obs.lachin_worst_rank?.p_two_sided, exp.lachin_worst_rank?.p_two_sided)) {
      fail(`${cid}.lachin_worst_rank.p_two_sided mismatch`);
    }
    if (!approxEq(obs.lee_bounds?.lee_lo, exp.lee_bounds?.lee_lo) || !approxEq(obs.lee_bounds?.lee_hi, exp.lee_bounds?.lee_hi)) {
      fail(`${cid}.lee_bounds mismatch`);
    }
    if (!approxEq(obs.rmst?.paired_mean_delta, exp.rmst?.paired_mean_delta)) {
      fail(`${cid}.rmst.paired_mean_delta mismatch`);
    }
  }

  return {
    ok: failures.length === 0,
    failures,
    notes,
    usedRecordedFallback: useFallback,
    recordedCommit,
  };
}

// ---- Self-Test Mode (--self-test) -------------------------------------------------------------

function rehashConfirmBundle(confirmDir) {
  const files = ['provenance.json', 'runs.csv', 'series.csv', 'final-observables.json', 'effects.json'];
  const lines = files.map((f) => `${sha256(readFileSync(join(confirmDir, f)))}  ${f}`);
  writeFileSync(join(confirmDir, 'checksums.sha256'), lines.join('\n') + '\n');
}

function buildSyntheticSel2p2Bundle(rootDir, prereg) {
  const confirmDir = join(rootDir, 'confirmation');
  const smokeDir = join(rootDir, 'smoke');
  mkdirSync(join(confirmDir, 'manifests'), { recursive: true });
  mkdirSync(join(smokeDir, 'manifests'), { recursive: true });

  const wid = { seed: 1337, generator_version: 1, width: 256, height: 256, checksum: 3152323152 };
  const seeds = prereg.seeds.execution_order.map(Number);

  const smokeReport = { seed: 999931, body_mass_consulted: false, gate_pass: true };
  writeFileSync(join(smokeDir, 'smoke-report.json'), JSON.stringify(smokeReport, null, 2));
  const smokeProv = {
    build: { profile: 'release', git_commit: 'deadbeef', git_dirty_paths: [] },
    host: { os: 'windows', arch: 'x86_64' },
    terrain_source: 'generated',
    world_identity_observed: wid,
  };
  writeFileSync(join(smokeDir, 'provenance.json'), JSON.stringify(smokeProv, null, 2));
  const smokeChecksums = [
    `${sha256(readFileSync(join(smokeDir, 'provenance.json')))}  provenance.json`,
    `${sha256(readFileSync(join(smokeDir, 'smoke-report.json')))}  smoke-report.json`,
  ].join('\n') + '\n';
  writeFileSync(join(smokeDir, 'checksums.sha256'), smokeChecksums);

  const runsRows = ['seed,arm,status,drift_valid,has_shortfall,terrain_source,world_seed,world_gen_version,world_width,world_height,world_checksum'];
  const seriesRows = ['seed,arm,tick,live.agent_count,live.deaths,live.births,live.mean_lifespan,live.mean_energy_integral,live.trait_mass_mean'];
  const finals = [];

  for (let i = 0; i < seeds.length; i += 1) {
    const seed = seeds[i];
    const jitter = (i % 6) * 0.015;
    const armMass = {
      selected: 2.95 + jitter,
      scarce_mut0: 2.50,
      scarce_drift: 2.52 + 0.3 * jitter,
    };
    for (const arm of ARM_ORDER) {
      runsRows.push(`${seed},${arm},Completed,true,false,generated,${wid.seed},${wid.generator_version},${wid.width},${wid.height},${wid.checksum}`);
      for (const tick of [20000, 100000, 180000, 200000]) {
        const energyInt = tick * 0.25;
        seriesRows.push(`${seed},${arm},${tick},130,4,6,4300,${energyInt},${armMass[arm]}`);
      }
      finals.push({
        seed,
        arm,
        terrain_source: 'generated',
        world_identity: wid,
        observables: {
          'live.agent_count': 130,
          'live.closed_eu_total': 153587.25,
          'live.trait_mass_mean': armMass[arm],
          'live.trait_mass_var': arm === 'scarce_mut0' ? 0.0 : 0.02,
          'live.mean_lifespan': 0.0,
          'live.cumulative_mean_lifespan': 4250,
        },
      });
    }
  }

  const runsText = runsRows.join('\n') + '\n';
  const seriesText = seriesRows.join('\n') + '\n';
  const finalsText = JSON.stringify(finals, null, 2);
  writeFileSync(join(confirmDir, 'runs.csv'), runsText);
  writeFileSync(join(confirmDir, 'series.csv'), seriesText);
  writeFileSync(join(confirmDir, 'final-observables.json'), finalsText);

  const runsParsed = parseCsv(runsText);
  const seriesParsed = parseCsv(seriesText);
  const computed = evaluateSel2p2Dataset({ runs: runsParsed, series: seriesParsed, finalObservables: finals, prereg });
  const effectsObj = { ...computed, s3_lifespan_source: 'series.csv' };
  writeFileSync(join(confirmDir, 'effects.json'), JSON.stringify(effectsObj, null, 2));

  const provObj = {
    stopped_early: false,
    build: { profile: 'release', git_commit: 'deadbeef', git_dirty_paths: [] },
    host: { os: 'windows', arch: 'x86_64' },
    terrain_source: 'generated',
    world_identity: wid,
    world_artifact_prerun_check: { anima_world_artifact_unset: true, temp_artifact_absent: true },
    smoke_reference: { gate_pass: true, smoke_dir: smokeDir },
  };
  writeFileSync(join(confirmDir, 'provenance.json'), JSON.stringify(provObj, null, 2));

  for (const mf of [
    PREREG_FILE,
    prereg.manifests.selected,
    prereg.manifests.scarce_mut0,
    prereg.manifests.scarce_drift,
  ]) {
    writeFileSync(join(confirmDir, 'manifests', mf), readFileSync(join(repoRoot, committedManifestDir, mf)));
  }

  rehashConfirmBundle(confirmDir);
  return { confirmDir, smokeDir, runsPath: join(confirmDir, 'runs.csv') };
}

function runSelfTest() {
  let bad = 0;
  const expect = (cond, label) => {
    if (!cond) {
      bad += 1;
      console.error(`SELF-TEST FAIL  ${label}`);
    } else {
      console.log(`self-test ok    ${label}`);
    }
  };

  // 1. Exact Student-t quantiles for N=48 (df=47) & SESOI = ln(1.10).
  const t975 = studentTInv(0.975, 47);
  const t950 = studentTInv(0.95, 47);
  expect(Math.abs(t975 - 2.01174051) < 1e-6, `studentTInv(0.975, 47) ~ 2.011741 (got ${t975.toFixed(6)})`);
  expect(Math.abs(t950 - 1.67792672) < 1e-6, `studentTInv(0.950, 47) ~ 1.677927 (got ${t950.toFixed(6)})`);

  // 2. QD-SEL2g pooled baseline (12 seeds, min_surviving = 6) & C1..C5 smoke gates.
  const makeBase = (nSurv) =>
    Array.from({ length: 12 }, (_, idx) => ({
      seed: 970501 + idx,
      completed: true,
      extinct_tick: idx < nSurv ? null : 4000,
      crop_second_half: 125000 + idx * 100,
    }));
  const bPass = evaluatePooledBaseline(makeBase(6), 6);
  const bFail = evaluatePooledBaseline(makeBase(5), 6);
  expect(bPass.sufficient === true && bFail.sufficient === false, 'QD-SEL2g 6/12 sufficient vs 5/12 insufficient');

  const prereg = loadCommittedPrereg(null, 'HEAD');
  const armOk = {
    completed: true,
    extinct_tick: null,
    min_agents: 63,
    mean_agents: 136,
    crop_second_half: 65000,
    total_deaths: 4400,
    total_births: 4500,
    mean_lifespan: 4200,
    total_starvation: 3000,
    total_dehydration: 1400,
    cause_partition_ok: true,
    births_last_quarter: 900,
    deaths_last_quarter: 880,
    drift_valid: true,
    has_shortfall: false,
  };
  const sgPass = evaluateSmokeGate(200000, bPass, armOk, armOk, armOk, prereg.smoke_gate);
  const sgFailC4 = evaluateSmokeGate(
    200000,
    bPass,
    { ...armOk, total_starvation: 1000, total_dehydration: 3400 },
    armOk,
    armOk,
    prereg.smoke_gate,
  );
  const sgFailDrift = evaluateSmokeGate(
    200000,
    bPass,
    armOk,
    armOk,
    { ...armOk, has_shortfall: true },
    prereg.smoke_gate,
  );
  expect(sgPass.gate_pass && !sgFailC4.gate_pass && !sgFailDrift.gate_pass, 'C1..C5 smoke gate pass & negative controls');

  // 3. Log mean ratio D, Holm m=3, Lee bounds, and Lachin worst-rank D6(a) strategy switch.
  const dPos = evaluatePairedLogMeanRatio([2.5, 2.51, 2.52, 2.5], [2.9, 2.95, 2.91, 2.92]);
  expect(dPos.mean_D > Math.log(1.1) && dPos.p_two_sided < 0.01, 'evaluatePairedLogMeanRatio positive shift > ln(1.10)');
  const bca = hodgesLehmannBcaCi([0.1, 0.12, 0.14, 0.15, 0.16, 0.18, 0.19, 0.2], { nBoot: 1000 });
  expect(bca.ci_low < bca.estimate && bca.estimate < bca.ci_high, 'hodgesLehmannBcaCi bracket');
  const lee = leeTrimmingBoundsLogMean([2.5, 2.52], [2.8, 2.9, 3.0]);
  expect(lee.k === 2 && lee.lee_hi > lee.lee_lo, 'leeTrimmingBoundsLogMean bounds');
  const obsLachinA = Array.from({ length: 16 }, (_, i) => ({ validMass: true, survivalTicks: 200000, meanInt: 2.50 + i * 0.001 }));
  const obsLachinB = Array.from({ length: 16 }, (_, i) =>
    i < 2
      ? { validMass: false, survivalTicks: 40000 + i * 1000, meanInt: null }
      : { validMass: true, survivalTicks: 200000, meanInt: 2.90 + i * 0.001 },
  );
  const lachin = lachinWorstRankPaired(obsLachinA, obsLachinB, 7);
  expect(lachin.n_pairs === 16 && lachin.wilcoxon.p_value < 0.05, 'lachinWorstRankPaired detects rank shift despite attrition');

  // 4. Exhaustive closed 3-label R10 discrete check (48 complete-pair states + Lachin attrition switch).
  const counts = { supports: 0, equivalent_to_zero: 0, insufficient_evidence: 0 };
  for (const host of [true, false]) {
    for (const driftOk of [true, false]) {
      for (const holm of [true, false]) {
        for (const meanD of [0.15, -0.15, 0.02]) {
          for (const tost of [true, false]) {
            const res = classifySel2p2Decision({
              smokeAndHostValid: host,
              failedRunsZero: driftOk,
              driftAndShortfallValid: driftOk,
              primaryMode: 'complete_pairs',
              nCompletePairs: 48,
              nRequested: 48,
              pHolm: holm ? 0.01 : 0.20,
              meanD,
              tostEquivalent: tost,
            });
            expect(res.label !== 'opposite', 'R10 3-label closed set never emits opposite');
            counts[res.label] += 1;
          }
        }
      }
    }
  }
  expect(
    counts.supports === 2 && counts.equivalent_to_zero === 2 && counts.insufficient_evidence === 44,
    'R10 exhaustive 48/48 closed 3-label partition (2/2/44)',
  );

  const lachinPos = classifySel2p2Decision({
    smokeAndHostValid: true,
    failedRunsZero: true,
    driftAndShortfallValid: true,
    primaryMode: 'lachin_worst_rank',
    nCompletePairs: 38,
    nRequested: 48,
    pHolm: 0.005,
    lachinMeanRankDiff: 18.5,
    saceLo: 0.11,
    saceHi: 0.16,
  });
  expect(lachinPos.label === 'supports' && lachinPos.direction_tag === 'lachin_two_sided_positive', 'QD-SEL2P2-D6(a) Lachin worst-rank positive branch');

  // 5. Full synthetic N=48 bundle pass + 4 negative tamper controls.
  const workDir = join(tmpdir(), `anima-sel2p2-selftest-${process.pid}-${Date.now()}`);
  try {
    const { confirmDir, smokeDir, runsPath } = buildSyntheticSel2p2Bundle(workDir, prereg);
    const clean = verifyArtifactDir(confirmDir, { preregOverride: prereg, skipCommittedManifestCheck: true });
    expect(clean.ok, `clean synthetic N=48 bundle passes (${clean.failures.join('; ')})`);

    // Tamper A: trait_mass leaked into smoke directory.
    writeFileSync(join(smokeDir, 'leak.txt'), 'live.trait_mass_mean = 2.55\n');
    const leak = verifyArtifactDir(confirmDir, { preregOverride: prereg, skipCommittedManifestCheck: true });
    expect(!leak.ok && leak.failures.some((f) => f.includes('trait_mass')), 'negative control: trait_mass in smoke fails');
    rmSync(join(smokeDir, 'leak.txt'));

    // Tamper B: corrupted R10 primary_decision in effects.json (rehashed).
    const effPath = join(confirmDir, 'effects.json');
    const origEff = readFileSync(effPath, 'utf8');
    const badEff = JSON.parse(origEff);
    badEff.primary_decision = 'equivalent_to_zero';
    writeFileSync(effPath, JSON.stringify(badEff, null, 2));
    rehashConfirmBundle(confirmDir);
    const tamDec = verifyArtifactDir(confirmDir, { preregOverride: prereg, skipCommittedManifestCheck: true });
    expect(!tamDec.ok && tamDec.failures.some((f) => f.includes('primary_decision')), 'negative control: tampered decision fails');
    writeFileSync(effPath, origEff);
    rehashConfirmBundle(confirmDir);

    // Tamper C: drift_valid=false or has_shortfall=true in runs.csv (rehashed) without updating effects.json.
    const origRuns = readFileSync(runsPath, 'utf8');
    const badDriftRuns = origRuns.replace('scarce_drift,Completed,true,false,', 'scarce_drift,Completed,false,true,');
    writeFileSync(runsPath, badDriftRuns);
    rehashConfirmBundle(confirmDir);
    const badDriftRes = verifyArtifactDir(confirmDir, { preregOverride: prereg, skipCommittedManifestCheck: true });
    expect(!badDriftRes.ok && badDriftRes.failures.some((f) => f.includes('H1.decision')), 'negative control: drift_valid/shortfall falsification fails');
    writeFileSync(runsPath, origRuns);
    rehashConfirmBundle(confirmDir);

    // Tamper D: checksum mismatch (trailing newline appended without rehash).
    writeFileSync(runsPath, origRuns + '\n');
    const badCsum = verifyArtifactDir(confirmDir, { preregOverride: prereg, skipCommittedManifestCheck: true });
    expect(!badCsum.ok && badCsum.failures.some((f) => f.includes('SHA-256 mismatch')), 'negative control: checksum mismatch fails');
  } finally {
    rmSync(workDir, { recursive: true, force: true });
  }

  if (bad > 0) {
    console.error(`verify_sel2p2_artifacts --self-test FAILED (${bad} failure(s))`);
    process.exit(1);
  }
  console.log('verify_sel2p2_artifacts --self-test PASSED');
}

function main() {
  const argv = process.argv.slice(2);
  if (argv.includes('--self-test')) {
    runSelfTest();
    return;
  }

  let dirArg = null;
  let preregPath = null;
  let skipCommittedManifestCheck = false;
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === '--prereg') preregPath = resolve(argv[++i]);
    else if (argv[i] === '--skip-committed-manifests') skipCommittedManifestCheck = true;
    else if (!argv[i].startsWith('--')) dirArg = resolve(argv[i]);
  }

  const targetDir = dirArg ?? join(repoRoot, DEFAULT_CONFIRM_DIR);
  if (!dirArg && !existsSync(targetDir)) {
    console.log(`check:sel2p2 — no confirmation artifacts at ${DEFAULT_CONFIRM_DIR} yet (pre-SEL2P2-run ok)`);
    return;
  }

  const res = verifyArtifactDir(targetDir, { preregPath, skipCommittedManifestCheck });
  for (const n of res.notes) console.warn(`verify_sel2p2_artifacts NOTE: ${n}`);
  if (!res.ok) {
    console.error(`verify_sel2p2_artifacts FAILED (${res.failures.length} failure(s)):`);
    for (const f of res.failures) console.error(`  - ${f}`);
    process.exit(1);
  }
  console.log(`verify_sel2p2_artifacts OK (${targetDir})`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main();
}
