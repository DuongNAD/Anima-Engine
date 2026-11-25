#!/usr/bin/env node
// Build-time World v2 C01 tile prefetch and SHA-256 verification (`§3.3.2`, `WV2-6`).
//
// Stages `artifacts/world-v2/index.json`, `artifacts/world-v2/sim_256.anmw3`, and every tile/cave/flora
// entry from either:
//   1. `--archive <path>` or `ANIMA_WORLD_V2_TILES_ARCHIVE` (local `.tar.gz`),
//   2. `--dir <path>` or `ANIMA_WORLD_V2_TILES_DIR` (already-extracted tile directory), or
//   3. `WORLD_V2_RELEASE_BASE_URL` (GitHub Release download constant — never creates releases/tags).
//
// Usage:
//   node scripts/fetch_world_v2_tiles.mjs [--archive PATH | --dir PATH | --verify-only] [--out DIR]

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const WORLD_V2_RELEASE_BASE_URL =
  'https://github.com/DuongNAD/Anima-Engine/releases/download';

const HERE = dirname(fileURLToPath(import.meta.url));
const DEFAULT_ROOT = resolve(HERE, '..');

export function sha256Hex(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

export function resolveArchiveUrl(releaseArchiveRel, baseUrl = WORLD_V2_RELEASE_BASE_URL) {
  const cleanBase = String(baseUrl).replace(/\/+$/, '');
  const cleanRel = String(releaseArchiveRel).replace(/^\/+/, '');
  return `${cleanBase}/${cleanRel}`;
}

export function verifyIndexAndSim256(repoRoot) {
  const indexPath = join(repoRoot, 'artifacts', 'world-v2', 'index.json');
  const simPath = join(repoRoot, 'artifacts', 'world-v2', 'sim_256.anmw3');
  if (!existsSync(indexPath)) {
    throw new Error(`Missing committed World v2 index: ${indexPath}`);
  }
  if (!existsSync(simPath)) {
    throw new Error(`Missing committed sim_256.anmw3: ${simPath}`);
  }
  const indexBytes = readFileSync(indexPath);
  const index = JSON.parse(indexBytes.toString('utf8'));
  if (
    !index ||
    index.format_version !== 1 ||
    index.generator_version !== 3 ||
    !Array.isArray(index.tiles) ||
    index.tiles.length === 0 ||
    typeof index.sim_world_identity !== 'string' ||
    typeof index.release_archive !== 'string' ||
    typeof index.release_archive_sha256 !== 'string'
  ) {
    throw new Error(`Invalid World v2 index.json schema at ${indexPath}`);
  }
  const simBytes = readFileSync(simPath);
  const actualSimSha = sha256Hex(simBytes);
  const expectedSimSha = index.sim_world_identity.toLowerCase();
  if (simBytes.byteLength !== index.sim_256_byte_len) {
    throw new Error(
      `sim_256.anmw3 byte length mismatch: expected ${index.sim_256_byte_len}, got ${simBytes.byteLength}`,
    );
  }
  if (actualSimSha !== expectedSimSha) {
    throw new Error(
      `sim_256.anmw3 SHA-256 mismatch: expected ${expectedSimSha}, got ${actualSimSha}`,
    );
  }
  return {
    index,
    indexBytes,
    simBytes,
    bundleIdentity: sha256Hex(indexBytes),
  };
}

export function verifyArchiveSha256(archivePath, expectedSha256) {
  if (!existsSync(archivePath)) {
    throw new Error(`Tile archive not found: ${archivePath}`);
  }
  const bytes = readFileSync(archivePath);
  const actual = sha256Hex(bytes);
  const expected = expectedSha256.toLowerCase();
  if (actual !== expected) {
    throw new Error(
      `Tile archive SHA-256 mismatch (${archivePath}): expected ${expected}, got ${actual}`,
    );
  }
  return { byteLength: bytes.byteLength, sha256: actual };
}

export function verifyStagedTiles(index, stagedDir) {
  let verified = 0;
  for (const entry of index.tiles) {
    const fullPath = join(stagedDir, entry.path);
    if (!existsSync(fullPath)) {
      throw new Error(`Missing staged World v2 asset: ${entry.path} (expected at ${fullPath})`);
    }
    const bytes = readFileSync(fullPath);
    if (bytes.byteLength !== entry.byte_len) {
      throw new Error(
        `Staged asset byte_len mismatch for ${entry.path}: expected ${entry.byte_len}, got ${bytes.byteLength}`,
      );
    }
    const actualSha = sha256Hex(bytes);
    const expectedSha = String(entry.sha256).toLowerCase();
    if (actualSha !== expectedSha) {
      throw new Error(
        `Staged asset SHA-256 mismatch for ${entry.path}: expected ${expectedSha}, got ${actualSha}`,
      );
    }
    verified += 1;
  }
  return verified;
}

function assertArchiveExcludesIndexJson(archivePath) {
  const listing = execFileSync('tar', ['-tzf', archivePath], {
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  });
  const lines = listing
    .split(/\r?\n/)
    .map((l) => l.trim().replace(/^\.\//, ''))
    .filter(Boolean);
  for (const name of lines) {
    if (name === 'index.json' || name.endsWith('/index.json')) {
      throw new Error(
        `Tile archive violates LD-2 anti-cycle invariant by containing ${name}`,
      );
    }
  }
}

function copyCheckedCoreFiles(indexBytes, simBytes, outDir) {
  mkdirSync(outDir, { recursive: true });
  writeFileSync(join(outDir, 'index.json'), indexBytes);
  writeFileSync(join(outDir, 'sim_256.anmw3'), simBytes);
}

async function downloadReleaseArchive(url, dstPath) {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(
      `Failed to fetch World v2 tile release from ${url} (HTTP ${res.status}). ` +
        `If the public release is not yet published, set ANIMA_WORLD_V2_TILES_ARCHIVE to the verified local .tar.gz path.`,
    );
  }
  const buf = Buffer.from(await res.arrayBuffer());
  mkdirSync(dirname(dstPath), { recursive: true });
  writeFileSync(dstPath, buf);
}

export async function runFetchWorldV2Tiles(argv = process.argv.slice(2), env = process.env) {
  let repoRoot = DEFAULT_ROOT;
  let archivePath = env.ANIMA_WORLD_V2_TILES_ARCHIVE ?? '';
  let sourceDir = env.ANIMA_WORLD_V2_TILES_DIR ?? '';
  let outDir = '';
  let verifyOnly = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root' && argv[i + 1]) {
      repoRoot = resolve(argv[++i]);
    } else if (arg === '--archive' && argv[i + 1]) {
      archivePath = argv[++i];
    } else if (arg === '--dir' && argv[i + 1]) {
      sourceDir = argv[++i];
    } else if (arg === '--out' && argv[i + 1]) {
      outDir = resolve(argv[++i]);
    } else if (arg === '--verify-only') {
      verifyOnly = true;
    } else if (arg === '--help' || arg === '-h') {
      console.log(
        'Usage: node scripts/fetch_world_v2_tiles.mjs [--archive PATH | --dir PATH | --verify-only] [--out DIR]',
      );
      return { verified: 0, outDir: '' };
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  const targetDir = outDir || join(repoRoot, 'public', 'world-v2');
  const { index, indexBytes, simBytes, bundleIdentity } = verifyIndexAndSim256(repoRoot);

  if (verifyOnly) {
    const count = verifyStagedTiles(index, targetDir);
    const stagedIdxPath = join(targetDir, 'index.json');
    if (existsSync(stagedIdxPath)) {
      const stagedIdxSha = sha256Hex(readFileSync(stagedIdxPath));
      if (stagedIdxSha !== bundleIdentity) {
        throw new Error(
          `Staged index.json SHA-256 mismatch: expected ${bundleIdentity}, got ${stagedIdxSha}`,
        );
      }
    }
    console.log(`fetch_world_v2_tiles: verified ${count} staged entries (bundle=${bundleIdentity.slice(0, 8)})`);
    return { verified: count, outDir: targetDir, bundleIdentity };
  }

  if (archivePath) {
    const absArchive = resolve(archivePath);
    const info = verifyArchiveSha256(absArchive, index.release_archive_sha256);
    assertArchiveExcludesIndexJson(absArchive);
    rmSync(targetDir, { recursive: true, force: true });
    mkdirSync(targetDir, { recursive: true });
    execFileSync('tar', ['-xzmf', absArchive, '-C', targetDir], { stdio: 'inherit' });
    copyCheckedCoreFiles(indexBytes, simBytes, targetDir);
    const count = verifyStagedTiles(index, targetDir);
    console.log(
      `fetch_world_v2_tiles: extracted & verified ${count} entries from ${absArchive} (${info.byteLength} B, bundle=${bundleIdentity.slice(0, 8)})`,
    );
    return { verified: count, outDir: targetDir, bundleIdentity };
  }

  if (sourceDir) {
    const absSrc = resolve(sourceDir);
    if (absSrc !== targetDir) {
      rmSync(targetDir, { recursive: true, force: true });
      mkdirSync(targetDir, { recursive: true });
      cpSync(absSrc, targetDir, { recursive: true });
    }
    copyCheckedCoreFiles(indexBytes, simBytes, targetDir);
    const count = verifyStagedTiles(index, targetDir);
    console.log(
      `fetch_world_v2_tiles: staged & verified ${count} entries from dir ${absSrc} (bundle=${bundleIdentity.slice(0, 8)})`,
    );
    return { verified: count, outDir: targetDir, bundleIdentity };
  }

  // If already staged and 100% verified (including bundle_identity), skip download cleanly.
  if (existsSync(targetDir) && existsSync(join(targetDir, 'index.json'))) {
    const stagedIdxSha = sha256Hex(readFileSync(join(targetDir, 'index.json')));
    if (stagedIdxSha === bundleIdentity) {
      const count = verifyStagedTiles(index, targetDir);
      console.log(
        `fetch_world_v2_tiles: already staged & verified ${count} entries (bundle=${bundleIdentity.slice(0, 8)})`,
      );
      return { verified: count, outDir: targetDir, bundleIdentity };
    }
  }

  // Otherwise attempt download from named release URL constant.
  const url = resolveArchiveUrl(index.release_archive);
  const tmpArchive = join(repoRoot, 'artifacts', 'world-v2', '.tmp-world-v2-tiles.tar.gz');
  try {
    await downloadReleaseArchive(url, tmpArchive);
    verifyArchiveSha256(tmpArchive, index.release_archive_sha256);
    assertArchiveExcludesIndexJson(tmpArchive);
    rmSync(targetDir, { recursive: true, force: true });
    mkdirSync(targetDir, { recursive: true });
    execFileSync('tar', ['-xzmf', tmpArchive, '-C', targetDir], { stdio: 'inherit' });
    copyCheckedCoreFiles(indexBytes, simBytes, targetDir);
    const count = verifyStagedTiles(index, targetDir);
    console.log(
      `fetch_world_v2_tiles: downloaded & verified ${count} entries from ${url} (bundle=${bundleIdentity.slice(0, 8)})`,
    );
    return { verified: count, outDir: targetDir, bundleIdentity };
  } finally {
    rmSync(tmpArchive, { force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  runFetchWorldV2Tiles().catch((err) => {
    console.error(`fetch_world_v2_tiles FAILED: ${err instanceof Error ? err.message : String(err)}`);
    process.exit(1);
  });
}
