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
//   node scripts/paper/gate_controls.mjs --self-test
//   node scripts/paper/gate_controls.mjs --json-out /abs/path/out.json
//   node scripts/paper/gate_controls.mjs --out /abs/path/out.md

import { execFileSync, spawnSync } from 'node:child_process';
import {
  closeSync,
  existsSync,
  lstatSync,
  mkdtempSync,
  openSync,
  readFileSync,
  readdirSync,
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
export const REPO_ROOT = resolve(__dirname, '..', '..');
export const FAULTS_DIR = join(__dirname, 'gate_faults');

export const VERDICTS = Object.freeze({
  CATCHES: 'CATCHES',
  VACUOUS: 'VACUOUS',
  UNMATCHED_FAILURE: 'UNMATCHED_FAILURE',
  NOT_RUN_BASELINE_RED: 'NOT RUN (baseline red)',
  NOT_RUN_NO_GATE: 'NOT RUN (no automated gate)',
  NOT_RUN_TIME: 'NOT RUN (time)',
  NOT_RUN_WINDOWS: 'NOT RUN (not measured on Windows)',
});

export function parseCliArgs(argv) {
  const gateIds = [];
  let listOnly = false;
  let selfTest = false;
  let jsonOut = null;
  let mdOut = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--list') {
      listOnly = true;
    } else if (arg === '--self-test') {
      selfTest = true;
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
    } else if (arg === '--out' && i + 1 < argv.length) {
      i += 1;
      mdOut = resolve(argv[i]);
    } else {
      throw new Error(`Unknown or incomplete option: ${arg}`);
    }
  }
  return { gateIds, listOnly, selfTest, jsonOut, mdOut };
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

function readWindowsCargoGuardState(targetDriveLetter) {
  const drive = /^[A-Z]$/i.test(targetDriveLetter) ? targetDriveLetter.toUpperCase() : 'E';
  const psScript = [
    '$os = Get-CimInstance Win32_OperatingSystem;',
    '$freeKb = [int64]$os.FreePhysicalMemory;',
    '$virtKb = [int64]$os.FreeVirtualMemory;',
    `$drv = [System.IO.DriveInfo]::new('${drive}');`,
    '$driveFreeBytes = [int64]$drv.AvailableFreeSpace;',
    '$procs = @(Get-CimInstance Win32_Process | Where-Object { $_.Name -match "cargo|rustc" });',
    'Write-Output ("FREE_KB=" + $freeKb + ";VIRT_KB=" + $virtKb + ";DRIVE_FREE=" + $driveFreeBytes + ";PROCS=" + $procs.Count);',
    'foreach ($p in $procs) { Write-Output ("PROC=" + $p.ProcessId + " " + $p.Name + " " + $p.CommandLine) }',
  ].join(' ');
  const res = spawnSync('powershell.exe', ['-NoProfile', '-Command', psScript], {
    encoding: 'utf8',
  });
  const out = res.stdout || '';
  const m = /FREE_KB=(\d+);VIRT_KB=(\d+);DRIVE_FREE=(\d+);PROCS=(\d+)/.exec(out);
  if (!m) {
    return { ok: false, raw: `${out}\n${res.stderr || ''}` };
  }
  return {
    ok: true,
    freeKb: Number(m[1]),
    virtKb: Number(m[2]),
    driveFreeBytes: Number(m[3]),
    procs: Number(m[4]),
    raw: out,
  };
}

function sleepMs(ms) {
  if (ms > 0) {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
  }
}

