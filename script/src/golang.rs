//! Go commands.
//!
//! The module is `golang` because `go` is a Rust keyword.

use std::ffi::OsString;
use std::sync::Once;

use neotales_exec::{Command, Executable, register};

use crate::common;

const NAME: &str = "go";
const EXTENSIONS: [&str; 1] = [".go"];

#[cfg(windows)]
const VARIABLE: &str = "RUN_WIN_GO_EXE";
#[cfg(not(windows))]
const VARIABLE: &str = "RUN_GO_EXE";

static REGISTER: Once = Once::new();

fn ensure_registered() {
    REGISTER.call_once(|| {
        register(
            NAME,
            Executable {
                name: NAME.to_owned(),
                variable: VARIABLE.to_owned(),
                windows: vec![
                    "${ProgramFiles}\\Go\\bin\\go.exe".to_owned(),
                    "${ChocolateyInstall}\\lib\\go\\tools\\go\\bin\\go.exe".to_owned(),
                ],
                linux: vec![
                    "${HOME}/.local/shared/go/bin/go".to_owned(),
                    "/usr/local/go/bin/go".to_owned(),
                    "/usr/local/bin/go".to_owned(),
                    "/usr/bin/go".to_owned(),
                ],
                ..Executable::default()
            },
        );
    });
}

fn build(args: Vec<OsString>) -> Command {
    ensure_registered();
    common::command(NAME, args)
}

/// Builds `go` with `args` and no script.
pub fn new<'a>(args: impl IntoIterator<Item = &'a str>) -> Command {
    build(args.into_iter().map(OsString::from).collect())
}

/// Builds `go run <path> <args>`.
pub fn file<'a>(path: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let mut all: Vec<OsString> = vec!["run".into(), path.into()];
    all.extend(args.into_iter().map(OsString::from));
    build(all)
}

/// Writes `script` to a content-addressed file in the temporary directory and builds
/// `go run <file> <args>`.
///
/// The file is left in the temporary directory, matching the reference implementation, and the
/// path is recorded on the command for [`Command::temp_file`].
pub fn inline<'a>(script: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let path = common::temp_file(script, "go");
    let mut all: Vec<OsString> = vec!["run".into(), path.clone().into_os_string()];
    all.extend(args.into_iter().map(OsString::from));
    build(all).with_temp_file(path)
}

/// Runs `text` as a file when it is one line ending in `.go`, and inline otherwise.
pub fn script<'a>(text: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    match common::script_file(text, &EXTENSIONS) {
        Some(path) => file(path, args),
        None => inline(text, args),
    }
}
