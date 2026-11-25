import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, render, screen, fireEvent, cleanup } from '@testing-library/react';
import '@testing-library/jest-dom';
import * as THREE from 'three';

import { CausalInterventionPanel } from '../../src/components/ResearchDashboard/CausalInterventionPanel';
import { Map2Showcase } from '../../src/components/Map2/Map2Showcase';
import * as Map2WildlifeModule from '../../src/components/Map2/Map2Wildlife';
import * as Map2CrystalsModule from '../../src/components/Map2/Map2FantasyCrystals';
import { DECORATION_LEGEND_TEXT } from '../../src/components/Landscape/utils/simEnvironment';

vi.setConfig({ testTimeout: 30_000 });

vi.mock('@react-three/fiber', () => ({
  Canvas: ({ children }: { children?: React.ReactNode }) => (
    <div data-testid="mock-r3f-canvas">{children}</div>
  ),
  useFrame: vi.fn(),
  useThree: () => ({
    camera: new THREE.PerspectiveCamera(),
    scene: new THREE.Scene(),
    gl: {
      setSize: vi.fn(),
      domElement: document.createElement('canvas'),
      shadowMap: { enabled: false, needsUpdate: false },
    },
    size: { width: 1280, height: 720 },
  }),
  extend: vi.fn(),
}));

const originalGetContext = HTMLCanvasElement.prototype.getContext;
const R3F_STUB_KEYS = [
  'setMatrixAt',
  'setColorAt',
  'instanceMatrix',
  'instanceColor',
  'computeBoundingBox',
] as const;

describe('C14 HONEST-UI negative controls', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    HTMLCanvasElement.prototype.getContext = function (
      this: HTMLCanvasElement,
      contextId: string,
      ...rest: unknown[]
    ) {
      const ctx = originalGetContext.apply(this, [contextId, ...rest] as never);
      if (ctx && contextId === '2d') {
        const ctxRecord = ctx as unknown as Record<string, unknown>;
        if (typeof ctxRecord.createImageData !== 'function') {
          ctxRecord.createImageData = (w: number, h: number) => ({
            width: w,
            height: h,
            data: new Uint8ClampedArray(w * h * 4),
          });
        }
        if (typeof ctxRecord.putImageData !== 'function') {
          ctxRecord.putImageData = vi.fn();
        }
      }
      return ctx;
    } as typeof HTMLCanvasElement.prototype.getContext;

    const proto = HTMLElement.prototype as unknown as Record<string, unknown>;
    proto.setMatrixAt = vi.fn();
    proto.setColorAt = vi.fn();
    proto.computeBoundingBox = vi.fn();
    proto.instanceMatrix = { needsUpdate: false };
    proto.instanceColor = { needsUpdate: false };
  });

  afterEach(() => {
    cleanup();
    HTMLCanvasElement.prototype.getContext = originalGetContext;
    const proto = HTMLElement.prototype as unknown as Record<string, unknown>;
    for (const key of R3F_STUB_KEYS) {
      delete proto[key];
    }
  });

  it('disables GeneticInjection in CausalInterventionPanel with "not implemented in the live world" and FINDINGS F-071 link', async () => {
    await act(async () => {
      render(<CausalInterventionPanel />);
    });

    const kindSelect = screen.getByTestId('intervention-kind-select') as HTMLSelectElement;
    const geneticOption = kindSelect.querySelector(
      'option[value="GeneticInjection"]',
    ) as HTMLOptionElement | null;

    expect(geneticOption).not.toBeNull();
    expect(geneticOption!.disabled).toBe(true);
    expect(geneticOption!.textContent?.toLowerCase()).toContain('not implemented in the live world');
    expect(geneticOption!.textContent).toContain('F-071');

    // Attempted selection must be refused so GeneticInjection is never offered as working.
    fireEvent.change(kindSelect, { target: { value: 'GeneticInjection' } });
    expect(kindSelect.value).not.toBe('GeneticInjection');

    // Always-visible explanation + clickable FINDINGS.md F-071 reference.
    const notice = screen.getByTestId('unimplemented-interventions-notice');
    expect(notice).toBeVisible();
    expect(notice.textContent?.toLowerCase()).toContain('not implemented in the live world');
    expect(notice.textContent).toContain('F-071');

    const link = screen.getByTestId('unimplemented-intervention-finding-link') as HTMLAnchorElement;
    expect(link).toBeVisible();
    expect(link.getAttribute('href')).toContain('FINDINGS.md');
    expect(link.textContent).toContain('F-071');

    // Positive control: implemented intervention kind stays enabled and selectable.
    const tempOption = kindSelect.querySelector(
      'option[value="TemperatureDelta"]',
    ) as HTMLOptionElement | null;
    expect(tempOption).not.toBeNull();
    expect(tempOption!.disabled).toBe(false);
    fireEvent.change(kindSelect, { target: { value: 'TemperatureDelta' } });
    expect(kindSelect.value).toBe('TemperatureDelta');
  });

  it('labels Map 2 wildlife and fantasy crystals with Landscape "Decoration, not simulated" legend', () => {
    const landscapePrefix = DECORATION_LEGEND_TEXT.split(':')[0];
    expect(landscapePrefix).toBe('Decoration, not simulated');

    render(<Map2Showcase />);

    const legend = screen.getByTestId('map2-decoration-legend');
    expect(legend).toBeVisible();
    expect(legend.textContent).toContain(landscapePrefix);
    expect(legend.textContent?.toLowerCase()).toContain('wildlife');
    expect(legend.textContent?.toLowerCase()).toContain('fantasy crystals');

    const wildlifeRoot = document.querySelector('[name="map2-wildlife-root"]');
    const crystalsRoot = document.querySelector('[name="map2-fantasy-crystals-root"]');
    expect(wildlifeRoot).not.toBeNull();
    expect(crystalsRoot).not.toBeNull();

    const wildlifeLabel = (Map2WildlifeModule as Record<string, unknown>)
      .MAP2_WILDLIFE_DECORATION_LABEL;
    const crystalsLabel = (Map2CrystalsModule as Record<string, unknown>)
      .MAP2_CRYSTALS_DECORATION_LABEL;

    expect(typeof wildlifeLabel).toBe('string');
    expect(String(wildlifeLabel)).toContain(landscapePrefix);
    expect(String(wildlifeLabel).toLowerCase()).toContain('wildlife');

    expect(typeof crystalsLabel).toBe('string');
    expect(String(crystalsLabel)).toContain(landscapePrefix);
    expect(String(crystalsLabel).toLowerCase()).toContain('fantasy crystals');
  });
});
