import { render, screen, act, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { App } from "../App";
import "@testing-library/jest-dom";
import { emit } from "@tauri-apps/api/event";

// Giả lập @tauri-apps/api/core
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation((command) => {
    if (command === "get_simulation_status") {
      return Promise.resolve({
        running: true,
        tick_count: 42,
        avg_tick_time_ms: 1.25,
        fps: 60.0,
      });
    }
    if (command === "toggle_simulation") {
      return Promise.resolve(true);
    }
    if (command === "get_map_elites_grid") {
      return Promise.resolve({
        grid: {},
        grid_resolution: 50,
      });
    }
    if (command === "get_pheromone_grid") {
      return Promise.resolve({
        grid: [],
        width: 128,
        height: 128,
      });
    }
    if (command === "get_active_raycasts") {
      return Promise.resolve([]);
    }
    if (command === "get_lineage_graph") {
      return Promise.resolve({
        nodes: [],
        links: [],
        db_connected: false,
      });
    }
    if (command === "get_chronicle_history") {
      return Promise.resolve([]);
    }
    return Promise.reject(`Unknown command: ${command}`);
  }),
}));

describe("Anima-Engine Frontend IPC Integration", () => {
  it("polls and renders the simulation status", async () => {
    render(<App />);
    
    await screen.findByText(/Số Ticks:/);
    const pTicks = screen.getByText(/Số Ticks:/).parentElement;
    expect(pTicks?.textContent).toContain("Số Ticks: 42");

    const pDelay = screen.getByText(/Độ trễ TB của Tick:/).parentElement;
    expect(pDelay?.textContent).toContain("Độ trễ TB của Tick: 1.25 ms");

    const pFps = screen.getByText(/Backend FPS:/).parentElement;
    expect(pFps?.textContent).toContain("Backend FPS: 60.0");
  });

  it("updates agent state when simulation-tick event is emitted", async () => {
    render(<App />);

    // Wait for the canvas to mount and listeners to register
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Kích hoạt giả lập bắn sự kiện IPC tick từ Rust với định dạng SegmentState[]
    await act(async () => {
      await emit("simulation-tick", [
        {
          agent_id: 1,
          segment_id: 0,
          parent_segment_id: null,
          x: 1.2,
          y: 3.4,
          z: 5.6,
          yaw: 0,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: 0,
          joint_axis_x: 0,
          joint_axis_y: 0,
          joint_axis_z: 0,
          energy: 95.5
        }
      ]);
    });

    const activeText = screen.getByText(/Số Agents hoạt động:/);
    expect(activeText.textContent).toContain("1");
    expect(screen.getByText(/1\.20, 3\.40, 5\.60/)).toBeInTheDocument();
  });

  it("verifies that the canvas rendering routine is executed and correct canvas APIs are invoked", async () => {
    const { container } = render(<App />);

    // Wait for the canvas to mount and initialize
    await waitFor(() => {
      expect(container.querySelector("canvas")).toBeInTheDocument();
    });
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;

    // Extra tick to let the async listen register
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const ctx = canvas.getContext("2d");
    expect(ctx).toBeDefined();

    // Trigger simulation-tick event
    await act(async () => {
      await emit("simulation-tick", [
        {
          agent_id: 1,
          segment_id: 0,
          parent_segment_id: null,
          x: 10,
          y: 20,
          z: 30,
          yaw: 0.5,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: 0,
          joint_axis_x: 0,
          joint_axis_y: 0,
          joint_axis_z: 0,
          energy: 80,
          agent_type: 'predator',
        },
        {
          agent_id: 1,
          segment_id: 1,
          parent_segment_id: 0,
          x: 15,
          y: 25,
          z: 35,
          yaw: 0.8,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: 0,
          joint_axis_x: 0,
          joint_axis_y: 0,
          joint_axis_z: 0,
          energy: 70,
          agent_type: 'prey',
        }
      ]);
    });

    // Wait a little for the requestAnimationFrame frame to execute
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Verify canvas APIs were invoked
    expect(ctx?.clearRect).toHaveBeenCalled();
    expect(ctx?.beginPath).toHaveBeenCalled();
    expect(ctx?.arc).toHaveBeenCalled();
    expect(ctx?.moveTo).toHaveBeenCalled();
    expect(ctx?.lineTo).toHaveBeenCalled();
    expect(ctx?.fill).toHaveBeenCalled();
  });
});