function ensureSafeAnimaCargo() {
  if (process.platform !== 'win32') return;
  try {
    os.setPriority(process.pid, os.constants.priority.PRIORITY_LOW);
  } catch {
    // Best-effort low priority so concurrent host tasks keep headroom.
  }
  const targetDir = cargoEnv().CARGO_TARGET_DIR;
  const driveMatch = /^([A-Za-z]):/.exec(targetDir);
  const driveLetter = driveMatch ? driveMatch[1].toUpperCase() : 'E';
  const minFreeKb = 5 * 1024 * 1024; // 5 GiB physical RAM (AGENT_RULES §5, 2026-11-20)
  const minDriveBytes = 20 * 1024 * 1024 * 1024; // 20 GiB free on target drive
  const quietSecs = Number(
    process.env.ANIMA_GATE_QUIET_SECS ?? (os.totalmem() < 24 * 1024 * 1024 * 1024 ? '60' : '0'),
  );
  const quietMs = Number.isFinite(quietSecs) && quietSecs > 0 ? Math.round(quietSecs * 1000) : 0;

  let lastState = null;
  for (let attempt = 0; attempt < 240; attempt += 1) {
    const first = readWindowsCargoGuardState(driveLetter);
    lastState = first;
    if (!first.ok) {
      if (attempt === 239) {
        throw new Error(`Could not inspect Windows cargo/RAM state:\n${first.raw}`);
      }
      sleepMs(5000);
      continue;
    }
    const firstReady =
      first.procs === 0 && first.freeKb >= minFreeKb && first.driveFreeBytes >= minDriveBytes;
    if (!firstReady) {
      console.error(
        `[m2] cargo guard waiting (check #1: procs=${first.procs}, freeRAM=${(first.freeKb / (1024 * 1024)).toFixed(2)}GiB, ${driveLetter}:=${(first.driveFreeBytes / (1024 * 1024 * 1024)).toFixed(2)}GiB)...`,
      );
      sleepMs(30_000);
      continue;
    }
    if (quietMs > 0) {
      sleepMs(quietMs);
      const second = readWindowsCargoGuardState(driveLetter);
      lastState = second;
      if (!second.ok) {
        sleepMs(5000);
        continue;
      }
      const secondReady =
        second.procs === 0 && second.freeKb >= minFreeKb && second.driveFreeBytes >= minDriveBytes;
      if (!secondReady) {
        console.error(
          `[m2] cargo guard waiting (check #2 after ${quietSecs}s: procs=${second.procs}, freeRAM=${(second.freeKb / (1024 * 1024)).toFixed(2)}GiB, ${driveLetter}:=${(second.driveFreeBytes / (1024 * 1024 * 1024)).toFixed(2)}GiB)...`,
        );
        sleepMs(30_000);
        continue;
      }
    }
    return;
  }
  throw new Error(
    `Dell no-parallel-cargo guard timed out (${JSON.stringify(lastState)}):\n${lastState?.raw || ''}`,
  );
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

export function removeLinkOrDir(linkPath) {
  const st = lstatSync(linkPath, { throwIfNoEntry: false });
  if (!st) return;
  if (st.isSymbolicLink()) {
    unlinkSync(linkPath);
  } else {
    rmSync(linkPath, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
}

export function cleanupStaleWorktree(wt) {
  for (const rel of ['node_modules', join('tests', 'node_modules')]) {
    removeLinkOrDir(join(wt, rel));
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
  rmSync(wt, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}

function linkNodeModules(wt, createdLinks) {
  const linkType = process.platform === 'win32' ? 'junction' : 'dir';
  const pairs = [
    [join(REPO_ROOT, 'node_modules'), join(wt, 'node_modules')],
    [join(REPO_ROOT, 'tests', 'node_modules'), join(wt, 'tests', 'node_modules')],
  ];
  for (const [src, dst] of pairs) {
    if (existsSync(src) && !lstatSync(dst, { throwIfNoEntry: false })) {
      symlinkSync(src, dst, linkType);
      createdLinks.push(dst);
    }
  }
}

export function unlinkNodeModules(createdLinks) {
  for (let i = createdLinks.length - 1; i >= 0; i -= 1) {
    const linkPath = createdLinks[i];
    removeLinkOrDir(linkPath);
    if (lstatSync(linkPath, { throwIfNoEntry: false })) {
      throw new Error(`Refusing to continue: junction/symlink still present at ${linkPath}`);
    }
  }
}

function withTempWorktree(patchFile, { needsNodeModules = false, needsCargo = false }, fn) {
  if (needsCargo) ensureSafeAnimaCargo();

  const tmpBase = mkdtempSync(join(os.tmpdir(), 'anima-m2-'));
  const wt = join(tmpBase, 'wt');
  const createdLinks = [];

  try {
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
    rmSync(tmpBase, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
    try {
      execFileSync('git', ['worktree', 'prune'], { cwd: REPO_ROOT, stdio: 'pipe' });
    } catch {
      // Best-effort prune
    }
  }
}

const ANSI_ESCAPE_RE = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, 'g');

export function normalizeOutputLines(output, failed = false) {
  const cleanText = String(output ?? '').replace(ANSI_ESCAPE_RE, '');
  const raw = cleanText
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  const lines = raw
    .filter((line) => !line.startsWith('>') && !line.startsWith('npm error'))
    .filter(
      (line) =>
        !failed ||
        !/^(?:test .+ \.\.\. ok|Running |Compiling |Checking |Finished |Blocking waiting for file lock)/.test(
          line,
        ),
    );
  return { lines, raw };
}

export function findMatchingLine(output, matchers = [], failed = false) {
  const { lines, raw } = normalizeOutputLines(output, failed);
  for (let i = 0; i < matchers.length; i += 1) {
    const matcher = matchers[i];
    const found = lines.find((line) => matcher.test(line));
    if (found) {
      return { matched: true, line: found, matcherIndex: i };
    }
  }
  const fallbackError =
    failed && lines.length > 0
      ? lines.find((line) => /^(?:Error:|error(?:\[[^\]]+\])?:|FAIL\b|Diff in\b|x\s+)/.test(line)) ||
        lines.find((line) => /^-\s\S/.test(line))
      : null;
  return {
    matched: false,
    line: fallbackError || lines[0] || raw[0] || '(no output)',
    matcherIndex: -1,
  };
}

export function pickLine(output, matchers = [], failed = false) {
  return findMatchingLine(output, matchers, failed).line;
}

export function classifyGateResult({ baselineRes, faultRes, matchers = [] }) {
  if (!baselineRes || baselineRes.status !== 0) {
    const exitCode = baselineRes && typeof baselineRes.status === 'number' ? baselineRes.status : 1;
    const picked = findMatchingLine(baselineRes?.output ?? '', [], true);
    return {
      verdict: VERDICTS.NOT_RUN_BASELINE_RED,
      exitCode,
      namingLine: picked.line,
      matched: false,
    };
  }

  if (!faultRes || faultRes.status === 0) {
    const exitCode = faultRes && typeof faultRes.status === 'number' ? faultRes.status : 0;
    const picked = findMatchingLine(faultRes?.output ?? '', matchers, false);
    return {
      verdict: VERDICTS.VACUOUS,
      exitCode,
      namingLine: picked.line,
      matched: picked.matched,
    };
  }

  const picked = findMatchingLine(faultRes.output ?? '', matchers, true);
  if (picked.matched) {
    const baselineLeak = findMatchingLine(baselineRes.output ?? '', matchers, false);
    if (baselineLeak.matched) {
      return {
        verdict: VERDICTS.UNMATCHED_FAILURE,
        exitCode: faultRes.status,
        namingLine: `matcher matched clean baseline output: ${baselineLeak.line}`,
        matched: false,
      };
    }
    return {
      verdict: VERDICTS.CATCHES,
      exitCode: faultRes.status,
      namingLine: picked.line,
      matched: true,
    };
  }

  return {
    verdict: VERDICTS.UNMATCHED_FAILURE,
    exitCode: faultRes.status,
    namingLine: picked.line,
    matched: false,
  };
}

export function sanitizeMdCell(text) {
  return String(text).replace(/\|/g, '\\|').replace(/\s+/g, ' ').trim();
}

export function formatMdCode(text) {
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

function unquoteYamlScalar(value) {
  const s = value.trim();
  if (
    s.length >= 2 &&
    ((s.startsWith('"') && s.endsWith('"') && !s.slice(1, -1).includes('"')) ||
      (s.startsWith("'") && s.endsWith("'") && !s.slice(1, -1).includes("'")))
  ) {
    return s.slice(1, -1);
  }
  return s;
}

export function extractWorkflowSteps(yamlText) {
  const lines = String(yamlText ?? '').split(/\r?\n/);
  const steps = [];
  let inJobs = false;
  let currentJob = null;
  let inSteps = false;
  let stepIndent = -1;
  let currentStep = null;
  let readingBlock = false;
  let blockKeyIndent = -1;
  let blockContentIndent = -1;
  let blockLines = [];
  let runStartLine = 0;

  const finishBlock = () => {
    if (!readingBlock || !currentStep) return;
    while (blockLines.length > 0 && blockLines[blockLines.length - 1] === '') {
      blockLines.pop();
    }
    const nonEmpty = blockLines.filter((l) => l.trim().length > 0);
    const minIndent =
      blockContentIndent >= 0
        ? blockContentIndent
        : nonEmpty.length > 0
          ? Math.min(...nonEmpty.map((l) => l.match(/^\s*/)[0].length))
          : 0;
    currentStep.run = blockLines.map((l) => (l.length >= minIndent ? l.slice(minIndent) : '')).join('\n');
    currentStep.runLine = runStartLine;
    readingBlock = false;
    blockKeyIndent = -1;
    blockContentIndent = -1;
    blockLines = [];
  };

  const finishStep = () => {
    finishBlock();
    if (currentStep) {
      steps.push(currentStep);
      currentStep = null;
    }
  };

  for (let idx = 0; idx < lines.length; idx += 1) {
    const rawLine = lines[idx];
    const lineNo = idx + 1;
    const trimmed = rawLine.trim();
    const indent = rawLine.match(/^\s*/)[0].length;

    if (readingBlock) {
      if (trimmed.length === 0) {
        blockLines.push('');
        continue;
      }
      if (indent > blockKeyIndent) {
        if (blockContentIndent < 0) blockContentIndent = indent;
        blockLines.push(rawLine);
        continue;
      }
      finishBlock();
    }

    if (trimmed.length === 0 || trimmed.startsWith('#')) {
      continue;
    }

    if (indent === 0) {
      finishStep();
      inJobs = /^jobs:\s*(?:#.*)?$/.test(trimmed);
      currentJob = null;
      inSteps = false;
      continue;
    }

    if (!inJobs) continue;

    if (indent === 2 && /^[A-Za-z0-9_-]+:\s*(?:#.*)?$/.test(trimmed)) {
      finishStep();
      currentJob = trimmed.split(':')[0];
      inSteps = false;
      continue;
    }

    if (currentJob && indent === 4) {
      finishStep();
      inSteps = /^steps:\s*(?:#.*)?$/.test(trimmed);
      continue;
    }

    if (!inSteps) continue;

    const dashMatch = /^(\s*)-\s+(.*)$/.exec(rawLine);
    if (dashMatch && (stepIndent < 0 || dashMatch[1].length <= stepIndent)) {
      finishStep();
      stepIndent = dashMatch[1].length;
      currentStep = {
        job: currentJob,
        name: null,
        workingDirectory: null,
        run: null,
        startLine: lineNo,
        runLine: lineNo,
      };
      const kv = /^([A-Za-z0-9_-]+):\s*(.*)$/.exec(dashMatch[2]);
      if (kv) {
        const [, key, val] = kv;
        if (key === 'name') currentStep.name = unquoteYamlScalar(val);
        else if (key === 'working-directory') currentStep.workingDirectory = unquoteYamlScalar(val);
        else if (key === 'run') {
          if (/^\|[+-]?$/.test(val.trim())) {
            readingBlock = true;
            blockKeyIndent = stepIndent;
            runStartLine = lineNo + 1;
          } else {
            currentStep.run = val.trim();
            currentStep.runLine = lineNo;
          }
        }
      }
      continue;
    }

    if (currentStep && indent === stepIndent + 2) {
      const kv = /^([A-Za-z0-9_-]+):\s*(.*)$/.exec(trimmed);
      if (!kv) continue;
      const [, key, val] = kv;
      if (key === 'name') currentStep.name = unquoteYamlScalar(val);
      else if (key === 'working-directory') currentStep.workingDirectory = unquoteYamlScalar(val);
      else if (key === 'run') {
        if (/^\|[+-]?$/.test(val.trim())) {
          readingBlock = true;
          blockKeyIndent = indent;
          runStartLine = lineNo + 1;
        } else {
          currentStep.run = val.trim();
          currentStep.runLine = lineNo;
        }
      }
    }
  }
  finishStep();
  return steps;
}

export function getWorkflowStep(wt, relPath, jobId, stepName) {
  const fullPath = join(wt, relPath);
  const steps = extractWorkflowSteps(readFileSync(fullPath, 'utf8'));
  const found = steps.find((s) => s.job === jobId && s.name === stepName && typeof s.run === 'string');
  if (!found) {
    throw new Error(`Could not locate workflow step "${stepName}" in ${relPath} (job: ${jobId})`);
  }
  return found;
}

export function formatWorkflowCommandDisplay(runText) {
  const joinedContinuations = String(runText ?? '').replace(/\\\r?\n\s*/g, ' ');
  const lines = joinedContinuations
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0 && !l.startsWith('#') && l !== 'set -o pipefail');
  let out = '';
  for (const line of lines) {
    if (!out) {
      out = line;
    } else if (
      /(?:then|do|else|elif|\|\||&&|\||\{|\()\s*$/.test(out) ||
      /^(?:then|do|else|elif|fi|done|\||&&|\|\||\})/.test(line)
    ) {
      out += ` ${line}`;
    } else {
      out += `; ${line}`;
    }
  }
  return out;
}

function getWorkflowCommandLine(wt, relPath, jobId, stepName) {
  const step = getWorkflowStep(wt, relPath, jobId, stepName);
  const shortFile = relPath.split('/').pop();
  return {
    source: `${shortFile}:${step.runLine}`,
    command: formatWorkflowCommandDisplay(step.run),
    raw: step.run,
  };
}

function getFeatureSeparationSlice(wt, sliceKind) {
  const step = getWorkflowStep(
    wt,
    '.github/workflows/ci.yml',
    'rust',
    'Default build excludes Neo4j, the WebSocket stack, Burn/WGPU and anima-worldgen',
  );
  const lines = step.run.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
  const idxEngine = lines.findIndex((l) => l.startsWith('engine_leaked='));
  const idxWorldgen = lines.findIndex((l) => l.startsWith('worldgen_leaked='));
  if (idxEngine < 0 || idxWorldgen < 0) {
    throw new Error('Could not parse feature separation sub-checks from ci.yml');
  }
  const picked =
    sliceKind === 'default'
      ? lines.slice(0, idxEngine)
      : sliceKind === 'engine'
        ? lines.slice(idxEngine, idxWorldgen)
        : lines.slice(idxWorldgen);
  return {
    source: `ci.yml:${step.runLine}`,
    raw: picked.join('\n'),
    command: picked.join(' && '),
  };
}

function getFrontendStepsMatching(wt, pattern) {
  const steps = extractWorkflowSteps(readFileSync(join(wt, '.github/workflows/ci.yml'), 'utf8'));
  return steps.filter((s) => s.job === 'frontend' && typeof s.run === 'string' && pattern.test(s.run));
}

function getFrontendChainCommand(wt, pattern) {
  const matched = getFrontendStepsMatching(wt, pattern);
  if (matched.length === 0) {
    throw new Error(`No matching frontend workflow steps found for ${pattern}`);
  }
  return matched.map((s) => formatWorkflowCommandDisplay(s.run)).join(' && ');
}

function extractProfileFromStep(wt, stepName) {
  const step = getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', stepName);
  const match = step.run.match(/--profile(?:=|\s+)([^\s]+)/);
  return match ? match[1] : null;
}

const POLICY_TEST_TARGETS = [
  'shared_domain_law_tests',
  'determinism_gate_tests',
  'energy_conservation_tests',
  'feature_gated_targets_tests',
  'adversarial_challenger_tests',
  'migration_tests',
  'migration_high_throughput_tests',
  'migration_narrow_bounds_loop_tests',
  'migration_panic_tests',
  'migration_stress_tests',
  'phase5_websocket_reuse',
  'phase5_burn_wgpu_fallback',
];

function extractCiCargoTestFeatures(wt) {
  const step = getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'cargo test');
  const cargoLine = step.run.split(/\r?\n/).find((l) => /^\s*cargo test\b/.test(l));
  if (!cargoLine) {
    throw new Error('Could not locate `cargo test` line in .github/workflows/ci.yml rust job');
  }
  const cargoSide = cargoLine.split('|')[0].split(/\s+--\s+/)[0];
  const featMatch = cargoSide.match(/--features(?:=|\s+)([^\s]+)/);
  if (!featMatch) return [];
  return featMatch[1]
    .split(',')
    .map((f) => f.trim())
    .filter(Boolean);
}

function buildCiCargoTestTargetCmd(wt, target) {
  const feats = extractCiCargoTestFeatures(wt);
  const featFlag = feats.length > 0 ? `--features ${feats.join(',')} ` : '';
  return `cargo test ${featFlag}--test ${target} -- --nocapture`;
}

function extractCiWorldgenFeatures(wt) {
  const step = getWorkflowStep(
    wt,
    '.github/workflows/ci.yml',
    'rust',
    'cargo test (anima-worldgen CPU reader/generate/bake)',
  );
  const cargoLine = step.run.split(/\r?\n/).find((l) => /^\s*cargo test\b/.test(l));
  if (!cargoLine) {
    throw new Error('Could not locate `cargo test -p anima-worldgen` line in .github/workflows/ci.yml rust job');
  }
  const cargoSide = cargoLine.split('|')[0].split(/\s+--\s+/)[0];
  const featMatch = cargoSide.match(/--features(?:=|\s+)(?:"([^"]+)"|'([^']+)'|([^\s]+))/);
  if (!featMatch) return [];
  const raw = featMatch[1] || featMatch[2] || featMatch[3] || '';
  return raw
    .split(',')
    .map((f) => f.trim())
    .filter(Boolean);
}

function runCheckTestTargetsFromCi(wt, tmpBase, ctx, profileArg, cacheKey = null) {
  if (cacheKey && ctx.captures[cacheKey] && existsSync(ctx.captures[cacheKey])) {
    return runCheckTestTargetsScript(wt, ctx.captures[cacheKey], profileArg);
  }

  const rawFeatures = extractCiCargoTestFeatures(wt);
  const tauriDir = join(wt, 'src-tauri');
  const env = { ...cargoEnv(), ANIMA_ENERGY_GATE_TICKS: '1000' };

  const metaRes = spawnSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], {
    cwd: tauriDir,
    env,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  });
  if (metaRes.status !== 0) {
    return {
      status: metaRes.status ?? 1,
      stdout: metaRes.stdout || '',
      stderr: metaRes.stderr || '',
      output: `${metaRes.stdout || ''}\n${metaRes.stderr || ''}`,
    };
  }

  const meta = JSON.parse(metaRes.stdout);
  const pkg = meta.packages.find((p) => p.name === 'anima-engine');
  if (!pkg) {
    return {
      status: 1,
      stdout: '',
      stderr: 'Could not find package `anima-engine` in cargo metadata output',
      output: 'Could not find package `anima-engine` in cargo metadata output',
    };
  }

  const activeFeatures = new Set();
  const queue = [...rawFeatures];
  while (queue.length > 0) {
    const feat = queue.pop();
    if (!feat || activeFeatures.has(feat)) continue;
    activeFeatures.add(feat);
    for (const dep of pkg.features?.[feat] || []) {
      if (!dep.startsWith('dep:') && !dep.includes('/')) {
        queue.push(dep);
      }
    }
  }

  const featureArgs =
    rawFeatures.length > 0 ? ['--features', rawFeatures.join(',')] : ['--no-default-features'];

  const scheduledTargets = POLICY_TEST_TARGETS.filter((name) => {
    const target = pkg.targets.find(
      (t) => Array.isArray(t.kind) && t.kind.includes('test') && t.name === name,
    );
    if (!target) {
      throw new Error(`Expected policy test target \`${name}\` missing from cargo metadata`);
    }
    const req = target['required-features'] || target.required_features || [];
    return req.every((f) => activeFeatures.has(f));
  });

  const testArgs = ['test', ...featureArgs, '--bin', 'anima-engine'];
  for (const targetName of scheduledTargets) {
    testArgs.push('--test', targetName);
  }
  testArgs.push('--', '--nocapture');

  const capturePath = cacheKey
    ? join(os.tmpdir(), `anima-m2-${cacheKey}-capture-${process.pid}.txt`)
    : join(tmpBase, 'cargo-test-output.txt');
  const docCapturePath = join(tmpBase, 'cargo-doc-output.txt');

  const testRes = runCommandToFile('cargo', testArgs, tauriDir, env, capturePath);
  if (testRes.status !== 0) {
    rmSync(capturePath, { force: true });
    return testRes;
  }

  ensureSafeAnimaCargo();
  const docRes = runCommandToFile(
    'cargo',
    ['test', '--workspace', '--doc', '--', '--nocapture'],
    tauriDir,
    env,
    docCapturePath,
  );
  if (docRes.status !== 0) {
    rmSync(capturePath, { force: true });
    rmSync(docCapturePath, { force: true });
    return docRes;
  }

  writeFileSync(capturePath, `${testRes.stdout}\n${docRes.stdout}\n`, 'utf8');
  rmSync(docCapturePath, { force: true });

  if (cacheKey) {
    ctx.captures[cacheKey] = capturePath;
    ctx.tempFilesToClean.push(capturePath);
  }
  return runCheckTestTargetsScript(wt, capturePath, profileArg);
}

function runWorldgenTestTargetsFromCi(wt, tmpBase, ctx, profileArg = 'worldgen-cpu', cacheKey = null) {
  if (cacheKey && ctx.captures[cacheKey] && existsSync(ctx.captures[cacheKey])) {
    return runCheckTestTargetsScript(wt, ctx.captures[cacheKey], profileArg);
  }

  const rawFeatures = extractCiWorldgenFeatures(wt);
  const tauriDir = join(wt, 'src-tauri');
  const env = cargoEnv();
  const featureArgs =
    rawFeatures.length > 0 ? ['--features', rawFeatures.join(',')] : ['--no-default-features'];

  const capturePath = cacheKey
    ? join(os.tmpdir(), `anima-m2-${cacheKey}-capture-${process.pid}.txt`)
    : join(tmpBase, 'cargo-test-worldgen-output.txt');

  const testRes = runCommandToFile(
    'cargo',
    ['test', '-p', 'anima-worldgen', ...featureArgs, '--', '--nocapture'],
    tauriDir,
    env,
    capturePath,
  );
  if (testRes.status !== 0) {
    rmSync(capturePath, { force: true });
    return testRes;
  }

  if (cacheKey) {
    ctx.captures[cacheKey] = capturePath;
    ctx.tempFilesToClean.push(capturePath);
  }
  return runCheckTestTargetsScript(wt, capturePath, profileArg);
}

export const GATES = [
  // ---- Expected VACUOUS / CI-omission controls (read dynamically from workflow files) -------
  {
    id: 'thread_rng_scan',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/hardening.yml', 'determinism', 'cấm thêm nguồn phi tất định vào src').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/hardening.yml', 'determinism', 'cấm thêm nguồn phi tất định vào src').command,
    faultSummary: 'Add `use rand::thread_rng; let _ = thread_rng();` to `src-tauri/src/main.rs`',
    patch: 'thread_rng_scan.patch',
    cleanKey: 'hardening:thread_rng_scan',
    needsCargo: false,
    run(wt) {
      const script = getWorkflowStep(wt, '.github/workflows/hardening.yml', 'determinism', 'cấm thêm nguồn phi tất định vào src').run;
      return runBash(script, wt);
    },
    matchers: [/::error::co [1-9]\d* cho goi thread_rng\(\) trong src/],
  },
  {
    id: 'check_test_targets_unscheduled',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No test target ran zero tests').source,
    ciCommand: `[harness-assembled: 12 policy targets] ${getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No test target ran zero tests').command}`,
    faultSummary: 'Drop `--features desktop` from `cargo test` in `.github/workflows/ci.yml:184`',
    patch: 'check_test_targets_unscheduled.patch',
    cleanKey: 'check_test_targets:from_ci',
    needsCargo: (ctx, isClean) => (isClean ? !ctx.captures.clean_ci_from_ci : !ctx.captures.unscheduled),
    runClean(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No test target ran zero tests');
      return runCheckTestTargetsFromCi(wt, tmpBase, ctx, profile, 'clean_ci_from_ci');
    },
    run(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No test target ran zero tests');
      return runCheckTestTargetsFromCi(wt, tmpBase, ctx, profile, 'unscheduled');
    },
    matchers: [/profile "desktop" enables "networking", so tests\/adversarial_challenger_tests\.rs must run/],
  },
  {
    id: 'ci_ipc_arg_case',
    source: 'ci.yml (frontend chain)',
    ciCommand: getFrontendChainCommand(
      REPO_ROOT,
      /(?:check:ipc|npm run lint|eslint_ratchet|typecheck:tests|npm run test(?:\s|$)|npm run test:frontend(?:\s|$)|npm run build|check_ipc_reference)/,
    ),
    faultSummary:
      'Add `invoke("save_simulation_state", { file_path })` (snake_case) helper in `src/components/Landscape/utils/worldCache.ts`',
    patch: 'check_ipc_arg_case.patch',
    needsNodeModules: true,
    run(wt) {
      const cmd = getFrontendChainCommand(
        wt,
        /(?:check:ipc|npm run lint|eslint_ratchet|typecheck:tests|npm run test(?:\s|$)|npm run test:frontend(?:\s|$)|npm run build|check_ipc_reference)/,
      );
      return runShell(cmd, wt, { ...process.env, CI: 'true' });
    },
    matchers: [/FAIL\s+src\/components\/Landscape\/utils\/worldCache\.ts:\d+ invoke\('save_simulation_state', \{ file_path/],
  },
  {
    id: 'ci_licenses_complete',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'Third-party licence texts are current and complete').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'Third-party licence texts are current and complete').command,
    faultSummary:
      'Remove `utf8-chars@3.0.7` election from `licensing/upstream/sources.json` and regenerate `licensing/` (`ci.yml` step "current and complete" omits `--require-complete`)',
    patch: 'check_licenses_complete.patch',
    cleanKey: 'npm run check:licenses',
    windowsOnly: true,
    needsCargo: true,
    needsNodeModules: true,
    runClean(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'Third-party licence texts are current and complete').run,
        wt,
        cargoEnv(),
      );
    },
    run(wt) {
      const env = cargoEnv();
      const genRes = runShell('node scripts/gen_third_party_licenses.mjs', wt, env);
      if (genRes.status !== 0) {
        return { status: genRes.status, output: genRes.output };
      }
      ensureSafeAnimaCargo();
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'Third-party licence texts are current and complete').run,
        wt,
        env,
      );
    },
    matchers: [/third-party licences check: up to date.*\b[1-9]\d* unresolved/i],
  },
  {
    id: 'ci_e3',
    source: 'ci.yml (frontend E2/E3/Vitest chain)',
    ciCommand: getFrontendChainCommand(REPO_ROOT, /(?:check:e2|check:e3|npm run test(?:\s|$)|npm run test:frontend(?:\s|$))/),
    faultSummary:
      'Alter `residual` for seed `731001` in `artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/effects.json`',
    patch: 'check_e3.patch',
    needsNodeModules: true,
    run(wt) {
      const cmd = getFrontendChainCommand(wt, /(?:check:e2|check:e3|npm run test(?:\s|$)|npm run test:frontend(?:\s|$))/);
      return runShell(cmd, wt, { ...process.env, CI: 'true' });
    },
    matchers: [/SHA-256 mismatch for effects\.json/],
  },

  // ---- Missing / >30 min gates -------------------------------------------------------------
  {
    id: 'conflict_marker_scan',
    source: 'CARD M2 (planned, unbuilt on trunk)',
    ciCommand: '(no automated check in ci.yml, hardening.yml, or package.json)',
    faultSummary: 'Conflict-marker scan planned in CARD M2 without implementation',
    patch: null,
    staticResult: {
      exitCode: '—',
      namingLine: 'NOT RUN (no automated gate in ci.yml, hardening.yml, or package.json)',
      verdict: VERDICTS.NOT_RUN_NO_GATE,
    },
  },
  {
    id: 'check_licenses_complete',
    source: 'package.json:43',
    ciCommand: 'npm run check:licenses-complete',
    faultSummary:
      'Remove `utf8-chars@3.0.7` election from `licensing/upstream/sources.json` and regenerate `licensing/`',
    patch: 'check_licenses_complete.patch',
    cleanKey: 'npm run check:licenses-complete',
    windowsOnly: true,
    needsCargo: true,
    needsNodeModules: true,
    runClean(wt) {
      return runShell('npm run check:licenses-complete', wt, cargoEnv());
    },
    run(wt) {
      const env = cargoEnv();
      const genRes = runShell('node scripts/gen_third_party_licenses.mjs', wt, env);
      if (genRes.status !== 0) {
        return { status: genRes.status, output: genRes.output };
      }
      ensureSafeAnimaCargo();
      return runShell('npm run check:licenses-complete', wt, env);
    },
    matchers: [/distributed component\(s\) have no obtainable licence text/i],
  },

  // ---- Gates > 30 min per fault (runtime measured once on clean tree) -----------------------
  {
    id: 'cargo_test_desktop_full',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo test').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo test').command,
    faultSummary: 'Full all-target desktop test run per fault exceeds 30 min budget (targeted suites below cover faults)',
    patch: null,
    staticResult: {
      exitCode: '—',
      namingLine:
        'NOT RUN (time: quiet-machine clean compile took 30m 13s and partial run took 38m 08s [2288.12s] across 52 targets before environmental_elements_stress_tests hit 65.1366ms vs <50ms debug timing limit under Idle priority; >30 min per fault)',
      verdict: VERDICTS.NOT_RUN_TIME,
    },
  },
  {
    id: 'hardening_stability_5x',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/hardening.yml', 'determinism', 'kiểm ổn định (5 lần)').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/hardening.yml', 'determinism', 'kiểm ổn định (5 lần)').command,
    faultSummary: '5x sequential full release test runs per fault exceed 30 min budget',
    patch: null,
    staticResult: {
      exitCode: '—',
      namingLine:
        'NOT RUN (time: single quiet-machine cargo test --release --quiet took 56m 38s [3398.22s; 159 targets, 1583 passed, 2 ignored, test_exec=264.87s]; 5x implies ~74m+ [compile + 5*test_exec] or ~283m cold, >30 min per fault)',
      verdict: VERDICTS.NOT_RUN_TIME,
    },
  },

  // ---- Rust CI & hardening gates ------------------------------------------------------------
  {
    id: 'thread_rng_scan_crates',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/hardening.yml', 'determinism', 'cấm thêm nguồn phi tất định vào src').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/hardening.yml', 'determinism', 'cấm thêm nguồn phi tất định vào src').command,
    faultSummary: 'Add `let _ = rand::thread_rng();` to `src-tauri/crates/anima-worldgen/src/lib.rs`',
    patch: 'thread_rng_scan_crates.patch',
    cleanKey: 'hardening:thread_rng_scan',
    needsCargo: false,
    run(wt) {
      const script = getWorkflowStep(wt, '.github/workflows/hardening.yml', 'determinism', 'cấm thêm nguồn phi tất định vào src').run;
      return runBash(script, wt);
    },
    matchers: [/::error::co [1-9]\d* cho goi thread_rng\(\) trong src/],
  },
  {
    id: 'cargo_fmt',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo fmt --check').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo fmt --check').command,
    faultSummary: 'Misformat `anima_engine_lib::run( );` in `src-tauri/src/main.rs`',
    patch: 'cargo_fmt.patch',
    needsCargo: true,
    run(wt) {
      return runShell(getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'cargo fmt --check').run, join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/Diff in .*main\.rs/i, /anima_engine_lib::run\(\s+\)/],
  },
  {
    id: 'cargo_audit',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo audit').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo audit').command,
    faultSummary: 'Remove `RUSTSEC-2026-0367` exemption from `src-tauri/.cargo/audit.toml`',
    patch: 'cargo_audit.patch',
    needsCargo: true,
    run(wt) {
      return runShell(getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'cargo audit').run, join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/ID:\s+RUSTSEC-2026-0367/],
  },
  {
    id: 'cargo_deny',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo deny check').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo deny check').command,
    faultSummary: 'Drop `"MIT"` from `licenses.allow` in `src-tauri/deny.toml`',
    patch: 'cargo_deny.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'cargo deny check').run,
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/├ MIT - MIT License:/],
  },
  {
    id: 'cargo_clippy_desktop',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo clippy --all-targets -D warnings').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo clippy --all-targets -D warnings').command,
    faultSummary: 'Add `let unused_m2_fault = 1usize;` in `src-tauri/src/main.rs`',
    patch: 'cargo_clippy_desktop.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'cargo clippy --all-targets -D warnings').run,
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/unused variable:\s*`unused_m2_fault`/],
  },
  {
    id: 'cargo_clippy_worldgen',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo clippy (anima-worldgen CPU reader/generate/bake)').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo clippy (anima-worldgen CPU reader/generate/bake)').command,
    faultSummary:
      'Call disallowed `elevation_m.sin()` in `src-tauri/crates/anima-worldgen/src/lib.rs` (deterministic math ban)',
    patch: 'cargo_clippy_worldgen.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'cargo clippy (anima-worldgen CPU reader/generate/bake)').run,
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/disallowed method `f32::sin`/],
  },
  {
    id: 'feature_separation',
    source: getFeatureSeparationSlice(REPO_ROOT, 'default').source,
    ciCommand: getFeatureSeparationSlice(REPO_ROOT, 'default').command,
    faultSummary: 'Make `tokio-tungstenite` non-optional in `src-tauri/Cargo.toml`',
    patch: 'feature_separation.patch',
    needsCargo: true,
    run(wt) {
      const slice = getFeatureSeparationSlice(wt, 'default');
      return runBash(`set -e\n${slice.raw}`, join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/gated crates present in --no-default-features workspace tree:\s*[1-9]\d*/],
  },
  {
    id: 'feature_separation_engine',
    source: getFeatureSeparationSlice(REPO_ROOT, 'engine').source,
    ciCommand: getFeatureSeparationSlice(REPO_ROOT, 'engine').command,
    faultSummary: 'Enable the `generate` feature on `anima-worldgen` in `src-tauri/Cargo.toml`',
    patch: 'feature_separation_engine.patch',
    needsCargo: true,
    run(wt) {
      const slice = getFeatureSeparationSlice(wt, 'engine');
      return runBash(`set -e\n${slice.raw}`, join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/gated crates present in default anima-engine tree:\s*[1-9]\d*/],
  },
  {
    id: 'feature_separation_worldgen',
    source: getFeatureSeparationSlice(REPO_ROOT, 'worldgen').source,
    ciCommand: getFeatureSeparationSlice(REPO_ROOT, 'worldgen').command,
    faultSummary: 'Add `bevy_ecs = "0.13.0"` to `src-tauri/crates/anima-worldgen/Cargo.toml`',
    patch: 'feature_separation_worldgen.patch',
    needsCargo: true,
    run(wt) {
      const slice = getFeatureSeparationSlice(wt, 'worldgen');
      return runBash(`set -e\n${slice.raw}`, join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/forbidden engine\/GPU crates present in anima-worldgen tree:\s*[1-9]\d*/],
  },
  {
    id: 'ts_rs_parity',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'TypeScript IPC bindings match the Rust structs').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'TypeScript IPC bindings match the Rust structs').command,
    faultSummary: 'Add `#[ts(rename = "x_m2_fault")]` to `SegmentState::x` in `simulation_state.rs`',
    patch: 'ts_rs_parity.patch',
    needsCargo: true,
    run(wt) {
      const step = getWorkflowStep(
        wt,
        '.github/workflows/ci.yml',
        'rust',
        'TypeScript IPC bindings match the Rust structs',
      ).run;
      return runBash(`set -e\n${step}`, join(wt, 'src-tauri'), { ...cargoEnv(), GIT_PAGER: 'cat' });
    },
    matchers: [/x_m2_fault/],
  },
  {
    id: 'check_test_targets_empty',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No test target ran zero tests').source,
    ciCommand: `[harness-assembled: 12 policy targets] ${getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No test target ran zero tests').command}`,
    faultSummary: 'Add `#![cfg(any())]` at top of `src-tauri/tests/shared_domain_law_tests.rs`',
    patch: 'check_test_targets_empty.patch',
    cleanKey: 'check_test_targets:from_ci',
    needsCargo: (ctx, isClean) => (isClean ? !ctx.captures.clean_ci_from_ci : true),
    runClean(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No test target ran zero tests');
      return runCheckTestTargetsFromCi(wt, tmpBase, ctx, profile, 'clean_ci_from_ci');
    },
    run(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No test target ran zero tests');
      return runCheckTestTargetsFromCi(wt, tmpBase, ctx, profile, null);
    },
    matchers: [/test target "tests\/shared_domain_law_tests\.rs" compiled but ran zero tests/],
  },
  {
    id: 'check_test_targets_ignored',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No test target ran zero tests').source,
    ciCommand: `[harness-assembled: 12 policy targets] ${getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No test target ran zero tests').command}`,
    faultSummary: 'Add `#[ignore]` to `the_engine_reads_the_domain_crates_values_for_the_time_laws`',
    patch: 'check_test_targets_ignored.patch',
    cleanKey: 'check_test_targets:from_ci',
    needsCargo: (ctx, isClean) => (isClean ? !ctx.captures.clean_ci_from_ci : true),
    runClean(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No test target ran zero tests');
      return runCheckTestTargetsFromCi(wt, tmpBase, ctx, profile, 'clean_ci_from_ci');
    },
    run(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No test target ran zero tests');
      return runCheckTestTargetsFromCi(wt, tmpBase, ctx, profile, null);
    },
    matchers: [
      /test "the_engine_reads_the_domain_crates_values_for_the_time_laws" \(tests\/shared_domain_law_tests\.rs\) is #\[ignore\]d and is not in IGNORED_TESTS/,
    ],
  },
  {
    id: 'check_test_targets_unscheduled_profile',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No test target ran zero tests').source,
    ciCommand: `[harness-assembled: 12 policy targets] ${getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No test target ran zero tests').command}`,
    faultSummary:
      'Drop `--features desktop` from `cargo test` in `.github/workflows/ci.yml:184` (checked with CI step profile)',
    patch: 'check_test_targets_unscheduled.patch',
    cleanKey: 'check_test_targets:from_ci',
    needsCargo: (ctx, isClean) => (isClean ? !ctx.captures.clean_ci_from_ci : !ctx.captures.unscheduled),
    runClean(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No test target ran zero tests');
      return runCheckTestTargetsFromCi(wt, tmpBase, ctx, profile, 'clean_ci_from_ci');
    },
    run(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No test target ran zero tests');
      return runCheckTestTargetsFromCi(wt, tmpBase, ctx, profile, 'unscheduled');
    },
    matchers: [/profile "desktop" enables "networking", so tests\/adversarial_challenger_tests\.rs must run/],
  },
  {
    id: 'cargo_test_worldgen',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo test (anima-worldgen CPU reader/generate/bake)').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'cargo test (anima-worldgen CPU reader/generate/bake)').command,
    faultSummary: 'Invert `LITHOLOGY_CLASS_COUNT` assertion (`6 -> 99`) in `src-tauri/crates/anima-worldgen/src/lib.rs`',
    patch: 'cargo_test_worldgen.patch',
    needsCargo: true,
    run(wt) {
      const step = getWorkflowStep(
        wt,
        '.github/workflows/ci.yml',
        'rust',
        'cargo test (anima-worldgen CPU reader/generate/bake)',
      ).run;
      return runBash(step, join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/test tests::elevation_normalization_anchors_match_v3_vertical_scale \.\.\. FAILED/],
  },
  {
    id: 'check_test_targets_worldgen_unscheduled',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No anima-worldgen test target ran zero tests').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No anima-worldgen test target ran zero tests').command,
    faultSummary: 'Drop `generate,bake` features from `cargo test -p anima-worldgen` in `.github/workflows/ci.yml:199`',
    patch: 'check_test_targets_worldgen_unscheduled.patch',
    cleanKey: 'check_test_targets:worldgen-cpu',
    needsCargo: (ctx, isClean) => (isClean ? !ctx.captures.clean_worldgen : true),
    runClean(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No anima-worldgen test target ran zero tests');
      return runWorldgenTestTargetsFromCi(wt, tmpBase, ctx, profile, 'clean_worldgen');
    },
    run(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No anima-worldgen test target ran zero tests');
      return runWorldgenTestTargetsFromCi(wt, tmpBase, ctx, profile, null);
    },
    matchers: [/profile "worldgen-cpu" enables "generate", so tests\/wv2_1a_tectonic_tests\.rs must run/],
  },
  {
    id: 'check_test_targets_worldgen_empty',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No anima-worldgen test target ran zero tests').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'No anima-worldgen test target ran zero tests').command,
    faultSummary: 'Add `#![cfg(any())]` at top of `src-tauri/crates/anima-worldgen/tests/wv2_1a_tectonic_tests.rs`',
    patch: 'check_test_targets_worldgen_empty.patch',
    cleanKey: 'check_test_targets:worldgen-cpu',
    needsCargo: (ctx, isClean) => (isClean ? !ctx.captures.clean_worldgen : true),
    runClean(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No anima-worldgen test target ran zero tests');
      return runWorldgenTestTargetsFromCi(wt, tmpBase, ctx, profile, 'clean_worldgen');
    },
    run(wt, tmpBase, ctx) {
      const profile = extractProfileFromStep(wt, 'No anima-worldgen test target ran zero tests');
      return runWorldgenTestTargetsFromCi(wt, tmpBase, ctx, profile, null);
    },
    matchers: [/test target "tests\/wv2_1a_tectonic_tests\.rs" compiled but ran zero tests/],
  },
  {
    id: 'feature_gated_targets',
    source: 'ci.yml (cargo test -> feature_gated_targets_tests)',
    ciCommand: `[harness-assembled] ${buildCiCargoTestTargetCmd(REPO_ROOT, 'feature_gated_targets_tests')}`,
    faultSummary:
      'Mistype `required-features = ["networking_m2_typo"]` on `migration_tests` in `src-tauri/Cargo.toml`',
    patch: 'feature_gated_targets.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        buildCiCargoTestTargetCmd(wt, 'feature_gated_targets_tests'),
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/test target `migration_tests` requires feature `networking_m2_typo`/],
  },
  {
    id: 'schedule_ambiguity_simrng',
    source: 'CARD M2 (ci.yml -> schedule_ambiguity_tests)',
    ciCommand: `[harness-assembled] ${buildCiCargoTestTargetCmd(REPO_ROOT, 'schedule_ambiguity_tests')}`,
    faultSummary: 'Put `(spawn_food_system, seed_dropping_system)` in parallel in `simulation_schedule.rs`',
    patch: 'schedule_ambiguity_simrng.patch',
    cleanKey: 'cargo_test:schedule_ambiguity_tests',
    needsCargo: true,
    run(wt) {
      return runShell(
        buildCiCargoTestTargetCmd(wt, 'schedule_ambiguity_tests'),
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [
      /SIMRNG\s+seed_dropping_system <-> spawn_food_system/,
      /"seed_dropping_system <-> spawn_food_system"/,
    ],
  },
  {
    id: 'schedule_ambiguity_energy',
    source: 'CARD M2 (ci.yml -> schedule_ambiguity_tests)',
    ciCommand: `[harness-assembled] ${buildCiCargoTestTargetCmd(REPO_ROOT, 'schedule_ambiguity_tests')}`,
    faultSummary: 'Put `(combat_system, metabolic_decay_system)` in parallel in `simulation_schedule.rs`',
    patch: 'schedule_ambiguity_energy.patch',
    cleanKey: 'cargo_test:schedule_ambiguity_tests',
    needsCargo: true,
    run(wt) {
      return runShell(
        buildCiCargoTestTargetCmd(wt, 'schedule_ambiguity_tests'),
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [
      /ENERGY\s+combat_system <-> metabolic_decay_system/,
      /"combat_system <-> metabolic_decay_system"/,
    ],
  },
  {
    id: 'zero_alloc',
    source: 'CARD M2 (ci.yml -> emit_zero_alloc_tests)',
    ciCommand: `[harness-assembled] ${buildCiCargoTestTargetCmd(REPO_ROOT, 'emit_zero_alloc_tests')}`,
    faultSummary: 'Allocate `Vec::<u8>::with_capacity(64)` in `refresh_tick_payload` (`emit.rs`)',
    patch: 'zero_alloc.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        buildCiCargoTestTargetCmd(wt, 'emit_zero_alloc_tests'),
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/test emit_path_is_allocation_free \.\.\. FAILED/],
  },
  {
    id: 'closed_eu',
    source: 'CARD M2 (ci.yml -> energy_conservation_tests)',
    ciCommand: `[harness-assembled] ${buildCiCargoTestTargetCmd(REPO_ROOT, 'energy_conservation_tests')}`,
    faultSummary: 'Inject `+ 1.0` EU per entity in `metabolic_decay_system` (`world_systems.rs`)',
    patch: 'closed_eu.patch',
    needsCargo: true,
    run(wt) {
      return runShell(
        buildCiCargoTestTargetCmd(wt, 'energy_conservation_tests'),
        join(wt, 'src-tauri'),
        { ...cargoEnv(), ANIMA_ENERGY_GATE_TICKS: '1000' },
      );
    },
    matchers: [/eating food changed the closed total by/i],
  },
  {
    id: 'live_experiment_fork',
    source: 'CARD M2 (ci.yml -> live_experiment_tests)',
    ciCommand: `[harness-assembled] ${buildCiCargoTestTargetCmd(REPO_ROOT, 'live_experiment_tests')}`,
    faultSummary:
      'Pass control `manifest.interventions` instead of `effective_treatment.interventions` in `checkpoint_fork_with_exotic` (`experiment_runner/fork.rs`)',
    patch: 'live_experiment_fork.patch',
    cleanKey: 'cargo_test:live_experiment_tests',
    needsCargo: true,
    run(wt) {
      return runShell(
        buildCiCargoTestTargetCmd(wt, 'live_experiment_tests'),
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [/the treatment factor must change the branch/],
  },
  {
    id: 'live_experiment_saveload',
    source: 'CARD M2 (ci.yml -> live_experiment_tests)',
    ciCommand: `[harness-assembled] ${buildCiCargoTestTargetCmd(REPO_ROOT, 'live_experiment_tests')}`,
    faultSummary: 'Reset `sim_rng_pos` to `0` on restore in `restore_energy_state` (`simulation_state.rs`)',
    patch: 'live_experiment_saveload.patch',
    cleanKey: 'cargo_test:live_experiment_tests',
    needsCargo: true,
    run(wt) {
      return runShell(
        buildCiCargoTestTargetCmd(wt, 'live_experiment_tests'),
        join(wt, 'src-tauri'),
        cargoEnv(),
      );
    },
    matchers: [
      /a live run resumed from a checkpoint diverged/,
      /a control branch forked at tick 60 diverged/,
    ],
  },
  {
    id: 'determinism_golden_hash',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/hardening.yml', 'determinism', 'golden hash tất định').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/hardening.yml', 'determinism', 'golden hash tất định').command,
    faultSummary: 'Perturb `SimRng::for_world(world_identity.seed.wrapping_add(1))` in `core/ecs.rs`',
    patch: 'determinism_golden_hash.patch',
    needsCargo: true,
    run(wt) {
      const step = getWorkflowStep(
        wt,
        '.github/workflows/hardening.yml',
        'determinism',
        'golden hash tất định',
      ).run;
      return runBash(step, join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/world_hash doi/],
  },
  {
    id: 'check_licenses',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'Third-party licence texts are current and complete').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'Third-party licence texts are current and complete').command,
    faultSummary: 'Flip 1 hex char in `sourceSha256` inside `licensing/third-party-index.json`',
    patch: 'check_licenses.patch',
    cleanKey: 'npm run check:licenses',
    windowsOnly: true,
    needsCargo: true,
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'Third-party licence texts are current and complete').run,
        wt,
        cargoEnv(),
      );
    },
    matchers: [/sourceSha256.*differs|third-party-index\.json is out of date|sha256.*mismatch/i],
  },
  {
    id: 'check_sbom',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'SBOM is current with the shipped dependency graph').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'SBOM is current with the shipped dependency graph').command,
    faultSummary: 'Change `metadata.component.name` to `anima-engine-m2-fault` in `sbom.cdx.json`',
    patch: 'check_sbom.patch',
    windowsOnly: true,
    needsCargo: true,
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'SBOM is current with the shipped dependency graph').run,
        wt,
        cargoEnv(),
      );
    },
    matchers: [/sbom\.cdx\.json is out of date/i],
  },
  {
    id: 'check_sbom_schema',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'SBOM validates against the official CycloneDX 1.5 schema').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'SBOM validates against the official CycloneDX 1.5 schema').command,
    faultSummary: 'Add `metadata.timestamp` disallowed by CycloneDX 1.5 strict check in `sbom.cdx.json`',
    patch: 'check_sbom_schema.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'SBOM validates against the official CycloneDX 1.5 schema').run,
        wt,
      );
    },
    matchers: [/metadata\.timestamp is present; the SBOM is generated without one/],
  },
  {
    id: 'check_notice',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'NOTICE is current with the shipped dependency graph').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'NOTICE is current with the shipped dependency graph').command,
    faultSummary: 'Delete `- **Rust — ... crates.**` bullet from `NOTICE`',
    patch: 'check_notice.patch',
    windowsOnly: true,
    needsCargo: true,
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'NOTICE is current with the shipped dependency graph').run,
        wt,
        cargoEnv(),
      );
    },
    matchers: [/NOTICE is out of date with the dependency graph/i],
  },
  {
    id: 'check_flora_footprint',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'Declared flora footprints match the real geometry').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'Declared flora footprints match the real geometry').command,
    faultSummary: 'Change `FLORA_FOOTPRINT_UNIT_RADIUS[FloraType.Pine]` to `0.99` in `floraClearance.ts`',
    patch: 'check_flora_footprint.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'rust', 'Declared flora footprints match the real geometry').run,
        wt,
      );
    },
    matchers: [/^- Pine: declared 0\.99 but the geometry measures 0\.5500/m],
  },
  {
    id: 'verify_dataflow',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'Module dependency graph matches dataflow.json').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'rust', 'Module dependency graph matches dataflow.json').command,
    faultSummary: 'Insert invented `physics -> commands` edge into `dataflow.json`',
    patch: 'verify_dataflow.patch',
    needsCargo: true,
    run(wt) {
      const step = getWorkflowStep(
        wt,
        '.github/workflows/ci.yml',
        'rust',
        'Module dependency graph matches dataflow.json',
      ).run;
      return runBash(`set -e\n${step}`, join(wt, 'src-tauri'), cargoEnv());
    },
    matchers: [/physics\s*->\s*commands/],
  },

  // ---- Frontend, Hygiene, Docs & Policy Gates ----------------------------------------------
  {
    id: 'check_npm_audit',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm audit, root and tests (policy in scripts/npm_audit_policy.mjs)').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm audit, root and tests (policy in scripts/npm_audit_policy.mjs)').command,
    faultSummary: 'Add stale withdrawn exemption `GHSA-fc5q-39xc-vjpf` to `scripts/npm_audit_policy.mjs`',
    patch: 'check_npm_audit.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'npm audit, root and tests (policy in scripts/npm_audit_policy.mjs)').run,
        wt,
      );
    },
    matchers: [/policy entry GHSA-fc5q-39xc-vjpf .* matches no advisory/],
  },
  {
    id: 'check_text_hygiene',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'No raw control bytes in tracked source').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'No raw control bytes in tracked source').command,
    faultSummary: 'Embed NUL byte `\\x00` inside `scripts/npm_audit_policy.mjs` via binary patch',
    patch: 'check_text_hygiene.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'No raw control bytes in tracked source').run,
        wt,
      );
    },
    matchers: [/scripts\/npm_audit_policy\.mjs: control byte\(s\)/],
  },
  {
    id: 'check_root_hygiene',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Repository root holds only reviewed entries').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Repository root holds only reviewed entries').command,
    faultSummary: 'Add unreviewed `unreviewed_m2_fault.txt` at repository root',
    patch: 'check_root_hygiene.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'Repository root holds only reviewed entries').run,
        wt,
      );
    },
    matchers: [/unreviewed_m2_fault\.txt: at the repository root but not in scripts\/root_allowlist\.json/],
  },
  {
    id: 'check_e2',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'E2 artifacts match the committed preregistration').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'E2 artifacts match the committed preregistration').command,
    faultSummary: 'Alter `control_mean` in `artifacts/experiments/e2-evolved-brain-default/effects.json`',
    patch: 'check_e2.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'E2 artifacts match the committed preregistration').run,
        wt,
      );
    },
    matchers: [/FAIL\s+analysis: effects\.json hashes to/],
  },
  {
    id: 'check_e3',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'E3 artifacts match the committed preregistration').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'E3 artifacts match the committed preregistration').command,
    faultSummary:
      'Alter `residual` for seed `731001` in `artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/effects.json`',
    patch: 'check_e3.patch',
    needsNodeModules: false,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'E3 artifacts match the committed preregistration').run,
        wt,
      );
    },
    matchers: [/SHA-256 mismatch for effects\.json/],
  },
  {
    id: 'npm_lint',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run lint').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run lint').command,
    faultSummary: 'Add empty statement `if (Boolean(1)) {}` to `src/main.tsx`',
    patch: 'npm_lint.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'npm run lint').run,
        wt,
      );
    },
    matchers: [/Empty block statement\s+no-empty/],
  },
  {
    id: 'eslint_ratchet',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'ESLint warning ratchet').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'ESLint warning ratchet').command,
    faultSummary: 'Add `export const _m2AnyFault: any = 1;` to `src/main.tsx`',
    patch: 'eslint_ratchet.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'ESLint warning ratchet').run,
        wt,
      );
    },
    matchers: [/^ESLint warnings went up: 1 > 0\.$/m],
  },
  {
    id: 'typecheck_tests',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Typecheck the tests/ package').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Typecheck the tests/ package').command,
    faultSummary: "Add `export const _m2TypeFault: number = 'bad';` to `tests/frontend/mapManifestEvidence.test.ts`",
    patch: 'typecheck_tests.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'Typecheck the tests/ package').run,
        wt,
      );
    },
    matchers: [/mapManifestEvidence\.test\.ts\(\d+,\d+\): error TS2322:/],
  },
  {
    id: 'vitest_src',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run test (src suite)').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run test (src suite)').command,
    faultSummary: 'Break `FLORA_COLLIDER_BASE_RADIUS` assertion in `src/__tests__/floraClearance.test.ts`',
    patch: 'vitest_src.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'npm run test (src suite)').run,
        wt,
      );
    },
    matchers: [/FAIL\s+src\/__tests__\/floraClearance\.test\.ts/],
  },
  {
    id: 'vitest_tests',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run test:frontend (tests/ suite)').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run test:frontend (tests/ suite)').command,
    faultSummary: 'Invert `expect(res.ok).toBe(false)` in `tests/frontend/mapManifestEvidence.test.ts`',
    patch: 'vitest_tests.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'npm run test:frontend (tests/ suite)').run,
        wt,
        { ...process.env, CI: 'true' },
      );
    },
    matchers: [/FAIL\s+frontend\/mapManifestEvidence\.test\.ts/],
  },
  {
    id: 'npm_build',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run build').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run build').command,
    faultSummary: "Add `export const _m2BuildFault: number = 'bad';` to `src/main.tsx`",
    patch: 'npm_build.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'npm run build').run,
        wt,
      );
    },
    matchers: [/src\/main\.tsx\(\d+,\d+\): error TS2322:/],
  },
  {
    id: 'check_bundle_closure',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'The committed npm distribution boundary matches a fresh build').source,
    ciCommand: `npm run build && ${getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'The committed npm distribution boundary matches a fresh build').command}`,
    faultSummary: 'Import dev-only `ajv` into `src/main.tsx` so bundle closure shifts',
    patch: 'check_bundle_closure.patch',
    needsNodeModules: true,
    run(wt) {
      const buildRes = runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'npm run build').run,
        wt,
      );
      if (buildRes.status !== 0) {
        return buildRes;
      }
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'The committed npm distribution boundary matches a fresh build').run,
        wt,
      );
    },
    matchers: [/licensing\/bundle-closure\.json differs from the committed copy/],
  },
  {
    id: 'check_csp',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'CSP compatibility of the shipped artifacts').source,
    ciCommand: `npm run build && ${getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'CSP compatibility of the shipped artifacts').command}`,
    faultSummary: 'Add external `<script src="https://example.com/m2-fault.js"></script>` to `index.html`',
    patch: 'check_csp.patch',
    needsNodeModules: true,
    run(wt) {
      const buildRes = runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'npm run build').run,
        wt,
      );
      if (buildRes.status !== 0) {
        return buildRes;
      }
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'CSP compatibility of the shipped artifacts').run,
        wt,
      );
    },
    matchers: [/loads an external origin.*example\.com\/m2-fault\.js/],
  },
  {
    id: 'check_bundle',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Bundle budget and route split').source,
    ciCommand: `npm run build && ${getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Bundle budget and route split').command}`,
    faultSummary: 'Eagerly import `three` into dashboard root `src/App.tsx`',
    patch: 'check_bundle.patch',
    needsNodeModules: true,
    run(wt) {
      const buildRes = runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'npm run build').run,
        wt,
      );
      if (buildRes.status !== 0) {
        return buildRes;
      }
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'Bundle budget and route split').run,
        wt,
      );
    },
    matchers: [/the 2D dashboard \(index\.html\) statically loads the three\.js chunk/],
  },
  {
    id: 'check_e2e_specs',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'E2E spec list matches what test:e2e schedules').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'E2E spec list matches what test:e2e schedules').command,
    faultSummary: 'Narrow `tests/package.json` `test:e2e` script to `e2e/console_hygiene.spec.ts`',
    patch: 'check_e2e_specs.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'E2E spec list matches what test:e2e schedules').run,
        wt,
      );
    },
    matchers: [/SPEC MISSING FROM RUN:.*ipc_contract\.spec\.ts/],
  },
  {
    id: 'playwright_e2e',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run test:e2e (Playwright)').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'npm run test:e2e (Playwright)').command,
    faultSummary: 'Change `expect(owned.length).toBe(0)` to `.toBe(999)` in `tests/e2e/console_hygiene.spec.ts`',
    patch: 'playwright_e2e.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'npm run test:e2e (Playwright)').run,
        wt,
      );
    },
    matchers: [/Expected:\s*999/],
  },
  {
    id: 'check_docs_links',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Docs link check').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Docs link check').command,
    faultSummary: 'Append broken markdown link labeled `m2-broken` targeting `./m2-missing-target.md` to `docs/README.md`',
    patch: 'check_docs_links.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'Docs link check').run,
        wt,
      );
    },
    matchers: [/docs\/README\.md:\d+: broken link -> \.\/m2-missing-target\.md/],
  },
  {
    id: 'check_docs_metadata',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Docs metadata check').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Docs metadata check').command,
    faultSummary: 'Set `status: invalid-m2` in `docs/README.md` front matter',
    patch: 'check_docs_metadata.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'Docs metadata check').run,
        wt,
      );
    },
    matchers: [/docs\/README\.md: status "invalid-m2" not in/],
  },
  {
    id: 'check_ipc_reference',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'IPC reference check').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'IPC reference check').command,
    faultSummary: 'Insert nonexistent `m2_ghost_cmd` row in `docs/reference/IPC_CONTRACT.md`',
    patch: 'check_ipc_reference.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'IPC reference check').run,
        wt,
      );
    },
    matchers: [/command `m2_ghost_cmd` is documented but not in the code/],
  },
  {
    id: 'check_ipc_arg_case',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Frontend invoke argument case matches Rust tauri commands').source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/ci.yml', 'frontend', 'Frontend invoke argument case matches Rust tauri commands').command,
    faultSummary:
      'Add `invoke("save_simulation_state", { file_path })` (snake_case) helper in `src/components/Landscape/utils/worldCache.ts`',
    patch: 'check_ipc_arg_case.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/ci.yml', 'frontend', 'Frontend invoke argument case matches Rust tauri commands').run,
        wt,
      );
    },
    matchers: [/FAIL\s+src\/components\/Landscape\/utils\/worldCache\.ts:\d+ invoke\('save_simulation_state', \{ file_path/],
  },
  {
    id: 'check_map_vision_manifest',
    source: 'package.json:23 (ci.yml -> vitest tests/)',
    ciCommand: 'npm run check:map-vision-manifest',
    faultSummary: 'Set `seaLevel: 0.99` in `animal-map.manifest.json`',
    patch: 'check_map_vision_manifest.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run check:map-vision-manifest', wt);
    },
    matchers: [/animal-map manifest is stale/],
  },
  {
    id: 'verify_upstream_licenses',
    source: 'package.json:48',
    ciCommand: 'npm run verify:upstream-licenses',
    faultSummary: 'Flip 1 hex char in `ts-rs` `sha256` inside `licensing/upstream/sources.json`',
    patch: 'verify_upstream_licenses.patch',
    needsNodeModules: true,
    run(wt) {
      return runShell('npm run verify:upstream-licenses', wt);
    },
    matchers: [/sources\[0\] \(github\.com\/Aleph-Alpha\/ts-rs.*sha256 is/],
  },
  {
    id: 'compare_checksums_self_test',
    source: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/determinism-matrix.yml', 'compare', "The compare script's negative controls").source,
    ciCommand: getWorkflowCommandLine(REPO_ROOT, '.github/workflows/determinism-matrix.yml', 'compare', "The compare script's negative controls").command,
    faultSummary: 'Force `checksumAgrees = true` in `scripts/paper/compare_checksums.mjs`',
    patch: 'compare_checksums_self_test.patch',
    needsNodeModules: false,
    run(wt) {
      return runShell(
        getWorkflowStep(wt, '.github/workflows/determinism-matrix.yml', 'compare', "The compare script's negative controls").run,
        wt,
      );
    },
    matchers: [/FAIL\s+a checksum difference is named at its tick/],
  },
];

