import type { Page } from '@playwright/test';
import type { RecordedInvocation, TauriEventEnvelope, TauriInvokeArgs } from '../tauri-mock-types';
import '../tauri-mock-types';

// ===============================================================================================
// CONSTANTS & SCIENTIFIC PARAMETERS (MTE, Holling, Physics, Energy Conservation)
// ===============================================================================================

export const BOLTZMANN_EV = 8.617333e-5;
export const REF_TEMP_K = 293.15; // 20 °C
export const E_ANIMAL_EV = 0.65;
export const E_PRODUCER_EV = 0.30;
export const METABOLIC_NORM = 0.06;
export const KLEIBER_EXP = 0.75;
export const RESIDUAL_ABS_TOLERANCE_EU = 1e-3;

// ===============================================================================================
// 1. MATHEMATICAL & ECOLOGICAL LAWS CALCULATORS
// ===============================================================================================

/** Arrhenius / Boltzmann temperature scaling factor */
export function boltzmannFactor(tempC: number, activationEv: number = E_ANIMAL_EV): number {
  const tk = tempC + 273.15;
  if (tk <= 0.0) return 0.0;
  return Math.exp((activationEv / BOLTZMANN_EV) * (1.0 / REF_TEMP_K - 1.0 / tk));
}

/** Whole-organism maintenance metabolic rate: MTE Kleiber M^(3/4) * e^(-E/kT) */
export function calculateMteMetabolicRate(
  mass: number,
  tempC: number = 20.0,
  activationEv: number = E_ANIMAL_EV,
): number {
  const m = Math.max(0.0, mass);
  return METABOLIC_NORM * Math.pow(m, KLEIBER_EXP) * boltzmannFactor(tempC, activationEv);
}

/** Holling Type II hyperbolic functional response: a*N / (1 + a*h*N) */
export function calculateHollingType2(preyDensity: number, attackRate: number, handlingTime: number): number {
  const n = Math.max(0.0, preyDensity);
  return (attackRate * n) / (1.0 + attackRate * handlingTime * n);
}

/** Holling Type III sigmoid functional response: a*N^2 / (1 + a*h*N^2) */
export function calculateHollingType3(preyDensity: number, attackRate: number, handlingTime: number): number {
  const n = Math.max(0.0, preyDensity);
  const n2 = n * n;
  return (attackRate * n2) / (1.0 + attackRate * handlingTime * n2);
}

/** Discrete logistic regrowth with tiny seed for depleted cells */
export function calculateLogisticRegrowth(r: number, growthRate: number, rMax: number): number {
  if (rMax <= 0.0) return 0.0;
  const seed = r < 1e-4 ? 1e-3 * rMax : 0.0;
  const next = r + seed + growthRate * r * (1.0 - r / rMax);
  return Math.min(Math.max(0.0, next), rMax);
}

/** Shannon diversity index H' = - sum(p_i * ln(p_i)) */
export function calculateShannonIndex(abundances: number[]): number {
  const total = abundances.reduce((acc, v) => acc + v, 0);
  if (total <= 0) return 0;
  let h = 0;
  for (const n of abundances) {
    if (n > 0) {
      const p = n / total;
      h -= p * Math.log(p);
    }
  }
  return h;
}

/** Simpson diversity index D = 1 - sum(p_i^2) */
export function calculateSimpsonIndex(abundances: number[]): number {
  const total = abundances.reduce((acc, v) => acc + v, 0);
  if (total <= 0) return 0;
  let sumP2 = 0;
  for (const n of abundances) {
    const p = n / total;
    sumP2 += p * p;
  }
  return 1.0 - sumP2;
}

/** Lindeman trophic transfer efficiency: secondary / primary */
export function calculateLindemanEfficiency(primaryBiomass: number, secondaryBiomass: number): number {
  if (primaryBiomass <= 0) return 0;
  return secondaryBiomass / primaryBiomass;
}

// ===============================================================================================
// 2. CLOSED-LOOP ENERGY & BIOMASS LEDGER
// ===============================================================================================

export interface EnergyLedgerSnapshot {
  plants: number;
  animals: number;
  detritus: number;
  total: number;
  baseline: number;
  drift: number;
  granted: number;
  refused: number;
  settled: number;
}

export class SimulatedEnergyLedger {
  public plants: number;
  public animals: number;
  public detritus: number;
  public baseline: number | null = null;
  public granted: number = 0;
  public refused: number = 0;
  public settled: number = 0;

  constructor(plants: number, animals: number, detritus: number) {
    this.plants = plants;
    this.animals = animals;
    this.detritus = detritus;
  }

  public lockBaseline(): number {
    if (this.baseline === null) {
      this.baseline = this.total();
    }
    return this.baseline;
  }

  public total(): number {
    return this.plants + this.animals + this.detritus;
  }

