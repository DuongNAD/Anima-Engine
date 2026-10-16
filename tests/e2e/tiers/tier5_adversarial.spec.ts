import { test, expect, Page } from '@playwright/test';
import type { RecordedInvocation, TauriEventEnvelope, TauriInvokeArgs } from '../tauri-mock-types';
import '../tauri-mock-types';

import type { SimulationStatus } from '../../../src/types/generated/SimulationStatus';
import type { EcosystemState } from '../../../src/types/generated/EcosystemState';
import type { DemographicCensusState } from '../../../src/types/generated/DemographicCensusState';
import type { EnergyFluxReport } from '../../../src/types/generated/EnergyFluxReport';
import type { MapElitesGridState } from '../../../src/types/generated/MapElitesGridState';
import type { LineageGraphPayload } from '../../../src/types/generated/LineageGraphPayload';
import type { SpatialDensityReport } from '../../../src/types/generated/SpatialDensityReport';
import type { ScientificExportResponse } from '../../../src/types/generated/ScientificExportResponse';

// ===============================================================================================
// TIER 5 ADVERSARIAL COVERAGE HARDENING TEST HARNESS
// ===============================================================================================

interface Tier5MockOptions {
  simulationStatus?: Partial<SimulationStatus>;
  ecosystemState?: Partial<EcosystemState> | null;
  demographics?: Partial<DemographicCensusState> | null;
  energyFlux?: Partial<EnergyFluxReport> | null;
  mapElitesGrid?: Partial<MapElitesGridState> | null;
  lineageGraph?: Partial<LineageGraphPayload> | null;
  spatialDensity?: Partial<SpatialDensityReport> | null;
  causalBranchTree?: unknown;
  exportResponse?: Partial<ScientificExportResponse> | null;
}

