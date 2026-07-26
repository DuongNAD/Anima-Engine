//! Where a save is allowed to be written, and what a frontend is allowed to name.
//!
//! # The hole this closes
//!
//! `save_simulation_state` and `load_simulation_state` took a `file_path: String` straight from the
//! frontend and handed it to `snapshot::write_atomic` / `snapshot::read`. Any string the webview
//! could produce was a path the backend would use — so anything that could reach `invoke` could
//! write a file anywhere the process has permission (`..\..\Windows\System32\...`, a startup
//! folder, another app's config) and read any file back into the running world.
//!
//! That is a privilege the save feature never needed. A save is *a named world belonging to this
//! app*, and naming one does not require addressing the filesystem.
//!
//! # The contract
//!
//! A save name is a **single file name** under the app's own data directory. Not a path: no
//! separators, no drive letters, no `..`, no absolute anchors. The extension is normalised to
//! `.json` so a caller cannot pick `.exe`, `.dll`, `.bat` or `.lnk` and drop an executable artefact
//! into a directory something else scans.
//!
//! Rejection is by allow-list on the characters, not a block-list of known-bad sequences. Block-lists
//! lose to encoding: `..%2f`, `..\\`, a UNC prefix, an NTFS alternate data stream (`save.json:evil`),
//! a trailing dot or space that Windows silently strips. Permitting only
//! `[A-Za-z0-9._-]` sidesteps the entire class rather than enumerating it.

use std::path::{Path, PathBuf};

/// Why a save name was refused. Kept as a type so the messages stay identical across call sites and
/// so tests assert on a discriminant rather than on prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveNameError {
    Empty,
    TooLong(usize),
    /// Contains something outside `[A-Za-z0-9._-]` — separators, drive letters, `:` streams,
    /// wildcards, control characters, non-ASCII.
    IllegalCharacter(char),
    /// `.`, `..`, or any name that is entirely dots.
    Traversal,
    /// A Windows reserved device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`..`COM9`, `LPT1`..`LPT9`),
    /// with or without an extension. Opening one of these does not touch the filesystem at all.
    ReservedDeviceName(String),
}

impl std::fmt::Display for SaveNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "save name is empty"),
            Self::TooLong(n) => write!(f, "save name is {n} characters; the maximum is {MAX_LEN}"),
            Self::IllegalCharacter(c) => write!(
                f,
                "save name contains {c:?}; only letters, digits, '.', '_' and '-' are allowed. \
                 A save is a name, not a path — it is always stored in this app's data directory."
            ),
            Self::Traversal => write!(f, "save name may not be '.', '..' or only dots"),
            Self::ReservedDeviceName(n) => {
                write!(
                    f,
                    "'{n}' is a reserved device name on Windows and cannot be a file"
                )
            }
        }
    }
}

/// Generous enough for any human-chosen name, short enough to stay well inside `MAX_PATH` once the
/// app-data prefix is added.
pub const MAX_LEN: usize = 100;

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validate a frontend-supplied save name and return the file name to use (always `*.json`).
///
/// Deliberately does no filesystem access, so it is cheap to test exhaustively.
pub fn sanitize_save_name(raw: &str) -> Result<String, SaveNameError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(SaveNameError::Empty);
    }
    if name.len() > MAX_LEN {
        return Err(SaveNameError::TooLong(name.len()));
    }
    // Allow-list. Everything a traversal or a stream needs — `/ \ : .. %` — is outside it.
    if let Some(c) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-'))
    {
        return Err(SaveNameError::IllegalCharacter(c));
    }
    // `.`, `..`, `...` — all dots is never a file, and `..` is the traversal itself.
    if name.chars().all(|c| c == '.') {
        return Err(SaveNameError::Traversal);
    }

    let stem = name.split('.').next().unwrap_or(name);
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Err(SaveNameError::ReservedDeviceName(stem.to_string()));
    }

    // Normalise the extension rather than trusting whatever was supplied.
    let base = name.strip_suffix(".json").unwrap_or(name);
    // Stripping `.json` from `"...json"`-style input can leave nothing behind.
    if base.is_empty() || base.chars().all(|c| c == '.') {
        return Err(SaveNameError::Traversal);
    }
    Ok(format!("{base}.json"))
}