  public residual(): number {
    if (this.baseline === null) return 0.0;
    return this.total() - this.baseline;
  }

  public driftPercentage(): number {
    if (this.baseline === null || this.baseline === 0) return 0.0;
    return (Math.abs(this.residual()) / this.baseline) * 100.0;
  }

  public transferPlantToAnimal(demand: number): number {
    const available = Math.min(demand, this.plants);
    if (available > 0) {
      this.plants -= available;
      this.animals += available;
      this.granted += available;
      this.settled += 1;
    }
    if (demand > available) {
      this.refused += (demand - available);
    }
    return available;
  }

  public transferAnimalToDetritus(amount: number): void {
    const actual = Math.min(amount, this.animals);
    this.animals -= actual;
    this.detritus += actual;
    this.settled += 1;
  }

  public transferDetritusToPlant(demand: number): number {
    const available = Math.min(demand, this.detritus);
    if (available > 0) {
      this.detritus -= available;
      this.plants += available;
      this.granted += available;
      this.settled += 1;
    }
    if (demand > available) {
      this.refused += (demand - available);
    }
    return available;
  }

  public getSnapshot(): EnergyLedgerSnapshot {
    return {
      plants: this.plants,
      animals: this.animals,
      detritus: this.detritus,
      total: this.total(),
      baseline: this.baseline ?? this.total(),
      drift: this.driftPercentage(),
      granted: this.granted,
      refused: this.refused,
      settled: this.settled,
    };
  }
}

// ===============================================================================================
// 3. MULTI-TROPHIC COEXISTENCE SIMULATION ENGINE
// ===============================================================================================

export interface TrophicAgent {
  id: number;
  type: 'prey' | 'predator';
  energy: number;
  mass: number;
  age: number;
  x: number;
  y: number;
  alive: boolean;
}

export class MultiTrophicSimulation {
  public ledger: SimulatedEnergyLedger;
  public agents: TrophicAgent[] = [];
  public currentTick: number = 0;
  public carryingCapacity: number = 5000.0;
  public plantGrowthRate: number = 0.08;
  public reproThresholdPrey: number = 80.0;
  public reproThresholdPredator: number = 140.0;
  public nextAgentId: number = 1;

  constructor(initialPlants: number = 2000, initialPrey: number = 40, initialPredators: number = 15, detritus: number = 500) {
    let animalEnergy = 0;
    for (let i = 0; i < initialPrey; i++) {
      const e = 50.0;
      this.agents.push({
        id: this.nextAgentId++,
        type: 'prey',
        energy: e,
        mass: 1.0,
        age: Math.floor(Math.random() * 50),
        x: Math.random() * 100,
        y: Math.random() * 100,
        alive: true,
      });
      animalEnergy += e;
    }
    for (let i = 0; i < initialPredators; i++) {
      const e = 100.0;
      this.agents.push({
        id: this.nextAgentId++,
        type: 'predator',
        energy: e,
        mass: 2.5,
        age: Math.floor(Math.random() * 50),
        x: Math.random() * 100,
        y: Math.random() * 100,
        alive: true,
      });
      animalEnergy += e;
    }

    this.ledger = new SimulatedEnergyLedger(initialPlants, animalEnergy, detritus);
    this.ledger.lockBaseline();
  }

