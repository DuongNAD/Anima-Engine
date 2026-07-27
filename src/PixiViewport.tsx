import React, { useCallback, useEffect, useRef } from 'react';
import * as PIXI from 'pixi.js';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { TerrainMapState } from './types';
// The IPC payloads this viewport draws, from the generated bindings rather than `any`. Each one was
// `any` at both ends — the ref that holds it and the `invoke`/`listen` that fills it — so a renamed
// Rust field would have surfaced as a blank canvas rather than a type error.
import type { SegmentState } from './types/generated/SegmentState';
import type { RaycastTelemetry } from './types/generated/RaycastTelemetry';
import type { PheromoneGridState } from './types/generated/PheromoneGridState';
import type { EnvironmentalState } from './types/generated/EnvironmentalState';
import type { EnvironmentalElement } from './types/generated/EnvironmentalElement';
import type { SimulationTickPayload } from './types/generated/SimulationTickPayload';
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
  } catch {
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
  segments?: SegmentState[] | null;
  raycasts?: RaycastTelemetry[] | null;
  pheromoneGrid?: { grid: number[]; width: number; height: number } | null;
  environmentalState?: EnvironmentalState | null;
  zoom?: number;
  pan?: { x: number; y: number };
}

// ---------------------------------------------------------------------------------------
// Graphics adapter: pixi 8's API, expressed in the call shape this file already uses.
//
// # Why an adapter and not a rewrite
//
// These helpers used to prefer `beginFill` / `endFill` / `lineStyle` and only fall back to the
// modern methods. pixi 8.19 still *has* the old ones — as deprecation stubs — so every redraw
// emitted a wall of warnings, once per call, at 30 Hz. Fresh Playwright output showed them at
// `beginFill` (~144), `endFill` (~154) and `lineStyle` (~159).
//
// The two APIs are not a rename. v7 is stateful — set a fill, draw shapes, close it — while v8 is
// path-then-style: record shapes, then `fill(...)` / `stroke(...)` applies to what was recorded.
// A straight swap would reverse the order of every drawing call in this file.
//
// So the adapter keeps the v7 *call shape* and defers: `beginFill` and `lineStyle` remember a
// style, the shape helpers record geometry, and `endFill` (or `strokePath`, for open paths that
// never had a fill) applies both. The `dirty` set is what makes nested strokes work — the
// minimap's three-ring border sets a new `lineStyle` between rects, and without flushing at that
// point all three rings would come out at whatever width happened to be set last.
//
// The v7 branch is kept because both Vitest configs mock `pixi.js`, and the mock implements the
// v7 surface. It is not dead code; it is the path the unit tests take.
// ---------------------------------------------------------------------------------------

type FillStyle = { color: number; alpha?: number };
type StrokeStyle = { width: number; color: number; alpha?: number };

/**
 * The union of the two Graphics APIs this adapter bridges.
 *
 * Every method is optional because no single object has all of them: real pixi 8 has
 * `fill`/`stroke`/`rect`/`circle`/`poly`, the v7-shaped test mock has
 * `beginFill`/`endFill`/`lineStyle`/`drawRect`/`drawCircle`/`drawPolygon`, and the helpers below
 * branch on which. That is exactly what `isModernGraphics` asks, so an optional-member interface is
 * the honest shape — `any` said nothing and let a typo in either branch through.
 */
interface GraphicsLike {
  // pixi 8
  fill?: (style: FillStyle) => void;
  stroke?: (style: StrokeStyle) => void;
  rect?: (x: number, y: number, w: number, h: number) => void;
  circle?: (x: number, y: number, r: number) => void;
  poly?: (points: number[]) => void;
  // pixi 7
  beginFill?: (color: number, alpha?: number) => void;
  endFill?: () => void;
  lineStyle?: (width: number, color: number, alpha?: number) => void;
  drawRect?: (x: number, y: number, w: number, h: number) => void;
  drawCircle?: (x: number, y: number, r: number) => void;
  drawPolygon?: (points: number[]) => void;
  // both
  moveTo?: (x: number, y: number) => void;
  lineTo?: (x: number, y: number) => void;
  clear?: () => void;
}

const pendingFill = new WeakMap<object, FillStyle>();
const pendingStroke = new WeakMap<object, StrokeStyle>();
/** Graphics with geometry recorded since the last style application. */
const dirty = new WeakSet<object>();

