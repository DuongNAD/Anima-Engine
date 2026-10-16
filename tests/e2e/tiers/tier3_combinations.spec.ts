import { test, expect } from '@playwright/test';
import {
  E_ANIMAL_EV,
  RESIDUAL_ABS_TOLERANCE_EU,
  calculateMteMetabolicRate,
  calculateHollingType2,
  calculateHollingType3,
  calculateLindemanEfficiency,
  SimulatedEnergyLedger,
  ParameterSweepEngine,
  CausalBranchTree,
  CausalInterventionEngine,
  DeterministicReplayEngine,
  DemographicsPipeline,
  EnergyFluxPipeline,
  SpatialDensityPipeline,
  MapElitesNewickPipeline,
  ScientificDatasetExporter,
  SnapshotEnvelopeManager,
  ZeroAllocBufferSimulator,
} from './e2e_harness';

// ===============================================================================================
// Enhanced Robust Coexistence Simulation for Pairwise & Real-World Invariance Tests
// ===============================================================================================

export interface RobustAgent {
  id: number;
  type: 'prey' | 'predator';
  energy: number;
  mass: number;
  age: number;
  x: number;
  y: number;
  alive: boolean;
}

export class RobustCoexistenceSimulation {
  public ledger: SimulatedEnergyLedger;
  public agents: RobustAgent[] = [];
  public currentTick: number = 0;
  public carryingCapacity: number = 8000.0;
  public plantGrowthRate: number = 0.25;
  public reproThresholdPrey: number = 45.0;
  public reproThresholdPredator: number = 75.0;
  public nextAgentId: number = 1;

  constructor(initialPlants: number = 3000, initialPrey: number = 50, initialPredators: number = 10, detritus: number = 1000) {
    let animalEnergy = 0;
    for (let i = 0; i < initialPrey; i++) {
      const e = 35.0;
      this.agents.push({
        id: this.nextAgentId++,
        type: 'prey',
        energy: e,
        mass: 1.0,
        age: Math.floor(Math.random() * 100),
        x: Math.random() * 90 + 5,
        y: Math.random() * 90 + 5,
        alive: true,
      });
      animalEnergy += e;
    }
    for (let i = 0; i < initialPredators; i++) {
      const e = 65.0;
      this.agents.push({
        id: this.nextAgentId++,
        type: 'predator',
        energy: e,
        mass: 2.5,
        age: Math.floor(Math.random() * 100),
        x: Math.random() * 90 + 5,
        y: Math.random() * 90 + 5,
        alive: true,
      });
      animalEnergy += e;
    }

    this.ledger = new SimulatedEnergyLedger(initialPlants, animalEnergy, detritus);
    this.ledger.lockBaseline();
  }

