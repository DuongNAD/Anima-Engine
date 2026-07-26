import React, { useEffect, useRef } from 'react';
import * as PIXI from 'pixi.js';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { TerrainMapState } from './types';
import {
  fetchHotRadius,
  focusForViewport,
  sendLodFocus,
  sendLodFocusNow,
  shouldSend,
  FOCUS_OFF,
  SAMPLE_INTERVAL_MS,
  type LodFocusPayload,
} from './utils/lodFocus';

const BIOME_COLORS: { [key: number]: number } = {
  0: 0x0a1450, // DeepOcean
  1: 0x235091, // Ocean
  2: 0xdcd38c, // Beach
  3: 0x3787d7, // River
  4: 0x8cbe64, // Grassland
  5: 0x41874b, // TemperateForest
  6: 0x2d5f50, // BorealForest
  7: 0x0f552d, // Rainforest
  8: 0xd7af69, // Desert
  9: 0x787d82, // MountainRock
  10: 0xf0f0f5, // Snow
};

function generateTerrainCanvas(terrainMap: TerrainMapState): HTMLCanvasElement {
  const { width, height, biomes, elevations } = terrainMap;

  // 1. Create a small canvas of size width x height
  const canvasSmall = document.createElement('canvas');
  canvasSmall.width = width;
  canvasSmall.height = height;
  const ctxSmall = canvasSmall.getContext('2d');

  if (!ctxSmall || typeof ctxSmall.createImageData !== 'function' || typeof ctxSmall.putImageData !== 'function') {
    return canvasSmall;
  }

  // 2. Calls createImageData(width, height) and fills it with biome colors.
  const imgDataSmall = ctxSmall.createImageData(width, height);
  for (let i = 0; i < biomes.length; i++) {
    const biome = biomes[i];
    const color = BIOME_COLORS[biome] !== undefined ? BIOME_COLORS[biome] : 0x000000;
    const r = (color >> 16) & 0xff;
    const g = (color >> 8) & 0xff;
    const b = color & 0xff;
    const idx = i * 4;
    imgDataSmall.data[idx] = r;
    imgDataSmall.data[idx + 1] = g;
    imgDataSmall.data[idx + 2] = b;
    imgDataSmall.data[idx + 3] = 255;
  }
  ctxSmall.putImageData(imgDataSmall, 0, 0);

  // 3. Creates a larger canvas canvasLarge of size Math.max(512, width) x Math.max(512, height).
  const targetWidth = Math.max(512, width);
  const targetHeight = Math.max(512, height);
  const canvasLarge = document.createElement('canvas');
  canvasLarge.width = targetWidth;
  canvasLarge.height = targetHeight;
  const ctxLarge = canvasLarge.getContext('2d');

  if (!ctxLarge) {
    return canvasSmall;
  }

  // Guard against missing canvas API methods (like drawImage, createImageData or getImageData) in testing spies by returning canvasSmall gracefully.
  if (typeof ctxLarge.drawImage !== 'function' || typeof ctxLarge.createImageData !== 'function' || typeof ctxLarge.getImageData !== 'function') {
    return canvasSmall;
  }

  // 4. Draws canvasSmall onto canvasLarge scaled up with image smoothing enabled (providing bilinear smooth blending).
  ctxLarge.imageSmoothingEnabled = true;
  ctxLarge.drawImage(canvasSmall, 0, 0, targetWidth, targetHeight);

  // 5. Checks if elevations array is present. If so, computes gradients (dzdx, dzdy) on the low-res elevations array,
  // interpolates them bilinearly for each high-res pixel, and applies 3D hillshading 1.0 - (slopeX + slopeY) * 1.5 clamped to [0.4, 1.6].
  let imgDataLarge: ImageData;
  try {
    imgDataLarge = ctxLarge.getImageData(0, 0, targetWidth, targetHeight);
  } catch (e) {
    return canvasSmall;
  }

  if (elevations && elevations.length === width * height) {
    const dzdx = new Float32Array(width * height);
    const dzdy = new Float32Array(width * height);
    for (let r = 0; r < height; r++) {
      for (let c = 0; c < width; c++) {
        const idx = r * width + c;
        let slopeX = 0;
        let slopeY = 0;

        if (width > 1) {
          if (c === 0) {
            slopeX = elevations[r * width + 1] - elevations[r * width];
          } else if (c === width - 1) {
            slopeX = elevations[r * width + width - 1] - elevations[r * width + width - 2];
          } else {
            slopeX = (elevations[r * width + c + 1] - elevations[r * width + c - 1]) / 2.0;
          }
        }

        if (height > 1) {
          if (r === 0) {
            slopeY = elevations[width + c] - elevations[c];
          } else if (r === height - 1) {
            slopeY = elevations[(height - 1) * width + c] - elevations[(height - 2) * width + c];
          } else {
            slopeY = (elevations[(r + 1) * width + c] - elevations[(r - 1) * width + c]) / 2.0;
          }
        }

        dzdx[idx] = slopeX;
        dzdy[idx] = slopeY;
      }
    }

    const scaleX = (width - 1) / (targetWidth - 1 || 1);
    const scaleY = (height - 1) / (targetHeight - 1 || 1);

    for (let y = 0; y < targetHeight; y++) {
      const lr = y * scaleY;
      const r0 = Math.floor(lr);
      const r1 = Math.min(height - 1, r0 + 1);
      const tr = lr - r0;

      for (let x = 0; x < targetWidth; x++) {
        const lc = x * scaleX;
        const c0 = Math.floor(lc);
        const c1 = Math.min(width - 1, c0 + 1);
        const tc = lc - c0;

        const g00_x = dzdx[r0 * width + c0];
        const g10_x = dzdx[r0 * width + c1];
        const g01_x = dzdx[r1 * width + c0];
        const g11_x = dzdx[r1 * width + c1];
        const slopeX = g00_x * (1 - tc) * (1 - tr) +
                       g10_x * tc * (1 - tr) +
                       g01_x * (1 - tc) * tr +
                       g11_x * tc * tr;

        const g00_y = dzdy[r0 * width + c0];
        const g10_y = dzdy[r0 * width + c1];
        const g01_y = dzdy[r1 * width + c0];
        const g11_y = dzdy[r1 * width + c1];
        const slopeY = g00_y * (1 - tc) * (1 - tr) +
                       g10_y * tc * (1 - tr) +
                       g01_y * (1 - tc) * tr +
                       g11_y * tc * tr;

        const hillshading = Math.max(0.4, Math.min(1.6, 1.0 - (slopeX + slopeY) * 1.5));

        const largeIdx = (y * targetWidth + x) * 4;
        imgDataLarge.data[largeIdx] = Math.max(0, Math.min(255, imgDataLarge.data[largeIdx] * hillshading));
        imgDataLarge.data[largeIdx + 1] = Math.max(0, Math.min(255, imgDataLarge.data[largeIdx + 1] * hillshading));
        imgDataLarge.data[largeIdx + 2] = Math.max(0, Math.min(255, imgDataLarge.data[largeIdx + 2] * hillshading));
      }
    }
  }

  // 6. Applies random grain noise (Math.random() - 0.5) * 12 to R, G, B channels of canvasLarge.
  for (let i = 0; i < imgDataLarge.data.length; i += 4) {
    const noise = (Math.random() - 0.5) * 12;
    imgDataLarge.data[i] = Math.max(0, Math.min(255, imgDataLarge.data[i] + noise));
    imgDataLarge.data[i + 1] = Math.max(0, Math.min(255, imgDataLarge.data[i + 1] + noise));
    imgDataLarge.data[i + 2] = Math.max(0, Math.min(255, imgDataLarge.data[i + 2] + noise));
  }

  ctxLarge.putImageData(imgDataLarge, 0, 0);
  return canvasLarge;
}


