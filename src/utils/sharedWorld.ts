import type { WorldShape } from '../components/Landscape/utils/worldGen';

// ---------------------------------------------------------------------------------------
// Which world everything means when it says "the world".
//
// These three values are an *identity*, not settings. `worldCache` keys its IndexedDB entry and
// its in-memory memo on exactly (seed, size, shape), and `loadOrGenerateWorld` hands whatever it
// produces to the backend as the shared World Artifact — which `init_world` then boots the agents
// on. So two callers that disagree about any one of them do not merely render differently: they
// generate two worlds, push two artifacts, and the simulation lives on whichever page loaded last.
//
// That is why they live here instead of at the top of the component that first needed them. A
// second definition would not fail — it would quietly make the world depend on navigation order.
// ---------------------------------------------------------------------------------------

/** Seed of the shared world. */
export const SHARED_WORLD_SEED = 'seed';

/** Data resolution in cells. 2048² ≈ 4M cells: generated once off-thread, then cached. Rendering
 * resolution is decoupled from this, and the artifact handed to the simulation is downsampled. */
export const SHARED_WORLD_SIZE = 2048;

/**
 * Landmass shape. `supercontinent` (since worldgen 22) is laid out so every environment has ground
 * to form on: one land belt from pole to pole, two mountain ranges and the climate bands across it.
 * `continent` put the equator under open sea, which left the hot biomes almost absent. See
 * `SUPERCONTINENT` in `worldGen.ts`.
 */
export const SHARED_WORLD_SHAPE: WorldShape = 'supercontinent';

/** The `WorldGenOptions` the shared world is generated with, ready to pass to `worldCache`. */
export const SHARED_WORLD_OPTS = {
  size: SHARED_WORLD_SIZE,
  shape: SHARED_WORLD_SHAPE,
} as const;
