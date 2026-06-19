import React, { useEffect, useRef } from 'react';
import * as PIXI from 'pixi.js';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

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
  const bgTilingSpriteRef = useRef<any>(null);

  const segmentsRef = useRef<any[]>([]);
  const raycastsRef = useRef<any[]>([]);
  const pheromoneGridRef = useRef<any>(null);
  const projectionRef = useRef<'xy' | 'xz'>(projection);
  const environmentalStateRef = useRef<any>({ elements: [] });

  const zoomRef = useRef<number>(zoom);
  const panRef = useRef<{ x: number; y: number }>(pan);

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

    let currentScale = 1.0;
    let minX = -100, maxX = 100;
    let minY = -100, maxY = 100;
    let scale = 1.0;
    let midX = 0;
    let midY = 0;
    let hasSegments = false;

    if (Array.isArray(segments) && segments.length > 0) {
      hasSegments = true;
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
      currentScale = scale;
      midX = (minX + maxX) / 2;
      midY = (minY + maxY) / 2;
    }

    const getCoordsNoPan = (x: number, y: number): [number, number] => {
      if (hasSegments) {
        const centerX = 500 / 2;
        const centerY = 350 / 2;
        const cx = centerX + (x - midX) * scale;
        const cy = centerY - (y - midY) * scale;
        return [cx, cy];
      }
      return [x, y];
    };

    const getCoords = (x: number, y: number): [number, number] => {
      const [cx, cy] = getCoordsNoPan(x, y);
      return [cx * zoomRef.current + panRef.current.x, cy * zoomRef.current + panRef.current.y];
    };

    const screenToWorld = (sx: number, sy: number): [number, number] => {
      const cx = (sx - panRef.current.x) / zoomRef.current;
      const cy = (sy - panRef.current.y) / zoomRef.current;
      if (hasSegments) {
        const centerX = 500 / 2;
        const centerY = 350 / 2;
        const wx = midX + (cx - centerX) / scale;
        const wy = midY - (cy - centerY) / scale;
        return [wx, wy];
      }
      return [cx, cy];
    };

    // Draw Terrain Background Grid taking pan & zoom into account
    if (bgTilingSpriteRef.current) {
      bgTilingSpriteRef.current.tileScale.set(zoomRef.current);
      bgTilingSpriteRef.current.tilePosition.set(panRef.current.x, panRef.current.y);
    } else {
      beginFill(graphics, 0x09090b, 1.0); // Soft dark HUD background fallback
      graphics.drawRect(0, 0, 500, 350);
      endFill(graphics);
    }

    const gridSize = 40;
    const startX = Math.floor((-panRef.current.x) / (gridSize * zoomRef.current)) * gridSize;
    const endX = startX + (500 / zoomRef.current) + gridSize * 2;
    const gridAlpha = Math.max(0.1, Math.min(0.3, 0.2 / zoomRef.current));
    lineStyle(graphics, 0.5, 0xffffff, gridAlpha);
    for (let gx = startX; gx <= endX; gx += gridSize) {
      const screenX = gx * zoomRef.current + panRef.current.x;
      graphics.moveTo(screenX, 0);
      graphics.lineTo(screenX, 350);
    }
    const startY = Math.floor((-panRef.current.y) / (gridSize * zoomRef.current)) * gridSize;
    const endY = startY + (350 / zoomRef.current) + gridSize * 2;
    for (let gy = startY; gy <= endY; gy += gridSize) {
      const screenY = gy * zoomRef.current + panRef.current.y;
      graphics.moveTo(0, screenY);
      graphics.lineTo(500, screenY);
    }

    // 1. Draw Pheromone Grid heatmap
    if (pheromoneGrid && pheromoneGrid.grid) {
      const { grid, width, height } = pheromoneGrid;
      if (width > 0 && height > 0 && Array.isArray(grid)) {
        const cellW = 500 / width;
        const cellH = 350 / height;
        grid.forEach((val: number, idx: number) => {
          if (val > 0) {
            const x = idx % width;
            const y = Math.floor(idx / width);
            beginFill(graphics, 0xffffff, val * 0.45);
            const rx = x * cellW * zoomRef.current + panRef.current.x;
            const ry = y * cellH * zoomRef.current + panRef.current.y;
            const rw = cellW * zoomRef.current;
            const rh = cellH * zoomRef.current;
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

      // Create procedural tile texture
      let hasTexture = false;
      let hasTilingSprite = false;
      try {
        if ((PIXI as any).Texture !== undefined) {
          hasTexture = true;
        }
      } catch (e) {}
      try {
        if ((PIXI as any).TilingSprite !== undefined) {
          hasTilingSprite = true;
        }
      } catch (e) {}

      if (hasTexture && hasTilingSprite) {
        const tileCanvas = document.createElement('canvas');
        tileCanvas.width = 64;
        tileCanvas.height = 64;
        const ctx = tileCanvas.getContext('2d');
        if (ctx) {
          ctx.fillStyle = '#121212'; // dark grayscale base
          ctx.fillRect(0, 0, 64, 64);
          
          ctx.strokeStyle = '#1c1c1c'; // Highlight bevels (top-left)
          ctx.lineWidth = 2;
          ctx.beginPath();
          ctx.moveTo(0, 64);
          ctx.lineTo(0, 0);
          ctx.lineTo(64, 0);
          ctx.stroke();

          ctx.strokeStyle = '#080808'; // Shadow borders (bottom-right)
          ctx.beginPath();
          ctx.moveTo(64, 0);
          ctx.lineTo(64, 64);
          ctx.lineTo(0, 64);
          ctx.stroke();

          ctx.strokeStyle = '#0f0f0f'; // Draw sub-tile bricks / seams
          ctx.lineWidth = 1.5;
          ctx.beginPath();
          ctx.moveTo(0, 32);
          ctx.lineTo(64, 32);
          ctx.moveTo(32, 0);
          ctx.lineTo(32, 32);
          ctx.moveTo(16, 32);
          ctx.lineTo(16, 64);
          ctx.stroke();
        }

        const tileTexture = (PIXI as any).Texture.from(tileCanvas);
        const bgTilingSprite = new (PIXI as any).TilingSprite({
          texture: tileTexture,
          width: 500,
          height: 350
        });
        bgTilingSpriteRef.current = bgTilingSprite;
        app.stage.addChild(bgTilingSprite);
      }

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
        if (Array.isArray(segments) && segments.length > 0) {
          let minX = Infinity, maxX = -Infinity;
          let minY = Infinity, maxY = -Infinity;

          segments.forEach((s) => {
            if (!s) return;
            const xVal = s.x;
            const yVal = projectionRef.current === 'xy' ? s.y : s.z;
            if (xVal < minX) minX = xVal;
            if (xVal > maxX) maxX = xVal;
            if (yVal < minY) minY = yVal;
            if (yVal > maxY) maxY = yVal;
          });

          const rangeX = maxX - minX || 1;
          const rangeY = maxY - minY || 1;
          const padding = 50;
          const drawWidth = 500 - padding * 2;
          const drawHeight = 350 - padding * 2;

          const scale = Math.min(drawWidth / rangeX, drawHeight / rangeY);
          const centerX = 500 / 2;
          const centerY = 350 / 2;
          const midX = (minX + maxX) / 2;
          const midY = (minY + maxY) / 2;

          const cx = centerX + (x - midX) * scale;
          const cy = centerY - (y - midY) * scale;
          return [cx, cy];
        }
        return [x, y];
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
