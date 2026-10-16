import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { CausalBranchTreeState, CheckpointBookmark } from './types';
import { BranchTreeCanvas } from './common/BranchTreeCanvas';
import { SparkLine } from './common/SparkLine';

export interface CheckpointStudioPanelProps {
  currentTick?: number;
  initialBranchTree?: CausalBranchTreeState | null;
}

export const CheckpointStudioPanel: React.FC<CheckpointStudioPanelProps> = ({
  currentTick = 1200,
  initialBranchTree,
}) => {
  const [branchTree, setBranchTree] = useState<CausalBranchTreeState | null>(initialBranchTree || null);
  const [selectedBranchId, setSelectedBranchId] = useState<number | null>(1002);
  const [scrubberTick, setScrubberTick] = useState<number>(currentTick);
  const [bookmarks, setBookmarks] = useState<CheckpointBookmark[]>([
    { tick: 0, label: 'Genesis Baseline', timestamp: Date.now() - 360000, worldChecksum: '0xa4f28e1' },
    { tick: 500, label: 'Pre-Intervention Fork Point', timestamp: Date.now() - 180000, worldChecksum: '0xb7d19c3' },
    { tick: 1000, label: 'Coexistence Checkpoint', timestamp: Date.now() - 60000, worldChecksum: '0x992ef40' },
  ]);

  const [showForkModal, setShowForkModal] = useState<boolean>(false);
  const [forkParentId, setForkParentId] = useState<number>(1001);
  const [forkLabel, setForkLabel] = useState<string>('Counterfactual: Culling +20%');
  const [forkSeedStrategy, setForkSeedStrategy] = useState<string>('DerivedStream (3)');

  // Trajectory Delta data ($Y_{treatment}(t) - Y_{control}(t)$)
  const deltaPoints = 60;
  const biomassDeltaSeries = Array.from({ length: deltaPoints }).map((_, i) => {
    if (i < 20) return 0;
    return -Math.sin((i - 20) * 0.15) * 80 - (i - 20) * 1.5;
  });

  const populationDeltaSeries = Array.from({ length: deltaPoints }).map((_, i) => {
    if (i < 20) return 0;
    return -((i - 20) * 0.45) + Math.cos(i * 0.2) * 4;
  });

  const diversityDeltaSeries = Array.from({ length: deltaPoints }).map((_, i) => {
    if (i < 20) return 0;
    return Math.sin((i - 20) * 0.1) * 0.12;
  });

  useEffect(() => {
    let alive = true;
    const fetchBranchTree = async () => {
      try {
        const tree = await invoke<CausalBranchTreeState>('get_causal_branch_tree');
        if (alive && tree) setBranchTree(tree);
      } catch {
        // Fallback
      }
    };
    fetchBranchTree();
    return () => {
      alive = false;
    };
  }, []);

  const handleCreateBookmark = () => {
    const newBm: CheckpointBookmark = {
      tick: scrubberTick,
      label: `Snapshot @ tick ${scrubberTick}`,
      timestamp: Date.now(),
      worldChecksum: `0x${(Date.now() % 0xffffff).toString(16)}`,
    };
    setBookmarks((prev) => [...prev, newBm]);
  };

  const handleForkBranchSubmit = () => {
    if (!branchTree) return;
    const newId = (Date.now() % 9000) + 1000;
    const updatedNodes = [
      ...branchTree.nodes,
      {
        id: newId,
        parentId: forkParentId,
        forkTick: scrubberTick,
        label: forkLabel,
        seedStrategy: forkSeedStrategy,
        status: 'Completed' as const,
        children: [],
      },
    ];

    setBranchTree({
      ...branchTree,
      nodes: updatedNodes,
    });
    setSelectedBranchId(newId);
    setShowForkModal(false);
  };

  return (
    <div
      data-testid="checkpoint-studio-panel"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
        color: '#e2e8f0',
      }}
    >
      {/* Top Timeline Scrubber & Bookmark Bar */}
      <div
        data-testid="timeline-scrubber-card"
        style={{
          backgroundColor: 'rgba(15, 23, 42, 0.55)',
          border: '1px solid rgba(255, 255, 255, 0.08)',
          borderRadius: '8px',
          padding: '14px',
          display: 'flex',
          flexDirection: 'column',
          gap: '10px',
        }}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
            Historical Simulation Timeline Scrubber
          </div>
          <div style={{ fontSize: '12px', color: '#38bdf8', fontWeight: 600 }}>
            Inspecting Tick: t = {scrubberTick} / {Math.max(scrubberTick, currentTick)}
          </div>
        </div>

        {/* Scrubber slider */}
        <input
          type="range"
          min="0"
          max={Math.max(2000, currentTick)}
          step="1"
          value={scrubberTick}
          onChange={(e) => setScrubberTick(parseInt(e.target.value))}
          data-testid="timeline-scrubber-slider"
          style={{ width: '100%' }}
        />

        {/* Scrubber Actions */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '8px' }}>
          <div style={{ display: 'flex', gap: '6px' }}>
            {[-100, -10, -1, 1, 10, 100].map((delta) => (
              <button
                key={delta}
                onClick={() => setScrubberTick((t) => Math.max(0, t + delta))}
                style={{
                  padding: '3px 8px',
                  fontSize: '11px',
                  backgroundColor: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  borderRadius: '4px',
                  color: '#cbd5e1',
                  cursor: 'pointer',
                }}
              >
                {delta > 0 ? `+${delta}` : delta}
              </button>
            ))}
          </div>

          <div style={{ display: 'flex', gap: '8px' }}>
            <button
              data-testid="save-snapshot-bookmark-btn"
              onClick={handleCreateBookmark}
              style={{
                padding: '4px 12px',
                fontSize: '11px',
                backgroundColor: 'rgba(16, 185, 129, 0.2)',
                border: '1px solid #10b981',
                borderRadius: '4px',
                color: '#a7f3d0',
                fontWeight: 600,
                cursor: 'pointer',
              }}
            >
              📌 Bookmark Checkpoint
            </button>
            <button
              data-testid="fork-branch-open-modal-btn"
              onClick={() => setShowForkModal(true)}
              style={{
                padding: '4px 12px',
                fontSize: '11px',
                backgroundColor: '#0284c7',
                border: 'none',
                borderRadius: '4px',
                color: '#ffffff',
                fontWeight: 600,
                cursor: 'pointer',
              }}
            >
              🌿 Fork Counterfactual Branch
            </button>
          </div>
        </div>

        {/* Checkpoint Bookmarks List */}
        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', marginTop: '4px' }}>
          {bookmarks.map((bm, idx) => (
            <div
              key={idx}
              onClick={() => setScrubberTick(bm.tick)}
              data-testid={`checkpoint-bookmark-${bm.tick}`}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                padding: '3px 8px',
                borderRadius: '4px',
                backgroundColor: scrubberTick === bm.tick ? 'rgba(56, 189, 248, 0.25)' : 'rgba(255, 255, 255, 0.05)',
                border: `1px solid ${scrubberTick === bm.tick ? '#38bdf8' : 'rgba(255, 255, 255, 0.1)'}`,
                fontSize: '11px',
                color: scrubberTick === bm.tick ? '#f8fafc' : '#94a3b8',
                cursor: 'pointer',
              }}
            >
              <span>🚩 t = {bm.tick}</span>
              <span style={{ color: '#cbd5e1' }}>({bm.label})</span>
            </div>
          ))}
        </div>
      </div>

      {/* Main Grid: Causal Branch Tree + Counterfactual Trajectory Delta */}
      <div style={{ display: 'grid', gridTemplateColumns: '1.2fr 1fr', gap: '16px' }}>
        {/* Causal Branch Tree Visualizer */}
        <BranchTreeCanvas
          tree={branchTree}
          selectedBranchId={selectedBranchId}
          onSelectBranch={(id) => setSelectedBranchId(id)}
          onForkBranch={(parentId) => {
            setForkParentId(parentId);
            setShowForkModal(true);
          }}
          width={420}
          height={260}
          testid="causal-branch-tree-svg"
        />

        {/* Counterfactual Trajectory Delta Comparator */}
        <div
          data-testid="counterfactual-delta-comparator"
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
            display: 'flex',
            flexDirection: 'column',
            gap: '10px',
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
              Counterfactual Delta: Y_treat(t) − Y_ctrl(t)
            </div>
            <span style={{ fontSize: '10px', color: '#94a3b8' }}>DTW Distance: 24.8</span>
          </div>

          {/* Divergence Metrics KPIs */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px' }}>
            <div style={{ padding: '6px 8px', borderRadius: '4px', backgroundColor: 'rgba(255, 255, 255, 0.03)', border: '1px solid rgba(255, 255, 255, 0.06)' }}>
              <div style={{ fontSize: '10px', color: '#94a3b8' }}>Dynamic Time Warping (DTW)</div>
              <div style={{ fontSize: '13px', fontWeight: 600, color: '#38bdf8' }}>
                {branchTree?.divergenceMetrics?.dtwDistance ?? 24.8}
              </div>
            </div>
            <div style={{ padding: '6px 8px', borderRadius: '4px', backgroundColor: 'rgba(255, 255, 255, 0.03)', border: '1px solid rgba(255, 255, 255, 0.06)' }}>
              <div style={{ fontSize: '10px', color: '#94a3b8' }}>RMSD Divergence</div>
              <div style={{ fontSize: '13px', fontWeight: 600, color: '#f59e0b' }}>
                {branchTree?.divergenceMetrics?.rmsd ?? 12.3}
              </div>
            </div>
          </div>

          {/* Sparkline Delta Plots */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            <div>
              <div style={{ fontSize: '11px', color: '#cbd5e1', marginBottom: '2px' }}>
                Biomass Delta ΔB(t): <span style={{ color: '#ef4444', fontWeight: 600 }}>-145.2 EU</span>
              </div>
              <SparkLine
                series={[{ label: 'Biomass Delta', color: '#ef4444', values: biomassDeltaSeries }]}
                height={32}
                testid="biomass-delta-sparkline"
              />
            </div>

            <div>
              <div style={{ fontSize: '11px', color: '#cbd5e1', marginBottom: '2px' }}>
                Population Delta ΔN(t): <span style={{ color: '#3b82f6', fontWeight: 600 }}>-18 agents</span>
              </div>
              <SparkLine
                series={[{ label: 'Population Delta', color: '#3b82f6', values: populationDeltaSeries }]}
                height={32}
                testid="population-delta-sparkline"
              />
            </div>

            <div>
              <div style={{ fontSize: '11px', color: '#cbd5e1', marginBottom: '2px' }}>
                Diversity Delta ΔH&apos;(t): <span style={{ color: '#10b981', fontWeight: 600 }}>+0.14 index</span>
              </div>
              <SparkLine
                series={[{ label: 'Diversity Delta', color: '#10b981', values: diversityDeltaSeries }]}
                height={32}
                testid="diversity-delta-sparkline"
              />
            </div>
          </div>
        </div>
      </div>

      {/* Fork New Branch Modal */}
      {showForkModal && (
        <div
          data-testid="fork-branch-modal"
          style={{
            position: 'fixed',
            inset: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.75)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
        >
          <div
            style={{
              backgroundColor: '#09090b',
              border: '1px solid rgba(255, 255, 255, 0.15)',
              borderRadius: '8px',
              padding: '18px',
              width: '380px',
              display: 'flex',
              flexDirection: 'column',
              gap: '12px',
              boxShadow: '0 8px 32px rgba(0,0,0,0.6)',
            }}
          >
            <div style={{ fontSize: '14px', fontWeight: 600, color: '#f8fafc' }}>
              Fork New Counterfactual Branch
            </div>

            <div style={{ fontSize: '11px', color: '#94a3b8' }}>
              Forking from Parent Branch #{forkParentId} at tick t = {scrubberTick}.
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '11px' }}>
              <label style={{ color: '#cbd5e1' }}>Branch Hypothesis Label:</label>
              <input
                type="text"
                value={forkLabel}
                onChange={(e) => setForkLabel(e.target.value)}
                data-testid="fork-label-input"
                style={{
                  backgroundColor: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.15)',
                  borderRadius: '4px',
                  padding: '6px',
                  color: '#ffffff',
                  fontSize: '12px',
                }}
              />
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '11px' }}>
              <label style={{ color: '#cbd5e1' }}>RNG Seed Strategy:</label>
              <select
                value={forkSeedStrategy}
                onChange={(e) => setForkSeedStrategy(e.target.value)}
                data-testid="fork-seed-strategy-select"
                style={{
                  backgroundColor: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.15)',
                  borderRadius: '4px',
                  padding: '6px',
                  color: '#ffffff',
                  fontSize: '12px',
                }}
              >
                <option value="InheritStream">InheritStream (Exact baseline RNG stream)</option>
                <option value="DerivedStream (3)">DerivedStream (Branch ID derived stream)</option>
                <option value="ExplicitSeed (999)">ExplicitSeed (Fixed arbitrary seed)</option>
              </select>
            </div>

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '8px' }}>
              <button
                onClick={() => setShowForkModal(false)}
                style={{
                  padding: '6px 12px',
                  backgroundColor: 'transparent',
                  border: '1px solid rgba(255, 255, 255, 0.15)',
                  color: '#cbd5e1',
                  borderRadius: '4px',
                  cursor: 'pointer',
                  fontSize: '12px',
                }}
              >
                Cancel
              </button>
              <button
                data-testid="confirm-fork-branch-btn"
                onClick={handleForkBranchSubmit}
                style={{
                  padding: '6px 14px',
                  backgroundColor: '#0284c7',
                  border: 'none',
                  color: '#ffffff',
                  borderRadius: '4px',
                  fontWeight: 600,
                  cursor: 'pointer',
                  fontSize: '12px',
                }}
              >
                Fork Branch
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
