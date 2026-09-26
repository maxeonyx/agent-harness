//! The read-only limb: two tools over one directory.
//!
//! Nothing here can write, so siblings sharing a filesystem cannot race —
//! a question the experiment deliberately leaves open.
//!
//! A failure here is in-band: the agent is told what went wrong and decides
//! what to do about it, which may be to report that it is blocked. That is a
//! completed agent, not a faulted one.

use std::path::{Path, PathBuf};

pub struct Limb {
    root: PathBuf,
}

impl Limb {
    pub fn new(dir: &Path) -> Result<Limb, String> {
        let root = dir
            .canonicalize()
            .map_err(|e| format!("--dir {}: {e}", dir.display()))?;
        if !root.is_dir() {
            return Err(format!("--dir {} is not a directory", root.display()));
        }
        Ok(Limb { root })
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let candidate = Path::new(path);
        if candidate.is_absolute() {
            return Err(format!(
                "path must be relative to the run directory; got {path}"
            ));
        }
        let joined = self.root.join(candidate);
        let resolved = joined
            .canonicalize()
            .map_err(|e| format!("{path}: {}", io_reason(&e)))?;
        if !resolved.starts_with(&self.root) {
            return Err(format!("{path} is outside the run directory"));
        }
        Ok(resolved)
    }

    pub fn list_dir(&self, path: &str) -> String {
        let resolved = match self.resolve(path) {
            Ok(resolved) => resolved,
            Err(error) => return format!("Error: {error}"),
        };
        let mut entries = match std::fs::read_dir(&resolved) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .map(|entry| {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if entry.path().is_dir() {
                        format!("{name}/")
                    } else {
                        name
                    }
                })
                .collect::<Vec<_>>(),
            Err(error) => return format!("Error: {path}: {}", io_reason(&error)),
        };
        entries.sort();
        if entries.is_empty() {
            format!("{path} is empty")
        } else {
            entries.join("\n")
        }
    }

    pub fn read_file(&self, path: &str) -> String {
        let resolved = match self.resolve(path) {
            Ok(resolved) => resolved,
            Err(error) => return format!("Error: {error}"),
        };
        match std::fs::read_to_string(&resolved) {
            Ok(text) => text,
            Err(error) => format!("Error: {path}: {}", io_reason(&error)),
        }
    }
}

fn io_reason(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => "no such file or directory".to_string(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => error.to_string(),
    }
}
