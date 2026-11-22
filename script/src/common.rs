//! Shared construction for the runtime modules.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;

use neotales_exec::{Command, find};
use sha2::{Digest, Sha256};

/// Resolves `name` through the registry, falling back to the bare name.
pub(crate) fn command(name: &str, args: Vec<OsString>) -> Command {
    let program = find(name, None).unwrap_or_else(|_| PathBuf::from(name));
    Command::new(program).args(args)
}

/// Applies the file-versus-inline rule: one line ending in a known extension is a file.
pub(crate) fn script_file<'a>(text: &'a str, extensions: &[&str]) -> Option<&'a str> {
    if text.contains('\n') || text.contains('\r') {
        return None;
    }

    let trimmed = text.trim();
    if extensions
        .iter()
        .any(|extension| trimmed.ends_with(extension))
    {
        Some(trimmed)
    } else {
        None
    }
}

/// Writes `script` to a content-addressed file in the temporary directory, once.
///
/// The file is left behind, matching the reference implementation. Nothing deletes it.
pub(crate) fn temp_file(script: &str, extension: &str) -> PathBuf {
    let digest = Sha256::digest(script.as_bytes());
    let mut hash = String::with_capacity(digest.len() * 2);
    for byte in &digest {
        let _ = write!(hash, "{byte:02x}");
    }

    let path = std::env::temp_dir().join(format!("run-{hash}.{extension}"));
    if !path.exists() {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o644);
        }
        if let Ok(mut file) = options.open(&path) {
            let _ = file.write_all(script.as_bytes());
        }
    }

    path
}
