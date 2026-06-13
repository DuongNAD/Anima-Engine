import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act, screen, fireEvent } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import RabbitVisualizer from '../../playground/RabbitVisualizer';

const originalCreateElement = React.createElement;
// Inject data-testid using name attribute for R3F compatibility in JSDOM
// @ts-ignore
React.createElement = function (type: any, props: any, ...children: any[]) {
  if (props && typeof type === 'string' && (type === 'mesh' || type === 'group') && props.name) {
    props = { ...props, 'data-testid': props.name };
  }
  return originalCreateElement.apply(this, [type, props, ...children]);
} as any;

let frameCallbacks: Array<(state: any) => void> = [];

vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: any) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: (cb: any) => {
      frameCallbacks = [cb];
    }
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('RabbitVisualizer Boundary and Memory Alignment Tests (13-Float Layout)', () => {
  let consoleErrorSpy: any;
  let consoleWarnSpy: any;

  const getRealErrors = () => {
    return consoleErrorSpy.mock.calls.filter((call: any) => {
      const msg = call[0] ? call[0].toString() : '';
      return !msg.includes("incorrect casing") && 
             !msg.includes("unrecognized in this browser") &&
             !msg.includes("React does not recognize the");
    });
  };

  const getRealWarnings = () => {
    return consoleWarnSpy.mock.calls;
  };

  beforeEach(() => {
    vi.clearAllMocks();
    frameCallbacks = [];
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    consoleWarnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});

    screen.getByTestId = (id: string) => {
      const el = document.querySelector(`[name="${id}"]`);
      if (!el) {
        throw new Error(`Unable to find element with name="${id}"`);
      }
      return el as any;
    };
    
    // Inject mock properties for ref manipulations
    Object.defineProperty(HTMLElement.prototype, 'position', {
      get() {
        if (!this._mockPosition) {
          this._mockPosition = { set: vi.fn() };
        }
        return this._mockPosition;
      },
      configurable: true
    });

    Object.defineProperty(HTMLElement.prototype, 'rotation', {
      get() {
        if (!this._mockRotation) {
          this._mockRotation = { set: vi.fn() };
        }
        return this._mockRotation;
      },
      configurable: true
    });

    Object.defineProperty(HTMLElement.prototype, 'scale', {
      get() {
        if (!this._mockScale) {
          this._mockScale = { set: vi.fn() };
        }
        return this._mockScale;
      },
      configurable: true
    });

    Object.defineProperty(HTMLElement.prototype, 'material', {
      get() {
        if (!this._mockMaterial) {
          this._mockMaterial = {
            color: {
              setRGB: vi.fn(),
              set: vi.fn()
            }
          };
        }
        return this._mockMaterial;
      },
      configurable: true
    });
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
    consoleWarnSpy.mockRestore();
    delete (HTMLElement.prototype as any).position;
    delete (HTMLElement.prototype as any).rotation;
    delete (HTMLElement.prototype as any).scale;
    delete (HTMLElement.prototype as any).material;
  });

  it('1. Correct memory alignment: aligns 13 floats returned from Rust correctly', async () => {
    // 10 parts, each with 13 floats: x, y, z, rx, ry, rz, sx, sy, sz, r, g, b, part_type
    const floatData = new Float32Array([
      // Part 0: Body (0.0)
      1.0, 2.0, 0.0, 0.0, 0.0, 0.5, 2.0, 2.0, 2.0, 0.9, 0.8, 0.7, 0.0,
      // Part 1: Head (1.0)
      2.0, 3.0, 0.0, 0.0, 0.0, 0.6, 1.2, 1.2, 1.2, 0.8, 0.7, 0.6, 1.0,
      // Part 2: Left Ear (2.0)
      3.0, 4.0, 0.5, 0.0, 0.0, 0.7, 0.8, 0.8, 0.8, 0.7, 0.6, 0.5, 2.0,
      // Part 3: Right Ear (3.0)
      4.0, 5.0, -0.5, 0.0, 0.0, 0.8, 0.8, 0.8, 0.8, 0.6, 0.5, 0.4, 3.0,
      // Part 4: Front-Left Leg (4.0)
      5.0, 6.0, 0.5, 0.0, 0.0, 0.9, 0.8, 0.8, 0.8, 0.5, 0.4, 0.3, 4.0,
      // Part 5: Front-Right Leg (5.0)
      6.0, 7.0, -0.5, 0.0, 0.0, 1.0, 0.8, 0.8, 0.8, 0.5, 0.4, 0.3, 5.0,
      // Part 6: Hind-Left Leg (6.0)
      7.0, 8.0, 0.6, 0.0, 0.0, 1.1, 1.4, 1.4, 1.4, 0.4, 0.3, 0.2, 6.0,
      // Part 7: Hind-Right Leg (7.0)
      8.0, 9.0, -0.6, 0.0, 0.0, 1.2, 1.4, 1.4, 1.4, 0.4, 0.3, 0.2, 7.0,
      // Part 8: Tail (8.0)
      9.0, 10.0, 0.0, 0.0, 0.0, 1.3, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 8.0,
      // Part 9: Mouth (9.0)
      10.0, 11.0, 0.0, 0.0, 0.0, 1.4, 0.3, 0.2, 0.3, 0.9, 0.7, 0.7, 9.0
    ]);

    vi.mocked(invoke).mockResolvedValue(floatData.buffer);

    render(<RabbitVisualizer />);

    // Trigger useEffect Fetch
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const bodyEl = screen.getByTestId('rabbit-body') as any;
    const headEl = screen.getByTestId('rabbit-head') as any;
    const leftEarEl = screen.getByTestId('rabbit-left-ear') as any;
    const rightEarEl = screen.getByTestId('rabbit-right-ear') as any;
    const frontLeftLegEl = screen.getByTestId('rabbit-front-left-leg') as any;
    const frontRightLegEl = screen.getByTestId('rabbit-front-right-leg') as any;
    const hindLeftLegEl = screen.getByTestId('rabbit-hind-left-leg') as any;
    const hindRightLegEl = screen.getByTestId('rabbit-hind-right-leg') as any;
    const tailEl = screen.getByTestId('rabbit-tail') as any;
    const jawEl = screen.getByTestId('rabbit-jaw') as any;

    // Verify Body (Offset 0)
    expect(bodyEl.position.set).toHaveBeenCalledWith(floatData[0], floatData[1], floatData[2]);
    expect(bodyEl.rotation.set).toHaveBeenCalledWith(floatData[3], floatData[4], floatData[5]);
    expect(bodyEl.scale.set).toHaveBeenCalledWith(floatData[6], floatData[7], floatData[8]);
    expect(bodyEl.material.color.setRGB).toHaveBeenCalledWith(floatData[9], floatData[10], floatData[11]);

    // Verify Head (Offset 13)
    expect(headEl.position.set).toHaveBeenCalledWith(floatData[13], floatData[14], floatData[15]);
    expect(headEl.rotation.set).toHaveBeenCalledWith(floatData[16], floatData[17], floatData[18]);
    expect(headEl.scale.set).toHaveBeenCalledWith(floatData[19], floatData[20], floatData[21]);

    // Verify Left Ear (Offset 26)
    expect(leftEarEl.position.set).toHaveBeenCalledWith(floatData[26], floatData[27], floatData[28]);

    // Verify Right Ear (Offset 39)
    expect(rightEarEl.position.set).toHaveBeenCalledWith(floatData[39], floatData[40], floatData[41]);

    // Verify Front Legs (Offsets 52 and 65)
    expect(frontLeftLegEl.position.set).toHaveBeenCalledWith(floatData[52], floatData[53], floatData[54]);
    expect(frontRightLegEl.position.set).toHaveBeenCalledWith(floatData[65], floatData[66], floatData[67]);

    // Verify Hind Legs (Offsets 78 and 91)
    expect(hindLeftLegEl.position.set).toHaveBeenCalledWith(floatData[78], floatData[79], floatData[80]);
    expect(hindRightLegEl.position.set).toHaveBeenCalledWith(floatData[91], floatData[92], floatData[93]);

    // Verify Tail (Offset 104)
    expect(tailEl.position.set).toHaveBeenCalledWith(floatData[104], floatData[105], floatData[106]);

    // Verify Jaw (Offset 117)
    expect(jawEl.position.set).toHaveBeenCalledWith(floatData[117], floatData[118], floatData[119]);

    expect(getRealErrors()).toHaveLength(0);
  });

  it('2. Boundary check: Empty buffer returns gracefully', async () => {
    vi.mocked(invoke).mockResolvedValue(new ArrayBuffer(0));
    render(<RabbitVisualizer />);
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const bodyEl = screen.getByTestId('rabbit-body') as any;
    expect(bodyEl.position.set).not.toHaveBeenCalled();
    expect(getRealErrors()).toHaveLength(0);
  });

  it('3. Boundary check: Null response does not throw or crash, behaves as empty buffer', async () => {
    vi.mocked(invoke).mockResolvedValue(null as any);
    render(<RabbitVisualizer />);
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const bodyEl = screen.getByTestId('rabbit-body') as any;
    expect(bodyEl.position.set).not.toHaveBeenCalled();
    expect(getRealErrors()).toHaveLength(0);
  });

  it('4. Boundary check: IPC rejection is caught by try-catch and logs warning without crashing', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("Network connection lost"));
    render(<RabbitVisualizer />);
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const bodyEl = screen.getByTestId('rabbit-body') as any;
    expect(bodyEl.position.set).not.toHaveBeenCalled();
    
    const realWarnings = getRealWarnings();
    expect(realWarnings.length).toBeGreaterThanOrEqual(1);
    expect(realWarnings[0][0]).toContain("Unable to fetch Tauri state, switching to browser simulation:");
  });

  it('5. Boundary check: Buffer with unexpected length (not multiple of 13) is handled without crash', async () => {
    // 16 floats: Part 1 complete (13 floats), Part 2 incomplete (3 floats)
    const floatData = new Float32Array([
      1.0, 2.0, 0.0, 0.0, 0.0, 0.5, 2.0, 2.0, 2.0, 0.9, 0.8, 0.7, 0.0, // Part 1
      2.0, 3.0, 0.0 // Part 2 (incomplete)
    ]);

    vi.mocked(invoke).mockResolvedValue(floatData.buffer);

    render(<RabbitVisualizer />);
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const bodyEl = screen.getByTestId('rabbit-body') as any;
    const headEl = screen.getByTestId('rabbit-head') as any;

    expect(bodyEl.position.set).toHaveBeenCalled();
    expect(headEl.position.set).not.toHaveBeenCalled();
    expect(getRealErrors()).toHaveLength(0);
  });

  it('6. Browser animation: triggers useFrame and updates mesh positions using JS simulation', async () => {
    render(<RabbitVisualizer />);

    const stateMock = {
      clock: {
        getElapsedTime: () => 1.0,
      },
    };
    
    act(() => {
      frameCallbacks.forEach(cb => cb(stateMock));
    });

    const bodyEl = screen.getByTestId('rabbit-body') as any;
    expect(bodyEl.position.set).toHaveBeenCalled();
    expect(getRealErrors()).toHaveLength(0);
  });

  it('7. Boundary check: handles NaN, Infinity, and extreme values gracefully without throwing', async () => {
    const floatData = new Float32Array([
      NaN, Infinity, -Infinity, NaN, NaN, NaN, NaN, NaN, NaN, 1.0, 1.0, 1.0, 0.0
    ]);

    vi.mocked(invoke).mockResolvedValue(floatData.buffer);

    render(<RabbitVisualizer />);
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const bodyEl = screen.getByTestId('rabbit-body') as any;
    expect(bodyEl.position.set).toHaveBeenCalledWith(NaN, Infinity, -Infinity);
    expect(getRealErrors()).toHaveLength(0);
  });

  it('8. Boundary check: handles corrupt string response from Tauri gracefully', async () => {
    vi.mocked(invoke).mockResolvedValue("corrupt_string_data");
    render(<RabbitVisualizer />);
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const bodyEl = screen.getByTestId('rabbit-body') as any;
    expect(bodyEl.position.set).not.toHaveBeenCalled();
    expect(getRealErrors()).toHaveLength(0);
  });

  it('9. Boundary check: handles unaligned Uint8Array offset response from Tauri gracefully by catching/recovering', async () => {
    const mainBuffer = new ArrayBuffer(520 + 4);
    // Construct unaligned Uint8Array at 1-byte offset
    const unalignedUint8 = new Uint8Array(mainBuffer, 1, 520);
    vi.mocked(invoke).mockResolvedValue(unalignedUint8);

    render(<RabbitVisualizer />);
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    // Verify recovery or fallback does not throw and is clean
    expect(getRealErrors()).toHaveLength(0);
  });

  it('10. Chewing animation: updates jaw position/rotation cyclically in browser simulation', async () => {
    render(<RabbitVisualizer />);

    const jawEl = screen.getByTestId('rabbit-jaw') as any;
    expect(jawEl).toBeDefined();

    vi.clearAllMocks();

    // Toggle chewing button first (it defaults to IDLE / false)
    const eatButton = screen.getByText(/Chewing Animation/i).nextElementSibling as HTMLElement;
    fireEvent.click(eatButton);

    // Trigger frame 1 (t = 0.0)
    act(() => {
      frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 0.0 } }));
    });
    const firstCallY = jawEl.position.set.mock.calls[0]?.[1];

    // Trigger frame 2 (t = 0.25)
    act(() => {
      frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 0.25 } }));
    });
    const secondCallY = jawEl.position.set.mock.calls[1]?.[1];

    // Assert vertical chewing oscillation occurred
    expect(firstCallY).not.toEqual(secondCallY);
    expect(jawEl.position.set).toHaveBeenCalled();
  });

  it('11a. Hunger eye colors: toggling hunger changes eye mesh material colors to red', async () => {
    render(<RabbitVisualizer />);

    const leftEyeEl = screen.getByTestId('rabbit-eye-left') as any;
    const rightEyeEl = screen.getByTestId('rabbit-eye-right') as any;

    expect(leftEyeEl).toBeDefined();
    expect(rightEyeEl).toBeDefined();

    vi.clearAllMocks();

    // Toggle hunger button
    const hungerButton = screen.getByText(/Hunger State/i).nextElementSibling as HTMLElement;
    fireEvent.click(hungerButton);

    // Apply frame tick to render
    act(() => {
      frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 1.0 } }));
    });

    // Check that eye materials were set to Red
    expect(leftEyeEl.material.color.setRGB).toHaveBeenCalledWith(1.0, 0.0, 0.0);
    expect(rightEyeEl.material.color.setRGB).toHaveBeenCalledWith(1.0, 0.0, 0.0);
  });

  it('11b. Hunger eye colors: parses eye colors from Tauri IPC buffer response', async () => {
    // Generate 130 floats where parts 8 and 9 represent eyes and are colored red (1.0, 0.0, 0.0)
    const floatData = new Float32Array(130);
    
    // Left eye (Part Index 8)
    floatData[8 * 13 + 9] = 1.0; // r
    floatData[8 * 13 + 10] = 0.0; // g
    floatData[8 * 13 + 11] = 0.0; // b
    floatData[8 * 13 + 12] = 7.0; // part_type = 7.0 (Eye)

    // Right eye (Part Index 9)
    floatData[9 * 13 + 9] = 1.0; // r
    floatData[9 * 13 + 10] = 0.0; // g
    floatData[9 * 13 + 11] = 0.0; // b
    floatData[9 * 13 + 12] = 7.0; // part_type = 7.0 (Eye)

    vi.mocked(invoke).mockResolvedValue(floatData.buffer);

    render(<RabbitVisualizer />);
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const leftEyeEl = screen.getByTestId('rabbit-eye-left') as any;
    const rightEyeEl = screen.getByTestId('rabbit-eye-right') as any;

    expect(leftEyeEl.material.color.setRGB).toHaveBeenCalledWith(1.0, 0.0, 0.0);
    expect(rightEyeEl.material.color.setRGB).toHaveBeenCalledWith(1.0, 0.0, 0.0);
  });

  it('11c. Hunger eye colors: parses eye colors from production Tauri IPC buffer response (indices 10 & 11)', async () => {
    // Generate 156 floats (12 parts: 0-9 normal, 10-11 eyes)
    const floatData = new Float32Array(156);
    
    // Set standard types for parts 0-9
    floatData[0 * 13 + 12] = 0.0; // Body
    floatData[1 * 13 + 12] = 1.0; // Head
    floatData[2 * 13 + 12] = 2.0; // Left Ear
    floatData[3 * 13 + 12] = 3.0; // Right Ear
    floatData[4 * 13 + 12] = 4.0; // FL Leg
    floatData[5 * 13 + 12] = 5.0; // FR Leg
    floatData[6 * 13 + 12] = 6.0; // HL Leg
    floatData[7 * 13 + 12] = 7.0; // HR Leg
    floatData[8 * 13 + 12] = 8.0; // Tail
    floatData[9 * 13 + 12] = 9.0; // Mouth

    // Left eye (Part Index 10)
    floatData[10 * 13 + 9] = 1.0; // r
    floatData[10 * 13 + 10] = 0.0; // g
    floatData[10 * 13 + 11] = 0.0; // b
    floatData[10 * 13 + 12] = 7.0; // part_type = 7.0 (Eye)

    // Right eye (Part Index 11)
    floatData[11 * 13 + 9] = 1.0; // r
    floatData[11 * 13 + 10] = 0.0; // g
    floatData[11 * 13 + 11] = 0.0; // b
    floatData[11 * 13 + 12] = 7.0; // part_type = 7.0 (Eye)

    vi.mocked(invoke).mockResolvedValue(floatData.buffer);

    render(<RabbitVisualizer />);
    const tauriButton = screen.getByText('Live Tauri State');
    fireEvent.click(tauriButton);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const leftEyeEl = screen.getByTestId('rabbit-eye-left') as any;
    const rightEyeEl = screen.getByTestId('rabbit-eye-right') as any;

    expect(leftEyeEl.material.color.setRGB).toHaveBeenCalledWith(1.0, 0.0, 0.0);
    expect(rightEyeEl.material.color.setRGB).toHaveBeenCalledWith(1.0, 0.0, 0.0);
  });
});