export function defaultCleanKey(gate) {
  const base = `${gate.ciCommand}@@${gate.needsCargo ? 'cargo' : 'nocargo'}@@${gate.needsNodeModules ? 'nm' : 'nonm'}`;
  return typeof gate.runClean === 'function' ? `${base}@@clean:${gate.id}` : base;
}

function createWorktreeRunner(patchFile, gate, ctx, isClean) {
  const needsCargo =
    typeof gate.needsCargo === 'function'
      ? Boolean(gate.needsCargo(ctx, isClean))
      : Boolean(gate.needsCargo);
  const runner = isClean && typeof gate.runClean === 'function' ? gate.runClean : gate.run;
  return withTempWorktree(
    patchFile,
    {
      needsNodeModules: Boolean(gate.needsNodeModules),
      needsCargo,
    },
    (wt, tmpBase) => runner(wt, tmpBase, ctx),
  );
}

export function evaluateGate(gate, ctx = { captures: {}, tempFilesToClean: [], cleanCache: new Map() }, deps = {}) {
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
      baselineExitCode: null,
      matched: false,
      elapsedSecs: 0,
    };
  }

  if (gate.windowsOnly && (deps.platform ?? process.platform) !== 'win32' && !deps.runClean && !deps.runFault) {
    return {
      id: gate.id,
      source: gate.source,
      ciCommand: gate.ciCommand,
      fault: gate.faultSummary,
      exitCode: '—',
      namingLine:
        'not measured on Windows (licensing/SBOM/NOTICE track the Windows CI dependency graph at ci.yml:210-213)',
      verdict: VERDICTS.NOT_RUN_WINDOWS,
      baselineExitCode: null,
      matched: false,
      elapsedSecs: 0,
    };
  }

  const runCleanFn = deps.runClean ?? ((g, c) => createWorktreeRunner(null, g, c, true));
  const runFaultFn = deps.runFault ?? ((g, c) => createWorktreeRunner(g.patch, g, c, false));

  if (!ctx.cleanCache) ctx.cleanCache = new Map();
  const cleanKey = gate.cleanKey ?? defaultCleanKey(gate);
  let baselineRes;
  if (ctx.cleanCache.has(cleanKey)) {
    baselineRes = ctx.cleanCache.get(cleanKey);
  } else {
    baselineRes = runCleanFn(gate, ctx);
    ctx.cleanCache.set(cleanKey, baselineRes);
  }

  let faultRes = null;
  if (baselineRes && baselineRes.status === 0) {
    faultRes = runFaultFn(gate, ctx);
  }

  const classified = classifyGateResult({
    baselineRes,
    faultRes,
    matchers: gate.matchers,
  });
  const elapsedSecs = Math.round(((performance.now() - t0) / 1000) * 100) / 100;
  return {
    id: gate.id,
    source: gate.source,
    ciCommand: gate.ciCommand,
    fault: gate.faultSummary,
    exitCode: classified.exitCode,
    namingLine: classified.namingLine,
    verdict: classified.verdict,
    baselineExitCode: baselineRes ? baselineRes.status : null,
    matched: classified.matched,
    elapsedSecs,
  };
}