  public stepTick(tempC: number = 20.0): void {
    this.currentTick++;

    // 1. Plant regrowth from detritus via Whittaker logistic dynamics
    const plantDemand = calculateLogisticRegrowth(this.ledger.plants, this.plantGrowthRate, this.carryingCapacity) - this.ledger.plants;
    if (plantDemand > 0) {
      this.ledger.transferDetritusToPlant(plantDemand);
    }

    // 2. Herbivore grazing (Holling II)
    const livingPrey = this.agents.filter(a => a.alive && a.type === 'prey');
    for (const prey of livingPrey) {
      const intakeRate = calculateHollingType2(this.ledger.plants / 100.0, 0.4, 0.1);
      const actualEaten = this.ledger.transferPlantToAnimal(intakeRate);
      prey.energy += actualEaten;
      // Subtract from ledger directly mediated by transfer
      this.ledger.animals += (actualEaten - actualEaten); // net zero adjustment
    }

    // 3. Carnivore predation (Holling III with satiety & rarity refuge)
    const livingPredators = this.agents.filter(a => a.alive && a.type === 'predator');
    const preyDensity = livingPrey.length;
    for (const pred of livingPredators) {
      if (pred.energy >= 120.0) continue; // Satiated predators do not hunt

      const killRate = calculateHollingType3(preyDensity, 0.005, 0.2);
      const attackProb = Math.min(0.2, killRate * 0.1);
      if (Math.random() < attackProb && livingPrey.length > 3) {
        // Attack a random prey
        const targetPrey = livingPrey[Math.floor(Math.random() * livingPrey.length)];
        if (targetPrey && targetPrey.alive) {
          const transferEnergy = Math.min(targetPrey.energy, 15.0);
          targetPrey.energy -= transferEnergy;
          pred.energy += transferEnergy * 0.7; // 70% assimilation
          const unassimilated = transferEnergy * 0.3; // 30% lost to detritus
          this.ledger.transferAnimalToDetritus(unassimilated);
          if (targetPrey.energy <= 0) {
            targetPrey.alive = false;
          }
        }
      }
    }

    // 4. Metabolic Decay according to MTE & Age
    for (const agent of this.agents.filter(a => a.alive)) {
      agent.age++;
      const burn = calculateMteMetabolicRate(agent.mass, tempC, E_ANIMAL_EV);
      const actualBurn = Math.min(burn, agent.energy);
      agent.energy -= actualBurn;
      this.ledger.transferAnimalToDetritus(actualBurn);

      // Starvation or Senescence
      if (agent.energy <= 0 || agent.age > 400) {
        agent.alive = false;
        if (agent.energy > 0) {
          this.ledger.transferAnimalToDetritus(agent.energy);
          agent.energy = 0;
        }
      }
    }

    // 5. Autonomous continuous intra-tick reproduction on surplus
    const newOffspring: TrophicAgent[] = [];
    for (const agent of this.agents.filter(a => a.alive)) {
      const threshold = agent.type === 'prey' ? this.reproThresholdPrey : this.reproThresholdPredator;
      if (agent.energy >= threshold) {
        const offspringEnergy = threshold * 0.45;
        agent.energy -= offspringEnergy;
        newOffspring.push({
          id: this.nextAgentId++,
          type: agent.type,
          energy: offspringEnergy,
          mass: agent.mass * (0.95 + Math.random() * 0.1),
          age: 0,
          x: agent.x + (Math.random() * 4 - 2),
          y: agent.y + (Math.random() * 4 - 2),
          alive: true,
        });
      }
    }
    this.agents.push(...newOffspring);
  }

  public getCensus(): { preyCount: number; predatorCount: number; shannon: number; simpson: number } {
    const preyCount = this.agents.filter(a => a.alive && a.type === 'prey').length;
    const predatorCount = this.agents.filter(a => a.alive && a.type === 'predator').length;
    const shannon = calculateShannonIndex([preyCount, predatorCount]);
    const simpson = calculateSimpsonIndex([preyCount, predatorCount]);
    return { preyCount, predatorCount, shannon, simpson };
  }
}

// ===============================================================================================
// 4. PARAMETER SWEEPS & SAMPLING ENGINE
// ===============================================================================================

export type SamplingStrategy = 'Grid' | 'Sobol' | 'LatinHypercube';

export interface ParameterDim {
  name: string;
  min: number;
  max: number;
  steps?: number;
}

export interface SweepResult {
  manifestFingerprint: string;
  strategy: SamplingStrategy;
  parameterMatrix: number[][];
  summaryMetrics: Record<string, number[]>;
}

export class ParameterSweepEngine {
  public static generateGrid(dims: ParameterDim[]): number[][] {
    if (dims.length === 0) return [];
    let combinations: number[][] = [[]];
    for (const dim of dims) {
      const steps = dim.steps && dim.steps > 1 ? dim.steps : 2;
      const values: number[] = [];
      const stepSize = (dim.max - dim.min) / (steps - 1);
      for (let i = 0; i < steps; i++) {
        values.push(dim.min + i * stepSize);
      }
      const nextCombos: number[][] = [];
      for (const combo of combinations) {
        for (const val of values) {
          nextCombos.push([...combo, val]);
        }
      }
      combinations = nextCombos;
    }
    return combinations;
  }

  public static generateSobol(dims: ParameterDim[], numSamples: number): number[][] {
    const matrix: number[][] = [];
    const d = dims.length;
    for (let i = 0; i < numSamples; i++) {
      const point: number[] = [];
      for (let j = 0; j < d; j++) {
        // Van der Corput quasi-random sequence base 2 / base 3
        let n = i + 1;
        let q = 0.0;
        let bk = (j === 0 ? 0.5 : 1.0 / 3.0);
        const base = (j === 0 ? 2 : 3);
        while (n > 0) {
          q += (n % base) * bk;
          n = Math.floor(n / base);
          bk /= base;
        }
        const val = dims[j].min + q * (dims[j].max - dims[j].min);
        point.push(val);
      }
      matrix.push(point);
    }
    return matrix;
  }

