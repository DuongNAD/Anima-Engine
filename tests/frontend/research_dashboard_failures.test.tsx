import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

import { DataExportDialog } from '../../src/components/ResearchDashboard/DataExportDialog';
import { EvolutionLineagePanel } from '../../src/components/ResearchDashboard/EvolutionLineagePanel';
import { TelemetryOverviewPanel } from '../../src/components/ResearchDashboard/TelemetryOverviewPanel';

// Card G4: when a backend command fails, the Research Dashboard shows the failure. Before G4 a
// failed export installed a mock response, a failed toggle_evolution flipped the state anyway, a
// failed Newick export printed a made-up tree, and the demographic panels drew invented cohorts.
//
// tests/setup-vitest.ts answers every command through mockIPC, which routes invoke() via
// window.__TAURI_INTERNALS__.invoke. These tests make one command reject there and let every other
// command through to the shared mock.

type IpcInvoke = (cmd: string, ...rest: unknown[]) => Promise<unknown>;

function failCommand(failing: string) {
  const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: IpcInvoke } }).__TAURI_INTERNALS__;
  const original = internals.invoke.bind(internals);
  return vi.spyOn(internals, 'invoke').mockImplementation((cmd: string, ...rest: unknown[]) =>
    cmd === failing ? Promise.reject(new Error(`${cmd} failed in test`)) : original(cmd, ...rest),
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Research Dashboard: failed commands are shown, not papered over (card G4)', () => {
  it('a rejected export shows the error and no preview or provenance', async () => {
    const ipc = failCommand('export_scientific_dataset');
    render(<DataExportDialog onClose={() => {}} />);

    fireEvent.click(screen.getByTestId('execute-export-dataset-btn'));

    await waitFor(() => {
      expect(screen.getByText(/Export failed: .*export_scientific_dataset failed in test/)).toBeInTheDocument();
    });
    expect(ipc.mock.calls.map((c) => c[0])).toContain('export_scientific_dataset');
    expect(screen.queryByTestId('export-preview-content')).not.toBeInTheDocument();
    expect(screen.getByTestId('run-provenance-pending')).toBeInTheDocument();
    expect(screen.getByTestId('run-provenance-card')).not.toHaveTextContent('c320fc0');
  });

  it('a failed toggle_evolution leaves the state as it was and says why', async () => {
    const ipc = failCommand('toggle_evolution');
    render(<EvolutionLineagePanel isRunning={false} />);

    const toggle = screen.getByTestId('toggle-evolution-btn');
    expect(toggle).toHaveTextContent('Start Evolution');
    fireEvent.click(toggle);

    await waitFor(() => {
      expect(screen.getByTestId('evolution-error')).toHaveTextContent('Evolution state unchanged');
    });
    expect(ipc.mock.calls.map((c) => c[0])).toContain('toggle_evolution');
    expect(screen.getByTestId('toggle-evolution-btn')).toHaveTextContent('Start Evolution');
  });

  it('a failed Newick export prints no tree', async () => {
    failCommand('export_phylogenetic_tree');
    render(<EvolutionLineagePanel isRunning={false} />);

    fireEvent.click(screen.getByTestId('export-newick-button'));

    await waitFor(() => {
      expect(screen.getByTestId('evolution-error')).toHaveTextContent('Newick export failed');
    });
    expect(screen.queryByTestId('newick-export-output')).not.toBeInTheDocument();
  });

  it('a rejected settings update puts the slider back on the value the backend still has', async () => {
    failCommand('update_evolution_settings');
    render(<EvolutionLineagePanel isRunning={false} />);

    const slider = screen.getByTestId('evolution-mutation-rate-slider') as HTMLInputElement;
    const before = slider.value;
    fireEvent.change(slider, { target: { value: '0.42' } });

    await waitFor(() => {
      expect(screen.getByTestId('evolution-error')).toHaveTextContent('Mutation rate not applied');
    });
    expect((screen.getByTestId('evolution-mutation-rate-slider') as HTMLInputElement).value).toBe(before);
  });

  it('before any census arrives the demographic panels say so instead of drawing cohorts', () => {
    // Render without letting the census command answer: the first paint must not invent data.
    failCommand('get_demographic_census');
    render(<TelemetryOverviewPanel isRunning={false} />);

    expect(screen.getByTestId('age-cohorts-unreported')).toBeInTheDocument();
    expect(screen.getByTestId('mortality-causes-unreported')).toBeInTheDocument();
    expect(screen.queryByText(/individuals/)).not.toBeInTheDocument();
    expect(screen.queryByText(/deaths/)).not.toBeInTheDocument();
  });

  it('"Copied" appears only after the clipboard write succeeds', async () => {
    const writeText = vi.fn().mockRejectedValue(new Error('Permission denied'));
    Object.assign(navigator, { clipboard: { writeText } });
    render(<DataExportDialog onClose={() => {}} />);

    fireEvent.click(screen.getByTestId('execute-export-dataset-btn'));
    await waitFor(() => expect(screen.getByTestId('export-copy-btn')).toBeInTheDocument());

    fireEvent.click(screen.getByTestId('export-copy-btn'));
    await waitFor(() => expect(writeText).toHaveBeenCalled());
    expect(screen.getByTestId('export-copy-btn')).not.toHaveTextContent('Copied');

    // Positive control: a successful write does show it.
    writeText.mockResolvedValue(undefined);
    fireEvent.click(screen.getByTestId('export-copy-btn'));
    await waitFor(() => expect(screen.getByTestId('export-copy-btn')).toHaveTextContent('Copied'));
  });
});
