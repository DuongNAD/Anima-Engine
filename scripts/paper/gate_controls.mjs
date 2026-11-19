#!/usr/bin/env node
// Run negative controls for every automated (and documented) gate in Anima Engine.
//
// Each control creates a detached temporary git worktree in os.tmpdir(), applies a single
// minimal fault patch from scripts/paper/gate_faults/<id>.patch without touching the working
// repository, executes the gate command, records the exit code and the verbatim line naming the
// fault, and removes the temporary worktree cleanly.
//
// Usage:
//   node scripts/paper/gate_controls.mjs
//   node scripts/paper/gate_controls.mjs --gate thread_rng_scan,cargo_fmt
//   node scripts/paper/gate_controls.mjs --list
//   node scripts/paper/gate_controls.mjs --json-out /abs/path/out.json

import { execFileSync, spawnSync } from 'node:child_process';
import {
  closeSync,
  existsSync,
  lstatSync,
  mkdtempSync,
  openSync,
  readFileSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import os from 'node:os';
import { dirname, join, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, '..', '..');
const FAULTS_DIR = join(__dirname, 'gate_faults');

function parseCliArgs(argv) {
  const gateIds = [];
  let listOnly = false;
  let jsonOut = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--list') {
      listOnly = true;
    } else if (arg === '--gate' && i + 1 < argv.length) {
      i += 1;
      for (const part of argv[i].split(',')) {
        const trimmed = part.trim();
        if (trimmed) gateIds.push(trimmed);
      }
    } else if (arg.startsWith('--gate=')) {
      for (const part of arg.slice('--gate='.length).split(',')) {
        const trimmed = part.trim();
        if (trimmed) gateIds.push(trimmed);
      }
    } else if (arg === '--json-out' && i + 1 < argv.length) {
      i += 1;
      jsonOut = resolve(argv[i]);
    } else {
      console.error(`Unknown or incomplete option: ${arg}`);
      process.exit(2);
    }
  }
  return { gateIds, listOnly, jsonOut };
}

function findGitBash() {
  if (process.platform !== 'win32') return 'bash';
  const candidates = [
    'E:\\Git\\bin\\bash.exe',
    'C:\\Program Files\\Git\\bin\\bash.exe',
    'C:\\Program Files (x86)\\Git\\bin\\bash.exe',
    'E:\\Git\\usr\\bin\\bash.exe',
  ];
  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate;
  }
  const whereRes = spawnSync('where.exe', ['bash'], { encoding: 'utf8' });
  if (whereRes.status === 0 && whereRes.stdout) {
    for (const line of whereRes.stdout.split(/\r?\n/)) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      if (/\\System32\\|\\WindowsApps\\/i.test(trimmed)) continue;
      if (existsSync(trimmed)) return trimmed;
    }
  }
  throw new Error('Could not locate Git Bash (bash.exe) on Windows');
}

function cargoEnv() {
  const winDebugDefaults =
    process.platform === 'win32'
      ? {
          CARGO_PROFILE_DEV_DEBUG: process.env.CARGO_PROFILE_DEV_DEBUG ?? '0',
          CARGO_PROFILE_TEST_DEBUG: process.env.CARGO_PROFILE_TEST_DEBUG ?? '0',
        }
      : {};
  return {
    ...process.env,
    ...winDebugDefaults,
    CARGO_BUILD_JOBS:
      process.env.CARGO_BUILD_JOBS || (process.platform === 'win32' ? '2' : '4'),
    CARGO_INCREMENTAL: '0',
    ANIMA_USE_GPU: '0',
    CARGO_TARGET_DIR:
      process.env.ANIMA_GATE_CARGO_TARGET_DIR ||
      process.env.CARGO_TARGET_DIR ||
      join(REPO_ROOT, 'src-tauri', 'target'),
  };
}

function ensureSafeAnimaCargo() {
  if (process.platform !== 'win32') return;
  try {
    os.setPriority(process.pid, os.constants.priority.PRIORITY_LOW);
  } catch {
    // Best-effort low priority so concurrent host tasks keep headroom.
  }
  const psScript = [
    '$os = Get-CimInstance Win32_OperatingSystem;',
    '$freeKb = [int64]$os.FreePhysicalMemory;',
    '$procs = @(Get-CimInstance Win32_Process | Where-Object { $_.Name -match "^(cargo|rustc)\\.exe$" });',
    '$anima = @($procs | Where-Object { $_.CommandLine -match "Anima-Engine|anima-wt|anima-m2-wt|anima_engine" });',
    '$other = @($procs | Where-Object { $_.CommandLine -notmatch "Anima-Engine|anima-wt|anima-m2-wt|anima_engine" });',
    'Write-Output ("FREE_KB=" + $freeKb + ";ANIMA=" + $anima.Count + ";OTHER=" + $other.Count);',
    'foreach ($p in $anima) { Write-Output ("ANIMA_PROC=" + $p.ProcessId + " " + $p.Name + " " + $p.CommandLine) }',
  ].join(' ');
  for (let attempt = 0; attempt < 60; attempt += 1) {
    const res = spawnSync('powershell.exe', ['-NoProfile', '-Command', psScript], {
      encoding: 'utf8',
    });
    const out = res.stdout || '';
    const m = /FREE_KB=(\d+);ANIMA=(\d+);OTHER=(\d+)/.exec(out);
    if (!m) {
      if (attempt === 59) {
        throw new Error(`Could not inspect Windows cargo/RAM state:\n${out}\n${res.stderr || ''}`);
      }
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 5000);
      continue;
    }
    const freeKb = Number(m[1]);
    const animaCount = Number(m[2]);
    const otherCount = Number(m[3]);
    const ramReady = otherCount === 0 || freeKb >= 5_000_000;
    if (animaCount === 0 && ramReady) return;
    if (attempt === 59) {
      throw new Error(
        `Shared Dell cargo guard timed out (FREE_KB=${freeKb}, ANIMA=${animaCount}, OTHER=${otherCount}):\n${out}`,
      );
    }
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 15_000);
  }
}