  public static generateLhs(dims: ParameterDim[], numSamples: number): number[][] {
    const d = dims.length;
    const matrix: number[][] = Array.from({ length: numSamples }, () => []);
    for (let j = 0; j < d; j++) {
      const dim = dims[j];
      const intervals = Array.from({ length: numSamples }, (_, i) => i);
      // Fisher-Yates shuffle
      for (let i = intervals.length - 1; i > 0; i--) {
        const randIdx = Math.floor(Math.random() * (i + 1));
        [intervals[i], intervals[randIdx]] = [intervals[randIdx], intervals[i]];
      }
      for (let i = 0; i < numSamples; i++) {
        const u = (intervals[i] + 0.5) / numSamples;
        const val = dim.min + u * (dim.max - dim.min);
        matrix[i].push(val);
      }
    }
    return matrix;
  }

  public static executeSweep(dims: ParameterDim[], strategy: SamplingStrategy, numSamples: number = 8): SweepResult {
    let parameterMatrix: number[][];
    if (strategy === 'Grid') {
      parameterMatrix = this.generateGrid(dims);
    } else if (strategy === 'Sobol') {
      parameterMatrix = this.generateSobol(dims, numSamples);
    } else {
      parameterMatrix = this.generateLhs(dims, numSamples);
    }

    const summaryMetrics: Record<string, number[]> = {
      finalBiomass: [],
      coexistenceTicks: [],
      shannonDiversity: [],
    };

    for (const point of parameterMatrix) {
      // Evaluate simulated outcome deterministically from parameter point
      const plantRate = point[0] ?? 0.05;
      const sim = new MultiTrophicSimulation(2000, 30, 10, 500);
      sim.plantGrowthRate = plantRate;
      for (let t = 0; t < 50; t++) sim.stepTick();
      const census = sim.getCensus();
      summaryMetrics.finalBiomass.push(sim.ledger.total());
      summaryMetrics.coexistenceTicks.push(census.preyCount > 0 && census.predatorCount > 0 ? 50 : 20);
      summaryMetrics.shannonDiversity.push(census.shannon);
    }

    return {
      manifestFingerprint: 'FP_SWEEP_' + strategy.toUpperCase() + '_' + parameterMatrix.length,
      strategy,
      parameterMatrix,
      summaryMetrics,
    };
  }
}

// ===============================================================================================
// 5. N-ARY CAUSAL BRANCH TREE & COUNTERFACTUAL ENGINE
// ===============================================================================================

export interface BranchNode {
  id: string;
  parentId: string | null;
  checkpointTick: number;
  label: string;
  stateDigest: string;
  trajectory: { tick: number; prey: number; predator: number; totalEnergy: number }[];
  childrenIds: string[];
}

export class CausalBranchTree {
  public rootId: string;
  public nodes: Map<string, BranchNode> = new Map();

  constructor(rootDigest: string = 'DIGEST_ROOT_0') {
    this.rootId = 'branch_root';
    this.nodes.set(this.rootId, {
      id: this.rootId,
      parentId: null,
      checkpointTick: 0,
      label: 'Root Baseline',
      stateDigest: rootDigest,
      trajectory: [{ tick: 0, prey: 40, predator: 15, totalEnergy: 2500 }],
      childrenIds: [],
    });
  }

  public forkBranch(parentId: string, tick: number, label: string, digest: string): BranchNode {
    const parent = this.nodes.get(parentId);
    if (!parent) throw new Error(`Parent node ${parentId} not found`);
    const newId = `branch_${Math.random().toString(36).substring(2, 9)}`;
    const newNode: BranchNode = {
      id: newId,
      parentId,
      checkpointTick: tick,
      label,
      stateDigest: digest,
      trajectory: parent.trajectory.filter(t => t.tick <= tick),
      childrenIds: [],
    };
    parent.childrenIds.push(newId);
    this.nodes.set(newId, newNode);
    return newNode;
  }

  public traceToRoot(nodeId: string): string[] {
    const path: string[] = [];
    let curr: string | null = nodeId;
    while (curr) {
      path.unshift(curr);
      curr = this.nodes.get(curr)?.parentId ?? null;
    }
    return path;
  }

  public calculateDivergence(branchAId: string, branchBId: string): { tick: number; divergenceDistance: number }[] {
    const nodeA = this.nodes.get(branchAId);
    const nodeB = this.nodes.get(branchBId);
    if (!nodeA || !nodeB) return [];

    const divergence: { tick: number; divergenceDistance: number }[] = [];
    const maxLen = Math.min(nodeA.trajectory.length, nodeB.trajectory.length);
    for (let i = 0; i < maxLen; i++) {
      const a = nodeA.trajectory[i];
      const b = nodeB.trajectory[i];
      const dPrey = a.prey - b.prey;
      const dPred = a.predator - b.predator;
      const dist = Math.sqrt(dPrey * dPrey + dPred * dPred);
      divergence.push({ tick: a.tick, divergenceDistance: dist });
    }
    return divergence;
  }
}

// ===============================================================================================
// 6. CAUSAL INTERVENTION ENGINE
// ===============================================================================================

