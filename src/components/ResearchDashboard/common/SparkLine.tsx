import React from 'react';

export interface SparkSeries {
  label: string;
  color: string;
  values: number[];
}

export interface SparkLineProps {
  series: SparkSeries[];
  testid?: string;
  height?: number;
  width?: number | string;
  showBaseline?: boolean;
  showLastDot?: boolean;
}

export const SparkLine: React.FC<SparkLineProps> = ({
  series,
  testid = 'sparkline-svg',
  height = 36,
  width = '100%',
  showBaseline = true,
  showLastDot = true,
}) => {
  const W = 240;
  const H = height;
  const pad = 3;
  const allValues = series.flatMap((s) => s.values.filter((v) => !isNaN(v) && isFinite(v)));
  const lo = allValues.length > 0 ? Math.min(...allValues, 0) : 0;
  const hi = allValues.length > 0 ? Math.max(...allValues, 1) : 1;
  const span = hi - lo || 1;

  const x = (i: number, len: number) => {
    if (len <= 1) return W / 2;
    return (i / (len - 1)) * (W - pad * 2) + pad;
  };

  const y = (v: number) => {
    const clamped = isNaN(v) || !isFinite(v) ? lo : v;
    return H - pad - ((clamped - lo) / span) * (H - pad * 2);
  };

  return (
    <svg
      data-testid={testid}
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      style={{ width, height, display: 'block', overflow: 'visible' }}
      role="img"
    >
      {showBaseline && (
        <line
          x1={0}
          y1={H - pad}
          x2={W}
          y2={H - pad}
          stroke="rgba(255, 255, 255, 0.1)"
          strokeWidth={1}
          strokeDasharray="2 2"
        />
      )}
      {series.map((s) => {
        if (s.values.length === 0) return null;
        if (s.values.length === 1) {
          const singleX = W / 2;
          const singleY = y(s.values[0]);
          return (
            <circle
              key={s.label}
              cx={singleX}
              cy={singleY}
              r={3}
              fill={s.color}
              data-testid={`spark-point-${s.label}`}
            >
              <title>{`${s.label}: ${s.values[0].toFixed(2)}`}</title>
            </circle>
          );
        }

        const pts = s.values.map((v, i) => `${x(i, s.values.length).toFixed(1)},${y(v).toFixed(1)}`).join(' ');
        const lastIdx = s.values.length - 1;
        const lastX = x(lastIdx, s.values.length);
        const lastY = y(s.values[lastIdx]);
        const lastVal = s.values[lastIdx];

        return (
          <g key={s.label} data-testid={`spark-series-${s.label}`}>
            <title>{`${s.label}: ${lastVal !== undefined ? lastVal.toFixed(2) : 'N/A'}`}</title>
            <polyline
              points={pts}
              fill="none"
              stroke={s.color}
              strokeWidth={1.8}
              strokeLinejoin="round"
              strokeLinecap="round"
            />
            {showLastDot && (
              <circle
                cx={lastX}
                cy={lastY}
                r={2.5}
                fill={s.color}
                stroke="#09090b"
                strokeWidth={1}
              />
            )}
          </g>
        );
      })}
    </svg>
  );
};
