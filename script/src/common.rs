//! Shared construction for the runtime modules.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::{DirBuilder, Metadata, OpenOptions};
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

/// Writes `script` into a directory the user controls and returns the file.
///
/// The directory is created exclusively with mode `0700`. An existing directory is accepted only
/// when it is a real directory, with no group or other access, owned by the current user, and no
/// parent lets another user replace it. The file is written under a temporary name and renamed
/// onto its content-addressed name, so no reader ever sees a partial file. The file is left
/// behind, matching the reference implementation.
pub(crate) fn temp_file(script: &str, extension: &str) -> io::Result<PathBuf> {
    let directory = private_directory()?;
    write_script(&directory, script, extension)
}

fn private_directory() -> io::Result<PathBuf> {
    if let Some(base) = user_cache_base() {
        std::fs::create_dir_all(&base)?;
        let directory = base.join("neotales-script");
        open_private_directory(&directory)?;
        return Ok(directory);
    }

    // No per-user location is known: a fresh directory under the system temporary directory, with
    // an unpredictable name. `create` refuses to reuse an entry that is already there.
    let directory = std::env::temp_dir().join(format!(
        "neotales-script-{}-{:016x}",
        std::process::id(),
        unique_token()
    ));
    create_exclusive_directory(&directory)?;
    verify_directory(&directory)?;
    verify_ancestry(&directory)?;
    Ok(directory)
}

/// The per-user cache directory.
///
/// On Windows this is `%LOCALAPPDATA%`, whose ACL already belongs to the user; tightening it
/// further would need `unsafe`, so a reparse point is refused instead of an ACL being written.
fn user_cache_base() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty()) {
            return Some(PathBuf::from(local));
        }
    }

    // `XDG_CACHE_HOME` is only used when it is absolute; a relative value would put the scripts
    // under the working directory.
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Some(cache);
    }

    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".cache"))
}

fn open_private_directory(directory: &Path) -> io::Result<()> {
    match create_exclusive_directory(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }

    verify_directory(directory)?;
    verify_ancestry(directory)
}

/// Creates one directory and never follows a path that already exists.
#[cfg(unix)]
fn create_exclusive_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = DirBuilder::new();
    builder.mode(0o700);
    builder.create(path)
}

/// Creates one directory and never follows a path that already exists.
#[cfg(not(unix))]
fn create_exclusive_directory(path: &Path) -> io::Result<()> {
    DirBuilder::new().create(path)
}

fn verify_directory(directory: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(directory)?;
    if !metadata.is_dir() || is_link(&metadata) {
        return Err(io::Error::other(
            "the private script directory is not a real directory",
        ));
    }

    #[cfg(unix)]
    verify_unix_directory(directory, &metadata)?;

    Ok(())
}

#[cfg(unix)]
fn verify_unix_directory(directory: &Path, metadata: &Metadata) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    if metadata.mode() & 0o777 != 0o700 {
        return Err(io::Error::other(
            "the private script directory does not have mode 0700",
        ));
    }

    // Mode and owner are separate facts, and another user can own a directory that is mode 0700.
    // A file created inside records the current user's id, which gives the id to compare with.
    let probe = directory.join(format!(".owner-{:016x}", unique_token()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&probe)?;
    let owner = file.metadata()?.uid();
    drop(file);
    let _ = std::fs::remove_file(&probe);

    if metadata.uid() != owner {
        return Err(io::Error::other(
            "the private script directory is not owned by the current user",
        ));
    }

    Ok(())
}

fn verify_ancestry(directory: &Path) -> io::Result<()> {
    let mut parent = directory.parent();
    while let Some(path) = parent {
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_dir() || is_link(&metadata) {
            return Err(io::Error::other(
                "a parent of the private script directory is not a real directory",
            ));
        }

        // A parent that another user can write lets them replace everything below it. The sticky
        // bit is the exception: it stops anyone but the entry's owner from renaming or deleting
        // the entry, which is why a system temporary directory is usable.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode();
            if mode & 0o022 != 0 && mode & 0o1000 == 0 {
                return Err(io::Error::other(
                    "a parent of the private script directory is writable by other users",
                ));
            }
        }

        parent = path.parent();
    }

    Ok(())
}