export function verdictRank(verdict) {
  if (verdict === VERDICTS.VACUOUS) return 0;
  if (verdict === VERDICTS.UNMATCHED_FAILURE) return 1;
  if (verdict.startsWith('NOT RUN')) return 2;
  return 3;
}

export function sortAndSummarizeResults(results) {
  const ordered = [...results].sort((a, b) => verdictRank(a.verdict) - verdictRank(b.verdict));
  const counts = {
    total: ordered.length,
    vacuous: ordered.filter((r) => r.verdict === VERDICTS.VACUOUS).length,
    unmatchedFailure: ordered.filter((r) => r.verdict === VERDICTS.UNMATCHED_FAILURE).length,
    catches: ordered.filter((r) => r.verdict === VERDICTS.CATCHES).length,
    notRunNoGate: ordered.filter((r) => r.verdict === VERDICTS.NOT_RUN_NO_GATE).length,
    notRunTime: ordered.filter((r) => r.verdict === VERDICTS.NOT_RUN_TIME).length,
    notRunWindows: ordered.filter((r) => r.verdict === VERDICTS.NOT_RUN_WINDOWS).length,
    notRunBaselineRed: ordered.filter((r) => r.verdict.startsWith('NOT RUN (baseline red')).length,
  };
  return { ordered, counts };
}