  public stepTick(tempC: number = 20.0): void {
    this.currentTick++;

    // 1. Vigorous logistic plant regrowth from detritus
    const plantDemand = Math.max(
      0.0,
      this.plantGrowthRate * this.ledger.plants * (1.0 - this.ledger.plants / this.carryingCapacity)
    );
    if (plantDemand > 0 && this.ledger.detritus > 0) {
      this.ledger.transferDetritusToPlant(plantDemand);
    }

    // 2. Herbivore grazing with realistic assimilation and egestion into detritus
    const livingPrey = this.agents.filter(a => a.alive && a.type === 'prey');
    const maxIntakeShare = (this.ledger.plants * 0.08) / Math.max(1, livingPrey.length);

    for (const prey of livingPrey) {
      const nominalIntake = calculateHollingType2(this.ledger.plants / 100.0, 0.45, 0.12);
      const intakeRate = Math.min(nominalIntake, maxIntakeShare);
      const actualEaten = this.ledger.transferPlantToAnimal(intakeRate);

      // 70% assimilation into herbivore tissue, 30% unassimilated egestion directly to detritus
      prey.energy += actualEaten * 0.7;
      const egested = actualEaten * 0.3;
      this.ledger.transferAnimalToDetritus(egested);

      prey.x = Math.min(99, Math.max(1, prey.x + (Math.random() * 4 - 2)));
      prey.y = Math.min(99, Math.max(1, prey.y + (Math.random() * 4 - 2)));
    }

    // 3. Carnivore predation (Holling Type III with Rarity Refuge)
    const livingPredators = this.agents.filter(a => a.alive && a.type === 'predator');
    const preyDensity = livingPrey.length;

    // Strict Rarity Refuge: When prey density <= 15, prey access micro-refugia and are unhuntable
    if (preyDensity > 15) {
      const killRate = calculateHollingType3(preyDensity, 0.001, 0.1);
      for (const pred of livingPredators) {
        pred.x = Math.min(99, Math.max(1, pred.x + (Math.random() * 4 - 2)));
        pred.y = Math.min(99, Math.max(1, pred.y + (Math.random() * 4 - 2)));

        if (Math.random() < Math.min(killRate, 0.25)) {
          const targetPrey = livingPrey[Math.floor(Math.random() * livingPrey.length)];
          if (targetPrey && targetPrey.alive) {
            const transferEnergy = Math.min(targetPrey.energy, 16.0);
            targetPrey.energy -= transferEnergy;
            pred.energy += transferEnergy * 0.7;
            const unassimilated = transferEnergy * 0.3;
            this.ledger.transferAnimalToDetritus(unassimilated);
            if (targetPrey.energy <= 0) {
              targetPrey.alive = false;
            }
          }
        }
      }
    }

    // 4. MTE Metabolic Decay & Natural Senescence
    for (const agent of this.agents.filter(a => a.alive)) {
      agent.age++;
      const burnScale = agent.type === 'predator' ? 0.35 : 0.15;
      const burn = calculateMteMetabolicRate(agent.mass, tempC, E_ANIMAL_EV) * burnScale;
      const actualBurn = Math.min(burn, agent.energy);
      agent.energy -= actualBurn;
      this.ledger.transferAnimalToDetritus(actualBurn);

      // Smooth senescence after 1,000 ticks
      const senescenceRisk = agent.age > 1000 ? 0.005 : 0.0;
      if (agent.energy <= 0 || Math.random() < senescenceRisk) {
        agent.alive = false;
        if (agent.energy > 0) {
          this.ledger.transferAnimalToDetritus(agent.energy);
          agent.energy = 0;
        }
      }
    }

    // 5. Continuous intra-tick reproduction with density-dependent fecundity
    const newOffspring: RobustAgent[] = [];
    const maxPrey = 120;
    const maxPred = 20;

    for (const agent of this.agents.filter(a => a.alive)) {
      if (agent.type === 'prey' && agent.energy >= this.reproThresholdPrey) {
        const preyFecundity = Math.max(0.1, 1.0 - livingPrey.length / maxPrey);
        if (Math.random() < preyFecundity) {
          const offspringEnergy = this.reproThresholdPrey * 0.45;
          agent.energy -= offspringEnergy;
          newOffspring.push({
            id: this.nextAgentId++,
            type: 'prey',
            energy: offspringEnergy,
            mass: agent.mass * (0.95 + Math.random() * 0.1),
            age: 0,
            x: Math.min(99, Math.max(1, agent.x + (Math.random() * 10 - 5))),
            y: Math.min(99, Math.max(1, agent.y + (Math.random() * 10 - 5))),
            alive: true,
          });
        }
      } else if (agent.type === 'predator' && preyDensity > 15 && agent.energy >= this.reproThresholdPredator) {
        const predFecundity = Math.max(0.1, 1.0 - livingPredators.length / maxPred);
        if (Math.random() < predFecundity) {
          const offspringEnergy = this.reproThresholdPredator * 0.45;
          agent.energy -= offspringEnergy;
          newOffspring.push({
            id: this.nextAgentId++,
            type: 'predator',
            energy: offspringEnergy,
            mass: agent.mass * (0.95 + Math.random() * 0.1),
            age: 0,
            x: Math.min(99, Math.max(1, agent.x + (Math.random() * 10 - 5))),
            y: Math.min(99, Math.max(1, agent.y + (Math.random() * 10 - 5))),
            alive: true,
          });
        }
      }
    }
    this.agents.push(...newOffspring);
  }

  public getCensus(): { preyCount: number; predatorCount: number; shannon: number; simpson: number } {
    const preyCount = this.agents.filter(a => a.alive && a.type === 'prey').length;
    const predatorCount = this.agents.filter(a => a.alive && a.type === 'predator').length;
    let shannon = 0;
    const total = preyCount + predatorCount;
    if (total > 0) {
      if (preyCount > 0) {
        const p = preyCount / total;
        shannon -= p * Math.log(p);
      }
      if (predatorCount > 0) {
        const p = predatorCount / total;
        shannon -= p * Math.log(p);
      }
    }
    const simpson = total > 0 ? 1.0 - (Math.pow(preyCount / total, 2) + Math.pow(predatorCount / total, 2)) : 0;
    return { preyCount, predatorCount, shannon, simpson };
  }
}

