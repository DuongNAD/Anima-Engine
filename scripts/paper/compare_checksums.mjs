#!/usr/bin/env node
// Card M1: build the cross-platform reproducibility matrix from the probe's output.
//
// Usage:
//   node scripts/paper/compare_checksums.mjs <probe-output>... [--markdown out.md]
//   node scripts/paper/compare_checksums.mjs --self-test
//
// Each input is the captured output of
//   ANIMA_M1_FULL=1 cargo test --release --test cross_platform_checksum_probe -- --nocapture
// on one platform. Only lines starting with `M1_META ` and `M1 ` are read; everything else is ignored.
//
// Lines starting with `M1_LIBM ` (the result bits of each math function for a fixed list of inputs)
// add a second table: per function, how many inputs give different bits on different platforms, and
// the largest distance in units in the last place (ULP).
//
// The matrix is a measurement, not a gate: the script exits 0 whether the platforms agree or not,
// and names the first (tick, model, seed) at which they differ. It exits 2 when an input is
// malformed or the inputs do not cover the same points, because then there is nothing to compare.

import { readFileSync, writeFileSync } from 'node:fs';

export function parseProbe(text, source) {
  let meta = null;
  const rows = new Map();
  const libm = new Map();
  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith('M1_LIBM ')) {
      const entry = JSON.parse(line.slice('M1_LIBM '.length));
      if (typeof entry.fn !== 'string' || !Array.isArray(entry.bits)) throw new Error(`${source}: a malformed M1_LIBM line`);
      if (libm.has(entry.fn)) throw new Error(`${source}: duplicate M1_LIBM line for ${entry.fn}`);
      libm.set(entry.fn, entry.bits);
    } else if (line.startsWith('M1_META ')) {
      if (meta) throw new Error(`${source}: more than one M1_META line`);
      meta = JSON.parse(line.slice('M1_META '.length));
    } else if (line.startsWith('M1 ')) {
      const row = JSON.parse(line.slice('M1 '.length));
      for (const field of ['model', 'seed', 'tick', 'checksum', 'observables']) {
        if (!(field in row)) throw new Error(`${source}: a row has no "${field}"`);
      }
      const key = `${row.model}|${row.seed}|${row.tick}`;
      if (rows.has(key)) throw new Error(`${source}: duplicate row ${key}`);
      rows.set(key, row);
    }
  }
  if (!meta) throw new Error(`${source}: no M1_META line (was the probe run with --nocapture?)`);
  if (rows.size === 0) throw new Error(`${source}: no M1 rows`);
  return { meta, rows, libm, label: `${meta.os}-${meta.arch}`, source };
}

const orderKey = (row) => [row.tick, row.model, row.seed];
function compareOrder(a, b) {
  const ka = orderKey(a);
  const kb = orderKey(b);
  for (let i = 0; i < ka.length; i += 1) {
    if (ka[i] < kb[i]) return -1;
    if (ka[i] > kb[i]) return 1;
  }
  return 0;
}

export function buildMatrix(probes) {
  if (probes.length < 2) throw new Error('need at least two probe outputs to compare');
  const labels = probes.map((p) => p.label);
  if (new Set(labels).size !== labels.length) throw new Error(`duplicate platform labels: ${labels.join(', ')}`);
  const modes = new Set(probes.map((p) => p.meta.mode));
  if (modes.size !== 1) throw new Error(`inputs were run in different modes: ${[...modes].join(', ')}`);
  const keys = [...probes[0].rows.keys()].sort();
  for (const p of probes.slice(1)) {
    const other = [...p.rows.keys()].sort();
    if (other.join('\n') !== keys.join('\n')) {
      throw new Error(`${p.label} does not cover the same (model, seed, tick) points as ${probes[0].label}`);
    }
  }
  const points = keys
    .map((key) => {
      const rows = probes.map((p) => p.rows.get(key));
      const checksums = rows.map((r) => r.checksum);
      const checksumAgrees = checksums.every((c) => c === checksums[0]);
      const names = rows[0].observables.map(([name]) => name);
      for (const r of rows.slice(1)) {
        const n = r.observables.map(([name]) => name);
        if (n.join('|') !== names.join('|')) {
          throw new Error(`observable names differ between platforms at ${key}`);
        }
      }
      const differingObservables = names.filter((name, i) => {
        const values = rows.map((r) => r.observables[i][1]);
        return values.some((v) => v !== values[0]);
      });
      const { model, seed, tick } = rows[0];
      return { key, model, seed, tick, checksums, checksumAgrees, differingObservables };
    })
    .sort(compareOrder);
  const firstDifference = points.find((p) => !p.checksumAgrees || p.differingObservables.length > 0) ?? null;
  return { labels, mode: probes[0].meta.mode, points, firstDifference };
}

