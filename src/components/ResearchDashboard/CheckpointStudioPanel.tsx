import React from 'react';

/**
 * Checkpoint branching is not available in the desktop app.
 *
 * The backend has no `get_causal_branch_tree` command, so there is no branch tree, bookmark or
 * counterfactual delta for this panel to show. It used to draw one anyway: three bookmarks with
 * invented world checksums, a "Fork" button that added a `Completed` branch to local state without
 * running anything, and delta curves made of `Math.sin`. Headless runs can fork from a checkpoint
 * through `anima-lab` and an experiment manifest; this panel says so and calls nothing.
 */
export const CheckpointStudioPanel: React.FC = () => (
  <div
    data-testid="checkpoint-studio-panel"
    style={{
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
      color: '#e2e8f0',
      backgroundColor: 'rgba(15, 23, 42, 0.55)',
      border: '1px solid rgba(255, 255, 255, 0.08)',
      borderRadius: '8px',
      padding: '14px',
    }}
  >
    <div data-testid="checkpoint-studio-unavailable" style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
      Checkpoint Studio is not available in this build
    </div>
    <div style={{ fontSize: '12px', color: '#94a3b8', lineHeight: 1.5 }}>
      The desktop backend has no command that lists causal branches or forks a run from a checkpoint, so
      nothing here can be inspected or forked. Headless runs fork from a checkpoint with <code>anima-lab</code> and
      an experiment manifest.
    </div>
  </div>
);
