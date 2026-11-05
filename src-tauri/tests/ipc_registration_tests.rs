//! Every `#[tauri::command]` is reachable from the webview.
//!
//! # The failure mode
//!
//! A command is declared in `src/commands/`, compiles, gets a `ts_rs` binding generated into
//! `src/types/generated/`, and is named in `PROJECT.md` — and is missing from the
//! `tauri::generate_handler![…]` list in `src/lib.rs`, so `invoke('…')` rejects with
//! `Unknown command`. Nothing else in the tree can see it:
//!
//! - the compiler cannot: a `pub fn` nobody registered is not dead code, it is public API;
//! - `cargo clippy` cannot, for the same reason;
//! - `scripts/check_ipc_arg_case.mjs` cannot: it checks argument *spelling* at call sites that
//!   exist, and an unregistered command usually has no call site yet;
//! - `ts-rs` cannot: it exports the payload type whether or not the command is reachable;
//! - the frontend tests cannot: they mock `invoke`, and a mock answers every name.
//!
//! It is the same silence as the ipc-argument-case bug this repository already has a gate for — a
//! feature that is complete, typed and documented on one side of a boundary and absent on the other.
//!
//! # Why the scanners strip nothing and match line starts instead
//!
//! An earlier count of the commands in this tree said 32 when there were 31. The extra hit was a
//! **comment** in `commands/evolution.rs` that quotes `#[tauri::command]` while explaining the
//! argument-case rule. A scanner that greps anywhere in a line finds it; one that requires the
//! attribute to *begin* the line does not, and does not need comment-stripping to be right.
//! `the_scanners_can_actually_fail` is the control that keeps that honest.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Command names declared in one Rust source.
///
/// A declaration is a line whose trimmed text **starts with** `#[tauri::command]`, followed within
/// a few lines by the `fn`. The forward scan tolerates the attributes that sit between them
/// (`#[allow(…)]`, a doc comment) without tolerating an arbitrary distance.
fn declared_commands(source: &str) -> BTreeSet<String> {
    const ATTRIBUTE: &str = "#[tauri::command]";
    let lines: Vec<&str> = source.lines().collect();
    let mut out = BTreeSet::new();

    for (i, line) in lines.iter().enumerate() {
        if !line.trim_start().starts_with(ATTRIBUTE) {
            continue;
        }
        for probe in lines.iter().skip(i + 1).take(6) {
            let t = probe.trim_start();
            let t = t.strip_prefix("pub ").unwrap_or(t);
            let t = t.strip_prefix("async ").unwrap_or(t);
            if let Some(rest) = t.strip_prefix("fn ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    out.insert(name);
                }
                break;
            }
        }
    }
    out
}

/// Command names inside `tauri::generate_handler![ … ]`.
///
/// Scans for every `commands::` occurrence rather than splitting the list on commas. The comma
/// version dropped the **first** entry, because that entry still carried the `generate_handler![`
/// text in front of it and so failed a `strip_prefix("commands::")` — a parser bug that made this
/// file's own gate report `get_simulation_status` as unregistered when it has always been
/// registered. Matching the marker wherever it appears has no first-element special case to get
/// wrong, and does not care how the list is wrapped by rustfmt.
fn registered_commands(source: &str) -> BTreeSet<String> {
    const OPEN: &str = "generate_handler![";
    let Some(start) = source.find(OPEN) else {
        return BTreeSet::new();
    };
    let body = &source[start + OPEN.len()..];
    let Some(end) = body.find(']') else {
        return BTreeSet::new();
    };
    let list = &body[..end];

    list.match_indices("commands::")
        .map(|(at, marker)| {
            list[at + marker.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .collect()
}

fn commands_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands")
}

fn all_declared() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let entries = std::fs::read_dir(commands_dir()).expect("src/commands must be readable");
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("command source must be readable");
        out.extend(declared_commands(&source));
    }
    out
}

fn all_registered() -> BTreeSet<String> {
    let lib = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib).expect("src/lib.rs must be readable");
    registered_commands(&source)
}

#[test]
fn every_declared_command_is_registered_in_the_invoke_handler() {
    let declared = all_declared();
    let registered = all_registered();

    let missing: Vec<&String> = declared.difference(&registered).collect();
    assert!(
        missing.is_empty(),
        "these #[tauri::command]s are not in generate_handler![] in src/lib.rs, so `invoke` \
         rejects them with `Unknown command` no matter how complete the rest of the feature is: \
         {missing:?}"
    );
}

