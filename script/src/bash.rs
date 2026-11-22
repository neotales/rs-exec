//! Bash commands.

use std::ffi::OsString;
use std::sync::Once;

use neotales_exec::{Command, Executable, register};

use crate::common;

const NAME: &str = "bash";
const SCRIPT_ARGS: [&str; 4] = ["--noprofile", "--norc", "-eo", "pipefail"];

#[cfg(windows)]
const VARIABLE: &str = "RUN_WIN_BASH_EXE";
#[cfg(not(windows))]
const VARIABLE: &str = "RUN_BASH_EXE";

static REGISTER: Once = Once::new();

fn ensure_registered() {
    REGISTER.call_once(|| {
        register(
            NAME,
            Executable {
                name: NAME.to_owned(),
                variable: VARIABLE.to_owned(),
                windows: vec![
                    "${ProgramFiles}\\Git\\bin\\bash.exe".to_owned(),
                    "%ProgramFiles(x86)%\\Git\\bin\\bash.exe".to_owned(),
                    "${SystemRoot}\\System32\\bash.exe".to_owned(),
                ],
                linux: vec!["/bin/bash".to_owned(), "/usr/bin/bash".to_owned()],
                ..Executable::default()
            },
        );
    });
}

fn build(args: Vec<OsString>) -> Command {
    ensure_registered();
    common::command(NAME, args)
}

/// Builds `bash` with `args` and no script.
pub fn new<'a>(args: impl IntoIterator<Item = &'a str>) -> Command {
    build(args.into_iter().map(OsString::from).collect())
}

/// Builds `bash --noprofile --norc -eo pipefail <path> <args>`.
pub fn file<'a>(path: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let path = resolve_script_file(path);
    let mut all: Vec<OsString> = SCRIPT_ARGS.iter().copied().map(OsString::from).collect();
    all.push(path.into());
    all.extend(args.into_iter().map(OsString::from));
    build(all)
}

/// Builds `bash --noprofile --norc -eo pipefail -c <script> <args>`.
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

/// Rewrites an absolute path for a WSL bash, which sees drives under `/mnt`.
#[cfg(windows)]
fn resolve_script_file(path: &str) -> String {
    use std::path::Path;

    let absolute = if Path::new(path).is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path).to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_owned())
    };

    let bash = neotales_exec::find(NAME, None)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !bash.to_lowercase().contains("system32") {
        return absolute;
    }

    let rewritten = absolute.replace('\\', "/");
    let bytes = rewritten.as_bytes();
    if bytes.len() > 2 && bytes[1] == b':' {
        return format!("/mnt/{}{}", rewritten[..1].to_lowercase(), &rewritten[2..]);
    }

    rewritten
}

#[cfg(not(windows))]
fn resolve_script_file(path: &str) -> String {
    path.to_owned()
}