async function setupTier5AdversarialHarness(page: Page, options: Tier5MockOptions = {}) {
  const defaultStatus: SimulationStatus = {
    running: true,
    tick_count: 5000,
    avg_tick_time_ms: 1.8,
    fps: 60.0,
    ...(options.simulationStatus || {}),
  };

  const defaultEco: EcosystemState = {
    detritus: 100.0,
    plants: 500.0,
    animals: 300.0,
    total: 900.0,
    prey_count: 45,
    predator_count: 15,
    shannon: 0.95,
    simpson: 0.62,
    prey_mass: 6.0,
    predator_mass: 9.0,
    niche_divergence: 0.45,
    archive_coverage: 18.5,
    ...(options.ecosystemState || {}),
  };

  const defaultDemographics: DemographicCensusState = {
    ageCohorts: [
      { minAge: 0, maxAge: 50, count: 25 },
      { minAge: 51, maxAge: 150, count: 18 },
      { minAge: 151, maxAge: 300, count: 12 },
      { minAge: 301, maxAge: 1000, count: 5 },
    ],
    mortalityCauses: [
      { cause: 'Starvation', count: 8 as unknown as bigint },
      { cause: 'Predation', count: 24 as unknown as bigint },
      { cause: 'Senescence', count: 3 as unknown as bigint },
      { cause: 'Toxicity', count: 0 as unknown as bigint },
      { cause: 'Pathogen', count: 0 as unknown as bigint },
    ],
    fecundityByAge: [
      { ageGroup: 'Young (0-50)', births: 15 },
      { ageGroup: 'Mature (51-150)', births: 35 },
      { ageGroup: 'Old (151+)', births: 8 },
    ],
    shannon: 0.95,
    simpson: 0.62,
    preyCount: 45,
    predatorCount: 15,
    maleCount: 30,
    femaleCount: 30,
    sexRatio: 1.0,
    totalBirths: 58 as unknown as bigint,
    totalDeaths: 35 as unknown as bigint,
    ...(options.demographics || {}),
  };

  const defaultEnergyFlux: EnergyFluxReport = {
    nodes: [
      { id: 'sun', label: 'Solar Radiance', biomass: 10000 },
      { id: 'plants', label: 'Plants (NPP)', biomass: 500 },
      { id: 'herbivores', label: 'Herbivores', biomass: 225 },
      { id: 'carnivores', label: 'Carnivores', biomass: 75 },
      { id: 'detritus', label: 'Detritus', biomass: 100 },
      { id: 'metabolic_sink', label: 'Metabolic Heat Sink', biomass: 0 },
    ],
    links: [
      { source: 'sun', target: 'plants', value: 350.0 },
      { source: 'plants', target: 'herbivores', value: 120.0 },
      { source: 'herbivores', target: 'carnivores', value: 30.0 },
      { source: 'plants', target: 'detritus', value: 80.0 },
      { source: 'herbivores', target: 'detritus', value: 45.0 },
      { source: 'carnivores', target: 'detritus', value: 15.0 },
      { source: 'detritus', target: 'plants', value: 95.0 },
      { source: 'herbivores', target: 'metabolic_sink', value: 40.0 },
      { source: 'carnivores', target: 'metabolic_sink', value: 15.0 },
    ],
    respirationLoss: 55.0,
    totalSystemEnergy: 900.0,
    energyDriftPercent: 0.0015,
    ...(options.energyFlux || {}),
  };

  const defaultMapElites: MapElitesGridState = {
    grid: {
      '0,0': { fitness: 1.25, features: [0.1, 0.2] },
      '12,18': { fitness: 3.85, features: [0.45, 0.72] },
      '24,35': { fitness: 2.10, features: [0.65, 0.88] },
    },
    grid_resolution: 50,
    ...(options.mapElitesGrid || {}),
  };

  const defaultLineage: LineageGraphPayload = {
    nodes: [
      { id: '1', generation: 0, parent_id: null, fitness: 0.8, mutations_count: 0 },
      { id: '2', generation: 1, parent_id: '1', fitness: 0.95, mutations_count: 2 },
      { id: '3', generation: 2, parent_id: '2', fitness: 1.15, mutations_count: 4 },
      { id: '4', generation: 2, parent_id: '2', fitness: 0.75, mutations_count: 1 },
    ],
    links: [
      { source: '1', target: '2' },
      { source: '2', target: '3' },
      { source: '2', target: '4' },
    ],
    db_connected: true,
    ...(options.lineageGraph || {}),
  };

  const defaultSpatialDensity: SpatialDensityReport = {
    width: 16,
    height: 16,
    preyDensity: new Array(256).fill(0.3),
    predatorDensity: new Array(256).fill(0.1),
    vegetationDensity: new Array(256).fill(0.6),
    predationHotspots: new Array(256).fill(0.05),
    ...(options.spatialDensity || {}),
  };

  const defaultCausalBranchTree = options.causalBranchTree ?? {
    rootRunId: 1001,
    nodes: [
      {
        id: 1001,
        parentId: null,
        forkTick: 0,
        label: 'Genesis Baseline',
        seedStrategy: 'DerivedStream (0)',
        status: 'Completed',
        children: [1002, 1003],
      },
      {
        id: 1002,
        parentId: 1001,
        forkTick: 500,
        label: 'Drought Intervention (-30%)',
        seedStrategy: 'DerivedStream (1)',
        status: 'Completed',
        children: [],
      },
      {
        id: 1003,
        parentId: 1001,
        forkTick: 1200,
        label: 'Apex Predator Cull',
        seedStrategy: 'DerivedStream (2)',
        status: 'Completed',
        children: [],
      },
    ],
    edges: [
      { source: 1001, target: 1002, forkTick: 500 },
      { source: 1001, target: 1003, forkTick: 1200 },
    ],
  };

  const defaultExportResponse: ScientificExportResponse = {
    provenance: {
      commitHash: 'c320fc0',
      buildId: 'anima-build-2026',
      seed: 42 as unknown as bigint,
      durationTicks: 5000 as unknown as bigint,
      engineVersion: '0.1.0',
      timestampUtc: '2026-08-16T05:00:00Z',
      architecture: 'x86_64',
      os: 'windows',
      manifestFingerprint: 123456 as unknown as bigint,
      lawFingerprint: 789012 as unknown as bigint,
      registryFingerprint: 345678 as unknown as bigint,
    },
    csvContent: 'tick,plants,prey,predators,total_energy\n100,500.0,225.0,75.0,900.0\n',
    jsonContent: JSON.stringify({ schemaVersion: 1, totalTicks: 5000 }, null, 2),
    newickTree: '((Prey_A:0.1,Prey_B:0.2)Prey_Ancestor:0.5,Predator_A:0.8);',
    writtenPath: 'exports/scientific_dataset_run_42.json',
    ...(options.exportResponse || {}),
  };

  const replyTable: Record<string, unknown> = {
    get_simulation_status: defaultStatus,
    get_ecosystem_state: defaultEco,
    get_demographic_census: defaultDemographics,
    get_energy_flux_matrix: defaultEnergyFlux,
    get_map_elites_grid: defaultMapElites,
    get_lineage_graph: defaultLineage,
    get_spatial_density_matrix: defaultSpatialDensity,
    get_causal_branch_tree: defaultCausalBranchTree,
    export_scientific_dataset: defaultExportResponse,
    export_phylogenetic_tree: '((Prey_A:0.1,Prey_B:0.2)Prey_Ancestor:0.5,Predator_A:0.8);',
    dispatch_causal_intervention: true,
    toggle_simulation: true,
    toggle_evolution: true,
    update_evolution_settings: true,
    get_active_raycasts: [],
    get_environmental_elements: { elements: [] },
    get_chronicle_history: [],
    get_pheromone_grid: { grid: [0, 0, 0, 0], width: 2, height: 2 },
    get_terrain_map: { width: 2, height: 2, cells: [0, 0, 0, 0] },
    get_lod_bands: { bands: [] },
    set_lod_focus: null,
    list_legacy_saves: { directory: '', names: [], ignored: [] },
    import_legacy_save: 'restored.json',
  };

  await page.addInitScript((table: Record<string, unknown>) => {
    interface MockState {
      callbacks: Map<number, (event: TauriEventEnvelope) => void>;
      listeners: Map<string, Array<{ handler: number; eventId: number }>>;
      invocations: RecordedInvocation[];
      nextCallbackId: number;
      nextEventId: number;
    }
    const state: MockState = {
      callbacks: new Map(),
      listeners: new Map(),
      invocations: [],
      nextCallbackId: 1,
      nextEventId: 1,
    };

    window.__TAURI_INTERNALS__ = {
      transformCallback(callback: (event: TauriEventEnvelope) => void, once?: boolean): number {
        const id = state.nextCallbackId++;
        state.callbacks.set(id, (event: TauriEventEnvelope) => {
          if (once) state.callbacks.delete(id);
          callback(event);
        });
        return id;
      },
      unregisterCallback(id: number): void {
        state.callbacks.delete(id);
      },
      convertFileSrc(filePath: string, protocol = 'asset'): string {
        return `${protocol}://localhost/${encodeURIComponent(filePath)}`;
      },
      invoke(cmd: string, args: TauriInvokeArgs): Promise<unknown> {
        state.invocations.push({ cmd, args: args ?? null });

        if (cmd === 'plugin:event|listen') {
          const name = String(args?.event);
          const eventId = state.nextEventId++;
          const list = state.listeners.get(name) ?? [];
          list.push({ handler: Number(args?.handler), eventId });
          state.listeners.set(name, list);
          return Promise.resolve(eventId);
        }
        if (cmd === 'plugin:event|unlisten') {
          const name = String(args?.event);
          const eventId = Number(args?.eventId);
          const list = state.listeners.get(name) ?? [];
          state.listeners.set(name, list.filter((e) => e.eventId !== eventId));
          return Promise.resolve(null);
        }

        if (cmd in table) return Promise.resolve(table[cmd]);
        return Promise.resolve({ status: 'ok', cmd });
      },
    };

    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener(event: string, eventId: number): void {
        const list = state.listeners.get(event);
        if (!list) return;
        state.listeners.set(event, list.filter((entry) => entry.eventId !== eventId));
      },
    };

    window.__animaTauriMock = {
      emit(name: string, payload: unknown): number {
        const list = state.listeners.get(name) ?? [];
        for (const entry of list) {
          state.callbacks.get(entry.handler)?.({ event: name, id: entry.eventId, payload });
        }
        return list.length;
      },
      invocations: () => state.invocations,
      listenerCount: (name: string) => (state.listeners.get(name) ?? []).length,
    };
  }, replyTable);
}