// Map an IEEE-754 bit pattern to an integer whose order matches the float order, so the difference
// of two of them is their distance in ULP.
function orderedBits(hex) {
  const width = BigInt(hex.length * 4);
  const v = BigInt(`0x${hex}`);
  const sign = 1n << (width - 1n);
  return v & sign ? -(v & (sign - 1n)) : v;
}

export function buildLibmTable(probes) {
  if (!probes.every((p) => p.libm.size > 0)) return null;
  const names = [...probes[0].libm.keys()];
  for (const p of probes.slice(1)) {
    if ([...p.libm.keys()].join('|') !== names.join('|')) throw new Error(`${p.label} prints different M1_LIBM functions`);
  }
  return names.map((fn) => {
    const columns = probes.map((p) => p.libm.get(fn));
    const n = columns[0].length;
    if (columns.some((c) => c.length !== n)) throw new Error(`${fn}: platforms printed different input counts`);
    let differing = 0;
    let maxUlp = 0n;
    for (let i = 0; i < n; i += 1) {
      const values = columns.map((c) => c[i]);
      if (values.some((v) => v !== values[0])) differing += 1;
      for (let a = 0; a < values.length; a += 1) {
        for (let b = a + 1; b < values.length; b += 1) {
          const d = orderedBits(values[a]) - orderedBits(values[b]);
          const ulp = d < 0n ? -d : d;
          if (ulp > maxUlp) maxUlp = ulp;
        }
      }
    }
    return { fn, inputs: n, differing, maxUlp: maxUlp.toString() };
  });
}

export function renderLibmMarkdown(table, labels) {
  const out = [];
  out.push(`Platforms: ${labels.map((l) => `\`${l}\``).join(', ')}`);
  out.push('');
  out.push('| Function | Inputs | Inputs whose bits differ between platforms | Largest difference (ULP) |');
  out.push('|---|---:|---:|---:|');
  for (const r of table) out.push(`| \`${r.fn}\` | ${r.inputs} | ${r.differing} | ${r.maxUlp} |`);
  return out.join('\n') + '\n';
}

export function renderMarkdown(matrix) {
  const { labels, mode, points, firstDifference } = matrix;
  const agreeing = points.filter((p) => p.checksumAgrees && p.differingObservables.length === 0).length;
  const out = [];
  out.push(`Platforms: ${labels.map((l) => `\`${l}\``).join(', ')} · mode \`${mode}\` · ${points.length} points`);
  out.push('');
  if (firstDifference === null) {
    out.push(`**All ${points.length} points agree on every platform**: same checksum and the same printed value for every observable.`);
  } else {
    const f = firstDifference;
    out.push(`**${agreeing} of ${points.length} points agree.** First difference (ordered by tick, then model, then seed): \`${f.model}\` seed ${f.seed} at tick ${f.tick}` +
      (f.checksumAgrees ? ', with equal checksums' : ', checksums differ') +
      (f.differingObservables.length ? `; observables that differ: ${f.differingObservables.map((n) => `\`${n}\``).join(', ')}` : '') + '.');
  }
  out.push('');
  out.push(`| Model | Seed | Tick | ${labels.map((l) => `Checksum \`${l}\``).join(' | ')} | Agree | Observables that differ |`);
  out.push(`|---|---|---|${labels.map(() => '---').join('|')}|---|---|`);
  for (const p of points) {
    const agree = p.checksumAgrees && p.differingObservables.length === 0 ? 'yes' : '**no**';
    const diff = p.differingObservables.length ? p.differingObservables.map((n) => `\`${n}\``).join(', ') : '—';
    out.push(`| \`${p.model}\` | ${p.seed} | ${p.tick} | ${p.checksums.join(' | ')} | ${agree} | ${diff} |`);
  }
  return out.join('\n') + '\n';
}