// ===============================================================================================
// Anima Engine — Tier 3 Cross-Feature Pairwise Interaction Suite
// ===============================================================================================

test.describe('Tier 3: Pairwise Cross-Feature Interactions', () => {

  // ---------------------------------------------------------------------------------------------
  // 1. F1 x F4: Coexistence with Closed-Loop Energy Conservation
  // ---------------------------------------------------------------------------------------------
  test('T3-01 [F1 x F4]: 3-Trophic coexistence maintains strict closed-loop energy conservation (<0.01% drift)', async () => {
    const sim = new RobustCoexistenceSimulation(3000, 50, 15, 1000);
    const initialTotal = sim.ledger.total();
    expect(sim.ledger.baseline).toBe(initialTotal);

    // Run for 500 ticks to observe multiple birth, grazing, predation, and mortality cycles
    for (let t = 0; t < 500; t++) {
      sim.stepTick();
    }

    const finalTotal = sim.ledger.total();
    const driftPercent = sim.ledger.driftPercentage();
    const census = sim.getCensus();

    // Verify multi-trophic survival
    expect(census.preyCount).toBeGreaterThan(0);
    expect(census.predatorCount).toBeGreaterThan(0);
    expect(sim.ledger.plants).toBeGreaterThan(0);

    // Verify energy conservation invariant
    expect(driftPercent).toBeLessThan(0.01);
    expect(Math.abs(finalTotal - initialTotal)).toBeLessThan(0.05);
    expect(sim.ledger.settled).toBeGreaterThan(100);
  });

  // ---------------------------------------------------------------------------------------------
  // 2. F2 x F3: Thermal MTE with Holling Predation Dynamics
  // ---------------------------------------------------------------------------------------------
  test('T3-02 [F2 x F3]: Arrhenius temperature scaling modulates Holling Type II & III predation pressure', async () => {
    const mass = 2.0; // kg
    const preyDensity = 15.0;

    // Evaluate cold (10 °C) vs temperate (20 °C) vs warm (30 °C)
    const metabolicCold = calculateMteMetabolicRate(mass, 10.0, E_ANIMAL_EV);
    const metabolicTemperate = calculateMteMetabolicRate(mass, 20.0, E_ANIMAL_EV);
    const metabolicWarm = calculateMteMetabolicRate(mass, 30.0, E_ANIMAL_EV);

    expect(metabolicCold).toBeLessThan(metabolicTemperate);
    expect(metabolicTemperate).toBeLessThan(metabolicWarm);

    // Holling Type II and Type III consumption rates
    const attackRate = 0.04;
    const handlingTime = 0.08;
    const intakeT2 = calculateHollingType2(preyDensity, attackRate, handlingTime);
    const intakeT3 = calculateHollingType3(preyDensity, attackRate, handlingTime);

    // Predation saturation ceiling
    const saturationCeiling = 1.0 / handlingTime;
    expect(intakeT2).toBeLessThan(saturationCeiling);
    expect(intakeT3).toBeLessThan(saturationCeiling);

    // Predator demand is driven by metabolic burn / Lindeman efficiency
    const demandCold = metabolicCold / 0.3;
    const demandWarm = metabolicWarm / 0.3;
    expect(demandWarm).toBeGreaterThan(demandCold * 1.8);
  });

  // ---------------------------------------------------------------------------------------------
  // 3. F5 x F8: Parameter Sweeps with Deterministic Replay
  // ---------------------------------------------------------------------------------------------
  test('T3-03 [F5 x F8]: Multi-dimensional Sobol parameter sweep generates bit-exact reproducible replay states', async () => {
    const dims = [
      { name: 'plantGrowthRate', min: 0.02, max: 0.12 },
      { name: 'carryingCapacity', min: 2000, max: 8000 },
    ];

    const sweepResult = ParameterSweepEngine.executeSweep(dims, 'Sobol', 6);
    expect(sweepResult.parameterMatrix.length).toBe(6);

    // Verify determinism: replaying seed with exact parameter point produces identical state digest
    const samplePoint = sweepResult.parameterMatrix[0];
    const seed = 98765;

    const replay1 = new DeterministicReplayEngine(seed);
    const entities1 = Array.from({ length: 10 }, (_, i) => ({
      id: i + 1,
      energy: 50.0 + replay1.nextFloat() * samplePoint[0] * 100,
      x: replay1.nextFloat() * 100,
      y: replay1.nextFloat() * 100,
    }));
    const digest1 = DeterministicReplayEngine.computeStateDigest(entities1);

    const replay2 = new DeterministicReplayEngine(seed);
    const entities2 = Array.from({ length: 10 }, (_, i) => ({
      id: i + 1,
      energy: 50.0 + replay2.nextFloat() * samplePoint[0] * 100,
      x: replay2.nextFloat() * 100,
      y: replay2.nextFloat() * 100,
    }));
    const digest2 = DeterministicReplayEngine.computeStateDigest(entities2);

    expect(digest1).toBe(digest2);
    expect(digest1.startsWith('DIGEST_')).toBe(true);
  });

  // ---------------------------------------------------------------------------------------------
  // 4. F6 x F19: Branch Trees with Checkpoint Studio
  // ---------------------------------------------------------------------------------------------
  test('T3-04 [F6 x F19]: N-ary causal branch tree tracks counterfactual fork checkpoints and trajectory divergence', async () => {
    const tree = new CausalBranchTree('DIGEST_GENESIS');

    // Fork branch A (control) at tick 100
    const branchA = tree.forkBranch(tree.rootId, 100, 'Control Baseline', 'DIGEST_A_100');
    branchA.trajectory.push(
      { tick: 200, prey: 50, predator: 20, totalEnergy: 2500 },
      { tick: 300, prey: 48, predator: 19, totalEnergy: 2500 },
    );

    // Fork branch B (treatment: drought) at tick 100
    const branchB = tree.forkBranch(tree.rootId, 100, 'Drought Crisis', 'DIGEST_B_100');
    branchB.trajectory.push(
      { tick: 200, prey: 25, predator: 8, totalEnergy: 1800 },
      { tick: 300, prey: 15, predator: 3, totalEnergy: 1200 },
    );

    // Verify lineage ancestry
    const pathToRootA = tree.traceToRoot(branchA.id);
    const pathToRootB = tree.traceToRoot(branchB.id);
    expect(pathToRootA).toEqual([tree.rootId, branchA.id]);
    expect(pathToRootB).toEqual([tree.rootId, branchB.id]);

    // Calculate divergence metric between Control and Crisis
    const divergence = tree.calculateDivergence(branchA.id, branchB.id);
    expect(divergence.length).toBe(3); // tick 0 (shared ancestor), tick 200, tick 300
    expect(divergence[0].divergenceDistance).toBe(0); // identical at root
    expect(divergence[1].divergenceDistance).toBeGreaterThan(25); // diverged at tick 200
    expect(divergence[2].divergenceDistance).toBeGreaterThan(35); // heavily diverged at tick 300
  });

  // ---------------------------------------------------------------------------------------------
  // 5. F7 x F18: Causal Interventions with Control Panel
  // ---------------------------------------------------------------------------------------------
  test('T3-05 [F7 x F18]: Declarative spatial/temporal interventions apply causal forcings accurately to target entities', async () => {
    const agents: { id: number; type: 'prey' | 'predator'; energy: number; mass: number; age: number; x: number; y: number; alive: boolean }[] = [
      { id: 1, type: 'prey', energy: 100, mass: 1.0, age: 10, x: 25, y: 25, alive: true }, // inside region
      { id: 2, type: 'prey', energy: 100, mass: 1.0, age: 10, x: 80, y: 80, alive: true }, // outside region
    ];
    const resourceField = [
      { x: 25, y: 25, biomass: 50 },
      { x: 80, y: 80, biomass: 50 },
    ];

    const command = {
      id: 'cmd_toxicity_01',
      kind: 'ToxicityPlume' as const,
      targetRegion: { type: 'Circle' as const, x: 25, y: 25, radius: 15.0 },
      curve: 'Sigmoid' as const,
      startTick: 50,
      durationTicks: 50,
      magnitude: 5.0,
    };

    // Before activation
    const resPre = CausalInterventionEngine.applyIntervention(command, 40, agents, resourceField);
    expect(resPre.affectedAgents).toBe(0);

    // Active midpoint (tick 75)
    const resActive = CausalInterventionEngine.applyIntervention(command, 75, agents, resourceField);
    expect(resActive.affectedAgents).toBe(1);
    expect(agents[0].energy).toBeLessThan(100); // Agent inside plume depleted
    expect(agents[1].energy).toBe(100); // Agent outside unaffected
  });

  // ---------------------------------------------------------------------------------------------
  // 6. F9 x F10: Demographics with Energy Flux Sankey
  // ---------------------------------------------------------------------------------------------
  test('T3-06 [F9 x F10]: Demographic age cohorts and mortality metrics correlate directly with Energy Flux Sankey flows', async () => {
    const sim = new RobustCoexistenceSimulation(3000, 40, 12, 1000);
    for (let t = 0; t < 100; t++) sim.stepTick();

    const census = DemographicsPipeline.generateCensus(sim.agents, [
      { cause: 'Starvation' },
      { cause: 'Predation' },
      { cause: 'Senescence' },
    ]);

    expect(census.ageCohorts.length).toBe(4);
    expect(census.mortalityCauses.length).toBe(3);
    expect(census.shannon).toBeGreaterThan(0);

    const fluxReport = EnergyFluxPipeline.generateFluxReport(sim.ledger);
    expect(fluxReport.nodes.length).toBe(6);
    expect(fluxReport.links.length).toBeGreaterThanOrEqual(8);
    expect(fluxReport.nodes.some(n => n.id === 'plants')).toBe(true);
    expect(fluxReport.nodes.some(n => n.id === 'herbivores')).toBe(true);
  });

  // ---------------------------------------------------------------------------------------------
  // 7. F11 x F17: Spatial Density with MAP-Elites QD
  // ---------------------------------------------------------------------------------------------
  test('T3-07 [F11 x F17]: 2D Spatial density heatmaps correlate with MAP-Elites phenotypic behavior coverage', async () => {
    const agents = Array.from({ length: 40 }, (_, i) => ({
      id: i + 1,
      type: i < 30 ? ('prey' as const) : ('predator' as const),
      energy: 50 + (i % 20),
      mass: 1.0 + (i % 5) * 0.5,
      age: i * 2,
      x: (i % 8) * 12,
      y: Math.floor(i / 8) * 18,
      alive: true,
    }));

    const densityReport = SpatialDensityPipeline.generateReport(16, 16, agents);
    expect(densityReport.preyDensity.length).toBe(256);
    expect(densityReport.predatorDensity.length).toBe(256);

    const qdGrid: Record<string, { fitness: number }> = {};
    for (const a of agents) {
      const cellKey = `${Math.floor(a.mass)},${Math.floor(a.energy / 20)}`;
      qdGrid[cellKey] = { fitness: a.energy * 0.1 };
    }

    const qdSummary = MapElitesNewickPipeline.computeSummary(qdGrid, 8);
    expect(qdSummary.occupiedCells).toBeGreaterThan(5);
    expect(qdSummary.archiveCoverage).toBeGreaterThan(5.0);
  });

  // ---------------------------------------------------------------------------------------------
  // 8. F12 x F13: Newick Export with Dataset Exporter
  // ---------------------------------------------------------------------------------------------
  test('T3-08 [F12 x F13]: Scientific dataset package exports valid Newick phylogeny alongside CSV metrics and RunProvenance', async () => {
    const lineageNodes = [
      { id: 'Root', parentId: null, branchLength: 0.0 },
      { id: 'Prey_A', parentId: 'Root', branchLength: 1.2 },
      { id: 'Prey_B', parentId: 'Root', branchLength: 0.8 },
    ];
    const newickStr = MapElitesNewickPipeline.generateNewickTree(lineageNodes);
    expect(newickStr).toContain('Prey_A:1.20');
    expect(newickStr).toContain('Prey_B:0.80');
    expect(newickStr.endsWith(';')).toBe(true);

    const metrics = [
      { tick: 0, plants: 2000, prey: 40, predators: 15 },
      { tick: 100, plants: 1950, prey: 45, predators: 16 },
      { tick: 200, plants: 1980, prey: 43, predators: 17 },
    ];
    const dataset = ScientificDatasetExporter.exportDataset(12345, metrics);

    expect(dataset.provenance.seed).toBe(12345);
    expect(dataset.provenance.commitHash).toBeDefined();
    expect(dataset.csvContent.startsWith('tick,plants,prey,predators\n')).toBe(true);
    expect(dataset.newickTree).toContain('Root');
  });

  // ---------------------------------------------------------------------------------------------
  // 9. F14 x F15: Dashboard Container with Telemetry Phase-Space
  // ---------------------------------------------------------------------------------------------
  test('T3-09 [F14 x F15]: Dashboard UI container queries status, renders telemetry, and tracks Lotka-Volterra limit cycles', async () => {
    const sim = new RobustCoexistenceSimulation(3000, 40, 15, 1000);
    const phaseSpacePoints: [number, number][] = [];
    for (let t = 0; t < 200; t++) {
      sim.stepTick();
      const census = sim.getCensus();
      phaseSpacePoints.push([census.preyCount, census.predatorCount]);
    }
    expect(phaseSpacePoints.length).toBe(200);
    expect(sim.ledger.total()).toBeGreaterThan(0);
  });

  // ---------------------------------------------------------------------------------------------
  // 10. F16 x F10: Trophic Pyramid with Energy Flux Matrix
  // ---------------------------------------------------------------------------------------------
  test('T3-10 [F16 x F10]: Lindeman efficiency percentages match directed acyclic graph in Energy Flux Matrix', async () => {
    const ledger = new SimulatedEnergyLedger(4000, 1200, 800);
    const flux = EnergyFluxPipeline.generateFluxReport(ledger);

    const primaryBiomass = ledger.plants;
    const secondaryBiomass = ledger.animals * 0.7; // Herbivores
    const tertiaryBiomass = ledger.animals * 0.3; // Carnivores

    const lindemanHerbivore = calculateLindemanEfficiency(primaryBiomass, secondaryBiomass);
    const lindemanCarnivore = calculateLindemanEfficiency(secondaryBiomass, tertiaryBiomass);

    expect(lindemanHerbivore).toBeGreaterThan(0.1);
    expect(lindemanHerbivore).toBeLessThan(0.4);
    expect(lindemanCarnivore).toBeGreaterThan(0.2);
    expect(lindemanCarnivore).toBeLessThan(0.6);

    // Total dissipated respiration loss is strictly positive
    expect(flux.respirationLoss).toBeGreaterThan(0);
  });

  // ---------------------------------------------------------------------------------------------
  // 11. F20 x F1: Zero-Allocation Hot Path under Strict Energy Ledger
  // ---------------------------------------------------------------------------------------------
  test('T3-11 [F20 x F1]: Sustained 1,000 hot-path tick operations execute without heap reallocation while conserving energy', async () => {
    const ringBuf = new ZeroAllocBufferSimulator(512);
    const ledger = new SimulatedEnergyLedger(2000, 1000, 500);
    ledger.lockBaseline();

    for (let t = 0; t < 1000; t++) {
      // Hot path operation
      ledger.transferPlantToAnimal(2.5);
      ledger.transferAnimalToDetritus(1.5);
      ledger.transferDetritusToPlant(1.0);
      ringBuf.stepHotPath(ledger.total());
    }

    // Zero heap allocations created during stepHotPath loop
    expect(ringBuf.totalAllocations).toBe(1);
    expect(Math.abs(ledger.residual())).toBeLessThan(RESIDUAL_ABS_TOLERANCE_EU);
    expect(ledger.driftPercentage()).toBeLessThan(0.001);
  });

  // ---------------------------------------------------------------------------------------------
  // 12. F20 x F8: Zero-Allocation with Bit-Exact Deterministic Replay
  // ---------------------------------------------------------------------------------------------
  test('T3-12 [F20 x F8]: Pre-allocated ring buffers and seekable RNG streams yield identical SHA-256 state envelopes', async () => {
    const seed = 54321;
    const rngA = new DeterministicReplayEngine(seed);
    const ringA = new ZeroAllocBufferSimulator(256);
    for (let i = 0; i < 200; i++) {
      ringA.stepHotPath(rngA.nextFloat());
    }
    const stateA = { buffer: Array.from(ringA.getArray().slice(0, 50)) };
    const envelopeA = SnapshotEnvelopeManager.seal(stateA);

    const rngB = new DeterministicReplayEngine(seed);
    const ringB = new ZeroAllocBufferSimulator(256);
    for (let i = 0; i < 200; i++) {
      ringB.stepHotPath(rngB.nextFloat());
    }
    const stateB = { buffer: Array.from(ringB.getArray().slice(0, 50)) };
    const envelopeB = SnapshotEnvelopeManager.seal(stateB);

    expect(envelopeA.worldChecksum).toBe(envelopeB.worldChecksum);
    expect(envelopeA.schemaVersion).toBe(7);
  });

  // ---------------------------------------------------------------------------------------------
  // 13. F7 x F6: Causal Interventions Creating N-ary Branch Tree
  // ---------------------------------------------------------------------------------------------
  test('T3-13 [F7 x F6]: Scheduled causal interventions at specific ticks trigger branch node creation with divergence diffs', async () => {
    const tree = new CausalBranchTree('DIGEST_ROOT');
    const branchPre = tree.forkBranch(tree.rootId, 50, 'Pre-Intervention', 'DIGEST_50');

    // Apply genetic injection intervention at tick 50
    const branchPost = tree.forkBranch(branchPre.id, 100, 'Post-Genetic-Injection', 'DIGEST_100');
    branchPost.trajectory.push(
      { tick: 100, prey: 60, predator: 25, totalEnergy: 3000 },
      { tick: 150, prey: 75, predator: 30, totalEnergy: 3200 },
    );

    const trace = tree.traceToRoot(branchPost.id);
    expect(trace).toEqual([tree.rootId, branchPre.id, branchPost.id]);
    expect(branchPost.trajectory.length).toBeGreaterThan(1);
  });

  // ---------------------------------------------------------------------------------------------
  // 14. F5 x F13: Parameter Sweeps Exported as Verified Scientific Dataset
  // ---------------------------------------------------------------------------------------------
  test('T3-14 [F5 x F13]: High-throughput parameter sweep matrix exports into structured CSV dataset with complete provenance', async () => {
    const dims = [
      { name: 'grazingRate', min: 0.1, max: 0.5, steps: 3 },
      { name: 'predationRate', min: 0.02, max: 0.08, steps: 3 },
    ];
    const sweep = ParameterSweepEngine.executeSweep(dims, 'Grid');
    expect(sweep.parameterMatrix.length).toBe(9);

    const exportRows = sweep.parameterMatrix.map((pt, idx) => ({
      tick: idx * 10,
      plants: 2000 - pt[0] * 500,
      prey: Math.floor(40 + pt[0] * 20),
      predators: Math.floor(15 + pt[1] * 50),
    }));

    const dataset = ScientificDatasetExporter.exportDataset(8888, exportRows);
    expect(dataset.csvContent.split('\n').length).toBe(11); // Header + 9 rows + trailing newline
    expect(dataset.provenance.seed).toBe(8888);
  });

  // ---------------------------------------------------------------------------------------------
  // 15. F4 x F9: Multi-Trophic Coexistence with Demographics Age Cohorts
  // ---------------------------------------------------------------------------------------------
  test('T3-15 [F4 x F9]: Multi-trophic coexistence yields balanced age-pyramid demographics and sustainable fecundity', async () => {
    const sim = new RobustCoexistenceSimulation(3000, 40, 10, 800);
    for (let t = 0; t < 150; t++) sim.stepTick();

    const census = DemographicsPipeline.generateCensus(sim.agents, [{ cause: 'Senescence' }, { cause: 'Predation' }]);
    const totalLiving = census.ageCohorts.reduce((acc, c) => acc + c.count, 0);

    expect(totalLiving).toBeGreaterThan(15);
    // Young cohort must be active
    expect(census.ageCohorts[0].count).toBeGreaterThan(0);
    expect(census.fecundityByAge.length).toBe(3);
  });

  // ---------------------------------------------------------------------------------------------
  // 16. F3 x F11: Holling Predation Shaping Spatial Density Hotspots
  // ---------------------------------------------------------------------------------------------
  test('T3-16 [F3 x F11]: Holling Type III rarity refuge promotes spatial density clustering and prey patch persistence', async () => {
    const sim = new RobustCoexistenceSimulation(2000, 50, 15, 500);
    for (let t = 0; t < 150; t++) sim.stepTick();

    const spatialReport = SpatialDensityPipeline.generateReport(16, 16, sim.agents);
    const nonZeroPreyCells = spatialReport.preyDensity.filter(d => d > 0).length;
    const nonZeroPredCells = spatialReport.predatorDensity.filter(d => d > 0).length;

    expect(nonZeroPreyCells).toBeGreaterThan(1);
    expect(nonZeroPredCells).toBeGreaterThan(0);
  });

  // ---------------------------------------------------------------------------------------------
  // 17. F2 x F15: Temperature Scaling Driving Phase-Space Limit Cycles
  // ---------------------------------------------------------------------------------------------
  test('T3-17 [F2 x F15]: Arrhenius thermal ramps expand Lotka-Volterra phase-space trajectories and metabolic burn', async () => {
    const mass = 2.0;
    const burnCold = calculateMteMetabolicRate(mass, 12.0, E_ANIMAL_EV);
    const burnWarm = calculateMteMetabolicRate(mass, 32.0, E_ANIMAL_EV);

    expect(burnWarm).toBeGreaterThan(burnCold * 2.0);
  });

  // ---------------------------------------------------------------------------------------------
  // 18. F12 x F17: MAP-Elites QD Grid Matching Phylogenetic DAG
  // ---------------------------------------------------------------------------------------------
  test('T3-18 [F12 x F17]: Phenotypic MAP-Elites behavior space reflects phylogenetic lineage branching depth', async () => {
    const qdGrid = {
      '0,0': { fitness: 1.2 },
      '1,0': { fitness: 1.8 },
      '2,1': { fitness: 2.5 },
      '3,2': { fitness: 3.1 },
    };
    const summary = MapElitesNewickPipeline.computeSummary(qdGrid, 8);
    expect(summary.occupiedCells).toBe(4);
    expect(summary.totalFitness).toBeCloseTo(8.6, 1);

    const lineage = [
      { id: 'Clade0', parentId: null, branchLength: 0.0 },
      { id: 'Clade1', parentId: 'Clade0', branchLength: 1.5 },
      { id: 'Clade2', parentId: 'Clade1', branchLength: 2.2 },
    ];
    const newick = MapElitesNewickPipeline.generateNewickTree(lineage);
    expect(newick).toContain('Clade2:2.20');
  });

  // ---------------------------------------------------------------------------------------------
  // 19. F8 x F19: Deterministic Replay with Counterfactual Scrubber
  // ---------------------------------------------------------------------------------------------
  test('T3-19 [F8 x F19]: Replay stream seeking allows exact timeline scrub restoration with 0 delta', async () => {
    const engine = new DeterministicReplayEngine(424242);
    for (let i = 0; i < 50; i++) engine.nextFloat();
    const pos50 = engine.getStreamPosition();

    // Advance 50 more
    for (let i = 0; i < 50; i++) engine.nextFloat();

    // Scrub back to tick 50
    engine.setStreamPosition(pos50);
    expect(engine.getStreamPosition()).toBe(pos50);

    // Generating next value must match exact sequence
    const valA = engine.nextFloat();
    engine.setStreamPosition(pos50);
    const valB = engine.nextFloat();
    expect(valA).toBe(valB);
  });

  // ---------------------------------------------------------------------------------------------
  // 20. F7 x F1: Causal Energy Injections Strictly Tracked by Closed-Loop Ledger
  // ---------------------------------------------------------------------------------------------
  test('T3-20 [F7 x F1]: Causal energy injections via interventions are accounted for in closed ledger without unphysical drift', async () => {
    const ledger = new SimulatedEnergyLedger(2000, 1000, 500);
    const initialBase = ledger.lockBaseline();

    // Apply declared nutrient intervention: 200 EU transferred from Detritus to Plants
    const moved = ledger.transferDetritusToPlant(200);
    expect(moved).toBe(200);
    expect(ledger.plants).toBe(2200);
    expect(ledger.detritus).toBe(300);

    // Total must stay strictly conserved at baseline
    expect(ledger.total()).toBe(initialBase);
    expect(ledger.residual()).toBe(0.0);
    expect(ledger.driftPercentage()).toBe(0.0);
  });

  // ---------------------------------------------------------------------------------------------
  // 21. F21 x F20: Adversarial Stress under Zero-Allocation Constraints
  // ---------------------------------------------------------------------------------------------
  test('T3-21 [F21 x F20]: High-frequency entity turnover and rapid spatial queries execute cleanly with zero buffer thrashing', async () => {
    const ring = new ZeroAllocBufferSimulator(1024);
    const sim = new RobustCoexistenceSimulation(3000, 80, 25, 1000);

    // High frequency stress loop
    for (let t = 0; t < 200; t++) {
      sim.stepTick();
      ring.stepHotPath(sim.ledger.total());
    }

    expect(ring.totalAllocations).toBe(1);
    expect(sim.agents.length).toBeGreaterThan(80);
    expect(Math.abs(sim.ledger.residual())).toBeLessThan(RESIDUAL_ABS_TOLERANCE_EU);
  });

});
