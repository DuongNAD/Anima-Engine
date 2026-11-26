// prototype, to be replaced — WV2-LOOK: scene scale derived from the World v2 index `extent_m`.
//
// The look was tuned on C01 (40.96 km drawn as a 1200-unit scene, 168 units for the 7200 m
// elevation span). Any other extent keeps C01's metres per unit (34.13 m) on both axes and makes
// the scene wider instead, so every constant written in units or metres keeps its meaning, and
// everything sized as a fraction of the scene (far plane, sky dome, fog, ocean plane, overview
// framing) grows with the map. The few things that are metre-calibrated but written as a fraction
// of the map (the map-edge bands, the cloud tile, the terrain mesh density) read `edgeScale`,
// `cloudTileExtents` and `meshResolution` from here.
//
// At C01's extent every field is the literal value the renderer used before, so C01 draws the
// same frame.

/** The extent the WV2-LOOK constants were tuned on (C01). */
export const C01_EXTENT_M = 40960;
/** C01's scene width in units (also `FLORA_RADIUS_REFERENCE_EXTENT`). */
export const C01_RENDER_SIZE = 1200;
const C01_HEIGHT_RATIO = 0.14;
const C01_MESH_RESOLUTION = 384;
const C01_CLOUD_TILE_EXTENTS = 5;
/**
 * Vertex budget for the single terrain mesh. C01's 107 m per vertex on the 81.92 km world would be
 * 768 segments (4x C01's vertices), which cost the overview half its frame rate on an M5 (169 vs
 * 310 fps at 384); 512 (~160 m per vertex) measured 238. The relief between vertices comes from the
 * baked normal map either way.
 */
const MAX_MESH_RESOLUTION = 512;

export interface WorldRenderScale {
  /** World extent in metres (C01's when unknown). */
  extentM: number;
  /** Scene width in units. */
  renderSize: number;
  /** Elevation span in units = renderSize * heightRatio (168 at every extent). */
  heightRatio: number;
  /** Terrain mesh segments per side (C01's ~107 m per vertex, capped at `MAX_MESH_RESOLUTION`). */
  meshResolution: number;
  /** C01 extent / this extent: multiplies map-edge band widths given as a fraction of the map. */
  edgeScale: number;
  /** Width of the cloud-puff tile in scene widths. */
  cloudTileExtents: number;
  /** Multiplier on the per-tier cloud grid, so puff spacing stays at C01's (~975 m). */
  cloudGridScale: number;
}

const C01_SCALE: WorldRenderScale = Object.freeze({
  extentM: C01_EXTENT_M,
  renderSize: C01_RENDER_SIZE,
  heightRatio: C01_HEIGHT_RATIO,
  meshResolution: C01_MESH_RESOLUTION,
  edgeScale: 1,
  cloudTileExtents: C01_CLOUD_TILE_EXTENTS,
  cloudGridScale: 1,
});

/** Scene scale for a World v2 extent; `null`/invalid (v1 worlds) gives C01's constants. */
export function worldRenderScale(extentM: number | null | undefined): WorldRenderScale {
  if (typeof extentM !== 'number' || !Number.isFinite(extentM) || extentM <= 0 || extentM === C01_EXTENT_M) {
    return C01_SCALE;
  }
  const k = extentM / C01_EXTENT_M;
  // C01's puff tile is 5 x 40.96 km around the camera. A bigger map needs at least ~3 map widths
  // so the overview camera (1.2 map widths out) still has cumulus over the far coast.
  const cloudTileExtents = Math.max(3, C01_CLOUD_TILE_EXTENTS / k);
  return {
    extentM,
    renderSize: C01_RENDER_SIZE * k,
    heightRatio: C01_HEIGHT_RATIO / k,
    meshResolution: Math.min(MAX_MESH_RESOLUTION, Math.round(C01_MESH_RESOLUTION * k)),
    edgeScale: 1 / k,
    cloudTileExtents,
    cloudGridScale: (cloudTileExtents * k) / C01_CLOUD_TILE_EXTENTS,
  };
}

/** A GLSL float literal for `v` (`2` -> `2.0`). */
export function glslFloat(v: number): string {
  return Number.isInteger(v) ? v.toFixed(1) : String(v);
}

/**
 * Map-edge threshold `t` on `boxN` (0 at the centre, 1 on the border) for a world whose edge bands
 * are `edgeScale` times as wide as a fraction of the map: the same distance in metres from the border.
 */
export function edgeThreshold(t: number, edgeScale: number): number {
  return edgeScale === 1 ? t : 1 - (1 - t) * edgeScale;
}

/** Replace `from` with `to` in `src`, and fail loudly if `from` is not there exactly once. */
export function replaceOnce(src: string, from: string, to: string): string {
  const at = src.indexOf(from);
  if (at < 0 || src.indexOf(from, at + 1) >= 0) {
    throw new Error(`worldScale: shader snippet not found exactly once: ${from}`);
  }
  return src.slice(0, at) + to + src.slice(at + from.length);
}