function selfTest() {
  // Negative controls: the script must notice a checksum difference, an observable-only difference,
  // and refuse inputs that cannot be compared.
  const meta = (os) => `M1_META ${JSON.stringify({ os, arch: 'x86_64', mode: 'full' })}`;
  const row = (model, tick, checksum, value) =>
    `M1 ${JSON.stringify({ model, seed: 1, tick, checksum, observables: [['plants', value]] })}`;
  const a = parseProbe([meta('linux'), row('live.p1', 180, 7, '1.0'), row('live.p1', 1800, 9, '2.0')].join('\n'), 'a');
  const same = parseProbe([meta('macos'), row('live.p1', 180, 7, '1.0'), row('live.p1', 1800, 9, '2.0')].join('\n'), 'b');
  const checksumDiff = parseProbe([meta('windows'), row('live.p1', 180, 7, '1.0'), row('live.p1', 1800, 8, '2.0')].join('\n'), 'c');
  const valueDiff = parseProbe([meta('windows'), row('live.p1', 180, 7, '1.5'), row('live.p1', 1800, 9, '2.0')].join('\n'), 'd');
  const missing = parseProbe([meta('windows'), row('live.p1', 180, 7, '1.0')].join('\n'), 'e');
  const checks = [];
  const check = (name, ok) => checks.push([name, ok]);
  check('identical inputs agree', buildMatrix([a, same]).firstDifference === null);
  const cd = buildMatrix([a, checksumDiff]).firstDifference;
  check('a checksum difference is named at its tick', cd?.tick === 1800 && cd.checksumAgrees === false);
  const vd = buildMatrix([a, valueDiff]).firstDifference;
  check('an observable-only difference is named', vd?.tick === 180 && vd.checksumAgrees && vd.differingObservables[0] === 'plants');
  const libmLine = (fn, bits) => `M1_LIBM ${JSON.stringify({ fn, bits })}`;
  const la = parseProbe([meta('linux'), row('live.p1', 180, 7, '1.0'), libmLine('f32::sin', ['3f800000', '3f800001'])].join('\n'), 'la');
  const lb = parseProbe([meta('macos'), row('live.p1', 180, 7, '1.0'), libmLine('f32::sin', ['3f800000', '3f800003'])].join('\n'), 'lb');
  const lt = buildLibmTable([la, lb]);
  check('a libm difference is counted with its ULP distance', lt?.[0].differing === 1 && lt[0].maxUlp === '2');
  let refused = false;
  try { buildMatrix([a, missing]); } catch { refused = true; }
  check('inputs covering different points are refused', refused);
  let noMeta = false;
  try { parseProbe(row('live.p1', 180, 7, '1.0'), 'f'); } catch { noMeta = true; }
  check('an input without M1_META is refused', noMeta);
  for (const [name, ok] of checks) console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}`);
  const failed = checks.filter(([, ok]) => !ok).length;
  if (failed) {
    console.error(`self-test: ${failed} control(s) failed`);
    process.exit(2);
  }
  console.log(`self-test: ${checks.length} controls ok`);
}

function main(argv) {
  if (argv.includes('--self-test')) return selfTest();
  const inputs = [...argv];
  let mdPath = null;
  const mdIndex = inputs.indexOf('--markdown');
  if (mdIndex >= 0) {
    mdPath = inputs[mdIndex + 1];
    inputs.splice(mdIndex, 2);
  }
  if (inputs.length < 2 || (mdIndex >= 0 && !mdPath)) {
    console.error('usage: compare_checksums.mjs <probe-output> <probe-output>... [--markdown out.md] | --self-test');
    process.exit(2);
  }
  let markdown;
  try {
    const probes = inputs.map((f) => parseProbe(readFileSync(f, 'utf8'), f));
    const matrix = buildMatrix(probes);
    markdown = renderMarkdown(matrix);
    const libm = buildLibmTable(probes);
    if (libm) markdown += `\n### Math functions\n\n${renderLibmMarkdown(libm, matrix.labels)}`;
  } catch (err) {
    console.error(`compare_checksums: ${err.message}`);
    process.exit(2);
  }
  process.stdout.write(markdown);
  if (mdPath) writeFileSync(mdPath, markdown);
}

main(process.argv.slice(2));
