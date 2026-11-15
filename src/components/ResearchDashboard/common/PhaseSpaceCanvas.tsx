import React, { useEffect, useRef } from 'react';

export interface PhaseSpaceCanvasProps {
  preyHistory: number[];
  predatorHistory: number[];
  width?: number;
  height?: number;
  testid?: string;
  preyColor?: string;
  predColor?: string;
}

export const PhaseSpaceCanvas: React.FC<PhaseSpaceCanvasProps> = ({
  preyHistory,
  predatorHistory,
  width = 280,
  height = 240,
  testid = 'phase-space-canvas',
  preyColor = '#3182ce',
  predColor = '#e53e3e',
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    ctx.scale(dpr, dpr);

    // Clear background
    ctx.clearRect(0, 0, width, height);

    const pad = 35;
    const plotW = width - pad * 1.5;
    const plotH = height - pad * 1.5;

    // Calculate bounds with fallback
    const validPrey = preyHistory.filter((v) => !isNaN(v) && isFinite(v));
    const validPred = predatorHistory.filter((v) => !isNaN(v) && isFinite(v));

    const minPrey = validPrey.length > 0 ? Math.min(0, Math.min(...validPrey)) : 0;
    const maxPrey = validPrey.length > 0 ? Math.max(10, Math.max(...validPrey) * 1.15) : 50;
    const minPred = validPred.length > 0 ? Math.min(0, Math.min(...validPred)) : 0;
    const maxPred = validPred.length > 0 ? Math.max(5, Math.max(...validPred) * 1.15) : 20;

    const spanPrey = maxPrey - minPrey || 1;
    const spanPred = maxPred - minPred || 1;

    const toX = (prey: number) => pad + ((prey - minPrey) / spanPrey) * plotW;
    const toY = (pred: number) => height - pad - ((pred - minPred) / spanPred) * plotH;

    // Draw Axes & Grid
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.08)';
    ctx.lineWidth = 1;

    // Vertical gridlines
    for (let i = 0; i <= 4; i++) {
      const p = i / 4;
      const x = pad + p * plotW;
      const val = minPrey + p * spanPrey;
      ctx.beginPath();
      ctx.moveTo(x, height - pad);
      ctx.lineTo(x, height - pad - plotH);
      ctx.stroke();

      ctx.fillStyle = '#64748b';
      ctx.font = '9px Inter, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(val.toFixed(0), x, height - pad + 14);
    }

    // Horizontal gridlines
    for (let i = 0; i <= 4; i++) {
      const p = i / 4;
      const y = height - pad - p * plotH;
      const val = minPred + p * spanPred;
      ctx.beginPath();
      ctx.moveTo(pad, y);
      ctx.lineTo(pad + plotW, y);
      ctx.stroke();

      ctx.fillStyle = '#64748b';
      ctx.font = '9px Inter, sans-serif';
      ctx.textAlign = 'right';
      ctx.fillText(val.toFixed(0), pad - 6, y + 3);
    }

    // Axis Labels
    ctx.fillStyle = preyColor;
    ctx.font = '10px Inter, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('N_prey (Herbivore Population)', pad + plotW / 2, height - 6);

    if (typeof ctx.save === 'function' && typeof ctx.translate === 'function' && typeof ctx.rotate === 'function') {
      ctx.save();
      ctx.translate(12, height - pad - plotH / 2);
      ctx.rotate(-Math.PI / 2);
      ctx.fillStyle = predColor;
      ctx.textAlign = 'center';
      ctx.fillText('N_pred (Predator Population)', 0, 0);
      if (typeof ctx.restore === 'function') ctx.restore();
    }

    const n = Math.min(preyHistory.length, predatorHistory.length);
    if (n < 2) {
      ctx.fillStyle = '#94a3b8';
      ctx.font = '11px Inter, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('Accumulating attractor trajectory...', pad + plotW / 2, height - pad - plotH / 2);
      return;
    }

    // Compute rolling equilibrium center (mean)
    let sumPrey = 0;
    let sumPred = 0;
    const windowSize = Math.min(n, 120);
    const startIdx = n - windowSize;
    for (let i = startIdx; i < n; i++) {
      sumPrey += preyHistory[i] || 0;
      sumPred += predatorHistory[i] || 0;
    }
    const meanPrey = sumPrey / windowSize;
    const meanPred = sumPred / windowSize;

    // Draw equilibrium crosshair
    const eqX = toX(meanPrey);
    const eqY = toY(meanPred);
    ctx.strokeStyle = 'rgba(245, 158, 11, 0.4)';
    if (typeof ctx.setLineDash === 'function') ctx.setLineDash([3, 3]);
    ctx.beginPath();
    ctx.moveTo(eqX - 10, eqY);
    ctx.lineTo(eqX + 10, eqY);
    ctx.moveTo(eqX, eqY - 10);
    ctx.lineTo(eqX, eqY + 10);
    ctx.stroke();
    if (typeof ctx.setLineDash === 'function') ctx.setLineDash([]);

    // Draw Trajectory with progressive alpha
    const maxTrail = 300;
    const trailStart = Math.max(0, n - maxTrail);

    for (let i = trailStart; i < n - 1; i++) {
      const p1 = preyHistory[i];
      const pr1 = predatorHistory[i];
      const p2 = preyHistory[i + 1];
      const pr2 = predatorHistory[i + 1];

      if (isNaN(p1) || isNaN(pr1) || isNaN(p2) || isNaN(pr2)) continue;

      const alpha = ((i - trailStart + 1) / (n - trailStart)) * 0.85 + 0.15;
      ctx.strokeStyle = `rgba(56, 189, 248, ${alpha.toFixed(2)})`;
      ctx.lineWidth = 1.8;
      ctx.beginPath();
      ctx.moveTo(toX(p1), toY(pr1));
      ctx.lineTo(toX(p2), toY(pr2));
      ctx.stroke();

      // Flow Vector Arrow on periodic segments
      if ((n - 1 - i) % 30 === 15 && i > trailStart + 10) {
        const x1 = toX(p1);
        const y1 = toY(pr1);
        const x2 = toX(p2);
        const y2 = toY(pr2);
        const angle = Math.atan2(y2 - y1, x2 - x1);
        const arrowLen = 5;

        ctx.fillStyle = `rgba(56, 189, 248, ${alpha.toFixed(2)})`;
        ctx.beginPath();
        ctx.moveTo(x2, y2);
        ctx.lineTo(
          x2 - arrowLen * Math.cos(angle - Math.PI / 6),
          y2 - arrowLen * Math.sin(angle - Math.PI / 6),
        );
        ctx.lineTo(
          x2 - arrowLen * Math.cos(angle + Math.PI / 6),
          y2 - arrowLen * Math.sin(angle + Math.PI / 6),
        );
        ctx.closePath();
        ctx.fill();
      }
    }

    // Current State Dot
    const currPrey = preyHistory[n - 1];
    const currPred = predatorHistory[n - 1];
    const currX = toX(currPrey);
    const currY = toY(currPred);

    // Glowing halo
    ctx.beginPath();
    ctx.arc(currX, currY, 6, 0, Math.PI * 2);
    ctx.fillStyle = 'rgba(6, 182, 212, 0.35)';
    ctx.fill();

    ctx.beginPath();
    ctx.arc(currX, currY, 3.5, 0, Math.PI * 2);
    ctx.fillStyle = '#38bdf8';
    ctx.fill();
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 1;
    ctx.stroke();
  }, [preyHistory, predatorHistory, width, height, preyColor, predColor]);

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        backgroundColor: 'rgba(15, 23, 42, 0.55)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        borderRadius: '8px',
        padding: '10px',
      }}
    >
      <div style={{ fontSize: '12px', fontWeight: 600, color: '#f8fafc', marginBottom: '6px', alignSelf: 'flex-start' }}>
        Lotka-Volterra Limit Cycle Orbits
      </div>
      <canvas
        ref={canvasRef}
        data-testid={testid}
        style={{
          width,
          height,
          display: 'block',
          borderRadius: '4px',
        }}
      />
    </div>
  );
};
