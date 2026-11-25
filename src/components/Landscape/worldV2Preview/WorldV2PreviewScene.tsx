import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Canvas, useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { loadWorldV2Preview } from './tileLoader';
import { RENDER_SIZE, WaterKind, type WorldV2PreviewGrid } from './types';
import { WorldV2Banner } from './WorldV2Banner';
import { WorldV2TerrainMesh } from './WorldV2TerrainMesh';
import { WorldV2WaterMesh } from './WorldV2WaterMesh';

interface PreviewCameraPose {
  position: [number, number, number];
  target: [number, number, number];
}

interface PreviewViews {
  overview: PreviewCameraPose;
  horizon: PreviewCameraPose;
  coast: PreviewCameraPose;
  peak: PreviewCameraPose;
  river: PreviewCameraPose;
  lake: PreviewCameraPose;
}

declare global {
  interface Window {
    __worldV2Preview?: {
      simWorldIdentity: string;
      mip: number;
      size: number;
      tileCountVerified: number;
      views: PreviewViews;
    };
    __mapCapture?: {
      pose?: PreviewCameraPose | null;
    };
  }
}

function readRequestedMip(): number {
  if (typeof window === 'undefined') return 2;
  const raw = new URLSearchParams(window.location.search).get('wv2Mip');
  const parsed = raw ? Number(raw) : 2;
  return Number.isInteger(parsed) && parsed >= 0 && parsed <= 4 ? parsed : 2;
}

function cellToWorldXZ(col: number, row: number, size: number): [number, number] {
  const half = RENDER_SIZE / 2;
  return [
    -half + (col / (size - 1)) * RENDER_SIZE,
    -half + (row / (size - 1)) * RENDER_SIZE,
  ];
}

function derivePreviewViews(grid: WorldV2PreviewGrid): PreviewViews {
  const { size, heightM, waterKind, waterHeightM, extentM } = grid;
  const scale = RENDER_SIZE / extentM;

  let peakIdx = 0;
  let peakH = -Infinity;
  let coastIdx = -1;
  let bestCoastDist = Infinity;
  let riverIdx = -1;
  let bestRiverH = -Infinity;
  let lakeIdx = -1;
  let bestLakeDist = Infinity;
  let homeIdx = -1;
  let bestHomeDist = Infinity;

  const mid = (size - 1) / 2;
  for (let r = 8; r < size - 8; r += 2) {
    for (let c = 8; c < size - 8; c += 2) {
      const idx = r * size + c;
      const h = heightM[idx];
      const wk = waterKind[idx];
      const dCenter = (r - mid) * (r - mid) + (c - mid) * (c - mid);

      if (h > peakH) {
        peakH = h;
        peakIdx = idx;
      }
      if (wk === WaterKind.None && h > 25 && h < 600 && dCenter < bestHomeDist) {
        bestHomeDist = dCenter;
        homeIdx = idx;
      }
      if (wk === WaterKind.None && h > 1 && h < 40 && dCenter < bestCoastDist) {
        // Check nearby ocean cell
        const nIdx = (r + 2) * size + c;
        const sIdx = (r - 2) * size + c;
        const eIdx = r * size + (c + 2);
        const wIdx = r * size + (c - 2);
        if (
          waterKind[nIdx] === WaterKind.Ocean ||
          waterKind[sIdx] === WaterKind.Ocean ||
          waterKind[eIdx] === WaterKind.Ocean ||
          waterKind[wIdx] === WaterKind.Ocean
        ) {
          bestCoastDist = dCenter;
          coastIdx = idx;
        }
      }
      if (wk === WaterKind.River && h > 80 && h > bestRiverH) {
        bestRiverH = h;
        riverIdx = idx;
      }
      if (
        (wk === WaterKind.FreshwaterLake || wk === WaterKind.EndorheicLake) &&
        dCenter < bestLakeDist
      ) {
        bestLakeDist = dCenter;
        lakeIdx = idx;
      }
    }
  }

  const posOf = (idx: number, yOffset: number): [number, number, number] => {
    const r = Math.floor(idx / size);
    const c = idx % size;
    const [x, z] = cellToWorldXZ(c, r, size);
    const surf = Math.max(heightM[idx], waterKind[idx] ? waterHeightM[idx] : -9999, 0);
    return [x, surf * scale + yOffset, z];
  };

  const peakT = posOf(peakIdx, 0);
  const homeP: [number, number, number] = homeIdx >= 0 ? posOf(homeIdx, 24) : [0, 36, 0];
  const coastT: [number, number, number] = coastIdx >= 0 ? posOf(coastIdx, 0) : [180, 0, 180];
  const riverT: [number, number, number] = riverIdx >= 0 ? posOf(riverIdx, 0) : [0, 20, 0];
  const lakeT: [number, number, number] = lakeIdx >= 0 ? posOf(lakeIdx, 0) : [60, 12, -60];

  return {
    overview: {
      position: [0, RENDER_SIZE * 0.72, RENDER_SIZE * 0.85],
      target: [0, 15, RENDER_SIZE * 0.04],
    },
    horizon: {
      position: [homeP[0], homeP[1], homeP[2]],
      target: [peakT[0], Math.max(homeP[1] - 4, peakT[1] * 0.65), peakT[2]],
    },
    coast: {
      position: [coastT[0] + 68, coastT[1] + 34, coastT[2] + 68],
      target: coastT,
    },
    peak: {
      position: [peakT[0] * 0.65, peakT[1] + 70, peakT[2] * 0.65 + 140],
      target: peakT,
    },
    river: {
      position: [riverT[0] + 75, riverT[1] + 52, riverT[2] + 75],
      target: riverT,
    },
    lake: {
      position: [lakeT[0] + 85, lakeT[1] + 55, lakeT[2] + 85],
      target: lakeT,
    },
  };
}