export type InterventionKind = 'GeneticInjection' | 'PathogenTransmission' | 'ToxicityPlume' | 'ResourceSpike';
export type TemporalCurveType = 'Immediate' | 'Linear' | 'Exponential' | 'Sigmoid';

export interface SpatialRegion {
  type: 'Circle' | 'Rectangle';
  x: number;
  y: number;
  radius?: number;
  width?: number;
  height?: number;
}

export interface InterventionCommand {
  id: string;
  kind: InterventionKind;
  targetRegion: SpatialRegion;
  curve: TemporalCurveType;
  startTick: number;
  durationTicks: number;
  magnitude: number;
}

export class CausalInterventionEngine {
  public static evaluateCurve(curve: TemporalCurveType, elapsed: number, duration: number): number {
    if (duration <= 0) return 1.0;
    const progress = Math.min(Math.max(0.0, elapsed / duration), 1.0);
    switch (curve) {
      case 'Immediate': return 1.0;
      case 'Linear': return progress;
      case 'Exponential': return Math.pow(progress, 2);
      case 'Sigmoid': return 1.0 / (1.0 + Math.exp(-10.0 * (progress - 0.5)));
    }
  }

  public static isInsideRegion(region: SpatialRegion, x: number, y: number): boolean {
    if (region.type === 'Circle') {
      const r = region.radius ?? 10.0;
      const dx = x - region.x;
      const dy = y - region.y;
      return dx * dx + dy * dy <= r * r;
    } else {
      const w = region.width ?? 20.0;
      const h = region.height ?? 20.0;
      return x >= region.x - w / 2 && x <= region.x + w / 2 && y >= region.y - h / 2 && y <= region.y + h / 2;
    }
  }

  public static applyIntervention(
    command: InterventionCommand,
    currentTick: number,
    agents: TrophicAgent[],
    resourceField: { x: number; y: number; biomass: number }[],
  ): { affectedAgents: number; affectedBiomass: number } {
    if (currentTick < command.startTick || currentTick > command.startTick + command.durationTicks) {
      return { affectedAgents: 0, affectedBiomass: 0 };
    }
    const elapsed = currentTick - command.startTick;
    const intensity = this.evaluateCurve(command.curve, elapsed, command.durationTicks) * command.magnitude;

    let affectedAgents = 0;
    for (const agent of agents) {
      if (agent.alive && this.isInsideRegion(command.targetRegion, agent.x, agent.y)) {
        if (command.kind === 'GeneticInjection') {
          agent.mass *= (1.0 + 0.1 * intensity);
          affectedAgents++;
        } else if (command.kind === 'PathogenTransmission') {
          agent.energy *= (1.0 - 0.05 * intensity);
          affectedAgents++;
        } else if (command.kind === 'ToxicityPlume') {
          agent.energy = Math.max(0, agent.energy - 2.0 * intensity);
          affectedAgents++;
        }
      }
    }

    let affectedBiomass = 0;
    for (const cell of resourceField) {
      if (this.isInsideRegion(command.targetRegion, cell.x, cell.y)) {
        if (command.kind === 'ResourceSpike') {
          cell.biomass += 10.0 * intensity;
          affectedBiomass++;
        }
      }
    }

    return { affectedAgents, affectedBiomass };
  }
}

// ===============================================================================================
// 7. DETERMINISTIC REPLAY & STATE PARITY ENGINE
// ===============================================================================================

export class DeterministicReplayEngine {
  private state: number;

  constructor(seed: number) {
    this.state = seed;
  }

  /** Simple LCG/ChaCha-like deterministic generator */
  public nextFloat(): number {
    this.state = (this.state * 1664525 + 1013904223) >>> 0;
    return this.state / 4294967296.0;
  }

  public getStreamPosition(): number {
    return this.state;
  }

  public setStreamPosition(pos: number): void {
    this.state = pos >>> 0;
  }

  public static computeStateDigest(entities: { id: number; energy: number; x: number; y: number }[]): string {
    let hash = 2166136261; // FNV-1a 32-bit
    for (const e of entities) {
      const str = `${e.id}:${e.energy.toFixed(4)}:${e.x.toFixed(4)}:${e.y.toFixed(4)};`;
      for (let i = 0; i < str.length; i++) {
        hash ^= str.charCodeAt(i);
        hash = Math.imul(hash, 16777619);
      }
    }
    return 'DIGEST_' + (hash >>> 0).toString(16).padStart(8, '0');
  }
}

// ===============================================================================================
// 8. DEMOGRAPHIC METRICS PIPELINE
// ===============================================================================================

export interface DemographicCensusState {
  ageCohorts: { minAge: number; maxAge: number; count: number }[];
  mortalityCauses: { cause: string; count: number }[];
  fecundityByAge: { ageGroup: string; births: number }[];
  shannon: number;
  simpson: number;
}

