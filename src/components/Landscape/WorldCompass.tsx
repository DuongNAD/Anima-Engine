import React, { useEffect, useRef } from 'react';
import type { CameraView } from './WorldMinimap';

// ---------------------------------------------------------------------------------------
// WorldCompass — a first-person heading ribbon (N / E / S / W with degree ticks) that scrolls
// as you turn. Like the minimap, it reads the shared `viewRef` in its own rAF loop and paints
// a 2D canvas, so it never triggers a React re-render per frame.
//
// Heading convention matches the minimap: North = −Z (top of the map), East = +X, clockwise.
// ---------------------------------------------------------------------------------------

export interface WorldCompassProps {
  viewRef: React.MutableRefObject<CameraView>;
  width?: number;
}

const HEIGHT = 30;
const PX_PER_DEG = 2.2; // how wide the visible arc is (±~65° across a 288px strip)

const CARDINALS: Array<{ deg: number; label: string }> = [
  { deg: 0, label: 'N' },
  { deg: 45, label: 'NE' },
  { deg: 90, label: 'E' },
  { deg: 135, label: 'SE' },
  { deg: 180, label: 'S' },
  { deg: 225, label: 'SW' },
  { deg: 270, label: 'W' },
  { deg: 315, label: 'NW' },
];

/** Wrap an angle difference into (−180, 180]. */
function wrapDeg(d: number): number {
  let x = ((d + 180) % 360) - 180;
  if (x <= -180) x += 360;
  return x;
}

export const WorldCompass: React.FC<WorldCompassProps> = ({ viewRef, width = 288 }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx || typeof ctx.clearRect !== 'function') return;
    let active = true;
    const cx = width / 2;

    const loop = () => {
      if (!active) return;
      const view = viewRef.current;
      const dirX = view.targetX - view.camX;
      const dirZ = view.targetZ - view.camZ;
      // Bearing: 0° at North (−Z), increasing clockwise toward East (+X).
      const bearing = ((Math.atan2(dirX, -dirZ) * 180) / Math.PI + 360) % 360;

      ctx.clearRect(0, 0, width, HEIGHT);

      // Degree ticks every 15°, taller at cardinals.
      ctx.strokeStyle = 'rgba(226,232,240,0.55)';
      ctx.lineWidth = 1;
      for (let d = 0; d < 360; d += 15) {
        const dx = wrapDeg(d - bearing);
        if (Math.abs(dx) > width / (2 * PX_PER_DEG)) continue;
        const x = cx + dx * PX_PER_DEG;
        const tall = d % 45 === 0;
        ctx.beginPath();
        ctx.moveTo(x, HEIGHT - 8);
        ctx.lineTo(x, HEIGHT - (tall ? 16 : 12));
        ctx.stroke();
      }

      // Cardinal labels.
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      for (const { deg, label } of CARDINALS) {
        const dx = wrapDeg(deg - bearing);
        if (Math.abs(dx) > width / (2 * PX_PER_DEG)) continue;
        const x = cx + dx * PX_PER_DEG;
        const primary = label.length === 1;
        ctx.fillStyle = label === 'N' ? '#fca5a5' : primary ? '#f1f5f9' : 'rgba(203,213,225,0.75)';
        ctx.font = `${primary ? 'bold 13px' : '10px'} sans-serif`;
        ctx.fillText(label, x, 9);
      }

      // Centre index marker.
      ctx.fillStyle = '#7dd3fc';
      ctx.beginPath();
      ctx.moveTo(cx, HEIGHT - 6);
      ctx.lineTo(cx - 5, HEIGHT);
      ctx.lineTo(cx + 5, HEIGHT);
      ctx.closePath();
      ctx.fill();

      requestAnimationFrame(loop);
    };
    loop();
    return () => {
      active = false;
    };
  }, [viewRef, width]);

  return (
    <div
      style={{
        position: 'absolute',
        top: 12,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 100,
        borderRadius: 8,
        overflow: 'hidden',
        background: 'rgba(2,6,23,0.5)',
        backdropFilter: 'blur(6px)',
        boxShadow: '0 2px 12px rgba(0,0,0,0.35)',
        pointerEvents: 'none',
      }}
    >
      <canvas ref={canvasRef} width={width} height={HEIGHT} style={{ display: 'block' }} />
    </div>
  );
};

export default WorldCompass;
