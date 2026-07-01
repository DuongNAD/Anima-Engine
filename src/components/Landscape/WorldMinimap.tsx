import React, { useEffect, useMemo, useRef, useState } from 'react';
import type { World } from './utils/worldGen';
import { Biome, BIOME_RGB } from './utils/worldGen';

// ---------------------------------------------------------------------------------------
// WorldMinimap — top-down biome map of the SoA world with a live camera marker.
//
// The 3D scene lives inside an R3F <Canvas>; this is a plain HTML/2D-canvas overlay. The two
// are bridged by a shared mutable ref (`viewRef`) that an in-canvas reporter writes to every
// frame, which this component reads in its own rAF loop (no React re-render per frame).
// Clicking the map calls `onTeleport(worldX, worldZ)` to recentre the camera.
// ---------------------------------------------------------------------------------------

/** Live camera state shared between the 3D scene and the HTML overlay. */
export interface CameraView {
  /** Orbit target (look-at point) in world XZ. */
  targetX: number;
  targetZ: number;
  /** Camera position in world XZ (for the heading arrow). */
  camX: number;
  camZ: number;
}

export interface WorldMinimapProps {
  world: World;
  renderSize: number;
  viewRef: React.MutableRefObject<CameraView>;
  onTeleport: (worldX: number, worldZ: number) => void;
  size?: number;
}

const MAP_SIZE = 192;

const LEGEND: Array<{ label: string; biome: Biome }> = [
  { label: 'Ocean', biome: Biome.Ocean },
  { label: 'Lake', biome: Biome.Lake },
  { label: 'River', biome: Biome.River },
  { label: 'Beach', biome: Biome.Beach },
  { label: 'Mangrove', biome: Biome.Mangrove },
  { label: 'Desert', biome: Biome.Desert },
  { label: 'Badlands', biome: Biome.Badlands },
  { label: 'Savanna', biome: Biome.Savanna },
  { label: 'Chaparral', biome: Biome.Chaparral },
  { label: 'Steppe', biome: Biome.Steppe },
  { label: 'Grass', biome: Biome.Grassland },
  { label: 'Shrub', biome: Biome.Shrubland },
  { label: 'Forest', biome: Biome.Forest },
  { label: 'Jungle', biome: Biome.Jungle },
  { label: 'Swamp', biome: Biome.Swamp },
  { label: 'Bog', biome: Biome.Bog },
  { label: 'Taiga', biome: Biome.Taiga },
  { label: 'Tundra', biome: Biome.Tundra },
  { label: 'Alpine', biome: Biome.Alpine },
  { label: 'Rock', biome: Biome.Rock },
  { label: 'Snow', biome: Biome.Snow },
  { label: 'Glacier', biome: Biome.Glacier },
];

