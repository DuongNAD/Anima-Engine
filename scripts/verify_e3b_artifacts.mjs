// Independent post-hoc verifier for Experiment E3b:
//   node scripts/verify_e3b_artifacts.mjs [artifact-dir] [--prereg <path>]
//   node scripts/verify_e3b_artifacts.mjs --self-test
//
// Defaults to `artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation`.
// Exits 0 without reading/creating results if confirmation artifacts do not yet exist (pre-E3B-run).

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
const committedManifestDir = 'src-tauri/tests/fixtures/experiments_e3b';
const DEFAULT_CONFIRM_DIR = 'artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation';
const PREREG_FILE = 'e3b-preregistration.json';

const ARM_ORDER = [
  'control_uniform',
  'control_uniform_scarcity',
  'treatment_gradient',
];

const CONTRAST_DEFS = [
  {
    id: 'H1',
    name: 'treatment_gradient / control_uniform_scarcity',
    controlArm: 'control_uniform_scarcity',
    treatmentArm: 'treatment_gradient',
    directionalPositive: true,
  },
  {
    id: 'H2',
    name: 'treatment_gradient / control_uniform',
    controlArm: 'control_uniform',
    treatmentArm: 'treatment_gradient',
    directionalPositive: false,
  },
  {
    id: 'H3',
    name: 'control_uniform_scarcity / control_uniform',
    controlArm: 'control_uniform',
    treatmentArm: 'control_uniform_scarcity',
    directionalPositive: false,
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
  // Acklam rational approximation followed by Newton polish against normCdf.
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

export function logVarianceRatio(varB, varA, eps = 5e-4) {
  return Math.log((varB + eps) / (varA + eps));
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
    // Exact distribution of W+ over 2^n signed subsets.
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
      // Normal approx matching scipy zero_method='wilcox' (without continuity correction unless 0).
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

export function hodgesLehmannBcaCi(xs, { nBoot = 10000, alpha = 0.05, seed = 0xe3b00001n } = {}) {
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

// ---- Paired L_i analysis, Holm-Bonferroni, Lee (2009), RMST, Mediator, R10 classifier ---------

export function evaluatePairedLogVarianceRatio(varA, varB, { eps = 5e-4, sesoi = Math.log(1.5), minNonZeroR7 = 7, nRequested = varA.length } = {}) {
  if (varA.length !== varB.length) {
    throw new Error('varA and varB must have equal length');
  }
  const n = varA.length;
  const diffs = varA.map((va, i) => logVarianceRatio(varB[i], va, eps));
  const wilcoxon = wilcoxonSignedRank(diffs, minNonZeroR7);
  const out = {
    n_requested: nRequested,
    n_complete_pairs: n,
    pi_ll: nRequested > 0 ? n / nRequested : 0,
    eps,
    mean_L: null,
    geom_var_ratio: null,
    sd_L: null,
    se_L: null,
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
    concordance_pos: n > 0 ? diffs.filter((d) => d > 0).length / n : null,
    concordance_neg: n > 0 ? diffs.filter((d) => d < 0).length / n : null,
    median_L: median(diffs),
    hl_pseudomedian: hodgesLehmannPseudian_safe(diffs),
    wilcoxon,
    diffs,
  };
  if (n === 0) return out;
  const m = mean(diffs);
  out.mean_L = m;
  out.geom_var_ratio = Math.exp(m);
  if (n < 2) return out;

  const sd = sampleSd(diffs);
  const zeroTol = 1e-12 * Math.max(1, Math.abs(m));
  if (sd < zeroTol) {
    out.sd_L = 0;
    out.se_L = 0;
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

  const tLowerBound = (m + sesoi) / se; // H01: L <= -sesoi
  const tUpperBound = (sesoi - m) / se; // H02: L >= +sesoi
  const tostP1 = 1 - studentTCdf(tLowerBound, df);
  const tostP2 = 1 - studentTCdf(tUpperBound, df);

  out.sd_L = sd;
  out.se_L = se;
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
  return out;
}

function hodgesLehmannPseudian_safe(xs) {
  return hodgesLehmannPseudomedian(xs);
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
    // Holm step-down multiplier across full family m = pValues.length: (m - rank) * p_(rank)
    const rawAdj = (m - rank) * valid[rank].p;
    runningMax = Math.min(1, Math.max(runningMax, rawAdj));
    adjusted[valid[rank].idx] = runningMax;
  }
  return adjusted;
}

export function leeTrimmingBounds(survivedVarA, survivedVarB, eps = 5e-4) {
  if (survivedVarA.length === 0 || survivedVarB.length === 0) {
    return { n_a: survivedVarA.length, n_b: survivedVarB.length, k: 0, lee_lo: null, lee_hi: null, width: null };
  }
  const ya = survivedVarA.map((v) => Math.log(v + eps)).sort((a, b) => a - b);
  const yb = survivedVarB.map((v) => Math.log(v + eps)).sort((a, b) => a - b);
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

export function pairedMediatorOls(birthsA, birthsB, diffsL) {
  if (birthsA.length !== birthsB.length || birthsA.length !== diffsL.length) {
    throw new Error('mediator vectors must share length');
  }
  const pairs = [];
  for (let i = 0; i < birthsA.length; i += 1) {
    if (birthsA[i] > 0 && birthsB[i] > 0 && Number.isFinite(diffsL[i])) {
      pairs.push({ x: Math.log(birthsB[i] / birthsA[i]), y: diffsL[i] });
    }
  }
  const n = pairs.length;
  if (n < 3) {
    return { n, slope: null, intercept: null, pearson_r: null, spearman_rho: null, r2: null, p_slope: null };
  }
  const xs = pairs.map((p) => p.x);
  const ys = pairs.map((p) => p.y);
  const mx = mean(xs);
  const my = mean(ys);
  let sxx = 0;
  let syy = 0;
  let sxy = 0;
  for (let i = 0; i < n; i += 1) {
    const dx = xs[i] - mx;
    const dy = ys[i] - my;
    sxx += dx * dx;
    syy += dy * dy;
    sxy += dx * dy;
  }
  const rx = ranksWithTies(xs).ranks;
  const ry = ranksWithTies(ys).ranks;
  const mrx = mean(rx);
  const mry = mean(ry);
  let srxx = 0;
  let sryy = 0;
  let srxy = 0;
  for (let i = 0; i < n; i += 1) {
    const dx = rx[i] - mrx;
    const dy = ry[i] - mry;
    srxx += dx * dx;
    sryy += dy * dy;
    srxy += dx * dy;
  }
  const spearman = srxx > 0 && sryy > 0 ? srxy / Math.sqrt(srxx * sryy) : null;
  if (sxx <= 0) {
    return { n, slope: null, intercept: my, pearson_r: null, spearman_rho: spearman, r2: null, p_slope: null };
  }
  const slope = sxy / sxx;
  const intercept = my - slope * mx;
  const r = syy > 0 ? sxy / Math.sqrt(sxx * syy) : 0;
  const r2 = r * r;
  const df = n - 2;
  const rss = Math.max(0, syy - slope * sxy);
  const seSlope = Math.sqrt(rss / (df * sxx));
  const pSlope = seSlope > 0 ? studentTTwoSidedP(slope / seSlope, df) : (slope === 0 ? 1 : 0);
  return {
    n,
    slope,
    intercept,
    pearson_r: r,
    spearman_rho: spearman,
    r2,
    p_slope: pSlope,
  };
}

export function classifyE3bDecision({
  wp2AndHostValid,
  failedRunsZero,
  nCompletePairs,
  nRequested,
  minPiLl = 0.85,
  minPairsR7 = 7,
  pHolm,
  fwerAlpha = 0.05,
  meanL,
  tostEquivalent,
  sesoi = Math.log(1.5),
  directionalPositive = true,
}) {
  if (!wp2AndHostValid) {
    return { label: 'insufficient_evidence', direction_tag: 'harness_invalid' };
  }
  const piLl = nRequested > 0 ? nCompletePairs / nRequested : 0;
  if (!failedRunsZero || nCompletePairs < minPairsR7 || piLl < minPiLl || typeof meanL !== 'number' || !Number.isFinite(meanL)) {
    return { label: 'insufficient_evidence', direction_tag: 'attrition_below_pi85_floor' };
  }

  const effBucket =
    Math.abs(meanL) < sesoi
      ? 'lt_sesoi'
      : meanL >= sesoi
        ? 'pos_ge_sesoi'
        : 'neg_ge_sesoi';

  if (effBucket !== 'lt_sesoi' && tostEquivalent) {
    return { label: 'insufficient_evidence', direction_tag: 'ci_inconsistent_impossible' };
  }

  const holmSig = typeof pHolm === 'number' && Number.isFinite(pHolm) && pHolm < fwerAlpha;
  if (holmSig) {
    if (effBucket === 'pos_ge_sesoi') {
      return {
        label: 'supports',
        direction_tag: directionalPositive ? 'predicted_positive_L' : 'non_zero_two_sided_L',
      };
    }
    if (effBucket === 'neg_ge_sesoi') {
      if (directionalPositive) {
        return { label: 'opposite', direction_tag: 'opposite_negative_L' };
      }
      return { label: 'supports', direction_tag: 'non_zero_two_sided_L' };
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

function summarizeRunRules(runMeta, runSeries, finalRec, prereg) {
  const durationTicks = Number(prereg.duration.duration_ticks);
  const warmupTicks = Number(prereg.duration.warmup_ticks);
  const tailRange = prereg.duration.tail_window_ticks.map(Number);

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
  const tailMinAgents = ticks
    .filter((s) => s.tick >= tailRange[0] && s.tick <= tailRange[1])
    .reduce((mn, s) => Math.min(mn, Number(s.obs['live.agent_count'])), Infinity);
  const singletonFinal = !extinct && finalAgents < 2;
  const singletonTail = !extinct && (singletonFinal || tailMinAgents < 2);

  const westMean = Number(finalObs['live.trait_mass_mean_west']);
  const eastMean = Number(finalObs['live.trait_mass_mean_east']);
  let emptyHalf = null;
  if (westMean === 0 && eastMean === 0) emptyHalf = 'both';
  else if (westMean === 0) emptyHalf = 'west';
  else if (eastMean === 0) emptyHalf = 'east';

  // F-053: ALWAYS read live.mean_lifespan from series.csv (never final_observables which drained).
  const finalTickSeries = ticks.find((s) => s.tick === durationTicks)?.obs ?? null;
  const lifespanAtT = finalTickSeries ? Number(finalTickSeries['live.mean_lifespan']) : null;
  const postWarmupPositiveDeaths = ticks
    .filter((s) => s.tick > warmupTicks && Number(s.obs['live.deaths']) > 0)
    .map((s) => Number(s.obs['live.mean_lifespan']));
  const lifespanPostWarmupPos = mean(postWarmupPositiveDeaths);

  const birthsSum = ticks.reduce((acc, s) => acc + Number(s.obs['live.births']), 0);
  const deathsSum = ticks.reduce((acc, s) => acc + Number(s.obs['live.deaths']), 0);
  const postWarmupCrop = mean(
    ticks.filter((s) => s.tick > warmupTicks).map((s) => Number(s.obs['live.standing_crop'])),
  );
  const tailVars = ticks
    .filter((s) => s.tick >= tailRange[0] && s.tick <= tailRange[1])
    .map((s) => Number(s.obs['live.trait_mass_var']));
  const tailVarMean = tailVars.length > 0 ? mean(tailVars) : null;

  return {
    seed: Number(runMeta.seed),
    arm: runMeta.arm,
    completed: runMeta.status === 'Completed',
    extinct,
    firstExtinctTick,
    survivalTicks,
    singletonFinal,
    singletonTail,
    validFinalVar: !extinct && !singletonFinal && runMeta.status === 'Completed',
    validTailVar: !extinct && !singletonTail && runMeta.status === 'Completed',
    emptyHalf,
    varFinal: Number(finalObs['live.trait_mass_var']),
    varBesselFinal: finalAgents >= 2
      ? (Number(finalObs['live.trait_mass_var']) * finalAgents) / (finalAgents - 1)
      : null,
    meanMassFinal: Number(finalObs['live.trait_mass_mean']),
    iqrMassFinal: Number(finalObs['live.trait_mass_p90']) - Number(finalObs['live.trait_mass_p10']),
    s3RegionalDiff: eastMean - westMean,
    tailVarMean,
    finalAgents,
    finalPredators: Number(finalObs['live.predator_count']),
    closedEuTotal: Number(finalObs['live.closed_eu_total']),
    standingCropFinal: Number(finalObs['live.standing_crop']),
    postWarmupCrop,
    birthsSum,
    deathsSum,
    lifespanAtT,
    lifespanPostWarmupPos,
    cumulativeMeanLifespan: Number(finalObs['live.cumulative_mean_lifespan'] ?? 0),
  };
}

// ---- Full E3b Dataset Evaluation (Independent Recomputation) ----------------------------------

export function evaluateE3bDataset({ runs, series, finalObservables, prereg, wp2AndHostValid = true }) {
  const executionOrder = prereg.seeds.execution_order.map(Number);
  const nRequested = executionOrder.length;
  const eps = Number(prereg.thresholds.eps_primary);
  const epsSens = Number(prereg.thresholds.eps_sensitivity);
  const sesoi = Number(prereg.thresholds.sesoi_log_var_ratio);
  const fwerAlpha = Number(prereg.thresholds.fwer_alpha);
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

  // Evaluate raw L_i across H1, H2, H3 first so Holm can adjust m=3 together.
  const rawContrastStats = CONTRAST_DEFS.map((c) => {
    const varA = [];
    const varB = [];
    const birthsA = [];
    const birthsB = [];
    const survAllA = [];
    const survAllB = [];
    const survValidVarA = [];
    const survValidVarB = [];
    const s1A = [];
    const s1B = [];
    const s2A = [];
    const s2B = [];
    const s3A = [];
    const s3B = [];
    const tailVarA = [];
    const tailVarB = [];

    for (const seed of executionOrder) {
      const sa = summaries.get(`${seed}::${c.controlArm}`);
      const sb = summaries.get(`${seed}::${c.treatmentArm}`);
      if (!sa || !sb) continue;

      survAllA.push(sa.survivalTicks);
      survAllB.push(sb.survivalTicks);
      if (sa.validFinalVar) survValidVarA.push(sa.varFinal);
      if (sb.validFinalVar) survValidVarB.push(sb.varFinal);

      if (sa.validFinalVar && sb.validFinalVar) {
        varA.push(sa.varFinal);
        varB.push(sb.varFinal);
        birthsA.push(sa.birthsSum);
        birthsB.push(sb.birthsSum);
        s1A.push(sa.iqrMassFinal);
        s1B.push(sb.iqrMassFinal);
        s2A.push(sa.meanMassFinal);
        s2B.push(sb.meanMassFinal);
        if (!sa.emptyHalf && !sb.emptyHalf) {
          s3A.push(sa.s3RegionalDiff);
          s3B.push(sb.s3RegionalDiff);
        }
      }
      if (sa.validTailVar && sb.validTailVar && sa.tailVarMean !== null && sb.tailVarMean !== null) {
        tailVarA.push(sa.tailVarMean);
        tailVarB.push(sb.tailVarMean);
      }
    }

    const primary = evaluatePairedLogVarianceRatio(varA, varB, {
      eps,
      sesoi,
      minNonZeroR7: minPairsR7,
      nRequested,
    });
    const sensitivityEps5e5 = evaluatePairedLogVarianceRatio(varA, varB, {
      eps: epsSens,
      sesoi,
      minNonZeroR7: minPairsR7,
      nRequested,
    });
    const tailWindow = evaluatePairedLogVarianceRatio(tailVarA, tailVarB, {
      eps,
      sesoi,
      minNonZeroR7: minPairsR7,
      nRequested,
    });
    const leeBounds = leeTrimmingBounds(survValidVarA, survValidVarB, eps);
    const rmst = {
      ...pairedStats(survAllA, survAllB, nRequested),
      wilcoxon: wilcoxonSignedRank(survAllA.map((a, i) => survAllB[i] - a), minPairsR7),
    };
    const mediator = pairedMediatorOls(birthsA, birthsB, primary.diffs);

    return {
      def: c,
      primary,
      sensitivityEps5e5,
      tailWindow,
      leeBounds,
      rmst,
      mediator,
      s1: pairedStats(s1A, s1B, nRequested),
      s2: pairedStats(s2A, s2B, nRequested),
      s3: pairedStats(s3A, s3B, nRequested),
    };
  });

  const holmP = holmAdjust(rawContrastStats.map((entry) => entry.primary.p_two_sided));
  const contrasts = {};
  for (let i = 0; i < rawContrastStats.length; i += 1) {
    const entry = rawContrastStats[i];
    const pHolm = holmP[i];
    const r10 = classifyE3bDecision({
      wp2AndHostValid,
      failedRunsZero,
      nCompletePairs: entry.primary.n_complete_pairs,
      nRequested,
      minPiLl,
      minPairsR7,
      pHolm,
      fwerAlpha,
      meanL: entry.primary.mean_L,
      tostEquivalent: entry.primary.tost_equivalent,
      sesoi,
      directionalPositive: entry.def.directionalPositive,
    });
    contrasts[entry.def.id] = {
      id: entry.def.id,
      name: entry.def.name,
      p_holm: pHolm,
      decision: r10.label,
      direction_tag: r10.direction_tag,
      primary: entry.primary,
      sensitivity_eps_5e5: entry.sensitivityEps5e5,
      tail_window_L: entry.tailWindow,
      lee_bounds: entry.leeBounds,
      rmst: entry.rmst,
      mediator_ln_births: entry.mediator,
      s1_iqr: entry.s1,
      s2_mean: entry.s2,
      s3_east_minus_west: entry.s3,
    };
  }

  return {
    primary_decision: contrasts.H1.decision,
    primary_direction_tag: contrasts.H1.direction_tag,
    failed_runs_count: failedRunsCount,
    contrasts,
  };
}

// ---- Provenance, Checksum & Smoke Verification ------------------------------------------------

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

function assertDirContainsNoTraitMass(dir, fail, label) {
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
      prereg.manifests.control,
      prereg.manifests.control_uniform_scarcity,
      prereg.manifests.treatment,
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

  // Blind smoke dir check if referenced on disk.
  const sr = prov.smoke_reference;
  if (sr?.smoke_dir) {
    const smokeDir = isAbsolute(sr.smoke_dir) ? sr.smoke_dir : join(repoRoot, sr.smoke_dir);
    if (!existsSync(smokeDir)) {
      fail(`referenced smoke_dir does not exist: ${sr.smoke_dir}`);
    } else {
      if (sr.wp2_passed !== true) fail('smoke_reference.wp2_passed is not true');
      verifyChecksumManifest(smokeDir, ['provenance.json', 'smoke-report.json'], fail, 'smoke_reference');
      assertDirContainsNoTraitMass(smokeDir, fail, 'smoke_reference');
    }
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

  // F-053 guard: if S6.mean_lifespan is reported, verify it matches series.csv and NOT drained final_observables.
  if (effects.s6_lifespan_source && effects.s6_lifespan_source !== 'series.csv') {
    fail(`F-053 violation: s6_lifespan_source must be 'series.csv', got ${effects.s6_lifespan_source}`);
  }

  const recomputed = evaluateE3bDataset({ runs, series, finalObservables: finals, prereg });
  if (effects.primary_decision !== recomputed.primary_decision) {
    fail(`primary_decision mismatch: expected '${recomputed.primary_decision}', got '${effects.primary_decision}'`);
  }
  if (effects.primary_direction_tag !== recomputed.primary_direction_tag) {
    fail(`primary_direction_tag mismatch: expected '${recomputed.primary_direction_tag}', got '${effects.primary_direction_tag}'`);
  }

  for (const cid of ['H1', 'H2', 'H3']) {
    const obs = effects.contrasts?.[cid];
    const exp = recomputed.contrasts[cid];
    if (!obs) {
      fail(`effects.json missing contrast '${cid}'`);
      continue;
    }
    if (obs.decision !== exp.decision) fail(`${cid}.decision expected '${exp.decision}', got '${obs.decision}'`);
    if (!approxEq(obs.p_holm, exp.p_holm)) fail(`${cid}.p_holm expected ${exp.p_holm}, got ${obs.p_holm}`);
    for (const k of ['n_complete_pairs', 'mean_L', 'sd_L', 'se_L', 'p_two_sided', 'ci95_low', 'ci95_high', 'ci90_tost_low', 'ci90_tost_high', 'paired_dz']) {
      if (!approxEq(obs.primary?.[k], exp.primary?.[k])) {
        fail(`${cid}.primary.${k} expected ${exp.primary?.[k]}, got ${obs.primary?.[k]}`);
      }
    }
    if (Boolean(obs.primary?.tost_equivalent) !== Boolean(exp.primary?.tost_equivalent)) {
      fail(`${cid}.primary.tost_equivalent mismatch`);
    }
    if (!approxEq(obs.sensitivity_eps_5e5?.mean_L, exp.sensitivity_eps_5e5?.mean_L)) {
      fail(`${cid}.sensitivity_eps_5e5.mean_L mismatch`);
    }
    if (!approxEq(obs.lee_bounds?.lee_lo, exp.lee_bounds?.lee_lo) || !approxEq(obs.lee_bounds?.lee_hi, exp.lee_bounds?.lee_hi)) {
      fail(`${cid}.lee_bounds mismatch`);
    }
    if (!approxEq(obs.rmst?.paired_mean_delta, exp.rmst?.paired_mean_delta)) {
      fail(`${cid}.rmst.paired_mean_delta mismatch`);
    }
    if (!approxEq(obs.mediator_ln_births?.slope, exp.mediator_ln_births?.slope)) {
      fail(`${cid}.mediator_ln_births.slope mismatch`);
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

function buildSyntheticE3bBundle(rootDir, prereg) {
  const confirmDir = join(rootDir, 'confirmation');
  const smokeDir = join(rootDir, 'smoke');
  mkdirSync(join(confirmDir, 'manifests'), { recursive: true });
  mkdirSync(join(smokeDir, 'manifests'), { recursive: true });

  const wid = { seed: 1337, generator_version: 2, width: 256, height: 256, checksum: 3152323152 };
  const seeds = prereg.seeds.execution_order.map(Number);

  // Blind smoke dir with zero trait_mass substrings anywhere.
  const smokeReport = { seed: 999953, body_mass_consulted: false, wp2_passed: true };
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

  const runsRows = ['seed,arm,status,terrain_source,world_seed,world_gen_version,world_width,world_height,world_checksum'];
  const seriesRows = ['seed,arm,tick,live.agent_count,live.deaths,live.births,live.mean_lifespan,live.standing_crop,live.trait_mass_var'];
  const finals = [];

  for (let i = 0; i < seeds.length; i += 1) {
    const seed = seeds[i];
    const jitter = (i % 5) * 0.0005;
    const armVars = {
      control_uniform: 0.010 + jitter,
      control_uniform_scarcity: 0.012 + jitter,
      treatment_gradient: 0.030 + jitter * 1.4,
    };
    const armBirths = {
      control_uniform: 18,
      control_uniform_scarcity: 12,
      treatment_gradient: 18,
    };
    for (const arm of ARM_ORDER) {
      runsRows.push(`${seed},${arm},Completed,generated,${wid.seed},${wid.generator_version},${wid.width},${wid.height},${wid.checksum}`);
      for (let tick = 600; tick <= 18000; tick += 600) {
        const crop = arm === 'control_uniform' ? 122000 : 63800;
        seriesRows.push(`${seed},${arm},${tick},150,2,${armBirths[arm]},4200,${crop},${armVars[arm]}`);
      }
      finals.push({
        seed,
        arm,
        terrain_source: 'generated',
        world_identity: wid,
        observables: {
          'live.agent_count': 150,
          'live.predator_count': arm === 'control_uniform_scarcity' ? 3 : 7,
          'live.closed_eu_total': 153587.25,
          'live.standing_crop': arm === 'control_uniform' ? 122000 : 63800,
          'live.trait_mass_var': armVars[arm],
          'live.trait_mass_mean': 2.55,
          'live.trait_mass_p10': 2.1,
          'live.trait_mass_p90': arm === 'treatment_gradient' ? 3.2 : 2.8,
          'live.trait_mass_mean_west': arm === 'treatment_gradient' ? 2.3 : 2.55,
          'live.trait_mass_mean_east': arm === 'treatment_gradient' ? 2.75 : 2.56,
          'live.mean_lifespan': 0.0, // drained sentinel per F-053
          'live.cumulative_mean_lifespan': 4150,
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
  const computed = evaluateE3bDataset({ runs: runsParsed, series: seriesParsed, finalObservables: finals, prereg });
  const effectsObj = { ...computed, s6_lifespan_source: 'series.csv' };
  writeFileSync(join(confirmDir, 'effects.json'), JSON.stringify(effectsObj, null, 2));

  const provObj = {
    stopped_early: false,
    build: { profile: 'release', git_commit: 'deadbeef', git_dirty_paths: [] },
    host: { os: 'windows', arch: 'x86_64' },
    terrain_source: 'generated',
    world_identity: wid,
    world_artifact_prerun_check: { anima_world_artifact_unset: true, temp_artifact_absent: true },
    smoke_reference: { wp2_passed: true, smoke_dir: smokeDir },
  };
  writeFileSync(join(confirmDir, 'provenance.json'), JSON.stringify(provObj, null, 2));

  for (const mf of [
    PREREG_FILE,
    prereg.manifests.control,
    prereg.manifests.control_uniform_scarcity,
    prereg.manifests.treatment,
  ]) {
    writeFileSync(join(confirmDir, 'manifests', mf), readFileSync(join(repoRoot, committedManifestDir, mf)));
  }

  rehashConfirmBundle(confirmDir);
  return { confirmDir, smokeDir, wid };
}

function rehashConfirmBundle(confirmDir) {
  const files = ['provenance.json', 'runs.csv', 'series.csv', 'final-observables.json', 'effects.json'];
  const lines = files.map((f) => `${sha256(readFileSync(join(confirmDir, f)))}  ${f}`);
  writeFileSync(join(confirmDir, 'checksums.sha256'), lines.join('\n') + '\n');
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

  // 1. Exact Student-t & R7 quantiles.
  const t63 = studentTInv(0.975, 63);
  expect(Math.abs(t63 - 1.99834054) < 1e-6, `studentTInv(0.975, 63) ~ 1.998341 (got ${t63.toFixed(6)})`);
  const t11 = studentTInv(0.975, 11);
  expect(Math.abs(t11 - 2.20098516) < 1e-6, `studentTInv(0.975, 11) ~ 2.200985`);

  // 2. Kerby r_c, HL + BCa, and additive L_i identity.
  const lr = evaluatePairedLogVarianceRatio([0.01, 0.015, 0.02], [0.03, 0.04, 0.05]);
  expect(lr.n_complete_pairs === 3 && lr.mean_L > 0, 'evaluatePairedLogVarianceRatio positive ratio');
  const bca = hodgesLehmannBcaCi([0.2, 0.4, 0.5, 0.6, 0.8, 0.9, 1.0, 1.1], { nBoot: 1000 });
  expect(bca.ci_low < bca.estimate && bca.estimate < bca.ci_high, 'hodgesLehmannBcaCi bracket');

  // 3. Lee bounds: balanced -> width 0; unbalanced -> nonzero interval bracketing effect.
  const leeBal = leeTrimmingBounds([0.01, 0.02, 0.03], [0.02, 0.04, 0.06], 5e-4);
  expect(Math.abs(leeBal.width) < 1e-12, 'Lee bounds width === 0 when nA == nB');
  const leeUnbal = leeTrimmingBounds([0.01, 0.02], [0.02, 0.04, 0.06], 5e-4);
  expect(leeUnbal.k === 2 && leeUnbal.lee_hi > leeUnbal.lee_lo, 'Lee bounds asymmetric trimming width > 0');

  // 4. R10 48/48 discrete partition (directional H1 vs two-sided H2/H3).
  const h1Counts = { supports: 0, opposite: 0, equivalent_to_zero: 0, insufficient_evidence: 0 };
  for (const wp2 of [true, false]) {
    for (const strat of [true, false]) {
      for (const holm of [true, false]) {
        for (const meanL of [0.5, -0.5, 0.1]) {
          for (const tost of [true, false]) {
            const res = classifyE3bDecision({
              wp2AndHostValid: wp2,
              failedRunsZero: strat,
              nCompletePairs: strat ? 64 : 50,
              nRequested: 64,
              pHolm: holm ? 0.01 : 0.20,
              meanL,
              tostEquivalent: tost,
              directionalPositive: true,
            });
            h1Counts[res.label] += 1;
          }
        }
      }
    }
  }
  expect(
    h1Counts.supports === 1 &&
      h1Counts.opposite === 1 &&
      h1Counts.equivalent_to_zero === 2 &&
      h1Counts.insufficient_evidence === 44,
    'R10 exhaustive 48/48 discrete partition (1/1/2/44)',
  );

  // 5. Full synthetic N=64 bundle pass + negative tamper controls.
  const prereg = loadCommittedPrereg(null, 'HEAD');
  const workDir = join(tmpdir(), `anima-e3b-selftest-${process.pid}-${Date.now()}`);
  try {
    const { confirmDir, smokeDir } = buildSyntheticE3bBundle(workDir, prereg);
    const clean = verifyArtifactDir(confirmDir, { preregOverride: prereg, skipCommittedManifestCheck: true });
    expect(clean.ok, `clean synthetic N=64 bundle passes (${clean.failures.join('; ')})`);

    // Tamper A: trait_mass leaked into smoke directory.
    writeFileSync(join(smokeDir, 'leak.txt'), 'live.trait_mass_var = 0.01\n');
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

    // Tamper C: corrupted checksum without updating checksums.sha256.
    const runsPath = join(confirmDir, 'runs.csv');
    const origRuns = readFileSync(runsPath, 'utf8');
    writeFileSync(runsPath, origRuns + '\n');
    const badCsum = verifyArtifactDir(confirmDir, { preregOverride: prereg, skipCommittedManifestCheck: true });
    expect(!badCsum.ok && badCsum.failures.some((f) => f.includes('SHA-256 mismatch')), 'negative control: checksum mismatch fails');
  } finally {
    rmSync(workDir, { recursive: true, force: true });
  }

  if (bad > 0) {
    console.error(`verify_e3b_artifacts --self-test FAILED (${bad} failure(s))`);
    process.exit(1);
  }
  console.log('verify_e3b_artifacts --self-test PASSED');
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
    console.log(`check:e3b — no confirmation artifacts at ${DEFAULT_CONFIRM_DIR} yet (pre-E3B-run ok)`);
    return;
  }

  const res = verifyArtifactDir(targetDir, { preregPath, skipCommittedManifestCheck });
  for (const n of res.notes) console.warn(`verify_e3b_artifacts NOTE: ${n}`);
  if (!res.ok) {
    console.error(`verify_e3b_artifacts FAILED (${res.failures.length} failure(s)):`);
    for (const f of res.failures) console.error(`  - ${f}`);
    process.exit(1);
  }
  console.log(`verify_e3b_artifacts OK (${targetDir})`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main();
}