/** True for the real pixi 8 Graphics; false for the v7-shaped test mock. */
const isModernGraphics = (g: GraphicsLike): boolean =>
  typeof g?.fill === 'function' && typeof g?.rect === 'function';

/** Apply whatever styles are pending to the geometry recorded so far. */
const flushStyles = (g: GraphicsLike) => {
  const fill = pendingFill.get(g);
  if (fill) {
    g.fill?.(fill);
    pendingFill.delete(g);
  }
  const stroke = pendingStroke.get(g);
  if (stroke) {
    g.stroke?.(stroke);
    pendingStroke.delete(g);
  }
  dirty.delete(g);
};

const beginFill = (g: GraphicsLike, color: number, alpha?: number) => {
  if (isModernGraphics(g)) {
    if (dirty.has(g)) flushStyles(g);
    pendingFill.set(g, { color, alpha });
  } else if (typeof g.beginFill === 'function') {
    g.beginFill(color, alpha);
  }
};

const lineStyle = (g: GraphicsLike, width: number, color: number, alpha?: number) => {
  if (isModernGraphics(g)) {
    if (dirty.has(g)) flushStyles(g);
    pendingStroke.set(g, { width, color, alpha });
  } else if (typeof g.lineStyle === 'function') {
    g.lineStyle(width, color, alpha);
  }
};

const endFill = (g: GraphicsLike) => {
  if (isModernGraphics(g)) {
    flushStyles(g);
  } else if (typeof g.endFill === 'function') {
    g.endFill();
  }
};

/**
 * Close an open stroked path — a `lineStyle` followed by `moveTo`/`lineTo` with no fill.
 *
 * v7 drew those as the calls were made, so they needed no closing call. v8 needs the `stroke()`
 * that applies them, and without it the grid, the raycast beams and the segment linkages simply
 * would not appear.
 */
const strokePath = (g: GraphicsLike) => {
  if (isModernGraphics(g)) flushStyles(g);
};

const drawRect = (g: GraphicsLike, x: number, y: number, w: number, h: number) => {
  if (isModernGraphics(g)) {
    g.rect?.(x, y, w, h);
    dirty.add(g);
  } else {
    g.drawRect?.(x, y, w, h);
  }
};

const drawCircle = (g: GraphicsLike, x: number, y: number, r: number) => {
  if (isModernGraphics(g)) {
    g.circle?.(x, y, r);
    dirty.add(g);
  } else {
    g.drawCircle?.(x, y, r);
  }
};

const drawPolygon = (g: GraphicsLike, points: number[]) => {
  if (isModernGraphics(g)) {
    g.poly?.(points);
    dirty.add(g);
  } else {
    g.drawPolygon?.(points);
  }
};

const moveTo = (g: GraphicsLike, x: number, y: number) => {
  g.moveTo?.(x, y);
  if (isModernGraphics(g)) dirty.add(g);
};

const lineTo = (g: GraphicsLike, x: number, y: number) => {
  g.lineTo?.(x, y);
  if (isModernGraphics(g)) dirty.add(g);
};

