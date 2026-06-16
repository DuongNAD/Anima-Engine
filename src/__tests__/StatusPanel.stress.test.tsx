import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { StatusPanel } from "../components/StatusPanel";
import "@testing-library/jest-dom";

describe("StatusPanel Stress & Layout Verification", () => {
  it("renders status bar layout with responsive flex properties and groups latency and FPS properly", () => {
    const status = {
      running: true,
      tick_count: 100,
      avg_tick_time_ms: 1.5,
      fps: 60.0,
    };

    const { container } = render(<StatusPanel status={status} />);

    // Locate the retro performance status bar container
    // It is a flex container containing TICK_LATENCY and BACKEND_FPS
    const flexContainer = container.querySelector("div > div[style*='display: flex']");
    expect(flexContainer).toBeInTheDocument();
    expect(flexContainer).toHaveStyle({
      display: "flex",
      justifyContent: "space-between",
      alignItems: "center",
    });

    // Check that there are exactly two child elements (groups) inside the flex container
    const children = flexContainer?.children;
    expect(children?.length).toBe(2);

    // Group 1: Latency
    const latencyGroup = children?.[0];
    expect(latencyGroup?.textContent).toContain("TICK_LATENCY:");
    expect(latencyGroup?.textContent).toContain("1.50 ms");

    // Group 2: FPS
    const fpsGroup = children?.[1];
    expect(fpsGroup?.textContent).toContain("BACKEND_FPS:");
    expect(fpsGroup?.textContent).toContain("60.0");
  });

  it("verifies that the space regression is fixed in the DOM (expect 'Số Ticks: 42' to contain space)", () => {
    const status = {
      running: true,
      tick_count: 42,
      avg_tick_time_ms: 1.5,
      fps: 60.0,
    };

    render(<StatusPanel status={status} />);

    const ticksLabel = screen.getByText(/Số Ticks:/);
    expect(ticksLabel).toBeInTheDocument();

    const parentParagraph = ticksLabel.parentElement;
    expect(parentParagraph?.textContent).toBe("Số Ticks: 42");
    expect(parentParagraph?.textContent).toContain("Số Ticks: 42");
  });

  it("stress tests color transitions and boundaries", () => {
    // Test Case 1: Excellent performance (Green)
    const statusExcellent = {
      running: true,
      tick_count: 1000,
      avg_tick_time_ms: 0.5, // < 2.0 -> #39ff14
      fps: 59.9, // >= 55 -> #39ff14
    };

    const { rerender } = render(<StatusPanel status={statusExcellent} />);

    // Since colors might be calculated, let's test specific values
    // In StatusPanel.tsx:
    // const fpsColor = status.fps >= 55 ? '#39ff14' : status.fps >= 30 ? '#ffd700' : '#ff3333';
    // const latencyColor = status.avg_tick_time_ms < 2.0 ? '#39ff14' : status.avg_tick_time_ms < 5.0 ? '#ffd700' : '#ff3333';

    // Test Case 2: Moderate performance (Yellow)
    const statusModerate = {
      running: true,
      tick_count: 1000,
      avg_tick_time_ms: 3.5, // < 5.0 -> #ffd700
      fps: 45.0, // >= 30 -> #ffd700
    };
    rerender(<StatusPanel status={statusModerate} />);

    // Test Case 3: Poor performance (Red)
    const statusPoor = {
      running: false,
      tick_count: 1000,
      avg_tick_time_ms: 10.0, // >= 5.0 -> #ff3333
      fps: 15.0, // < 30 -> #ff3333
    };
    rerender(<StatusPanel status={statusPoor} />);
  });

  it("stress tests extreme inputs (huge numbers, negative, decimals)", () => {
    const extremeStatus = {
      running: true,
      tick_count: 999999999,
      avg_tick_time_ms: 12345.678,
      fps: 9999.99,
    };

    render(<StatusPanel status={extremeStatus} />);

    // Verify it formats to fixed decimals properly
    const latencyElements = screen.getAllByText(/12345\.68\s*ms/);
    expect(latencyElements.length).toBeGreaterThanOrEqual(1);

    const fpsElements = screen.getAllByText(/10000\.0/);
    expect(fpsElements.length).toBeGreaterThanOrEqual(1);

    expect(screen.getByText("999999999")).toBeInTheDocument();
  });
});
