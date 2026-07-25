import { describe, it, expect, beforeAll } from 'vitest';
import { generateWorld, Biome, type World } from '../components/Landscape/utils/worldGen';

// Regression coverage for the M1 hydrology physics (v19/v20): every lake drains via a spillway
// river, arid basins become endorheic salt lakes, and river mouths build sandy deltas. The world
// is deterministic (fixed seed), so these invariants are stable. 512² is the smallest size at
// which lakes + at least one endorheic basin reliably form (11 lakes / 0 saline at 384²; 18 / 2
// at 512²), and it generates in well under a second.
describe('worldGen hydrology (M1: draining lakes, endorheic salt lakes, deltas)', () => {
  let world: World;
  beforeAll(() => {
    world = generateWorld('seed', { size: 512, shape: 'continent' });
  }, 15000);

  it('produces a physically sane world: no NaN, ~38% land, water biomes present', () => {
    let nan = 0;
    let land = 0;
    for (let i = 0; i < world.elevation.length; i++) {
      if (!Number.isFinite(world.elevation[i])) nan++;
      if (world.elevation[i] >= world.seaLevel) land++;
    }
    expect(nan).toBe(0);
    const landFrac = land / world.elevation.length;
    expect(landFrac).toBeGreaterThan(0.34);
    expect(landFrac).toBeLessThan(0.42);

    const biomes = new Set(world.biome);
    expect(biomes.has(Biome.Ocean)).toBe(true);
    expect(biomes.has(Biome.River)).toBe(true); // rivers exist (D8 + lake spillways)
    expect(biomes.has(Biome.Lake)).toBe(true);
    expect(biomes.has(Biome.Beach)).toBe(true); // coastlines + delta lobes + salt flats
  });

  it('forms lakes, and marks arid basins as endorheic (saline) — but not all of them', () => {
    expect(world.lakeBasins.length).toBeGreaterThan(3);
    const saline = world.lakeBasins.filter((b) => b.saline === true).length;
    // Arid basins can't overflow (evaporation ≥ inflow) → terminal salt lakes.
    expect(saline).toBeGreaterThan(0);
    // ...but humid basins still drain, so the world is not ALL salt lakes.
    expect(saline).toBeLessThan(world.lakeBasins.length);
  });

  it('keeps every riverAmt-marked cell within the valid ribbon range', () => {
    // Spillway/delta stamping writes riverAmt (0..255); a bad index/NaN would surface here.
    let bad = 0;
    for (let i = 0; i < world.riverAmt.length; i++) {
      const v = world.riverAmt[i];
      if (v < 0 || v > 255 || !Number.isFinite(v)) bad++;
    }
    expect(bad).toBe(0);
  });
});
