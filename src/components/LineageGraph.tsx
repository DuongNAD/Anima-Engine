import React from 'react';
import { LineageGraphPayload } from '../types';

interface LineageGraphProps {
  lineageGraph: LineageGraphPayload;
}

export const LineageGraph: React.FC<LineageGraphProps> = ({ lineageGraph }) => {
  const nodes = Array.isArray(lineageGraph?.nodes) ? lineageGraph.nodes : [];
  const svgHeight = Math.max(180, nodes.length * 30 + 30);

  return (
    <div
      className="hud-panel"
      style={{
        padding: '18px',
        borderRadius: '15px',
      }}
    >
      <h3
        className="hud-title-teal"
        style={{
          margin: '0 0 15px 0',
          fontSize: '18px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.15)',
          paddingBottom: '8px',
        }}
      >
        Genotype Lineage Graph
      </h3>
      <div
        data-testid="lineage-svg-container"
        style={{
          width: '100%',
          height: '200px',
          border: '1px solid rgba(255, 255, 255, 0.1)',
          backgroundColor: 'rgba(255, 255, 255, 0.02)',
          display: 'block',
          overflow: 'auto',
          borderRadius: '8px',
        }}
      >
        {nodes.length === 0 ? (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              height: '100%',
              color: '#a1a1aa',
              fontFamily: "'Inter', sans-serif",
            }}
          >
            No lineage data available
          </div>
        ) : (
          <svg width="100%" height={svgHeight} style={{ minWidth: '180px' }}>
            {(() => {
              const nodesFiltered = nodes.filter(Boolean);
              const nodesMap = new Map(nodesFiltered.map((n) => [n?.id, n]));
              const nodeIndexMap = new Map(nodesFiltered.map((n, i) => [n?.id, i]));
              const links = Array.isArray(lineageGraph?.links) ? lineageGraph.links : [];

              return links
                .filter(Boolean)
                .map((link, idx) => {
                  const sourceNode = nodesMap.get(link?.source);
                  const targetNode = nodesMap.get(link?.target);
                  if (!sourceNode || !targetNode) return null;
                  const sourceIdx = nodeIndexMap.get(link?.source);
                  const targetIdx = nodeIndexMap.get(link?.target);
                  if (sourceIdx === undefined || targetIdx === undefined) return null;
                  const x1 = 30 + (sourceNode.generation || 0) * 40;
                  const y1 = 30 + sourceIdx * 30;
                  const x2 = 30 + (targetNode.generation || 0) * 40;
                  const y2 = 30 + targetIdx * 30;
                  return (
                    <line key={idx} x1={x1} y1={y1} x2={x2} y2={y2} stroke="rgba(255, 255, 255, 0.2)" strokeWidth="2" />
                  );
                });
            })()}
            {nodes
              .filter(Boolean)
              .map((node, idx) => {
                if (!node) return null;
                const cx = 30 + (node.generation || 0) * 40;
                const cy = 30 + idx * 30;
                return (
                  <g key={node.id} data-testid={`lineage-node-${node.id}`}>
                    <circle cx={cx} cy={cy} r="8" fill="#18181b" stroke="rgba(255, 255, 255, 0.8)" strokeWidth="2" />
                    <text x={cx} y={cy - 12} fontSize="10" textAnchor="middle" fill="#cbd5e1" fontFamily="'Inter', sans-serif" fontWeight="bold">
                      {node.id}
                    </text>
                  </g>
                );
              })}
          </svg>
        )}
      </div>
    </div>
  );
};
export default LineageGraph;