export function formatMarkdownTable({ ordered, counts }) {
  const lines = [
    `SUMMARY total=${counts.total} VACUOUS=${counts.vacuous} UNMATCHED_FAILURE=${counts.unmatchedFailure} CATCHES=${counts.catches} ` +
      `NOT_RUN_NO_GATE=${counts.notRunNoGate} NOT_RUN_TIME=${counts.notRunTime} ` +
      `NOT_RUN_WINDOWS=${counts.notRunWindows} NOT_RUN_BASELINE_RED=${counts.notRunBaselineRed}`,
    '| gate | CI command | fault | exit code | naming line (verbatim) | verdict |',
    '|---|---|---|---|---|---|',
  ];
  for (const row of ordered) {
    lines.push(
      `| ${formatMdCode(row.id)} | ${formatMdCode(row.ciCommand)} | ${sanitizeMdCell(row.fault)} | ${row.exitCode} | ${formatMdCode(row.namingLine)} | **${row.verdict}** |`,
    );
  }
  return lines.join('\n');
}

export const CLEAN_BASELINE_SAMPLES = Object.freeze([
  'ok',
  'MIT',
  'Apache-2.0 OR MIT',
  'NOTICE',
  'NOTICE is up to date with the dependency graph',
  'ESLint',
  'eslint: 0 errors, 0 warnings (baseline 0)',
  'AssertionError',
  'test unrelated_other_test ... FAILED',
  'test targets: 56, empty: 0',
  'Test Files  12 passed (12)',
  'Tests  42 passed (42)',
  'verify_e2_artifacts OK',
  'verify_e3_artifacts OK (/tmp/e3)',
  'thread_rng() literal matches: 0',
  'gated crates present in --no-default-features workspace tree: 0',
  'gated crates present in default anima-engine tree: 0',
  'forbidden engine/GPU crates present in anima-worldgen tree: 0',
  'third-party licences check: up to date',
  'sbom.cdx.json is up to date',
  'ipc reference check: 36 commands, 7 events, 0 differences',
  'ipc arg case: 36 tauri commands, 39 invoke call sites, 15 argument keys checked',
  'docs link check: 1643 relative links + 97 same-page anchors in 243 files, 0 broken',
  'docs metadata check: 181 files, 0 errors, 0 overdue reviews',
  'text hygiene: 1136 tracked text file(s), no raw control bytes',
  'root hygiene: 64 root entries, all listed in scripts/root_allowlist.json; self-test 4 controls ok',
  'entry index.html 12 static chunk(s), 3D renderer: no',
  'OK — every chunk within budget, total 180.0/550 KiB',
  'test tests::elevation_normalization_anchors_match_v3_vertical_scale ... ok',
  'test emit_path_is_allocation_free ... ok',
  'Pine declared 0.5500 measured 0.5500',
  'animal-map manifest is up to date',
  'e2e spec policy: 6 specs scheduled',
  'compare_checksums: 2 self-test controls ok',
  'world_hash ok',
]);

