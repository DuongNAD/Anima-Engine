import React from 'react';
import { RenderSegment } from '../types';

interface SegmentNodeViewerProps {
  segment: RenderSegment;
  level: number;
  visited?: Set<number>;
}

/**
 * Format a value that is *supposed* to be a number.
 *
 * `unknown` rather than `any`: the whole point is that a live IPC payload has been seen to carry a
 * string, `null` or nothing at all in a numeric field, so the parameter must be a type that forces
 * this function to check — which is exactly what it does. `any` said the same thing while also
 * promising the caller that whatever they passed had a `.toFixed`.
 */
const safeToFixed = (val: unknown, fractionDigits = 2) => {
  const num = typeof val === 'number' ? val : parseFloat(String(val));
  return isNaN(num) ? 'N/A' : num.toFixed(fractionDigits);
};

export const SegmentNodeViewer: React.FC<SegmentNodeViewerProps> = ({
  segment,
  level,
  visited = new Set(),
}) => {
  if (visited.has(segment.segment_id)) {
    return null;
  }
  const nextVisited = new Set(visited);
  nextVisited.add(segment.segment_id);

  return (
    <div
      style={{
        marginLeft: `${level * 16}px`,
        borderLeft: '1px dashed rgba(255, 255, 255, 0.15)',
        paddingLeft: '12px',
        margin: '6px 0',
      }}
    >
      <div
        style={{
          padding: '6px 10px',
          backgroundColor: 'rgba(255, 255, 255, 0.03)',
          borderRadius: '8px',
          display: 'inline-block',
          fontSize: '12px',
          border: '1px solid rgba(255, 255, 255, 0.08)',
          fontFamily: "'Inter', -apple-system, sans-serif",
          color: '#cbd5e1',
          backdropFilter: 'blur(8px)',
          WebkitBackdropFilter: 'blur(8px)',
        }}
      >
        <strong className="neon-teal" style={{ marginRight: '10px' }}>Segment #{segment.segment_id}</strong>
        <span style={{ fontSize: '11px', color: '#94a3b8' }}>
          Tọa độ: ({safeToFixed(segment.x, 2)}, {safeToFixed(segment.y, 2)}, {safeToFixed(segment.z, 2)}) | Yaw:{' '}
          {safeToFixed(segment.yaw, 2)} rad | Anchor:{' '}
          {Array.isArray(segment.joint_anchor)
            ? `[${segment.joint_anchor.map((v) => safeToFixed(v, 1)).join(', ')}]`
            : 'N/A'}
        </span>
      </div>
      {segment.children.map((child) => (
        <SegmentNodeViewer key={child.segment_id} segment={child} level={level + 1} visited={nextVisited} />
      ))}
    </div>
  );
};
export default SegmentNodeViewer;
