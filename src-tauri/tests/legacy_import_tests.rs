//! The legacy-save migration, exercised against a real filesystem.
//!
//! # Why this file exists separately from the resolver's unit tests
//!
//! `save_paths.rs` proves things about *names*: that `../evil` is refused, that an accepted name
//! lands in the directory it was given. Those are worth having and they do not prove the migration.
//! What the accepted design promises is behavioural — an old save stays loadable, through a path that
//! is read-only and explicitly opted into — and the only way to show that is to put a real legacy
//! file on a real disk, import it, and look at what changed.
//!
//! An earlier version of these tests argued the read-only property from a resolver identity: because
//! `resolve_save_path` joins onto the directory it is handed, it "cannot" write into the import
//! directory. That is not the claim. `resolve_save_path(&import_dir, "x")` happily returns a path
//! inside the import directory — the reason nothing is written there is that the importer is never
//! *given* the import directory as its save root. That is a property of the importer, so it is tested
//! by running the importer with two separate roots and comparing the source directory byte for byte
//! before and after.

use anima_engine_lib::commands::simulation::{import_legacy_save_into, list_legacy_saves_in};
use anima_engine_lib::core::simulation_lifecycle::SavedSimulationState;
use anima_engine_lib::core::snapshot::{SnapshotEnvelope, SCHEMA_VERSION};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A pair of temporary roots that removes itself, so a failing assertion cannot leak a directory
/// into `%TEMP%` that the next run then finds already populated.
struct Roots {
    base: PathBuf,
    import: PathBuf,
    saves: PathBuf,
}

impl Roots {
    fn new(label: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "anima_legacy_import_{}_{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let import = base.join("legacy-import");
        let saves = base.join("saves");
        std::fs::create_dir_all(&import).expect("create import dir");
        std::fs::create_dir_all(&saves).expect("create saves dir");
        Self {
            base,
            import,
            saves,
        }
    }
}

impl Drop for Roots {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// A pre-envelope save: a bare `SavedSimulationState`, exactly what the app wrote before G1.2.
fn legacy_bytes(tick_count: u64) -> Vec<u8> {
    let mut state = anima_engine_lib::core::simulation_state::empty_saved_state_for_tests();
    state.tick_count = tick_count;
    serde_json::to_vec_pretty(&state).expect("a saved state serialises")
}

/// Every file in `dir`, name to bytes. The comparison unit for "nothing here changed".
fn snapshot_dir(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("dir entry");
        if entry.file_type().expect("file type").is_file() {
            out.insert(
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).expect("read file"),
            );
        }
    }
    out
}

#[test]
fn a_pre_envelope_save_imports_into_a_current_sealed_envelope() {
    let roots = Roots::new("roundtrip");
    let original = legacy_bytes(4242);
    let source_path = roots.import.join("old_world.json");
    std::fs::write(&source_path, &original).expect("write legacy save");

    // The fixture really is a pre-envelope save, so the test below exercises migration rather than a
    // copy. `loaded_from_schema` is `#[serde(skip)]` — it describes a *read*, not the state — so this
    // is the only place it can be observed, and it is why it cannot be asserted after the re-seal.
    let as_read =
        anima_engine_lib::core::snapshot::read(&source_path).expect("the legacy file reads");
    assert_eq!(
        as_read.loaded_from_schema, 1,
        "a bare state with no energy fields is schema 1"
    );

    let written = import_legacy_save_into(&roots.import, &roots.saves, "old_world", "restored")
        .expect("a legacy save must import");
    assert_eq!(written, "restored.json");

    // The destination is a *current* envelope, not a copy of the old bytes: sealed at the current
    // schema, and passing its own checksum. This is the difference between "the migration ran" and
    // "the file was copied".
    let dest = roots.saves.join("restored.json");
    let bytes = std::fs::read(&dest).expect("the imported save exists");
    let envelope: SnapshotEnvelope =
        serde_json::from_slice(&bytes).expect("the destination is an envelope");
    assert_eq!(envelope.schema_version, SCHEMA_VERSION);
    envelope.verify().expect("the imported envelope verifies");

    // ...and it carries the world that was in the legacy file, not a default one.
    let state: SavedSimulationState = envelope.parse_state().expect("the state parses");
    assert_eq!(state.tick_count, 4242);

    // The destination goes through the ordinary load path — the one `load_simulation_state` uses —
    // and comes back as a current-schema read. That is what "still loadable" has to mean.
    let reloaded = anima_engine_lib::core::snapshot::read(&dest).expect("the imported save loads");
    assert_eq!(reloaded.tick_count, 4242);
    assert_eq!(reloaded.loaded_from_schema, SCHEMA_VERSION);

    // The source is untouched, byte for byte. "Read-only" is the promise; this is the check.
    let after =
        std::fs::read(roots.import.join("old_world.json")).expect("the source still exists");
    assert_eq!(
        after, original,
        "the legacy file must not be rewritten, truncated or re-sealed in place"
    );
}