export function validateGateRegistry(
  gates = GATES,
  { faultsDir = FAULTS_DIR, cleanSamples = CLEAN_BASELINE_SAMPLES, cleanOutputs = {} } = {},
) {
  const problems = [];
  const seenIds = new Set();
  const referencedPatches = new Set();

  for (const gate of gates) {
    if (!gate.id || typeof gate.id !== 'string') {
      problems.push('gate missing valid string id');
      continue;
    }
    if (seenIds.has(gate.id)) {
      problems.push(`duplicate gate id: ${gate.id}`);
    }
    seenIds.add(gate.id);

    if (!gate.source || !gate.ciCommand || !gate.faultSummary) {
      problems.push(`${gate.id}: missing source, ciCommand, or faultSummary`);
    }

    if (gate.staticResult) {
      if (gate.patch !== null) {
        problems.push(`${gate.id}: staticResult gate must set patch: null`);
      }
      if (!gate.staticResult.verdict || !gate.staticResult.namingLine) {
        problems.push(`${gate.id}: staticResult missing verdict or namingLine`);
      }
      continue;
    }

    if (typeof gate.run !== 'function') {
      problems.push(`${gate.id}: missing run() function`);
    }
    if (!Array.isArray(gate.matchers) || gate.matchers.length === 0) {
      problems.push(`${gate.id}: missing non-empty matchers array`);
    } else {
      const gateCleanLines = [
        ...cleanSamples,
        ...normalizeOutputLines(cleanOutputs[gate.id] ?? '', false).lines,
      ];
      for (const m of gate.matchers) {
        if (!(m instanceof RegExp)) {
          problems.push(`${gate.id}: matcher is not a RegExp`);
        } else if (m.global) {
          problems.push(`${gate.id}: matcher ${m} must not carry /g flag`);
        } else {
          const hit = gateCleanLines.find((line) => m.test(line));
          if (hit) {
            problems.push(`${gate.id}: matcher ${m} matches clean-baseline line ${JSON.stringify(hit)}`);
          }
        }
      }
    }
    if (typeof gate.patch !== 'string' || !gate.patch.endsWith('.patch')) {
      problems.push(`${gate.id}: missing valid .patch filename`);
    } else {
      referencedPatches.add(gate.patch);
      const patchPath = join(faultsDir, gate.patch);
      if (!existsSync(patchPath)) {
        problems.push(`${gate.id}: missing fault patch file ${patchPath}`);
      }
    }
  }

  if (existsSync(faultsDir)) {
    for (const file of readdirSync(faultsDir)) {
      if (file.endsWith('.patch') && !referencedPatches.has(file)) {
        problems.push(`unreferenced fault patch in gate_faults/: ${file}`);
      }
    }
  }
  return problems;
}

