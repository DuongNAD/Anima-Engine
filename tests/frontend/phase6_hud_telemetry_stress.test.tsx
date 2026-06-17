import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { StatusPanel } from '../../src/components/StatusPanel';
import { SimulationStatus } from '../../src/types';

describe('HUD Telemetry StatusPanel Stress and Verification Tests', () => {

  const getElementStyle = (element: HTMLElement) => {
    return (element as any).style || {};
  };

  it('Verify that the status bar layout groups latency and FPS and is responsive', () => {
    const mockStatus: SimulationStatus = {
      running: true,
      tick_count: 100,
      avg_tick_time_ms: 1.5,
      fps: 60.0,
    };

    render(<StatusPanel status={mockStatus} />);

    // Get the status bar parent
    const latencyLabel = screen.getByText('TICK_LATENCY:');
    const statusBar = latencyLabel.parentElement?.parentElement;
    
    expect(statusBar).toBeDefined();
    
    // Check responsive design flexbox rules on the status bar container
    const style = getElementStyle(statusBar!);
    expect(style.display).toBe('flex');
    expect(style.justifyContent).toBe('space-between');
    expect(style.alignItems).toBe('center');

    // Verify both indicators are children of the status bar
    const children = statusBar?.children;
    expect(children?.length).toBe(2);
    expect(children?.[0].textContent).toContain('TICK_LATENCY');
    expect(children?.[1].textContent).toContain('BACKEND_FPS');
  });

  it('Verify dynamic glow colors under EXCELLENT telemetry conditions', () => {
    const mockStatus: SimulationStatus = {
      running: true,
      tick_count: 100,
      avg_tick_time_ms: 1.5, // Excellent (< 2.0 ms)
      fps: 60.0,            // Excellent (>= 55 FPS)
    };

    render(<StatusPanel status={mockStatus} />);

    const latencyLabel = screen.getByText('TICK_LATENCY:');
    const latencyValue = latencyLabel.nextElementSibling as HTMLElement;

    const fpsLabel = screen.getByText('BACKEND_FPS:');
    const fpsValue = fpsLabel.nextElementSibling as HTMLElement;

    // Both should glow neon green: #f8fafc (monochromatic)
    expect(getElementStyle(latencyValue).color).toBe('rgb(248, 250, 252)'); // equivalent of #f8fafc in RGB format
    expect(getElementStyle(fpsValue).color).toBe('rgb(248, 250, 252)');

    expect(getElementStyle(latencyValue).textShadow || '').toBeFalsy();
    expect(getElementStyle(fpsValue).textShadow || '').toBeFalsy();
  });

  it('Verify dynamic glow colors under WARNING/MODERATE telemetry conditions', () => {
    const mockStatus: SimulationStatus = {
      running: true,
      tick_count: 100,
      avg_tick_time_ms: 3.5, // Warning (2.0 ms <= latency < 5.0 ms)
      fps: 45.0,            // Warning (30 FPS <= fps < 55 FPS)
    };

    render(<StatusPanel status={mockStatus} />);

    const latencyLabel = screen.getByText('TICK_LATENCY:');
    const latencyValue = latencyLabel.nextElementSibling as HTMLElement;

    const fpsLabel = screen.getByText('BACKEND_FPS:');
    const fpsValue = fpsLabel.nextElementSibling as HTMLElement;

    // Both should glow neon gold: #cbd5e1 (monochromatic)
    expect(getElementStyle(latencyValue).color).toBe('rgb(203, 213, 225)'); // equivalent of #cbd5e1
    expect(getElementStyle(fpsValue).color).toBe('rgb(203, 213, 225)');

    expect(getElementStyle(latencyValue).textShadow || '').toBeFalsy();
    expect(getElementStyle(fpsValue).textShadow || '').toBeFalsy();
  });

  it('Verify dynamic glow colors under CRITICAL/LAGGING telemetry conditions', () => {
    const mockStatus: SimulationStatus = {
      running: true,
      tick_count: 100,
      avg_tick_time_ms: 6.0, // Critical (>= 5.0 ms)
      fps: 25.0,            // Critical (< 30 FPS)
    };

    render(<StatusPanel status={mockStatus} />);

    const latencyLabel = screen.getByText('TICK_LATENCY:');
    const latencyValue = latencyLabel.nextElementSibling as HTMLElement;

    const fpsLabel = screen.getByText('BACKEND_FPS:');
    const fpsValue = fpsLabel.nextElementSibling as HTMLElement;

    // Both should glow neon red: #64748b (monochromatic)
    expect(getElementStyle(latencyValue).color).toBe('rgb(100, 116, 139)'); // equivalent of #64748b
    expect(getElementStyle(fpsValue).color).toBe('rgb(100, 116, 139)');

    expect(getElementStyle(latencyValue).textShadow || '').toBeFalsy();
    expect(getElementStyle(fpsValue).textShadow || '').toBeFalsy();
  });

  it('Verify boundary values for color transitions', () => {
    // Boundary 1: FPS exact transitions: 55 and 30
    const mockStatus1: SimulationStatus = {
      running: true,
      tick_count: 100,
      avg_tick_time_ms: 2.0, // Should map to warning (#ffd700)
      fps: 55.0,            // Should map to excellent (#39ff14)
    };

    const { unmount } = render(<StatusPanel status={mockStatus1} />);
    
    const latencyLabel1 = screen.getByText('TICK_LATENCY:');
    const latencyValue1 = latencyLabel1.nextElementSibling as HTMLElement;
    const fpsLabel1 = screen.getByText('BACKEND_FPS:');
    const fpsValue1 = fpsLabel1.nextElementSibling as HTMLElement;

    expect(getElementStyle(fpsValue1).color).toBe('rgb(248, 250, 252)'); // #f8fafc
    expect(getElementStyle(latencyValue1).color).toBe('rgb(203, 213, 225)'); // #cbd5e1
    unmount();

    // Boundary 2: Latency exact transition: 5.0
    const mockStatus2: SimulationStatus = {
      running: true,
      tick_count: 100,
      avg_tick_time_ms: 5.0, // Should map to critical (#ff3333)
      fps: 30.0,            // Should map to warning (#ffd700)
    };

    render(<StatusPanel status={mockStatus2} />);
    
    const latencyLabel2 = screen.getByText('TICK_LATENCY:');
    const latencyValue2 = latencyLabel2.nextElementSibling as HTMLElement;
    const fpsLabel2 = screen.getByText('BACKEND_FPS:');
    const fpsValue2 = fpsLabel2.nextElementSibling as HTMLElement;

    expect(getElementStyle(fpsValue2).color).toBe('rgb(203, 213, 225)'); // #cbd5e1
    expect(getElementStyle(latencyValue2).color).toBe('rgb(100, 116, 139)'); // #64748b
  });

  it('Detect whitespace regressions in DOM textContent rendering', () => {
    const mockStatus: SimulationStatus = {
      running: true,
      tick_count: 42,
      avg_tick_time_ms: 1.25,
      fps: 60.0,
    };

    render(<StatusPanel status={mockStatus} />);

    // Get paragraph texts
    const ticksParagraph = screen.getByText('Số Ticks:').parentElement;
    const delayParagraph = screen.getByText('Độ trễ TB của Tick:').parentElement;
    const fpsParagraph = screen.getByText('Backend FPS:').parentElement;

    // Verify whitespace regression is resolved:
    expect(ticksParagraph?.textContent).toBe('Số Ticks: 42');
    expect(delayParagraph?.textContent).toBe('Độ trễ TB của Tick: 1.25 ms');
    expect(fpsParagraph?.textContent).toBe('Backend FPS: 60.0');

    // If we expect the standard "Số Ticks: 42", this test verifies it is NOT regressed
    const hasSpaceRegression = ticksParagraph?.textContent === 'Số Ticks:42';
    expect(hasSpaceRegression).toBe(false);
  });
});
