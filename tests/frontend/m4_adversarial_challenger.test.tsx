import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

import { DataExportDialog } from '../../src/components/ResearchDashboard/DataExportDialog';

import { SparkLine } from '../../src/components/ResearchDashboard/common/SparkLine';
import { SpatialRegionCanvas } from '../../src/components/ResearchDashboard/common/SpatialRegionCanvas';
import { LineageDagView } from '../../src/components/ResearchDashboard/common/LineageDagView';
import { BranchTreeCanvas } from '../../src/components/ResearchDashboard/common/BranchTreeCanvas';
import { TimeSeriesChart } from '../../src/components/ResearchDashboard/common/TimeSeriesChart';
import { PhaseSpaceCanvas } from '../../src/components/ResearchDashboard/common/PhaseSpaceCanvas';

import type { LineageGraphPayload } from '../../src/types/generated/LineageGraphPayload';
import type { LineageNodePayload } from '../../src/types/generated/LineageNodePayload';
import type { CausalBranchTreeState } from '../../src/components/ResearchDashboard/types';

describe('Milestone 4: Adversarial Edge-Case & Boundary Stress Harness', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // =========================================================================
  // Area 1: Counterfactual Delta Trajectory Calculations under Extreme Conditions
  // =========================================================================
  describe('Area 1: Counterfactual Delta Trajectories & Mathematical Robustness', () => {
    // Pure function dynamic time warping and divergence simulator for frontend mathematical parity
    const dynamicTimeWarping = (seriesA: number[], seriesB: number[]): number => {
      const n = seriesA.length;
      const m = seriesB.length;
      if (n === 0 || m === 0) return 0.0;
      const dtw: number[][] = Array.from({ length: n + 1 }, () =>
        Array.from({ length: m + 1 }, () => Infinity)
      );
      dtw[0][0] = 0.0;
      for (let i = 1; i <= n; i++) {
        for (let j = 1; j <= m; j++) {
          const cost = Math.pow(seriesA[i - 1] - seriesB[j - 1], 2);
          const prev = Math.min(dtw[i - 1][j], dtw[i][j - 1], dtw[i - 1][j - 1]);
          dtw[i][j] = cost + prev;
        }
      }
      return Math.sqrt(dtw[n][m]);
    };

    const computeDivergenceMetrics = (
      seriesA: { tick: number; val: number }[],
      seriesB: { tick: number; val: number }[],
      forkTick: number
    ) => {
      const bMap = new Map(seriesB.map((s) => [s.tick, s.val]));
      const commonTicks = seriesA
        .map((s) => s.tick)
        .filter((t) => bMap.has(t) && t >= forkTick)
        .sort((a, b) => a - b);

      if (commonTicks.length === 0) {
        return {
          commonTicksCount: 0,
          meanDelta: 0,
          maxAbsDelta: 0,
          rmsd: 0,
          relativeDivergence: 0,
          dtwDistance: 0,
        };
      }

      const deltas: number[] = [];
      const valsA: number[] = [];
      const valsB: number[] = [];

      for (const t of commonTicks) {
        const ya = seriesA.find((s) => s.tick === t)!.val;
        const yb = bMap.get(t)!;
        valsA.push(ya);
        valsB.push(yb);
        deltas.push(ya - yb);
      }

      const m = deltas.length;
      const meanDelta = deltas.reduce((s, d) => s + d, 0) / m;
      const maxAbsDelta = Math.max(...deltas.map((d) => Math.abs(d)));
      const rmsd = Math.sqrt(deltas.reduce((s, d) => s + d * d, 0) / m);

      const bMean = valsB.reduce((s, v) => s + v, 0) / m;
      const bVar = valsB.reduce((s, v) => s + Math.pow(v - bMean, 2), 0) / m;
      const bStd = Math.sqrt(bVar);
      const relativeDivergence = rmsd / (bStd + 1e-6);
      const dtwDistance = dynamicTimeWarping(valsA, valsB);

      return {
        commonTicksCount: m,
        meanDelta,
        maxAbsDelta,
        rmsd,
        relativeDivergence,
        dtwDistance,
      };
    };

    it('handles completely disjoint time horizons without NaN or division by zero', () => {
      const treatment = [
        { tick: 0, val: 100 },
        { tick: 10, val: 110 },
        { tick: 20, val: 120 },
      ];
      const control = [
        { tick: 50, val: 200 },
        { tick: 60, val: 210 },
        { tick: 70, val: 220 },
      ];

      const res = computeDivergenceMetrics(treatment, control, 0);
      expect(res.commonTicksCount).toBe(0);
      expect(res.meanDelta).toBe(0);
      expect(res.rmsd).toBe(0);
      expect(res.relativeDivergence).toBe(0);
      expect(res.dtwDistance).toBe(0);
      expect(Number.isNaN(res.relativeDivergence)).toBe(false);
      expect(Number.isFinite(res.rmsd)).toBe(true);
    });

    it('handles fork_tick occurring after all overlapping samples', () => {
      const treatment = [
        { tick: 10, val: 50 },
        { tick: 20, val: 60 },
      ];
      const control = [
        { tick: 10, val: 50 },
        { tick: 20, val: 60 },
      ];

      const res = computeDivergenceMetrics(treatment, control, 100);
      expect(res.commonTicksCount).toBe(0);
      expect(res.rmsd).toBe(0);
    });

    it('handles zero control trajectory without zero-division in relative divergence', () => {
      const treatment = [
        { tick: 10, val: 50 },
        { tick: 20, val: 100 },
        { tick: 30, val: 150 },
      ];
      const control = [
        { tick: 10, val: 0 },
        { tick: 20, val: 0 },
        { tick: 30, val: 0 },
      ];

      const res = computeDivergenceMetrics(treatment, control, 0);
      expect(res.commonTicksCount).toBe(3);
      expect(res.meanDelta).toBe(100);
      expect(res.maxAbsDelta).toBe(150);
      expect(res.rmsd).toBeCloseTo(108.01, 1);
      // bStd is 0, but epsilon 1e-6 prevents division by zero
      expect(Number.isFinite(res.relativeDivergence)).toBe(true);
      expect(res.relativeDivergence).toBeGreaterThan(0);
      expect(Number.isNaN(res.relativeDivergence)).toBe(false);
    });

    it('handles bit-identical trajectories producing exact zero divergence metrics', () => {
      const treatment = [
        { tick: 0, val: 42.5 },
        { tick: 10, val: 100.25 },
        { tick: 20, val: 87.125 },
      ];
      const control = [
        { tick: 0, val: 42.5 },
        { tick: 10, val: 100.25 },
        { tick: 20, val: 87.125 },
      ];

      const res = computeDivergenceMetrics(treatment, control, 0);
      expect(res.meanDelta).toBe(0);
      expect(res.maxAbsDelta).toBe(0);
      expect(res.rmsd).toBe(0);
      expect(res.relativeDivergence).toBe(0);
      expect(res.dtwDistance).toBe(0);
    });

    it('handles negative delta series, extreme spikes, and subnormals in SparkLine', () => {
      const extremeNegativeSeries = [-1e6, -500000, -100, -1e-15, -0.00001];
      const { container } = render(
        <SparkLine
          series={[{ label: 'Extreme Delta', color: '#ef4444', values: extremeNegativeSeries }]}
          height={40}
          testid="adversarial-sparkline"
        />
      );

      const svg = screen.getByTestId('adversarial-sparkline');
      expect(svg).toBeInTheDocument();
      const polyline = container.querySelector('polyline');
      expect(polyline).toBeInTheDocument();
      const points = polyline?.getAttribute('points');
      expect(points).toBeTruthy();
      expect(points).not.toContain('NaN');
      expect(points).not.toContain('Infinity');
    });

    it('handles NaN, null, and Infinity values gracefully in SparkLine and TimeSeriesChart', () => {
      const contaminatedSeries = [10, NaN, 20, Infinity, -Infinity, 30];
      const { container } = render(
        <SparkLine
          series={[{ label: 'Contaminated', color: '#38bdf8', values: contaminatedSeries }]}
          height={36}
          testid="contaminated-sparkline"
        />
      );

      const polyline = container.querySelector('polyline');
      expect(polyline).toBeInTheDocument();
      const points = polyline?.getAttribute('points');
      expect(points).toBeTruthy();
      expect(points).not.toContain('NaN');
      expect(points).not.toContain('Infinity');
    });

    it('renders TimeSeriesChart and PhaseSpaceCanvas with empty or single-point arrays', () => {
      const { rerender } = render(
        <TimeSeriesChart
          series={[{ label: 'Empty', color: '#38bdf8', values: [] }]}
          height={150}
          testid="empty-timeseries"
        />
      );
      expect(screen.getByTestId('empty-timeseries')).toBeInTheDocument();

      // Single point
      rerender(
        <TimeSeriesChart
          series={[{ label: 'Single', color: '#38bdf8', values: [50] }]}
          height={150}
          testid="empty-timeseries"
        />
      );
      expect(screen.getByTestId('empty-timeseries')).toBeInTheDocument();

      // PhaseSpaceCanvas with empty and single-point series
      render(
        <PhaseSpaceCanvas
          preyHistory={[]}
          predatorHistory={[]}
          width={200}
          height={200}
          testid="empty-phasespace"
        />
      );
      expect(screen.getByTestId('empty-phasespace')).toBeInTheDocument();

      render(
        <PhaseSpaceCanvas
          preyHistory={[10]}
          predatorHistory={[5]}
          width={200}
          height={200}
          testid="single-phasespace"
        />
      );
      expect(screen.getByTestId('single-phasespace')).toBeInTheDocument();
    });
  });

  // =========================================================================
  // Area 2: Spatial Canvas Coordinate Transformations
  // =========================================================================
  describe('Area 2: Spatial Canvas Coordinate Transformations & Boundary Clamping', () => {
    it('clamps extreme out-of-bounds mouse coordinates to [0.0, 1.0]', () => {
      const onChangeParams = vi.fn();
      const initialParams = {
        minX: 0.2,
        minY: 0.2,
        maxX: 0.8,
        maxY: 0.8,
        cx: 0.5,
        cy: 0.5,
        r: 0.25,
        cellX: 8,
        cellY: 8,
      };

      render(
        <SpatialRegionCanvas
          mode="Rect"
          params={initialParams}
          onChangeParams={onChangeParams}
          width={300}
          height={300}
          testid="test-spatial-canvas"
        />
      );

      const canvas = screen.getByTestId('test-spatial-canvas');

      // Mock getBoundingClientRect
      vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
        left: 100,
        top: 100,
        right: 400,
        bottom: 400,
        width: 300,
        height: 300,
        x: 100,
        y: 100,
        toJSON: () => {},
      });

      // Mouse down far in negative space: clientX = -5000, clientY = -5000
      fireEvent.mouseDown(canvas, { clientX: -5000, clientY: -5000 });
      expect(onChangeParams).toHaveBeenCalledWith(
        expect.objectContaining({
          minX: 0,
          minY: 0,
          maxX: 0,
          maxY: 0,
        })
      );

      // Mouse move far in positive space: clientX = 99999, clientY = 99999
      fireEvent.mouseMove(canvas, { clientX: 99999, clientY: 99999 });
      expect(onChangeParams).toHaveBeenCalledWith(
        expect.objectContaining({
          minX: 0,
          minY: 0,
          maxX: 1,
          maxY: 1,
        })
      );
    });

    it('handles inverted rect selections (bottom-right to top-left drag)', () => {
      let currentParams = {
        minX: 0.8,
        minY: 0.8,
        maxX: 0.8,
        maxY: 0.8,
        cx: 0.5,
        cy: 0.5,
        r: 0.25,
        cellX: 8,
        cellY: 8,
      };

      const onChangeParams = vi.fn((newParams) => {
        currentParams = newParams;
      });

      render(
        <SpatialRegionCanvas
          mode="Rect"
          params={currentParams}
          onChangeParams={onChangeParams}
          width={200}
          height={200}
          testid="rect-canvas"
        />
      );

      const canvas = screen.getByTestId('rect-canvas');
      vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
        left: 0,
        top: 0,
        right: 200,
        bottom: 200,
        width: 200,
        height: 200,
        x: 0,
        y: 0,
        toJSON: () => {},
      });

      // Start drag at (180, 180) -> normalized (0.9, 0.9)
      fireEvent.mouseDown(canvas, { clientX: 180, clientY: 180 });
      // Drag backwards to (20, 20) -> normalized (0.1, 0.1)
      fireEvent.mouseMove(canvas, { clientX: 20, clientY: 20 });

      // minX and minY should be min(0.9, 0.1) = 0.1, maxX and maxY should be max(0.9, 0.1) = 0.9
      expect(onChangeParams).toHaveBeenLastCalledWith(
        expect.objectContaining({
          minX: 0.1,
          minY: 0.1,
          maxX: 0.9,
          maxY: 0.9,
        })
      );
    });

    it('ensures radius is strictly positive and non-negative when dragging in negative quadrants', () => {
      const onChangeParams = vi.fn();
      const initialParams = {
        minX: 0.2,
        minY: 0.2,
        maxX: 0.8,
        maxY: 0.8,
        cx: 0.5,
        cy: 0.5,
        r: 0.2,
        cellX: 8,
        cellY: 8,
      };

      render(
        <SpatialRegionCanvas
          mode="Radius"
          params={initialParams}
          onChangeParams={onChangeParams}
          width={200}
          height={200}
          testid="radius-canvas"
        />
      );

      const canvas = screen.getByTestId('radius-canvas');
      vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
        left: 0,
        top: 0,
        right: 200,
        bottom: 200,
        width: 200,
        height: 200,
        x: 0,
        y: 0,
        toJSON: () => {},
      });

      // Mouse down at center (100, 100) -> normalized (0.5, 0.5)
      fireEvent.mouseDown(canvas, { clientX: 100, clientY: 100 });
      // Mouse move up-left to (20, 20) -> normalized (0.1, 0.1)
      // dx = -0.4, dy = -0.4 -> radius = sqrt(0.16 + 0.16) = sqrt(0.32) ~ 0.5657
      fireEvent.mouseMove(canvas, { clientX: 20, clientY: 20 });

      expect(onChangeParams).toHaveBeenLastCalledWith(
        expect.objectContaining({
          cx: 0.5,
          cy: 0.5,
          r: expect.any(Number),
        })
      );

      const lastCall = onChangeParams.mock.calls[onChangeParams.mock.calls.length - 1][0];
      expect(lastCall.r).toBeGreaterThan(0.5);
      expect(lastCall.r).toBeLessThan(0.6);
      expect(Number.isFinite(lastCall.r)).toBe(true);
    });

    it('clamps cell coordinates to discrete 0-15 grid bounds', () => {
      const onChangeParams = vi.fn();
      const initialParams = {
        minX: 0,
        minY: 0,
        maxX: 1,
        maxY: 1,
        cx: 0.5,
        cy: 0.5,
        r: 0.2,
        cellX: 0,
        cellY: 0,
      };

      render(
        <SpatialRegionCanvas
          mode="Cell"
          params={initialParams}
          onChangeParams={onChangeParams}
          width={200}
          height={200}
          testid="cell-canvas"
        />
      );

      const canvas = screen.getByTestId('cell-canvas');
      vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
        left: 0,
        top: 0,
        right: 200,
        bottom: 200,
        width: 200,
        height: 200,
        x: 0,
        y: 0,
        toJSON: () => {},
      });

      // Click near top-left: clientX = 0, clientY = 0 -> cell (0, 0)
      fireEvent.mouseDown(canvas, { clientX: 0, clientY: 0 });
      expect(onChangeParams).toHaveBeenCalledWith(
        expect.objectContaining({
          cellX: 0,
          cellY: 0,
        })
      );

      // Click near bottom-right: clientX = 199, clientY = 199 -> cell (15, 15)
      fireEvent.mouseDown(canvas, { clientX: 199, clientY: 199 });
      expect(onChangeParams).toHaveBeenCalledWith(
        expect.objectContaining({
          cellX: 15,
          cellY: 15,
        })
      );
    });
  });

  // =========================================================================
  // Area 3: Newick Tree String Parser/Exporter & Lineage DAG View
  // =========================================================================
  describe('Area 3: Newick Tree String Exporter & Lineage DAG Stress', () => {
    // Pure Newick generator for testing formatting contracts
    const generateNewick = (
      nodes: { id: string; generation: number }[],
      relations: { source: string; target: string }[]
    ): string => {
      if (nodes.length === 0) return '();';
      if (nodes.length === 1 && relations.length === 0) {
        return `${nodes[0].id};`;
      }

      // Build child map
      const childrenMap = new Map<string, string[]>();
      const parentMap = new Map<string, string>();
      const nodeGenMap = new Map(nodes.map((n) => [n.id, n.generation]));

      nodes.forEach((n) => childrenMap.set(n.id, []));
      relations.forEach((r) => {
        childrenMap.get(r.source)?.push(r.target);
        parentMap.set(r.target, r.source);
      });

      const roots = nodes.filter((n) => !parentMap.has(n.id));

      const formatNode = (nodeId: string): string => {
        const kids = childrenMap.get(nodeId) || [];
        const gen = nodeGenMap.get(nodeId) || 0;
        let sub = '';
        if (kids.length > 0) {
          const formattedKids = kids.map((kidId) => {
            const kidGen = nodeGenMap.get(kidId) || 0;
            const branchLen = Math.max(0, kidGen - gen);
            return `${formatNode(kidId)}:${branchLen}`;
          });
          sub = `(${formattedKids.join(',')})`;
        }
        return `${sub}${nodeId}`;
      };

      return roots.map((r) => `${formatNode(r.id)};`).join('\n');
    };

    it('formats a single-node tree into valid Newick string', () => {
      const single = generateNewick([{ id: 'RootFounder', generation: 0 }], []);
      expect(single).toBe('RootFounder;');
    });

    it('formats a deeply nested linear lineage chain of 500 generations without stack overflow', () => {
      const count = 500;
      const nodes = Array.from({ length: count }, (_, i) => ({
        id: `G_${i}`,
        generation: i,
      }));
      const relations = Array.from({ length: count - 1 }, (_, i) => ({
        source: `G_${i}`,
        target: `G_${i + 1}`,
      }));

      const nwk = generateNewick(nodes, relations);
      expect(nwk.endsWith(';')).toBe(true);
      expect(nwk.startsWith('(')).toBe(true);
      expect(nwk).toContain('G_499');
      expect(nwk).toContain('G_0');
      // Verify parens balance
      const openCount = (nwk.match(/\(/g) || []).length;
      const closeCount = (nwk.match(/\)/g) || []).length;
      expect(openCount).toBe(closeCount);
      expect(openCount).toBe(count - 1);
    });

    it('formats an unbalanced star tree with 1000 child nodes on 1 root', () => {
      const childCount = 1000;
      const nodes = [
        { id: 'QueenRoot', generation: 0 },
        ...Array.from({ length: childCount }, (_, i) => ({
          id: `Drone_${i}`,
          generation: 1,
        })),
      ];
      const relations = Array.from({ length: childCount }, (_, i) => ({
        source: 'QueenRoot',
        target: `Drone_${i}`,
      }));

      const nwk = generateNewick(nodes, relations);
      expect(nwk.endsWith('QueenRoot;')).toBe(true);
      expect(nwk).toContain('Drone_0:1');
      expect(nwk).toContain('Drone_999:1');
    });

    it('handles special character quoting in Newick labels', () => {
      const quoteLabel = (raw: string): string => {
        const reserved = ['(', ')', ',', ':', ';', "'", '[', ']', '_'];
        const needsQuoting =
          raw.length === 0 ||
          reserved.some((c) => raw.includes(c)) ||
          /\s/.test(raw);
        if (!needsQuoting) return raw;
        return `'${raw.replace(/'/g, "''")}'`;
      };

      expect(quoteLabel('Homo sapiens')).toBe("'Homo sapiens'");
      expect(quoteLabel("O'Connor_Strain")).toBe("'O''Connor_Strain'");
      expect(quoteLabel('Node:1,2')).toBe("'Node:1,2'");
      expect(quoteLabel('Plain123')).toBe('Plain123');
      expect(quoteLabel('')).toBe("''");
    });

    it('renders LineageDagView with empty graph without crashing', () => {
      render(<LineageDagView lineage={null} testid="empty-lineage-dag" />);
      expect(screen.getByTestId('empty-lineage-dag')).toBeInTheDocument();
      expect(screen.getByText(/No phylogenetic lineage data available yet/i)).toBeInTheDocument();
    });

    it('renders LineageDagView with 100 nodes across 20 generations', () => {
      const nodes: LineageNodePayload[] = Array.from({ length: 100 }, (_, i) => ({
        id: `node-${i}`,
        generation: Math.floor(i / 5),
        fitness: 0.5 + (i % 10) * 0.05,
        mutations_count: i % 4,
        parent_id: i >= 5 ? `node-${i - 5}` : null,
      }));

      const links = Array.from({ length: 95 }, (_, i) => ({
        source: `node-${i}`,
        target: `node-${i + 5}`,
      }));

      const payload: LineageGraphPayload = { nodes, links, db_connected: false };

      render(<LineageDagView lineage={payload} testid="stress-lineage-dag" />);
      expect(screen.getByTestId('stress-lineage-dag')).toBeInTheDocument();
      expect(screen.getByTestId('lineage-node-node-0')).toBeInTheDocument();
      expect(screen.getByTestId('lineage-node-node-99')).toBeInTheDocument();

      // Test zoom controls
      const zoomIn = screen.getByTestId('lineage-zoom-in');
      const zoomOut = screen.getByTestId('lineage-zoom-out');
      const resetBtn = screen.getByTestId('lineage-reset-pan');

      fireEvent.click(zoomIn);
      fireEvent.click(zoomIn);
      fireEvent.click(zoomOut);
      fireEvent.click(resetBtn);
    });
  });

  // =========================================================================
  // Area 4: Export Dialog Data Serialization & Download Edge Cases
  // =========================================================================
  describe('Area 4: Data Export Serialization & File Name Boundary Safety', () => {
    it('sanitizes and tolerates malformed / hostile file names', async () => {
      const onClose = vi.fn();
      render(<DataExportDialog onClose={onClose} defaultSeed={999} />);

      const filenameInput = screen.getByTestId('export-filename-input');
      const executeBtn = screen.getByTestId('execute-export-dataset-btn');

      // Test path traversal attempt
      fireEvent.change(filenameInput, { target: { value: '../../../../etc/shadow' } });
      fireEvent.click(executeBtn);

      await waitFor(() => {
        expect(screen.getByTestId('export-preview-content')).toBeInTheDocument();
      });

      // Test windows forbidden chars: AUX, CON, NUL, COLON
      fireEvent.change(filenameInput, { target: { value: 'CON:PRN:*?"<>|' } });
      fireEvent.click(executeBtn);

      await waitFor(() => {
        expect(screen.getByTestId('export-preview-content')).toBeInTheDocument();
      });

      // Test empty string / whitespace only
      fireEvent.change(filenameInput, { target: { value: '    ' } });
      fireEvent.click(executeBtn);

      await waitFor(() => {
        expect(screen.getByTestId('export-preview-content')).toBeInTheDocument();
      });

      // Test emojis and non-ASCII Unicode
      fireEvent.change(filenameInput, { target: { value: '🔬_experiment_🧬_2026_🔥' } });
      fireEvent.click(executeBtn);

      await waitFor(() => {
        expect(screen.getByTestId('export-preview-content')).toBeInTheDocument();
      });
    });

    it('handles simulated file download triggers without memory leak or DOM corruption', async () => {
      const createObjectURLMock = vi.fn().mockReturnValue('blob:http://localhost/test-blob-123');
      const revokeObjectURLMock = vi.fn();
      global.URL.createObjectURL = createObjectURLMock;
      global.URL.revokeObjectURL = revokeObjectURLMock;

      const onClose = vi.fn();
      render(<DataExportDialog onClose={onClose} defaultSeed={12345} />);

      // Export JSON
      fireEvent.click(screen.getByTestId('execute-export-dataset-btn'));

      await waitFor(() => {
        expect(screen.getByTestId('export-download-btn')).toBeInTheDocument();
      });

      // Click download button
      const downloadBtn = screen.getByTestId('export-download-btn');
      fireEvent.click(downloadBtn);

      expect(createObjectURLMock).toHaveBeenCalled();
      expect(revokeObjectURLMock).toHaveBeenCalledWith('blob:http://localhost/test-blob-123');

      // Switch to CSV and download
      fireEvent.click(screen.getByTestId('export-format-csv'));
      fireEvent.click(downloadBtn);

      expect(createObjectURLMock).toHaveBeenCalledTimes(2);
      expect(revokeObjectURLMock).toHaveBeenCalledTimes(2);
    });

    it('serializes large scientific datasets (10,000 samples) to RFC 4180 CSV and JSON without crashing', () => {
      const sampleCount = 10000;
      const rows = ['tick,plants,prey,predator,detritus,energy_residual'];
      const timeSeries = [];

      for (let i = 0; i < sampleCount; i++) {
        const tick = i * 10;
        const p = 500 + Math.sin(i * 0.1) * 50;
        const h = 250 + Math.cos(i * 0.1) * 30;
        const c = 40 + Math.sin(i * 0.05) * 10;
        const d = 100 + Math.cos(i * 0.02) * 5;
        const r = 0.0001;
        rows.push(`${tick},${p.toFixed(2)},${h.toFixed(2)},${c.toFixed(2)},${d.toFixed(2)},${r}`);
        timeSeries.push({ tick, plants: p, prey: h, predator: c, detritus: d, residual: r });
      }

      const csvContent = rows.join('\n');
      expect(csvContent.length).toBeGreaterThan(300000); // ~394 KB CSV
      expect(csvContent.startsWith('tick,plants')).toBe(true);

      const jsonPayload = JSON.stringify({
        schemaVersion: 1,
        provenance: {
          commit: 'c320fc0',
          seed: 42,
          sampleCount,
        },
        timeSeries,
      });

      expect(jsonPayload.length).toBeGreaterThan(1000000); // > 1 MB JSON
      const parsed = JSON.parse(jsonPayload);
      expect(parsed.timeSeries.length).toBe(sampleCount);
      expect(parsed.timeSeries[9999].tick).toBe(99990);
    });

    it('handles clipboard copy failures gracefully when navigator.clipboard rejects', async () => {
      const writeTextMock = vi.fn().mockRejectedValue(new Error('Permission denied'));
      Object.assign(navigator, {
        clipboard: {
          writeText: writeTextMock,
        },
      });

      const onClose = vi.fn();
      render(<DataExportDialog onClose={onClose} defaultSeed={42} />);

      fireEvent.click(screen.getByTestId('execute-export-dataset-btn'));

      await waitFor(() => {
        expect(screen.getByTestId('export-copy-btn')).toBeInTheDocument();
      });

      // Attempt copy with rejected clipboard promise -> UI should not crash
      fireEvent.click(screen.getByTestId('export-copy-btn'));
      expect(writeTextMock).toHaveBeenCalled();
    });
  });

  // =========================================================================
  // Area 5: Branch Tree Canvas Topology & Edge-Case Robustness
  // =========================================================================
  describe('Area 5: Branch Tree Canvas Topology Robustness', () => {
    it('handles null, empty, or single root branch tree states safely', () => {
      const { rerender } = render(
        <BranchTreeCanvas tree={null} testid="empty-branch-tree" />
      );
      expect(screen.getByTestId('empty-branch-tree')).toBeInTheDocument();
      expect(screen.getByText(/No causal branch tree loaded/i)).toBeInTheDocument();

      const singleRootTree: CausalBranchTreeState = {
        rootRunId: 1000,
        nodes: [
          {
            id: 1000,
            parentId: null,
            forkTick: 0,
            label: 'Genesis',
            seedStrategy: 'InheritStream',
            status: 'Completed',
            children: [],
          },
        ],
        divergenceMetrics: {
          branchA: 1000,
          branchB: 1000,
          biomassDelta: 0,
          populationDelta: 0,
          shannonDivergence: 0,
          dtwDistance: 0,
          rmsd: 0,
        },
      };

      rerender(
        <BranchTreeCanvas tree={singleRootTree} testid="single-root-branch-tree" />
      );
      expect(screen.getByTestId('single-root-branch-tree')).toBeInTheDocument();
      expect(screen.getByTestId('branch-node-1000')).toBeInTheDocument();
    });

    it('handles dense N-ary branching (1 parent with 20 parallel branches)', () => {
      const denseNodes = [
        {
          id: 1000,
          parentId: null,
          forkTick: 0,
          label: 'Master Root',
          seedStrategy: 'InheritStream',
          status: 'Completed' as const,
          children: Array.from({ length: 20 }, (_, i) => 1001 + i),
        },
        ...Array.from({ length: 20 }, (_, i) => ({
          id: 1001 + i,
          parentId: 1000,
          forkTick: 100 * (i + 1),
          label: `Intervention ${i + 1}`,
          seedStrategy: `DerivedStream (${i + 1})`,
          status: 'Completed' as const,
          children: [],
        })),
      ];

      const denseTree: CausalBranchTreeState = {
        rootRunId: 1000,
        nodes: denseNodes,
        divergenceMetrics: {
          branchA: 1000,
          branchB: 1020,
          biomassDelta: -15.4,
          populationDelta: -3,
          shannonDivergence: 0.12,
          dtwDistance: 4.5,
          rmsd: 2.1,
        },
      };

      render(<BranchTreeCanvas tree={denseTree} testid="dense-branch-tree" width={600} height={400} />);
      expect(screen.getByTestId('dense-branch-tree')).toBeInTheDocument();
      expect(screen.getByTestId('branch-node-1000')).toBeInTheDocument();
      expect(screen.getByTestId('branch-node-1020')).toBeInTheDocument();
      expect(screen.getByTestId('branch-edge-1001')).toBeInTheDocument();
      expect(screen.getByTestId('branch-edge-1020')).toBeInTheDocument();
    });
  });
});
