//! zsh commands.

use std::ffi::OsString;
use std::sync::Once;

use neotales_exec::{Command, Executable, register};

use crate::common;

const NAME: &str = "zsh";
const SCRIPT_ARGS: [&str; 4] = ["--no-rcs", "-e", "-o", "pipefail"];
const EXTENSIONS: [&str; 1] = [".zsh"];
const VARIABLE: &str = "RUN_ZSH_EXE";

static REGISTER: Once = Once::new();

fn ensure_registered() {
    REGISTER.call_once(|| {
        register(
            NAME,
            Executable {
                name: NAME.to_owned(),
                variable: VARIABLE.to_owned(),
                windows: vec![
                    "${ProgramFiles}\\Git\\usr\\bin\\zsh.exe".to_owned(),
                    "${ProgramFiles}\\Git\\bin\\zsh.exe".to_owned(),
                ],
                linux: vec![
                    "/bin/zsh".to_owned(),
                    "/usr/bin/zsh".to_owned(),
                    "/usr/local/bin/zsh".to_owned(),
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

/// Builds `zsh` with `args` and no script.
pub fn new<'a>(args: impl IntoIterator<Item = &'a str>) -> Command {
    build(args.into_iter().map(OsString::from).collect())
}

/// Builds `zsh --no-rcs -e -o pipefail <path> <args>`.
pub fn file<'a>(path: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let mut all: Vec<OsString> = SCRIPT_ARGS.iter().copied().map(OsString::from).collect();
    all.push(path.into());
    all.extend(args.into_iter().map(OsString::from));
    build(all)
}

/// Builds `zsh --no-rcs -e -o pipefail -c <script> zsh <args>`.
///
/// The caller's arguments follow a `$0` placeholder, so the first one reaches the script as `$1`.
pub fn inline<'a>(script: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    let mut all: Vec<OsString> = SCRIPT_ARGS.iter().copied().map(OsString::from).collect();
    all.push("-c".into());
    all.push(script.into());
    // `-c` makes the next word `$0`, so a placeholder keeps the first caller argument at `$1`.
    all.push(NAME.into());
    all.extend(args.into_iter().map(OsString::from));
    build(all)
}

/// Runs `text` as a file when it is one line ending in `.zsh`, and inline otherwise.
pub fn script<'a>(text: &str, args: impl IntoIterator<Item = &'a str>) -> Command {
    match common::script_file(text, &EXTENSIONS) {
        Some(path) => file(path, args),
        None => inline(text, args),
    }
}