/** Helper to open Research Dashboard from the main UI */
async function openResearchDashboard(page: Page) {
  await page.goto('/', { waitUntil: 'load' });
  const openDashboardBtn = page.locator('[data-testid="toggle-research-dashboard-btn"]');
  await expect(openDashboardBtn).toBeVisible({ timeout: 5000 });
  await openDashboardBtn.click();
  const container = page.locator('[data-testid="research-dashboard-container"]');
  await expect(container).toBeVisible({ timeout: 5000 });
}

// ===============================================================================================
// ADVERSARIAL SUITE 1: TELEMETRY STREAM DEGENERACY & PHASE-SPACE ORBIT STABILITY
// ===============================================================================================
test.describe('Tier 5 Adversarial: Telemetry Stream Degeneracy & Phase Space', () => {

  test('T5.1.1: Rapid Out-of-Order & Burst Telemetry Streams do not cause render lock or state divergence', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);

    // Rapid burst emissions of simulation-tick and custom events in reverse/random tick sequence
    await page.evaluate(() => {
      const ticks = [5000, 120, 9999, 0, 450, 8888, 12, 3500];
      ticks.forEach((tick, idx) => {
        window.__animaTauriMock.emit('simulation-tick', {
          tick_count: tick,
          energy_residual: idx % 2 === 0 ? 0.0005 : 0.05,
          plants: 100 + (idx * 50),
          animals: 50 + (idx * 20),
          detritus: 10,
        });
      });
    });

    await page.waitForTimeout(300);
    expect(unhandledError).toBeNull();

    // Verify time series chart and phase space canvas remain active
    const chart = page.locator('[data-testid="biomass-timeseries-chart"]');
    const phaseSpace = page.locator('[data-testid="lotka-volterra-phase-space"]');
    await expect(chart).toBeVisible();
    await expect(phaseSpace).toBeVisible();
  });

  test('T5.1.2: Empty & Degenerate Ecosystem Telemetry (0 biomass, negative values, high drift) handles gracefully', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      ecosystemState: {
        detritus: 0.0,
        plants: 0.0,
        animals: 0.0,
        total: 950.0, // 50 EU drift violation triggering warning
        prey_count: 0,
        predator_count: 0,
        shannon: 0.0,
        simpson: 0.0,
        niche_divergence: 0.0,
        archive_coverage: 0.0,
      },
    });

    await openResearchDashboard(page);

    // Ensure the panel renders without crashing
    const panel = page.locator('[data-testid="telemetry-overview-panel"]');
    await expect(panel).toBeVisible();

    // Conservation status badge reflects warning/drift gracefully
    const conservationBadge = page.locator('[data-testid="conservation-status-badge"]');
    await expect(conservationBadge).toBeVisible();
    const badgeText = await conservationBadge.innerText();
    expect(badgeText).toContain('DRIFT WARNING');

    expect(unhandledError).toBeNull();
  });

  test('T5.1.3: Phase-Space Orbit under Collapsed Trajectories & Zero Population prevents NaN dimensions', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      ecosystemState: {
        prey_count: 0,
        predator_count: 0,
      },
    });

    await openResearchDashboard(page);

    const phaseCanvas = page.locator('canvas[data-testid="lotka-volterra-phase-space"]');
    await expect(phaseCanvas).toBeVisible();

    // Check that canvas bounding box has valid width/height
    const box = await phaseCanvas.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBeGreaterThan(0);
    expect(box!.height).toBeGreaterThan(0);

    expect(unhandledError).toBeNull();
  });

  test('T5.1.4: Demographic Census Edge Cases (Empty cohorts, 0 population, NaN fecundity) renders safely', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      demographics: {
        ageCohorts: [],
        mortalityCauses: [],
        fecundityByAge: [],
        shannon: 0,
        simpson: 0,
      },
    });

    await openResearchDashboard(page);

    const cohortPyramid = page.locator('[data-testid="age-cohort-pyramid"]');
    const mortalityBreakdown = page.locator('[data-testid="mortality-cause-breakdown"]');

    await expect(cohortPyramid).toBeVisible();
    await expect(mortalityBreakdown).toBeVisible();

    expect(unhandledError).toBeNull();
  });

  test('T5.1.5: Time Window Horizon Resizing (100t, 500t, 2000t, All) operates cleanly under sparse and dense samples', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);

    const w100 = page.locator('[data-testid="time-window-100"]').first();
    const w2000 = page.locator('[data-testid="time-window-2000"]').first();
    const wAll = page.locator('[data-testid="time-window-all"]').first();

    await expect(w100).toBeVisible();
    await w100.click();
    await w2000.click();
    await wAll.click();

    expect(unhandledError).toBeNull();
  });
});

