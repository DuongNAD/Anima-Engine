use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SOURCE_DIRECTORIES: &[&str] = &["src", "crates"];
const SOURCE_FILES: &[&str] = &["Cargo.toml", "Cargo.lock", "build.rs"];

fn main() {
    println!("cargo:rerun-if-env-changed=ANIMA_BUILD_ID");

    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    let package_version = env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");
    let build_id = match explicit_build_suffix() {
        Some(suffix) => format!("{package_version}+{suffix}"),
        None => {
            track_git_revision(&manifest_dir);
            let revision = git_stdout(&manifest_dir, &["rev-parse", "--verify", "HEAD"])
                .unwrap_or_else(|| {
                    println!(
                        "cargo:warning=Git revision unavailable; ANIMA_BUILD_ID uses 'unknown'"
                    );
                    "unknown".to_owned()
                });
            let source_fingerprint = source_fingerprint(&manifest_dir);
            let compilation_fingerprint = compilation_fingerprint();
            format!(
                "{package_version}+{revision}.src-{}.cfg-{}",
                &source_fingerprint[..16],
                &compilation_fingerprint[..16]
            )
        }
    };

    println!("cargo:rustc-env=ANIMA_BUILD_ID={build_id}");
    tauri_build::build()
}

fn explicit_build_suffix() -> Option<String> {
    let raw = env::var("ANIMA_BUILD_ID").ok()?;
    let suffix = raw.trim();
    if suffix.is_empty() {
        return None;
    }
    assert!(
        suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')),
        "ANIMA_BUILD_ID may contain only ASCII letters, digits, '.', '_' and '-'"
    );
    Some(suffix.to_owned())
}

fn source_fingerprint(manifest_dir: &Path) -> String {
    let mut paths = Vec::new();
    for relative in SOURCE_FILES {
        paths.push(manifest_dir.join(relative));
    }
    for relative in SOURCE_DIRECTORIES {
        let directory = manifest_dir.join(relative);
        println!("cargo:rerun-if-changed={}", directory.display());
        collect_files(&directory, &mut paths);
    }
    paths.sort();

    let mut digest = Sha256::new();
    for path in paths {
        println!("cargo:rerun-if-changed={}", path.display());
        let relative = path
            .strip_prefix(manifest_dir)
            .expect("source path remains inside CARGO_MANIFEST_DIR");
        let normalized = relative.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(&path)
            .unwrap_or_else(|error| panic!("failed to fingerprint {}: {error}", path.display()));
        digest.update((normalized.len() as u64).to_le_bytes());
        digest.update(normalized.as_bytes());
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    format!("{:x}", digest.finalize())
}

fn compilation_fingerprint() -> String {
    let mut digest = Sha256::new();
    let mut variables: Vec<_> = env::vars()
        .filter(|(name, _)| {
            matches!(
                name.as_str(),
                "PROFILE"
                    | "OPT_LEVEL"
                    | "DEBUG"
                    | "HOST"
                    | "TARGET"
                    | "RUSTC"
                    | "RUSTC_WRAPPER"
                    | "RUSTC_WORKSPACE_WRAPPER"
                    | "CARGO_ENCODED_RUSTFLAGS"
            ) || name.starts_with("CARGO_CFG_")
                || name.starts_with("CARGO_FEATURE_")
        })
        .collect();
    variables.sort();
    for (name, value) in variables {
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }

    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let version = Command::new(rustc)
        .arg("-vV")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| output.stdout)
        .unwrap_or_else(|| b"rustc-version-unknown".to_vec());
    digest.update((version.len() as u64).to_le_bytes());
    digest.update(version);
    format!("{:x}", digest.finalize())
}

fn collect_files(directory: &Path, paths: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to scan {}: {error}", directory.display()));
    for entry in entries {
        let entry = entry
            .unwrap_or_else(|error| panic!("failed to inspect {}: {error}", directory.display()));
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!("failed to inspect {}: {error}", entry.path().display())
        });
        if file_type.is_dir() {
            collect_files(&entry.path(), paths);
        } else if file_type.is_file() {
            paths.push(entry.path());
        }
    }
}

fn track_git_revision(manifest_dir: &Path) {
    if let Some(head) = git_path(manifest_dir, "HEAD") {
        println!("cargo:rerun-if-changed={}", head.display());
    }
    if let Some(reference) = git_stdout(manifest_dir, &["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git_path(manifest_dir, &reference) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn git_path(manifest_dir: &Path, name: &str) -> Option<PathBuf> {
    let raw = git_stdout(manifest_dir, &["rev-parse", "--git-path", name])?;
    let path = PathBuf::from(raw);
    Some(if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    })
}

fn git_stdout(manifest_dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}
