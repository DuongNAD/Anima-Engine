import React from 'react';

export type CurveKind = 'Step' | 'RampUp' | 'RampDown' | 'Triangle';

export interface CurveRampPreviewProps {
  curve: CurveKind;
  intensity: number;
  duration: number;
  signedNegative?: boolean;
  width?: number;
  height?: number;
  testid?: string;
}

export const CurveRampPreview: React.FC<CurveRampPreviewProps> = ({
  curve,
  intensity,
  duration,
  signedNegative = false,
  width = 240,
  height = 80,
  testid = 'curve-ramp-preview-svg',
}) => {
  const W = width;
  const H = height;
  const pad = 12;
  const chartW = W - 2 * pad;
  const chartH = H - 2 * pad;

  // Generate curve path points (progress 0 to 1)
  const steps = 30;
  const points: { x: number; y: number }[] = [];

  for (let i = 0; i <= steps; i++) {
    const p = i / steps;
    let factor = 1.0;
    if (curve === 'Step') {
      factor = 1.0;
    } else if (curve === 'RampUp') {
      factor = p;
    } else if (curve === 'RampDown') {
      factor = 1.0 - p;
    } else if (curve === 'Triangle') {
      factor = 1.0 - Math.abs(2.0 * p - 1.0);
    }

    const effectiveFactor = signedNegative ? -factor : factor;
    const normY = signedNegative ? 0.5 - (effectiveFactor * 0.45) : 1.0 - (factor * 0.9);

    points.push({
      x: pad + p * chartW,
      y: pad + normY * chartH,
    });
  }

  const pathD = points.map((pt, i) => `${i === 0 ? 'M' : 'L'} ${pt.x.toFixed(1)} ${pt.y.toFixed(1)}`).join(' ');
  const baselineY = signedNegative ? pad + 0.5 * chartH : pad + chartH;
  const areaD = `${pathD} L ${pad + chartW} ${baselineY} L ${pad} ${baselineY} Z`;

  const strokeColor = signedNegative ? '#ef4444' : '#10b981';
  const fillColor = signedNegative ? 'rgba(239, 68, 68, 0.2)' : 'rgba(16, 185, 129, 0.2)';

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        backgroundColor: 'rgba(15, 23, 42, 0.6)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        borderRadius: '6px',
        padding: '6px',
      }}
    >
      <svg
        data-testid={testid}
        viewBox={`0 0 ${W} ${H}`}
        style={{ width: '100%', height, display: 'block', overflow: 'visible' }}
        role="img"
      >
        {/* Baseline */}
        <line
          x1={pad}
          y1={baselineY}
          x2={pad + chartW}
          y2={baselineY}
          stroke="rgba(255, 255, 255, 0.15)"
          strokeWidth={1}
          strokeDasharray="2 2"
        />

        {/* Shaded Area */}
        <path d={areaD} fill={fillColor} />

        {/* Curve Line */}
        <path
          d={pathD}
          fill="none"
          stroke={strokeColor}
          strokeWidth={2}
          strokeLinecap="round"
          strokeLinejoin="round"
        />

        {/* Start / End Points */}
        <circle cx={points[0].x} cy={points[0].y} r={2.5} fill={strokeColor} />
        <circle cx={points[points.length - 1].x} cy={points[points.length - 1].y} r={2.5} fill={strokeColor} />

        {/* Labels */}
        <text x={pad} y={H - 2} fontSize="9" fill="#64748b">
          t = 0
        </text>
        <text x={W - pad} y={H - 2} fontSize="9" textAnchor="end" fill="#64748b">
          t = {duration}t
        </text>
        <text x={W / 2} y={10} fontSize="9" textAnchor="middle" fill="#cbd5e1" fontWeight="600">
          {curve} ({signedNegative ? '-' : '+'}{(intensity * 100).toFixed(0)}%)
        </text>
      </svg>
    </div>
  );
};