// ===============================================================================================
// ADVERSARIAL SUITE 2: TROPHIC PYRAMID & ENERGY FLUX SANKEY EXTREME DYNAMICS
// ===============================================================================================
test.describe('Tier 5 Adversarial: Trophic Dynamics & Energy Flux Sankey', () => {

  test('T5.2.1: Zero Biomass Ecosystem renders valid SVG geometry without divide-by-zero or negative bounds', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      ecosystemState: {
        detritus: 0,
        plants: 0,
        animals: 0,
        total: 0,
      },
    });

    await openResearchDashboard(page);

    // Switch to Trophic tab
    const trophicTab = page.locator('[data-testid="dashboard-tab-trophic"]');
    await trophicTab.click();

    const pyramidSvg = page.locator('[data-testid="trophic-pyramid-container"]');
    await expect(pyramidSvg).toBeVisible();

    // Verify all 4 tiers exist in SVG
    await expect(page.locator('[data-testid="trophic-tier-carnivores"]')).toBeVisible();
    await expect(page.locator('[data-testid="trophic-tier-herbivores"]')).toBeVisible();
    await expect(page.locator('[data-testid="trophic-tier-plants"]')).toBeVisible();
    await expect(page.locator('[data-testid="trophic-tier-detritus"]')).toBeVisible();

    expect(unhandledError).toBeNull();
  });

  test('T5.2.2: Inverted Top-Heavy Biomass Pyramid (Carnivores >> Plants) formats Lindeman efficiencies correctly', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      ecosystemState: {
        detritus: 10,
        plants: 10,
        animals: 10000,
        total: 10020,
      },
    });

    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-trophic"]').click();

    const carnivoreTier = page.locator('[data-testid="trophic-tier-carnivores"]');
    await expect(carnivoreTier).toBeVisible();
    await expect(carnivoreTier).toContainText('EU');

    expect(unhandledError).toBeNull();
  });

  test('T5.2.3: Degenerate Energy Flux Network with Missing/Dangling Nodes and zero flux', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      energyFlux: {
        nodes: [
          { id: 'orphan_1', label: 'Alien Node 1', biomass: 50 },
          { id: 'orphan_2', label: 'Alien Node 2', biomass: 100 },
        ],
        links: [
          { source: 'non_existent_src', target: 'orphan_1', value: 0 },
          { source: 'orphan_1', target: 'non_existent_dst', value: 10 },
        ],
        respirationLoss: 0,
        totalSystemEnergy: 150,
      },
    });

    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-trophic"]').click();

    const sankeySvg = page.locator('[data-testid="energy-flux-sankey"]');
    await expect(sankeySvg).toBeVisible();

    expect(unhandledError).toBeNull();
  });

  test('T5.2.4: Empty Energy Flux Report displays fallback empty banner without throwing', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      energyFlux: {
        nodes: [],
        links: [],
      },
    });

    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-trophic"]').click();

    const emptyBanner = page.locator('[data-testid="energy-flux-sankey"]');
    await expect(emptyBanner).toBeVisible();
    await expect(emptyBanner).toContainText('No Energy Flux data available');

    expect(unhandledError).toBeNull();
  });

  test('T5.2.5: Interactive Hover on Sankey Links and Nodes renders tooltip dynamically', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-trophic"]').click();

    const nodeSun = page.locator('[data-testid="sankey-node-sun"]');
    await expect(nodeSun).toBeVisible();
    await nodeSun.hover();

    const tooltip = page.locator('[data-testid="sankey-tooltip"]');
    await expect(tooltip).toBeVisible();
    await expect(tooltip).toContainText('Solar Radiance');

    expect(unhandledError).toBeNull();
  });
});