export const WorldMinimap: React.FC<WorldMinimapProps> = ({
  world,
  renderSize,
  viewRef,
  onTeleport,
  size = MAP_SIZE,
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [showLegend, setShowLegend] = useState(false);

  // Pre-render the biome map (with a light elevation hillshade for relief) once per world.
  const biomeImage = useMemo(() => {
    const { size: n, biome, elevation, water, seaLevel } = world;
    const data = new Uint8ClampedArray(size * size * 4);
    for (let my = 0; my < size; my++) {
      for (let mx = 0; mx < size; mx++) {
        const gx = Math.min(n - 1, Math.floor((mx / size) * n));
        const gy = Math.min(n - 1, Math.floor((my / size) * n));
        const idx = gy * n + gx;
        let [r, g, b] = BIOME_RGB[biome[idx]] ?? [0, 0, 0];

        // Cheap hillshade: brighten with elevation on land, flat under the sea.
        const e = elevation[idx];
        const shade = e < seaLevel ? 1.0 : 0.72 + 0.5 * (e - seaLevel);
        r *= shade;
        g *= shade;
        b *= shade;

        // Lakes: tint standing-water cells towards a lake blue.
        if (water[idx] > 0) {
          r = r * 0.25 + 40 * 0.75;
          g = g * 0.25 + 120 * 0.75;
          b = b * 0.25 + 170 * 0.75;
        }

        const pi = (my * size + mx) * 4;
        data[pi] = Math.min(255, r);
        data[pi + 1] = Math.min(255, g);
        data[pi + 2] = Math.min(255, b);
        data[pi + 3] = 255;
      }
    }
    return data;
  }, [world, size]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    let active = true;

    // world XZ (terrain spans [-renderSize/2, renderSize/2]) -> minimap pixel.
    const toPx = (world: number) => ((world + renderSize / 2) / renderSize) * size;
    const clamp = (v: number) => Math.max(0, Math.min(size, v));

    const loop = () => {
      if (!active) return;

      if (typeof ctx.createImageData === 'function' && typeof ctx.putImageData === 'function') {
        const img = ctx.createImageData(size, size);
        img.data.set(biomeImage);
        ctx.putImageData(img, 0, 0);
      } else if (typeof ctx.fillRect === 'function') {
        ctx.fillStyle = '#0a3b66';
        ctx.fillRect(0, 0, size, size);
      }

      const view = viewRef.current;
      if (view && typeof ctx.beginPath === 'function') {
        const tx = clamp(toPx(view.targetX));
        const tz = clamp(toPx(view.targetZ));
        const cx = toPx(view.camX);
        const cz = toPx(view.camZ);

        // Heading arrow: from the target toward the camera-look direction.
        const dx = tx - cx;
        const dz = tz - cz;
        const len = Math.hypot(dx, dz) || 1;
        const ux = dx / len;
        const uz = dz / len;
        ctx.strokeStyle = 'rgba(255,255,255,0.85)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(tx, tz);
        ctx.lineTo(tx + ux * 16, tz + uz * 16);
        ctx.stroke();

        // Target marker.
        ctx.fillStyle = '#ff3b3b';
        ctx.beginPath();
        ctx.arc(tx, tz, 4, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = 1.5;
        ctx.stroke();
      }

      requestAnimationFrame(loop);
    };
    loop();
    return () => {
      active = false;
    };
  }, [biomeImage, renderSize, size, viewRef]);

  const handleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const mx = (e.clientX - rect.left) / rect.width;
    const my = (e.clientY - rect.top) / rect.height;
    onTeleport((mx - 0.5) * renderSize, (my - 0.5) * renderSize);
  };

  return (
    <div
      style={{
        position: 'absolute',
        bottom: 20,
        right: 20,
        zIndex: 100,
        borderRadius: 10,
        overflow: 'hidden',
        border: '2px solid rgba(125,211,252,0.35)',
        boxShadow: '0 4px 20px rgba(0,0,0,0.5)',
        background: '#080c18',
        fontFamily: 'sans-serif',
      }}
    >
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          padding: '4px 8px',
          fontSize: 11,
          color: '#cbd5e1',
          background: 'rgba(2,6,23,0.7)',
        }}
      >
        <span>🗺 World Map</span>
        <span
          onClick={() => setShowLegend((v) => !v)}
          style={{ cursor: 'pointer', opacity: 0.8 }}
          title="Toggle legend"
        >
          {showLegend ? '▾' : 'legend ▸'}
        </span>
      </div>

      <div style={{ position: 'relative' }}>
        <canvas
          ref={canvasRef}
          width={size}
          height={size}
          onClick={handleClick}
          style={{ display: 'block', cursor: 'crosshair' }}
        />
        {showLegend && (
          <div
            style={{
              position: 'absolute',
              inset: 0,
              padding: 8,
              display: 'grid',
              gridTemplateColumns: '1fr 1fr',
              gap: '2px 8px',
              alignContent: 'start',
              background: 'rgba(2,6,23,0.72)',
              fontSize: 10,
              color: '#e2e8f0',
            }}
          >
            {LEGEND.map(({ label, biome }) => {
              const [r, g, b] = BIOME_RGB[biome];
              return (
                <div key={label} style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
                  <span
                    style={{
                      width: 10,
                      height: 10,
                      borderRadius: 2,
                      background: `rgb(${r},${g},${b})`,
                      flex: '0 0 auto',
                    }}
                  />
                  {label}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

export default WorldMinimap;
