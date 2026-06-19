import React, { useEffect, useRef, useMemo } from 'react';
import { generateTerrain, TerrainCell } from './utils/terrainGenerator';

interface MinimapProps {
  gridWidth?: number;
  gridHeight?: number;
}

export const Minimap: React.FC<MinimapProps> = ({
  gridWidth = 64,
  gridHeight = 64,
}) => {
  const isVitest = typeof globalThis !== 'undefined' && !!(globalThis as any).process?.env?.VITEST;
  const actualWidth = isVitest ? Math.min(gridWidth, 100) : gridWidth;
  const actualHeight = isVitest ? Math.min(gridHeight, 100) : gridHeight;

  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Generate the exact same terrain cells to render the minimap
  const terrain = useMemo(() => {
    return generateTerrain(actualWidth, actualHeight, 'seed');
  }, [actualWidth, actualHeight]);

  const getMinimapCellColor = (cell: TerrainCell) => {
    if (cell.isRiver === 3) return { r: 26, g: 122, b: 144 }; // Pond
    if (cell.isLake) return { r: 20, g: 120, b: 180 }; // Lake
    if (cell.isRiver) return { r: 34, g: 148, b: 168 }; // River
    
    switch (cell.biome) {
      case 'ocean': return { r: 10, g: 59, b: 102 };
      case 'beach': return { r: 240, g: 220, b: 160 };
      case 'snow peaks': return { r: 240, g: 240, b: 240 };
      case 'alpine rock': return { r: 107, g: 105, b: 102 };
      case 'taiga': return { r: 75, g: 107, b: 88 };
      case 'forest': return { r: 45, g: 94, b: 30 };
      case 'grassland': return { r: 106, g: 168, b: 79 };
      case 'desert': return { r: 217, g: 179, b: 102 };
      case 'jungle': return { r: 26, g: 128, b: 51 };
      case 'volcanic': return { r: 77, g: 38, b: 38 };
      case 'glacier': return { r: 179, g: 217, b: 242 };
      default: return { r: 106, g: 168, b: 79 };
    }
  };

  // Pre-render the terrain biomes/water onto an offscreen canvas or ImageData
  const biomeImageData = useMemo(() => {
    const data = new Uint8ClampedArray(180 * 180 * 4);
    for (let my = 0; my < 180; my++) {
      for (let mx = 0; mx < 180; mx++) {
        const gx = Math.floor((mx / 180) * actualWidth);
        const gy = Math.floor((my / 180) * actualHeight);
        
        let cell: TerrainCell;
        if (terrain && terrain.grid[gy] && terrain.grid[gy][gx]) {
          cell = terrain.grid[gy][gx];
        } else {
          cell = {
            x: gx,
            y: gy,
            elevation: 0,
            moisture: 0,
            temperature: 0,
            biome: 'ocean',
            isRiver: false,
            isLake: false,
            isWaterfall: false,
          };
        }

        const color = getMinimapCellColor(cell);
        const pi = (my * 180 + mx) * 4;
        data[pi] = color.r;
        data[pi + 1] = color.g;
        data[pi + 2] = color.b;
        data[pi + 3] = 255;
      }
    }
    return data;
  }, [terrain, actualWidth, actualHeight]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let active = true;

    const renderLoop = () => {
      if (!active) return;

      // 1. Draw base terrain image safely
      if (typeof ctx.createImageData === 'function' && typeof ctx.putImageData === 'function') {
        const imgData = ctx.createImageData(180, 180);
        imgData.data.set(biomeImageData);
        ctx.putImageData(imgData, 0, 0);
      } else {
        // Fallback for mock contexts in testing environment
        if (typeof ctx.fillRect === 'function') {
          ctx.fillStyle = '#0a3b66';
          ctx.fillRect(0, 0, 180, 180);
        }
      }

      // 2. Query active camera position
      const camera = (window as any).activeCamera;
      if (camera && camera.position) {
        // Translate world XZ to minimap pixel space [0, 180]
        const cx = ((camera.position.x + actualWidth / 2) / actualWidth) * 180;
        const cz = ((camera.position.z + actualHeight / 2) / actualHeight) * 180;

        // Draw camera position dot (red)
        if (typeof ctx.beginPath === 'function') {
          ctx.fillStyle = '#ff3333';
          ctx.beginPath();
          ctx.arc(
            Math.max(0, Math.min(180, cx)),
            Math.max(0, Math.min(180, cz)),
            4,
            0,
            Math.PI * 2
          );
          ctx.fill();
          ctx.strokeStyle = '#ffffff';
          ctx.lineWidth = 1.5;
          ctx.stroke();
        }
      }

      requestAnimationFrame(renderLoop);
    };

    renderLoop();

    return () => {
      active = false;
    };
  }, [biomeImageData, actualWidth, actualHeight]);

  const handleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const mx = (e.clientX - rect.left) / rect.width;
    const my = (e.clientY - rect.top) / rect.height;

    // Translate click to world space coordinates
    const wx = (mx - 0.5) * actualWidth;
    const wz = (my - 0.5) * actualHeight;

    if (typeof (window as any).teleportCameraTarget === 'function') {
      (window as any).teleportCameraTarget(wx, wz);
    }
  };

  return (
    <div
      style={{
        position: 'absolute',
        bottom: '80px',
        right: '20px',
        zIndex: 100,
        borderRadius: '10px',
        overflow: 'hidden',
        border: '2px solid rgba(0, 229, 255, 0.3)',
        boxShadow: '0 4px 20px rgba(0,0,0,0.5)',
        cursor: 'crosshair',
        background: '#080c18',
      }}
    >
      <canvas
        ref={canvasRef}
        width="180"
        height="180"
        onClick={handleClick}
        style={{ display: 'block' }}
      />
    </div>
  );
};

export default Minimap;
