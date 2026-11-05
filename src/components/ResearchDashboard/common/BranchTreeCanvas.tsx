import React from 'react';
import type { CausalBranchTreeState } from '../types';

export interface BranchTreeCanvasProps {
  tree: CausalBranchTreeState | null;
  selectedBranchId?: number | null;
  onSelectBranch?: (id: number) => void;
  onForkBranch?: (parentId: number) => void;
  width?: number;
  height?: number;
  testid?: string;
}

export const BranchTreeCanvas: React.FC<BranchTreeCanvasProps> = ({
  tree,
  selectedBranchId,
  onSelectBranch,
  onForkBranch,
  width = 540,
  height = 220,
  testid = 'branch-tree-canvas-svg',
}) => {
  if (!tree || !tree.nodes || tree.nodes.length === 0) {
    return (
      <div
        data-testid={testid}
        style={{
          width: '100%',
          height,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: 'rgba(15, 23, 42, 0.4)',
          borderRadius: '8px',
          border: '1px dashed rgba(255, 255, 255, 0.1)',
          color: '#94a3b8',
          fontSize: '13px',
        }}
      >
        No causal branch tree loaded. Fork a counterfactual branch to begin divergence analysis.
      </div>
    );
  }

  const W = width;
  const H = height;

  // Compute node coordinates
  // Root on left, children branched to right
  const rootNode = tree.nodes.find((n) => n.parentId === null) || tree.nodes[0];
  const children = tree.nodes.filter((n) => n.parentId !== null);

  const rootX = 60;
  const rootY = H / 2;
  const childX = W - 140;

  const nodePos = new Map<number, { x: number; y: number }>();
  nodePos.set(rootNode.id, { x: rootX, y: rootY });

  children.forEach((c, idx) => {
    const totalChildren = children.length;
    const spacing = totalChildren > 1 ? (H - 80) / (totalChildren - 1) : 0;
    const y = totalChildren > 1 ? 40 + idx * spacing : H / 2;
    nodePos.set(c.id, { x: childX, y });
  });

  return (
    <div
      style={{
        backgroundColor: 'rgba(15, 23, 42, 0.55)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        borderRadius: '8px',
        padding: '12px',
        position: 'relative',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '8px' }}>
        <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
          N-ary Causal Branch DAG & Fork Hierarchy
        </div>
        <div style={{ fontSize: '11px', color: '#94a3b8' }}>
          Root Run ID: <span style={{ color: '#38bdf8', fontWeight: 600 }}>#{tree.rootRunId}</span>
        </div>
      </div>

      <svg
        data-testid={testid}
        viewBox={`0 0 ${W} ${H}`}
        style={{ width: '100%', height, display: 'block', overflow: 'visible' }}
        role="img"
      >
        {/* Edges */}
        {children.map((c) => {
          const parent = nodePos.get(c.parentId || rootNode.id) || { x: rootX, y: rootY };
          const pos = nodePos.get(c.id);
          if (!pos) return null;

          const cpx1 = parent.x + (pos.x - parent.x) * 0.5;
          const cpy1 = parent.y;
          const cpx2 = parent.x + (pos.x - parent.x) * 0.5;
          const cpy2 = pos.y;

          return (
            <g key={c.id} data-testid={`branch-edge-${c.id}`}>
              <path
                d={`M ${parent.x} ${parent.y} C ${cpx1} ${cpy1}, ${cpx2} ${cpy2}, ${pos.x} ${pos.y}`}
                fill="none"
                stroke="rgba(56, 189, 248, 0.4)"
                strokeWidth={2}
                strokeDasharray={c.status === 'Completed' ? undefined : '4 4'}
              />
              {/* Fork Tick Tag */}
              <rect
                x={cpx1 - 24}
                y={(parent.y + pos.y) / 2 - 10}
                width={48}
                height={18}
                rx={3}
                fill="rgba(15, 23, 42, 0.9)"
                stroke="rgba(255, 255, 255, 0.15)"
                strokeWidth={1}
              />
              <text
                x={cpx1}
                y={(parent.y + pos.y) / 2 + 3}
                fontSize="9"
                fill="#94a3b8"
                textAnchor="middle"
                fontFamily="Inter, sans-serif"
              >
                t = {c.forkTick}
              </text>
            </g>
          );
        })}

        {/* Nodes */}
        {tree.nodes.map((n) => {
          const pos = nodePos.get(n.id) || { x: rootX, y: rootY };
          const isSelected = selectedBranchId === n.id;
          const isRoot = n.parentId === null;

          return (
            <g
              key={n.id}
              data-testid={`branch-node-${n.id}`}
              transform={`translate(${pos.x}, ${pos.y})`}
              onClick={() => onSelectBranch && onSelectBranch(n.id)}
              style={{ cursor: 'pointer' }}
            >
              {/* Node Outer Card */}
              <rect
                x={-50}
                y={-22}
                width={100}
                height={44}
                rx={6}
                fill={isRoot ? 'rgba(16, 185, 129, 0.2)' : isSelected ? 'rgba(56, 189, 248, 0.3)' : 'rgba(30, 41, 59, 0.8)'}
                stroke={isSelected ? '#38bdf8' : isRoot ? '#10b981' : 'rgba(255, 255, 255, 0.15)'}
                strokeWidth={isSelected ? 2 : 1}
                style={{ filter: isSelected ? 'drop-shadow(0 0 8px #38bdf8)' : 'none' }}
              />

              {/* Label */}
              <text
                y={-4}
                fontSize="10"
                fontWeight="600"
                fill="#f8fafc"
                textAnchor="middle"
                fontFamily="Inter, sans-serif"
              >
                {n.label.length > 14 ? `${n.label.slice(0, 13)}…` : n.label}
              </text>

              {/* Seed strategy & ID */}
              <text
                y={12}
                fontSize="9"
                fill="#94a3b8"
                textAnchor="middle"
                fontFamily="Inter, sans-serif"
              >
                #{n.id} • {n.seedStrategy.split(' ')[0]}
              </text>

              {/* Fork button icon */}
              {onForkBranch && (
                <circle
                  cx={42}
                  cy={0}
                  r={7}
                  fill="#0284c7"
                  stroke="#ffffff"
                  strokeWidth={1}
                  onClick={(e) => {
                    e.stopPropagation();
                    onForkBranch(n.id);
                  }}
                  data-testid={`fork-btn-${n.id}`}
                >
                  <title>Fork new branch from here</title>
                </circle>
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
};
