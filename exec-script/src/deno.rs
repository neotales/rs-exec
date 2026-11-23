//! Deno commands.

use std::ffi::OsString;
use std::sync::Once;

use neotales_exec::{Command, Executable, register};

use crate::common;

const NAME: &str = "deno";
const EXTENSIONS: [&str; 4] = [".js", ".mjs", ".cjs", ".ts"];
const VARIABLE: &str = "RUN_DENO_EXE";

static REGISTER: Once = Once::new();

fn ensure_registered() {
    REGISTER.call_once(|| {
        register(
            NAME,
            Executable {
                name: NAME.to_owned(),
                variable: VARIABLE.to_owned(),
                windows: vec![
                    "C:\\Program Files\\deno\\deno.exe".to_owned(),
                    "${USERPROFILE}\\.deno\\bin\\deno.exe".to_owned(),
                ],
                linux: vec![
                    "/usr/bin/deno".to_owned(),
                    "/usr/local/bin/deno".to_owned(),
                    "${HOME}/.deno/bin/deno".to_owned(),
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

/// Builds `deno` with `args` and no script.
pub fn new<'a>(args: impl IntoIterator<Item = &'a str>) -> Command {
    build(args.into_iter().map(OsString::from).collect())
}

/// Builds `deno run -A <path> <args>`.
///
/// `-A` is cast's allow-all flag, kept as the default the reference implementation uses.
pub fn file<'a>(path: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let mut all: Vec<OsString> = vec!["run".into(), "-A".into(), path.into()];
    all.extend(args.into_iter().map(OsString::from));
    build(all)
}

/// Builds `deno eval <script> <args>`.
pub fn inline<'a>(script: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let mut all: Vec<OsString> = vec!["eval".into(), script.into()];
    all.extend(args.into_iter().map(OsString::from));
    build(all)
}

/// Runs `text` as a file when it is one line ending in a JavaScript extension, and inline
/// otherwise.
pub fn script<'a>(text: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    match common::script_file(text, &EXTENSIONS) {
        Some(path) => file(path, args),
        None => inline(text, args),
    }
}
