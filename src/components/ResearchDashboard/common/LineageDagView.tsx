import React, { useState } from 'react';
import type { LineageGraphPayload } from '../../../types/generated/LineageGraphPayload';
import type { LineageNodePayload } from '../../../types/generated/LineageNodePayload';

export interface LineageDagViewProps {
  lineage: LineageGraphPayload | null;
  selectedNodeId?: string | null;
  onSelectNode?: (node: LineageNodePayload) => void;
  testid?: string;
}

export const LineageDagView: React.FC<LineageDagViewProps> = ({
  lineage,
  selectedNodeId,
  onSelectNode,
  testid = 'lineage-dag-view-svg',
}) => {
  const [zoom, setZoom] = useState<number>(1.0);
  const [pan, setPan] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState<boolean>(false);
  const [dragStart, setDragStart] = useState<{ x: number; y: number }>({ x: 0, y: 0 });

  const nodes = (Array.isArray(lineage?.nodes) ? lineage.nodes : []).filter(
    (n): n is LineageNodePayload => Boolean(n && n.id)
  );
  const links = (Array.isArray(lineage?.links) ? lineage.links : []).filter(
    (l) => Boolean(l && l.source && l.target)
  );

  if (nodes.length === 0) {
    return (
      <div
        data-testid={testid}
        style={{
          width: '100%',
          height: '240px',
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
        No phylogenetic lineage data available yet. Start the simulation to observe speciation.
      </div>
    );
  }

  // Calculate coordinates by generation and index
  const genMap = new Map<number, LineageNodePayload[]>();
  nodes.forEach((n) => {
    const gen = n.generation || 0;
    if (!genMap.has(gen)) genMap.set(gen, []);
    genMap.get(gen)!.push(n);
  });

  const nodePos = new Map<string, { x: number; y: number }>();
  const genWidth = 110;
  const nodeHeight = 55;

  Array.from(genMap.entries()).forEach(([gen, genNodes]) => {
    const startY = 40;
    genNodes.forEach((node, idx) => {
      nodePos.set(node.id, {
        x: 40 + gen * genWidth,
        y: startY + idx * nodeHeight,
      });
    });
  });

  const maxGen = Math.max(...nodes.map((n) => n.generation || 0), 0);
  const maxPerGen = Math.max(...Array.from(genMap.values()).map((list) => list.length), 1);
  const svgWidth = Math.max(500, (maxGen + 1) * genWidth + 100);
  const svgHeight = Math.max(260, maxPerGen * nodeHeight + 80);

  const handleMouseDown = (e: React.MouseEvent) => {
    setIsDragging(true);
    setDragStart({ x: e.clientX - pan.x, y: e.clientY - pan.y });
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (isDragging) {
      setPan({ x: e.clientX - dragStart.x, y: e.clientY - dragStart.y });
    }
  };

  const handleMouseUp = () => {
    setIsDragging(false);
  };

  return (
    <div
      style={{
        position: 'relative',
        width: '100%',
        height: '280px',
        overflow: 'hidden',
        backgroundColor: 'rgba(9, 9, 11, 0.7)',
        borderRadius: '8px',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        cursor: isDragging ? 'grabbing' : 'grab',
      }}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
    >
      {/* Zoom / Pan Controls */}
      <div
        style={{
          position: 'absolute',
          top: '8px',
          right: '8px',
          display: 'flex',
          gap: '4px',
          zIndex: 10,
        }}
      >
        <button
          data-testid="lineage-zoom-in"
          onClick={(e) => {
            e.stopPropagation();
            setZoom((z) => Math.min(2.5, z + 0.2));
          }}
          style={{
            padding: '2px 8px',
            backgroundColor: 'rgba(255, 255, 255, 0.1)',
            border: '1px solid rgba(255, 255, 255, 0.15)',
            borderRadius: '4px',
            color: '#fff',
            cursor: 'pointer',
            fontSize: '12px',
          }}
        >
          +
        </button>
        <button
          data-testid="lineage-zoom-out"
          onClick={(e) => {
            e.stopPropagation();
            setZoom((z) => Math.max(0.4, z - 0.2));
          }}
          style={{
            padding: '2px 8px',
            backgroundColor: 'rgba(255, 255, 255, 0.1)',
            border: '1px solid rgba(255, 255, 255, 0.15)',
            borderRadius: '4px',
            color: '#fff',
            cursor: 'pointer',
            fontSize: '12px',
          }}
        >
          -
        </button>
        <button
          data-testid="lineage-reset-pan"
          onClick={(e) => {
            e.stopPropagation();
            setZoom(1.0);
            setPan({ x: 0, y: 0 });
          }}
          style={{
            padding: '2px 8px',
            backgroundColor: 'rgba(255, 255, 255, 0.1)',
            border: '1px solid rgba(255, 255, 255, 0.15)',
            borderRadius: '4px',
            color: '#fff',
            cursor: 'pointer',
            fontSize: '12px',
          }}
        >
          Reset
        </button>
      </div>

      <svg
        data-testid={testid}
        viewBox={`0 0 ${svgWidth} ${svgHeight}`}
        width="100%"
        height="100%"
        style={{ overflow: 'visible' }}
        role="img"
      >
        <g transform={`translate(${pan.x}, ${pan.y}) scale(${zoom})`}>
          {/* Generation Grid Background */}
          {Array.from({ length: maxGen + 1 }).map((_, g) => (
            <g key={g}>
              <line
                x1={40 + g * genWidth}
                y1={0}
                x2={40 + g * genWidth}
                y2={svgHeight}
                stroke="rgba(255, 255, 255, 0.04)"
                strokeDasharray="4 4"
              />
              <text
                x={40 + g * genWidth}
                y={18}
                fontSize="10"
                fill="#64748b"
                textAnchor="middle"
                fontFamily="Inter, sans-serif"
              >
                Gen {g}
              </text>
            </g>
          ))}

          {/* Links / Bezier edges */}
          {links.map((link, idx) => {
            const p1 = nodePos.get(link.source);
            const p2 = nodePos.get(link.target);
            if (!p1 || !p2) return null;

            const cpx1 = p1.x + (p2.x - p1.x) * 0.5;
            const cpy1 = p1.y;
            const cpx2 = p1.x + (p2.x - p1.x) * 0.5;
            const cpy2 = p2.y;

            return (
              <path
                key={idx}
                d={`M ${p1.x} ${p1.y} C ${cpx1} ${cpy1}, ${cpx2} ${cpy2}, ${p2.x} ${p2.y}`}
                fill="none"
                stroke="rgba(148, 163, 184, 0.4)"
                strokeWidth={1.8}
                strokeLinecap="round"
              />
            );
          })}

          {/* Nodes */}
          {nodes.map((node) => {
            const pos = nodePos.get(node.id);
            if (!pos) return null;
            const isSelected = selectedNodeId === node.id;
            const fitness = typeof node.fitness === 'number' ? node.fitness : 0.5;
            const fillColor = isSelected ? '#38bdf8' : fitness > 0.7 ? '#10b981' : fitness > 0.4 ? '#3b82f6' : '#8b5cf6';

            return (
              <g
                key={node.id}
                data-testid={`lineage-node-${node.id}`}
                transform={`translate(${pos.x}, ${pos.y})`}
                onClick={(e) => {
                  e.stopPropagation();
                  if (onSelectNode) onSelectNode(node);
                }}
                style={{ cursor: 'pointer' }}
              >
                {/* Node Outer Circle */}
                <circle
                  r={isSelected ? 16 : 13}
                  fill={fillColor}
                  stroke={isSelected ? '#ffffff' : 'rgba(255, 255, 255, 0.25)'}
                  strokeWidth={isSelected ? 2.5 : 1.5}
                  style={{ filter: isSelected ? 'drop-shadow(0 0 8px #38bdf8)' : 'none' }}
                />

                {/* Node ID label */}
                <text
                  y={4}
                  fontSize="9"
                  fontWeight="600"
                  fill="#ffffff"
                  textAnchor="middle"
                  fontFamily="Inter, sans-serif"
                >
                  {node.id.length > 5 ? node.id.slice(0, 4) : node.id}
                </text>

                {/* Mutation Count Pill */}
                {node.mutations_count > 0 && (
                  <g transform="translate(10, -10)">
                    <circle r={6} fill="#f59e0b" />
                    <text
                      y={3}
                      fontSize="8"
                      fontWeight="bold"
                      fill="#000000"
                      textAnchor="middle"
                    >
                      {node.mutations_count}
                    </text>
                  </g>
                )}
              </g>
            );
          })}
        </g>
      </svg>
    </div>
  );
};
