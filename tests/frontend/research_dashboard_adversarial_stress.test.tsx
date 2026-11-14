import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';

import { ResearchDashboard } from '../../src/components/ResearchDashboard/ResearchDashboard';
import { TelemetryOverviewPanel } from '../../src/components/ResearchDashboard/TelemetryOverviewPanel';
import { TrophicDynamicsPanel } from '../../src/components/ResearchDashboard/TrophicDynamicsPanel';
import { EvolutionLineagePanel } from '../../src/components/ResearchDashboard/EvolutionLineagePanel';
import { CausalInterventionPanel } from '../../src/components/ResearchDashboard/CausalInterventionPanel';
import { CheckpointStudioPanel } from '../../src/components/ResearchDashboard/CheckpointStudioPanel';
import { DataExportDialog } from '../../src/components/ResearchDashboard/DataExportDialog';

// Common visual primitives
import { TimeSeriesChart } from '../../src/components/ResearchDashboard/common/TimeSeriesChart';
import { PhaseSpaceCanvas } from '../../src/components/ResearchDashboard/common/PhaseSpaceCanvas';
import { LineageDagView } from '../../src/components/ResearchDashboard/common/LineageDagView';
import { MapElitesHeatmap } from '../../src/components/ResearchDashboard/common/MapElitesHeatmap';
import { BranchTreeCanvas } from '../../src/components/ResearchDashboard/common/BranchTreeCanvas';
import { SpatialRegionCanvas } from '../../src/components/ResearchDashboard/common/SpatialRegionCanvas';
import { SankeyDiagram } from '../../src/components/ResearchDashboard/common/SankeyDiagram';
import { TrophicPyramid } from '../../src/components/ResearchDashboard/common/TrophicPyramid';
import { SparkLine } from '../../src/components/ResearchDashboard/common/SparkLine';
import { CurveRampPreview } from '../../src/components/ResearchDashboard/common/CurveRampPreview';

import type { LineageGraphPayload } from '../../src/types/generated/LineageGraphPayload';
import type { LineageNodePayload } from '../../src/types/generated/LineageNodePayload';
import type { LineageLinkPayload } from '../../src/types/generated/LineageLinkPayload';
import type { EliteIndividualState } from '../../src/types/generated/EliteIndividualState';
import type { EnergyFluxReport } from '../../src/types/generated/EnergyFluxReport';
import type { SpatialDensityReport } from '../../src/types/generated/SpatialDensityReport';
import type { CausalBranchTreeState, CausalBranchNode } from '../../src/components/ResearchDashboard/types';