function attachPreviewSceneHooks(
  scene: THREE.Scene,
  grid: WorldV2PreviewGrid,
  views: PreviewViews,
): void {
  scene.background = new THREE.Color('#9ec5e6');
  window.__worldScene = scene;
  window.__worldV2Preview = {
    simWorldIdentity: grid.simWorldIdentity,
    mip: grid.mip,
    size: grid.size,
    tileCountVerified: grid.tileCountVerified,
    views,
  };
}

interface OrbitState {
  yaw: number;
  pitch: number;
  distance: number;
  target: THREE.Vector3;
}

const PreviewRig: React.FC<{ grid: WorldV2PreviewGrid; views: PreviewViews }> = ({
  grid,
  views,
}) => {
  const { camera, gl, scene } = useThree();
  const orbitRef = useRef<OrbitState>({
    yaw: 0.78,
    pitch: 0.56,
    distance: RENDER_SIZE * 0.95,
    target: new THREE.Vector3(0, 15, 0),
  });
  const draggingRef = useRef<{ active: boolean; x: number; y: number }>({
    active: false,
    x: 0,
    y: 0,
  });

  useEffect(() => {
    attachPreviewSceneHooks(scene, grid, views);
  }, [scene, grid, views]);

  useEffect(() => {
    const dom = gl.domElement;
    const onDown = (e: PointerEvent) => {
      draggingRef.current = { active: true, x: e.clientX, y: e.clientY };
    };
    const onMove = (e: PointerEvent) => {
      if (!draggingRef.current.active) return;
      const dx = e.clientX - draggingRef.current.x;
      const dy = e.clientY - draggingRef.current.y;
      draggingRef.current.x = e.clientX;
      draggingRef.current.y = e.clientY;
      orbitRef.current.yaw -= dx * 0.005;
      orbitRef.current.pitch = Math.max(0.08, Math.min(1.45, orbitRef.current.pitch + dy * 0.005));
    };
    const onUp = () => {
      draggingRef.current.active = false;
    };
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const factor = 1 + Math.sign(e.deltaY) * 0.1;
      orbitRef.current.distance = Math.max(
        40,
        Math.min(RENDER_SIZE * 2.2, orbitRef.current.distance * factor),
      );
    };

    dom.addEventListener('pointerdown', onDown);
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    dom.addEventListener('wheel', onWheel, { passive: false });
    return () => {
      dom.removeEventListener('pointerdown', onDown);
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      dom.removeEventListener('wheel', onWheel);
    };
  }, [gl]);

  useFrame(() => {
    const pinned = window.__mapCapture?.pose;
    if (pinned) {
      camera.position.set(pinned.position[0], pinned.position[1], pinned.position[2]);
      camera.lookAt(pinned.target[0], pinned.target[1], pinned.target[2]);
      camera.updateMatrixWorld();
      return;
    }
    const st = orbitRef.current;
    const cosP = Math.cos(st.pitch);
    camera.position.set(
      st.target.x + st.distance * cosP * Math.cos(st.yaw),
      st.target.y + st.distance * Math.sin(st.pitch),
      st.target.z + st.distance * cosP * Math.sin(st.yaw),
    );
    camera.lookAt(st.target);
    camera.updateMatrixWorld();
  });

  return null;
};

export const WorldV2PreviewScene: React.FC = () => {
  const mip = useMemo(() => readRequestedMip(), []);
  const [grid, setGrid] = useState<WorldV2PreviewGrid | null>(null);
  const [statusText, setStatusText] = useState('Initializing World v2 Preview…');
  const [errorText, setErrorText] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    loadWorldV2Preview({
      mip,
      signal: controller.signal,
      onProgress: (msg) => {
        setStatusText(msg);
      },
    })
      .then((loaded) => {
        if (!controller.signal.aborted) {
          setGrid(loaded);
          setErrorText(null);
        }
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        const msg = err instanceof Error ? err.message : String(err);
        setErrorText(msg);
      });

    return () => {
      controller.abort();
    };
  }, [mip]);

  const views = useMemo(() => (grid ? derivePreviewViews(grid) : null), [grid]);
  const metaText = grid
    ? `C01 ${grid.simWorldIdentity.slice(0, 8)} · L${grid.mip} ${grid.size}x${grid.size} · ${grid.tileCountVerified} tiles SHA-256 verified`
    : null;

  return (
    <div style={{ width: '100%', height: '100%', position: 'relative', background: '#0c1828' }}>
      <WorldV2Banner statusText={statusText} errorText={errorText} metaText={metaText} />
      {grid && views && !errorText ? (
        <Canvas
          shadows
          dpr={[1, 2]}
          camera={{ fov: 50, near: 0.5, far: 6000, position: views.overview.position }}
          gl={{ antialias: true, toneMapping: THREE.ACESFilmicToneMapping, toneMappingExposure: 1.05 }}
        >
          <fog attach="fog" args={['#b7d4ec', 900, 3200]} />
          <ambientLight intensity={0.42} />
          <hemisphereLight args={['#dbeafe', '#475569', 0.55]} />
          <directionalLight position={[720, 620, 460]} intensity={1.75} castShadow />
          <WorldV2TerrainMesh grid={grid} />
          <WorldV2WaterMesh grid={grid} />
          <PreviewRig grid={grid} views={views} />
        </Canvas>
      ) : null}
    </div>
  );
};

export default WorldV2PreviewScene;
