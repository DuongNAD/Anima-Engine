// prototype, to be replaced — unit tests for geomorphed 20 m L0 near-camera terrain patch.

import { describe, expect, it } from 'vitest';
import { L0HeightTileCache } from './l0HeightCache';
import {
  buildNearPatchBuffers,
  buildNearPatchIndices,
  clearActiveNearPatchState,
  sampleActiveNearPatchY,
  sampleCoarseTriangleAttr,
  setActiveNearPatchState,
  updateCoarseCutoutIndices,
} from './terrainNearPatch';
import { TILE_ACTIVE_SIZE, WaterKind, type L0HeightManifest, type TileManifestEntry, type WorldV2LoadedBundle } from './types';
import type { World } from '../utils/worldGen';

describe('terrainNearPatch', () => {
  it('welds perimeter heights to coarse mesh, cuts coarse hole, and adopts inner L0 heights', () => {
    const meshRes = 8;
    const verts = meshRes + 1;
    const renderSize = 100;
    const heightUnits = 20;
    const coarsePos = new Float32Array(verts * verts * 3);
    const coarseNrm = new Float32Array(verts * verts * 3);
    for (let z = 0; z <= meshRes; z += 1) {
      for (let x = 0; x <= meshRes; x += 1) {
        const i = z * verts + x;
        coarsePos[i * 3] = (x / meshRes - 0.5) * renderSize;
        coarsePos[i * 3 + 1] = 10 + (x + z) * 0.25;
        coarsePos[i * 3 + 2] = (z / meshRes - 0.5) * renderSize;
        coarseNrm[i * 3] = 0;
        coarseNrm[i * 3 + 1] = 1;
        coarseNrm[i * 3 + 2] = 0;
      }
    }

    const rSize = 33;
    const bundle = {
      size: rSize,
      extentM: 40960,
      waterKind: new Uint8Array(rSize * rSize),
      heightM: new Float32Array(rSize * rSize).fill(1000),
      waterHeightM: new Float32Array(rSize * rSize),
      hydroFlowRgba: new Uint8Array(rSize * rSize * 4),
      normalRg: new Uint8Array(rSize * rSize * 2).fill(128),
      sim: { waterfalls: [] },
    } as unknown as WorldV2LoadedBundle;

    const wSize = 16;
    const world = {
      size: wSize,
      elevation: new Float32Array(wSize * wSize).fill(0.5),
      waterfallCount: 0,
    } as unknown as World;

    const entries = new Map<string, TileManifestEntry>([
      [
        '0:0',
        { layer: 'height', mip: 0, tx: 0, ty: 0, path: 'h', byte_len: 1, sha256: '00'.repeat(32) },
      ],
    ]);
    const manifest: L0HeightManifest = { tilesBaseUrl: '/x', gridSize: TILE_ACTIVE_SIZE, tilesPerSide: 1, entries };
    const l0Cache = new L0HeightTileCache(manifest);
    const tileH = new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE).fill(3500);
    l0Cache.setDecodedTile(0, 0, tileH);

    const coarseSpan = 4;
    const subDiv = 4;
    const patchRes = coarseSpan * subDiv;
    const { vertCount } = buildNearPatchIndices(patchRes);
    const outPos = new Float32Array(vertCount * 3);
    const outUvs = new Float32Array(vertCount * 2);
    const outNrm = new Float32Array(vertCount * 3);
    const outTex = new Float32Array((patchRes + 1) * (patchRes + 1) * 4);

    buildNearPatchBuffers({
      gx0: 2,
      gz0: 2,
      coarseSpan,
      subDiv,
      meshRes,
      renderSize,
      heightUnits,
      world,
      bundle,
      coarsePositions: coarsePos,
      coarseNormals: coarseNrm,
      l0Cache,
      outPositions: outPos,
      outUvs,
      outNormals: outNrm,
      outTexData: outTex,
    });

    const side = patchRes + 1;
    // Perimeter vertices must match coarse mesh triangles identically
    for (let k = 0; k <= patchRes; k += 1) {
      const topIdx = k;
      const botIdx = patchRes * side + k;
      expect(outPos[topIdx * 3 + 1]).toBeCloseTo(
        sampleCoarseTriangleAttr(coarsePos, 3, 1, meshRes, outUvs[topIdx * 2], outUvs[topIdx * 2 + 1]),
        5,
      );
      expect(outPos[botIdx * 3 + 1]).toBeCloseTo(
        sampleCoarseTriangleAttr(coarsePos, 3, 1, meshRes, outUvs[botIdx * 2], outUvs[botIdx * 2 + 1]),
        5,
      );
    }

    // Center vertex must sit at full L0 elevation (~((3500 - -1500)/7200)*20 = 13.8889)
    const mid = (patchRes >> 1) * side + (patchRes >> 1);
    expect(outPos[mid * 3 + 1]).toBeCloseTo(((3500 - -1500) / 7200) * heightUnits, 3);

    // Coarse cutout index count omits the coarseSpan x coarseSpan quads
    const dstIdx = new Uint32Array(meshRes * meshRes * 6);
    const activeCount = updateCoarseCutoutIndices(dstIdx, meshRes, 2, 2, 6, 6);
    expect(activeCount).toBe((meshRes * meshRes - coarseSpan * coarseSpan) * 6);

    // Shared active near-patch CPU sampler matches interior height
    setActiveNearPatchState(null, [-25, -25, 25, 25], patchRes, outTex);
    expect(sampleActiveNearPatchY(0, 0)).toBeCloseTo(outPos[mid * 3 + 1], 4);
    expect(sampleActiveNearPatchY(40, 40)).toBeNull();
    clearActiveNearPatchState();
  });

  it('carves a submerged river bed below outTexData.a waterY and asymmetric bend banks', () => {
    const meshRes = 8;
    const verts = meshRes + 1;
    const renderSize = 100;
    const heightUnits = 20;
    const elev3500 = ((3500 - -1500) / 7200) * heightUnits;
    const coarsePos = new Float32Array(verts * verts * 3);
    const coarseNrm = new Float32Array(verts * verts * 3);
    for (let z = 0; z <= meshRes; z += 1) {
      for (let x = 0; x <= meshRes; x += 1) {
        const i = z * verts + x;
        coarsePos[i * 3] = (x / meshRes - 0.5) * renderSize;
        coarsePos[i * 3 + 1] = elev3500;
        coarsePos[i * 3 + 2] = (z / meshRes - 0.5) * renderSize;
        coarseNrm[i * 3 + 1] = 1;
      }
    }

    const rSize = 33;
    const waterKind = new Uint8Array(rSize * rSize);
    const heightM = new Float32Array(rSize * rSize).fill(3500);
    const waterHeightM = new Float32Array(rSize * rSize).fill(3500);
    const hydroFlowRgba = new Uint8Array(rSize * rSize * 4);
    const normalRg = new Uint8Array(rSize * rSize * 2).fill(128);
    for (let x = 4; x <= 28; x += 1) {
      const ci = 16 * rSize + x;
      waterKind[ci] = WaterKind.River;
      hydroFlowRgba[ci * 4] = 255;
      hydroFlowRgba[ci * 4 + 1] = 128;
      hydroFlowRgba[ci * 4 + 2] = 55;
      hydroFlowRgba[ci * 4 + 3] = 200;
      heightM[ci] = 3500 - x * 0.5;
    }

    const bundle = {
      size: rSize,
      extentM: 2560,
      waterKind,
      heightM,
      waterHeightM,
      hydroFlowRgba,
      normalRg,
      sim: { waterfalls: [] },
    } as unknown as WorldV2LoadedBundle;

    const wSize = 16;
    const world = {
      size: wSize,
      elevation: new Float32Array(wSize * wSize).fill((3500 - -1500) / 7200),
      waterfallCount: 0,
    } as unknown as World;

    const entries = new Map<string, TileManifestEntry>([
      ['0:0', { layer: 'height', mip: 0, tx: 0, ty: 0, path: 'h', byte_len: 1, sha256: '00'.repeat(32) }],
    ]);
    const manifest: L0HeightManifest = { tilesBaseUrl: '/x', gridSize: TILE_ACTIVE_SIZE, tilesPerSide: 1, entries };
    const l0Cache = new L0HeightTileCache(manifest);
    l0Cache.setDecodedTile(0, 0, new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE).fill(3500));

    const coarseSpan = 4;
    const subDiv = 4;
    const patchRes = coarseSpan * subDiv;
    const { vertCount } = buildNearPatchIndices(patchRes);
    const outPos = new Float32Array(vertCount * 3);
    const outUvs = new Float32Array(vertCount * 2);
    const outNrm = new Float32Array(vertCount * 3);
    const outTex = new Float32Array((patchRes + 1) * (patchRes + 1) * 4);

    buildNearPatchBuffers({
      gx0: 2,
      gz0: 2,
      coarseSpan,
      subDiv,
      meshRes,
      renderSize,
      heightUnits,
      world,
      bundle,
      coarsePositions: coarsePos,
      coarseNormals: coarseNrm,
      l0Cache,
      outPositions: outPos,
      outUvs,
      outNormals: outNrm,
      outTexData: outTex,
    });

    const side = patchRes + 1;
    const mid = (patchRes >> 1) * side + (patchRes >> 1);
    const bedY = outTex[mid * 4];
    const waterY = outTex[mid * 4 + 3];
    // Bed at centerline is strictly submerged below waterY, and waterY is strictly below dry bank height
    expect(waterY).toBeLessThan(elev3500);
    expect(bedY).toBeLessThan(waterY);
  });
});
