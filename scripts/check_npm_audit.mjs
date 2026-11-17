// The npm advisory gate: `npm audit` for both npm packages, with a reasoned exemption list.
//
// # Why this replaces a bare `npm audit --audit-level=high`
//
// The bare command has no way to say "this advisory is known, here is why it cannot reach a user".
// On 2026-11-18 an advisory landed against every published version of `picomatch`, a transitive
// dependency of vite, vitest and typescript-eslint. There was no version to upgrade to, so the gate
// could only be red: every pull request failed for a reason no pull request could fix. The choices
// were to wait with the trunk frozen, to drop development dependencies from the gate altogether
// (`--omit=dev`, which would stop looking at the build tooling for good), or to do what
// `src-tauri/.cargo/audit.toml` already does for Rust: name the advisory, record the check that shows
// it is unreachable, and re-run that check on every build.
//
// # What it enforces
//
// For each package (the repository root and `tests/`):
//
//  - every advisory of severity high or critical must be listed in `scripts/npm_audit_policy.mjs`;
//  - a listed advisory must be absent from `npm audit --omit=dev`, the audit of what ships;
//  - a listed advisory that appears in no package at all is stale, and fails.
//
// Low and moderate advisories are reported in the summary line and do not fail, exactly as with
// `--audit-level=high`: a gate that fires on every transitive moderate gets muted.
//
// # Negative controls
//
// Before it touches the network the script runs five synthetic cases through the same decision
// function and exits 2 if any of them comes out the wrong way round. A gate that cannot fail is the
// defect this project has shipped most often.
//
// Usage: node scripts/check_npm_audit.mjs
// Exit:  0 clean, 1 policy violation or audit could not be read, 2 a negative control misbehaved.

import { spawnSync } from 'node:child_process';
import { argv, exit, platform } from 'node:process';
import { pathToFileURL } from 'node:url';
import { DEV_ONLY_EXEMPTIONS } from './npm_audit_policy.mjs';

const PACKAGES = [
  { label: 'root', prefix: null },
  { label: 'tests', prefix: 'tests' },
];

const GATED = new Set(['high', 'critical']);

/** The advisories in an `npm audit --json` report (auditReportVersion 2), one per GHSA id. */
export function advisoriesOf(report) {
  const found = new Map();
  for (const vuln of Object.values(report.vulnerabilities ?? {})) {
    for (const via of vuln.via ?? []) {
      // A string `via` names another vulnerable package; the advisory itself is the object form.
      if (typeof via !== 'object' || via === null) continue;
      const id = String(via.url ?? '').split('/').pop() || `source-${via.source}`;
      if (!found.has(id)) {
        found.set(id, {
          id,
          package: via.name,
          severity: via.severity,
          title: via.title,
          range: via.range,
        });
      }
    }
  }
  return [...found.values()];
}

/**
 * Decide one run. `audits` is `[{ label, full, prod }]`, each an `npm audit --json` report with and
 * without development dependencies. Returns the list of violations; empty means the gate passes.
 */
export function evaluate(audits, exemptions) {
  const violations = [];
  const exempt = new Map(exemptions.map((entry) => [entry.advisory, entry]));
  const seen = new Set();

  for (const entry of exemptions) {
    if (!entry.advisory || !String(entry.reason ?? '').trim()) {
      violations.push(`policy entry ${entry.advisory || '(no id)'} has no reason`);
    }
  }

  for (const { label, full, prod } of audits) {
    const shipped = new Set(advisoriesOf(prod).map((advisory) => advisory.id));
    for (const advisory of advisoriesOf(full)) {
      if (!GATED.has(advisory.severity)) continue;
      const where = `${label}: ${advisory.id} (${advisory.package} ${advisory.range}, ${advisory.severity})`;
      if (!exempt.has(advisory.id)) {
        violations.push(`${where} is not in scripts/npm_audit_policy.mjs: ${advisory.title}`);
        continue;
      }
      seen.add(advisory.id);
      if (shipped.has(advisory.id)) {
        violations.push(
          `${where} is exempted as development-only but appears in \`npm audit --omit=dev\`: ` +
            'it now reaches a shipped dependency, so the exemption no longer holds',
        );
      }
    }
  }

  for (const entry of exemptions) {
    if (entry.advisory && !seen.has(entry.advisory)) {
      violations.push(
        `policy entry ${entry.advisory} (${entry.package}) matches no advisory in any package: ` +
          'it is stale, remove it from scripts/npm_audit_policy.mjs',
      );
    }
  }
  return violations;
}

function report(vulnerabilities) {
  return { auditReportVersion: 2, vulnerabilities };
}

