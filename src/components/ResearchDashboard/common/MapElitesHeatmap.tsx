import React, { useEffect, useRef, useState } from 'react';
import type { EliteIndividualState } from '../../../types/generated/EliteIndividualState';

export interface MapElitesHeatmapProps {
  grid: { [key: string]: EliteIndividualState | undefined };
  gridResolution?: number;
  width?: number;
  height?: number;
  onSelectCell?: (coord: string, elite: EliteIndividualState | null) => void;
  testid?: string;
}

export const MapElitesHeatmap: React.FC<MapElitesHeatmapProps> = ({
  grid,
  gridResolution = 50,
  width = 300,
  height = 300,
  onSelectCell,
  testid = 'map-elites-heatmap-canvas',
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [hoveredCell, setHoveredCell] = useState<{ x: number; y: number; elite: EliteIndividualState | null } | null>(null);

  // Compute fitness color ramp: 0.0 -> blue, 0.5 -> yellow, 1.0 -> crimson
  const getFitnessColor = (fitness: number) => {
    const f = Math.max(0, Math.min(1, fitness));
    if (f < 0.33) {
      // Blue to Cyan
      const r = Math.round(30 + f * 3 * 30);
      const g = Math.round(58 + f * 3 * 150);
      const b = Math.round(138 + f * 3 * 100);
      return `rgb(${r}, ${g}, ${b})`;
    } else if (f < 0.66) {
      // Cyan to Yellow
      const p = (f - 0.33) * 3;
      const r = Math.round(60 + p * 180);
      const g = Math.round(208 + p * 40);
      const b = Math.round(238 - p * 200);
      return `rgb(${r}, ${g}, ${b})`;
    } else {
      // Yellow to Crimson
      const p = (f - 0.66) * 3;
      const r = Math.round(240 + p * 15);
      const g = Math.round(248 - p * 200);
      const b = Math.round(38 + p * 30);
      return `rgb(${r}, ${g}, ${b})`;
    }
  };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    if (typeof ctx.scale === 'function') {
      ctx.scale(dpr, dpr);
    }

    ctx.clearRect(0, 0, width, height);

    const res = Math.max(1, gridResolution);
    const cellW = width / res;
    const cellH = height / res;

    // Draw all cells
    for (let y = 0; y < res; y++) {
      for (let x = 0; x < res; x++) {
        const key = `${x},${y}`;
        const elite = grid[key];

        if (elite && typeof elite.fitness === 'number') {
          ctx.fillStyle = getFitnessColor(elite.fitness);
          ctx.fillRect(x * cellW, (res - 1 - y) * cellH, cellW, cellH);
        } else {
          ctx.fillStyle = (x + y) % 2 === 0 ? 'rgba(255, 255, 255, 0.02)' : 'rgba(255, 255, 255, 0.04)';
          ctx.fillRect(x * cellW, (res - 1 - y) * cellH, cellW, cellH);
        }
      }
    }

    // Draw hover outline if present
    if (hoveredCell) {
      ctx.strokeStyle = '#ffffff';
      ctx.lineWidth = 1.5;
      ctx.strokeRect(
        hoveredCell.x * cellW,
        (res - 1 - hoveredCell.y) * cellH,
        cellW,
        cellH
      );
    }
  }, [grid, gridResolution, width, height, hoveredCell]);

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const clientX = e.clientX - rect.left;
    const clientY = e.clientY - rect.top;

    const res = Math.max(1, gridResolution);
    const cellX = Math.floor((clientX / rect.width) * res);
    const rawCellY = Math.floor((clientY / rect.height) * res);
    const cellY = res - 1 - rawCellY;

    if (cellX >= 0 && cellX < res && cellY >= 0 && cellY < res) {
      const key = `${cellX},${cellY}`;
      const elite = grid[key] || null;
      setHoveredCell({ x: cellX, y: cellY, elite });
    }
  };

  const handleClick = () => {
    if (hoveredCell && onSelectCell) {
      onSelectCell(`${hoveredCell.x},${hoveredCell.y}`, hoveredCell.elite);
    }
  };

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        position: 'relative',
      }}
    >
      <canvas
        ref={canvasRef}
        data-testid={testid}
        onMouseMove={handleMouseMove}
        onMouseLeave={() => setHoveredCell(null)}
        onClick={handleClick}
        style={{
          width,
          height,
          display: 'block',
          borderRadius: '6px',
          border: '1px solid rgba(255, 255, 255, 0.1)',
          cursor: 'pointer',
          backgroundColor: '#09090b',
        }}
      />

      {/* Hover Info Tooltip */}
      {hoveredCell && (
        <div
          data-testid="map-elites-cell-tooltip"
          style={{
            marginTop: '6px',
            fontSize: '11px',
            color: '#cbd5e1',
            backgroundColor: 'rgba(15, 23, 42, 0.9)',
            border: '1px solid rgba(255, 255, 255, 0.1)',
            padding: '4px 8px',
            borderRadius: '4px',
            width: '100%',
            textAlign: 'center',
          }}
        >
          Cell ({hoveredCell.x}, {hoveredCell.y}):{' '}
          {hoveredCell.elite ? (
            <span style={{ color: '#38bdf8', fontWeight: 600 }}>
              Fitness: {hoveredCell.elite.fitness.toFixed(3)} | Features: [
              {hoveredCell.elite.features.map((f) => f.toFixed(2)).join(', ')}]
            </span>
          ) : (
            <span style={{ color: '#94a3b8', fontStyle: 'italic' }}>Empty niche</span>
          )}
        </div>
      )}
    </div>
  );
};
