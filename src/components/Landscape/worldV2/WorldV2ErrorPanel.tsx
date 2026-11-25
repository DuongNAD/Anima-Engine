import React from 'react';
import { TileIntegrityError, WorldV2LoadError } from './types';

export interface WorldV2ErrorPanelProps {
  error: unknown;
  onRetry?: () => void;
}

/**
 * Loud, non-dismissible World v2 load/verification failure screen (`WV2-6`, `§7.1`).
 * Never provides or triggers a silent fallback to legacy procedural v1 generation.
 */
export const WorldV2ErrorPanel: React.FC<WorldV2ErrorPanelProps> = ({ error, onRetry }) => {
  const message = error instanceof Error ? error.message : String(error);
  const isIntegrity = error instanceof TileIntegrityError;
  const code = error instanceof WorldV2LoadError ? error.code : isIntegrity ? 'INTEGRITY' : 'UNKNOWN';

  return (
    <div
      data-testid="world-v2-load-error"
      role="alert"
      style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        background: '#14080a',
        color: '#fecaca',
        fontFamily: 'sans-serif',
        padding: 24,
        textAlign: 'center',
        gap: 12,
      }}
    >
      <div style={{ fontSize: 18, fontWeight: 700, color: '#f87171' }}>
        World v2 Failed to Load ({code}) — No Procedural Fallback
      </div>
      <div style={{ maxWidth: 680, fontSize: 13, lineHeight: 1.5, color: '#fee2e2' }}>
        {message}
      </div>
      {isIntegrity ? (
        <div
          style={{
            maxWidth: 680,
            fontSize: 12,
            fontFamily: 'monospace',
            background: 'rgba(0,0,0,0.45)',
            padding: '10px 14px',
            borderRadius: 6,
            border: '1px solid #7f1d1d',
            textAlign: 'left',
          }}
        >
          <div>asset: {error.tilePath}</div>
          <div>expected: {error.expectedSha256}</div>
          <div>actual: {error.actualSha256}</div>
        </div>
      ) : null}
      <div style={{ maxWidth: 620, fontSize: 12, color: '#fca5a5', opacity: 0.9 }}>
        Stage verified C01 tiles into <code>public/world-v2/</code> via{' '}
        <code>npm run fetch:world-v2-tiles</code> and reload.
      </div>
      {onRetry ? (
        <button
          type="button"
          data-testid="world-v2-retry-btn"
          onClick={onRetry}
          style={{
            marginTop: 4,
            padding: '6px 14px',
            borderRadius: 6,
            border: '1px solid #f87171',
            background: 'rgba(185,28,28,0.35)',
            color: '#fff',
            cursor: 'pointer',
            fontSize: 13,
          }}
        >
          Retry World v2 Load
        </button>
      ) : null}
    </div>
  );
};

export default WorldV2ErrorPanel;