export class DemographicsPipeline {
  public static generateCensus(agents: TrophicAgent[], mortalityEvents: { cause: string }[]): DemographicCensusState {
    const cohorts = [
      { minAge: 0, maxAge: 50, count: 0 },
      { minAge: 51, maxAge: 150, count: 0 },
      { minAge: 151, maxAge: 300, count: 0 },
      { minAge: 301, maxAge: 1000, count: 0 },
    ];

    for (const agent of agents.filter(a => a.alive)) {
      for (const c of cohorts) {
        if (agent.age >= c.minAge && agent.age <= c.maxAge) {
          c.count++;
          break;
        }
      }
    }

    const causeMap = new Map<string, number>();
    for (const e of mortalityEvents) {
      causeMap.set(e.cause, (causeMap.get(e.cause) ?? 0) + 1);
    }
    const mortalityCauses = Array.from(causeMap.entries()).map(([cause, count]) => ({ cause, count }));

    const preyCount = agents.filter(a => a.alive && a.type === 'prey').length;
    const predCount = agents.filter(a => a.alive && a.type === 'predator').length;

    return {
      ageCohorts: cohorts,
      mortalityCauses: mortalityCauses.length > 0 ? mortalityCauses : [{ cause: 'Starvation', count: 0 }],
      fecundityByAge: [
        { ageGroup: 'Young (0-50)', births: 12 },
        { ageGroup: 'Mature (51-150)', births: 28 },
        { ageGroup: 'Old (151+)', births: 4 },
      ],
      shannon: calculateShannonIndex([preyCount, predCount]),
      simpson: calculateSimpsonIndex([preyCount, predCount]),
    };
  }
}

// ===============================================================================================
// 9. ENERGY FLUX MATRIX & SANKEY PIPELINE
// ===============================================================================================

export interface EnergyFluxReport {
  nodes: { id: string; label: string; biomass: number }[];
  links: { source: string; target: string; value: number }[];
  respirationLoss: number;
}

export class EnergyFluxPipeline {
  public static generateFluxReport(ledger: SimulatedEnergyLedger): EnergyFluxReport {
    const nodes = [
      { id: 'sun', label: 'Solar Input', biomass: 10000 },
      { id: 'plants', label: 'Primary Producers', biomass: ledger.plants },
      { id: 'herbivores', label: 'Primary Consumers', biomass: ledger.animals * 0.7 },
      { id: 'carnivores', label: 'Secondary Consumers', biomass: ledger.animals * 0.3 },
      { id: 'detritus', label: 'Detritus & Recycled', biomass: ledger.detritus },
      { id: 'metabolic_sink', label: 'Dissipated Heat', biomass: 0 },
    ];

    const links = [
      { source: 'sun', target: 'plants', value: 350.0 },
      { source: 'plants', target: 'herbivores', value: 120.0 },
      { source: 'herbivores', target: 'carnivores', value: 25.0 },
      { source: 'plants', target: 'detritus', value: 80.0 },
      { source: 'herbivores', target: 'detritus', value: 40.0 },
      { source: 'carnivores', target: 'detritus', value: 15.0 },
      { source: 'detritus', target: 'plants', value: 100.0 },
      { source: 'herbivores', target: 'metabolic_sink', value: 30.0 },
      { source: 'carnivores', target: 'metabolic_sink', value: 10.0 },
    ];

    return {
      nodes,
      links,
      respirationLoss: 40.0,
    };
  }
}

// ===============================================================================================
// 10. SPATIAL DENSITY PIPELINE
// ===============================================================================================

export interface SpatialDensityReport {
  width: number;
  height: number;
  preyDensity: number[];
  predatorDensity: number[];
  vegetationDensity: number[];
  predationHotspots: number[];
}

export class SpatialDensityPipeline {
  public static generateReport(width: number, height: number, agents: TrophicAgent[]): SpatialDensityReport {
    const size = width * height;
    const prey = new Array(size).fill(0);
    const pred = new Array(size).fill(0);
    const veg = new Array(size).fill(0.5);
    const hotspots = new Array(size).fill(0);

    for (const a of agents.filter(a => a.alive)) {
      const gx = Math.min(width - 1, Math.max(0, Math.floor((a.x / 100) * width)));
      const gy = Math.min(height - 1, Math.max(0, Math.floor((a.y / 100) * height)));
      const idx = gy * width + gx;
      if (a.type === 'prey') prey[idx]++;
      else pred[idx]++;
    }

    return {
      width,
      height,
      preyDensity: prey,
      predatorDensity: pred,
      vegetationDensity: veg,
      predationHotspots: hotspots,
    };
  }
}

// ===============================================================================================
// 11. MAP-ELITES & NEWICK PHYLOGENY PIPELINE
// ===============================================================================================

