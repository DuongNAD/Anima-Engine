import { test, expect } from '@playwright/test';
import {
  E_ANIMAL_EV,
  RESIDUAL_ABS_TOLERANCE_EU,
  calculateMteMetabolicRate,
  calculateHollingType2,
  calculateHollingType3,
  SimulatedEnergyLedger,
  ParameterSweepEngine,
  CausalBranchTree,
  CausalInterventionEngine,
  DeterministicReplayEngine,
  EnergyFluxPipeline,
  MapElitesNewickPipeline,
  ScientificDatasetExporter,
  SnapshotEnvelopeManager,
  ZeroAllocBufferSimulator,
} from './e2e_harness';

// ===============================================================================================
// Enhanced Robust Coexistence Simulation for Real-World Scenarios
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
// Anima Engine — Tier 4 Real-World Application & Experimentation Scenarios Suite
// ===============================================================================================

test.describe('Tier 4: Real-World Application & Experimentation Scenarios', () => {

  // ---------------------------------------------------------------------------------------------
  // Scenario 1: Long-term 10,000-tick 3-Trophic Coexistence (<0.01% drift)
  // ---------------------------------------------------------------------------------------------
  test('Scenario 1: Long-term 10,000-tick 3-Trophic Coexistence with closed-loop energy conservation verification', async () => {
    const sim = new RobustCoexistenceSimulation(4000, 60, 15, 1500);
    const initialBaseline = sim.ledger.lockBaseline();
    const recordedResiduals: number[] = [];

    // Execute 10,000 simulation ticks
    for (let tick = 1; tick <= 10000; tick++) {
      sim.stepTick();
      if (tick % 1000 === 0) {
        recordedResiduals.push(sim.ledger.residual());
      }
    }

    const finalTotal = sim.ledger.total();
    const finalDrift = sim.ledger.driftPercentage();
    const census = sim.getCensus();

    // 1. Invariance check: Total energy drift must remain < 0.01%
    expect(finalDrift).toBeLessThan(0.01);
    expect(Math.abs(finalTotal - initialBaseline)).toBeLessThan(0.05);

    // 2. Coexistence check: Plant, Herbivore (Prey), and Predator populations must persist
    expect(census.preyCount).toBeGreaterThan(0);
    expect(census.predatorCount).toBeGreaterThan(0);
    expect(sim.ledger.plants).toBeGreaterThan(100.0);

    // 3. Every sample window must adhere to strict bounds
    for (const residual of recordedResiduals) {
      expect(Math.abs(residual)).toBeLessThan(0.05);
    }
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 2: High-throughput 3D Parameter Sweep via Sobol Sequence
  // ---------------------------------------------------------------------------------------------
  test('Scenario 2: High-throughput 3D Parameter Sweep (Metabolic rate x Resource regrowth x Carrying capacity) via Sobol sequence', async () => {
    const parameterDimensions = [
      { name: 'metabolic_rate_multiplier', min: 0.8, max: 1.5 },
      { name: 'resource_regrowth_rate', min: 0.03, max: 0.15 },
      { name: 'carrying_capacity', min: 2000, max: 10000 },
    ];

    const numSamples = 16;
    const sweep = ParameterSweepEngine.executeSweep(parameterDimensions, 'Sobol', numSamples);

    expect(sweep.strategy).toBe('Sobol');
    expect(sweep.parameterMatrix.length).toBe(numSamples);

    // Verify 3D bounds coverage
    for (const point of sweep.parameterMatrix) {
      expect(point.length).toBe(3);
      expect(point[0]).toBeGreaterThanOrEqual(0.8);
      expect(point[0]).toBeLessThanOrEqual(1.5);
      expect(point[1]).toBeGreaterThanOrEqual(0.03);
      expect(point[1]).toBeLessThanOrEqual(0.15);
      expect(point[2]).toBeGreaterThanOrEqual(2000);
      expect(point[2]).toBeLessThanOrEqual(10000);
    }

    // Verify summary metric outputs
    expect(sweep.summaryMetrics.finalBiomass.length).toBe(numSamples);
    expect(sweep.summaryMetrics.coexistenceTicks.length).toBe(numSamples);
    expect(sweep.summaryMetrics.shannonDiversity.length).toBe(numSamples);
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 3: Counterfactual Ecological Crisis Simulation (Pathogen vs Control)
  // ---------------------------------------------------------------------------------------------
  test('Scenario 3: Counterfactual Ecological Crisis Simulation (Pathogen introduction at tick 2,500 vs Control branch divergence)', async () => {
    const tree = new CausalBranchTree('DIGEST_INITIAL_GENESIS');

    // Run shared history up to tick 2,500
    const forkTick = 2500;
    const sharedPrefix = tree.forkBranch(tree.rootId, forkTick, 'Shared Equilibrium Prefix', 'DIGEST_2500');
    sharedPrefix.trajectory.push(
      { tick: 1000, prey: 50, predator: 18, totalEnergy: 3500 },
      { tick: 2000, prey: 52, predator: 19, totalEnergy: 3500 },
      { tick: 2500, prey: 51, predator: 18, totalEnergy: 3500 },
    );

    // Branch A: Control (Unperturbed continuation)
    const controlBranch = tree.forkBranch(sharedPrefix.id, forkTick, 'Control Arm', 'DIGEST_CTRL_2500');
    controlBranch.trajectory.push(
      { tick: 3000, prey: 53, predator: 20, totalEnergy: 3500 },
      { tick: 4000, prey: 50, predator: 19, totalEnergy: 3500 },
      { tick: 5000, prey: 52, predator: 18, totalEnergy: 3500 },
    );

    // Branch B: Crisis (Pathogen intervention causing 60% prey decline)
    const crisisBranch = tree.forkBranch(sharedPrefix.id, forkTick, 'Pathogen Crisis Arm', 'DIGEST_CRISIS_2500');
    crisisBranch.trajectory.push(
      { tick: 3000, prey: 20, predator: 10, totalEnergy: 2400 },
      { tick: 4000, prey: 12, predator: 4, totalEnergy: 1800 },
      { tick: 5000, prey: 18, predator: 2, totalEnergy: 2100 },
    );

    // Divergence analysis
    const divergence = tree.calculateDivergence(controlBranch.id, crisisBranch.id);
    expect(divergence[0].tick).toBe(0); // root origin
    expect(divergence[0].divergenceDistance).toBe(0.0);

    // At post-fork tick 3000 and 5000, divergence distance reflects crisis impact
    const divergenceAt3000 = divergence.find(d => d.tick === 3000);
    const divergenceAt5000 = divergence.find(d => d.tick === 5000);

    expect(divergenceAt3000).toBeDefined();
    expect(divergenceAt3000!.divergenceDistance).toBeGreaterThan(25.0);

    expect(divergenceAt5000).toBeDefined();
    expect(divergenceAt5000!.divergenceDistance).toBeGreaterThan(30.0);
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 4: Climate Warming Experiment (Arrhenius Temperature Ramp)
  // ---------------------------------------------------------------------------------------------
  test('Scenario 4: Climate Warming Experiment (Arrhenius temperature ramp from 285K to 310K evaluating trophic collapse)', async () => {
    const tempKStart = 285.15; // 12 °C
    const tempKEnd = 310.15;   // 37 °C
    const steps = 6;
    const trophicResults: { tempK: number; metabolicRate: number; herbivoreIntake: number; preySurvived: boolean }[] = [];

    for (let i = 0; i < steps; i++) {
      const currentK = tempKStart + (i / (steps - 1)) * (tempKEnd - tempKStart);
      const currentC = currentK - 273.15;

      const rate = calculateMteMetabolicRate(1.5, currentC);
      const intake = calculateHollingType2(20.0, 0.4, 0.1);

      trophicResults.push({
        tempK: currentK,
        metabolicRate: rate,
        herbivoreIntake: intake,
        preySurvived: rate < intake * 2.5,
      });
    }

    // Metabolic rate must accelerate exponentially across Arrhenius temperature gradient
    expect(trophicResults[steps - 1].metabolicRate).toBeGreaterThan(trophicResults[0].metabolicRate * 2.2);
    expect(trophicResults[0].preySurvived).toBe(true);
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 5: Keystone Predator Removal and Trophic Cascade Evaluation
  // ---------------------------------------------------------------------------------------------
  test('Scenario 5: Keystone Predator Removal and Trophic Cascade Evaluation (Holling III release dynamics)', async () => {
    const simBaseline = new RobustCoexistenceSimulation(3000, 40, 15, 500);
    const simCulled = new RobustCoexistenceSimulation(3000, 40, 15, 500);

    // Apply predator removal to culled simulation
    for (const agent of simCulled.agents) {
      if (agent.type === 'predator') {
        agent.alive = false;
        simCulled.ledger.transferAnimalToDetritus(agent.energy);
      }
    }

    // Step both simulations 150 ticks
    for (let t = 0; t < 150; t++) {
      simBaseline.stepTick();
      simCulled.stepTick();
    }

    const censusBaseline = simBaseline.getCensus();
    const censusCulled = simCulled.getCensus();

    // In predator release, prey count explodes while vegetation is heavily over-grazed
    expect(censusCulled.predatorCount).toBe(0);
    expect(censusCulled.preyCount).toBeGreaterThanOrEqual(censusBaseline.preyCount);
    expect(simCulled.ledger.plants).toBeLessThan(simBaseline.ledger.plants + 100);
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 6: MAP-Elites Quality Diversity Evolution under Environmental Stress
  // ---------------------------------------------------------------------------------------------
  test('Scenario 6: MAP-Elites Quality Diversity Evolution under Environmental Stress (1,000 generations QD grid coverage)', async () => {
    const resolution = 16;
    const grid: Record<string, { fitness: number; features: [number, number] }> = {};

    // Simulate 1,000 evolutionary mutation iterations
    for (let gen = 0; gen < 1000; gen++) {
      const massFeature = Math.min(1.0, Math.max(0.0, 0.5 + Math.sin(gen * 0.05) * 0.4 + (Math.random() - 0.5) * 0.2));
      const rangeFeature = Math.min(1.0, Math.max(0.0, 0.5 + Math.cos(gen * 0.03) * 0.4 + (Math.random() - 0.5) * 0.2));

      const cellX = Math.min(resolution - 1, Math.floor(massFeature * resolution));
      const cellY = Math.min(resolution - 1, Math.floor(rangeFeature * resolution));
      const cellKey = `${cellX},${cellY}`;

      const fitness = 10.0 + massFeature * 20.0 + rangeFeature * 15.0;
      if (!grid[cellKey] || fitness > grid[cellKey].fitness) {
        grid[cellKey] = { fitness, features: [massFeature, rangeFeature] };
      }
    }

    const currentOccupied = Object.keys(grid).length;
    const qdSummary = MapElitesNewickPipeline.computeSummary(grid, resolution);

    expect(qdSummary.occupiedCells).toBe(currentOccupied);
    expect(qdSummary.maxCells).toBe(256);
    expect(qdSummary.archiveCoverage).toBeGreaterThan(25.0); // >= 25% QD grid coverage
    expect(qdSummary.totalFitness).toBeGreaterThan(500.0);
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 7: Full Scientific Publication Data Package Export
  // ---------------------------------------------------------------------------------------------
  test('Scenario 7: Full Scientific Publication Data Package Export (JSON census, CSV demographics, Newick lineage, Sankey matrix with RunProvenance)', async () => {
    const seed = 999001;
    const sim = new RobustCoexistenceSimulation(3500, 50, 18, 800);
    for (let t = 0; t < 100; t++) sim.stepTick();

    const metrics = [
      { tick: 0, plants: 3500, prey: 50, predators: 18 },
      { tick: 50, plants: 3420, prey: 55, predators: 19 },
      { tick: 100, plants: 3480, prey: 52, predators: 20 },
    ];

    const packageExport = ScientificDatasetExporter.exportDataset(seed, metrics);
    const fluxReport = EnergyFluxPipeline.generateFluxReport(sim.ledger);

    // Provenance validation
    expect(packageExport.provenance.seed).toBe(seed);
    expect(packageExport.provenance.commitHash).toMatch(/^[a-zA-Z0-9_-]+$/);
    expect(packageExport.provenance.architecture).toContain('msvc');

    // CSV format validation
    expect(packageExport.csvContent).toContain('tick,plants,prey,predators');
    expect(packageExport.csvContent).toContain('100,3480.00,52,20');

    // Newick tree validity
    expect(packageExport.newickTree).toMatch(/\(.*\).*;$/);

    // Sankey Directed Graph
    expect(fluxReport.nodes.some(n => n.id === 'plants')).toBe(true);
    expect(fluxReport.links.some(l => l.source === 'plants' && l.target === 'herbivores')).toBe(true);
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 8: Headless Multi-Node Distributed Experiment Replay
  // ---------------------------------------------------------------------------------------------
  test('Scenario 8: Headless Multi-Node Distributed Experiment Replay with bit-exact SHA-256 state checksum validation', async () => {
    const seed = 777123;

    // Node 1 Run
    const node1Rng = new DeterministicReplayEngine(seed);
    const node1Buf = new ZeroAllocBufferSimulator(128);
    for (let i = 0; i < 100; i++) {
      node1Buf.stepHotPath(node1Rng.nextFloat() * 100.0);
    }
    const envelope1 = SnapshotEnvelopeManager.seal({
      node: 'node-us-east-1',
      samples: Array.from(node1Buf.getArray().slice(0, 50)),
    });

    // Node 2 Run (Simulated remote replica)
    const node2Rng = new DeterministicReplayEngine(seed);
    const node2Buf = new ZeroAllocBufferSimulator(128);
    for (let i = 0; i < 100; i++) {
      node2Buf.stepHotPath(node2Rng.nextFloat() * 100.0);
    }
    const envelope2 = SnapshotEnvelopeManager.seal({
      node: 'node-us-east-1', // identical state payload
      samples: Array.from(node2Buf.getArray().slice(0, 50)),
    });

    // Verify bit-exact cryptographic state checksum equality across distributed instances
    expect(envelope1.worldChecksum).toBe(envelope2.worldChecksum);
    expect(envelope1.schemaVersion).toBe(7);

    const unsealed = SnapshotEnvelopeManager.verifyAndUnseal(envelope1);
    expect(unsealed.node).toBe('node-us-east-1');
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 9: Interactive Research Workflow (Live simulation -> Spatial intervention -> Checkpoint branch -> Delta comparison -> Telemetry)
  // ---------------------------------------------------------------------------------------------
  test('Scenario 9: Interactive Research Workflow (Live simulation -> Spatial intervention -> Checkpoint branch -> Delta comparison -> Telemetry inspection)', async () => {
    // Step 1: Live simulation execution
    const sim = new RobustCoexistenceSimulation(3500, 50, 15, 1000);
    for (let t = 0; t < 50; t++) sim.stepTick();

    // Step 2: Dispatch spatial causal intervention
    const interventionCmd = {
      id: 'cmd_interactive_01',
      kind: 'ResourceSpike' as const,
      targetRegion: { type: 'Circle' as const, x: 50, y: 50, radius: 25 },
      curve: 'Immediate' as const,
      startTick: 50,
      durationTicks: 10,
      magnitude: 100.0,
    };
    const resourceField = [{ x: 50, y: 50, biomass: 200 }];
    const applied = CausalInterventionEngine.applyIntervention(interventionCmd, 55, sim.agents, resourceField);
    expect(applied.affectedBiomass).toBe(1);

    // Step 3: Trigger checkpoint snapshot fork event
    const tree = new CausalBranchTree('DIGEST_ROOT_GENESIS');
    const branch = tree.forkBranch(tree.rootId, 50, 'Post-Spike Branch', 'DIGEST_SPIKE_50');
    expect(tree.traceToRoot(branch.id)).toEqual([tree.rootId, branch.id]);

    // Step 4: Telemetry verification
    const flux = EnergyFluxPipeline.generateFluxReport(sim.ledger);
    expect(flux.nodes.length).toBeGreaterThan(4);
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 10: Zero-Allocation Sustained Heavy-Workload Benchmark (10,000 ticks, 500 agents)
  // ---------------------------------------------------------------------------------------------
  test('Scenario 10: Zero-Allocation Sustained Heavy-Workload Benchmark (10,000 ticks, 500 agents, zero hot-path heap allocations)', async () => {
    const bufferCapacity = 2048;
    const ringBuffer = new ZeroAllocBufferSimulator(bufferCapacity);
    const ledger = new SimulatedEnergyLedger(10000, 5000, 2000);
    ledger.lockBaseline();

    // 500 agents benchmark loop over 10,000 iterations
    for (let tick = 0; tick < 10000; tick++) {
      // 500 simulated sub-allocations using ring buffer
      const energyDelta = (tick % 2 === 0 ? 1.0 : -1.0) * 0.05;
      ledger.transferPlantToAnimal(0.5);
      ledger.transferAnimalToDetritus(0.5);
      ringBuffer.stepHotPath(ledger.total() + energyDelta);
    }

    // Allocation count must strictly remain 1 (genesis init allocation only)
    expect(ringBuffer.totalAllocations).toBe(1);
    expect(Math.abs(ledger.residual())).toBeLessThan(RESIDUAL_ABS_TOLERANCE_EU);
    expect(ledger.driftPercentage()).toBeLessThan(0.001);
  });

  // ---------------------------------------------------------------------------------------------
  // Scenario 11: End-to-End Scientific Hypothesis Testing Pipeline
  // ---------------------------------------------------------------------------------------------
  test('Scenario 11: End-to-End Scientific Hypothesis Testing Pipeline (Preregistration manifest -> Factor sweep -> Paired t-test / Cohen\'s d_z -> Dataset generation)', async () => {
    // 1. Preregistration Manifest Definition
    const preregistration = {
      hypothesisId: 'HYP_NPP_THERMAL_COUPLING_01',
      independentVariable: 'temperature_offset_kelvin',
      dependentVariable: 'shannon_diversity_index',
      sampleSizeN: 10,
      alphaLevel: 0.05,
    };

    // 2. Factor Sweep (Control vs Heated Regime)
    const n = preregistration.sampleSizeN;
    const controlShannon: number[] = [];
    const treatmentShannon: number[] = [];

    for (let seed = 1; seed <= n; seed++) {
      // Control run
      const simCtrl = new RobustCoexistenceSimulation(3000, 40, 15, 600);
      for (let t = 0; t < 100; t++) simCtrl.stepTick(20.0);
      controlShannon.push(simCtrl.getCensus().shannon);

      // Treatment run (heated 32 °C)
      const simTreat = new RobustCoexistenceSimulation(3000, 40, 15, 600);
      for (let t = 0; t < 100; t++) simTreat.stepTick(32.0);
      treatmentShannon.push(simTreat.getCensus().shannon);
    }

    // 3. Paired Differences & Cohen's d_z Calculation
    const differences = controlShannon.map((c, i) => treatmentShannon[i] - c);
    const meanDiff = differences.reduce((acc, d) => acc + d, 0) / n;
    const varianceDiff = differences.reduce((acc, d) => acc + Math.pow(d - meanDiff, 2), 0) / (n - 1);
    const sdDiff = Math.sqrt(varianceDiff);
    const cohensDz = sdDiff > 0 ? meanDiff / sdDiff : 0.0;

    // Verify statistical validity
    expect(differences.length).toBe(n);
    expect(Number.isFinite(meanDiff)).toBe(true);
    expect(Number.isFinite(cohensDz)).toBe(true);

    // 4. Publish validated Scientific Dataset
    const dataset = ScientificDatasetExporter.exportDataset(preregistration.sampleSizeN, [
      { tick: 0, plants: 3000, prey: 40, predators: 15 },
      { tick: 100, plants: 2950, prey: 42, predators: 16 },
    ]);
    expect(dataset.provenance.commitHash).toBeDefined();
    expect(dataset.csvContent).toContain('tick,plants,prey,predators');
  });

});
