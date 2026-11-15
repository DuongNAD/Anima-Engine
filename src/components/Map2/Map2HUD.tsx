import React, { useEffect, useMemo, useRef, useState } from 'react';
import { sampleMap2Terrain } from './utils/map2TerrainGen';
import { Map2Biome, BIOME_METADATA } from './utils/map2BiomeConfig';
import { CameraMode, CANONICAL_CINE_KEYFRAMES } from './utils/map2Physics';
import { Map2CameraView } from './Map2CameraRig';

// -------------------------------------------------------------------------------------------------
// Map 2 Interactive Explorer HUD & Diagnostic Control Panel
// Components:
// 1. Biome Banner (Real-time telemetry, Vietnamese/English names, climate, elevation)
// 2. 2D Canvas Compass Ribbon (rAF heading ribbon with degree bearing)
// 3. 2D Canvas Minimap (Terrain hillshading, live player cone, click-to-teleport, biome legend)
// 4. Glassmorphism Control Drawer (Mode pills, 24h clock, weather presets, landmark shortcuts)
// -------------------------------------------------------------------------------------------------

export interface Map2HUDProps {
  viewRef: React.MutableRefObject<Map2CameraView>;
  cameraMode: CameraMode;
  onCameraModeChange: (mode: CameraMode) => void;
  timeOfDay: number;
  onTimeOfDayChange: (time: number) => void;
  isPlayingTime: boolean;
  onTogglePlayTime: () => void;
  seaLevel: number;
  onTeleport: (x: number, z: number) => void;
  showVegetation?: boolean;
  onToggleVegetation?: () => void;
  showCrystals?: boolean;
  onToggleCrystals?: () => void;
  showWildlife?: boolean;
  onToggleWildlife?: () => void;
  isStandalone?: boolean;
  quality?: 'high' | 'low';
  onQualityChange?: (quality: 'high' | 'low') => void;
  worldSize?: number;
}

const MINIMAP_SIZE = 192;
const COMPASS_WIDTH = 288;
const COMPASS_HEIGHT = 32;
const COMPASS_PX_PER_DEG = 2.2;

const CARDINALS = [
  { deg: 0, label: 'N' },
  { deg: 45, label: 'NE' },
  { deg: 90, label: 'E' },
  { deg: 135, label: 'SE' },
  { deg: 180, label: 'S' },
  { deg: 225, label: 'SW' },
  { deg: 270, label: 'W' },
  { deg: 315, label: 'NW' },
];

function wrapDeg(d: number): number {
  let x = ((d + 180) % 360) - 180;
  if (x <= -180) x += 360;
  return x;
}

function hexToRgb(hex: string): [number, number, number] {
  const clean = hex.replace('#', '');
  if (clean.length === 3) {
    return [
      parseInt(clean[0] + clean[0], 16),
      parseInt(clean[1] + clean[1], 16),
      parseInt(clean[2] + clean[2], 16),
    ];
  }
  return [
    parseInt(clean.substring(0, 2), 16),
    parseInt(clean.substring(2, 4), 16),
    parseInt(clean.substring(4, 6), 16),
  ];
}