#[test]
fn the_import_writes_nothing_into_the_directory_it_read_from() {
    // The real form of the read-only claim: run the actual importer with two separate roots and
    // compare the whole source directory before and after. No resolver identity, no comment.
    let roots = Roots::new("readonly");
    std::fs::write(roots.import.join("a.json"), legacy_bytes(1)).expect("write a");
    std::fs::write(roots.import.join("b.json"), legacy_bytes(2)).expect("write b");
    let before = snapshot_dir(&roots.import);

    import_legacy_save_into(&roots.import, &roots.saves, "a.json", "imported_a").expect("import a");
    import_legacy_save_into(&roots.import, &roots.saves, "b", "imported_b").expect("import b");

    assert_eq!(
        snapshot_dir(&roots.import),
        before,
        "the import directory gained, lost or changed a file"
    );
    // And the destination really did receive both, so the equality above is not the trivial one that
    // holds when nothing happened at all.
    let saved = snapshot_dir(&roots.saves);
    assert_eq!(
        saved.keys().collect::<Vec<_>>(),
        vec!["imported_a.json", "imported_b.json"]
    );
}

#[test]
fn every_listed_name_is_a_name_the_importer_can_open() {
    // The defect this is the regression for: the listing returned raw directory entries for anything
    // that merely *passed* sanitisation, and sanitisation normalises. `old.txt` was listed as
    // `old.txt` and imported as `old.txt.json`, which is not a file that exists.
    let roots = Roots::new("listing");
    std::fs::write(roots.import.join("good.json"), legacy_bytes(1)).expect("write good");
    std::fs::write(roots.import.join("also-good.json"), legacy_bytes(2)).expect("write also");
    std::fs::write(roots.import.join("old.txt"), legacy_bytes(3)).expect("write old.txt");
    std::fs::write(roots.import.join("no-extension"), legacy_bytes(4)).expect("write bare");
    std::fs::write(roots.import.join("has space.json"), legacy_bytes(5)).expect("write spaced");
    std::fs::create_dir_all(roots.import.join("a-directory")).expect("create subdir");

    let listing = list_legacy_saves_in(&roots.import).expect("listing");
    assert_eq!(listing.names, vec!["also-good.json", "good.json"]);
    // Present but not importable, and *reported* — a user who dropped `old.txt` in has to be told
    // something other than an empty list.
    assert_eq!(
        listing.ignored,
        vec!["has space.json", "no-extension", "old.txt"]
    );
    assert_eq!(listing.directory, roots.import.to_string_lossy());

    // The property that failed before: every name the listing offers can actually be opened.
    for (i, name) in listing.names.iter().enumerate() {
        import_legacy_save_into(&roots.import, &roots.saves, name, &format!("out{i}"))
            .unwrap_or_else(|e| panic!("listed name {name:?} could not be imported: {e}"));
    }
}

#[test]
fn a_name_that_is_not_in_the_directory_is_refused_without_writing_a_save() {
    let roots = Roots::new("missing");
    let err = import_legacy_save_into(&roots.import, &roots.saves, "nothing_here", "dest")
        .expect_err("importing an absent file must fail");
    assert!(
        err.contains("no file named"),
        "the error should name the problem, got: {err}"
    );
    assert!(
        snapshot_dir(&roots.saves).is_empty(),
        "a failed import must not leave a file in the save directory"
    );
}

#[test]
fn a_path_shaped_argument_is_refused_on_either_side() {
    let roots = Roots::new("traversal");
    std::fs::write(roots.import.join("real.json"), legacy_bytes(7)).expect("write real");

    // The source side: the drop directory is addressed by name, so nothing outside it is reachable.
    for evil in [
        "../real",
        r"..\real",
        "/etc/passwd",
        r"C:\Users\me\.ssh\id_rsa",
        "real.json:stream",
        "CON",
    ] {
        assert!(
            import_legacy_save_into(&roots.import, &roots.saves, evil, "dest").is_err(),
            "{evil:?} was accepted as a legacy source"
        );
    }

    // The destination side: a migration is not a way to choose where a file lands either.
    for evil in ["../escape", r"..\escape", "sub/dir/escape", "NUL"] {
        assert!(
            import_legacy_save_into(&roots.import, &roots.saves, "real.json", evil).is_err(),
            "{evil:?} was accepted as an import destination"
        );
    }

    // Nothing escaped into the parent of either root while all that was being refused.
    let stray: Vec<_> = std::fs::read_dir(&roots.base)
        .expect("read base")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        stray.is_empty(),
        "files were written outside both roots: {stray:?}"
    );
}

#[test]
fn an_already_enveloped_save_imports_too_and_keeps_its_world() {
    // The drop directory is where a user puts "the old save I have", and for a user upgrading from a
    // build that already wrote envelopes, that file is an envelope. Importing one must work rather
    // than being an unhandled shape.
    let roots = Roots::new("enveloped");
    let mut state = anima_engine_lib::core::simulation_state::empty_saved_state_for_tests();
    state.tick_count = 99;
    let sealed = SnapshotEnvelope::seal(state).expect("seal");
    std::fs::write(
        roots.import.join("sealed.json"),
        serde_json::to_vec(&sealed).expect("serialise"),
    )
    .expect("write sealed");

    import_legacy_save_into(&roots.import, &roots.saves, "sealed.json", "from_sealed")
        .expect("an enveloped legacy file imports");

    let bytes = std::fs::read(roots.saves.join("from_sealed.json")).expect("read imported");
    let envelope: SnapshotEnvelope = serde_json::from_slice(&bytes).expect("envelope");
    envelope.verify().expect("verifies");
    assert_eq!(envelope.parse_state().expect("state").tick_count, 99);
}