export interface MapElitesSummary {
  totalFitness: number;
  archiveCoverage: number;
  occupiedCells: number;
  maxCells: number;
}

export class MapElitesNewickPipeline {
  public static computeSummary(grid: Record<string, { fitness: number }>, resolution: number): MapElitesSummary {
    const keys = Object.keys(grid);
    const occupiedCells = keys.length;
    const maxCells = resolution * resolution;
    let totalFitness = 0;
    for (const k of keys) {
      totalFitness += grid[k].fitness;
    }
    const archiveCoverage = maxCells > 0 ? (occupiedCells / maxCells) * 100.0 : 0.0;
    return {
      totalFitness,
      archiveCoverage,
      occupiedCells,
      maxCells,
    };
  }

  public static generateNewickTree(nodes: { id: string; parentId: string | null; branchLength: number }[]): string {
    const root = nodes.find(n => n.parentId === null);
    if (!root) return '();';

    function buildSubtree(currId: string): string {
      const children = nodes.filter(n => n.parentId === currId);
      const currNode = nodes.find(n => n.id === currId)!;
      if (children.length === 0) {
        return `${currNode.id}:${currNode.branchLength.toFixed(2)}`;
      }
      const childStrings = children.map(c => buildSubtree(c.id)).join(',');
      return `(${childStrings})${currNode.id}:${currNode.branchLength.toFixed(2)}`;
    }

    return buildSubtree(root.id) + ';';
  }
}

// ===============================================================================================
// 12. SCIENTIFIC DATASET EXPORTER & SNAPSHOT ENVELOPE V7
// ===============================================================================================

export interface RunProvenance {
  commitHash: string;
  seed: number;
  engineVersion: string;
  timestampUtc: string;
  architecture: string;
}

export interface ScientificExportResponse {
  provenance: RunProvenance;
  csvContent: string;
  jsonSummary: Record<string, unknown>;
  newickTree: string;
}

export class ScientificDatasetExporter {
  public static exportDataset(seed: number, metrics: { tick: number; plants: number; prey: number; predators: number }[]): ScientificExportResponse {
    const provenance: RunProvenance = {
      commitHash: 'e2e-commit-7f3b89',
      seed,
      engineVersion: '0.1.0',
      timestampUtc: new Date().toISOString(),
      architecture: 'x86_64-pc-windows-msvc',
    };

    let csv = 'tick,plants,prey,predators\n';
    for (const m of metrics) {
      csv += `${m.tick},${m.plants.toFixed(2)},${m.prey},${m.predators}\n`;
    }

    return {
      provenance,
      csvContent: csv,
      jsonSummary: {
        totalSamples: metrics.length,
        initialPlants: metrics[0]?.plants ?? 0,
        finalPlants: metrics[metrics.length - 1]?.plants ?? 0,
      },
      newickTree: '((Prey_A:0.5,Prey_B:0.8):1.2,Predator_A:2.0)Root:0.0;',
    };
  }
}

export interface SnapshotEnvelopeV7 {
  schemaVersion: number;
  magicHeader: string;
  worldChecksum: string;
  payload: Record<string, unknown>;
}

export class SnapshotEnvelopeManager {
  public static seal(state: Record<string, unknown>): SnapshotEnvelopeV7 {
    const raw = JSON.stringify(state);
    let checksum = 0;
    for (let i = 0; i < raw.length; i++) {
      checksum = (checksum + raw.charCodeAt(i) * (i + 1)) % 1000000007;
    }
    return {
      schemaVersion: 7,
      magicHeader: 'ANIMA_V7_SNAP',
      worldChecksum: 'SHA256_' + checksum.toString(16).padStart(8, '0'),
      payload: state,
    };
  }

  public static verifyAndUnseal(envelope: SnapshotEnvelopeV7): Record<string, unknown> {
    if (envelope.schemaVersion !== 7 || envelope.magicHeader !== 'ANIMA_V7_SNAP') {
      throw new Error('Corrupted or unsupported snapshot envelope schema version');
    }
    return envelope.payload;
  }
}

// ===============================================================================================
// 13. ZERO-ALLOCATION HOT PATH SIMULATOR
// ===============================================================================================

export class ZeroAllocBufferSimulator {
  private bufferSize: number;
  private ringBuffer: Float64Array;
  private writePointer: number = 0;
  public totalAllocations: number = 0;

  constructor(capacity: number = 1024) {
    this.bufferSize = capacity;
    this.ringBuffer = new Float64Array(capacity);
    this.totalAllocations = 1; // 1 alloc on init
  }

  public stepHotPath(value: number): void {
    // Zero allocations during hot path execution
    this.ringBuffer[this.writePointer] = value;
    this.writePointer = (this.writePointer + 1) % this.bufferSize;
  }

  public getArray(): Float64Array {
    return this.ringBuffer;
  }
}