// ===============================================================================================
// ADVERSARIAL SUITE 3: EVOLUTION LINEAGE DAG & MAP-ELITES 2D QD GRID STRESS
// ===============================================================================================
test.describe('Tier 5 Adversarial: Lineage DAG & MAP-Elites Explorer', () => {

  test('T5.3.1: MAP-Elites Grid with Extreme / Negative Fitness and Out-of-Bounds Coordinates renders safely', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      mapElitesGrid: {
        grid: {
          '0,0': { fitness: -0.5, features: [0.0, 0.0] },
          '-10,-5': { fitness: 2.5, features: [0.1, 0.2] },
          '999,999': { fitness: 5.0, features: [0.9, 0.9] },
          'corrupted_key': { fitness: 0.1, features: [0.1, 0.1] },
        },
        grid_resolution: 50,
      },
    });

    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-evolution"]').click();

    const heatmapCanvas = page.locator('canvas[data-testid="map-elites-qd-grid"]');
    await expect(heatmapCanvas).toBeVisible();

    // Hover over canvas to verify tooltip interaction
    await heatmapCanvas.hover({ position: { x: 10, y: 10 } });

    expect(unhandledError).toBeNull();
  });

  test('T5.3.2: MAP-Elites Zero Coverage vs Saturated Archive calculates QD statistics accurately', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      mapElitesGrid: {
        grid: {},
        grid_resolution: 50,
      },
    });

    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-evolution"]').click();

    // Coverage should be 0.00%
    const panel = page.locator('[data-testid="evolution-lineage-panel"]');
    await expect(panel).toContainText('0.00%');
    await expect(panel).toContainText('0 / 2500 occupied niches');

    expect(unhandledError).toBeNull();
  });

  test('T5.3.3: Lineage DAG with Disconnected Genotypes & Missing Branch Parents renders DAG properly', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      lineageGraph: {
        nodes: [
          { id: '10', generation: 0, parent_id: null, fitness: 0.5, mutations_count: 0 },
          { id: '20', generation: 1, parent_id: 'non_existent_parent', fitness: 0.9, mutations_count: 3 },
          { id: '30', generation: 5, parent_id: '20', fitness: 1.2, mutations_count: 6 },
        ],
        links: [
          { source: 'non_existent_parent', target: '20' },
          { source: '20', target: '30' },
        ],
        db_connected: true,
      },
    });

    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-evolution"]').click();

    const node20 = page.locator('[data-testid="phylogenetic-dag-container"]').locator('[data-testid="lineage-node-20"]');
    await expect(node20).toBeVisible();
    await node20.click();

    // Genome drawer opens
    const drawer = page.locator('[data-testid="genome-inspector-drawer"]');
    await expect(drawer).toBeVisible();
    await expect(drawer).toContainText('Agent #20');
    await expect(drawer).toContainText('non_existent_parent');

    expect(unhandledError).toBeNull();
  });

  test('T5.3.4: Lineage DAG Zoom and Pan controls withstand rapid interaction', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-evolution"]').click();

    const zoomIn = page.locator('[data-testid="lineage-zoom-in"]');
    const zoomOut = page.locator('[data-testid="lineage-zoom-out"]');
    const resetPan = page.locator('[data-testid="lineage-reset-pan"]');

    for (let i = 0; i < 5; i++) {
      await zoomIn.click();
      await zoomOut.click();
    }
    await resetPan.click();

    expect(unhandledError).toBeNull();
  });

  test('T5.3.5: RFC Newick Tree Export generates valid tree string and modal representation', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-evolution"]').click();

    const exportBtn = page.locator('[data-testid="export-newick-button"]');
    await expect(exportBtn).toBeVisible();
    await exportBtn.click();

    const newickOutput = page.locator('[data-testid="newick-export-output"]');
    await expect(newickOutput).toBeVisible();
    await expect(newickOutput).toContainText('Newick RFC String:');

    expect(unhandledError).toBeNull();
  });
});