/** Clear the canvas and any style this adapter was holding for it. */
const clearGraphics = (g: GraphicsLike) => {
  pendingFill.delete(g);
  pendingStroke.delete(g);
  dirty.delete(g);
  g.clear?.();
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


  const segmentsRef = useRef<SegmentState[]>([]);
  const raycastsRef = useRef<RaycastTelemetry[]>([]);
  const pheromoneGridRef = useRef<PheromoneGridState | null>(null);
  const projectionRef = useRef<'xy' | 'xz'>(projection);
  const environmentalStateRef = useRef<EnvironmentalState>({ elements: [] });

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

  const draw = useCallback(() => {
    const graphics = graphicsRef.current;
    if (!graphics) return;

    clearGraphics(graphics);

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
      drawRect(graphics, 0, 0, 500, 350);
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
      moveTo(graphics, screenVX1, screenVY1);
      lineTo(graphics, screenVX2, screenVY2);
      
      // Horizontal lines
      const hy = minY + i * gridSizeY;
      const [screenHX1, screenHY1] = getCoords(minX, hy);
      const [screenHX2, screenHY2] = getCoords(maxX, hy);
      moveTo(graphics, screenHX1, screenHY1);
      lineTo(graphics, screenHX2, screenHY2);
    }
    // After the loop, not inside it: `lineStyle` is set once above, and `strokePath` consumes the
    // pending style. Stroking per iteration would draw the first grid line and then, with nothing
    // pending, silently draw none of the other thirty-one. v8's `stroke()` applies to every
    // subpath recorded since the last style, so one call here is both correct and cheaper.
    strokePath(graphics);

    // 0.5. Draw POI markers on top of the background
    if (terrainMap && Array.isArray(terrainMap.pois)) {
      terrainMap.pois.forEach((poi: [number, number]) => {
        // Assume poi is [px, py] coordinate on the 1024x1024 map. 
        // We need to map [0, 1024] to [minX, maxX]
        const px = poi[0];
        const py = poi[1];
        const wx = minX + (px / (terrainMap.width || 1024)) * (maxX - minX);
        const wy = maxY - (py / (terrainMap.height || 1024)) * (maxY - minY); // invert Y
        const [cx, cy] = getCoords(wx, wy);

        // Draw a nice blue icon with a white border
        beginFill(graphics, 0xffffff, 1.0); // white border
        drawCircle(graphics, cx, cy, 5 * zoomRef.current);
        endFill(graphics);
        
        beginFill(graphics, 0x1d9bf0, 1.0); // blue center
        drawCircle(graphics, cx, cy, 3.5 * zoomRef.current);
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
            drawRect(graphics, rx, ry, rw, rh);
            endFill(graphics);
          }
        });
      }
    }

    // 2. Draw environmental elements (Lakes & Trees with animations)
    const time = performance.now();
    if (environmentalState && Array.isArray(environmentalState.elements)) {
      environmentalState.elements.forEach((elem: EnvironmentalElement) => {
        if (!elem) return;
        // `proj === 'xy' ? elem.y : elem.z` — and `EnvironmentalElement` has no `z`. The Rust struct
        // is `{ type, x, y, radius, resources }` with `y` commented "Maps to Bevy's z coordinate",
        // so these are ground-plane features carrying no height at all. In the `xz` projection the
        // old expression read `undefined`, `getCoords` returned NaN, and every lake and tree was
        // drawn nowhere. The `any` on this callback is what let it compile.
        const [cx, cy] = getCoords(elem.x, elem.y);

        if (elem.type === 'lake') {
          // Ripple wave effect
          const waveOffset = Math.sin(time * 0.003 + elem.x * 0.1) * 3;
          const radius = (elem.radius + waveOffset) * currentScale * zoomRef.current;

          beginFill(graphics, 0xcccccc, 0.1);
          drawCircle(graphics, cx, cy, radius);
          endFill(graphics);

          beginFill(graphics, 0xcccccc, 0.15);
          drawCircle(graphics, cx, cy, radius * 0.7);
          endFill(graphics);

          beginFill(graphics, 0xcccccc, 0.2);
          drawCircle(graphics, cx, cy, radius * 0.4);
          endFill(graphics);
        } else {
          // Trees: botanical assets
          const treeSize = ((elem.resources / 100.0) * 12 + 10) * currentScale * zoomRef.current;

          beginFill(graphics, 0x444444, 0.9); // Trunk
          drawRect(graphics, cx - 3 * zoomRef.current, cy, 6 * zoomRef.current, treeSize * 0.8);
          endFill(graphics);

          beginFill(graphics, 0x888888, 0.85); // Canopy leaves
          drawCircle(graphics, cx, cy - treeSize * 0.4, treeSize * 0.8);
          endFill(graphics);
          
          beginFill(graphics, 0xaaaaaa, 0.8);
          drawCircle(graphics, cx - treeSize * 0.3, cy - treeSize * 0.2, treeSize * 0.6);
          drawCircle(graphics, cx + treeSize * 0.3, cy - treeSize * 0.2, treeSize * 0.6);
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
          moveTo(graphics, scx, scy);
          lineTo(graphics, ecx, ecy);
          strokePath(graphics);

          // Draw small hit-point marker
          if (r.hit_entity_type !== 'None') {
            beginFill(graphics, 0xffffff, 0.9);
            drawCircle(graphics, ecx, ecy, 3 * zoomRef.current);
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
            moveTo(graphics, px, py);
            lineTo(graphics, cx, cy);
            strokePath(graphics);
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
          drawPolygon(graphics, [p1_x, p1_y, p2_x, p2_y, p3_x, p3_y]);
          endFill(graphics);

          // Head arrow indicator line
          lineStyle(graphics, 2 * zoomRef.current, 0xffffff, 0.7);
          moveTo(graphics, cx, cy);
          lineTo(graphics, cx + Math.cos(angle) * predSize * 1.4, cy + Math.sin(angle) * predSize * 1.4);
          strokePath(graphics);
        } else if (s.agent_type === 'prey') {
          const preySize = 10 * zoomRef.current;
          beginFill(graphics, 0x777777, opacity);
          drawCircle(graphics, cx, cy, preySize);
          endFill(graphics);

          // Tail indicator pointing backward
          lineStyle(graphics, 2.5 * zoomRef.current, 0x777777, opacity * 0.8);
          moveTo(graphics, cx, cy);
          lineTo(graphics, cx - Math.cos(angle) * preySize * 1.6, cy - Math.sin(angle) * preySize * 1.6);
          strokePath(graphics);

          // Prey hydration bar
          if (s.hydration !== undefined) {
            const barW = 16 * zoomRef.current;
            const barH = 3 * zoomRef.current;
            const bx = cx - barW / 2;
            const by = cy - preySize - 6 * zoomRef.current;

            beginFill(graphics, 0x333333, 0.85); // Backing dark gray
            drawRect(graphics, bx, by, barW, barH);
            endFill(graphics);

            beginFill(graphics, 0xdddddd, 0.95); // Hydration level light gray
            drawRect(graphics, bx, by, barW * (s.hydration / 100.0), barH);
            endFill(graphics);
          }
        } else {
          const isRoot = s.parent_segment_id === null || s.parent_segment_id === undefined;
          const color = isRoot ? 0x888888 : 0xaaaaaa;
          beginFill(graphics, color, opacity);
          drawCircle(graphics, cx, cy, 10 * zoomRef.current);
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
    drawRect(graphics, mmX, mmY, mmW, mmH);
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
      environmentalState.elements.forEach((elem: EnvironmentalElement) => {
        // Same as the main pass above: the payload has no `z`, and `y` already is the world z.
        const [mx, my] = getMinimapCoords(elem.x, elem.y);
        beginFill(graphics, elem.type === 'lake' ? 0xcccccc : 0x888888, 0.9);
        drawCircle(graphics, mx, my, 3.5);
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
          drawCircle(graphics, mx, my, 2.5);
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
    drawRect(graphics, bx - 2, by - 2, bw + 4, bh + 4);

    lineStyle(graphics, 5, 0xaaaaaa, 0.35); // Medium glow border
    drawRect(graphics, bx - 1, by - 1, bw + 2, bh + 2);

    lineStyle(graphics, 2.5, 0xffffff, 0.8); // Core thick border
    drawRect(graphics, bx, by, bw, bh);
    endFill(graphics);
    // The four payload props, and nothing else. Everything else this reads is a ref — the Pixi
    // objects, the live view, and the event-driven fallbacks each prop overrides when supplied.
  }, [propSegments, propRaycasts, propPheromoneGrid, propEnvironmentalState]);

  // Sync the refs `draw` reads from the props, then repaint.
  //
  // One effect rather than three ref-syncs and a separate repaint, because the order between them is
  // the point. `draw` reads `projectionRef` / `zoomRef` / `panRef` rather than the props themselves,
  // and that is not an oversight: the wheel and double-click handlers registered at mount write
  // those refs directly and repaint immediately, without going through a React render. The refs are
  // the live view; the props are one of the two things that change it.
  //
  // Doing both here makes the ordering explicit instead of a consequence of which `useEffect` was
  // written first — and it gives this effect a dependency list where every entry is genuinely read,
  // which the previous arrangement could not have (`draw` was rebuilt every render, so naming it
  // would have repainted on every render, and the payload props it reads were named without being
  // referenced).
  useEffect(() => {
    projectionRef.current = projection;
    zoomRef.current = zoom;
    panRef.current = pan;
    draw();
  }, [draw, projection, zoom, pan]);

  // The latest `draw`, for the mount-only effect below.
  //
  // That effect creates the Pixi application, its canvas listeners and the Tauri subscriptions once;
  // naming `draw` in its dependency list would tear the renderer down and rebuild it every time a
  // payload arrived. But the listeners it registers still have to call the *current* `draw`, not the
  // one that existed at mount — so the identity it needs is stable and the value it needs is fresh,
  // which is exactly what a ref is for.
  const drawRef = useRef(draw);
  useEffect(() => {
    drawRef.current = draw;
  }, [draw]);

  // Same reasoning for the segments the hit-test reads. This one was also a live defect: the
  // double-click handler closed over the segments the component was given at mount, so once a
  // parent started supplying them, "centre on the nearest agent" aimed at where the agents were the
  // first time the viewport rendered.
  const propSegmentsRef = useRef(propSegments);
  useEffect(() => {
    propSegmentsRef.current = propSegments;
  }, [propSegments]);

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
        drawRef.current();
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
        drawRef.current();
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

        const segments =
          propSegmentsRef.current !== undefined ? propSegmentsRef.current : segmentsRef.current;
        let nearestSegment: SegmentState | null = null;
        let minDist = Infinity;

        // A plain loop rather than `forEach`, so the assignment below is one TypeScript can see.
        // Narrowing does not survive a callback: with `forEach`, `nearestSegment` stayed `null` as
        // far as the checker was concerned and every read of it after this block was an error.
        if (Array.isArray(segments)) {
          for (const s of segments) {
            if (!s) continue;
            const yVal = projectionRef.current === 'xy' ? s.y : s.z;
            const [cx, cy] = getCoordsLocal(s.x, yVal);
            const dist = Math.hypot(mouseX - cx, mouseY - cy);
            if (dist < minDist) {
              minDist = dist;
              nearestSegment = s;
            }
          }
        }

        if (nearestSegment && minDist < 100) {
          const currentY = projectionRef.current === 'xy' ? nearestSegment.y : nearestSegment.z;
          const [cxNoPan, cyNoPan] = getCoordsNoPan(nearestSegment.x, currentY);
          panRef.current = {
            x: 250 - cxNoPan * zoomRef.current,
            y: 175 - cyNoPan * zoomRef.current
          };
          drawRef.current();
        }
      };
      canvasElement.addEventListener('dblclick', onDblClick);

      const getCoordsNoPan = (x: number, y: number): [number, number] => {
        const segments =
          propSegmentsRef.current !== undefined ? propSegmentsRef.current : segmentsRef.current;
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
        const env = await invoke<EnvironmentalState>('get_environmental_elements');
        if (active && env) environmentalStateRef.current = env;
      } catch {}

      try {
        const grid = await invoke<PheromoneGridState>('get_pheromone_grid');
        if (active && grid) pheromoneGridRef.current = grid;
      } catch {}

      try {
        const raycasts = await invoke<RaycastTelemetry[]>('get_active_raycasts');
        if (active && raycasts) raycastsRef.current = raycasts;
      } catch {}

      try {
        // `simulation-tick` carries either the bare segment array or the whole tick payload,
        // depending on the emitter, which is why this narrows rather than trusting one shape.
        const uTick = await listen<SegmentState[] | SimulationTickPayload>(
          'simulation-tick',
          (event) => {
            if (!active) return;
            const payload = event.payload;
            if (Array.isArray(payload)) {
              segmentsRef.current = payload;
            } else if (payload && typeof payload === 'object') {
              segmentsRef.current = Array.isArray(payload.segments) ? payload.segments : [];
              if (payload.environmental_state) {
                environmentalStateRef.current = payload.environmental_state;
              }
            } else {
              segmentsRef.current = [];
            }
            drawRef.current();
          },
        );
        if (!active) uTick();
        else unlistenTick = uTick;

        const uRay = await listen<RaycastTelemetry[]>('raycast-update', (event) => {
          if (active) {
            raycastsRef.current = Array.isArray(event.payload) ? event.payload : [];
            drawRef.current();
          }
        });
        if (!active) uRay();
        else unlistenRaycast = uRay;

        const uPheromone = await listen<PheromoneGridState>('pheromone-update', (event) => {
          if (active && event.payload) {
            pheromoneGridRef.current = event.payload;
            drawRef.current();
          }
        });
        if (!active) uPheromone();
        else unlistenPheromone = uPheromone;
      } catch (err) {
        console.error('Failed to setup Tauri listeners in PixiViewport:', err);
      }

      const tick = () => {
        if (!active) return;
        drawRef.current();
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
          } catch {
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
          } catch (err: unknown) {
            const message = err instanceof Error ? err.message : String(err);
            if (message.includes('No "Texture" export')) {
              // Suppress Vitest mock warning
            } else {
              console.error('Failed to initialize fallback terrain map texture:', err);
            }
          }
        }

        if (active) drawRef.current();
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