export const Map2HUD: React.FC<Map2HUDProps> = ({
  viewRef,
  cameraMode,
  onCameraModeChange,
  timeOfDay,
  onTimeOfDayChange,
  isPlayingTime,
  onTogglePlayTime,
  seaLevel,
  onTeleport,
  showVegetation = true,
  onToggleVegetation,
  showCrystals = true,
  onToggleCrystals,
  showWildlife = true,
  onToggleWildlife,
  isStandalone = false,
  quality = 'high',
  onQualityChange,
  worldSize = 200,
}) => {
  const [drawerOpen, setDrawerOpen] = useState<boolean>(false);
  const [showLegend, setShowLegend] = useState<boolean>(false);
  const [activeWeather, setActiveWeather] = useState<'clear' | 'rain' | 'snow' | 'fog' | 'thunder'>('clear');
  const [hudTelemetry, setHudTelemetry] = useState<{
    x: number;
    y: number;
    z: number;
    biomeId: Map2Biome;
    fps: number;
    speed: number;
    elevation: number;
    temperature: number;
    moisture: number;
    steepness: number;
  }>({
    x: 0,
    y: 15,
    z: 0,
    biomeId: Map2Biome.TEMPERATE_PLAINS,
    fps: 60,
    speed: 0,
    elevation: 15,
    temperature: 0.5,
    moisture: 0.5,
    steepness: 0,
  });

  const compassCanvasRef = useRef<HTMLCanvasElement>(null);
  const minimapCanvasRef = useRef<HTMLCanvasElement>(null);

  // Periodic Telemetry Sample (10Hz) to keep DOM updates lightweight while 3D runs at 60+ FPS
  useEffect(() => {
    const timer = setInterval(() => {
      const v = viewRef.current;
      if (!v) return;
      const sample = sampleMap2Terrain(v.camX, v.camZ);
      setHudTelemetry({
        x: v.camX,
        y: v.camY,
        z: v.camZ,
        biomeId: sample.biomeId,
        fps: Math.round(v.fps || 60),
        speed: v.speed || 0,
        elevation: sample.height,
        temperature: sample.temperature,
        moisture: sample.moisture,
        steepness: sample.steepness,
      });
    }, 100);
    return () => clearInterval(timer);
  }, [viewRef]);

  // Pre-render Minimap Terrain & Biome Hillshaded Texture
  const minimapImageData = useMemo(() => {
    const data = new Uint8ClampedArray(MINIMAP_SIZE * MINIMAP_SIZE * 4);

    for (let my = 0; my < MINIMAP_SIZE; my++) {
      for (let mx = 0; mx < MINIMAP_SIZE; mx++) {
        const wx = ((mx / MINIMAP_SIZE) - 0.5) * worldSize;
        const wz = ((my / MINIMAP_SIZE) - 0.5) * worldSize;
        const sample = sampleMap2Terrain(wx, wz);

        const meta = BIOME_METADATA[sample.biomeId];
        let [r, g, b] = hexToRgb(meta ? meta.color : '#3b82f6');

        // Hillshade relief
        const shade = sample.height < seaLevel ? 0.85 : 0.7 + (sample.height / 45) * 0.45;
        r = Math.min(255, r * shade);
        g = Math.min(255, g * shade);
        b = Math.min(255, b * shade);

        // Water tinting
        if (sample.isWater) {
          if (sample.temperature < 0.2) {
            // Frozen ice lake
            r = r * 0.2 + 214 * 0.8;
            g = g * 0.2 + 232 * 0.8;
            b = b * 0.2 + 242 * 0.8;
          } else {
            // Deep water blue
            r = r * 0.3 + 2 * 0.7;
            g = g * 0.3 + 132 * 0.7;
            b = b * 0.3 + 199 * 0.7;
          }
        }

        const idx = (my * MINIMAP_SIZE + mx) * 4;
        data[idx] = Math.floor(r);
        data[idx + 1] = Math.floor(g);
        data[idx + 2] = Math.floor(b);
        data[idx + 3] = 255;
      }
    }
    return data;
  }, [worldSize, seaLevel]);

  // Compass Ribbon 2D Canvas rAF Render Loop
  useEffect(() => {
    const canvas = compassCanvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    let animId: number;
    const cx = COMPASS_WIDTH / 2;

    const renderCompass = () => {
      const v = viewRef.current;
      const bearing = v ? v.bearing : 0;

      ctx.clearRect(0, 0, COMPASS_WIDTH, COMPASS_HEIGHT);

      // Background subtle gradient
      ctx.fillStyle = 'rgba(15, 23, 42, 0.75)';
      ctx.fillRect(0, 0, COMPASS_WIDTH, COMPASS_HEIGHT);

      // Degree ticks every 15 deg
      ctx.strokeStyle = 'rgba(226, 232, 240, 0.45)';
      ctx.lineWidth = 1;

      for (let d = 0; d < 360; d += 15) {
        const dx = wrapDeg(d - bearing);
        if (Math.abs(dx) > COMPASS_WIDTH / (2 * COMPASS_PX_PER_DEG)) continue;
        const x = cx + dx * COMPASS_PX_PER_DEG;
        const isMajor = d % 45 === 0;

        ctx.beginPath();
        ctx.moveTo(x, COMPASS_HEIGHT - 6);
        ctx.lineTo(x, COMPASS_HEIGHT - (isMajor ? 16 : 10));
        ctx.stroke();
      }

      // Cardinal Labels
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      for (let i = 0; i < CARDINALS.length; i++) {
        const card = CARDINALS[i];
        const dx = wrapDeg(card.deg - bearing);
        if (Math.abs(dx) > COMPASS_WIDTH / (2 * COMPASS_PX_PER_DEG)) continue;
        const x = cx + dx * COMPASS_PX_PER_DEG;

        const isNorth = card.label === 'N';
        const isMajor = card.label.length === 1;

        ctx.fillStyle = isNorth ? '#f87171' : isMajor ? '#f8fafc' : 'rgba(203, 213, 225, 0.75)';
        ctx.font = `${isMajor ? 'bold 12px' : '10px'} system-ui, sans-serif`;
        ctx.fillText(card.label, x, 10);
      }

      // Center Pointer Needle
      ctx.fillStyle = '#38bdf8';
      ctx.beginPath();
      ctx.moveTo(cx, COMPASS_HEIGHT - 4);
      ctx.lineTo(cx - 5, COMPASS_HEIGHT);
      ctx.lineTo(cx + 5, COMPASS_HEIGHT);
      ctx.closePath();
      ctx.fill();

      animId = requestAnimationFrame(renderCompass);
    };

    animId = requestAnimationFrame(renderCompass);
    return () => cancelAnimationFrame(animId);
  }, [viewRef]);

  // Minimap 2D Canvas rAF Render Loop
  useEffect(() => {
    const canvas = minimapCanvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    let animId: number;

    const img = ctx.createImageData(MINIMAP_SIZE, MINIMAP_SIZE);
    img.data.set(minimapImageData);

    const renderMinimap = () => {
      ctx.putImageData(img, 0, 0);

      const v = viewRef.current;
      if (v) {
        // World coordinates to minimap pixel
        const px = ((v.camX + worldSize / 2) / worldSize) * MINIMAP_SIZE;
        const py = ((v.camZ + worldSize / 2) / worldSize) * MINIMAP_SIZE;

        const clampedPX = Math.max(2, Math.min(MINIMAP_SIZE - 2, px));
        const clampedPY = Math.max(2, Math.min(MINIMAP_SIZE - 2, py));

        // Heading Cone / Field of View Frustum
        const headingRad = (v.bearing * Math.PI) / 180;
        const dirX = Math.sin(headingRad);
        const dirZ = -Math.cos(headingRad);

        ctx.fillStyle = 'rgba(56, 189, 248, 0.25)';
        ctx.beginPath();
        ctx.moveTo(clampedPX, clampedPY);
        ctx.arc(clampedPX, clampedPY, 22, headingRad - Math.PI / 2 - 0.45, headingRad - Math.PI / 2 + 0.45);
        ctx.closePath();
        ctx.fill();

        // Player Center Indicator Dot
        ctx.fillStyle = '#ef4444';
        ctx.beginPath();
        ctx.arc(clampedPX, clampedPY, 4, 0, Math.PI * 2);
        ctx.fill();

        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = 1.5;
        ctx.stroke();

        // Direction indicator line
        ctx.strokeStyle = '#38bdf8';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(clampedPX, clampedPY);
        ctx.lineTo(clampedPX + dirX * 12, clampedPY + dirZ * 12);
        ctx.stroke();
      }

      animId = requestAnimationFrame(renderMinimap);
    };

    animId = requestAnimationFrame(renderMinimap);
    return () => cancelAnimationFrame(animId);
  }, [minimapImageData, viewRef, worldSize]);

  // Handle Minimap Click-to-Teleport
  const handleMinimapClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = minimapCanvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const u = (e.clientX - rect.left) / rect.width;
    const v = (e.clientY - rect.top) / rect.height;

    const wx = (u - 0.5) * worldSize;
    const wz = (v - 0.5) * worldSize;
    onTeleport(wx, wz);
  };

  const currentBiomeMeta = BIOME_METADATA[hudTelemetry.biomeId] ?? BIOME_METADATA[Map2Biome.TEMPERATE_PLAINS];

  return (
    <div
      data-testid="map2-hud-container"
      style={{
        position: 'absolute',
        inset: 0,
        pointerEvents: 'none',
        zIndex: 40,
        fontFamily: 'system-ui, -apple-system, sans-serif',
        userSelect: 'none',
      }}
    >
      {/* --------------------------------------------------------------------------------------- */}
      {/* TOP-CENTER: COMPASS RIBBON                                                              */}
      {/* --------------------------------------------------------------------------------------- */}
      <div
        data-testid="map2-compass-ribbon"
        style={{
          position: 'absolute',
          top: 14,
          left: '50%',
          transform: 'translateX(-50%)',
          borderRadius: 8,
          overflow: 'hidden',
          border: '1px solid rgba(56, 189, 248, 0.4)',
          boxShadow: '0 4px 16px rgba(0, 0, 0, 0.6)',
          backdropFilter: 'blur(8px)',
        }}
      >
        <canvas ref={compassCanvasRef} width={COMPASS_WIDTH} height={COMPASS_HEIGHT} style={{ display: 'block' }} />
      </div>

      {/* --------------------------------------------------------------------------------------- */}
      {/* TOP-LEFT: BIOME BANNER & REAL-TIME TELEMETRY                                            */}
      {/* --------------------------------------------------------------------------------------- */}
      <div
        data-testid="map2-biome-banner"
        style={{
          position: 'absolute',
          top: 14,
          left: 14,
          display: 'flex',
          flexDirection: 'column',
          gap: 6,
          padding: '10px 14px',
          borderRadius: 10,
          background: 'rgba(15, 23, 42, 0.85)',
          backdropFilter: 'blur(12px)',
          border: '1px solid rgba(56, 189, 248, 0.35)',
          color: '#f8fafc',
          boxShadow: '0 8px 24px rgba(0, 0, 0, 0.5)',
          maxWidth: 380,
          pointerEvents: 'auto',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <span style={{ fontSize: 20 }}>{currentBiomeMeta.emoji}</span>
            <div>
              <div style={{ color: '#38bdf8', fontWeight: 'bold', fontSize: 14 }}>
                {currentBiomeMeta.nameVi}
              </div>
              <div style={{ color: '#94a3b8', fontSize: 11 }}>{currentBiomeMeta.nameEn}</div>
            </div>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
            {isStandalone && (
              <span
                style={{
                  padding: '2px 6px',
                  borderRadius: 4,
                  fontSize: 10,
                  fontWeight: 700,
                  background: '#0284c7',
                  color: '#fff',
                }}
              >
                STANDALONE
              </span>
            )}
            <span
              style={{
                padding: '2px 8px',
                borderRadius: 4,
                fontSize: 10,
                fontWeight: 600,
                textTransform: 'uppercase',
                background: cameraMode === 'walk' ? '#059669' : cameraMode === 'cine' ? '#7c3aed' : '#0284c7',
                color: '#fff',
              }}
            >
              {cameraMode}
            </span>
          </div>
        </div>

        {/* Climate & Telemetry Grid */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(3, 1fr)',
            gap: 6,
            marginTop: 4,
            padding: '6px 8px',
            background: 'rgba(30, 41, 59, 0.65)',
            borderRadius: 6,
            fontSize: 11,
          }}
        >
          <div>
            <span style={{ color: '#94a3b8' }}>Cao độ:</span>{' '}
            <strong style={{ color: '#e2e8f0' }}>{hudTelemetry.elevation.toFixed(1)}m</strong>
          </div>
          <div>
            <span style={{ color: '#94a3b8' }}>Nhiệt độ:</span>{' '}
            <strong style={{ color: '#fca5a5' }}>{(hudTelemetry.temperature * 45 - 15).toFixed(0)}°C</strong>
          </div>
          <div>
            <span style={{ color: '#94a3b8' }}>Độ ẩm:</span>{' '}
            <strong style={{ color: '#7dd3fc' }}>{(hudTelemetry.moisture * 100).toFixed(0)}%</strong>
          </div>
          <div>
            <span style={{ color: '#94a3b8' }}>Độ dốc:</span>{' '}
            <strong style={{ color: '#fed7aa' }}>{(hudTelemetry.steepness * 90).toFixed(0)}°</strong>
          </div>
          <div>
            <span style={{ color: '#94a3b8' }}>Tốc độ:</span>{' '}
            <strong style={{ color: '#a7f3d0' }}>{hudTelemetry.speed.toFixed(1)}m/s</strong>
          </div>
          <div>
            <span style={{ color: '#94a3b8' }}>FPS:</span>{' '}
            <strong
              style={{
                color: hudTelemetry.fps >= 50 ? '#4ade80' : hudTelemetry.fps >= 30 ? '#facc15' : '#f87171',
              }}
            >
              {hudTelemetry.fps}
            </strong>
          </div>
        </div>

        {/* Coordinates */}
        <div style={{ fontSize: 10, color: '#64748b', display: 'flex', justifyContent: 'space-between' }}>
          <span>Tọa độ: X: {hudTelemetry.x.toFixed(1)} | Y: {hudTelemetry.y.toFixed(1)} | Z: {hudTelemetry.z.toFixed(1)}</span>
        </div>
      </div>

      {/* --------------------------------------------------------------------------------------- */}
      {/* BOTTOM-RIGHT: MINIMAP WIDGET WITH CLICK-TO-TELEPORT                                     */}
      {/* --------------------------------------------------------------------------------------- */}
      <div
        data-testid="map2-minimap-widget"
        style={{
          position: 'absolute',
          bottom: 16,
          right: 16,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'flex-end',
          gap: 6,
          pointerEvents: 'auto',
        }}
      >
        <div
          style={{
            position: 'relative',
            borderRadius: 10,
            overflow: 'hidden',
            border: '2px solid rgba(56, 189, 248, 0.45)',
            boxShadow: '0 8px 24px rgba(0, 0, 0, 0.6)',
            cursor: 'crosshair',
          }}
          title="Nhấp để dịch chuyển tức thời (Click to teleport)"
        >
          <canvas
            ref={minimapCanvasRef}
            width={MINIMAP_SIZE}
            height={MINIMAP_SIZE}
            onClick={handleMinimapClick}
            style={{ display: 'block' }}
          />
          <div
            style={{
              position: 'absolute',
              bottom: 4,
              right: 6,
              fontSize: 9,
              color: 'rgba(255, 255, 255, 0.8)',
              background: 'rgba(0, 0, 0, 0.5)',
              padding: '1px 4px',
              borderRadius: 3,
            }}
          >
            {worldSize}x{worldSize}m
          </div>
        </div>

        <div style={{ display: 'flex', gap: 6 }}>
          <button
            onClick={() => setShowLegend(!showLegend)}
            style={{
              background: showLegend ? '#0284c7' : 'rgba(15, 23, 42, 0.85)',
              border: '1px solid rgba(56, 189, 248, 0.35)',
              color: '#fff',
              padding: '4px 8px',
              borderRadius: 6,
              fontSize: 11,
              cursor: 'pointer',
              boxShadow: '0 2px 8px rgba(0,0,0,0.4)',
            }}
          >
            🗺️ {showLegend ? 'Ẩn chú giải' : 'Hiện chú giải'}
          </button>
          <button
            onClick={() => onTeleport(0, 0)}
            style={{
              background: 'rgba(15, 23, 42, 0.85)',
              border: '1px solid rgba(56, 189, 248, 0.35)',
              color: '#38bdf8',
              padding: '4px 8px',
              borderRadius: 6,
              fontSize: 11,
              cursor: 'pointer',
              boxShadow: '0 2px 8px rgba(0,0,0,0.4)',
            }}
          >
            📍 Về Spawn
          </button>
        </div>

        {/* Expandable Biome Legend */}
        {showLegend && (
          <div
            data-testid="map2-biome-legend"
            style={{
              background: 'rgba(15, 23, 42, 0.92)',
              backdropFilter: 'blur(12px)',
              border: '1px solid rgba(56, 189, 248, 0.35)',
              padding: 10,
              borderRadius: 8,
              fontSize: 10,
              color: '#f8fafc',
              display: 'flex',
              flexDirection: 'column',
              gap: 4,
              maxWidth: 220,
              boxShadow: '0 8px 24px rgba(0, 0, 0, 0.6)',
            }}
          >
            <div style={{ fontWeight: 'bold', color: '#38bdf8', borderBottom: '1px solid rgba(255,255,255,0.1)', paddingBottom: 3 }}>
              8 Sinh Cảnh Bản Đồ 2
            </div>
            {Object.values(BIOME_METADATA).map((b) => (
              <div key={b.id} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                <span style={{ width: 10, height: 10, borderRadius: 2, background: b.color, display: 'inline-block' }} />
                <span>{b.emoji} {b.nameVi}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* --------------------------------------------------------------------------------------- */}
      {/* BOTTOM-LEFT / DRAWER TOGGLE: GLASSMORPHISM CONTROL DRAWER                               */}
      {/* --------------------------------------------------------------------------------------- */}
      <div
        data-testid="map2-control-drawer-toggle"
        style={{
          position: 'absolute',
          bottom: 16,
          left: 16,
          pointerEvents: 'auto',
        }}
      >
        <button
          onClick={() => setDrawerOpen(!drawerOpen)}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            padding: '8px 14px',
            borderRadius: 8,
            background: drawerOpen ? '#0284c7' : 'rgba(15, 23, 42, 0.88)',
            border: '1px solid rgba(56, 189, 248, 0.4)',
            color: '#fff',
            fontSize: 13,
            fontWeight: 600,
            cursor: 'pointer',
            boxShadow: '0 4px 16px rgba(0, 0, 0, 0.5)',
            backdropFilter: 'blur(10px)',
          }}
        >
          <span>🎛️</span>
          <span>{drawerOpen ? 'Đóng Bảng Điều Khiển' : 'Bảng Điều Khiển Khám Phá'}</span>
        </button>
      </div>

      {/* Glassmorphism Control Drawer Panel */}
      {drawerOpen && (
        <div
          data-testid="map2-control-drawer-panel"
          style={{
            position: 'absolute',
            bottom: 60,
            left: 16,
            width: 380,
            maxHeight: 'calc(100vh - 120px)',
            overflowY: 'auto',
            background: 'rgba(15, 23, 42, 0.92)',
            backdropFilter: 'blur(16px)',
            border: '1px solid rgba(56, 189, 248, 0.4)',
            borderRadius: 12,
            padding: 16,
            color: '#f8fafc',
            boxShadow: '0 12px 32px rgba(0, 0, 0, 0.7)',
            display: 'flex',
            flexDirection: 'column',
            gap: 12,
            pointerEvents: 'auto',
          }}
        >
          {/* Section 1: Camera Mode Selector Pills */}
          <div>
            <div style={{ fontSize: 12, fontWeight: 'bold', color: '#38bdf8', marginBottom: 6 }}>
              📷 Chế độ Camera (Camera Mode)
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 6 }}>
              {(
                [
                  { id: 'orbit', label: '🌀 Quỹ đạo', name: 'Orbit' },
                  { id: 'fly', label: '🕊️ Bay', name: 'Fly' },
                  { id: 'walk', label: '🚶 Đi bộ', name: 'Walk' },
                  { id: 'cine', label: '🎬 Cine', name: 'Cine' },
                ] as const
              ).map((m) => (
                <button
                  key={m.id}
                  onClick={() => onCameraModeChange(m.id)}
                  style={{
                    padding: '6px 4px',
                    borderRadius: 6,
                    border: cameraMode === m.id ? '1px solid #38bdf8' : '1px solid rgba(148, 163, 184, 0.2)',
                    background: cameraMode === m.id ? '#0284c7' : 'rgba(30, 41, 59, 0.7)',
                    color: '#fff',
                    fontSize: 11,
                    fontWeight: 600,
                    cursor: 'pointer',
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    gap: 2,
                  }}
                >
                  <span>{m.label}</span>
                </button>
              ))}
            </div>
          </div>

          {/* Section 2: 24-Hour Day-Night Cycle & Presets */}
          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 6 }}>
              <span style={{ fontSize: 12, fontWeight: 'bold', color: '#38bdf8' }}>
                ☀️ Chu kỳ ngày đêm (24h Time of Day)
              </span>
              <button
                onClick={onTogglePlayTime}
                style={{
                  background: isPlayingTime ? '#e11d48' : '#0284c7',
                  border: 'none',
                  color: '#fff',
                  padding: '2px 8px',
                  borderRadius: 4,
                  cursor: 'pointer',
                  fontSize: 10,
                  fontWeight: 600,
                }}
              >
                {isPlayingTime ? '⏸ Tạm dừng' : '▶ Chạy 24h'}
              </button>
            </div>
            <input
              type="range"
              min="0"
              max="24"
              step="0.1"
              value={timeOfDay}
              onChange={(e) => onTimeOfDayChange(parseFloat(e.target.value))}
              style={{ width: '100%', cursor: 'pointer', accentColor: '#38bdf8' }}
            />
            {/* Quick Presets */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 4, marginTop: 4 }}>
              {[
                { time: 0, label: '🌙 0h Đêm' },
                { time: 6, label: '🌅 6h Bình minh' },
                { time: 12, label: '☀️ 12h Trưa' },
                { time: 18, label: '🌇 18h Hoàng hôn' },
              ].map((p) => (
                <button
                  key={p.time}
                  onClick={() => onTimeOfDayChange(p.time)}
                  style={{
                    padding: '3px 4px',
                    borderRadius: 4,
                    background: Math.abs(timeOfDay - p.time) < 1.0 ? '#0369a1' : 'rgba(30, 41, 59, 0.6)',
                    border: '1px solid rgba(148, 163, 184, 0.2)',
                    color: '#e2e8f0',
                    fontSize: 10,
                    cursor: 'pointer',
                  }}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>

          {/* Section 3: Weather Presets */}
          <div>
            <div style={{ fontSize: 12, fontWeight: 'bold', color: '#38bdf8', marginBottom: 6 }}>
              🌦️ Thời tiết & Khí quyển (Weather)
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: 4 }}>
              {(
                [
                  { id: 'clear', label: '☀️ Quang' },
                  { id: 'rain', label: '🌧️ Mưa' },
                  { id: 'snow', label: '❄️ Tuyết' },
                  { id: 'fog', label: '🌫️ Mù' },
                  { id: 'thunder', label: '⚡ Giông' },
                ] as const
              ).map((w) => (
                <button
                  key={w.id}
                  onClick={() => setActiveWeather(w.id)}
                  style={{
                    padding: '4px 2px',
                    borderRadius: 4,
                    background: activeWeather === w.id ? '#0284c7' : 'rgba(30, 41, 59, 0.6)',
                    border: '1px solid rgba(148, 163, 184, 0.2)',
                    color: '#fff',
                    fontSize: 10,
                    cursor: 'pointer',
                  }}
                >
                  {w.label}
                </button>
              ))}
            </div>
          </div>

          {/* Section 4: 8 Landmark Biome Shortcuts */}
          <div>
            <div style={{ fontSize: 12, fontWeight: 'bold', color: '#38bdf8', marginBottom: 6 }}>
              📍 Dịch chuyển tức thời đến 8 Sinh cảnh
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 4 }}>
              {CANONICAL_CINE_KEYFRAMES.map((kf, i) => (
                <button
                  key={i}
                  onClick={() => onTeleport(kf.position[0], kf.position[2])}
                  style={{
                    padding: '4px 6px',
                    borderRadius: 4,
                    background: 'rgba(30, 41, 59, 0.6)',
                    border: '1px solid rgba(56, 189, 248, 0.2)',
                    color: '#e2e8f0',
                    fontSize: 10,
                    textAlign: 'left',
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 4,
                  }}
                >
                  <span>{BIOME_METADATA[kf.biome]?.emoji}</span>
                  <span style={{ whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                    {kf.nameVi}
                  </span>
                </button>
              ))}
            </div>
          </div>

          {/* Section 5: Layer Toggles & Quality */}
          <div>
            <div style={{ fontSize: 12, fontWeight: 'bold', color: '#38bdf8', marginBottom: 6 }}>
              🌿 Lớp hiển thị & Hiệu năng
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 4 }}>
              {onToggleVegetation && (
                <button
                  onClick={onToggleVegetation}
                  style={{
                    padding: '4px',
                    borderRadius: 4,
                    background: showVegetation ? '#15803d' : 'rgba(30, 41, 59, 0.6)',
                    border: '1px solid rgba(148, 163, 184, 0.2)',
                    color: '#fff',
                    fontSize: 10,
                    cursor: 'pointer',
                  }}
                >
                  🌿 Thực vật ({showVegetation ? 'Bật' : 'Tắt'})
                </button>
              )}
              {onToggleCrystals && (
                <button
                  onClick={onToggleCrystals}
                  style={{
                    padding: '4px',
                    borderRadius: 4,
                    background: showCrystals ? '#7e22ce' : 'rgba(30, 41, 59, 0.6)',
                    border: '1px solid rgba(148, 163, 184, 0.2)',
                    color: '#fff',
                    fontSize: 10,
                    cursor: 'pointer',
                  }}
                >
                  🔮 Tinh thể ({showCrystals ? 'Bật' : 'Tắt'})
                </button>
              )}
              {onToggleWildlife && (
                <button
                  onClick={onToggleWildlife}
                  style={{
                    padding: '4px',
                    borderRadius: 4,
                    background: showWildlife ? '#0284c7' : 'rgba(30, 41, 59, 0.6)',
                    border: '1px solid rgba(148, 163, 184, 0.2)',
                    color: '#fff',
                    fontSize: 10,
                    cursor: 'pointer',
                  }}
                >
                  🦅 Sinh vật ({showWildlife ? 'Bật' : 'Tắt'})
                </button>
              )}
              {onQualityChange && (
                <button
                  onClick={() => onQualityChange(quality === 'high' ? 'low' : 'high')}
                  style={{
                    padding: '4px',
                    borderRadius: 4,
                    background: quality === 'high' ? '#0369a1' : 'rgba(30, 41, 59, 0.6)',
                    border: '1px solid rgba(148, 163, 184, 0.2)',
                    color: '#fff',
                    fontSize: 10,
                    cursor: 'pointer',
                    gridColumn: 'span 3',
                  }}
                >
                  ⚡ Đồ họa: {quality === 'high' ? 'Cao (High Res)' : 'Tiết kiệm (Low)'}
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default Map2HUD;
