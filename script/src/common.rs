//! Shared construction for the runtime modules.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::{DirBuilder, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Writes `script` into a directory the user owns and returns the file.
///
/// The directory is created with mode `0700` and the file with mode `0600` and `create_new`. An
/// existing file is reused only when its contents match, so a file planted by another user is
/// never run. The file is left behind, matching the reference implementation.
pub(crate) fn temp_file(script: &str, extension: &str) -> io::Result<PathBuf> {
    let directory = private_directory()?;
    write_script(&directory, script, extension)
}

fn private_directory() -> io::Result<PathBuf> {
    if let Some(base) = user_cache_base() {
        let directory = base.join("neotales-script");
        create_private_directory(&directory)?;
        return Ok(directory);
    }

    // No user cache to build on: a fresh directory under the system temporary directory, which
    // `create_dir` refuses to reuse if anything already sits there.
    let directory = std::env::temp_dir().join(format!(
        "neotales-script-{}-{:016x}",
        std::process::id(),
        unique_token()
    ));
    create_private_directory(&directory)?;
    Ok(directory)
}

fn user_cache_base() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty()) {
            return Some(PathBuf::from(local));
        }
    }

    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(cache));
    }

    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".cache"))
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;

    // An earlier run may have created this directory with looser permissions, so make the mode
    // certain instead of trusting what is there.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}

fn write_script(directory: &Path, script: &str, extension: &str) -> io::Result<PathBuf> {
    let path = directory.join(file_name(script, extension));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(script.as_bytes())?;
            Ok(path)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if std::fs::read(&path)? == script.as_bytes() {
                Ok(path)
            } else {
                Err(io::Error::other(
                    "a temporary script file already exists with different contents",
                ))
            }
        }
        Err(error) => Err(error),
    }
}

fn file_name(script: &str, extension: &str) -> String {
    let digest = Sha256::digest(script.as_bytes());
    let mut hash = String::with_capacity(digest.len() * 2);
    for byte in &digest {
        let _ = write!(hash, "{byte:02x}");
    }
    format!("run-{hash}.{extension}")
}

fn unique_token() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
        });
    nanos ^ (u64::from(std::process::id()) << 32) ^ COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn test_directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "neotales-script-test-{}-{:016x}",
            std::process::id(),
            unique_token()
        ))
    }

    #[test]
    fn a_write_failure_is_returned() {
        let root = test_directory();
        let directory = root.join("scripts");
        create_private_directory(&directory).expect("create the script directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o500))
            .expect("make the directory read-only");

        let result = write_script(&directory, "package main", "go");

        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("restore the directory mode");
        fs::remove_dir_all(&root).expect("remove the test directory");

        assert!(result.is_err(), "a failed write must be an error");
    }

    #[test]
    fn an_existing_file_is_reused_only_when_the_contents_match() {
        let directory = test_directory();
        create_private_directory(&directory).expect("create the script directory");

        let first = write_script(&directory, "one", "go").expect("write the first file");
        let second = write_script(&directory, "one", "go").expect("reuse the first file");
        assert_eq!(first, second);

        let planted = directory.join(file_name("two", "go"));
        fs::write(&planted, "tampered").expect("plant a file");

        let result = write_script(&directory, "two", "go");

        fs::remove_dir_all(&directory).expect("remove the test directory");

        assert!(
            result.is_err(),
            "a file with different contents must not run"
        );
    }
}
