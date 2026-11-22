//! POSIX shell commands.

use std::ffi::OsString;
use std::sync::Once;

use neotales_exec::{Command, Executable, register};

use crate::common;

const NAME: &str = "sh";
const SCRIPT_ARGS: [&str; 1] = ["-e"];

#[cfg(windows)]
const VARIABLE: &str = "RUN_WIN_SH_EXE";
#[cfg(not(windows))]
const VARIABLE: &str = "RUN_SH_EXE";

static REGISTER: Once = Once::new();

fn ensure_registered() {
    REGISTER.call_once(|| {
        register(
            NAME,
            Executable {
                name: NAME.to_owned(),
                variable: VARIABLE.to_owned(),
                windows: vec![
                    "${ProgramFiles}\\Git\\bin\\sh.exe".to_owned(),
                    "${ProgramFiles(x86)}\\Git\\bin\\sh.exe".to_owned(),
                ],
                linux: vec!["/bin/sh".to_owned(), "/usr/bin/sh".to_owned()],
                ..Executable::default()
            },
        );
    });
}

fn build(args: Vec<OsString>) -> Command {
    ensure_registered();
    common::command(NAME, args)
}

/// Builds `sh` with `args` and no script.
pub fn new<'a>(args: impl IntoIterator<Item = &'a str>) -> Command {
    build(args.into_iter().map(OsString::from).collect())
}

/// Builds `sh -e <path> <args>`.
pub fn file<'a>(path: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let mut all: Vec<OsString> = SCRIPT_ARGS.iter().copied().map(OsString::from).collect();
    all.push(path.into());
    all.extend(args.into_iter().map(OsString::from));
    build(all)
}

/// Builds `sh -e -c <script> <args>`.
pub fn inline<'a>(script: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let mut all: Vec<OsString> = SCRIPT_ARGS.iter().copied().map(OsString::from).collect();
    all.push("-c".into());
    all.push(script.into());
    all.extend(args.into_iter().map(OsString::from));
    build(all)
}

/// Runs `text` as a file when it is one line ending in `.sh`, and inline otherwise.
pub fn script<'a>(text: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    match common::script_file(text, &[".sh"]) {
        Some(path) => file(path, args),
        None => inline(text, args),
    }
}
