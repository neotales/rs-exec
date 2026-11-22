//! Shared construction for the runtime modules.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::{DirBuilder, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use neotales_exec::{Command, find};

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

/// Writes a fresh private script file and leaves it on disk.
pub(crate) fn temp_file(script: &str, extension: &str) -> io::Result<PathBuf> {
    let directory = private_directory()?;
    let result = write_script_with(&directory, extension, |file| {
        file.write_all(script.as_bytes())
    });
    if result.is_err() {
        let _ = std::fs::remove_dir(&directory);
    }
    result
}

fn random_name() -> io::Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(io::Error::other)?;
    let mut name = String::with_capacity(32);
    for byte in bytes {
        write!(name, "{byte:02x}").expect("writing to a string cannot fail");
    }
    Ok(name)
}

fn private_directory() -> io::Result<PathBuf> {
    let base = std::env::temp_dir();
    #[cfg(windows)]
    refuse_reparse_points(&base)?;
    loop {
        let path = base.join(format!("neotales-script-{}", random_name()?));
        let mut builder = DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => {
                #[cfg(windows)]
                if let Err(error) = refuse_reparse_points(&path) {
                    let _ = std::fs::remove_dir(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

#[cfg(windows)]
fn refuse_reparse_points(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    const REPARSE_POINT: u32 = 0x0400;
    for ancestor in path.ancestors() {
        if std::fs::symlink_metadata(ancestor)?.file_attributes() & REPARSE_POINT != 0 {
            return Err(io::Error::other(
                "a temporary directory path contains a reparse point",
            ));
        }
    }
    Ok(())
}

fn write_script_with(
    directory: &Path,
    extension: &str,
    write_body: impl FnOnce(&mut std::fs::File) -> io::Result<()>,
) -> io::Result<PathBuf> {
    let path = directory.join(format!("run-{}.{extension}", random_name()?));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    let result = write_body(&mut file).and_then(|()| file.flush());
    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_failure_leaves_no_file_behind() {
        let directory = private_directory().expect("create directory");
        let result = write_script_with(&directory, "go", |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("the disk is full"))
        });
        assert!(result.is_err());
        assert_eq!(
            std::fs::read_dir(&directory)
                .expect("read directory")
                .count(),
            0
        );
        std::fs::remove_dir(&directory).expect("remove directory");
    }
}