export interface PixiViewportProps {
  projection?: 'xy' | 'xz';
  segments?: any[] | null;
  raycasts?: any[] | null;
  pheromoneGrid?: { grid: number[]; width: number; height: number } | null;
  environmentalState?: { elements: any[] } | null;
  zoom?: number;
  pan?: { x: number; y: number };
}

const beginFill = (g: any, color: number, alpha?: number) => {
  if (typeof g.beginFill === 'function') {
    g.beginFill(color, alpha);
  } else if (typeof g.fill === 'function') {
    g.fill({ color, alpha });
  }
};

const endFill = (g: any) => {
  if (typeof g.endFill === 'function') {
    g.endFill();
  }
};

const lineStyle = (g: any, width: number, color: number, alpha?: number) => {
  if (typeof g.lineStyle === 'function') {
    g.lineStyle(width, color, alpha);
  } else if (typeof g.stroke === 'function') {
    g.stroke({ width, color, alpha });
  }
};

export const PixiViewport: React.FC<PixiViewportProps> = ({
  projection = 'xy',
  segments: propSegments,
  raycasts: propRaycasts,
  pheromoneGrid: propPheromoneGrid,
  environmentalState: propEnvironmentalState,
  zoom = 1.0,
  pan = { x: 0, y: 0 }
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<PIXI.Application | null>(null);
  const graphicsRef = useRef<PIXI.Graphics | null>(null);
  const bgSpriteRef = useRef<PIXI.Sprite | null>(null);
  const terrainMapRef = useRef<TerrainMapState | null>(null);


  const segmentsRef = useRef<any[]>([]);
  const raycastsRef = useRef<any[]>([]);
  const pheromoneGridRef = useRef<any>(null);
  const projectionRef = useRef<'xy' | 'xz'>(projection);
  const environmentalStateRef = useRef<any>({ elements: [] });

  const zoomRef = useRef<number>(zoom);
  const panRef = useRef<{ x: number; y: number }>(pan);

  // What this viewport is currently looking at, in backend world units — the input to simulation
  // LOD (see the effect near the bottom of this file). Written by `draw`, which is the only place
  // that knows the pan/zoom/auto-fit mapping. `null` means "no usable focus": the side-on `xy`
  // projection shows height, not depth, so it carries no z to point the simulation at.
  const lodViewRef = useRef<{ x: number; z: number; visibleHalfExtent: number } | null>(null);

  useEffect(() => {
    if (propEnvironmentalState) {
      environmentalStateRef.current = propEnvironmentalState;
    }
  }, [propEnvironmentalState]);

  useEffect(() => {
    projectionRef.current = projection;
  }, [projection]);

  useEffect(() => {
    zoomRef.current = zoom;
  }, [zoom]);

  useEffect(() => {
    panRef.current = pan;
  }, [pan]);



  const draw = () => {
    const graphics = graphicsRef.current;
    if (!graphics) return;

    graphics.clear();

    const segments = propSegments !== undefined ? propSegments : segmentsRef.current;
    const raycasts = propRaycasts !== undefined ? propRaycasts : raycastsRef.current;
    const pheromoneGrid = propPheromoneGrid !== undefined ? propPheromoneGrid : pheromoneGridRef.current;
    const environmentalState = propEnvironmentalState !== undefined ? propEnvironmentalState : environmentalStateRef.current;
    const proj = projectionRef.current;

    let minX = -100, maxX = 100;
    let minY = -100, maxY = 100;
    let scale = 1.0;
    let midX = 0;
    let midY = 0;

    if (Array.isArray(segments) && segments.length > 0) {
      let sMinX = Infinity, sMaxX = -Infinity;
      let sMinY = Infinity, sMaxY = -Infinity;

      segments.forEach((s) => {
        if (!s) return;
        const xVal = s.x;
        const yVal = proj === 'xy' ? s.y : s.z;
        if (xVal < sMinX) sMinX = xVal;
        if (xVal > sMaxX) sMaxX = xVal;
        if (yVal < sMinY) sMinY = yVal;
        if (yVal > sMaxY) sMaxY = yVal;
      });

      minX = sMinX;
      maxX = sMaxX;
      minY = sMinY;
      maxY = sMaxY;

      const rangeX = maxX - minX || 1;
      const rangeY = maxY - minY || 1;
      const padding = 50;
      const drawWidth = 500 - padding * 2;
      const drawHeight = 350 - padding * 2;

      scale = Math.min(drawWidth / rangeX, drawHeight / rangeY);
      midX = (minX + maxX) / 2;
      midY = (minY + maxY) / 2;
    } else if (terrainMapRef.current && terrainMapRef.current.bounds && terrainMapRef.current.bounds.min && terrainMapRef.current.bounds.max) {
      const bounds = terrainMapRef.current.bounds;
      minX = bounds.min.x;
      maxX = bounds.max.x;
      minY = proj === 'xy' ? bounds.min.y : bounds.min.z;
      maxY = proj === 'xy' ? bounds.max.y : bounds.max.z;

      const rangeX = maxX - minX || 1;
      const rangeY = maxY - minY || 1;
      const padding = 50;
      const drawWidth = 500 - padding * 2;
      const drawHeight = 350 - padding * 2;

      scale = Math.min(drawWidth / rangeX, drawHeight / rangeY);
      midX = (minX + maxX) / 2;
      midY = (minY + maxY) / 2;
    } else {
      minX = -100;
      maxX = 100;
      minY = -100;
      maxY = 100;
      scale = 1.0;
      midX = 0;
      midY = 0;
    }

    const currentScale = scale;

    const getCoordsNoPan = (x: number, y: number): [number, number] => {
      const centerX = 500 / 2;
      const centerY = 350 / 2;
      const cx = centerX + (x - midX) * scale;
      const cy = centerY - (y - midY) * scale;
      return [cx, cy];
    };

    const getCoords = (x: number, y: number): [number, number] => {
      const [cx, cy] = getCoordsNoPan(x, y);
      return [cx * zoomRef.current + panRef.current.x, cy * zoomRef.current + panRef.current.y];
    };

    const screenToWorld = (sx: number, sy: number): [number, number] => {
      const cx = (sx - panRef.current.x) / zoomRef.current;
      const cy = (sy - panRef.current.y) / zoomRef.current;
      const centerX = 500 / 2;
      const centerY = 350 / 2;
      const wx = midX + (cx - centerX) / scale;
      const wy = midY - (cy - centerY) / scale;
      return [wx, wy];
    };

    // Publish the view for simulation LOD. Only under the top-down projection: in `xy` the second
    // screen axis is height, so `screenToWorld` yields no z, and inventing one would tier half the
    // world against a coordinate the user never chose.
    if (proj === 'xz') {
      const [viewX, viewZ] = screenToWorld(500 / 2, 350 / 2);
      const worldPerPixel = 1 / (scale * (zoomRef.current || 1));
      // Half the diagonal, not half the width: the screen corner is the farthest point still
      // visible, and it is the one that must not fall outside the hot band.
      const visibleHalfExtent = Math.hypot(500 / 2, 350 / 2) * worldPerPixel;
      lodViewRef.current = { x: viewX, z: viewZ, visibleHalfExtent };
    } else {
      lodViewRef.current = null;
    }

    // Draw Terrain Background Sprite or HUD background fallback
    const terrainMap = terrainMapRef.current;
    if (terrainMap && terrainMap.bounds && terrainMap.bounds.min && terrainMap.bounds.max && bgSpriteRef.current) {
      const bounds = terrainMap.bounds;
      const tMinX = bounds.min.x;
      const tMaxX = bounds.max.x;
      const tMinY = proj === 'xy' ? bounds.min.y : bounds.min.z;
      const tMaxY = proj === 'xy' ? bounds.max.y : bounds.max.z;

      const [leftX, topY] = getCoords(tMinX, tMaxY);
      const [rightX, bottomY] = getCoords(tMaxX, tMinY);

      bgSpriteRef.current.position.set(leftX, topY);
      bgSpriteRef.current.width = rightX - leftX;
      bgSpriteRef.current.height = bottomY - topY;
    } else {
      beginFill(graphics, 0x09090b, 1.0); // Soft dark HUD background fallback
      graphics.drawRect(0, 0, 500, 350);
      endFill(graphics);
    }

    // 0. Draw the Grid Over the Map (A-P, 1-16 style)
    const gridSizeX = (maxX - minX) / 16;
    const gridSizeY = (maxY - minY) / 16;
    const gridAlpha = Math.max(0.2, Math.min(0.5, 0.4 / zoomRef.current));
    lineStyle(graphics, 1.5, 0xffffff, gridAlpha);
    for (let i = 0; i <= 16; i++) {
      // Vertical lines
      const vx = minX + i * gridSizeX;
      const [screenVX1, screenVY1] = getCoords(vx, maxY);
      const [screenVX2, screenVY2] = getCoords(vx, minY);
      graphics.moveTo(screenVX1, screenVY1);
      graphics.lineTo(screenVX2, screenVY2);
      
      // Horizontal lines
      const hy = minY + i * gridSizeY;
      const [screenHX1, screenHY1] = getCoords(minX, hy);
      const [screenHX2, screenHY2] = getCoords(maxX, hy);
      graphics.moveTo(screenHX1, screenHY1);
      graphics.lineTo(screenHX2, screenHY2);
    }

    // 0.5. Draw POI markers on top of the background
    if (terrainMap && Array.isArray(terrainMap.pois)) {
      terrainMap.pois.forEach((poi: any) => {
        // Assume poi is [px, py] coordinate on the 1024x1024 map. 
        // We need to map [0, 1024] to [minX, maxX]
        const px = poi[0];
        const py = poi[1];
        const wx = minX + (px / (terrainMap.width || 1024)) * (maxX - minX);
        const wy = maxY - (py / (terrainMap.height || 1024)) * (maxY - minY); // invert Y
        const [cx, cy] = getCoords(wx, wy);

        // Draw a nice blue icon with a white border
        beginFill(graphics, 0xffffff, 1.0); // white border
        graphics.drawCircle(cx, cy, 5 * zoomRef.current);
        endFill(graphics);
        
        beginFill(graphics, 0x1d9bf0, 1.0); // blue center
        graphics.drawCircle(cx, cy, 3.5 * zoomRef.current);
        endFill(graphics);
      });
    }

    // 1. Draw Pheromone Grid heatmap
    if (pheromoneGrid && pheromoneGrid.grid) {
      const { grid, width, height } = pheromoneGrid;
      if (width > 0 && height > 0 && Array.isArray(grid)) {
        const cellWorldW = (maxX - minX) / width;
        const cellWorldH = (maxY - minY) / height;
        grid.forEach((val: number, idx: number) => {
          if (val > 0) {
            const x = idx % width;
            const y = Math.floor(idx / width);
            beginFill(graphics, 0xffffff, val * 0.45);
            const wx1 = minX + x * cellWorldW;
            const wy1 = maxY - y * cellWorldH;
            const wx2 = minX + (x + 1) * cellWorldW;
            const wy2 = maxY - (y + 1) * cellWorldH;
            const [rx, ry] = getCoords(wx1, wy1);
            const [rx2, ry2] = getCoords(wx2, wy2);
            const rw = rx2 - rx;
            const rh = ry2 - ry;
            graphics.drawRect(rx, ry, rw, rh);
            endFill(graphics);
          }
        });
      }
    }

    // 2. Draw environmental elements (Lakes & Trees with animations)
    const time = performance.now();
    if (environmentalState && Array.isArray(environmentalState.elements)) {
      environmentalState.elements.forEach((elem: any) => {
        if (!elem) return;
        const eyVal = proj === 'xy' ? elem.y : elem.z;
        const [cx, cy] = getCoords(elem.x, eyVal);

        if (elem.type === 'lake') {
          // Ripple wave effect
          const waveOffset = Math.sin(time * 0.003 + elem.x * 0.1) * 3;
          const radius = (elem.radius + waveOffset) * currentScale * zoomRef.current;

          beginFill(graphics, 0xcccccc, 0.1);
          graphics.drawCircle(cx, cy, radius);
          endFill(graphics);

          beginFill(graphics, 0xcccccc, 0.15);
          graphics.drawCircle(cx, cy, radius * 0.7);
          endFill(graphics);

          beginFill(graphics, 0xcccccc, 0.2);
          graphics.drawCircle(cx, cy, radius * 0.4);
          endFill(graphics);
        } else {
          // Trees: botanical assets
          const treeSize = ((elem.resources / 100.0) * 12 + 10) * currentScale * zoomRef.current;

          beginFill(graphics, 0x444444, 0.9); // Trunk
          graphics.drawRect(cx - 3 * zoomRef.current, cy, 6 * zoomRef.current, treeSize * 0.8);
          endFill(graphics);

          beginFill(graphics, 0x888888, 0.85); // Canopy leaves
          graphics.drawCircle(cx, cy - treeSize * 0.4, treeSize * 0.8);
          endFill(graphics);
          
          beginFill(graphics, 0xaaaaaa, 0.8);
          graphics.drawCircle(cx - treeSize * 0.3, cy - treeSize * 0.2, treeSize * 0.6);
          graphics.drawCircle(cx + treeSize * 0.3, cy - treeSize * 0.2, treeSize * 0.6);
          endFill(graphics);
        }
      });
    }

    // 3. Draw active sensor raycast beams
    if (Array.isArray(raycasts)) {
      raycasts.forEach((r) => {
        if (r && r.origin && r.direction && r.origin.length >= 3 && r.direction.length >= 3) {
          const startX = r.origin[0];
          const startY = proj === 'xy' ? r.origin[1] : r.origin[2];
          const endX = startX + r.direction[0] * r.hit_distance;
          const endY = startY + (proj === 'xy' ? r.direction[1] : r.direction[2]) * r.hit_distance;

          const [scx, scy] = getCoords(startX, startY);
          const [ecx, ecy] = getCoords(endX, endY);

          lineStyle(graphics, 1.5 * zoomRef.current, r.hit_entity_type === 'None' ? 0xaaaaaa : 0xffffff, r.hit_entity_type === 'None' ? 0.25 : 0.75);
          graphics.moveTo(scx, scy);
          graphics.lineTo(ecx, ecy);

          // Draw small hit-point marker
          if (r.hit_entity_type !== 'None') {
            beginFill(graphics, 0xffffff, 0.9);
            graphics.drawCircle(ecx, ecy, 3 * zoomRef.current);
            endFill(graphics);
          }
        }
      });
    }

    // 4. Draw segment connections/linkages
    if (Array.isArray(segments)) {
      segments.forEach((s) => {
        if (s && s.parent_segment_id !== null && s.parent_segment_id !== undefined) {
          const parent = segments.find(
            (p) => p && p.agent_id === s.agent_id && p.segment_id === s.parent_segment_id
          );
          if (parent) {
            const pyVal = proj === 'xy' ? parent.y : parent.z;
            const syVal = proj === 'xy' ? s.y : s.z;
            const [px, py] = getCoords(parent.x, pyVal);
            const [cx, cy] = getCoords(s.x, syVal);

            const opacity = 0.3 + ((s.energy || 0) / 100.0) * 0.7;
            lineStyle(graphics, 3.5 * zoomRef.current, 0x888888, opacity);
            graphics.moveTo(px, py);
            graphics.lineTo(cx, cy);
          }
        }
      });
    }

    // 5. Draw segment geometries (Predators and Prey with direction indicators)
    if (Array.isArray(segments)) {
      segments.forEach((s) => {
        if (!s) return;
        const yVal = proj === 'xy' ? s.y : s.z;
        const [cx, cy] = getCoords(s.x, yVal);
        const opacity = 0.3 + ((s.energy || 0) / 100.0) * 0.7;

        const angle = Array.isArray(s.head_direction)
          ? Math.atan2(proj === 'xy' ? s.head_direction[1] : s.head_direction[2], s.head_direction[0])
          : 0;

        if (s.agent_type === 'predator') {
          const predSize = ((s.energy || 50) / 100.0 * 10 + 10) * zoomRef.current;
          beginFill(graphics, 0xffffff, opacity);
          const p1_x = cx + Math.cos(angle) * predSize;
          const p1_y = cy + Math.sin(angle) * predSize;
          const p2_x = cx + Math.cos(angle + 2.3) * predSize * 0.7;
          const p2_y = cy + Math.sin(angle + 2.3) * predSize * 0.7;
          const p3_x = cx + Math.cos(angle - 2.3) * predSize * 0.7;
          const p3_y = cy + Math.sin(angle - 2.3) * predSize * 0.7;
          graphics.drawPolygon([p1_x, p1_y, p2_x, p2_y, p3_x, p3_y]);
          endFill(graphics);

          // Head arrow indicator line
          lineStyle(graphics, 2 * zoomRef.current, 0xffffff, 0.7);
          graphics.moveTo(cx, cy);
          graphics.lineTo(cx + Math.cos(angle) * predSize * 1.4, cy + Math.sin(angle) * predSize * 1.4);
        } else if (s.agent_type === 'prey') {
          const preySize = 10 * zoomRef.current;
          beginFill(graphics, 0x777777, opacity);
          graphics.drawCircle(cx, cy, preySize);
          endFill(graphics);

          // Tail indicator pointing backward
          lineStyle(graphics, 2.5 * zoomRef.current, 0x777777, opacity * 0.8);
          graphics.moveTo(cx, cy);
          graphics.lineTo(cx - Math.cos(angle) * preySize * 1.6, cy - Math.sin(angle) * preySize * 1.6);

          // Prey hydration bar
          if (s.hydration !== undefined) {
            const barW = 16 * zoomRef.current;
            const barH = 3 * zoomRef.current;
            const bx = cx - barW / 2;
            const by = cy - preySize - 6 * zoomRef.current;

            beginFill(graphics, 0x333333, 0.85); // Backing dark gray
            graphics.drawRect(bx, by, barW, barH);
            endFill(graphics);

            beginFill(graphics, 0xdddddd, 0.95); // Hydration level light gray
            graphics.drawRect(bx, by, barW * (s.hydration / 100.0), barH);
            endFill(graphics);
          }
        } else {
          const isRoot = s.parent_segment_id === null || s.parent_segment_id === undefined;
          const color = isRoot ? 0x888888 : 0xaaaaaa;
          beginFill(graphics, color, opacity);
          graphics.drawCircle(cx, cy, 10 * zoomRef.current);
          endFill(graphics);
        }
      });
    }

    // 6. Draw Minimap corner overlay widget
    const mmX = 380;
    const mmY = 230;
    const mmW = 110;
    const mmH = 110;

    beginFill(graphics, 0x09090b, 0.8); // Dark semi-transparent background
    lineStyle(graphics, 1.5, 0x888888, 0.7);
    graphics.drawRect(mmX, mmY, mmW, mmH);
    endFill(graphics);

    const getMinimapCoords = (x: number, y: number): [number, number] => {
      const boundsRange = 200; // -100 to 100 range
      const mx = mmX + mmW / 2 + (x / boundsRange) * mmW;
      const my = mmY + mmH / 2 - (y / boundsRange) * mmH;
      return [
        Math.max(mmX + 2, Math.min(mmX + mmW - 2, mx)),
        Math.max(mmY + 2, Math.min(mmY + mmH - 2, my))
      ];
    };

    const getMinimapCoordsNoClamp = (x: number, y: number): [number, number] => {
      const boundsRange = 200;
      const mx = mmX + mmW / 2 + (x / boundsRange) * mmW;
      const my = mmY + mmH / 2 - (y / boundsRange) * mmH;
      return [mx, my];
    };

    if (environmentalState && Array.isArray(environmentalState.elements)) {
      environmentalState.elements.forEach((elem: any) => {
        const ey = proj === 'xy' ? elem.y : elem.z;
        const [mx, my] = getMinimapCoords(elem.x, ey);
        beginFill(graphics, elem.type === 'lake' ? 0xcccccc : 0x888888, 0.9);
        graphics.drawCircle(mx, my, 3.5);
        endFill(graphics);
      });
    }

    if (Array.isArray(segments)) {
      segments.forEach((s) => {
        if (!s) return;
        const sy = proj === 'xy' ? s.y : s.z;
        const [mx, my] = getMinimapCoords(s.x, sy);
        const isRoot = s.parent_segment_id === null || s.parent_segment_id === undefined;
        if (isRoot) {
          beginFill(graphics, s.agent_type === 'predator' ? 0xffffff : 0x777777, 1.0);
          graphics.drawCircle(mx, my, 2.5);
          endFill(graphics);
        }
      });
    }

    // 7. Draw camera viewport indicator box (highlighted white/gray box bottom right)
    const [wLeft, wTop] = screenToWorld(0, 0);
    const [wRight, wBottom] = screenToWorld(500, 350);

    const [mLeft, mTop] = getMinimapCoordsNoClamp(wLeft, wTop);
    const [mRight, mBottom] = getMinimapCoordsNoClamp(wRight, wBottom);

    const bx = Math.max(mmX + 2, Math.min(mmX + mmW - 2, mLeft));
    const by = Math.max(mmY + 2, Math.min(mmY + mmH - 2, mTop));
    const bw = Math.max(2, Math.min(mmX + mmW - 2 - bx, mRight - mLeft));
    const bh = Math.max(2, Math.min(mmY + mmH - 2 - by, mBottom - mTop));

    beginFill(graphics, 0xffffff, 0.05); 
    lineStyle(graphics, 8, 0x555555, 0.15); // Outer soft border
    graphics.drawRect(bx - 2, by - 2, bw + 4, bh + 4);

    lineStyle(graphics, 5, 0xaaaaaa, 0.35); // Medium glow border
    graphics.drawRect(bx - 1, by - 1, bw + 2, bh + 2);

    lineStyle(graphics, 2.5, 0xffffff, 0.8); // Core thick border
    graphics.drawRect(bx, by, bw, bh);
    endFill(graphics);
  };

  useEffect(() => {
    let active = true;
    let unlistenTick: (() => void) | null = null;
    let unlistenRaycast: (() => void) | null = null;
    let unlistenPheromone: (() => void) | null = null;
    let animationFrameId: number;

    const initPixi = async () => {
      if (!containerRef.current) return;

      const app = new PIXI.Application();
      await app.init({ width: 500, height: 350, backgroundColor: 0x09090b });
      
      if (!active) {
        app.destroy(true);
        return;
      }

      appRef.current = app;

      const graphics = new PIXI.Graphics();
      graphicsRef.current = graphics;
      app.stage.addChild(graphics);

      containerRef.current.appendChild(app.canvas);

      // Local camera control event handlers (drag-pan and zoom)
      const canvasElement = app.canvas;
      let isDragging = false;
      let startPos = { x: 0, y: 0 };
      let startPan = { x: 0, y: 0 };

      const onPointerDown = (e: PointerEvent) => {
        isDragging = true;
        startPos = { x: e.clientX, y: e.clientY };
        startPan = { ...panRef.current };
        canvasElement.setPointerCapture(e.pointerId);
      };

      const onPointerMove = (e: PointerEvent) => {
        if (!isDragging) return;
        const dx = e.clientX - startPos.x;
        const dy = e.clientY - startPos.y;
        panRef.current = { x: startPan.x + dx, y: startPan.y + dy };
        draw();
      };

      const onPointerUp = (e: PointerEvent) => {
        if (!isDragging) return;
        isDragging = false;
        canvasElement.releasePointerCapture(e.pointerId);
      };

      canvasElement.addEventListener('pointerdown', onPointerDown);
      canvasElement.addEventListener('pointermove', onPointerMove);
      canvasElement.addEventListener('pointerup', onPointerUp);
      canvasElement.addEventListener('pointercancel', onPointerUp);

      const onWheel = (e: WheelEvent) => {
        e.preventDefault();
        const zoomFactor = 1.1;
        const oldZoom = zoomRef.current;
        const newZoom = e.deltaY < 0 
          ? Math.min(10.0, oldZoom * zoomFactor) 
          : Math.max(0.1, oldZoom / zoomFactor);
        
        if (newZoom === oldZoom) return;

        const rect = canvasElement.getBoundingClientRect();
        const mouseX = e.clientX - rect.left;
        const mouseY = e.clientY - rect.top;

        // Zoom center math
        const worldX = (mouseX - panRef.current.x) / oldZoom;
        const worldY = (mouseY - panRef.current.y) / oldZoom;

        zoomRef.current = newZoom;
        panRef.current = {
          x: mouseX - worldX * newZoom,
          y: mouseY - worldY * newZoom,
        };
        draw();
      };

      canvasElement.addEventListener('wheel', onWheel, { passive: false });

      const getCoordsLocal = (x: number, y: number): [number, number] => {
        const [cx, cy] = getCoordsNoPan(x, y);
        return [cx * zoomRef.current + panRef.current.x, cy * zoomRef.current + panRef.current.y];
      };

      // Double-click to center on nearest agent
      const onDblClick = (e: MouseEvent) => {
        const rect = canvasElement.getBoundingClientRect();
        const mouseX = e.clientX - rect.left;
        const mouseY = e.clientY - rect.top;

        const segments = propSegments !== undefined ? propSegments : segmentsRef.current;
        let nearestSegment: any = null;
        let minDist = Infinity;

        if (Array.isArray(segments)) {
          segments.forEach((s) => {
            if (!s) return;
            const yVal = projectionRef.current === 'xy' ? s.y : s.z;
            const [cx, cy] = getCoordsLocal(s.x, yVal);
            const dist = Math.hypot(mouseX - cx, mouseY - cy);
            if (dist < minDist) {
              minDist = dist;
              nearestSegment = s;
            }
          });
        }

        if (nearestSegment && minDist < 100) {
          const currentY = projectionRef.current === 'xy' ? nearestSegment.y : nearestSegment.z;
          const [cxNoPan, cyNoPan] = getCoordsNoPan(nearestSegment.x, currentY);
          panRef.current = {
            x: 250 - cxNoPan * zoomRef.current,
            y: 175 - cyNoPan * zoomRef.current
          };
          draw();
        }
      };
      canvasElement.addEventListener('dblclick', onDblClick);

      const getCoordsNoPan = (x: number, y: number): [number, number] => {
        const segments = propSegments !== undefined ? propSegments : segmentsRef.current;
        // Only these three escape the branches. The extents used to be declared out here with
        // fallback values that every branch immediately overwrote — `no-useless-assignment` flags
        // the dead initialisers, and scoping the extents to the branch that computes them says the
        // same thing more directly: outside, they mean nothing.
        let scale: number;
        let midX: number;
        let midY: number;

        if (Array.isArray(segments) && segments.length > 0) {
          let sMinX = Infinity, sMaxX = -Infinity;
          let sMinY = Infinity, sMaxY = -Infinity;

          segments.forEach((s) => {
            if (!s) return;
            const xVal = s.x;
            const yVal = projectionRef.current === 'xy' ? s.y : s.z;
            if (xVal < sMinX) sMinX = xVal;
            if (xVal > sMaxX) sMaxX = xVal;
            if (yVal < sMinY) sMinY = yVal;
            if (yVal > sMaxY) sMaxY = yVal;
          });

          const rangeX = sMaxX - sMinX || 1;
          const rangeY = sMaxY - sMinY || 1;
          const padding = 50;
          const drawWidth = 500 - padding * 2;
          const drawHeight = 350 - padding * 2;

          scale = Math.min(drawWidth / rangeX, drawHeight / rangeY);
          midX = (sMinX + sMaxX) / 2;
          midY = (sMinY + sMaxY) / 2;
        } else if (terrainMapRef.current && terrainMapRef.current.bounds && terrainMapRef.current.bounds.min && terrainMapRef.current.bounds.max) {
          const bounds = terrainMapRef.current.bounds;
          const minX = bounds.min.x;
          const maxX = bounds.max.x;
          const minY = projectionRef.current === 'xy' ? bounds.min.y : bounds.min.z;
          const maxY = projectionRef.current === 'xy' ? bounds.max.y : bounds.max.z;

          const rangeX = maxX - minX || 1;
          const rangeY = maxY - minY || 1;
          const padding = 50;
          const drawWidth = 500 - padding * 2;
          const drawHeight = 350 - padding * 2;

          scale = Math.min(drawWidth / rangeX, drawHeight / rangeY);
          midX = (minX + maxX) / 2;
          midY = (minY + maxY) / 2;
        } else {
          // No segments and no terrain: an identity transform about the origin.
          scale = 1.0;
          midX = 0;
          midY = 0;
        }

        const centerX = 500 / 2;
        const centerY = 350 / 2;
        const cx = centerX + (x - midX) * scale;
        const cy = centerY - (y - midY) * scale;
        return [cx, cy];
      };

      try {
        const env = await invoke<any>('get_environmental_elements');
        if (active && env) environmentalStateRef.current = env;
      } catch (err) {}

      try {
        const grid = await invoke<any>('get_pheromone_grid');
        if (active && grid) pheromoneGridRef.current = grid;
      } catch (err) {}

      try {
        const raycasts = await invoke<any>('get_active_raycasts');
        if (active && raycasts) raycastsRef.current = raycasts;
      } catch (err) {}

      try {
        const uTick = await listen<any>('simulation-tick', (event) => {
          if (active) {
            if (Array.isArray(event.payload)) {
              segmentsRef.current = event.payload;
            } else if (event.payload && typeof event.payload === 'object') {
              if (Array.isArray(event.payload.segments)) {
                segmentsRef.current = event.payload.segments;
              } else {
                segmentsRef.current = [];
              }
              if (event.payload.environmental_state) {
                environmentalStateRef.current = event.payload.environmental_state;
              }
            } else {
              segmentsRef.current = [];
            }
            draw();
          }
        });
        if (!active) uTick();
        else unlistenTick = uTick;

        const uRay = await listen<any>('raycast-update', (event) => {
          if (active) {
            raycastsRef.current = Array.isArray(event.payload) ? event.payload : [];
            draw();
          }
        });
        if (!active) uRay();
        else unlistenRaycast = uRay;

        const uPheromone = await listen<any>('pheromone-update', (event) => {
          if (active && event.payload) {
            pheromoneGridRef.current = event.payload;
            draw();
          }
        });
        if (!active) uPheromone();
        else unlistenPheromone = uPheromone;
      } catch (err) {
        console.error('Failed to setup Tauri listeners in PixiViewport:', err);
      }

      const tick = () => {
        if (!active) return;
        draw();
        animationFrameId = requestAnimationFrame(tick);
      };
      tick();

      // Load the terrain background asynchronously AFTER graphics, listeners, and the
      // first draw are in place. Terrain loading may retry for seconds (or fail when
      // PIXI.Texture is unavailable, e.g. under test), so it must never block rendering.
      void (async () => {
        let loaded = false;
        for (let retries = 0; active && retries < 20; retries++) {
          try {
            const terrainMap = await invoke<TerrainMapState>('get_terrain_map');
            if (terrainMap && terrainMap.biomes) {
              terrainMapRef.current = terrainMap;
              const canvas = generateTerrainCanvas(terrainMap);
              const texture = PIXI.Texture.from(canvas);
              const bgSprite = new PIXI.Sprite(texture);
              bgSpriteRef.current = bgSprite;
              if (typeof app.stage.addChildAt === 'function') {
                app.stage.addChildAt(bgSprite, 0);
              } else {
                app.stage.addChild(bgSprite);
              }
              loaded = true;
              break; // Stop retrying if successful
            }
          } catch (err) {
            // Backend may not be ready yet (or Texture unavailable) — wait and retry.
            await new Promise(resolve => setTimeout(resolve, 100));
          }
        }

        if (!loaded && active) {
          // Programmatic fallback instead of loading static base_map.png
          const fallbackMap: TerrainMapState = {
            width: 512,
            height: 512,
            biomes: new Array(512 * 512).fill(4), // Grassland (index 4)
            elevations: new Array(512 * 512).fill(0), // Flat terrain
            bounds: { min: { x: -100, y: -100, z: -100 }, max: { x: 100, y: 100, z: 100 } },
            pois: [],
          };
          try {
            const canvas = generateTerrainCanvas(fallbackMap);
            const texture = PIXI.Texture.from(canvas);
            const bgSprite = new PIXI.Sprite(texture);
            bgSpriteRef.current = bgSprite;
            terrainMapRef.current = fallbackMap;
            if (typeof app.stage.addChildAt === 'function') {
              app.stage.addChildAt(bgSprite, 0);
            } else {
              app.stage.addChild(bgSprite);
            }
          } catch (err: any) {
            if (err && err.message && err.message.includes('No "Texture" export')) {
              // Suppress Vitest mock warning
            } else {
              console.error('Failed to initialize fallback terrain map texture:', err);
            }
          }
        }

        if (active) draw();
      })();
    };

    initPixi();

    return () => {
      active = false;
      cancelAnimationFrame(animationFrameId);
      if (unlistenTick) unlistenTick();
      if (unlistenRaycast) unlistenRaycast();
      if (unlistenPheromone) unlistenPheromone();
      if (appRef.current) {
        appRef.current.destroy(true);
      }
    };
  }, []);

  useEffect(() => {
    draw();
  }, [propSegments, propRaycasts, propPheromoneGrid, projection, propEnvironmentalState, zoom, pan]);

  // Simulation LOD: tell the backend where this viewport is looking, so it can spend its per-tick
  // brain inference there instead of uniformly (`core/simulation_lod.rs`).
  //
  // This view *draws the agents*, which makes it stricter than the landscape showcase: a focus that
  // is wrong here shows up as agents moving sluggishly. Two guards carry that, both inside
  // `focusForViewport`: the hot radius is asked of the backend rather than assumed, and no focus is
  // sent while the viewport shows more world than that radius covers — zoomed out, every agent on
  // screen is one the user is looking at, and uniform detail is the correct answer.
  useEffect(() => {
    let hotRadius: number | null = null;
    let last: LodFocusPayload | null = null;
    let inFlight = false;
    let stopped = false;

    void fetchHotRadius().then((r) => {
      if (!stopped) hotRadius = r;
    });

    const id = setInterval(() => {
      if (inFlight) return;
      const v = lodViewRef.current;
      const next = v
        ? focusForViewport(v.x, v.z, v.visibleHalfExtent, hotRadius)
        : FOCUS_OFF;
      if (!shouldSend(last, next)) return;
      inFlight = true;
      void sendLodFocus(next).then((ok) => {
        inFlight = false;
        if (ok) last = next;
      });
    }, SAMPLE_INTERVAL_MS);

    // Leaving must hand detail back. React's cleanup does not run for a document navigation, so
    // `pagehide` covers that path, and the send is synchronous because a dynamic import does not
    // resolve on a page being torn down.
    const leave = () => {
      sendLodFocusNow(FOCUS_OFF);
    };
    window.addEventListener('pagehide', leave);
    return () => {
      stopped = true;
      clearInterval(id);
      window.removeEventListener('pagehide', leave);
      leave();
    };
  }, []);

  return (
    <div
      ref={containerRef}
      data-testid="pixi-canvas-container"
      style={{
        border: '2px solid #27272a',
        borderRadius: '15px',
        backgroundColor: '#18181b',
        backgroundImage: `
          linear-gradient(to right, #27272a 1px, transparent 1px),
          linear-gradient(to bottom, #27272a 1px, transparent 1px)
        `,
        backgroundSize: '32px 32px',
        width: '100%',
        height: '350px',
        boxSizing: 'border-box',
        overflow: 'hidden'
      }}
    />
  );
};

export default PixiViewport;