/// Resolve a validated save name to an absolute path inside `saves_dir`.
///
/// The containment assertion is belt-and-braces: `sanitize_save_name` already makes escape
/// impossible, and this catches it anyway if that function is ever loosened. A security check worth
/// having is worth having twice when the second copy is one comparison.
pub fn resolve_save_path(saves_dir: &Path, raw_name: &str) -> Result<PathBuf, String> {
    let name = sanitize_save_name(raw_name).map_err(|e| e.to_string())?;
    let full = saves_dir.join(&name);

    if full.parent() != Some(saves_dir) {
        return Err(format!(
            "refusing to resolve save {raw_name:?} outside the app save directory"
        ));
    }
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        PathBuf::from(if cfg!(windows) {
            r"C:\Users\test\AppData\Roaming\com.anima.engine\saves"
        } else {
            "/home/test/.local/share/com.anima.engine/saves"
        })
    }

    #[test]
    fn a_plain_name_gets_a_json_extension_exactly_once() {
        assert_eq!(sanitize_save_name("world1").unwrap(), "world1.json");
        assert_eq!(sanitize_save_name("world1.json").unwrap(), "world1.json");
        // The shape the existing frontend already sends.
        assert_eq!(
            sanitize_save_name("save_test.json").unwrap(),
            "save_test.json"
        );
        assert_eq!(sanitize_save_name("  spaced  ").unwrap(), "spaced.json");
    }

    #[test]
    fn traversal_is_refused_in_every_encoding_that_reaches_this_function() {
        // The allow-list means each of these fails on a character, not on a pattern match, so a
        // variant nobody thought of fails too. That is the property being tested, not the list.
        for evil in [
            "../evil",
            "..\\evil",
            "../../Windows/System32/drivers/etc/hosts",
            "..%2fevil",
            "/etc/passwd",
            "\\\\server\\share\\evil",
            "C:\\Windows\\System32\\evil",
            "C:/Windows/evil",
            "save.json:stream",
            "sub/dir/save",
            "~/evil",
            "save\0.json",
            "save*.json",
            "save?.json",
            "save|evil",
        ] {
            assert!(
                sanitize_save_name(evil).is_err(),
                "{evil:?} should have been refused"
            );
            assert!(
                resolve_save_path(&dir(), evil).is_err(),
                "{evil:?} resolved"
            );
        }
    }

    #[test]
    fn dots_alone_are_never_a_save() {
        for evil in [".", "..", "...", ".json", "..json"] {
            assert!(
                sanitize_save_name(evil).is_err(),
                "{evil:?} should have been refused"
            );
        }
    }

    #[test]
    fn windows_device_names_are_refused_with_or_without_an_extension() {
        // Opening `CON` or `NUL` does not create a file at all; on Windows it addresses a device.
        for evil in ["CON", "con", "NUL.json", "com1", "LPT9.json", "AuX"] {
            assert!(
                matches!(
                    sanitize_save_name(evil),
                    Err(SaveNameError::ReservedDeviceName(_))
                ),
                "{evil:?} should have been refused as a device name"
            );
        }
        // A name that merely starts with those letters is fine.
        assert!(sanitize_save_name("console-world").is_ok());
        assert!(sanitize_save_name("nullify").is_ok());
    }

    #[test]
    fn empty_and_overlong_names_are_refused() {
        assert_eq!(sanitize_save_name(""), Err(SaveNameError::Empty));
        assert_eq!(sanitize_save_name("   "), Err(SaveNameError::Empty));
        let long = "a".repeat(MAX_LEN + 1);
        assert!(matches!(
            sanitize_save_name(&long),
            Err(SaveNameError::TooLong(_))
        ));
        assert!(sanitize_save_name(&"a".repeat(MAX_LEN)).is_ok());
    }

    #[test]
    fn an_accepted_name_always_lands_directly_in_the_save_directory() {
        // The containment property, asserted over everything the allow-list permits rather than a
        // couple of examples.
        for ok in ["world1", "a", "my-save_2", "World.1.json", "9"] {
            let p = resolve_save_path(&dir(), ok).expect("should be accepted");
            assert_eq!(p.parent(), Some(dir().as_path()), "{ok:?} escaped");
            assert!(
                p.extension().is_some_and(|e| e == "json"),
                "{ok:?} not json"
            );
        }
    }
}
