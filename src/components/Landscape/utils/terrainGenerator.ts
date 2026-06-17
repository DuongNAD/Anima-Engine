/**
 * Seeded Mulberry32 pseudo-random number generator.
 */
export function mulberry32(a: number): () => number {
  return function () {
    let t = (a += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * FNV-1a hash function to convert seed strings to 32-bit integers.
 */
export function hashString(str: string): number {
  let hash = 2166136261;
  for (let i = 0; i < str.length; i++) {
    hash = Math.imul(hash ^ str.charCodeAt(i), 16777619);
  }
  return hash >>> 0;
}

/**
 * Seeded 2D Improved Perlin Noise class.
 */
export class ImprovedNoise2D {
  private p: number[] = new Array(512);

  constructor(seed: number | string) {
    const seedNum = typeof seed === 'string' ? hashString(seed) : seed;
    const random = mulberry32(seedNum);
    const permutation = Array.from({ length: 256 }, (_, i) => i);

    // Fisher-Yates Shuffle
    for (let i = 255; i > 0; i--) {
      const j = Math.floor(random() * (i + 1));
      const temp = permutation[i];
      permutation[i] = permutation[j];
      permutation[j] = temp;
    }

    for (let i = 0; i < 256; i++) {
      this.p[i] = permutation[i];
      this.p[256 + i] = permutation[i];
    }
  }

  private fade(t: number): number {
    return t * t * t * (t * (t * 6 - 15) + 10);
  }

  private lerp(t: number, a: number, b: number): number {
    return a + t * (b - a);
  }

  private grad2d(hash: number, x: number, y: number): number {
    // 8 symmetric 2D unit vectors
    const h = hash & 7;
    switch (h) {
      case 0: return x;
      case 1: return -x;
      case 2: return y;
      case 3: return -y;
      case 4: return 0.707 * (x + y);
      case 5: return 0.707 * (-x + y);
      case 6: return 0.707 * (x - y);
      case 7: return 0.707 * (-x - y);
    }
    return 0;
  }

  /**
   * Generates 2D noise in the range [-1.0, 1.0].
   */
  public noise(x: number, y: number): number {
    const X = Math.floor(x) & 255;
    const Y = Math.floor(y) & 255;

    const xf = x - Math.floor(x);
    const yf = y - Math.floor(y);

    const u = this.fade(xf);
    const v = this.fade(yf);

    const aa = this.p[this.p[X] + Y];
    const ab = this.p[this.p[X] + Y + 1];
    const ba = this.p[this.p[X + 1] + Y];
    const bb = this.p[this.p[X + 1] + Y + 1];

    const x1 = this.lerp(u, this.grad2d(aa, xf, yf), this.grad2d(ba, xf - 1, yf));
    const x2 = this.lerp(u, this.grad2d(ab, xf, yf - 1), this.grad2d(bb, xf - 1, yf - 1));

    const rawVal = this.lerp(v, x1, x2);
    // Scale by 1/0.707 to normalize to roughly [-1.0, 1.0]
    return Math.max(-1, Math.min(1, rawVal / 0.707));
  }
}

/**
 * Fractional Brownian Motion (layered noise).
 */
export function fbm(
  noiseGen: ImprovedNoise2D,
  x: number,
  y: number,
  octaves: number,
  persistence: number,
  lacunarity: number
): number {
  let total = 0;
  let frequency = 1;
  let amplitude = 1;
  let maxValue = 0;

  for (let i = 0; i < octaves; i++) {
    total += noiseGen.noise(x * frequency, y * frequency) * amplitude;
    maxValue += amplitude;
    amplitude *= persistence;
    frequency *= lacunarity;
  }

  return total / maxValue;
}

export type BiomeType = 'ocean' | 'beach' | 'grassland' | 'forest' | 'taiga' | 'alpine rock' | 'snow peaks';

export interface TerrainCell {
  x: number;
  y: number;
  elevation: number; // 0 to 100
  moisture: number;  // 0 to 100
  biome: BiomeType;
  isRiver: boolean | number;
  isLake: boolean;
  isWaterfall: boolean;
  waterY?: number;
}

export interface FloraPlacement {
  x: number;
  y: number;
  type: 'Oak' | 'Pine' | 'Bush' | 'Rock' | 'Palm' | 'Cactus' | 'Jungle' | 'Birch' | 'Flowers';
  scale: number;
}

export interface TerrainData {
  width: number;
  height: number;
  seed: string | number;
  grid: TerrainCell[][];
  flora: FloraPlacement[];
}

export function getBilinearInterpolatedElevation(px: number, py: number, width: number, height: number, grid: TerrainCell[][]): number {
  const x0 = Math.floor(px);
  const y0 = Math.floor(py);
  const x1 = Math.min(width - 1, x0 + 1);
  const y1 = Math.min(height - 1, y0 + 1);
  const tx = px - x0;
  const ty = py - y0;
  const h00 = grid[y0]?.[x0]?.elevation ?? 0;
  const h10 = grid[y0]?.[x1]?.elevation ?? 0;
  const h01 = grid[y1]?.[x0]?.elevation ?? 0;
  const h11 = grid[y1]?.[x1]?.elevation ?? 0;
  return h00 * (1 - tx) * (1 - ty) + h10 * tx * (1 - ty) + h01 * (1 - tx) * ty + h11 * tx * ty;
}

/**
 * Determines the biome based on elevation and moisture thresholds.
 */
export function determineBiome(elevation: number, moisture: number): BiomeType {
  if (elevation < 3.0) {
    return 'ocean';
  }
  if (elevation < 5.0) {
    return 'beach';
  }
  if (elevation >= 80) {
    return 'snow peaks';
  }
  if (elevation >= 60) {
    return moisture >= 50 ? 'taiga' : 'alpine rock';
  }
  return moisture >= 45 ? 'forest' : 'grassland';
}

/**
 * Bridson's Poisson-disk sampling algorithm.
 */
export function poissonDiskSampling(
  width: number,
  height: number,
  r: number,
  k: number,
  random: () => number
): { x: number; y: number }[] {
  if (width <= 0 || height <= 0 || r <= 0) return [];
  const cellSize = r / Math.sqrt(2);
  const gridWidth = Math.ceil(width / cellSize);
  const gridHeight = Math.ceil(height / cellSize);

  const grid: number[] = new Array(gridWidth * gridHeight).fill(-1);
  const points: { x: number; y: number }[] = [];
  const active: number[] = [];

  function insertPoint(p: { x: number; y: number }) {
    points.push(p);
    const idx = points.length - 1;
    const gx = Math.floor(p.x / cellSize);
    const gy = Math.floor(p.y / cellSize);
    if (gx >= 0 && gx < gridWidth && gy >= 0 && gy < gridHeight) {
      grid[gy * gridWidth + gx] = idx;
    }
    active.push(idx);
  }

  // Initial random point
  const p0 = { x: random() * width, y: random() * height };
  insertPoint(p0);

  while (active.length > 0) {
    const activeIdx = Math.floor(random() * active.length);
    const pointIdx = active[activeIdx];
    const p = points[pointIdx];

    let found = false;
    for (let i = 0; i < k; i++) {
      const angle = random() * Math.PI * 2;
      const distance = r + random() * r; // Range [r, 2r]
      const candidate = {
        x: p.x + Math.cos(angle) * distance,
        y: p.y + Math.sin(angle) * distance,
      };

      if (candidate.x >= 0 && candidate.x < width && candidate.y >= 0 && candidate.y < height) {
        const gx = Math.floor(candidate.x / cellSize);
        const gy = Math.floor(candidate.y / cellSize);

        let tooClose = false;
        const startX = Math.max(0, gx - 2);
        const endX = Math.min(gridWidth - 1, gx + 2);
        const startY = Math.max(0, gy - 2);
        const endY = Math.min(gridHeight - 1, gy + 2);

        for (let ny = startY; ny <= endY; ny++) {
          for (let nx = startX; nx <= endX; nx++) {
            const neighborIdx = grid[ny * gridWidth + nx];
            if (neighborIdx !== -1) {
              const np = points[neighborIdx];
              const dx = candidate.x - np.x;
              const dy = candidate.y - np.y;
              if (dx * dx + dy * dy < r * r) {
                tooClose = true;
                break;
              }
            }
          }
          if (tooClose) break;
        }

        if (!tooClose) {
          insertPoint(candidate);
          found = true;
          break;
        }
      }
    }

    if (!found) {
      active.splice(activeIdx, 1);
    }
  }

  return points;
}

/**
 * Generates the complete terrain data including heightmap, biomes, hydrology, and flora.
 */
function gaussSmooth(arr: Float32Array, width: number, height: number, n: number) {
  const tot = width * height;
  const tmp = new Float32Array(tot);
  for (let p = 0; p < n; p++) {
    for (let y = 1; y < height - 1; y++) {
      for (let x = 1; x < width - 1; x++) {
        const i = y * width + x;
        tmp[i] =
          arr[i] * 0.36 +
          (arr[i - 1] + arr[i + 1] + arr[i - width] + arr[i + width]) * 0.12 +
          (arr[i - 1 - width] +
            arr[i + 1 - width] +
            arr[i - 1 + width] +
            arr[i + 1 + width]) *
            0.04;
      }
    }
    for (let y = 1; y < height - 1; y++) {
      for (let x = 1; x < width - 1; x++) {
        arr[y * width + x] = tmp[y * width + x];
      }
    }
  }
}

/**
 * Generates the complete terrain data including heightmap, biomes, hydrology, and flora.
 */
export function generateTerrain(
  width: number,
  height: number,
  seed: string | number
): TerrainData {
  const baseSeed = typeof seed === 'number' ? seed : hashString(seed);

  // Initialize decorellated noise generators
  const elevationNoise = new ImprovedNoise2D(baseSeed);
  const tempNoise = new ImprovedNoise2D(baseSeed + 6000);
  const moistureNoiseLocal = new ImprovedNoise2D(baseSeed + 7000);
  const lakeNoise = new ImprovedNoise2D(baseSeed + 5000);

  // Initialize seeded random generator for vegetation/sampling
  const random = mulberry32(baseSeed);

  // PASS 1: Elevation with peak boosts
  const terrElev = new Float32Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      const nx = width > 1 ? (2 * x) / (width - 1) - 1 : 0;
      const nz = height > 1 ? (2 * y) / (height - 1) - 1 : 0;

      let e = elevationNoise.noise(nx * 2.5, nz * 2.5) +
              0.5 * elevationNoise.noise(nx * 5, nz * 5) +
              0.25 * elevationNoise.noise(nx * 10, nz * 10) +
              0.1 * elevationNoise.noise(nx * 20, nz * 20);
      e = (e / 1.85 + 1) / 2;

      const dist = Math.sqrt(nx * nx + nz * nz);
      e *= Math.max(0, 1 - Math.pow(dist * 1.8, 1.8));

      const ridge = Math.abs(elevationNoise.noise(nx * 3 + 5, nz * 3 + 5));
      const ridgeBoost = Math.pow(ridge, 1.2) * Math.max(0, 1 - dist * 3);
      e += ridgeBoost * 0.4;

      const rg = elevationNoise.noise(nx * 1.5 + 7, nz * 1.5 + 3);
      let factor = 1.0;
      if (rg < -0.2) {
        const t = Math.min(1.0, Math.max(0.0, (-0.2 - rg) / 0.2));
        factor = 1.0 - t * 0.5;
      } else if (rg > 0.2) {
        const t = Math.min(1.0, Math.max(0.0, (rg - 0.2) / 0.2));
        factor = 1.0 + t * 0.5;
      }
      e *= factor;

      terrElev[i] = Math.pow(Math.max(0, e), 1.15) * 180;
    }
  }

  // Smooth terrain first
  gaussSmooth(terrElev, width, height, 1);

  // PASS 2: Basin / Lakes detection
  const basins: { x: number; y: number; el: number }[] = [];
  const borderX = Math.floor(width * 0.075);
  const borderY = Math.floor(height * 0.075);

  const sr = Math.max(1, Math.floor(width * 0.02));
  for (let y = borderY; y < height - borderY; y += 4) {
    for (let x = borderX; x < width - borderX; x += 4) {
      const i = y * width + x;
      const el = terrElev[i];
      if (el < 25 || el > 80) continue;

      let higherCount = 0;
      let totalCount = 0;
      for (let dy = -sr; dy <= sr; dy++) {
        for (let dx = -sr; dx <= sr; dx++) {
          const ny = y + dy;
          const nx2 = x + dx;
          if (ny < 0 || ny >= height || nx2 < 0 || nx2 >= width) continue;
          if (dx === 0 && dy === 0) continue;
          if (terrElev[ny * width + nx2] > el + 0.5) higherCount++;
          totalCount++;
        }
      }
      if (higherCount >= totalCount * 0.7) {
        basins.push({ x, y, el });
      }
    }
  }

  basins.sort((a, b) => a.el - b.el);
  const topBasins: typeof basins = [];
  for (const b of basins) {
    let far = true;
    for (const tb of topBasins) {
      const dx = b.x - tb.x;
      const dy = b.y - tb.y;
      const minDist = Math.max(10, Math.floor(width * 0.175));
      if (Math.sqrt(dx * dx + dy * dy) < minDist) {
        far = false;
        break;
      }
    }
    if (far) {
      topBasins.push(b);
      if (topBasins.length >= 4) break;
    }
  }

  let selectedCenters: typeof basins = [];
  if (topBasins.length > 0) {
    selectedCenters = topBasins;
  } else {
    // Fallback to peaks
    const peaks: typeof basins = [];
    for (let y = borderY; y < height - borderY; y += 3) {
      for (let x = borderX; x < width - borderX; x += 3) {
        const i = y * width + x;
        if (terrElev[i] < 30) continue;
        let pk = true;
        for (let dy = -sr; dy <= sr && pk; dy++) {
          for (let dx = -sr; dx <= sr && pk; dx++) {
            if (!dy && !dx) continue;
            const ny = y + dy;
            const nx2 = x + dx;
            if (ny < 0 || ny >= height || nx2 < 0 || nx2 >= width) continue;
            if (terrElev[ny * width + nx2] > terrElev[i]) pk = false;
          }
        }
        if (pk) peaks.push({ x, y, el: terrElev[i] });
      }
    }
    peaks.sort((a, b) => b.el - a.el);
    const topPeaks = peaks.slice(0, Math.min(4, peaks.length));
    for (const pk of topPeaks) {
      const lx = Math.max(borderX, Math.min(width - borderX, pk.x + Math.round(lakeNoise.noise(pk.x * 0.1, pk.y * 0.1) * 4)));
      const ly = Math.max(borderY, Math.min(height - borderY, pk.y + Math.round(lakeNoise.noise(pk.y * 0.1, pk.x * 0.1) * 4)));
      selectedCenters.push({ x: lx, y: ly, el: terrElev[ly * width + lx] });
    }
  }

  const lakes: { x: number; y: number; waterY: number; r: number; rx: number; ry: number }[] = [];
  for (const center of selectedCenters) {
    const lx = center.x;
    const ly = center.y;
    const baseR = Math.max(3, Math.floor(width * 0.045));
    const lr = baseR + Math.floor(Math.abs(lakeNoise.noise(lx * 0.05, ly * 0.05)) * (baseR * 0.7));

    let minBoundaryHeight = Infinity;
    let rx = lx;
    let ry = ly + lr + 1;
    for (let dy = -lr - 2; dy <= lr + 2; dy++) {
      for (let dx = -lr - 2; dx <= lr + 2; dx++) {
        const cx = lx + dx;
        const cy = ly + dy;
        if (cx < 0 || cx >= width || cy < 0 || cy >= height) continue;
        const d = Math.sqrt(dx * dx + dy * dy);
        if (d >= lr && d <= lr + 2.5) {
          const h = terrElev[cy * width + cx];
          if (h < minBoundaryHeight) {
            minBoundaryHeight = h;
            rx = cx;
            ry = cy;
          }
        }
      }
    }

    const waterY = minBoundaryHeight - 1.0;
    if (waterY <= 6.0) continue;

    // Carve basin
    for (let dy = -lr - 4; dy <= lr + 4; dy++) {
      for (let dx = -lr - 4; dx <= lr + 4; dx++) {
        const cx = lx + dx;
        const cy = ly + dy;
        if (cx < 0 || cx >= width || cy < 0 || cy >= height) continue;
        const ci = cy * width + cx;
        const d = Math.sqrt(dx * dx + dy * dy);
        if (d <= lr) {
          terrElev[ci] = waterY - 2.0;
        } else if (d <= lr + 4) {
          const t = (d - lr) / 4;
          terrElev[ci] = (waterY - 2.0) * (1 - t) + terrElev[ci] * t;
          terrElev[ci] = Math.max(terrElev[ci], waterY + 0.3 * t);
        }
      }
    }
    lakes.push({ x: lx, y: ly, waterY, r: lr, rx, ry });
  }

  // PASS 3: Ponds detection (local valleys)
  const isRiver = new Uint8Array(width * height);
  for (let y = borderY; y < height - borderY; y += 8) {
    for (let x = borderX; x < width - borderX; x += 8) {
      const i = y * width + x;
      const el = terrElev[i];
      if (el < 10 || el > 35) continue;

      let lowerCount = 0;
      for (let dy = -3; dy <= 3; dy += 3) {
        for (let dx = -3; dx <= 3; dx += 3) {
          if (!dx && !dy) continue;
          const nx2 = x + dx;
          const ny2 = y + dy;
          if (nx2 < 0 || nx2 >= width || ny2 < 0 || ny2 >= height) continue;
          const ni = ny2 * width + nx2;
          if (terrElev[ni] > el + 1.0) lowerCount++;
        }
      }
      if (lowerCount < 5) continue;

      const pr = 2 + Math.floor(random() * 3);
      for (let dy = -pr; dy <= pr; dy++) {
        for (let dx = -pr; dx <= pr; dx++) {
          const cx = x + dx;
          const cy = y + dy;
          if (cx < 0 || cx >= width || cy < 0 || cy >= height) continue;
          const ci = cy * width + cx;
          const d = Math.sqrt(dx * dx + dy * dy);
          if (d <= pr && isRiver[ci] === 0) {
            terrElev[ci] = Math.max(5.5, el - 1.5);
            isRiver[ci] = 3; // Pond
          }
        }
      }
    }
  }

  // PASS 4: Rivers routed downhill starting from lake spillways
  for (const lk of lakes) {
    let cx = lk.rx;
    let cy = lk.ry;
    if (cx < 3 || cx >= width - 3 || cy < 3 || cy >= height - 3) continue;
    for (let s = 0; s < 400; s++) {
      if (cx < 3 || cx >= width - 3 || cy < 3 || cy >= height - 3) break;
      const ci = cy * width + cx;
      const ce = terrElev[ci];
      if (ce <= 6.0) break;

      let bx = cx;
      let by = cy;
      let be = ce;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          if (!dx && !dy) continue;
          const nx2 = cx + dx;
          const ny2 = cy + dy;
          if (nx2 < 0 || nx2 >= width || ny2 < 0 || ny2 >= height) continue;
          const ne = terrElev[ny2 * width + nx2];
          if (ne < be) {
            be = ne;
            bx = nx2;
            by = ny2;
          }
        }
      }
      if (bx === cx && by === cy) {
        bx = Math.max(3, Math.min(width - 4, cx + (cx < width / 2 ? -1 : 1)));
        by = Math.max(3, Math.min(height - 4, cy + (cy < height / 2 ? -1 : 1)));
        terrElev[by * width + bx] = ce - 0.5;
      }
      const drop = ce - be;
      const baseRw = ce > 42 ? 3 : ce > 25 ? 5 : 6;
      const rw = Math.max(1, Math.round(baseRw * (width / 200)));
      const dropThreshold = Math.max(0.1, Math.min(2.0, 2.0 * (200 / Math.max(width, height))));
      for (let dy = -rw; dy <= rw; dy++) {
        for (let dx = -rw; dx <= rw; dx++) {
          const rx = cx + dx;
          const ry = cy + dy;
          if (rx < 0 || rx >= width || ry < 0 || ry >= height) continue;
          const ri = ry * width + rx;
          const d = Math.sqrt(dx * dx + dy * dy);
          if (d <= rw && isRiver[ri] === 0) {
            const profile = 1 - Math.pow(d / rw, 2);
            terrElev[ri] = Math.max(5.5, terrElev[ri] - profile * 2);
            if (d <= rw * 0.7) {
              isRiver[ri] = drop > dropThreshold ? 2 : 1;
            }
          }
        }
      }
      cx = bx;
      cy = by;
    }
  }

  // Smooth terrain again after carving
  gaussSmooth(terrElev, width, height, 1);

  // Re-flatten lake beds
  const cellIsLake = new Uint8Array(width * height);
  const cellWaterY = new Float32Array(width * height);
  for (const lk of lakes) {
    for (let dy = -lk.r; dy <= lk.r; dy++) {
      for (let dx = -lk.r; dx <= lk.r; dx++) {
        const cx = lk.x + dx;
        const cy = lk.y + dy;
        if (cx < 0 || cx >= width || cy < 0 || cy >= height) continue;
        if (Math.sqrt(dx * dx + dy * dy) <= lk.r) {
          const ci = cy * width + cx;
          terrElev[ci] = lk.waterY - 1.5;
          cellIsLake[ci] = 1;
          cellWaterY[ci] = lk.waterY;
        }
      }
    }
  }

  // Calculate moisture and temperature arrays matching ecosystem.html scales
  const temperature = new Float32Array(width * height);
  const moisture = new Float32Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      const nx = width > 1 ? (2 * x) / (width - 1) - 1 : 0;
      const nz = height > 1 ? (2 * y) / (height - 1) - 1 : 0;
      const el = terrElev[i];

      temperature[i] = tempNoise.noise(nx * 2, nz * 2) * 0.4 + (nx + nz + 1) * 0.5 - el * 0.005;

      let m = (moistureNoiseLocal.noise(nx * 3, nz * 3) + 0.5 * moistureNoiseLocal.noise(nx * 6, nz * 6)) / 1.5;
      if (el < 12) m += 0.3;
      moisture[i] = m;
    }
  }

  // Populate grid cells (with elevation scaled down to [0, 100])
  const grid: TerrainCell[][] = [];
  for (let y = 0; y < height; y++) {
    const row: TerrainCell[] = [];
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      const rawEl = terrElev[i] / 1.8;
      const rawMoist = Math.max(0, Math.min(100, (moisture[i] + 0.8) * 55));

      row.push({
        x,
        y,
        elevation: rawEl,
        moisture: rawMoist,
        biome: determineBiome(rawEl, rawMoist),
        isRiver: isRiver[i] > 0 ? isRiver[i] : false,
        isLake: cellIsLake[i] === 1,
        isWaterfall: isRiver[i] === 2,
        waterY: cellIsLake[i] === 1 ? cellWaterY[i] / 1.8 : undefined,
      });
    }
    grid.push(row);
  }

  // PASS 5: Vegetation placements considering temperature/moisture rules and lake proximity
  const flora: FloraPlacement[] = [];
  const candidates = poissonDiskSampling(width, height, 2.2, 30, random);

  for (const p of candidates) {
    const cx = Math.floor(p.x);
    const cy = Math.floor(p.y);

    if (cx >= 1 && cx < width - 1 && cy >= 1 && cy < height - 1) {
      const ci = cy * width + cx;
      const el = terrElev[ci];
      const tmp = temperature[ci];
      const mst = moisture[ci];

      if (isRiver[ci] > 0 || el <= 5.5 || (el / 1.8) < 3.0) continue;

      let nearWater = false;
      for (const lk of lakes) {
        const dx = cx - lk.x;
        const dy = cy - lk.y;
        if (Math.sqrt(dx * dx + dy * dy) < lk.r + 2) {
          nearWater = true;
          break;
        }
      }
      if (nearWater) continue;

      const sl = Math.max(
        Math.abs(el - terrElev[ci + 1]),
        Math.abs(el - terrElev[ci - 1]),
        Math.abs(el - terrElev[ci - width]),
        Math.abs(el - terrElev[ci + width])
      );
      const maxSlope = 6 * (200 / Math.min(width, height));
      if (sl > maxSlope) continue;

      const r = random();
      const sc = 0.6 + random() * 0.8;
      let type: FloraPlacement['type'] | null = null;

      if (tmp > 0.5 && mst < -0.3) {
        if (r < 0.03) type = 'Cactus';
        else if (r < 0.05) type = 'Rock';
      } else if (tmp > 0.5 && mst < 0.1) {
        if (r < 0.015) type = 'Oak';
      } else if (tmp > 0.5) {
        if ((el >= 50 || mst > 0.5) && r < 0.2) type = 'Jungle';
        else if (r < 0.06) type = 'Palm';
      } else if (tmp <= 0.1 && mst > 0.3 && el < 50) {
        if (r < 0.12) type = 'Birch';
      } else if (el >= 55 && el < 85) {
        if (r < 0.16) type = 'Pine';
      } else if (tmp > 0.1 && mst > 0.4 && el > 30 && el < 55) {
        if (r < 0.15) type = 'Flowers';
      } else if (el >= 50 && el < 70 && tmp > 0.1) {
        if (r < 0.18) type = 'Oak';
      } else if (el >= 9 && el < 50) {
        if (r < 0.04) type = 'Oak';
        else if (r < 0.07) type = 'Bush';
      }

      if (!type && el >= 70 && r < 0.05) {
        type = 'Rock';
      }

      if (type) {
        flora.push({
          x: p.x,
          y: p.y,
          type,
          scale: sc,
        });
      }
    }
  }

  return {
    width,
    height,
    seed,
    grid,
    flora,
  };
}

