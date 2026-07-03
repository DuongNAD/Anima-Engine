// Temporary smoke harness for worldGen v11 — run via esbuild+node, delete afterwards.
// Prints generation timing + climate/biome distribution stats and writes a hillshaded
// biome preview PNG so the map can be eyeballed without a browser.
import { deflateSync } from 'zlib';
import { writeFileSync } from 'fs';
import { generateWorld, Biome, BIOME_RGB, BIOME_COUNT } from './src/components/Landscape/utils/worldGen';

const SIZE = Number(process.env.SMOKE_SIZE ?? 1024);
const OUT = process.env.SMOKE_OUT ?? 'world_v11_preview.png';

// ---- minimal PNG encoder (truecolor 8-bit, no interlace) ----
const CRC_TABLE = new Uint32Array(256).map((_, k) => {
  let c = k;
  for (let i = 0; i < 8; i++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(buf: Uint8Array): number {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.length);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, data.length);
  for (let i = 0; i < 4; i++) out[4 + i] = type.charCodeAt(i);
  out.set(data, 8);
  dv.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}
function writePng(path: string, rgb: Uint8Array, w: number, h: number): void {
  const ihdr = new Uint8Array(13);
  const dv = new DataView(ihdr.buffer);
  dv.setUint32(0, w);
  dv.setUint32(4, h);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 2; // truecolor
  const raw = new Uint8Array(h * (1 + w * 3));
  for (let y = 0; y < h; y++) raw.set(rgb.subarray(y * w * 3, (y + 1) * w * 3), y * (1 + w * 3) + 1);
  const png = Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    Buffer.from(chunk('IHDR', ihdr)),
    Buffer.from(chunk('IDAT', deflateSync(raw))),
    Buffer.from(chunk('IEND', new Uint8Array(0))),
  ]);
  writeFileSync(path, png);
}

// ---- generate ----
console.log(`generating ${SIZE}x${SIZE}…`);
const t0 = performance.now();
const world = generateWorld('seed', { size: SIZE, shape: 'continent' });
const genMs = performance.now() - t0;
console.log(`generateWorld: ${genMs.toFixed(0)} ms`);

const n = SIZE * SIZE;
const { elevation, moisture, temperature, biome, seaLevel, flow } = world;

// ---- sanity ----
let nan = 0;
let minE = Infinity;
let maxE = -Infinity;
for (let i = 0; i < n; i++) {
  const e = elevation[i];
  if (Number.isNaN(e) || Number.isNaN(moisture[i]) || Number.isNaN(temperature[i])) nan++;
  if (e < minE) minE = e;
  if (e > maxE) maxE = e;
}
console.log(`elevation [${minE.toFixed(3)}, ${maxE.toFixed(3)}], NaN=${nan}`);
console.log(`lakeBasins=${world.lakeBasins.length} flora=${world.floraCount}`);

// ---- biome histogram ----
const names = Object.keys(Biome).filter((k) => isNaN(Number(k)));
const hist = new Array(BIOME_COUNT).fill(0);
let land = 0;
for (let i = 0; i < n; i++) {
  hist[biome[i]]++;
  if (biome[i] !== Biome.Ocean) land++;
}
const rows = hist
  .map((c, b) => ({ b, c, name: names[b] }))
  .sort((a, z) => z.c - a.c);
console.log(`land: ${((land / n) * 100).toFixed(1)}%`);
for (const r of rows) {
  const pctAll = (r.c / n) * 100;
  const pctLand = (r.c / Math.max(1, land)) * 100;
  console.log(
    `${r.name.padEnd(10)} ${String(r.c).padStart(9)}  ${pctAll.toFixed(2).padStart(6)}% map  ${pctLand
      .toFixed(2)
      .padStart(6)}% land`,
  );
}
const missing = rows.filter((r) => r.c === 0).map((r) => r.name);
console.log(`present: ${BIOME_COUNT - missing.length}/${BIOME_COUNT}${missing.length ? '  missing: ' + missing.join(', ') : ''}`);

// ---- climate deciles (tuning aid) ----
function deciles(a: Float32Array, label: string, landOnly: boolean): void {
  const buckets = new Array(10).fill(0);
  let m = 0;
  for (let i = 0; i < n; i++) {
    if (landOnly && elevation[i] < seaLevel) continue;
    buckets[Math.min(9, (a[i] * 10) | 0)]++;
    m++;
  }
  console.log(`${label}: ${buckets.map((b) => ((b / m) * 100).toFixed(0).padStart(3)).join(' ')}`);
}
deciles(temperature, 'temp deciles (land, %)', true);
deciles(moisture, 'moist deciles (land, %)', true);
deciles(flow, 'flow deciles (land, %)', true);

// ---- preview PNG: biome colour x hillshade ----
const rgb = new Uint8Array(n * 3);
for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const i = y * SIZE + x;
    let [r, g, b] = BIOME_RGB[biome[i]];
    if (biome[i] === Biome.Ocean) {
      // depth-graded ocean
      const d = Math.min(1, (seaLevel - elevation[i]) / seaLevel);
      r = 26 + (1 - d) * 30;
      g = 60 + (1 - d) * 60;
      b = 120 + (1 - d) * 60;
    } else {
      const xl = elevation[y * SIZE + Math.max(0, x - 1)];
      const xr = elevation[y * SIZE + Math.min(SIZE - 1, x + 1)];
      const yu = elevation[Math.max(0, y - 1) * SIZE + x];
      const yd = elevation[Math.min(SIZE - 1, y + 1) * SIZE + x];
      const shade = Math.max(0.55, Math.min(1.35, 1 + ((xl - xr) + (yu - yd)) * 14));
      r *= shade;
      g *= shade;
      b *= shade;
    }
    rgb[i * 3] = Math.min(255, r);
    rgb[i * 3 + 1] = Math.min(255, g);
    rgb[i * 3 + 2] = Math.min(255, b);
  }
}
writePng(OUT, rgb, SIZE, SIZE);
console.log(`preview -> ${OUT}`);