// ===============================================================================================
// 14. PLAYWRIGHT TAURI IPC MOCK & DRIVER HARNESS
// ===============================================================================================

export interface TierHarnessHandle {
  emit(name: string, payload: unknown): Promise<void>;
  invocations(): Promise<RecordedInvocation[]>;
  invokedCommands(): Promise<string[]>;
  getLedger(): SimulatedEnergyLedger;
}

export async function installTierHarness(
  page: Page,
  initialLedger: SimulatedEnergyLedger = new SimulatedEnergyLedger(2000, 1500, 500),
): Promise<TierHarnessHandle> {
  const replies: Record<string, unknown> = {
    get_simulation_status: {
      running: true,
      tick_count: 5432,
      avg_tick_time_ms: 2.1,
      fps: 60,
    },
    get_ecosystem_state: {
      detritus: initialLedger.detritus,
      plants: initialLedger.plants,
      animals: initialLedger.animals,
      total: initialLedger.total(),
      prey_count: 45,
      predator_count: 18,
      shannon: 0.88,
      simpson: 0.58,
      prey_mass: 1.1,
      predator_mass: 2.3,
      niche_divergence: 0.6,
      archive_coverage: 15,
    },
    get_demographic_census: DemographicsPipeline.generateCensus(
      [
        { id: 1, type: 'prey', energy: 50, mass: 1.0, age: 25, x: 10, y: 10, alive: true },
        { id: 2, type: 'predator', energy: 90, mass: 2.5, age: 40, x: 20, y: 20, alive: true },
      ],
      [{ cause: 'Starvation' }, { cause: 'Predation' }],
    ),
    get_energy_flux_matrix: EnergyFluxPipeline.generateFluxReport(initialLedger),
    get_spatial_density_matrix: SpatialDensityPipeline.generateReport(16, 16, [
      { id: 1, type: 'prey', energy: 50, mass: 1.0, age: 25, x: 10, y: 10, alive: true },
    ]),
    get_map_elites_grid: {
      grid: {
        '0,0': { fitness: 2.1, features: [0.1, 0.4] },
        '1,1': { fitness: 3.4, features: [0.5, 0.8] },
      },
      grid_resolution: 8,
    },
    get_lineage_graph: {
      nodes: [
        { id: '1', generation: 0, parent_id: null },
        { id: '2', generation: 1, parent_id: '1' },
      ],
      links: [{ source: '1', target: '2' }],
      db_connected: true,
    },
    get_causal_branch_tree: {
      root_run_id: 'root',
      nodes: {},
      edges: [],
    },
    dispatch_causal_intervention: true,
    export_scientific_dataset: ScientificDatasetExporter.exportDataset(42, [
      { tick: 0, plants: 2000, prey: 40, predators: 15 },
      { tick: 10, plants: 1950, prey: 42, predators: 16 },
    ]),
    export_phylogenetic_tree: '((Prey_A:0.5,Prey_B:0.8):1.2,Predator_A:2.0)Root:0.0;',
    execute_parameter_sweep: ParameterSweepEngine.executeSweep(
      [{ name: 'plant_growth', min: 0.01, max: 0.1, steps: 3 }],
      'Grid',
    ),
    get_active_raycasts: [],
    get_environmental_elements: { elements: [] },
    get_chronicle_history: [],
    get_pheromone_grid: { grid: [0, 0, 0, 0], width: 2, height: 2 },
    get_terrain_map: { width: 4, height: 4, cells: new Array(16).fill(0) },
    get_lod_bands: { bands: [] },
    set_lod_focus: null,
    toggle_simulation: true,
    toggle_evolution: true,
    update_evolution_settings: null,
    trigger_migration: null,
    save_simulation_state: null,
    load_simulation_state: null,
    save_world_artifact: null,
    list_legacy_saves: {
      directory: 'C:\\Users\\test\\AppData\\Roaming\\com.anima.engine\\legacy-import',
      names: ['old_world.json', 'second_world.json'],
      ignored: ['notes.txt'],
    },
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

    const internals: Window['__TAURI_INTERNALS__'] = {
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

    window.__TAURI_INTERNALS__ = internals;
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
  }, replies);

  return {
    async emit(name: string, payload: unknown): Promise<void> {
      const call: [string, unknown] = [name, payload];
      await page.evaluate(([n, p]) => window.__animaTauriMock.emit(n, p), call);
    },
    async invocations(): Promise<RecordedInvocation[]> {
      return page.evaluate(() => window.__animaTauriMock.invocations());
    },
    async invokedCommands(): Promise<string[]> {
      const calls = await page.evaluate(() => window.__animaTauriMock.invocations());
      return [...new Set(calls.map((c) => c.cmd))];
    },
    getLedger(): SimulatedEnergyLedger {
      return initialLedger;
    },
  };
}
