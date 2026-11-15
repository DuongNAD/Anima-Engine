import React, { useState, useRef } from 'react';

export interface TimeSeriesItem {
  label: string;
  color: string;
  values: number[];
  strokeDash?: string;
  strokeWidth?: number;
}

export interface TimeSeriesChartProps {
  series: TimeSeriesItem[];
  title?: string;
  unit?: string;
  height?: number;
  timeWindow?: number;
  onTimeWindowChange?: (w: number) => void;
  referenceLine?: { y: number; label: string; color: string };
  testid?: string;
}

export const TimeSeriesChart: React.FC<TimeSeriesChartProps> = ({
  series,
  title,
  unit = '',
  height = 200,
  timeWindow = 500,
  onTimeWindowChange,
  referenceLine,
  testid = 'time-series-chart',
}) => {
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Apply timeWindow slice to all series
  const slicedSeries = series.map((s) => {
    const len = s.values.length;
    const startIndex = timeWindow > 0 && len > timeWindow ? len - timeWindow : 0;
    return {
      ...s,
      values: s.values.slice(startIndex),
    };
  });

  const maxLen = Math.max(...slicedSeries.map((s) => s.values.length), 0);
  const allValues = slicedSeries
    .flatMap((s) => s.values)
    .concat(referenceLine ? [referenceLine.y] : [])
    .filter((v) => !isNaN(v) && isFinite(v));

  const rawMin = allValues.length > 0 ? Math.min(...allValues) : 0;
  const rawMax = allValues.length > 0 ? Math.max(...allValues) : 1;
  const minVal = Math.min(rawMin, 0);
  const maxVal = Math.max(rawMax, minVal + 1);
  const valSpan = maxVal - minVal || 1;

  const W = 600;
  const H = height;
  const padLeft = 45;
  const padRight = 15;
  const padTop = 15;
  const padBottom = 25;
  const chartW = W - padLeft - padRight;
  const chartH = H - padTop - padBottom;

  const getX = (i: number, len: number) => {
    if (len <= 1) return padLeft + chartW / 2;
    return padLeft + (i / (len - 1)) * chartW;
  };

  const getY = (v: number) => {
    const clamped = isNaN(v) || !isFinite(v) ? minVal : v;
    return padTop + chartH - ((clamped - minVal) / valSpan) * chartH;
  };

  const handleMouseMove = (e: React.MouseEvent<SVGSVGElement>) => {
    if (!containerRef.current || maxLen <= 1) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const clientX = e.clientX - rect.left;
    const normX = (clientX - (padLeft / W) * rect.width) / ((chartW / W) * rect.width);
    const clampedNormX = Math.max(0, Math.min(1, normX));
    const idx = Math.round(clampedNormX * (maxLen - 1));
    setHoverIndex(idx);
  };

  const handleMouseLeave = () => {
    setHoverIndex(null);
  };

  return (
    <div
      ref={containerRef}
      data-testid={testid}
      style={{
        backgroundColor: 'rgba(15, 23, 42, 0.55)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        borderRadius: '8px',
        padding: '12px',
        color: '#e2e8f0',
        display: 'flex',
        flexDirection: 'column',
        gap: '8px',
      }}
    >
      {/* Chart Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap' }}>
        {title && (
          <div style={{ fontWeight: 600, fontSize: '13px', color: '#f8fafc' }}>
            {title}
          </div>
        )}
        {onTimeWindowChange && (
          <div style={{ display: 'flex', gap: '4px', fontSize: '11px' }}>
            {[100, 500, 2000, 0].map((w) => (
              <button
                key={w}
                data-testid={`time-window-${w || 'all'}`}
                onClick={() => onTimeWindowChange(w)}
                style={{
                  padding: '2px 8px',
                  borderRadius: '4px',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  backgroundColor: timeWindow === w ? '#0284c7' : 'rgba(255, 255, 255, 0.05)',
                  color: timeWindow === w ? '#ffffff' : '#94a3b8',
                  cursor: 'pointer',
                  fontSize: '11px',
                }}
              >
                {w === 0 ? 'All' : `${w}t`}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* SVG Canvas */}
      <div style={{ width: '100%', position: 'relative' }}>
        <svg
          viewBox={`0 0 ${W} ${H}`}
          style={{ width: '100%', height, display: 'block', overflow: 'visible', cursor: 'crosshair' }}
          onMouseMove={handleMouseMove}
          onMouseLeave={handleMouseLeave}
          role="img"
        >
          {/* Grid lines */}
          {[0, 0.25, 0.5, 0.75, 1].map((pct) => {
            const y = padTop + chartH * (1 - pct);
            const val = minVal + pct * valSpan;
            return (
              <g key={pct}>
                <line
                  x1={padLeft}
                  y1={y}
                  x2={W - padRight}
                  y2={y}
                  stroke="rgba(255, 255, 255, 0.06)"
                  strokeWidth={1}
                />
                <text
                  x={padLeft - 6}
                  y={y + 3}
                  fontSize="10"
                  textAnchor="end"
                  fill="#64748b"
                  fontFamily="sans-serif"
                >
                  {val >= 1000 ? `${(val / 1000).toFixed(1)}k` : val.toFixed(val < 10 ? 1 : 0)}
                </text>
              </g>
            );
          })}

          {/* Reference Line */}
          {referenceLine && (
            <g data-testid="chart-reference-line">
              <line
                x1={padLeft}
                y1={getY(referenceLine.y)}
                x2={W - padRight}
                y2={getY(referenceLine.y)}
                stroke={referenceLine.color}
                strokeWidth={1.5}
                strokeDasharray="4 4"
              />
              <text
                x={W - padRight - 5}
                y={getY(referenceLine.y) - 4}
                fontSize="9"
                textAnchor="end"
                fill={referenceLine.color}
              >
                {referenceLine.label}
              </text>
            </g>
          )}

          {/* Series lines */}
          {slicedSeries.map((s) => {
            if (s.values.length < 2) return null;
            const pts = s.values.map((v, i) => `${getX(i, s.values.length).toFixed(1)},${getY(v).toFixed(1)}`).join(' ');
            return (
              <polyline
                key={s.label}
                data-testid={`series-line-${s.label}`}
                points={pts}
                fill="none"
                stroke={s.color}
                strokeWidth={s.strokeWidth || 1.8}
                strokeDasharray={s.strokeDash}
                strokeLinejoin="round"
                strokeLinecap="round"
              />
            );
          })}

          {/* Hover Crosshair & Indicators */}
          {hoverIndex !== null && hoverIndex >= 0 && hoverIndex < maxLen && (
            <g data-testid="chart-crosshair">
              <line
                x1={getX(hoverIndex, maxLen)}
                y1={padTop}
                x2={getX(hoverIndex, maxLen)}
                y2={padTop + chartH}
                stroke="rgba(255, 255, 255, 0.4)"
                strokeWidth={1}
                strokeDasharray="3 3"
              />
              {slicedSeries.map((s) => {
                const val = s.values[hoverIndex];
                if (val === undefined || isNaN(val)) return null;
                return (
                  <circle
                    key={s.label}
                    cx={getX(hoverIndex, s.values.length)}
                    cy={getY(val)}
                    r={3.5}
                    fill={s.color}
                    stroke="#ffffff"
                    strokeWidth={1}
                  />
                );
              })}
            </g>
          )}
        </svg>

        {/* Hover Tooltip Overlay */}
        {hoverIndex !== null && hoverIndex >= 0 && hoverIndex < maxLen && (
          <div
            data-testid="chart-tooltip"
            style={{
              position: 'absolute',
              top: '10px',
              right: '10px',
              backgroundColor: 'rgba(9, 9, 11, 0.85)',
              border: '1px solid rgba(255, 255, 255, 0.15)',
              borderRadius: '6px',
              padding: '6px 10px',
              fontSize: '11px',
              color: '#f8fafc',
              pointerEvents: 'none',
              boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
            }}
          >
            <div style={{ color: '#94a3b8', marginBottom: '4px', fontWeight: 500 }}>
              Sample #{hoverIndex}
            </div>
            {slicedSeries.map((s) => {
              const val = s.values[hoverIndex];
              if (val === undefined || isNaN(val)) return null;
              return (
                <div key={s.label} style={{ display: 'flex', alignItems: 'center', gap: '6px', marginBottom: '2px' }}>
                  <span style={{ width: '8px', height: '8px', borderRadius: '2px', backgroundColor: s.color }} />
                  <span style={{ color: '#cbd5e1' }}>{s.label}:</span>
                  <span style={{ fontWeight: 600 }}>{val.toFixed(2)} {unit}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Legend Row */}
      <div style={{ display: 'flex', gap: '14px', flexWrap: 'wrap', fontSize: '11px', color: '#94a3b8', marginTop: '2px' }}>
        {slicedSeries.map((s) => {
          const lastVal = s.values[s.values.length - 1];
          return (
            <div key={s.label} style={{ display: 'inline-flex', alignItems: 'center', gap: '5px' }}>
              <span style={{ width: '8px', height: '8px', borderRadius: '2px', backgroundColor: s.color }} />
              <span>{s.label}</span>
              {lastVal !== undefined && (
                <span style={{ color: '#f1f5f9', fontWeight: 600 }}>
                  ({lastVal.toFixed(1)} {unit})
                </span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};