export function generateTerrainData(width: number, height: number): Float32Array {
  const data = new Float32Array(width * height);
  const terrain = generateTerrain(width, height, 'seed');
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      data[y * width + x] = terrain.grid[y][x].elevation;
    }
  }
  return data;
}

export function getBiomeColor(elevation: number, moisture: number): { r: number; g: number; b: number } {
  const biome = determineBiome(elevation, moisture);
  switch (biome) {
    case 'ocean': return { r: 0.1, g: 0.3, b: 0.8 };
    case 'beach': return { r: 0.9, g: 0.8, b: 0.6 };
    case 'snow peaks': return { r: 0.95, g: 0.95, b: 0.95 };
    case 'taiga': return { r: 0.1, g: 0.4, b: 0.3 };
    case 'alpine rock': return { r: 0.5, g: 0.5, b: 0.5 };
    case 'forest': return { r: 0.2, g: 0.6, b: 0.2 };
    case 'grassland': return { r: 0.4, g: 0.7, b: 0.3 };
    default: return { r: 0.4, g: 0.7, b: 0.3 };
  }
}

export function generateFloraPlacements(width: number, height: number): { x: number; y: number; type: string; scale: number }[] {
  const terrain = generateTerrain(width, height, 'seed');
  return terrain.flora;
}