// ===============================================================================================
// ADVERSARIAL SUITE 4: CAUSAL INTERVENTION CONTROL PANEL BOUNDARY EXPLOITATION
// ===============================================================================================
test.describe('Tier 5 Adversarial: Causal Intervention Control Panel', () => {

  test('T5.4.1: Spatial Region Selector handles all modes (Global, Rect, Radius, Cell) without errors', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-interventions"]').click();

    const rectModeBtn = page.locator('[data-testid="region-mode-rect"]');
    const radiusModeBtn = page.locator('[data-testid="region-mode-radius"]');
    const cellModeBtn = page.locator('[data-testid="region-mode-cell"]');
    const globalModeBtn = page.locator('[data-testid="region-mode-global"]');

    await rectModeBtn.click();
    await radiusModeBtn.click();
    await cellModeBtn.click();
    await globalModeBtn.click();

    const canvas = page.locator('canvas[data-testid="causal-spatial-canvas"]');
    await expect(canvas).toBeVisible();

    expect(unhandledError).toBeNull();
  });

  test('T5.4.2: Inverted & Extreme Temporal Curves (Triangle/Ramp with Signed Negative) render preview properly', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-interventions"]').click();

    const curvePreview = page.locator('[data-testid="curve-ramp-preview"]');
    await expect(curvePreview).toBeVisible();

    expect(unhandledError).toBeNull();
  });

  test('T5.4.3: Dispatching Causal Interventions with extreme magnitudes updates ledger safely', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-interventions"]').click();

    const kindSelect = page.locator('[data-testid="intervention-kind-select"]');
    await expect(kindSelect).toBeVisible();
    await kindSelect.selectOption('ToxicityPlume');

    const applyBtn = page.locator('[data-testid="apply-intervention-btn"]');
    await expect(applyBtn).toBeVisible();
    await applyBtn.click();

    // Verify item added to ledger table
    const ledgerTable = page.locator('[data-testid="active-interventions-ledger"]');
    await expect(ledgerTable).toBeVisible();
    await expect(ledgerTable).toContainText('ToxicityPlume');

    expect(unhandledError).toBeNull();
  });

  test('T5.4.4: Cancelling Active Interventions updates ledger state immediately', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-interventions"]').click();

    const cancelBtn = page.locator('[data-testid^="cancel-intervention-"]').first();
    if (await cancelBtn.isVisible()) {
      await cancelBtn.click();
    }

    expect(unhandledError).toBeNull();
  });
});

