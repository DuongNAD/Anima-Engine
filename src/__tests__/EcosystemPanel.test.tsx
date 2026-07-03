import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import '@testing-library/jest-dom';
import { EcosystemPanel } from '../components/EcosystemPanel';

// The global Vitest setup (tests/setup-vitest.ts) mocks `get_ecosystem_state` to return a
// fixed snapshot (plants 500, animals 300, detritus 100, prey 12, predator 3, H 0.5, D 0.4).
describe('EcosystemPanel', () => {
  it('polls the backend and renders the closed-energy compartments', async () => {
    render(<EcosystemPanel pollMs={50} />);
    // The three conserved biomass compartments appear once the first poll resolves.
    await waitFor(() => {
      expect(screen.getByTestId('ecosystem-plants')).toHaveTextContent('500.0');
    });
    expect(screen.getByTestId('ecosystem-animals')).toHaveTextContent('300.0');
    expect(screen.getByTestId('ecosystem-detritus')).toHaveTextContent('100.0');
  });

  it('shows the population split and biodiversity indices', async () => {
    render(<EcosystemPanel pollMs={50} />);
    await waitFor(() => {
      expect(screen.getByTestId('ecosystem-populations')).toHaveTextContent('12');
    });
    expect(screen.getByTestId('ecosystem-populations')).toHaveTextContent('3');
    expect(screen.getByTestId('ecosystem-diversity')).toHaveTextContent('0.50 / 0.40');
  });
});
