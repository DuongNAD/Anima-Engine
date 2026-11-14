import React, { useState } from 'react';
import type { EnergyFluxReport } from '../../../types/generated/EnergyFluxReport';
import type { EnergyFluxNode } from '../../../types/generated/EnergyFluxNode';
import type { EnergyFluxLink } from '../../../types/generated/EnergyFluxLink';

export interface SankeyDiagramProps {
  report: EnergyFluxReport | null;
  width?: number;
  height?: number;
  testid?: string;
}

export const SankeyDiagram: React.FC<SankeyDiagramProps> = ({
  report,
  width = 620,
  height = 340,
  testid = 'sankey-diagram-svg',
}) => {
  const [hoveredLink, setHoveredLink] = useState<EnergyFluxLink | null>(null);
  const [hoveredNode, setHoveredNode] = useState<EnergyFluxNode | null>(null);

  if (!report || !report.nodes || report.nodes.length === 0) {
    return (
      <div
        data-testid={testid}
        style={{
          width: '100%',
          height,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: 'rgba(15, 23, 42, 0.55)',
          border: '1px solid rgba(255, 255, 255, 0.08)',
          borderRadius: '8px',
          color: '#94a3b8',
          fontSize: '13px',
        }}
      >
        No Energy Flux data available. Run the simulation to observe energy dissipation.
      </div>
    );
  }

  const W = width;
  const H = height;
  const padX = 50;
  const padY = 30;
  const nodeWidth = 24;

  // Layout columns for trophic levels:
  // Col 0: Sun (Photosynthesis) & Detritus Substrate
  // Col 1: Plants (Primary Producers)
  // Col 2: Herbivores (Primary Consumers)
  // Col 3: Carnivores (Secondary Consumers)
  // Col 4: Metabolic Sink (Heat Dissipation)
  const nodePositions: Record<string, { x: number; y: number; height: number; color: string; label: string }> = {
    sun: { x: padX, y: padY + 20, height: 60, color: '#f59e0b', label: 'Solar Radiance' },
    plants: { x: padX + (W - 2 * padX) * 0.25, y: padY + 30, height: 110, color: '#10b981', label: 'Plants (NPP)' },
    herbivores: { x: padX + (W - 2 * padX) * 0.52, y: padY + 50, height: 80, color: '#3b82f6', label: 'Herbivores' },
    carnivores: { x: padX + (W - 2 * padX) * 0.78, y: padY + 60, height: 50, color: '#ef4444', label: 'Carnivores' },
    detritus: { x: padX + (W - 2 * padX) * 0.38, y: H - padY - 70, height: 55, color: '#a855f7', label: 'Detritus' },
    metabolic_sink: { x: W - padX - nodeWidth, y: padY + 90, height: 95, color: '#eab308', label: 'Metabolic Heat Sink' },
  };

  const totalFlux = report.links.reduce((sum, link) => sum + link.value, 0) || 1;

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        backgroundColor: 'rgba(15, 23, 42, 0.55)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        borderRadius: '8px',
        padding: '12px',
        position: 'relative',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
        <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
          Interactive Energy Flux Sankey Network
        </div>
        <div style={{ fontSize: '11px', color: '#94a3b8' }}>
          Respiration Loss: <span style={{ color: '#f59e0b', fontWeight: 600 }}>{report.respirationLoss.toFixed(1)} EU</span> | 
          System Energy: <span style={{ color: '#f1f5f9', fontWeight: 600 }}>{report.totalSystemEnergy.toFixed(1)} EU</span>
        </div>
      </div>

      <svg
        data-testid={testid}
        viewBox={`0 0 ${W} ${H}`}
        style={{ width: '100%', height, display: 'block', overflow: 'visible' }}
        role="img"
      >
        <defs>
          <style>{`
            @keyframes dashFlow {
              from { stroke-dashoffset: 24; }
              to { stroke-dashoffset: 0; }
            }
            .sankey-flow-ribbon {
              animation: dashFlow 1.2s linear infinite;
            }
          `}</style>
          {report.links.map((link, idx) => {
            const srcPos = nodePositions[link.source];
            const dstPos = nodePositions[link.target];
            const srcColor = srcPos?.color || '#94a3b8';
            const dstColor = dstPos?.color || '#94a3b8';
            return (
              <linearGradient
                key={idx}
                id={`grad-${link.source}-${link.target}`}
                x1="0%"
                y1="0%"
                x2="100%"
                y2="0%"
              >
                <stop offset="0%" stopColor={srcColor} stopOpacity={0.65} />
                <stop offset="100%" stopColor={dstColor} stopOpacity={0.65} />
              </linearGradient>
            );
          })}
        </defs>

        {/* Links / Ribbons */}
        {report.links.map((link, idx) => {
          const src = nodePositions[link.source];
          const dst = nodePositions[link.target];
          if (!src || !dst) return null;

          const strokeW = Math.max(2, Math.min(28, (link.value / totalFlux) * 120));
          const startX = src.x + nodeWidth;
          const startY = src.y + src.height / 2;
          const endX = dst.x;
          const endY = dst.y + dst.height / 2;

          const cpx1 = startX + (endX - startX) * 0.5;
          const cpy1 = startY;
          const cpx2 = startX + (endX - startX) * 0.5;
          const cpy2 = endY;

          const pathD = `M ${startX} ${startY} C ${cpx1} ${cpy1}, ${cpx2} ${cpy2}, ${endX} ${endY}`;
          const isHovered = hoveredLink === link;

          return (
            <g
              key={idx}
              data-testid={`sankey-link-${link.source}-${link.target}`}
              onMouseEnter={() => setHoveredLink(link)}
              onMouseLeave={() => setHoveredLink(null)}
              style={{ cursor: 'pointer' }}
            >
              <path
                d={pathD}
                fill="none"
                stroke={`url(#grad-${link.source}-${link.target})`}
                strokeWidth={isHovered ? strokeW + 3 : strokeW}
                strokeLinecap="round"
                strokeOpacity={isHovered ? 0.95 : 0.65}
              />
              <path
                className="sankey-flow-ribbon"
                d={pathD}
                fill="none"
                stroke="rgba(255, 255, 255, 0.4)"
                strokeWidth={Math.max(1, strokeW * 0.35)}
                strokeDasharray="4 8"
              />
            </g>
          );
        })}

        {/* Nodes */}
        {report.nodes.map((node) => {
          const pos = nodePositions[node.id];
          if (!pos) return null;
          const isHovered = hoveredNode === node;

          return (
            <g
              key={node.id}
              data-testid={`sankey-node-${node.id}`}
              onMouseEnter={() => setHoveredNode(node)}
              onMouseLeave={() => setHoveredNode(null)}
              style={{ cursor: 'pointer' }}
            >
              {/* Node Vertical Bar */}
              <rect
                x={pos.x}
                y={pos.y}
                width={nodeWidth}
                height={pos.height}
                rx={4}
                fill={pos.color}
                stroke={isHovered ? '#ffffff' : 'rgba(255,255,255,0.2)'}
                strokeWidth={isHovered ? 2 : 1}
                style={{ filter: isHovered ? 'drop-shadow(0 0 6px rgba(255,255,255,0.4))' : 'none' }}
              />

              {/* Node Label */}
              <text
                x={pos.x + nodeWidth / 2}
                y={pos.y - 6}
                fontSize="10"
                fontWeight="600"
                fill="#f8fafc"
                textAnchor="middle"
                fontFamily="Inter, sans-serif"
              >
                {pos.label}
              </text>

              {/* Biomass metric */}
              <text
                x={pos.x + nodeWidth / 2}
                y={pos.y + pos.height + 12}
                fontSize="9"
                fill="#94a3b8"
                textAnchor="middle"
                fontFamily="Inter, sans-serif"
              >
                {node.biomass.toFixed(0)} EU
              </text>
            </g>
          );
        })}
      </svg>

      {/* Hover Information Box */}
      {(hoveredLink || hoveredNode) && (
        <div
          data-testid="sankey-tooltip"
          style={{
            position: 'absolute',
            bottom: '16px',
            left: '16px',
            backgroundColor: 'rgba(9, 9, 11, 0.9)',
            border: '1px solid rgba(255, 255, 255, 0.15)',
            borderRadius: '6px',
            padding: '8px 12px',
            fontSize: '11px',
            color: '#f8fafc',
            boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
          }}
        >
          {hoveredLink && (
            <div>
              <div style={{ fontWeight: 600, color: '#38bdf8', marginBottom: '2px' }}>
                Flow: {nodePositions[hoveredLink.source]?.label || hoveredLink.source} ➔ {nodePositions[hoveredLink.target]?.label || hoveredLink.target}
              </div>
              <div>Flux Rate: <strong>{hoveredLink.value.toFixed(2)} EU/tick</strong> ({((hoveredLink.value / totalFlux) * 100).toFixed(1)}% of total flux)</div>
            </div>
          )}
          {hoveredNode && !hoveredLink && (
            <div>
              <div style={{ fontWeight: 600, color: nodePositions[hoveredNode.id]?.color || '#f8fafc', marginBottom: '2px' }}>
                Compartment: {hoveredNode.label || nodePositions[hoveredNode.id]?.label || hoveredNode.id}
              </div>
              <div>Biomass Pool: <strong>{hoveredNode.biomass.toFixed(1)} EU</strong></div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