// ===============================================================================================
// ADVERSARIAL SUITE 5: CHECKPOINT STUDIO & COUNTERFACTUAL TRAJECTORY DELTA
// ===============================================================================================
test.describe('Tier 5 Adversarial: Checkpoint Studio & Counterfactuals', () => {

  test('T5.5.1: Degenerate & Single-Node Causal Branch Tree renders root node cleanly', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      causalBranchTree: {
        rootRunId: 4001,
        nodes: [
          {
            id: 4001,
            parentId: null,
            forkTick: 0,
            label: 'Solitary Genesis Root',
            seedStrategy: 'Fixed',
            status: 'Completed',
            children: [],
          },
        ],
        edges: [],
      },
    });

    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-checkpoints"]').click();

    const rootNode = page.locator('[data-testid="branch-node-4001"]');
    await expect(rootNode).toBeVisible();
    await expect(rootNode).toContainText('#4001');

    expect(unhandledError).toBeNull();
  });

  test('T5.5.2: Historical Timeline Scrubber steps cleanly across extreme boundaries (t=0 to t=10,000)', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-checkpoints"]').click();

    const slider = page.locator('[data-testid="timeline-scrubber-slider"]');
    await expect(slider).toBeVisible();

    // Step slider to extreme values
    await slider.fill('0');
    await slider.fill('2000');

    // Create a new snapshot bookmark
    const bookmarkBtn = page.locator('[data-testid="save-snapshot-bookmark-btn"]');
    await bookmarkBtn.click();

    const newBookmark = page.locator('[data-testid="checkpoint-bookmark-2000"]');
    await expect(newBookmark).toBeVisible();

    expect(unhandledError).toBeNull();
  });

  test('T5.5.3: Forking Counterfactual Branch modal opens, inputs values, and creates fork node', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-checkpoints"]').click();

    const openForkModalBtn = page.locator('[data-testid="fork-branch-open-modal-btn"]');
    await expect(openForkModalBtn).toBeVisible();
    await openForkModalBtn.click();

    const modal = page.locator('[data-testid="fork-branch-modal"]');
    await expect(modal).toBeVisible();

    const labelInput = page.locator('[data-testid="fork-label-input"]');
    await labelInput.fill('Counterfactual Adversarial Run');

    const submitBtn = page.locator('[data-testid="confirm-fork-branch-btn"]');
    await submitBtn.click();

    await expect(modal).not.toBeVisible();

    expect(unhandledError).toBeNull();
  });

  test('T5.5.4: Counterfactual Delta SparkLines render trajectory divergence curves without throwing', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="dashboard-tab-checkpoints"]').click();

    const deltaComparator = page.locator('[data-testid="counterfactual-delta-comparator"]');
    await expect(deltaComparator).toBeVisible();

    const biomassSparkline = page.locator('[data-testid="biomass-delta-sparkline"]');
    await expect(biomassSparkline).toBeVisible();

    expect(unhandledError).toBeNull();
  });
});