function runShell(cmd, cwd, env = process.env) {
  if (process.platform === 'win32') {
    const res = spawnSync('cmd.exe', ['/d', '/c', cmd], {
      cwd,
      env,
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
    });
    return {
      status: res.status ?? 1,
      stdout: res.stdout || '',
      stderr: res.stderr || '',
      output: `${res.stdout || ''}\n${res.stderr || ''}`,
    };
  }
  const res = spawnSync('sh', ['-c', cmd], {
    cwd,
    env,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  return {
    status: res.status ?? 1,
    stdout: res.stdout || '',
    stderr: res.stderr || '',
    output: `${res.stdout || ''}\n${res.stderr || ''}`,
  };
}

function runBash(script, cwd, env = process.env) {
  const bashExe = findGitBash();
  const res = spawnSync(bashExe, ['-eo', 'pipefail', '-c', script], {
    cwd,
    env,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  return {
    status: res.status ?? 1,
    stdout: res.stdout || '',
    stderr: res.stderr || '',
    output: `${res.stdout || ''}\n${res.stderr || ''}`,
  };
}

function runCommandToFile(exe, args, cwd, env, outFile) {
  const fd = openSync(outFile, 'w');
  let res;
  try {
    res = spawnSync(exe, args, {
      cwd,
      env,
      stdio: ['ignore', fd, fd],
    });
  } finally {
    closeSync(fd);
  }
  if (res.error) throw res.error;
  const captured = existsSync(outFile) ? readFileSync(outFile, 'utf8') : '';
  return {
    status: res.status ?? 1,
    stdout: captured,
    stderr: '',
    output: captured,
  };
}

function cleanupStaleWorktree(wt) {
  for (const rel of ['node_modules', join('tests', 'node_modules')]) {
    const linkPath = join(wt, rel);
    if (existsSync(linkPath) || lstatSync(linkPath, { throwIfNoEntry: false })) {
      unlinkSync(linkPath);
    }
  }
  try {
    execFileSync('git', ['worktree', 'remove', '--force', wt], { cwd: REPO_ROOT, stdio: 'pipe' });
  } catch {
    // Fall through to prune + rmSync below
  }
  try {
    execFileSync('git', ['worktree', 'prune'], { cwd: REPO_ROOT, stdio: 'pipe' });
  } catch {
    // Best-effort prune
  }
  rmSync(wt, { recursive: true, force: true });
}

function linkNodeModules(wt, createdLinks) {
  const linkType = process.platform === 'win32' ? 'junction' : 'dir';
  const pairs = [
    [join(REPO_ROOT, 'node_modules'), join(wt, 'node_modules')],
    [join(REPO_ROOT, 'tests', 'node_modules'), join(wt, 'tests', 'node_modules')],
  ];
  for (const [src, dst] of pairs) {
    if (existsSync(src) && !existsSync(dst)) {
      symlinkSync(src, dst, linkType);
      createdLinks.push(dst);
    }
  }
}

function unlinkNodeModules(createdLinks) {
  for (let i = createdLinks.length - 1; i >= 0; i -= 1) {
    const linkPath = createdLinks[i];
    if (existsSync(linkPath) || lstatSync(linkPath, { throwIfNoEntry: false })) {
      unlinkSync(linkPath);
    }
    if (existsSync(linkPath)) {
      throw new Error(`Refusing to continue: junction/symlink still present at ${linkPath}`);
    }
  }
}

function withTempWorktree(patchFile, { needsNodeModules = false, needsCargo = false }, fn) {
  if (needsCargo) ensureSafeAnimaCargo();

  const tmpBase = mkdtempSync(join(os.tmpdir(), 'anima-m2-'));
  const wt = join(os.tmpdir(), 'anima-m2-wt');
  const createdLinks = [];

  try {
    cleanupStaleWorktree(wt);
    execFileSync(
      'git',
      ['-c', 'core.autocrlf=false', '-c', 'core.eol=lf', 'worktree', 'add', '--detach', wt, 'HEAD'],
      { cwd: REPO_ROOT, stdio: 'pipe' },
    );

    if (patchFile) {
      const patchPath = join(FAULTS_DIR, patchFile);
      if (!existsSync(patchPath)) {
        throw new Error(`Missing fault patch: ${patchPath}`);
      }
      const patchBuf = readFileSync(patchPath);
      const applied = spawnSync(
        'git',
        ['-c', 'core.autocrlf=false', 'apply', '--index', '--whitespace=nowarn', '-'],
        { cwd: wt, input: patchBuf, encoding: 'buffer' },
      );
      if (applied.status !== 0) {
        throw new Error(
          `git apply failed for ${patchFile}:\n${(applied.stderr || '').toString('utf8')}`,
        );
      }
    }

    if (needsNodeModules) {
      linkNodeModules(wt, createdLinks);
    }

    return fn(wt, tmpBase);
  } finally {
    unlinkNodeModules(createdLinks);
    cleanupStaleWorktree(wt);
    rmSync(tmpBase, { recursive: true, force: true });
  }
}

const ANSI_ESCAPE_RE = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, 'g');

function pickLine(output, matchers) {
  const lines = output
    .replace(ANSI_ESCAPE_RE, '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith('>') && !line.startsWith('npm error'));
  for (const matcher of matchers) {
    const found = lines.find((line) => matcher.test(line));
    if (found) return found;
  }
  const raw = output
    .replace(ANSI_ESCAPE_RE, '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  return lines[0] || raw[0] || '(no output)';
}

function sanitizeMdCell(text) {
  return String(text).replace(/\|/g, '\\|').replace(/\s+/g, ' ').trim();
}

function formatMdCode(text) {
  const clean = sanitizeMdCell(text);
  return clean.includes('`') ? `\`\` ${clean} \`\`` : `\`${clean}\``;
}

function runCheckTestTargetsScript(wt, capturePath, profileArg) {
  const args = ['scripts/check_test_targets.mjs', capturePath];
  if (profileArg) args.push('--profile', profileArg);
  const check = spawnSync('node', args, {
    cwd: wt,
    env: process.env,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  return {
    status: check.status ?? 1,
    stdout: check.stdout || '',
    stderr: check.stderr || '',
    output: `${check.stdout || ''}\n${check.stderr || ''}`,
  };
}

function runCheckTestTargetsPolicySubset(wt, tmpBase, profileArg) {
  const capturePath = join(tmpBase, 'cargo-test-output.txt');
  const docCapturePath = join(tmpBase, 'cargo-doc-output.txt');
  const env = { ...cargoEnv(), ANIMA_ENERGY_GATE_TICKS: '10' };
  const tauriDir = join(wt, 'src-tauri');
  const subsetArgs = [
    'test',
    '--features',
    'desktop',
    '--bin',
    'anima-engine',
    '--test',
    'shared_domain_law_tests',
    '--test',
    'determinism_gate_tests',
    '--test',
    'energy_conservation_tests',
    '--test',
    'adversarial_challenger_tests',
    '--test',
    'migration_tests',
    '--test',
    'migration_high_throughput_tests',
    '--test',
    'migration_narrow_bounds_loop_tests',
    '--test',
    'migration_panic_tests',
    '--test',
    'migration_stress_tests',
    '--test',
    'phase5_websocket_reuse',
    '--test',
    'phase5_burn_wgpu_fallback',
    '--',
    '--nocapture',
  ];
  const testRes = runCommandToFile('cargo', subsetArgs, tauriDir, env, capturePath);
  if (testRes.status !== 0) {
    throw new Error(
      `Prerequisite cargo test policy subset failed (exit ${testRes.status}):\n${testRes.output.slice(-2000)}`,
    );
  }
  const docRes = runCommandToFile(
    'cargo',
    ['test', '--workspace', '--doc', '--', '--nocapture'],
    tauriDir,
    env,
    docCapturePath,
  );
  if (docRes.status !== 0) {
    throw new Error(
      `Prerequisite cargo test --workspace --doc failed (exit ${docRes.status}):\n${docRes.output.slice(-2000)}`,
    );
  }
  writeFileSync(capturePath, `${testRes.stdout}\n${docRes.stdout}\n`, 'utf8');
  return runCheckTestTargetsScript(wt, capturePath, profileArg);
}

function runCheckTestTargetsFullUnscheduled(wt, ctx, profileArg) {
  if (!ctx.unscheduledCapturePath || !existsSync(ctx.unscheduledCapturePath)) {
    const capturePath = join(os.tmpdir(), `anima-m2-unscheduled-capture-${process.pid}.txt`);
    const docCapturePath = join(os.tmpdir(), `anima-m2-unscheduled-doc-capture-${process.pid}.txt`);
    const env = { ...cargoEnv(), ANIMA_ENERGY_GATE_TICKS: '10' };
    const tauriDir = join(wt, 'src-tauri');
    // Use Cargo's real `--test` glob over the migration/phase5 policy targets so Cargo evaluates
    // `required-features` on `migration_tests` (dropping it when mistyped) without compiling all
    // 161 targets (>30 min / MSVC link.exe OOM on 16 GB RAM).
    const unscheduledArgs = [
      'test',
      '--features',
      'desktop',
      '--bin',
      'anima-engine',
      '--test',
      'shared_domain_law_tests',
      '--test',
      'determinism_gate_tests',
      '--test',
      'energy_conservation_tests',
      '--test',
      'adversarial_challenger_tests',
      '--test',
      'migration*',
      '--test',
      'phase5_*',
      '--',
      '--nocapture',
    ];
    const testRes = runCommandToFile('cargo', unscheduledArgs, tauriDir, env, capturePath);
    if (testRes.status !== 0) {
      throw new Error(
        `Prerequisite cargo test --features desktop (policy globs) failed (exit ${testRes.status}):\n${testRes.output.slice(-2000)}`,
      );
    }
    const docRes = runCommandToFile(
      'cargo',
      ['test', '--workspace', '--doc', '--', '--nocapture'],
      tauriDir,
      env,
      docCapturePath,
    );
    if (docRes.status !== 0) {
      rmSync(docCapturePath, { force: true });
      throw new Error(
        `Prerequisite cargo test --workspace --doc failed (exit ${docRes.status}):\n${docRes.output.slice(-2000)}`,
      );
    }
    writeFileSync(capturePath, `${testRes.stdout}\n${docRes.stdout}\n`, 'utf8');
    rmSync(docCapturePath, { force: true });
    ctx.unscheduledCapturePath = capturePath;
    ctx.tempFilesToClean.push(capturePath);
  }
  return runCheckTestTargetsScript(wt, ctx.unscheduledCapturePath, profileArg);
}

const GATES = [
  // ---- Expected VACUOUS / CI-omission controls (listed first) -------------------------------
  {
    id: 'thread_rng_scan',
    source: 'hardening.yml:75-76',
    ciCommand:
      "n=$(grep -rn 'rand::thread_rng()' src-tauri/src --include='*.rs' | grep -vc ':[[:space:]]*//' || true); [ \"$n\" -eq 0 ]",
    faultSummary: 'Add `use rand::thread_rng; let _ = thread_rng();` to `src-tauri/src/main.rs`',
    patch: 'thread_rng_scan.patch',
    needsCargo: false,
    run(wt) {
      return runBash(
        "n=$(grep -rn 'rand::thread_rng()' src-tauri/src --include='*.rs' | grep -vc ':[[:space:]]*//' || true); echo \"thread_rng() literal matches: $n\"; [ \"$n\" -eq 0 ]",
        wt,
      );
    },
    matchers: [/thread_rng\(\) literal matches/],
  },
  {
    id: 'check_test_targets_unscheduled',
    source: 'ci.yml:182',
    ciCommand: 'node ../scripts/check_test_targets.mjs ../cargo-test-output.txt',
    faultSummary:
      'Mistype `required-features = ["networking_m2_typo"]` on `migration_tests` in `src-tauri/Cargo.toml`',
    patch: 'check_test_targets_unscheduled.patch',
    needsCargo: true,
    run(wt, _tmpBase, ctx) {
      return runCheckTestTargetsFullUnscheduled(wt, ctx, null);
    },
    matchers: [/no --profile given/, /test targets:/],
  },
  {
    id: 'ci_ipc_arg_case',
    source: 'ci.yml vs package.json:33',
    ciCommand:
      'npm run lint && node scripts/eslint_ratchet.mjs && npm run typecheck:tests && npm run test && npm run test:frontend && npm run build && node scripts/check_ipc_reference.mjs',
    faultSummary:
      'Add `invoke("save_simulation_state", { file_path })` (snake_case) helper in `src/components/Landscape/utils/worldCache.ts` (`check:ipc` absent from `ci.yml`)',
    patch: 'check_ipc_arg_case.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        'npm run lint && node scripts/eslint_ratchet.mjs && npm run typecheck:tests && npm run test && npm run test:frontend && npm run build && node scripts/check_ipc_reference.mjs',
        wt,
        { ...process.env, CI: 'true' },
      );
    },
    matchers: [/ipc reference check:/, /eslint: 0 errors, 0 warnings/],
  },

  // ---- Missing / baseline-red / >30 min gates ----------------------------------------------
  {
    id: 'conflict_marker_scan',
    source: 'CARD M2 (planned, unbuilt)',
    ciCommand: '(no automated check in ci.yml, hardening.yml, or package.json)',
    faultSummary: 'Conflict-marker scan planned in CARD M2 without implementation',
    patch: null,
    staticResult: {
      exitCode: '—',
      namingLine: 'NOT RUN (no automated gate in ci.yml, hardening.yml, or package.json)',
      verdict: 'NOT RUN (no automated gate)',
    },
  },
  {
    id: 'check_licenses_complete',
    source: 'package.json:42',
    ciCommand: 'npm run check:licenses-complete',
    faultSummary: 'Release completeness check requires 0 unresolved licence texts (`utf8-chars@3.0.7` unresolved on trunk)',
    patch: null,
    staticResult: {
      exitCode: '—',
      namingLine:
        'NOT RUN (baseline red: 1 distributed component(s) have no obtainable licence text. See licensing/UNRESOLVED.md.)',
      verdict: 'NOT RUN (baseline red: 1 unresolved crate)',
    },
  },

  // ---- Gates > 30 min per fault (runtime measured once on clean tree) -----------------------
  {
    id: 'cargo_test_desktop_full',
    source: 'ci.yml:174',
    ciCommand: 'cargo test --features desktop -- --nocapture 2>&1 | tee ../cargo-test-output.txt',
    faultSummary: 'Full all-target desktop test run per fault exceeds 30 min budget (targeted suites below cover faults)',
    patch: null,
    staticResult: {
      exitCode: '—',
      namingLine: 'NOT RUN (time: full clean desktop suite exceeds 30 min budget per fault; covered via targeted suites)',
      verdict: 'NOT RUN (time)',
    },
  },
  {
    id: 'hardening_stability_5x',
    source: 'hardening.yml:52,59-62',
    ciCommand: 'for i in 1 2 3 4 5; do RAYON_NUM_THREADS=1 cargo test --release --quiet; done',
    faultSummary: '5x sequential full release test runs per fault exceed 30 min budget',
    patch: null,
    staticResult: {
      exitCode: '—',
      namingLine: 'NOT RUN (time: 5x sequential cargo test --release runs exceed 30 min budget per fault)',
      verdict: 'NOT RUN (time)',
    },
  },

  // ---- Rust CI & hardening gates ------------------------------------------------------------
  {
    id: 'cargo_fmt',
    source: 'ci.yml:92',
    ciCommand: 'cargo fmt --check',
    faultSummary: 'Misformat `anima_engine_lib::run( );` in `src-tauri/src/main.rs`',
    patch: 'cargo_fmt.patch',
    needsCargo: true,
    run(wt) {
      return runShell('cargo fmt --check', join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/Diff in .*main\.rs/i, /anima_engine_lib::run/],
  },
  {
    id: 'cargo_audit',
    source: 'ci.yml:107',
    ciCommand: 'cargo audit',
    faultSummary: 'Remove `RUSTSEC-2026-0367` exemption from `src-tauri/.cargo/audit.toml`',
    patch: 'cargo_audit.patch',
    needsCargo: true,
    run(wt) {
      return runShell('cargo audit', join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/RUSTSEC-2026-0367/, /ID:\s+RUSTSEC-2026-0367/],
  },
  {
    id: 'cargo_deny',
    source: 'ci.yml:122',
    ciCommand: 'cargo deny check',
    faultSummary: 'Drop `"MIT"` from `licenses.allow` in `src-tauri/deny.toml`',
    patch: 'cargo_deny.patch',
    needsCargo: true,
    run(wt) {
      return runShell('cargo deny check', join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/failed to satisfy license requirements/, /MIT/, /licenses FAILED/],
  },
  {
    id: 'cargo_clippy_desktop',
    source: 'ci.yml:126',
    ciCommand: 'cargo clippy --all-targets --features desktop -- -D warnings',
    faultSummary: 'Add `let unused_m2_fault = 1usize;` in `src-tauri/src/main.rs`',
    patch: 'cargo_clippy_desktop.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        'cargo clippy --all-targets --features desktop -- -D warnings',
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/unused_m2_fault/, /unused variable/],
  },
  {
    id: 'feature_separation',
    source: 'ci.yml:135-138',
    ciCommand:
      "cargo clippy --all-targets --no-default-features -- -D warnings && leaked=$(cargo tree --no-default-features -e normal | grep -cE 'neo4rs|tokio-tungstenite|burn-wgpu|naga' || true); test \"$leaked\" -eq 0",
    faultSummary: 'Make `tokio-tungstenite` non-optional in `src-tauri/Cargo.toml`',
    patch: 'feature_separation.patch',
    needsCargo: true,
    run(wt) {
      const tauriDir = join(wt, 'src-tauri');
      const env = cargoEnv();
      const clippyRes = runShell(
        'cargo clippy --all-targets --no-default-features -- -D warnings',
        tauriDir,
        env,
      );
      if (clippyRes.status !== 0) {
        throw new Error(
          `Prerequisite default-feature clippy failed (exit ${clippyRes.status}):\n${clippyRes.output.slice(-2000)}`,
        );
      }
      return runBash(
        "leaked=$(cargo tree --no-default-features -e normal | grep -cE 'neo4rs|tokio-tungstenite|burn-wgpu|naga' || true); echo \"gated crates present in a default build: $leaked\"; test \"$leaked\" -eq 0",
        tauriDir,
        env,
      );
    },
    matchers: [/gated crates present in a default build:/],
  },
  {
    id: 'ts_rs_parity',
    source: 'ci.yml:147-148',
    ciCommand: 'cargo test --lib export_bindings && git diff --exit-code -- ../src/types/generated',
    faultSummary: 'Add `#[ts(rename = "x_m2_fault")]` to `SegmentState::x` in `simulation_state.rs`',
    patch: 'ts_rs_parity.patch',
    needsCargo: true,
    run(wt) {
      const tauriDir = join(wt, 'src-tauri');
      const env = cargoEnv();
      const testRes = runShell('cargo test --lib export_bindings', tauriDir, env);
      if (testRes.status !== 0) {
        throw new Error(
          `Prerequisite export_bindings test failed (exit ${testRes.status}):\n${testRes.output.slice(-2000)}`,
        );
      }
      return runShell('git --no-pager diff --exit-code -- ../src/types/generated', tauriDir, env);
    },
    matchers: [/x_m2_fault/, /SegmentState\.ts/],
  },
  {
    id: 'check_test_targets_empty',
    source: 'ci.yml:182 (policy)',
    ciCommand: 'node ../scripts/check_test_targets.mjs ../cargo-test-output.txt --profile desktop',
    faultSummary: 'Add `#![cfg(any())]` at top of `src-tauri/tests/shared_domain_law_tests.rs`',
    patch: 'check_test_targets_empty.patch',
    needsCargo: true,
    run(wt, tmpBase) {
      return runCheckTestTargetsPolicySubset(wt, tmpBase, 'desktop');
    },
    matchers: [/compiled but ran zero tests/, /shared_domain_law_tests\.rs/],
  },
  {
    id: 'check_test_targets_ignored',
    source: 'ci.yml:182 (policy)',
    ciCommand: 'node ../scripts/check_test_targets.mjs ../cargo-test-output.txt --profile desktop',
    faultSummary: 'Add `#[ignore]` to `the_engine_reads_the_domain_crates_values_for_the_time_laws`',
    patch: 'check_test_targets_ignored.patch',
    needsCargo: true,
    run(wt, tmpBase) {
      return runCheckTestTargetsPolicySubset(wt, tmpBase, 'desktop');
    },
    matchers: [/is #\[ignore\]d and is not in IGNORED_TESTS/, /the_engine_reads_the_domain_crates_values_for_the_time_laws/],
  },
  {
    id: 'check_test_targets_unscheduled_profile',
    source: 'scripts/check_test_targets.mjs:29',
    ciCommand: 'node ../scripts/check_test_targets.mjs ../cargo-test-output.txt --profile desktop',
    faultSummary:
      'Mistype `required-features = ["networking_m2_typo"]` on `migration_tests` (checked with `--profile desktop`)',
    patch: 'check_test_targets_unscheduled.patch',
    needsCargo: true,
    run(wt, _tmpBase, ctx) {
      return runCheckTestTargetsFullUnscheduled(wt, ctx, 'desktop');
    },
    matchers: [/profile "desktop" enables "networking", so tests\/migration_tests\.rs must run/, /migration_tests\.rs/],
  },
  {
    id: 'schedule_ambiguity_simrng',
    source: 'CARD M2 (ci.yml:174)',
    ciCommand: 'cargo test --test schedule_ambiguity_tests',
    faultSummary: 'Put `(spawn_food_system, seed_dropping_system)` in parallel in `simulation_schedule.rs`',
    patch: 'schedule_ambiguity_simrng.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        'cargo test --test schedule_ambiguity_tests -- --nocapture',
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/ResMut<SimRng>/, /spawn_food_system/, /schedule ambiguity|conflicting access/i],
  },
  {
    id: 'schedule_ambiguity_energy',
    source: 'CARD M2 (ci.yml:174)',
    ciCommand: 'cargo test --test schedule_ambiguity_tests',
    faultSummary: 'Put `(combat_system, metabolic_decay_system)` in parallel in `simulation_schedule.rs`',
    patch: 'schedule_ambiguity_energy.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        'cargo test --test schedule_ambiguity_tests -- --nocapture',
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/combat_system.*metabolic_decay_system|metabolic_decay_system.*combat_system/, /ambiguity/i],
  },
  {
    id: 'zero_alloc',
    source: 'CARD M2 (ci.yml:174)',
    ciCommand: 'cargo test --test emit_zero_alloc_tests',
    faultSummary: 'Allocate `Vec::<u8>::with_capacity(64)` in `refresh_tick_payload` (`emit.rs`)',
    patch: 'zero_alloc.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        'cargo test --test emit_zero_alloc_tests -- --nocapture',
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/allocated/, /zero-allocation|0 bytes/, /test .* FAILED/],
  },
  {
    id: 'closed_eu',
    source: 'CARD M2 (ci.yml:174)',
    ciCommand: 'cargo test --test energy_conservation_tests',
    faultSummary: 'Inject `+ 1.0` EU per entity in `metabolic_decay_system` (`world_systems.rs`)',
    patch: 'closed_eu.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        'cargo test --test energy_conservation_tests -- --nocapture',
        join(wt, 'src-tauri'),
        { ...cargoEnv(), ANIMA_ENERGY_GATE_TICKS: '60' },
      );
    },
    matchers: [/energy|closed|drift|conserve|residual/i, /test .* FAILED/],
  },
  {
    id: 'live_experiment_fork',
    source: 'CARD M2 (ci.yml:174)',
    ciCommand: 'cargo test --test live_experiment_tests',
    faultSummary: 'Ignore branch `forcings` in `checkpoint_fork_with_exotic` (`experiment_runner/fork.rs`)',
    patch: 'live_experiment_fork.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        'cargo test --test live_experiment_tests -- --nocapture',
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/exotic|branch|divergence|fork/i, /test .* FAILED/],
  },
  {
    id: 'live_experiment_saveload',
    source: 'CARD M2 (ci.yml:174)',
    ciCommand: 'cargo test --test live_experiment_tests',
    faultSummary: 'Reset `sim_rng_pos` to `0` on restore in `build_live_world` (`live_experiment.rs`)',
    patch: 'live_experiment_saveload.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        'cargo test --test live_experiment_tests -- --nocapture',
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/save|restore|checksum|diverge|rng/i, /test .* FAILED/],
  },
  {
    id: 'determinism_golden_hash',
    source: 'hardening.yml:68',
    ciCommand: 'cargo test --release --test determinism_tests',
    faultSummary: 'Perturb `SimRng::for_world(world_identity.seed.wrapping_add(1))` in `core/ecs.rs`',
    patch: 'determinism_golden_hash.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        'cargo test --release --test determinism_tests -- --nocapture',
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/golden|GOLDEN|world_hash|assertion `left == right` failed/i, /test .* FAILED/],
  },
  {
    id: 'check_licenses',
    source: 'ci.yml:211',
    ciCommand: 'npm run check:licenses',
    faultSummary: 'Flip 1 hex char in `sourceSha256` inside `licensing/third-party-index.json`',
    patch: 'check_licenses.patch',
    needsCargo: true,
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:licenses', wt, cargoEnv());
    },
    matchers: [/differs|stale|sha256|adler2|check:licenses|third-party/i],
  },
  {
    id: 'check_sbom',
    source: 'ci.yml:216',
    ciCommand: 'npm run check:sbom',
    faultSummary: 'Change `metadata.component.name` to `anima-engine-m2-fault` in `sbom.cdx.json`',
    patch: 'check_sbom.patch',
    needsCargo: true,
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:sbom', wt, cargoEnv());
    },
    matchers: [/differs|out of date|sbom|gen:sbom/i],
  },
  {
    id: 'check_sbom_schema',
    source: 'ci.yml:225',
    ciCommand: 'npm run check:sbom-schema',
    faultSummary: 'Add `metadata.timestamp` disallowed by CycloneDX 1.5 strict check in `sbom.cdx.json`',
    patch: 'check_sbom_schema.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:sbom-schema', wt);
    },
    matchers: [/timestamp/, /schema|CycloneDX/i],
  },
  {
    id: 'check_notice',
    source: 'ci.yml:230',
    ciCommand: 'npm run check:notice',
    faultSummary: 'Delete `- **Rust — ... crates.**` bullet from `NOTICE`',
    patch: 'check_notice.patch',
    needsCargo: true,
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:notice', wt, cargoEnv());
    },
    matchers: [/NOTICE/, /differs|gen:notice|stale/i],
  },
  {
    id: 'check_flora_footprint',
    source: 'ci.yml:235',
    ciCommand: 'npm run check:flora-footprint',
    faultSummary: 'Change `FLORA_FOOTPRINT_UNIT_RADIUS[FloraType.Pine]` to `0.99` in `floraClearance.ts`',
    patch: 'check_flora_footprint.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:flora-footprint', wt);
    },
    matchers: [/Pine|unit radius|mismatch|differs|expected|0\.99/i],
  },
  {
    id: 'verify_dataflow',
    source: 'ci.yml:242-243',
    ciCommand:
      'cargo modules dependencies --lib --no-externs --package anima-engine > ../dataflow-truth.dot && python3 ../scripts/verify_dataflow.py ../dataflow.json ../dataflow-truth.dot 1',
    faultSummary: 'Insert invented `physics -> commands` edge into `dataflow.json`',
    patch: 'verify_dataflow.patch',
    needsCargo: true,
    run(wt, tmpBase) {
      const dotPath = join(tmpBase, 'dataflow-truth.dot');
      const tauriDir = join(wt, 'src-tauri');
      const modRes = runShell(
        `cargo modules dependencies --lib --no-externs --package anima-engine > "${dotPath}"`,
        tauriDir,
        cargoEnv(),
      );
      if (modRes.status !== 0) {
        throw new Error(
          `Prerequisite cargo modules failed (exit ${modRes.status}):\n${modRes.output.slice(-2000)}`,
        );
      }
      const pyExe = process.platform === 'win32' ? 'python' : 'python3';
      const pyRes = spawnSync(
        pyExe,
        [join(wt, 'scripts', 'verify_dataflow.py'), join(wt, 'dataflow.json'), dotPath, '1'],
        { cwd: tauriDir, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
      );
      return {
        status: pyRes.status ?? 1,
        stdout: pyRes.stdout || '',
        stderr: pyRes.stderr || '',
        output: `${pyRes.stdout || ''}\n${pyRes.stderr || ''}`,
      };
    },
    matchers: [/physics\s*->\s*commands/, /invented|unverified|extra/i],
  },

  // ---- Frontend, Hygiene, Docs & Policy Gates ----------------------------------------------
  {
    id: 'check_npm_audit',
    source: 'ci.yml:280',
    ciCommand: 'node scripts/check_npm_audit.mjs',
    faultSummary: 'Add stale withdrawn exemption `GHSA-fc5q-39xc-vjpf` to `scripts/npm_audit_policy.mjs`',
    patch: 'check_npm_audit.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('node scripts/check_npm_audit.mjs', wt);
    },
    matchers: [/GHSA-fc5q-39xc-vjpf/, /stale/i],
  },
  {
    id: 'check_text_hygiene',
    source: 'ci.yml:291',
    ciCommand: 'npm run check:text-hygiene',
    faultSummary: 'Embed NUL byte `\\x00` inside `scripts/npm_audit_policy.mjs` via binary patch',
    patch: 'check_text_hygiene.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:text-hygiene', wt);
    },
    matchers: [/npm_audit_policy\.mjs/, /control byte|U\+0000/i],
  },
  {
    id: 'check_root_hygiene',
    source: 'ci.yml:297',
    ciCommand: 'npm run check:root-hygiene',
    faultSummary: 'Add unreviewed `unreviewed_m2_fault.txt` at repository root',
    patch: 'check_root_hygiene.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:root-hygiene', wt);
    },
    matchers: [/unreviewed_m2_fault\.txt/, /allow-list|not in root_allowlist/i],
  },
  {
    id: 'check_e2',
    source: 'ci.yml:301',
    ciCommand: 'npm run check:e2',
    faultSummary: 'Alter `control_mean` in `artifacts/experiments/e2-evolved-brain-default/effects.json`',
    patch: 'check_e2.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:e2', wt);
    },
    matchers: [/effects\.json/, /digest|sha256|mismatch/i],
  },
  {
    id: 'npm_lint',
    source: 'ci.yml:305',
    ciCommand: 'npm run lint',
    faultSummary: 'Add empty statement `if (Boolean(1)) {}` to `src/main.tsx`',
    patch: 'npm_lint.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run lint', wt);
    },
    matchers: [/no-empty/, /Empty block statement/i],
  },
  {
    id: 'eslint_ratchet',
    source: 'ci.yml:309',
    ciCommand: 'node scripts/eslint_ratchet.mjs',
    faultSummary: 'Add `export const _m2AnyFault: any = 1;` to `src/main.tsx`',
    patch: 'eslint_ratchet.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('node scripts/eslint_ratchet.mjs', wt);
    },
    matchers: [/warning\(s\)|ceiling/i, /no-explicit-any/, /ESLint/i],
  },
  {
    id: 'typecheck_tests',
    source: 'ci.yml:322',
    ciCommand: 'npm run typecheck:tests',
    faultSummary: "Add `export const _m2TypeFault: number = 'bad';` to `tests/frontend/mapManifestEvidence.test.ts`",
    patch: 'typecheck_tests.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run typecheck:tests', wt);
    },
    matchers: [/_m2TypeFault/, /TS2322/],
  },
  {
    id: 'vitest_src',
    source: 'ci.yml:326',
    ciCommand: 'npm run test',
    faultSummary: 'Break `FLORA_COLLIDER_BASE_RADIUS` assertion in `src/__tests__/floraClearance.test.ts`',
    patch: 'vitest_src.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run test', wt);
    },
    matchers: [/FAIL\s+src\/__tests__\/floraClearance\.test\.ts/, /AssertionError/],
  },
  {
    id: 'vitest_tests',
    source: 'ci.yml:330',
    ciCommand: 'npm run test:frontend',
    faultSummary: 'Invert `expect(res.ok).toBe(false)` in `tests/frontend/mapManifestEvidence.test.ts`',
    patch: 'vitest_tests.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run test:frontend', wt, { ...process.env, CI: 'true' });
    },
    matchers: [/FAIL\s+frontend\/mapManifestEvidence\.test\.ts/, /AssertionError/],
  },
  {
    id: 'npm_build',
    source: 'ci.yml:334',
    ciCommand: 'npm run build',
    faultSummary: "Add `export const _m2BuildFault: number = 'bad';` to `src/main.tsx`",
    patch: 'npm_build.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run build', wt);
    },
    matchers: [/_m2BuildFault/, /TS2322/],
  },
  {
    id: 'check_bundle_closure',
    source: 'ci.yml:352',
    ciCommand: 'npm run build && npm run check:bundle-closure',
    faultSummary: 'Import dev-only `ajv` into `src/main.tsx` so bundle closure shifts',
    patch: 'check_bundle_closure.patch',
    needsNodeModules: true,
    run(wt) {
      const buildRes = runShell('npm run build', wt);
      if (buildRes.status !== 0) {
        throw new Error(
          `Prerequisite npm run build failed for check_bundle_closure (exit ${buildRes.status}):\n${buildRes.output.slice(-2000)}`,
        );
      }
      return runShell('npm run check:bundle-closure', wt);
    },
    matchers: [/bundle-closure\.json differs/, /\+.*"ajv"/],
  },
  {
    id: 'check_csp',
    source: 'ci.yml:356',
    ciCommand: 'npm run build && npm run check:csp',
    faultSummary: 'Add external `<script src="https://example.com/m2-fault.js"></script>` to `index.html`',
    patch: 'check_csp.patch',
    needsNodeModules: true,
    run(wt) {
      const buildRes = runShell('npm run build', wt);
      if (buildRes.status !== 0) {
        throw new Error(
          `Prerequisite npm run build failed for check_csp (exit ${buildRes.status}):\n${buildRes.output.slice(-2000)}`,
        );
      }
      return runShell('npm run check:csp', wt);
    },
    matchers: [/example\.com\/m2-fault\.js/, /CSP|script-src|external/i],
  },
  {
    id: 'check_bundle',
    source: 'ci.yml:364',
    ciCommand: 'npm run build && npm run check:bundle',
    faultSummary: 'Eagerly import `three` into dashboard root `src/App.tsx`',
    patch: 'check_bundle.patch',
    needsNodeModules: true,
    run(wt) {
      const buildRes = runShell('npm run build', wt);
      if (buildRes.status !== 0) {
        throw new Error(
          `Prerequisite npm run build failed for check_bundle (exit ${buildRes.status}):\n${buildRes.output.slice(-2000)}`,
        );
      }
      return runShell('npm run check:bundle', wt);
    },
    matchers: [/^- .*statically loads the three\.js chunk/, /^- .*(?:budget|KiB|exceeds|three)/i, /^- /, /bundle budget FAILED/],
  },
  {
    id: 'check_e2e_specs',
    source: 'ci.yml:405',
    ciCommand: 'node scripts/check_e2e_specs.mjs',
    faultSummary: 'Narrow `tests/package.json` `test:e2e` script to `e2e/console_hygiene.spec.ts`',
    patch: 'check_e2e_specs.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('node scripts/check_e2e_specs.mjs', wt);
    },
    matchers: [/missing from|policy|scheduled/i, /tier1_features|rw_budget/i],
  },
  {
    id: 'playwright_e2e',
    source: 'ci.yml:409',
    ciCommand: 'npm run test:e2e',
    faultSummary: 'Change `expect(owned.length).toBe(0)` to `.toBe(999)` in `tests/e2e/console_hygiene.spec.ts`',
    patch: 'playwright_e2e.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run test:e2e', wt);
    },
    matchers: [/Anima-owned console output/, /console_hygiene\.spec\.ts/, /Expected: 999/],
  },
  {
    id: 'check_docs_links',
    source: 'ci.yml:421',
    ciCommand: 'node scripts/check_docs_links.mjs',
    faultSummary: 'Append broken link `[m2-broken](./m2-missing-target.md)` to `docs/README.md`',
    patch: 'check_docs_links.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('node scripts/check_docs_links.mjs', wt);
    },
    matchers: [/m2-missing-target\.md/, /broken link/i],
  },
  {
    id: 'check_docs_metadata',
    source: 'ci.yml:427',
    ciCommand: 'node scripts/check_docs_metadata.mjs',
    faultSummary: 'Set `status: invalid-m2` in `docs/README.md` front matter',
    patch: 'check_docs_metadata.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('node scripts/check_docs_metadata.mjs', wt);
    },
    matchers: [/invalid-m2/, /docs\/README\.md/],
  },
  {
    id: 'check_ipc_reference',
    source: 'ci.yml:432',
    ciCommand: 'node scripts/check_ipc_reference.mjs',
    faultSummary: 'Insert nonexistent `m2_ghost_cmd` row in `docs/reference/IPC_CONTRACT.md`',
    patch: 'check_ipc_reference.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('node scripts/check_ipc_reference.mjs', wt);
    },
    matchers: [/m2_ghost_cmd/],
  },
  {
    id: 'check_ipc_arg_case',
    source: 'package.json:33',
    ciCommand: 'npm run check:ipc',
    faultSummary:
      'Add `invoke("save_simulation_state", { file_path })` (snake_case) helper in `src/components/Landscape/utils/worldCache.ts`',
    patch: 'check_ipc_arg_case.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:ipc', wt);
    },
    matchers: [/file_path/, /worldCache\.ts/],
  },
  {
    id: 'check_map_vision_manifest',
    source: 'package.json:23 (ci.yml:330)',
    ciCommand: 'npm run check:map-vision-manifest',
    faultSummary: 'Set `seaLevel: 0.99` in `animal-map.manifest.json`',
    patch: 'check_map_vision_manifest.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:map-vision-manifest', wt);
    },
    matchers: [/seaLevel|animal-map\.manifest\.json|differs|stale|mismatch|out of date|Regenerate/i],
  },
  {
    id: 'verify_upstream_licenses',
    source: 'package.json:47',
    ciCommand: 'npm run verify:upstream-licenses',
    faultSummary: 'Flip 1 hex char in `ts-rs` `sha256` inside `licensing/upstream/sources.json`',
    patch: 'verify_upstream_licenses.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run verify:upstream-licenses', wt);
    },
    matchers: [/manifest says/, /ts-rs/, /sha256 is/],
  },
  {
    id: 'compare_checksums_self_test',
    source: 'determinism-matrix.yml:113',
    ciCommand: 'node scripts/paper/compare_checksums.mjs --self-test',
    faultSummary: 'Force `checksumAgrees = true` in `scripts/paper/compare_checksums.mjs`',
    patch: 'compare_checksums_self_test.patch',
    needsNodeModules: false,
    run(wt) {
      return runShell('node scripts/paper/compare_checksums.mjs --self-test', wt);
    },
    matchers: [/FAIL\s+a checksum difference/i, /self-test: \d+ control\(s\) failed/],
  },
];

