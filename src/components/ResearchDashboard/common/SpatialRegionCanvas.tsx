import React, { useEffect, useRef, useState } from 'react';
import type { SpatialDensityReport } from '../../../types/generated/SpatialDensityReport';

export interface SpatialRegionParams {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
  cx: number;
  cy: number;
  r: number;
  cellX: number;
  cellY: number;
}

export interface SpatialRegionCanvasProps {
  mode: 'Global' | 'Rect' | 'Radius' | 'Cell';
  params: SpatialRegionParams;
  onChangeParams: (params: SpatialRegionParams) => void;
  densityReport?: SpatialDensityReport | null;
  width?: number;
  height?: number;
  testid?: string;
}

export const SpatialRegionCanvas: React.FC<SpatialRegionCanvasProps> = ({
  mode,
  params,
  onChangeParams,
  densityReport,
  width = 300,
  height = 300,
  testid = 'spatial-region-canvas',
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [isDrawing, setIsDrawing] = useState<boolean>(false);
  const [dragStart, setDragStart] = useState<{ x: number; y: number }>({ x: 0, y: 0 });

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

    // Clear background
    ctx.fillStyle = '#09090b';
    ctx.fillRect(0, 0, width, height);

    // Draw Density Grid Underlay if present
    if (densityReport && densityReport.width > 0 && densityReport.height > 0) {
      const rw = densityReport.width;
      const rh = densityReport.height;
      const cellW = width / rw;
      const cellH = height / rh;

      for (let y = 0; y < rh; y++) {
        for (let x = 0; x < rw; x++) {
          const idx = y * rw + x;
          const veg = densityReport.vegetationDensity[idx] || 0;
          const prey = densityReport.preyDensity[idx] || 0;

          if (veg > 0 || prey > 0) {
            const green = Math.round(veg * 120);
            const blue = Math.round(prey * 180);
            ctx.fillStyle = `rgba(16, ${100 + green}, ${50 + blue}, 0.3)`;
            ctx.fillRect(x * cellW, y * cellH, cellW, cellH);
          }
        }
      }
    }

    // Draw Grid Lines
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)';
    ctx.lineWidth = 1;
    const gridStep = width / 10;
    for (let i = 0; i <= 10; i++) {
      ctx.beginPath();
      ctx.moveTo(i * gridStep, 0);
      ctx.lineTo(i * gridStep, height);
      ctx.stroke();

      ctx.beginPath();
      ctx.moveTo(0, i * gridStep);
      ctx.lineTo(width, i * gridStep);
      ctx.stroke();
    }

    // Draw Region Overlay based on Mode
    if (mode === 'Global') {
      ctx.fillStyle = 'rgba(6, 182, 212, 0.15)';
      ctx.fillRect(0, 0, width, height);
      ctx.strokeStyle = '#06b6d4';
      ctx.lineWidth = 2;
      ctx.strokeRect(2, 2, width - 4, height - 4);
    } else if (mode === 'Rect') {
      const rx = Math.min(params.minX, params.maxX) * width;
      const ry = Math.min(params.minY, params.maxY) * height;
      const rw = Math.abs(params.maxX - params.minX) * width;
      const rh = Math.abs(params.maxY - params.minY) * height;

      ctx.fillStyle = 'rgba(59, 130, 246, 0.25)';
      ctx.fillRect(rx, ry, rw, rh);
      ctx.strokeStyle = '#3b82f6';
      ctx.lineWidth = 2;
      ctx.strokeRect(rx, ry, rw, rh);
    } else if (mode === 'Radius') {
      const cx = params.cx * width;
      const cy = params.cy * height;
      const r = params.r * width;

      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(245, 158, 11, 0.25)';
      ctx.fill();
      ctx.strokeStyle = '#f59e0b';
      ctx.lineWidth = 2;
      ctx.stroke();

      // Center dot
      ctx.beginPath();
      ctx.arc(cx, cy, 3, 0, Math.PI * 2);
      ctx.fillStyle = '#fef08a';
      ctx.fill();
    } else if (mode === 'Cell') {
      const gridRes = 16;
      const cellW = width / gridRes;
      const cellH = height / gridRes;
      const cx = params.cellX * cellW;
      const cy = params.cellY * cellH;

      ctx.fillStyle = 'rgba(239, 68, 68, 0.35)';
      ctx.fillRect(cx, cy, cellW, cellH);
      ctx.strokeStyle = '#ef4444';
      ctx.lineWidth = 2;
      ctx.strokeRect(cx, cy, cellW, cellH);
    }
  }, [mode, params, densityReport, width, height]);

  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (mode === 'Global') return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const nx = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const ny = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));

    setIsDrawing(true);
    setDragStart({ x: nx, y: ny });

    if (mode === 'Cell') {
      const gridRes = 16;
      const cellX = Math.floor(nx * gridRes);
      const cellY = Math.floor(ny * gridRes);
      onChangeParams({ ...params, cellX, cellY });
    } else if (mode === 'Radius') {
      onChangeParams({ ...params, cx: nx, cy: ny, r: 0.1 });
    } else if (mode === 'Rect') {
      onChangeParams({ ...params, minX: nx, minY: ny, maxX: nx, maxY: ny });
    }
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!isDrawing) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const nx = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const ny = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));

    if (mode === 'Rect') {
      onChangeParams({
        ...params,
        minX: Math.min(dragStart.x, nx),
        minY: Math.min(dragStart.y, ny),
        maxX: Math.max(dragStart.x, nx),
        maxY: Math.max(dragStart.y, ny),
      });
    } else if (mode === 'Radius') {
      const dx = nx - dragStart.x;
      const dy = ny - dragStart.y;
      const r = Math.sqrt(dx * dx + dy * dy);
      onChangeParams({
        ...params,
        cx: dragStart.x,
        cy: dragStart.y,
        r: Math.max(0.02, r),
      });
    }
  };

  const handleMouseUp = () => {
    setIsDrawing(false);
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
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        style={{
          width,
          height,
          display: 'block',
          borderRadius: '6px',
          border: '1px solid rgba(255, 255, 255, 0.1)',
          cursor: mode === 'Global' ? 'default' : 'crosshair',
        }}
      />
      <div style={{ marginTop: '6px', fontSize: '11px', color: '#94a3b8' }}>
        {mode === 'Global' && 'Region: Global (All Entities)'}
        {mode === 'Rect' && `Region: Rect [(${params.minX.toFixed(2)}, ${params.minY.toFixed(2)}) ➔ (${params.maxX.toFixed(2)}, ${params.maxY.toFixed(2)})]`}
        {mode === 'Radius' && `Region: Radius [Center: (${params.cx.toFixed(2)}, ${params.cy.toFixed(2)}), r: ${params.r.toFixed(2)}]`}
        {mode === 'Cell' && `Region: Cell [(${params.cellX}, ${params.cellY})]`}
      </div>
    </div>
  );
};
