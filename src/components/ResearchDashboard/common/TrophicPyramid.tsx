import React from 'react';

/**
 * Biomass per compartment, in EU. `null` means the backend does not report that compartment: the
 * tier is drawn as "not reported" and no efficiency involving it is computed. Today the backend
 * reports animal energy as one pool, so the panel passes `null` for herbivores and carnivores and
 * the whole pool as `consumers`, rather than inventing a split.
 */
export interface TrophicBiomassData {
  detritus: number | null;
  plants: number | null;
  herbivores: number | null;
  carnivores: number | null;
  /** All animals together, shown when the consumer split is not reported. */
  consumers?: number | null;
}

export interface TrophicPyramidProps {
  data: TrophicBiomassData;
  width?: number;
  height?: number;
  testid?: string;
}

export const TrophicPyramid: React.FC<TrophicPyramidProps> = ({
  data,
  width = 380,
  height = 280,
  testid = 'trophic-pyramid-svg',
}) => {
  const splitReported = data.herbivores !== null && data.carnivores !== null;
  const consumerBiomass = splitReported ? (data.herbivores ?? 0) + (data.carnivores ?? 0) : (data.consumers ?? null);
  const totalBiomass = Math.max(1, (data.detritus ?? 0) + (data.plants ?? 0) + (consumerBiomass ?? 0));

  // Lindeman transfer efficiencies (%), only where both tiers are reported.
  const l12 =
    data.plants !== null && data.herbivores !== null && data.plants > 0 ? (data.herbivores / data.plants) * 100 : null;
  const l23 =
    data.herbivores !== null && data.carnivores !== null && data.herbivores > 0
      ? (data.carnivores / data.herbivores) * 100
      : null;
  const pctText = (v: number | null) => (v === null ? '—' : `${v.toFixed(1)}%`);

  // Tier definitions: [Index, Key, Label, Color, Biomass]
  const tiers = [
    {
      id: 'carnivores',
      label: 'Secondary Consumers (Carnivores)',
      shortLabel: 'Carnivores (Apex)',
      biomass: data.carnivores,
      color: '#e53e3e',
      bgColor: 'rgba(229, 62, 62, 0.25)',
      borderColor: '#ef4444',
      level: 3,
    },
    {
      id: 'herbivores',
      label: 'Primary Consumers (Herbivores)',
      shortLabel: 'Herbivores / Prey',
      biomass: data.herbivores,
      color: '#3182ce',
      bgColor: 'rgba(49, 130, 206, 0.25)',
      borderColor: '#3b82f6',
      level: 2,
    },
    {
      id: 'plants',
      label: 'Primary Producers (Plants & Flora)',
      shortLabel: 'Primary Producers',
      biomass: data.plants,
      color: '#38a169',
      bgColor: 'rgba(56, 161, 105, 0.25)',
      borderColor: '#10b981',
      level: 1,
    },
    {
      id: 'detritus',
      label: 'Substrate & Recycled Organics (Detritus)',
      shortLabel: 'Detritus & Substrate',
      biomass: data.detritus,
      color: '#805ad5',
      bgColor: 'rgba(128, 90, 213, 0.25)',
      borderColor: '#a855f7',
      level: 0,
    },
  ];

  const W = width;
  const H = height;
  const tierHeight = 52;
  const gap = 8;
  const maxTierWidth = W - 120;
  const minTierWidth = 90;

  // Width proportion based on log / linear hybrid to preserve visibility
  const getTierWidth = (biomass: number | null) => {
    if (biomass === null) return minTierWidth;
    const rawRatio = biomass / totalBiomass;
    const logRatio = Math.log10(1 + biomass) / Math.log10(1 + totalBiomass);
    const blend = rawRatio * 0.4 + logRatio * 0.6;
    return Math.max(minTierWidth, blend * maxTierWidth);
  };

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        backgroundColor: 'rgba(15, 23, 42, 0.55)',
        border: '1px solid rgba(255, 255, 255, 0.08)',
        borderRadius: '8px',
        padding: '12px',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', width: '100%', marginBottom: '10px' }}>
        <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
          Trophic Biomass Pyramid & Lindeman Efficiency
        </div>
        <div style={{ fontSize: '11px', color: '#94a3b8' }}>
          Total Biomass: <span style={{ color: '#f1f5f9', fontWeight: 600 }}>{totalBiomass.toFixed(1)} EU</span>
        </div>
      </div>

      {!splitReported && (
        <div data-testid="trophic-split-unreported" style={{ fontSize: '11px', color: '#94a3b8', width: '100%', marginBottom: '6px' }}>
          Consumers (all animals):{' '}
          <span style={{ color: '#f1f5f9', fontWeight: 600 }}>
            {consumerBiomass === null ? 'not reported' : `${consumerBiomass.toFixed(1)} EU`}
          </span>
          . The backend does not split animal energy by trophic role, so the two consumer tiers below
          are not reported.
        </div>
      )}

      <svg
        data-testid={testid}
        viewBox={`0 0 ${W} ${H}`}
        style={{ width: '100%', height, display: 'block', overflow: 'visible' }}
        role="img"
      >
        {tiers.map((tier, idx) => {
          const tWidth = getTierWidth(tier.biomass);
          const tX = (W - tWidth) / 2;
          const tY = 15 + idx * (tierHeight + gap);
          const pct = tier.biomass === null ? null : ((tier.biomass / totalBiomass) * 100).toFixed(1);

          return (
            <g key={tier.id} data-testid={`trophic-tier-${tier.id}`}>
              {/* Tier Polygon / Trapezoid Card */}
              <rect
                x={tX}
                y={tY}
                width={tWidth}
                height={tierHeight}
                rx={6}
                fill={tier.bgColor}
                stroke={tier.borderColor}
                strokeWidth={1.5}
                strokeDasharray={tier.biomass === null ? '4 3' : undefined}
              />

              {/* Tier Label */}
              <text
                x={W / 2}
                y={tY + 20}
                fontSize="11"
                fontWeight="600"
                textAnchor="middle"
                fill="#f8fafc"
                fontFamily="Inter, sans-serif"
              >
                {tier.shortLabel}
              </text>

              {/* Tier Biomass Metrics */}
              <text
                x={W / 2}
                y={tY + 38}
                fontSize="10"
                textAnchor="middle"
                fill="#cbd5e1"
                fontFamily="Inter, sans-serif"
              >
                {tier.biomass === null ? 'not reported' : `${tier.biomass.toFixed(1)} EU (${pct}%)`}
              </text>

              {/* Lindeman Efficiency Badge between tiers */}
              {idx === 0 && (
                <g data-testid="efficiency-badge-l23">
                  <path
                    d={`M ${W / 2 + 100} ${tY + tierHeight} L ${W / 2 + 100} ${tY + tierHeight + gap} `}
                    stroke="#f59e0b"
                    strokeWidth={1.5}
                    markerEnd="url(#arrow)"
                  />
                  <rect
                    x={W / 2 + 115}
                    y={tY + tierHeight - 12}
                    width={72}
                    height={22}
                    rx={4}
                    fill="rgba(120, 53, 15, 0.4)"
                    stroke="#f59e0b"
                    strokeWidth={1}
                  />
                  <text
                    x={W / 2 + 151}
                    y={tY + tierHeight + 3}
                    fontSize="9"
                    fontWeight="600"
                    fill="#fef08a"
                    textAnchor="middle"
                  >
                    λ₂₃: {pctText(l23)}
                  </text>
                </g>
              )}

              {idx === 1 && (
                <g data-testid="efficiency-badge-l12">
                  <rect
                    x={W / 2 + 115}
                    y={tY + tierHeight - 12}
                    width={72}
                    height={22}
                    rx={4}
                    fill="rgba(6, 78, 59, 0.4)"
                    stroke="#10b981"
                    strokeWidth={1}
                  />
                  <text
                    x={W / 2 + 151}
                    y={tY + tierHeight + 3}
                    fontSize="9"
                    fontWeight="600"
                    fill="#a7f3d0"
                    textAnchor="middle"
                  >
                    λ₁₂: {pctText(l12)}
                  </text>
                </g>
              )}
            </g>
          );
        })}
      </svg>

      {/* Summary Footnote */}
      <div style={{ display: 'flex', justifyContent: 'space-around', width: '100%', fontSize: '11px', color: '#94a3b8', marginTop: '6px', borderTop: '1px solid rgba(255,255,255,0.06)', paddingTop: '6px' }}>
        <div>
          Primary Herbivore Transfer: <span style={{ color: '#10b981', fontWeight: 600 }}>{pctText(l12)}</span>
        </div>
        <div>
          Secondary Carnivore Transfer: <span style={{ color: '#f59e0b', fontWeight: 600 }}>{pctText(l23)}</span>
        </div>
      </div>
    </div>
  );
};
