//! fish commands.

use std::ffi::OsString;
use std::sync::Once;

use neotales_exec::{Command, Executable, register};

use crate::common;

const NAME: &str = "fish";
const EXTENSIONS: [&str; 1] = [".fish"];
const VARIABLE: &str = "RUN_FISH_EXE";

static REGISTER: Once = Once::new();

fn ensure_registered() {
    REGISTER.call_once(|| {
        register(
            NAME,
            Executable {
                name: NAME.to_owned(),
                variable: VARIABLE.to_owned(),
                windows: vec!["${ProgramFiles}\\fish\\bin\\fish.exe".to_owned()],
                linux: vec!["/usr/bin/fish".to_owned(), "/usr/local/bin/fish".to_owned()],
                ..Executable::default()
            },
        );
    });
}

fn build(args: Vec<OsString>) -> Command {
    ensure_registered();
    common::command(NAME, args)
}

/// Builds `fish` with `args` and no script.
pub fn new<'a>(args: impl IntoIterator<Item = &'a str>) -> Command {
    build(args.into_iter().map(OsString::from).collect())
}

/// Builds `fish <path> <args>`.
pub fn file<'a>(path: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let mut all: Vec<OsString> = vec![path.into()];
    all.extend(args.into_iter().map(OsString::from));
    build(all)
}

/// Writes `script` to a private `.fish` file and builds `fish <file> <args>`.
///
/// `fish -c` does not forward the caller's arguments to `$argv`, so the source is materialised and
/// run as a file. The file is left in place, and the path is recorded on the command for
/// [`Command::temp_file`].
///
/// # Errors
///
/// Returns the I/O error when the private file cannot be written.
pub fn inline<'a>(
    script: &str,
    args: impl IntoIterator<Item = &'a str>,
) -> std::io::Result<Command> {
    let path = common::temp_file(script, "fish")?;
    let argument = path.to_string_lossy().into_owned();
    Ok(file(&argument, args).with_temp_file(path))
}

/// Runs `text` as a file when it is one line ending in `.fish`, and inline otherwise.
///
/// # Errors
///
/// Returns the I/O error from [`inline`] when the source has to be written.
pub fn script<'a>(text: &str, args: impl IntoIterator<Item = &'a str>) -> std::io::Result<Command> {
    match common::script_file(text, &EXTENSIONS) {
        Some(path) => Ok(file(path, args)),
        None => inline(text, args),
    }
}