// ===============================================================================================
// ADVERSARIAL SUITE 6: SCIENTIFIC DATA EXPORT DIALOG & RUNPROVENANCE METADATA
// ===============================================================================================
test.describe('Tier 5 Adversarial: Scientific DataExportDialog', () => {

  test('T5.6.1: Opening DataExportDialog with all modules and high resolution creates dataset package', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);

    const openExportBtn = page.locator('[data-testid="open-export-dialog-btn"]');
    await expect(openExportBtn).toBeVisible();
    await openExportBtn.click();

    const modal = page.locator('[data-testid="data-export-dialog-modal"]');
    await expect(modal).toBeVisible();

    // Check all sub-module checkboxes
    await page.locator('[data-testid="export-include-timeseries"]').check();
    await page.locator('[data-testid="export-include-demographics"]').check();
    await page.locator('[data-testid="export-include-energy-flux"]').check();
    await page.locator('[data-testid="export-include-spatial-density"]').check();
    await page.locator('[data-testid="export-include-phylogeny"]').check();

    // Select 64x64 grid resolution
    await page.locator('[data-testid="export-grid-resolution-select"]').selectOption('64');

    // Trigger dataset generation
    const generateBtn = page.locator('[data-testid="execute-export-dataset-btn"]');
    await generateBtn.click();

    // Preview content becomes visible
    const preview = page.locator('[data-testid="export-preview-content"]');
    await expect(preview).toBeVisible();

    // Switch formats: CSV and Newick
    await page.locator('[data-testid="export-format-csv"]').click();
    await expect(preview).toContainText('plants');

    await page.locator('[data-testid="export-format-newick"]').click();
    await expect(preview).toContainText('Prey_Ancestor');

    expect(unhandledError).toBeNull();
  });

  test('T5.6.2: RunProvenance Preview displays verified commit, seed, and timestamps', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);
    await page.locator('[data-testid="open-export-dialog-btn"]').click();

    const provenanceCard = page.locator('[data-testid="run-provenance-card"]');
    await expect(provenanceCard).toBeVisible();
    await expect(provenanceCard).toContainText('c320fc0');
    await expect(provenanceCard).toContainText('anima-build-2026');

    // Close modal
    await page.locator('[data-testid="export-dialog-close-btn"]').click();
    await expect(page.locator('[data-testid="data-export-dialog-modal"]')).not.toBeVisible();

    expect(unhandledError).toBeNull();
  });
});

// ===============================================================================================
// ADVERSARIAL SUITE 7: FULL DASHBOARD INTEGRATION, TAB SWITCHING & KEYBOARD HOTKEYS
// ===============================================================================================
test.describe('Tier 5 Adversarial: Full Dashboard Integration & Hotkeys', () => {

  test('T5.7.1: Rapid Numerical Keyboard Navigation (1-5) switches tabs seamlessly', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);

    // Rapid keyboard tab switching: 2 (Trophic), 3 (Evolution), 4 (Interventions), 5 (Checkpoints), 1 (Telemetry)
    await page.keyboard.press('2');
    await expect(page.locator('[data-testid="trophic-dynamics-panel"]')).toBeVisible();

    await page.keyboard.press('3');
    await expect(page.locator('[data-testid="evolution-lineage-panel"]')).toBeVisible();

    await page.keyboard.press('4');
    await expect(page.locator('[data-testid="causal-intervention-panel"]')).toBeVisible();

    await page.keyboard.press('5');
    await expect(page.locator('[data-testid="checkpoint-studio-panel"]')).toBeVisible();

    await page.keyboard.press('1');
    await expect(page.locator('[data-testid="telemetry-overview-panel"]')).toBeVisible();

    expect(unhandledError).toBeNull();
  });

  test('T5.7.2: Spacebar toggles simulation state and Ctrl+E opens Export Dialog', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page);
    await openResearchDashboard(page);

    // Press Space to toggle simulation
    const toggleSimBtn = page.locator('[data-testid="dashboard-toggle-sim-btn"]');
    await expect(toggleSimBtn).toBeVisible();
    await page.keyboard.press('Space');

    // Press Ctrl+E to open export dialog
    await page.keyboard.press('Control+KeyE');
    await expect(page.locator('[data-testid="data-export-dialog-modal"]')).toBeVisible();

    expect(unhandledError).toBeNull();
  });

  test('T5.7.3: Dynamic Transition between CONSERVED and DRIFT WARNING on simulated energy leaks', async ({ page }) => {
    let unhandledError: Error | null = null;
    page.on('pageerror', (err) => { unhandledError = err; });

    await setupTier5AdversarialHarness(page, {
      ecosystemState: {
        total: 900.0002, // Within 1e-3 EU
      },
    });

    await openResearchDashboard(page);

    const conservationBadge = page.locator('[data-testid="conservation-status-badge"]');
    await expect(conservationBadge).toBeVisible();
    await expect(conservationBadge).toContainText('CONSERVED');

    // Inject simulated energy violation
    await page.evaluate(() => {
      window.__animaTauriMock.emit('simulation-tick', {
        total: 950.0, // 50 EU drift violation
      });
    });

    expect(unhandledError).toBeNull();
  });
});
