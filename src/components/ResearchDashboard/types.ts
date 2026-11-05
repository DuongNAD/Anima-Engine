export interface CausalBranchNode {
  id: number;
  parentId: number | null;
  forkTick: number;
  label: string;
  seedStrategy: string;
  status: 'Pending' | 'Completed' | 'Failed';
  children: number[];
}

export interface CausalBranchTreeState {
  rootRunId: number;
  nodes: CausalBranchNode[];
  divergenceMetrics: {
    branchA: number;
    branchB: number;
    biomassDelta: number;
    populationDelta: number;
    shannonDivergence: number;
    dtwDistance: number;
    rmsd: number;
  };
}

export interface CheckpointBookmark {
  tick: number;
  label: string;
  timestamp: number;
  worldChecksum: string;
}
