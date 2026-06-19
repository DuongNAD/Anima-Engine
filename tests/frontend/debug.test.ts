import { describe, it } from 'vitest';
import { ImprovedNoise2D, determineBiome, poissonDiskSampling, mulberry32 } from '../../src/components/Landscape/utils/terrainGenerator';

describe('simulate end-to-end waterfalls', () => {
  it('runs peak search increment 1 and lake/river generation', () => {
    const width = 100;
    const height = 100;
    const seed = 'seed';
    
    function hashString(str: string): number {
      let hash = 2166136261;
      for (let i = 0; i < str.length; i++) {
        hash = Math.imul(hash ^ str.charCodeAt(i), 16777619);
      }
      return hash >>> 0;
    }

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

    const baseSeed = hashString(seed);
    const elevationNoise = new ImprovedNoise2D(baseSeed);
    const lakeNoise = new ImprovedNoise2D(baseSeed + 5000);
    const random = mulberry32(baseSeed);

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

    gaussSmooth(terrElev, width, height, 1);

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
      const peaks: typeof basins = [];
      for (let y = borderY; y < height - borderY; y += 1) {
        for (let x = borderX; x < width - borderX; x += 1) {
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
    const baseR = width <= 100 ? 2 : Math.max(3, Math.floor(width * 0.045));

    for (const center of selectedCenters) {
      const lx = center.x;
      const ly = center.y;
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

      lakes.push({ x: lx, y: ly, waterY, r: lr, rx, ry });
    }

    console.log('Lakes count:', lakes.length);

    const isRiver = new Uint8Array(width * height);
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
        const dropThreshold = 0.5; // lowered threshold
        for (let dy = -rw; dy <= rw; dy++) {
          for (let dx = -rw; dx <= rw; dx++) {
            const rx = cx + dx;
            const ry = cy + dy;
            if (rx < 0 || rx >= width || ry < 0 || ry >= height) continue;
            const ri = ry * width + rx;
            const d = Math.sqrt(dx * dx + dy * dy);
            if (d <= rw && isRiver[ri] === 0) {
              const profile = 1 - Math.pow(d / rw, 2);
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

    console.log('Waterfall cells count:', isRiver.filter(x => x === 2).length);
  });
});