export function runSelfTest() {
  const failures = [];
  let controls = 0;

  // 1. classifyGateResult covers CATCHES, VACUOUS, NOT RUN (baseline red), and UNMATCHED_FAILURE
  {
    controls += 1;
    const res = classifyGateResult({
      baselineRes: { status: 0, output: 'ok' },
      faultRes: { status: 1, output: 'error: expected failure_token in foo.rs' },
      matchers: [/failure_token/],
    });
    if (res.verdict !== VERDICTS.CATCHES || res.exitCode !== 1 || !res.matched) {
      failures.push(`CATCHES control failed: ${JSON.stringify(res)}`);
    }
  }
  {
    controls += 1;
    const res = classifyGateResult({
      baselineRes: { status: 0, output: 'ok' },
      faultRes: { status: 0, output: 'all green 0 matches' },
      matchers: [/all green/],
    });
    if (res.verdict !== VERDICTS.VACUOUS || res.exitCode !== 0) {
      failures.push(`VACUOUS control failed: ${JSON.stringify(res)}`);
    }
  }
  {
    controls += 1;
    const res = classifyGateResult({
      baselineRes: { status: 2, output: 'Error: clean baseline broken on host' },
      faultRes: { status: 1, output: 'should never be credited' },
      matchers: [/should never/],
    });
    if (res.verdict !== VERDICTS.NOT_RUN_BASELINE_RED || res.exitCode !== 2) {
      failures.push(`NOT RUN (baseline red) control failed: ${JSON.stringify(res)}`);
    }
  }
  {
    controls += 1;
    const res = classifyGateResult({
      baselineRes: { status: 0, output: 'ok' },
      faultRes: { status: 101, output: 'error: unrelated compiler panic' },
      matchers: [/expected_fault_signature/],
    });
    if (res.verdict !== VERDICTS.UNMATCHED_FAILURE || res.verdict === VERDICTS.CATCHES || res.matched) {
      failures.push(`UNMATCHED_FAILURE control failed: ${JSON.stringify(res)}`);
    }
  }

  // 2. evaluateGate does NOT run fault patch if clean baseline is red, and caches shared clean checks
  {
    controls += 1;
    let cleanCalls = 0;
    let faultCalls = 0;
    const gateA = {
      id: 'mock_a',
      source: 'test',
      ciCommand: 'mock cmd',
      faultSummary: 'mock fault a',
      patch: 'a.patch',
      cleanKey: 'shared_mock',
      matchers: [/fault_a/],
      run: () => ({ status: 1, output: 'fault_a' }),
    };
    const gateB = {
      id: 'mock_b',
      source: 'test',
      ciCommand: 'mock cmd',
      faultSummary: 'mock fault b',
      patch: 'b.patch',
      cleanKey: 'shared_mock',
      matchers: [/fault_b/],
      run: () => ({ status: 1, output: 'fault_b' }),
    };
    const ctx = { captures: {}, tempFilesToClean: [], cleanCache: new Map() };
    const deps = {
      runClean: () => {
        cleanCalls += 1;
        return { status: 1, output: 'Error: baseline red' };
      },
      runFault: () => {
        faultCalls += 1;
        return { status: 1, output: 'fault_a' };
      },
    };
    const rA = evaluateGate(gateA, ctx, deps);
    const rB = evaluateGate(gateB, ctx, deps);
    if (
      cleanCalls !== 1 ||
      faultCalls !== 0 ||
      rA.verdict !== VERDICTS.NOT_RUN_BASELINE_RED ||
      rB.verdict !== VERDICTS.NOT_RUN_BASELINE_RED
    ) {
      failures.push(
        `evaluateGate baseline-red/cache control failed: clean=${cleanCalls} fault=${faultCalls} rA=${rA.verdict} rB=${rB.verdict}`,
      );
    }
  }

  // 3. Ordering & summary counts
  {
    controls += 1;
    const summary = sortAndSummarizeResults([
      { id: 'c', verdict: VERDICTS.CATCHES, ciCommand: 'c', fault: 'c', exitCode: 1, namingLine: 'c' },
      { id: 'r', verdict: VERDICTS.NOT_RUN_BASELINE_RED, ciCommand: 'r', fault: 'r', exitCode: 1, namingLine: 'r' },
      { id: 'u', verdict: VERDICTS.UNMATCHED_FAILURE, ciCommand: 'u', fault: 'u', exitCode: 1, namingLine: 'u' },
      { id: 'v', verdict: VERDICTS.VACUOUS, ciCommand: 'v', fault: 'v', exitCode: 0, namingLine: 'v' },
    ]);
    const ids = summary.ordered.map((r) => r.id);
    if (
      JSON.stringify(ids) !== JSON.stringify(['v', 'u', 'r', 'c']) ||
      summary.counts.vacuous !== 1 ||
      summary.counts.unmatchedFailure !== 1 ||
      summary.counts.notRunBaselineRed !== 1 ||
      summary.counts.catches !== 1
    ) {
      failures.push(`sortAndSummarizeResults control failed: ${JSON.stringify(summary)}`);
    }
  }

  // 4. Registry, clean-baseline matcher rejection & patch completeness
  {
    controls += 1;
    const regProblems = validateGateRegistry(GATES, { faultsDir: FAULTS_DIR });
    if (regProblems.length > 0) {
      failures.push(`registry validation failed: ${regProblems.join('; ')}`);
    }
  }
  {
    controls += 1;
    const leakRes = classifyGateResult({
      baselineRes: { status: 0, output: 'Test Files  12 passed (12)' },
      faultRes: { status: 1, output: 'Test Files  12 passed (12)\nFAIL unrelated' },
      matchers: [/Test Files\s+\d+ passed/],
    });
    if (leakRes.verdict !== VERDICTS.UNMATCHED_FAILURE || leakRes.matched) {
      failures.push(`clean-baseline matcher leak control failed: ${JSON.stringify(leakRes)}`);
    }
  }

  // 5. git apply --check on every registered patch against HEAD (isolated temp worktree)
  {
    const uniquePatches = [...new Set(GATES.map((g) => g.patch).filter(Boolean))];
    try {
      withTempWorktree(null, { needsNodeModules: false, needsCargo: false }, (wt) => {
        for (const patchFile of uniquePatches) {
          controls += 1;
          const patchBuf = readFileSync(join(FAULTS_DIR, patchFile));
          const check = spawnSync(
            'git',
            ['-c', 'core.autocrlf=false', 'apply', '--check', '--whitespace=nowarn', '-'],
            { cwd: wt, input: patchBuf, encoding: 'buffer' },
          );
          if (check.status !== 0) {
            failures.push(
              `git apply --check against HEAD failed for ${patchFile}: ${(check.stderr || '').toString('utf8').trim()}`,
            );
          }
        }
      });
    } catch (err) {
      controls += uniquePatches.length;
      failures.push(
        `HEAD worktree setup for patch self-test failed: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  }

  return { failures, controls };
}

function main() {
  let cli;
  try {
    cli = parseCliArgs(process.argv.slice(2));
  } catch (err) {
    console.error(err.message);
    process.exit(2);
  }
  const { gateIds, listOnly, selfTest, jsonOut, mdOut } = cli;

  if (selfTest) {
    const st = runSelfTest();
    if (st.failures.length > 0) {
      for (const f of st.failures) console.error(`FAIL: ${f}`);
      process.exit(1);
    }
    console.log(`gate_controls self-test: ${st.controls} controls ok (${GATES.length} gates registered)`);
    return;
  }

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
    captures: {},
    tempFilesToClean: [],
    cleanCache: new Map(),
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

  const summary = sortAndSummarizeResults(results);
  const tableText = formatMarkdownTable(summary);
  console.log(tableText);

  if (mdOut) {
    writeFileSync(mdOut, `${tableText}\n`, 'utf8');
  }
  if (jsonOut) {
    writeFileSync(
      jsonOut,
      JSON.stringify({ counts: summary.counts, results: summary.ordered }, null, 2),
      'utf8',
    );
  }
}

const isDirectExecution =
  process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isDirectExecution) {
  main();
}