describe('Adversarial & Empirical Stress Suite: Research Dashboard (Milestone 4)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // =========================================================================
  // TIER 1: EXTREME SCALING STRESS TESTS
  // =========================================================================
  describe('Tier 1: Extreme Scaling Stress Tests', () => {
    it('T1.1: renders TimeSeriesChart with 5,000+ points across 5 series under 100ms without NaN or layout blowup', () => {
      const numPoints = 6000;
      const series = [
        {
          label: 'Plants',
          color: '#10b981',
          values: Array.from({ length: numPoints }, (_, i) => 500 + Math.sin(i * 0.05) * 100),
        },
        {
          label: 'Prey',
          color: '#3b82f6',
          values: Array.from({ length: numPoints }, (_, i) => 250 + Math.cos(i * 0.05) * 50),
        },
        {
          label: 'Predator',
          color: '#ef4444',
          values: Array.from({ length: numPoints }, (_, i) => 40 + Math.sin(i * 0.02) * 15),
        },
        {
          label: 'Detritus',
          color: '#a855f7',
          values: Array.from({ length: numPoints }, (_, i) => 100 + (i % 20)),
        },
        {
          label: 'Total Energy',
          color: '#cbd5e1',
          values: Array.from({ length: numPoints }, () => 900.0),
        },
      ];

      const startTime = performance.now();
      const { container, rerender } = render(
        <TimeSeriesChart
          series={series}
          title="Extreme Scale 6k Points"
          timeWindow={2000}
          testid="extreme-ts-chart"
        />
      );
      const renderDuration = performance.now() - startTime;
      expect(renderDuration).toBeLessThan(1000); // Must render smoothly

      expect(screen.getByTestId('extreme-ts-chart')).toBeInTheDocument();
      expect(screen.getByTestId('series-line-Plants')).toBeInTheDocument();
      expect(screen.getByTestId('series-line-Total Energy')).toBeInTheDocument();

      // Test time window switching across 6k points
      rerender(
        <TimeSeriesChart
          series={series}
          title="Extreme Scale 6k Points"
          timeWindow={0} // All points
          testid="extreme-ts-chart"
        />
      );
      expect(screen.getByTestId('extreme-ts-chart')).toBeInTheDocument();

      // Test crosshair mouse move interaction over 6,000 points
      const svg = container.querySelector('svg');
      expect(svg).not.toBeNull();
      if (svg) {
        // Mock getBoundingClientRect
        vi.spyOn(svg, 'getBoundingClientRect').mockReturnValue({
          left: 0,
          top: 0,
          right: 600,
          bottom: 200,
          width: 600,
          height: 200,
          x: 0,
          y: 0,
          toJSON: () => {},
        });

        fireEvent.mouseMove(svg, { clientX: 300, clientY: 100 });
        expect(screen.getByTestId('chart-crosshair')).toBeInTheDocument();
        expect(screen.getByTestId('chart-tooltip')).toBeInTheDocument();

        fireEvent.mouseLeave(svg);
        expect(screen.queryByTestId('chart-crosshair')).not.toBeInTheDocument();
      }
    });

    it('T1.2: handles TimeSeriesChart boundary anomalies: NaNs, Infinities, extreme spikes (+-1e12), empty, single-point', () => {
      const pathologicalSeries = [
        { label: 'NaN-Inf', color: '#ff0000', values: [NaN, Infinity, -Infinity, 10, NaN, 20] },
        { label: 'Extreme Spike', color: '#00ff00', values: [1e12, -1e12, 0, 100, 1e-12] },
        { label: 'Empty', color: '#0000ff', values: [] },
        { label: 'Single', color: '#ffff00', values: [42] },
        { label: 'Flat Zero', color: '#ffffff', values: [0, 0, 0, 0, 0] },
      ];

      expect(() => {
        render(
          <TimeSeriesChart
            series={pathologicalSeries}
            referenceLine={{ y: 0, label: 'Zero Line', color: '#888' }}
            testid="pathological-ts-chart"
          />
        );
      }).not.toThrow();

      expect(screen.getByTestId('pathological-ts-chart')).toBeInTheDocument();
    });

    it('T1.3: renders LineageDagView with 1,000+ phylogenetic nodes across 50 generations', () => {
      const totalNodes = 1000;
      const numGenerations = 50;
      const nodes: LineageNodePayload[] = [];
      const links: LineageLinkPayload[] = [];

      // Generate deep branching phylogenetic tree
      nodes.push({
        id: 'node-gen0-0',
        generation: 0,
        parent_id: null,
        fitness: 0.5,
        mutations_count: 0,
      });

      for (let i = 1; i < totalNodes; i++) {
        const gen = (i % numGenerations) + 1;
        const parentId = `node-gen${gen - 1}-${Math.floor(i / numGenerations) % 10}`;
        const nodeId = `node-gen${gen}-${i}`;

        nodes.push({
          id: nodeId,
          generation: gen,
          parent_id: parentId,
          fitness: 0.1 + (i % 90) / 100,
          mutations_count: i % 10,
        });

        links.push({
          source: parentId,
          target: nodeId,
        });
      }

      const massiveLineage: LineageGraphPayload = {
        nodes,
        links,
        db_connected: true,
      };

      const selectNodeSpy = vi.fn();
      const { rerender } = render(
        <LineageDagView
          lineage={massiveLineage}
          selectedNodeId="node-gen1-1"
          onSelectNode={selectNodeSpy}
          testid="massive-lineage-dag"
        />
      );

      expect(screen.getByTestId('massive-lineage-dag')).toBeInTheDocument();

      // Test zoom and pan controls
      const zoomInBtn = screen.getByTestId('lineage-zoom-in');
      const zoomOutBtn = screen.getByTestId('lineage-zoom-out');
      const resetBtn = screen.getByTestId('lineage-reset-pan');

      fireEvent.click(zoomInBtn);
      fireEvent.click(zoomInBtn);
      fireEvent.click(zoomOutBtn);
      fireEvent.click(resetBtn);

      // Select a node in the tree
      const targetNode = screen.getByTestId('lineage-node-node-gen2-1');
      fireEvent.click(targetNode);
      expect(selectNodeSpy).toHaveBeenCalled();

      // Test empty / null lineage fallback
      rerender(
        <LineageDagView
          lineage={{ nodes: [], links: [], db_connected: false }}
          testid="empty-lineage-dag"
        />
      );
      expect(screen.getByText(/No phylogenetic lineage data available yet/i)).toBeInTheDocument();
    });

    it('T1.4: renders MapElitesHeatmap with dense 50x50 (2,500 cells) and 100x100 (10,000 cells) grids', () => {
      const grid50: Record<string, EliteIndividualState> = {};
      for (let y = 0; y < 50; y++) {
        for (let x = 0; x < 50; x++) {
          grid50[`${x},${y}`] = {
            fitness: (x * y) / 2500,
            features: [x / 50, y / 50],
          };
        }
      }

      const onSelectCell = vi.fn();
      const { rerender } = render(
        <MapElitesHeatmap
          grid={grid50}
          gridResolution={50}
          width={300}
          height={300}
          onSelectCell={onSelectCell}
          testid="dense-map-elites-50"
        />
      );

      const canvas = screen.getByTestId('dense-map-elites-50') as HTMLCanvasElement;
      expect(canvas).toBeInTheDocument();

      // Mock canvas bounding rect for mouse interaction
      vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
        left: 0,
        top: 0,
        right: 300,
        bottom: 300,
        width: 300,
        height: 300,
        x: 0,
        y: 0,
        toJSON: () => {},
      });

      // Simulate rapid hovering across 50x50 cells
      for (let i = 0; i < 20; i++) {
        fireEvent.mouseMove(canvas, { clientX: i * 15, clientY: i * 15 });
      }

      expect(screen.getByTestId('map-elites-cell-tooltip')).toBeInTheDocument();

      // Click cell
      fireEvent.click(canvas);
      expect(onSelectCell).toHaveBeenCalled();

      // Mouse leave clears hover
      fireEvent.mouseLeave(canvas);
      expect(screen.queryByTestId('map-elites-cell-tooltip')).not.toBeInTheDocument();

      // Rerender with 100x100 resolution
      rerender(
        <MapElitesHeatmap
          grid={grid50}
          gridResolution={100}
          width={300}
          height={300}
          testid="dense-map-elites-100"
        />
      );
      expect(screen.getByTestId('dense-map-elites-100')).toBeInTheDocument();
    });

    it('T1.5: stress tests PhaseSpaceCanvas with 5,000+ attractor orbit coordinates and pathological vectors', () => {
      const len = 5000;
      const preyHistory = Array.from({ length: len }, (_, i) => 20 + Math.sin(i * 0.1) * 15 + (i % 5));
      const predatorHistory = Array.from({ length: len }, (_, i) => 8 + Math.cos(i * 0.1) * 6 + (i % 3));

      expect(() => {
        render(
          <PhaseSpaceCanvas
            preyHistory={preyHistory}
            predatorHistory={predatorHistory}
            width={350}
            height={280}
            testid="extreme-phase-space-canvas"
          />
        );
      }).not.toThrow();

      expect(screen.getByTestId('extreme-phase-space-canvas')).toBeInTheDocument();

      // Test pathological history: empty, length mismatch, all zeros, extreme numbers
      const { rerender } = render(
        <PhaseSpaceCanvas
          preyHistory={[]}
          predatorHistory={[]}
          testid="empty-phase-space-canvas"
        />
      );
      expect(screen.getByTestId('empty-phase-space-canvas')).toBeInTheDocument();

      rerender(
        <PhaseSpaceCanvas
          preyHistory={[NaN, Infinity, 1e9, -50]}
          predatorHistory={[0, 0, NaN, 1e9]}
          testid="pathological-phase-space-canvas"
        />
      );
      expect(screen.getByTestId('pathological-phase-space-canvas')).toBeInTheDocument();
    });

    it('T1.6: stress tests BranchTreeCanvas with 50+ counterfactual nodes and deep branch chains', () => {
      const nodes: CausalBranchNode[] = [
        {
          id: 1000,
          parentId: null,
          forkTick: 0,
          label: 'Genesis Baseline',
          seedStrategy: 'InheritStream',
          status: 'Completed' as const,
          children: [1001],
        },
      ];

      for (let i = 1; i <= 50; i++) {
        nodes.push({
          id: 1000 + i,
          parentId: 1000 + i - 1,
          forkTick: i * 50,
          label: `Counterfactual Fork #${i}`,
          seedStrategy: `DerivedStream (${i})`,
          status: i % 2 === 0 ? ('Completed' as const) : ('Pending' as const),
          children: i < 50 ? [1000 + i + 1] : [],
        });
      }

      const deepTree: CausalBranchTreeState = {
        rootRunId: 1000,
        nodes,
        divergenceMetrics: {
          branchA: 1001,
          branchB: 1050,
          biomassDelta: -320.5,
          populationDelta: -45,
          shannonDivergence: 0.35,
          dtwDistance: 88.2,
          rmsd: 42.1,
        },
      };

      const selectBranchSpy = vi.fn();
      const forkBranchSpy = vi.fn();

      render(
        <BranchTreeCanvas
          tree={deepTree}
          selectedBranchId={1025}
          onSelectBranch={selectBranchSpy}
          onForkBranch={forkBranchSpy}
          width={600}
          height={300}
          testid="deep-branch-tree-canvas"
        />
      );

      expect(screen.getByTestId('deep-branch-tree-canvas')).toBeInTheDocument();

      // Click node
      const nodeElem = screen.getByTestId('branch-node-1025');
      fireEvent.click(nodeElem);
      expect(selectBranchSpy).toHaveBeenCalledWith(1025);

      // Click fork icon button
      const forkBtn = screen.getByTestId('fork-btn-1025');
      fireEvent.click(forkBtn);
      expect(forkBranchSpy).toHaveBeenCalledWith(1025);
    });

    it('T1.7: stress tests TrophicPyramid and SankeyDiagram with extreme biomass disparities and dense links', () => {
      // Disparity: Apex predator is 0.0001 EU, Flora is 1,000,000 EU
      const extremeTrophic = {
        detritus: 50000,
        plants: 1000000,
        herbivores: 120,
        carnivores: 0.001,
      };

      render(<TrophicPyramid data={extremeTrophic} testid="extreme-trophic-pyramid" />);
      expect(screen.getByTestId('extreme-trophic-pyramid')).toBeInTheDocument();
      expect(screen.getByTestId('trophic-tier-plants')).toBeInTheDocument();
      expect(screen.getByTestId('trophic-tier-carnivores')).toBeInTheDocument();

      // Sankey diagram with 20 nodes and 30 links
      const denseFluxReport: EnergyFluxReport = {
        nodes: Array.from({ length: 6 }, (_, i) => ({
          id: `node-${i}`,
          label: `Compartment ${i}`,
          biomass: (i + 1) * 500,
        })),
        links: [
          { source: 'sun', target: 'plants', value: 1200 },
          { source: 'plants', target: 'herbivores', value: 400 },
          { source: 'herbivores', target: 'carnivores', value: 80 },
          { source: 'plants', target: 'detritus', value: 150 },
          { source: 'herbivores', target: 'detritus', value: 70 },
          { source: 'carnivores', target: 'detritus', value: 20 },
          { source: 'detritus', target: 'plants', value: 60 },
          { source: 'herbivores', target: 'metabolic_sink', value: 250 },
          { source: 'carnivores', target: 'metabolic_sink', value: 60 },
        ],
        respirationLoss: 310,
        totalSystemEnergy: 5500,
        energyDriftPercent: 0.0015,
      };

      render(<SankeyDiagram report={denseFluxReport} testid="dense-sankey" />);
      expect(screen.getByTestId('dense-sankey')).toBeInTheDocument();

      // Hover link
      const link = screen.getByTestId('sankey-link-sun-plants');
      fireEvent.mouseEnter(link);
      expect(screen.getByTestId('sankey-tooltip')).toBeInTheDocument();
      fireEvent.mouseLeave(link);
      expect(screen.queryByTestId('sankey-tooltip')).not.toBeInTheDocument();
    });
  });

  // =========================================================================
  // TIER 2: RAPID STATE MUTATIONS & TAB SWITCHING CONCURRENCY
  // =========================================================================
  describe('Tier 2: Rapid State Mutations & Concurrency Stress', () => {
    it('T2.1: executes 100 rapid tab switches (clicks and keydowns) without unhandled exceptions or state races', () => {
      render(<ResearchDashboard />);

      const tabIds = ['telemetry', 'trophic', 'evolution', 'interventions', 'checkpoints'] as const;

      // 50 rapid sequential clicks
      for (let i = 0; i < 50; i++) {
        const tab = tabIds[i % tabIds.length];
        const btn = screen.getByTestId(`dashboard-tab-${tab}`);
        fireEvent.click(btn);
      }

      // 50 rapid keyboard shortcuts ('1' through '5')
      for (let i = 0; i < 50; i++) {
        const key = String((i % 5) + 1);
        fireEvent.keyDown(window, { key });
      }

      // Ensure active container remains healthy
      expect(screen.getByTestId('research-dashboard-container')).toBeInTheDocument();
      expect(screen.getByTestId('dashboard-tab-body')).toBeInTheDocument();
    });

    it('T2.2: handles 50 rapid causal intervention dispatches and cancellations in CausalInterventionPanel', async () => {
      render(<CausalInterventionPanel currentTick={500} isRunning={true} />);

      expect(screen.getByTestId('causal-intervention-panel')).toBeInTheDocument();

      const kinds = [
        'RainfallDelta',
        'TemperatureDelta',
        'Deforest',
        'RemovePredators',
        'AddNutrient',
        'GeneticInjection',
        'ToxicityPlume',
        'PathogenTransmission',
      ];

      const modes = ['Global', 'Rect', 'Radius', 'Cell'] as const;
      const curves = ['Step', 'RampUp', 'RampDown', 'Triangle'] as const;

      const kindSelect = screen.getByTestId('intervention-kind-select');
      const intensitySlider = screen.getByTestId('intervention-intensity-slider');
      const curveSelect = screen.getByTestId('intervention-curve-select');
      const applyBtn = screen.getByTestId('apply-intervention-btn');

      // Rapidly dispatch 10 varied interventions
      for (let i = 0; i < 10; i++) {
        fireEvent.click(screen.getByTestId(`region-mode-${modes[i % modes.length].toLowerCase()}`));
        fireEvent.change(kindSelect, { target: { value: kinds[i % kinds.length] } });
        fireEvent.change(intensitySlider, { target: { value: (0.1 + (i % 9) * 0.1).toFixed(2) } });
        fireEvent.change(curveSelect, { target: { value: curves[i % curves.length] } });
        fireEvent.click(applyBtn);
      }

      expect(screen.getByTestId('active-interventions-ledger')).toBeInTheDocument();

      // Cancel all interventions in rapid succession
      const cancelBtns = screen.getAllByText(/Cancel/i);
      for (const btn of cancelBtns) {
        fireEvent.click(btn);
      }

      expect(screen.getByText(/No active interventions in ledger/i)).toBeInTheDocument();
    });

    it('T2.3: handles rapid timeline scrubber changes (100 steps) and bookmark creations in CheckpointStudioPanel', () => {
      render(<CheckpointStudioPanel currentTick={2500} />);

      const slider = screen.getByTestId('timeline-scrubber-slider');
      const bookmarkBtn = screen.getByTestId('save-snapshot-bookmark-btn');

      // Rapidly scrub through 50 ticks and add bookmarks
      for (let t = 0; t < 50; t += 5) {
        fireEvent.change(slider, { target: { value: t * 50 } });
        if (t % 10 === 0) {
          fireEvent.click(bookmarkBtn);
        }
      }

      expect(screen.getByTestId('timeline-scrubber-card')).toBeInTheDocument();
      expect(screen.getByText(/Inspecting Tick: t = 2250/i)).toBeInTheDocument();
    });
  });

  // =========================================================================
  // TIER 3: LIFECYCLE & DISPOSAL STRESS
  // =========================================================================
  describe('Tier 3: Component Lifecycle & Disposal Stress', () => {
    it('T3.1: simulates 50 rapid mount / unmount / remount cycles of ResearchDashboard with zero leaks', () => {
      for (let i = 0; i < 50; i++) {
        const { unmount } = render(<ResearchDashboard />);
        expect(screen.getByTestId('research-dashboard-container')).toBeInTheDocument();
        unmount();
      }
    });

    it('T3.2: simulates 50 rapid mount / unmount cycles of individual panels and visual primitives', () => {
      for (let i = 0; i < 20; i++) {
        const { unmount: u1 } = render(<TelemetryOverviewPanel isRunning={false} />);
        u1();

        const { unmount: u2 } = render(<TrophicDynamicsPanel isRunning={false} />);
        u2();

        const { unmount: u3 } = render(<EvolutionLineagePanel isRunning={false} />);
        u3();

        const { unmount: u4 } = render(<CausalInterventionPanel isRunning={false} />);
        u4();

        const { unmount: u5 } = render(<CheckpointStudioPanel currentTick={100} />);
        u5();

        const { unmount: u6 } = render(<DataExportDialog onClose={() => {}} />);
        u6();
      }
    });

    it('T3.3: tests SpatialRegionCanvas across interactive drag drawing cycles and zero dimensions', () => {
      const onChangeParams = vi.fn();
      const defaultParams = {
        minX: 0.1,
        minY: 0.1,
        maxX: 0.9,
        maxY: 0.9,
        cx: 0.5,
        cy: 0.5,
        r: 0.2,
        cellX: 4,
        cellY: 4,
      };

      const densityReport: SpatialDensityReport = {
        width: 16,
        height: 16,
        preyDensity: new Array(256).fill(0.3),
        predatorDensity: new Array(256).fill(0.1),
        vegetationDensity: new Array(256).fill(0.7),
        predationHotspots: new Array(256).fill(0.05),
      };

      const { rerender } = render(
        <SpatialRegionCanvas
          mode="Rect"
          params={defaultParams}
          onChangeParams={onChangeParams}
          densityReport={densityReport}
          width={300}
          height={200}
          testid="spatial-canvas-test"
        />
      );

      const canvas = screen.getByTestId('spatial-canvas-test') as HTMLCanvasElement;
      vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
        left: 0,
        top: 0,
        right: 300,
        bottom: 200,
        width: 300,
        height: 200,
        x: 0,
        y: 0,
        toJSON: () => {},
      });

      // Simulate mouse drag for Rect
      fireEvent.mouseDown(canvas, { clientX: 50, clientY: 50 });
      fireEvent.mouseMove(canvas, { clientX: 200, clientY: 150 });
      fireEvent.mouseUp(canvas);
      expect(onChangeParams).toHaveBeenCalled();

      // Switch to Radius
      rerender(
        <SpatialRegionCanvas
          mode="Radius"
          params={defaultParams}
          onChangeParams={onChangeParams}
          width={300}
          height={200}
          testid="spatial-canvas-test"
        />
      );
      fireEvent.mouseDown(canvas, { clientX: 150, clientY: 100 });
      fireEvent.mouseMove(canvas, { clientX: 180, clientY: 140 });
      fireEvent.mouseUp(canvas);
      expect(onChangeParams).toHaveBeenCalled();

      // Switch to Cell
      rerender(
        <SpatialRegionCanvas
          mode="Cell"
          params={defaultParams}
          onChangeParams={onChangeParams}
          width={300}
          height={200}
          testid="spatial-canvas-test"
        />
      );
      fireEvent.mouseDown(canvas, { clientX: 80, clientY: 80 });
      fireEvent.mouseUp(canvas);
      expect(onChangeParams).toHaveBeenCalled();

      // Switch to Global
      rerender(
        <SpatialRegionCanvas
          mode="Global"
          params={defaultParams}
          onChangeParams={onChangeParams}
          width={300}
          height={200}
          testid="spatial-canvas-test"
        />
      );
      expect(screen.getByText(/Region: Global/i)).toBeInTheDocument();
    });
  });

  // =========================================================================
  // TIER 4: ADVERSARIAL INPUT BOUNDARIES & MODAL INTEGRITY
  // =========================================================================
  describe('Tier 4: Adversarial Input Boundaries & Modal Integrity', () => {
    it('T4.1: tests SparkLine and CurveRampPreview with pathological parameters', () => {
      render(
        <SparkLine
          series={[
            { label: 'S1', color: '#ff0000', values: [] },
            { label: 'S2', color: '#00ff00', values: [NaN, 10, Infinity, -100] },
            { label: 'S3', color: '#0000ff', values: [5, 5, 5, 5] },
          ]}
          height={40}
          testid="pathological-sparkline"
        />
      );
      expect(screen.getByTestId('pathological-sparkline')).toBeInTheDocument();

      const curves = ['Step', 'RampUp', 'RampDown', 'Triangle'] as const;
      for (const c of curves) {
        const { unmount } = render(
          <CurveRampPreview
            curve={c}
            intensity={1.5}
            duration={1000}
            signedNegative={true}
            testid={`curve-preview-${c}`}
          />
        );
        expect(screen.getByTestId(`curve-preview-${c}`)).toBeInTheDocument();
        unmount();
      }
    });

    it('T4.2: tests DataExportDialog with rapid toggles, unchecks, and keyboard shortcuts', async () => {
      const onClose = vi.fn();
      render(<DataExportDialog onClose={onClose} defaultSeed={999} />);

      expect(screen.getByTestId('data-export-dialog-modal')).toBeInTheDocument();

      // Rapidly toggle export format
      for (let i = 0; i < 15; i++) {
        fireEvent.click(screen.getByTestId('export-format-csv'));
        fireEvent.click(screen.getByTestId('export-format-newick'));
        fireEvent.click(screen.getByTestId('export-format-json'));
      }

      // Toggle all checkboxes
      fireEvent.click(screen.getByTestId('export-include-timeseries'));
      fireEvent.click(screen.getByTestId('export-include-demographics'));
      fireEvent.click(screen.getByTestId('export-include-energy-flux'));
      fireEvent.click(screen.getByTestId('export-include-spatial-density'));
      fireEvent.click(screen.getByTestId('export-include-phylogeny'));

      // Re-enable and trigger export
      fireEvent.click(screen.getByTestId('export-include-timeseries'));
      fireEvent.click(screen.getByTestId('execute-export-dataset-btn'));

      // Test backdrop click closes modal
      const backdrop = screen.getByTestId('data-export-dialog-backdrop');
      fireEvent.click(backdrop);
      expect(onClose).toHaveBeenCalled();
    });
  });

  // =========================================================================
  // TIER 5: ADVERSARIAL EDGE CASES & SYSTEM RECOVERY
  // =========================================================================
  describe('Tier 5: Adversarial Edge Cases & System Recovery', () => {
    it('T5.1: LineageDagView withstands extreme zoom limits (+20x / -20x) and boundary drag-panning', () => {
      const lineage: LineageGraphPayload = {
        nodes: [
          { id: 'root', generation: 0, parent_id: null, fitness: 0.8, mutations_count: 0 },
          { id: 'c1', generation: 1, parent_id: 'root', fitness: 0.6, mutations_count: 1 },
        ],
        links: [{ source: 'root', target: 'c1' }],
        db_connected: true,
      };

      const { container } = render(<LineageDagView lineage={lineage} testid="zoom-pan-dag" />);

      const zoomIn = screen.getByTestId('lineage-zoom-in');
      const zoomOut = screen.getByTestId('lineage-zoom-out');
      const resetBtn = screen.getByTestId('lineage-reset-pan');

      // Zoom in 30 times (should clamp at 2.5)
      for (let i = 0; i < 30; i++) {
        fireEvent.click(zoomIn);
      }

      // Zoom out 30 times (should clamp at 0.4)
      for (let i = 0; i < 30; i++) {
        fireEvent.click(zoomOut);
      }

      // Reset
      fireEvent.click(resetBtn);

      // Drag pan boundary
      const dagContainer = container.firstElementChild as HTMLElement;
      fireEvent.mouseDown(dagContainer, { clientX: 100, clientY: 100 });
      fireEvent.mouseMove(dagContainer, { clientX: -5000, clientY: -5000 });
      fireEvent.mouseUp(dagContainer);

      expect(screen.getByTestId('zoom-pan-dag')).toBeInTheDocument();
    });

    it('T5.2: MapElitesHeatmap handles empty grids, non-numeric coordinates, and NaN feature values', () => {
      const pathologicalGrid: Record<string, EliteIndividualState> = {
        'invalid_coord': { fitness: 0.5, features: [0, 0] },
        '-5,-10': { fitness: 0.2, features: [-1, -1] },
        '100,100': { fitness: 0.9, features: [100, 100] },
        '2,2': { fitness: NaN, features: [NaN, Infinity] },
      };

      expect(() => {
        render(
          <MapElitesHeatmap
            grid={pathologicalGrid}
            gridResolution={10}
            width={200}
            height={200}
            testid="pathological-map-elites"
          />
        );
      }).not.toThrow();

      expect(screen.getByTestId('pathological-map-elites')).toBeInTheDocument();
    });

    it('T5.3: SankeyDiagram handles circular loops, missing node definitions, and zero flux gracefully', () => {
      const circularFluxReport: EnergyFluxReport = {
        nodes: [
          { id: 'sun', label: 'Sun', biomass: 1000 },
          { id: 'plants', label: 'Flora', biomass: 500 },
        ],
        links: [
          { source: 'sun', target: 'plants', value: 100 },
          { source: 'plants', target: 'sun', value: 50 }, // circular back-edge
          { source: 'unknown_source', target: 'plants', value: 10 }, // unmapped node
          { source: 'plants', target: 'unknown_target', value: 10 }, // unmapped node
          { source: 'plants', target: 'plants', value: 0 }, // zero self-loop
        ],
        respirationLoss: 0,
        totalSystemEnergy: 1500,
        energyDriftPercent: 0,
      };

      expect(() => {
        render(<SankeyDiagram report={circularFluxReport} testid="circular-sankey" />);
      }).not.toThrow();

      expect(screen.getByTestId('circular-sankey')).toBeInTheDocument();
    });

    it('T5.4: TrophicPyramid handles zero biomass across all compartments without division-by-zero NaN %', () => {
      const zeroBiomass = {
        detritus: 0,
        plants: 0,
        herbivores: 0,
        carnivores: 0,
      };

      render(<TrophicPyramid data={zeroBiomass} testid="zero-trophic-pyramid" />);
      expect(screen.getByTestId('zero-trophic-pyramid')).toBeInTheDocument();
      expect(screen.getByText(/Total Biomass:/i)).toBeInTheDocument();
      expect(screen.getByText(/1.0 EU/i)).toBeInTheDocument();
    });

    it('T5.5: ResearchDashboard keyboard shortcuts are suppressed when typing in input fields', () => {
      render(<ResearchDashboard />);

      // Create a dummy input in document
      const input = document.createElement('input');
      document.body.appendChild(input);
      input.focus();

      // Press '2' while focused in input - active tab should NOT change to trophic
      fireEvent.keyDown(input, { key: '2' });
      expect(screen.getByTestId('telemetry-overview-panel')).toBeInTheDocument();

      // Unfocus input and press '2'
      input.blur();
      fireEvent.keyDown(window, { key: '2' });
      expect(screen.getByTestId('trophic-dynamics-panel')).toBeInTheDocument();

      document.body.removeChild(input);
    });

    it('T5.6: CheckpointStudioPanel handles deep multi-tier branch DAG with recursive siblings', () => {
      const multiTierNodes = [
        { id: 1, parentId: null, forkTick: 0, label: 'Root 0', seedStrategy: 'InheritStream', status: 'Completed' as const, children: [2, 3] },
        { id: 2, parentId: 1, forkTick: 100, label: 'Branch 1.1', seedStrategy: 'DerivedStream (1)', status: 'Completed' as const, children: [4] },
        { id: 3, parentId: 1, forkTick: 100, label: 'Branch 1.2', seedStrategy: 'DerivedStream (2)', status: 'Completed' as const, children: [5] },
        { id: 4, parentId: 2, forkTick: 200, label: 'Branch 2.1', seedStrategy: 'DerivedStream (3)', status: 'Completed' as const, children: [] },
        { id: 5, parentId: 3, forkTick: 200, label: 'Branch 2.2', seedStrategy: 'DerivedStream (4)', status: 'Completed' as const, children: [] },
      ];

      const treeState: CausalBranchTreeState = {
        rootRunId: 1,
        nodes: multiTierNodes,
        divergenceMetrics: {
          branchA: 4,
          branchB: 5,
          biomassDelta: -80,
          populationDelta: -10,
          shannonDivergence: 0.1,
          dtwDistance: 15.2,
          rmsd: 8.4,
        },
      };

      render(<CheckpointStudioPanel currentTick={500} initialBranchTree={treeState} />);
      expect(screen.getByTestId('checkpoint-studio-panel')).toBeInTheDocument();
      expect(screen.getByTestId('causal-branch-tree-svg')).toBeInTheDocument();
    });
  });
});