#[test]
fn every_registered_command_is_declared() {
    // The other direction. A stale entry naming a renamed or deleted command is a compile error
    // today, so this cannot currently fail — which is the point of asserting it rather than
    // assuming it: it stays true only while `generate_handler!` keeps resolving real paths.
    let declared = all_declared();
    let registered = all_registered();

    let unknown: Vec<&String> = registered.difference(&declared).collect();
    assert!(
        unknown.is_empty(),
        "generate_handler![] names commands that no #[tauri::command] declares: {unknown:?}"
    );
}

#[test]
fn the_scan_found_a_realistic_number_of_commands() {
    // Guards against the whole gate passing vacuously — two empty sets are equal. A count is
    // deliberately not asserted: it changes every time a command is added, and a gate that must be
    // edited to add a feature gets edited without being read.
    let declared = all_declared();
    assert!(
        declared.len() > 20,
        "found only {} commands; the scanner is probably broken rather than the tree empty",
        declared.len()
    );
    for expected in [
        "get_simulation_status",
        "get_lineage_graph",
        "save_simulation_state",
    ] {
        assert!(
            declared.contains(expected),
            "{expected} is a command this repository is documented to have, and the scan missed it"
        );
    }
}

#[test]
fn the_scanners_can_actually_fail() {
    // Negative control, in three parts, because each one is a way the gate above could be green
    // while proving nothing.

    // 1. A command absent from the handler list is detected.
    let source = "#[tauri::command]\npub fn lonely_command(x: u32) -> u32 { x }\n";
    let declared = declared_commands(source);
    assert_eq!(declared.len(), 1);
    assert!(declared.contains("lonely_command"));

    // The handler side is pinned by VALUE, not merely by "the difference is non-empty". The first
    // version of this control asserted only the latter, and it passed while `registered_commands`
    // was silently dropping the first entry of every list — a difference from an under-parsed set
    // is still non-empty, so the control agreed with a parser that could not parse. What exposed
    // it was the real gate reporting `get_simulation_status` as unregistered when it never was.
    let registered = registered_commands("generate_handler![commands::something_else]");
    assert_eq!(
        registered.iter().map(String::as_str).collect::<Vec<_>>(),
        vec!["something_else"],
        "the handler parser must read names, including the first one in the list"
    );
    let wrapped = registered_commands(
        "tauri::generate_handler![\n  commands::first,\n  commands::second\n])\n",
    );
    assert_eq!(
        wrapped.iter().map(String::as_str).collect::<Vec<_>>(),
        vec!["first", "second"],
        "a list wrapped across lines is the shape src/lib.rs actually has"
    );
    assert!(
        !declared
            .difference(&registered)
            .collect::<Vec<_>>()
            .is_empty(),
        "an unregistered command must show up as a difference"
    );

    // 2. A comment that merely *mentions* the attribute is not a declaration. This is the exact hit
    //    that made an ad-hoc count read 32 instead of 31.
    let commented = "// `#[tauri::command]` defaults to camelCase, so a parameter\n\
                     // named file_path arrives as filePath.\npub fn not_a_command() {}\n";
    assert!(
        declared_commands(commented).is_empty(),
        "a comment quoting the attribute was counted as a command"
    );

    // 3. Attributes between the attribute and the fn do not hide the name.
    let spaced = "#[tauri::command]\n#[allow(clippy::too_many_arguments)]\n\
                  /// docs\npub async fn spaced_command() {}\n";
    assert!(declared_commands(spaced).contains("spaced_command"));
}

#[test]
fn the_lineage_analysis_commands_are_reachable() {
    // OSS-070/071/072 named specifically. `to_newick` and `simplify` existed, were tested, and were
    // callable from no command at all — recorded as an open gap in STATE_OF_THE_PROJECT.md §3.15.1
    // until this package. Naming them here is what stops that from silently recurring.
    let registered = all_registered();
    for command in [
        "get_lineage_graph",
        "get_lineage_mrca",
        "export_lineage_newick",
        "get_simplified_lineage",
    ] {
        assert!(
            registered.contains(command),
            "{command} is not reachable from the webview"
        );
    }
}
