import React from 'react';
import { BANNER_TEXT } from './types';

interface WorldV2BannerProps {
  statusText: string;
  errorText: string | null;
  metaText: string | null;
}

export const WorldV2Banner: React.FC<WorldV2BannerProps> = ({
  statusText,
  errorText,
  metaText,
}) => {
  return (
    <>
      <div
        data-testid="wv2-preview-banner"
        style={{
          position: 'fixed',
          top: 12,
          left: '50%',
          transform: 'translateX(-50%)',
          zIndex: 1000,
          background: 'rgba(120, 53, 15, 0.94)',
          border: '1px solid rgba(251, 191, 36, 0.8)',
          color: '#fef3c7',
          padding: '7px 16px',
          borderRadius: 999,
          fontFamily: 'system-ui, -apple-system, sans-serif',
          fontSize: 13,
          fontWeight: 600,
          letterSpacing: '0.01em',
          boxShadow: '0 6px 20px rgba(0,0,0,0.45)',
          pointerEvents: 'none',
          whiteSpace: 'nowrap',
        }}
      >
        {BANNER_TEXT}
      </div>

      {metaText && !errorText ? (
        <div
          data-testid="wv2-preview-meta"
          style={{
            position: 'fixed',
            bottom: 12,
            left: 12,
            zIndex: 1000,
            background: 'rgba(15, 23, 42, 0.86)',
            border: '1px solid rgba(148, 163, 184, 0.35)',
            color: '#e2e8f0',
            padding: '6px 12px',
            borderRadius: 8,
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
            fontSize: 11,
            pointerEvents: 'none',
          }}
        >
          {metaText}
        </div>
      ) : null}

      {!metaText && !errorText ? (
        <div
          data-testid="wv2-preview-loading"
          style={{
            position: 'fixed',
            inset: 0,
            zIndex: 900,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            background: 'rgba(10, 15, 26, 0.92)',
            color: '#e2e8f0',
            fontFamily: 'system-ui, -apple-system, sans-serif',
            fontSize: 14,
          }}
        >
          {statusText}
        </div>
      ) : null}

      {errorText ? (
        <div
          data-testid="wv2-preview-error"
          role="alert"
          style={{
            position: 'fixed',
            inset: 0,
            zIndex: 1100,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 12,
            padding: 32,
            background: 'rgba(69, 10, 10, 0.96)',
            color: '#fee2e2',
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
            textAlign: 'center',
          }}
        >
          <div style={{ fontSize: 18, fontWeight: 700, color: '#fecaca' }}>
            World v2 Preview Verification Error (no v1 fallback)
          </div>
          <pre
            style={{
              maxWidth: 820,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
              fontSize: 13,
              background: 'rgba(0,0,0,0.35)',
              padding: 16,
              borderRadius: 8,
              border: '1px solid rgba(248, 113, 113, 0.45)',
            }}
          >
            {errorText}
          </pre>
        </div>
      ) : null}
    </>
  );
};
