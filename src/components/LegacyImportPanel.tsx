import { useCallback, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { LegacyImportListing } from '../types/generated/LegacyImportListing';

// ---------------------------------------------------------------------------------------
// Importing a save written before path confinement.
//
// # Why this panel exists
//
// `save_simulation_state` used to take any string the webview could produce and hand it to the
// filesystem. Closing that meant a save became a *name* resolved inside the app's own data
// directory — which also meant every save anyone already had, addressed by absolute path, stopped
// being loadable. The accepted design promised those stay reachable through a read-only, explicitly
// opt-in migration, and the backend implements one (`commands/simulation.rs`).
//
// Until this panel existed, no user could reach it. The commands were registered and nothing called
// them: there was no way to find out where to put the old file, no way to see what was there, and no
// way to name the result. A migration nobody can perform is not a migration, so the backend work was
// not finished by the backend work.
//
// # Why the authorising act is a file copy
//
// The obvious design is a file picker that hands the backend a path, and that reopens the exact hole
// the confinement closed — a compromised page would call it with an SSH key and read the parse
// error. So the act that authorises reading a file is one the page cannot perform: the user copies
// it into the drop directory themselves. This panel's job is therefore to *tell them where that is*
// and to name what it finds, not to browse a filesystem.
//
// The panel is only useful once, per old save, which is why it is collapsed by default rather than
// occupying the persistence card forever.
// ---------------------------------------------------------------------------------------

/** What the panel is doing right now. Kept as one value so two of them cannot both be true. */
type Phase =
  | { kind: 'idle' }
  | { kind: 'listing' }
  | { kind: 'importing'; name: string }
  | { kind: 'imported'; wrote: string };

const cardStyle: React.CSSProperties = {
  border: '1px solid #edf2f7',
  padding: '10px',
  borderRadius: '4px',
  marginTop: '10px',
};

const buttonStyle: React.CSSProperties = {
  padding: '6px',
  border: 'none',
  borderRadius: '4px',
  cursor: 'pointer',
  color: 'white',
};

export interface LegacyImportPanelProps {
  /** Called after a successful import, with the name that was written into `saves/`. */
  onImported?: (savedAs: string) => void;
}

export function LegacyImportPanel({ onImported }: LegacyImportPanelProps) {
  const [open, setOpen] = useState(false);
  const [listing, setListing] = useState<LegacyImportListing | null>(null);
  const [selected, setSelected] = useState('');
  const [saveAs, setSaveAs] = useState('');
  const [phase, setPhase] = useState<Phase>({ kind: 'idle' });
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setPhase({ kind: 'listing' });
    setError(null);
    try {
      const next = await invoke<LegacyImportListing>('list_legacy_saves');
      setListing(next);
      // Keep the current selection only if it is still there — a stale selection would send the
      // importer a name that has since been removed, and the error it returns would be about the
      // wrong thing.
      setSelected((s) => (next.names.includes(s) ? s : (next.names[0] ?? '')));
      setPhase({ kind: 'idle' });
    } catch (e) {
      setError(String(e));
      setPhase({ kind: 'idle' });
    }
  }, []);

  const open_ = useCallback(() => {
    setOpen(true);
    void refresh();
  }, [refresh]);

  const runImport = useCallback(async () => {
    if (!selected) return;
    setPhase({ kind: 'importing', name: selected });
    setError(null);
    try {
      const wrote = await invoke<string>('import_legacy_save', {
        legacy_name: selected,
        save_as: saveAs,
      });
      setPhase({ kind: 'imported', wrote });
      onImported?.(wrote);
    } catch (e) {
      // The backend's refusals are the useful text here — "save name contains '/'", "no file named
      // …" — so they are shown verbatim rather than replaced with a generic failure.
      setError(String(e));
      setPhase({ kind: 'idle' });
    }
  }, [selected, saveAs, onImported]);

  if (!open) {
    return (
      <div style={cardStyle}>
        <button
          data-testid="legacy-import-open"
          onClick={open_}
          style={{ ...buttonStyle, backgroundColor: '#718096', width: '100%' }}
        >
          Nhập bản lưu cũ (import a pre-2.0 save)
        </button>
      </div>
    );
  }

  const busy = phase.kind === 'listing' || phase.kind === 'importing';

  return (
    <div style={cardStyle} data-testid="legacy-import-panel">
      <h3>Nhập bản lưu cũ (Legacy import)</h3>

      <p style={{ fontSize: 12, color: '#4a5568', margin: '4px 0' }}>
        Bản lưu cũ dùng đường dẫn tuyệt đối. Ứng dụng chỉ đọc trong thư mục của chính nó, nên hãy
        chép tệp <code>.json</code> cũ vào thư mục dưới đây bằng trình quản lý tệp, rồi bấm{' '}
        <em>Làm mới</em>. Tệp gốc chỉ được đọc, không bị sửa hay xoá.
      </p>

      <div style={{ display: 'flex', gap: '8px', alignItems: 'center', margin: '6px 0' }}>
        <code
          data-testid="legacy-import-dir"
          style={{
            flex: 1,
            fontSize: 11,
            background: '#f7fafc',
            padding: '4px',
            borderRadius: '4px',
            overflowWrap: 'anywhere',
          }}
        >
          {listing?.directory ?? '…'}
        </code>
        <button
          data-testid="legacy-import-refresh"
          onClick={() => void refresh()}
          disabled={busy}
          style={{ ...buttonStyle, backgroundColor: '#3182ce' }}
        >
          Làm mới
        </button>
      </div>

      {phase.kind === 'listing' && (
        <p data-testid="legacy-import-loading" style={{ fontSize: 12 }}>
          Đang đọc thư mục…
        </p>
      )}

      {listing && listing.names.length === 0 && phase.kind !== 'listing' && (
        <p data-testid="legacy-import-empty" style={{ fontSize: 12, color: '#718096' }}>
          Chưa có tệp nào nhập được. Chép một bản lưu <code>.json</code> cũ vào thư mục trên.
        </p>
      )}

      {listing && listing.names.length > 0 && (
        <>
          <label htmlFor="legacy-import-select" style={{ fontSize: 12, color: '#4a5568' }}>
            Tệp cũ
          </label>
          <select
            id="legacy-import-select"
            data-testid="legacy-import-select"
            value={selected}
            onChange={(e) => setSelected(e.target.value)}
            disabled={busy}
            style={{ width: '100%', padding: '4px', margin: '2px 0 6px' }}
          >
            {listing.names.map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </>
      )}

      {/* Files that are present but cannot be imported. Reported rather than hidden: a user who
          dropped `My Save (old).sav` in and saw an empty list has been told nothing at all. */}
      {listing && listing.ignored.length > 0 && (
        <p data-testid="legacy-import-ignored" style={{ fontSize: 11, color: '#c05621' }}>
          Bỏ qua {listing.ignored.length} tệp không hợp lệ ({listing.ignored.join(', ')}). Tên phải
          kết thúc bằng <code>.json</code> và chỉ gồm chữ, số, <code>. _ -</code>.
        </p>
      )}

      <label htmlFor="legacy-import-save-as" style={{ fontSize: 12, color: '#4a5568' }}>
        Lưu thành (save name mới)
      </label>
      <input
        id="legacy-import-save-as"
        data-testid="legacy-import-save-as"
        type="text"
        value={saveAs}
        onChange={(e) => setSaveAs(e.target.value)}
        placeholder="imported_world"
        disabled={busy}
        style={{
          width: '100%',
          padding: '4px',
          border: '1px solid #cbd5e0',
          borderRadius: '4px',
          margin: '2px 0 6px',
        }}
      />

      <div style={{ display: 'flex', gap: '8px' }}>
        <button
          data-testid="legacy-import-run"
          onClick={() => void runImport()}
          disabled={busy || !selected || saveAs.trim() === ''}
          style={{ ...buttonStyle, backgroundColor: '#38a169', flex: 1 }}
        >
          {phase.kind === 'importing' ? 'Đang nhập…' : 'Nhập'}
        </button>
        <button
          data-testid="legacy-import-close"
          onClick={() => setOpen(false)}
          style={{ ...buttonStyle, backgroundColor: '#a0aec0' }}
        >
          Đóng
        </button>
      </div>

      {phase.kind === 'imported' && (
        <p data-testid="legacy-import-ok" style={{ fontSize: 12, color: '#2f855a', marginTop: 6 }}>
          Đã nhập thành <code>{phase.wrote}</code>. Nhập tên đó vào ô Save name ở trên rồi bấm Load
          State. Tệp gốc vẫn nằm nguyên trong thư mục nhập.
        </p>
      )}

      {error !== null && (
        <p data-testid="legacy-import-error" role="alert" style={{ fontSize: 12, color: '#c53030', marginTop: 6 }}>
          {error}
        </p>
      )}
    </div>
  );
}

export default LegacyImportPanel;