function vulnerable(name, id, severity) {
  return {
    [name]: {
      name,
      severity,
      via: [
        {
          source: 1,
          name,
          title: `synthetic advisory against ${name}`,
          url: `https://github.com/advisories/${id}`,
          severity,
          range: '*',
        },
      ],
    },
  };
}

/** Run the decision function on cases whose answer is known. Returns the controls that misbehaved. */
export function selfTest() {
  const clean = report({});
  const high = report(vulnerable('left-pad', 'GHSA-aaaa-aaaa-aaaa', 'high'));
  const moderate = report(vulnerable('left-pad', 'GHSA-bbbb-bbbb-bbbb', 'moderate'));
  const policy = [{ advisory: 'GHSA-aaaa-aaaa-aaaa', package: 'left-pad', reason: 'synthetic' }];
  const cases = [
    {
      name: 'an unlisted high advisory fails',
      audits: [{ label: 'x', full: high, prod: clean }],
      policy: [],
      expectFail: true,
    },
    {
      name: 'a listed advisory that reaches a shipped dependency fails',
      audits: [{ label: 'x', full: high, prod: high }],
      policy,
      expectFail: true,
    },
    {
      name: 'a stale policy entry fails',
      audits: [{ label: 'x', full: clean, prod: clean }],
      policy,
      expectFail: true,
    },
    {
      name: 'a policy entry without a reason fails',
      audits: [{ label: 'x', full: high, prod: clean }],
      policy: [{ advisory: 'GHSA-aaaa-aaaa-aaaa', package: 'left-pad', reason: '  ' }],
      expectFail: true,
    },
    {
      name: 'a listed development-only advisory passes, and a moderate one is not gated',
      audits: [
        { label: 'x', full: high, prod: clean },
        { label: 'y', full: moderate, prod: moderate },
      ],
      policy,
      expectFail: false,
    },
  ];
  return cases
    .filter((testCase) => (evaluate(testCase.audits, testCase.policy).length > 0) !== testCase.expectFail)
    .map((testCase) => testCase.name);
}

/** Run `npm audit --json` and parse it. `npm audit` exits non-zero when it finds anything. */
function audit(prefix, omitDev) {
  const args = ['audit', '--json'];
  if (omitDev) args.push('--omit=dev');
  if (prefix) args.push('--prefix', prefix);
  const result = spawnSync('npm', args, {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    shell: platform === 'win32',
  });
  let parsed;
  try {
    parsed = JSON.parse(result.stdout);
  } catch {
    throw new Error(
      `\`npm ${args.join(' ')}\` did not print JSON (exit ${result.status}): ` +
        `${String(result.stderr || result.stdout).trim().slice(0, 400)}`,
    );
  }
  if (parsed.error || typeof parsed.vulnerabilities !== 'object') {
    throw new Error(
      `\`npm ${args.join(' ')}\` reported an error instead of an audit: ` +
        `${JSON.stringify(parsed.error ?? parsed).slice(0, 400)}`,
    );
  }
  return parsed;
}

function main() {
  const broken = selfTest();
  if (broken.length > 0) {
    console.error('npm audit gate: negative controls misbehaved, the gate cannot be trusted:');
    for (const name of broken) console.error(`  - ${name}`);
    exit(2);
  }

  let audits;
  try {
    audits = PACKAGES.map(({ label, prefix }) => ({
      label,
      full: audit(prefix, false),
      prod: audit(prefix, true),
    }));
  } catch (error) {
    console.error(`npm audit gate: ${error.message}`);
    console.error('An audit that cannot be read is not a clean audit.');
    exit(1);
  }

  const violations = evaluate(audits, DEV_ONLY_EXEMPTIONS);
  for (const { label, full, prod } of audits) {
    const counts = full.metadata?.vulnerabilities ?? {};
    const shipped = prod.metadata?.vulnerabilities?.total ?? advisoriesOf(prod).length;
    console.log(
      `npm audit (${label}): ${counts.critical ?? 0} critical, ${counts.high ?? 0} high, ` +
        `${counts.moderate ?? 0} moderate, ${counts.low ?? 0} low vulnerable package(s); ` +
        `${shipped} in shipped dependencies`,
    );
  }
  if (violations.length > 0) {
    console.error(`npm audit gate: ${violations.length} violation(s):`);
    for (const violation of violations) console.error(`  - ${violation}`);
    exit(1);
  }
  console.log(
    `npm audit gate: no unlisted high or critical advisory; ${DEV_ONLY_EXEMPTIONS.length} ` +
      'development-only exemption(s) verified against `npm audit --omit=dev`; self-test 5 controls ok',
  );
}

if (argv[1] && import.meta.url === pathToFileURL(argv[1]).href) {
  main();
}