function evaluateGate(gate, ctx) {
  const t0 = performance.now();
  if (gate.staticResult) {
    return {
      id: gate.id,
      source: gate.source,
      ciCommand: gate.ciCommand,
      fault: gate.faultSummary,
      exitCode: gate.staticResult.exitCode,
      namingLine: gate.staticResult.namingLine,
      verdict: gate.staticResult.verdict,
      elapsedSecs: 0,
    };
  }

  const res = withTempWorktree(
    gate.patch,
    {
      needsNodeModules: Boolean(gate.needsNodeModules),
      needsCargo: Boolean(gate.needsCargo),
    },
    (wt, tmpBase) => gate.run(wt, tmpBase, ctx),
  );
  const elapsedSecs = Math.round(((performance.now() - t0) / 1000) * 100) / 100;
  const verdict = res.status === 0 ? 'VACUOUS' : 'CATCHES';
  const namingLine = pickLine(res.output, gate.matchers);
  return {
    id: gate.id,
    source: gate.source,
    ciCommand: gate.ciCommand,
    fault: gate.faultSummary,
    exitCode: res.status,
    namingLine,
    verdict,
    elapsedSecs,
  };
}

function main() {
  const { gateIds, listOnly, jsonOut } = parseCliArgs(process.argv.slice(2));
  if (listOnly) {
    for (const gate of GATES) {
      console.log(`${gate.id}\t(${gate.source})`);
    }
    return;
  }

  const selected =
    gateIds.length > 0
      ? gateIds.map((id) => {
          const found = GATES.find((g) => g.id === id);
          if (!found) {
            console.error(`Unknown gate id: ${id}`);
            process.exit(2);
          }
          return found;
        })
      : GATES;

  const statusBefore = execFileSync('git', ['status', '--porcelain'], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  });

  const ctx = {
    unscheduledCapturePath: null,
    tempFilesToClean: [],
  };
  const results = [];
  try {
    for (const gate of selected) {
      console.error(`[m2] running ${gate.id}...`);
      const record = evaluateGate(gate, ctx);
      results.push(record);
      console.error(
        `[m2] ${record.id}: verdict=${record.verdict} exit=${record.exitCode} elapsed=${record.elapsedSecs}s | ${record.namingLine}`,
      );
    }
  } finally {
    for (const tempFile of ctx.tempFilesToClean) {
      rmSync(tempFile, { force: true });
    }
  }

  const statusAfter = execFileSync('git', ['status', '--porcelain'], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  });
  if (statusAfter !== statusBefore) {
    console.error('ERROR: repository working tree status changed during gate_controls run!');
    console.error(`Before:\n${statusBefore}\nAfter:\n${statusAfter}`);
    process.exit(2);
  }

  // Sort VACUOUS first, then NOT RUN, then CATCHES
  const rank = (verdict) => {
    if (verdict === 'VACUOUS') return 0;
    if (verdict.startsWith('NOT RUN')) return 1;
    return 2;
  };
  const ordered = [...results].sort((a, b) => rank(a.verdict) - rank(b.verdict));

  const counts = {
    total: ordered.length,
    vacuous: ordered.filter((r) => r.verdict === 'VACUOUS').length,
    catches: ordered.filter((r) => r.verdict === 'CATCHES').length,
    notRunNoGate: ordered.filter((r) => r.verdict === 'NOT RUN (no automated gate)').length,
    notRunTime: ordered.filter((r) => r.verdict === 'NOT RUN (time)').length,
    notRunBaselineRed: ordered.filter((r) => r.verdict.startsWith('NOT RUN (baseline red')).length,
  };

  console.log(
    `SUMMARY total=${counts.total} VACUOUS=${counts.vacuous} CATCHES=${counts.catches} ` +
      `NOT_RUN_NO_GATE=${counts.notRunNoGate} NOT_RUN_TIME=${counts.notRunTime} ` +
      `NOT_RUN_BASELINE_RED=${counts.notRunBaselineRed}`,
  );
  console.log('| gate | CI command | fault | exit code | naming line (verbatim) | verdict |');
  console.log('|---|---|---|---|---|---|');
  for (const row of ordered) {
    console.log(
      `| ${formatMdCode(row.id)} | ${formatMdCode(row.ciCommand)} | ${sanitizeMdCell(row.fault)} | ${row.exitCode} | ${formatMdCode(row.namingLine)} | **${row.verdict}** |`,
    );
  }

  if (jsonOut) {
    writeFileSync(jsonOut, JSON.stringify({ counts, results: ordered }, null, 2), 'utf8');
  }
}

main();