/// A symlink, or on Windows any reparse point, including a junction.
#[cfg(windows)]
fn is_link(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    /// `FILE_ATTRIBUTE_REPARSE_POINT`.
    const REPARSE_POINT: u32 = 0x0400;

    metadata.file_type().is_symlink() || metadata.file_attributes() & REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn write_script(directory: &Path, script: &str, extension: &str) -> io::Result<PathBuf> {
    write_script_with(directory, script, extension, |file| {
        file.write_all(script.as_bytes())
    })
}

fn write_script_with(
    directory: &Path,
    script: &str,
    extension: &str,
    write_body: impl FnOnce(&mut std::fs::File) -> io::Result<()>,
) -> io::Result<PathBuf> {
    let path = directory.join(file_name(script, extension));

    match std::fs::read(&path) {
        Ok(existing) if existing == script.as_bytes() => return Ok(path),
        Ok(_) => return Err(contents_mismatch()),
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        Err(_) => {}
    }

    let temporary = directory.join(format!(".run-{:016x}.{extension}.tmp", unique_token()));
    if let Err(error) = write_temporary(&temporary, write_body) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }

    match std::fs::rename(&temporary, &path) {
        Ok(()) => Ok(path),
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            // A second caller may have published the same bytes first, and Windows refuses to
            // replace an existing file. Identical contents are success, not tampering.
            match std::fs::read(&path) {
                Ok(existing) if existing == script.as_bytes() => Ok(path),
                Ok(_) => Err(contents_mismatch()),
                Err(_) => Err(error),
            }
        }
    }
}

fn contents_mismatch() -> io::Error {
    io::Error::other("a temporary script file already exists with different contents")
}

fn write_temporary(
    path: &Path,
    write_body: impl FnOnce(&mut std::fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(path)?;
    write_body(&mut file)?;
    file.flush()
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

    fn private_test_directory() -> PathBuf {
        let directory = test_directory();
        create_exclusive_directory(&directory).expect("create the test directory");
        directory
    }

    #[test]
    fn a_write_failure_leaves_no_file_behind() {
        let directory = private_test_directory();

        let result = write_script_with(&directory, "package main", "go", |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("the disk is full"))
        });

        let leftovers = fs::read_dir(&directory)
            .expect("read the test directory")
            .count();
        fs::remove_dir_all(&directory).expect("remove the test directory");

        assert!(result.is_err());
        assert_eq!(leftovers, 0, "a failed write must leave no file behind");
    }

    #[test]
    fn a_read_only_directory_reports_the_error() {
        let directory = private_test_directory();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o500))
            .expect("make the directory read-only");

        let result = write_script(&directory, "package main", "go");

        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("restore the directory mode");
        fs::remove_dir_all(&directory).expect("remove the test directory");

        assert!(result.is_err(), "a failed write must be an error");
    }

    #[test]
    fn an_existing_file_is_reused_only_when_the_contents_match() {
        let directory = private_test_directory();

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

    #[test]
    fn two_writers_publish_one_complete_file() {
        let directory = private_test_directory();
        let other = directory.clone();
        let writer = std::thread::spawn(move || write_script(&other, "shared", "go"));

        let first = write_script(&directory, "shared", "go").expect("first writer");
        let second = writer
            .join()
            .expect("join the writer")
            .expect("second writer");
        let published = fs::read(directory.join(file_name("shared", "go"))).expect("read the file");
        fs::remove_dir_all(&directory).expect("remove the test directory");

        assert_eq!(first, second);
        assert_eq!(published, b"shared");
    }

    #[test]
    fn a_symlinked_directory_is_refused() {
        let root = private_test_directory();
        let target = root.join("target");
        create_exclusive_directory(&target).expect("create the target directory");
        let link = root.join("link");
        std::os::unix::fs::symlink(&target, &link).expect("create the symlink");

        let result = open_private_directory(&link);

        fs::remove_dir_all(&root).expect("remove the test directory");

        assert!(result.is_err(), "a symlinked directory must be refused");
    }

    #[test]
    fn a_directory_with_other_access_is_refused() {
        let root = private_test_directory();
        let directory = root.join("scripts");
        create_exclusive_directory(&directory).expect("create the scripts directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))
            .expect("loosen the directory mode");

        let result = open_private_directory(&directory);

        fs::remove_dir_all(&root).expect("remove the test directory");

        assert!(
            result.is_err(),
            "a directory open to others must be refused"
        );
    }
}
