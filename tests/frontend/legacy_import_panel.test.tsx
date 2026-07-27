import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { LegacyImportPanel } from '@/components/LegacyImportPanel';
import type { LegacyImportListing } from '@/types/generated/LegacyImportListing';

// The user-facing half of the legacy-save migration.
//
// The backend commands existed and were registered before this panel did, and nothing called them:
// a user could not find the drop directory, could not see what was in it, could not choose a
// destination name, and could not read a refusal. That is not a migration a user can perform, so
// these tests are about reachability as much as correctness — every step of the flow is driven here
// through the same DOM a person would use.
//
// Assertions are plain DOM ones: this suite does not load `@testing-library/jest-dom`, and adding a
// dependency to phrase `toBeInTheDocument` is not worth it.

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return { ...original, invoke };
});

const DIR = 'C:\\Users\\test\\AppData\\Roaming\\com.anima.engine\\legacy-import';

function listing(over: Partial<LegacyImportListing> = {}): LegacyImportListing {
  return { directory: DIR, names: ['old_world.json'], ignored: [], ...over };
}

/** Open the panel and wait for its first listing to land. */
async function openPanel(): Promise<void> {
  fireEvent.click(screen.getByTestId('legacy-import-open'));
  await waitFor(() => expect(screen.queryByTestId('legacy-import-panel')).not.toBeNull());
}

const text = (id: string): string => screen.getByTestId(id).textContent ?? '';
const disabled = (id: string): boolean => (screen.getByTestId(id) as HTMLButtonElement).disabled;

describe('LegacyImportPanel', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('is collapsed until asked for, and calls nothing before then', () => {
    // It is useful once per old save. Opening on mount would put a directory listing in front of
    // every user forever, and would call the backend on every dashboard load.
    render(<LegacyImportPanel />);
    expect(screen.queryByTestId('legacy-import-open')).not.toBeNull();
    expect(screen.queryByTestId('legacy-import-panel')).toBeNull();
    expect(invoke).not.toHaveBeenCalled();
  });

  it('shows the drop directory, because the user has to put the file somewhere', async () => {
    // The authorising act is a file copy the page cannot perform. Telling the user where to copy it
    // is therefore not a nicety; it is the only way the feature can be used at all.
    invoke.mockResolvedValue(listing());
    render(<LegacyImportPanel />);
    await openPanel();

    expect(invoke).toHaveBeenCalledWith('list_legacy_saves');
    await waitFor(() => expect(text('legacy-import-dir')).toBe(DIR));
  });

  it('refreshes on demand, since the user copies the file in after opening the panel', async () => {
    invoke.mockResolvedValueOnce(listing({ names: [] }));
    render(<LegacyImportPanel />);
    await openPanel();
    await waitFor(() => expect(screen.queryByTestId('legacy-import-empty')).not.toBeNull());

    invoke.mockResolvedValueOnce(listing({ names: ['dropped_in_later.json'] }));
    fireEvent.click(screen.getByTestId('legacy-import-refresh'));

    await waitFor(() =>
      expect((screen.getByTestId('legacy-import-select') as HTMLSelectElement).value).toBe(
        'dropped_in_later.json',
      ),
    );
    expect(screen.queryByTestId('legacy-import-empty')).toBeNull();
  });

  it('reports files it had to ignore rather than hiding them', async () => {
    // The listing only offers names that resolve to themselves — `old.txt` would be opened as
    // `old.txt.json`, which is not a file. Dropping those silently leaves a user staring at an empty
    // list holding a file they can see in the folder.
    invoke.mockResolvedValue(listing({ names: [], ignored: ['old.txt', 'My Save.sav'] }));
    render(<LegacyImportPanel />);
    await openPanel();

    await waitFor(() => {
      const note = text('legacy-import-ignored');
      expect(note).toContain('old.txt');
      expect(note).toContain('My Save.sav');
    });
  });

  it('imports the selected file under the name the user chose', async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === 'list_legacy_saves'
        ? Promise.resolve(listing({ names: ['a.json', 'b.json'] }))
        : Promise.resolve('restored.json'),
    );
    const onImported = vi.fn();
    render(<LegacyImportPanel onImported={onImported} />);
    await openPanel();
    await waitFor(() => expect(screen.queryByTestId('legacy-import-select')).not.toBeNull());

    fireEvent.change(screen.getByTestId('legacy-import-select'), { target: { value: 'b.json' } });
    fireEvent.change(screen.getByTestId('legacy-import-save-as'), { target: { value: 'restored' } });
    fireEvent.click(screen.getByTestId('legacy-import-run'));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('import_legacy_save', {
        legacy_name: 'b.json',
        save_as: 'restored',
      }),
    );
    // The name the backend actually wrote, not the one that was typed: the backend normalises the
    // extension, and the user's next action is to put this into the load field.
    await waitFor(() => expect(text('legacy-import-ok')).toContain('restored.json'));
    expect(onImported).toHaveBeenCalledWith('restored.json');
  });

  it('will not import without a destination name', async () => {
    // `save_as` is what the imported world will be called. Empty would reach the backend and come
    // back as "save name is empty", which is a worse way to learn it.
    invoke.mockResolvedValue(listing());
    render(<LegacyImportPanel />);
    await openPanel();
    await waitFor(() => expect(disabled('legacy-import-run')).toBe(true));

    fireEvent.change(screen.getByTestId('legacy-import-save-as'), { target: { value: 'ok_name' } });
    expect(disabled('legacy-import-run')).toBe(false);
  });

  it('shows the backend refusal verbatim', async () => {
    // The refusals are the useful text — "save name contains '/'", "no file named …". Replacing them
    // with a generic failure would leave the user with a rejected name and no way to fix it.
    invoke.mockImplementation((cmd: string) =>
      cmd === 'list_legacy_saves'
        ? Promise.resolve(listing())
        : Promise.reject(
            "save name contains '/'; only letters, digits, '.', '_' and '-' are allowed.",
          ),
    );
    render(<LegacyImportPanel />);
    await openPanel();
    await waitFor(() => expect(screen.queryByTestId('legacy-import-select')).not.toBeNull());

    fireEvent.change(screen.getByTestId('legacy-import-save-as'), { target: { value: 'sub/dir' } });
    fireEvent.click(screen.getByTestId('legacy-import-run'));

    const alert = await screen.findByTestId('legacy-import-error');
    expect(alert.textContent).toContain('only letters, digits');
    expect(alert.getAttribute('role')).toBe('alert');
    // Still usable afterwards: a failed import must not leave the panel stuck in its busy state.
    expect(disabled('legacy-import-run')).toBe(false);
  });

  it('surfaces a failure to even read the directory', async () => {
    invoke.mockRejectedValue('cannot read C:\\...\\legacy-import: Access is denied. (os error 5)');
    render(<LegacyImportPanel />);
    fireEvent.click(screen.getByTestId('legacy-import-open'));

    const alert = await screen.findByTestId('legacy-import-error');
    expect(alert.textContent).toContain('Access is denied');
  });
});
