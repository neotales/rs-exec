//! The argument vectors each runtime builds.

use neotales_exec::{Command, which};
use neotales_script::{bash, bun, cmd, dotnet, golang, node, powershell, pwsh, sh};

fn args(command: &Command) -> Vec<String> {
    command
        .arguments()
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn bash_inline_puts_script_after_dash_c() {
    let command = bash::inline("echo hi", []);

    assert!(command.program().to_string_lossy().contains("bash"));
    assert_eq!(
        args(&command),
        [
            "--noprofile",
            "--norc",
            "-eo",
            "pipefail",
            "-c",
            "echo hi",
            "bash"
        ]
    );
}

#[test]
fn bash_script_treats_one_sh_line_as_a_file() {
    let command = bash::script("  script.sh  ", ["one"]);

    assert_eq!(command.arguments()[4], "script.sh");
    assert_eq!(args(&command)[5], "one");
}

#[test]
fn sh_inline_passes_dash_c() {
    let command = sh::inline("echo hi", []);

    assert_eq!(args(&command), ["-e", "-c", "echo hi", "sh"]);
}

#[test]
fn node_script_chooses_file_or_inline() {
    let file = node::script("app.js", ["--x"]);
    assert_eq!(args(&file), ["app.js", "--x"]);
    assert!(file.program().to_string_lossy().contains("node"));

    let inline = node::script("console.log(1)", []);
    assert_eq!(args(&inline), ["-e", "console.log(1)"]);
}

#[test]
fn bun_script_chooses_file_or_inline() {
    assert_eq!(args(&bun::script("app.ts", [])), ["app.ts"]);
    assert_eq!(
        args(&bun::script("console.log(1)", [])),
        ["-e", "console.log(1)"]
    );
}

#[test]
fn cmd_file_runs_through_dash_c() {
    let command = cmd::file("a.bat", ["1"]);

    assert_eq!(args(&command), ["/d", "/c", "a.bat", "1"]);
}

#[test]
fn golang_file_runs_the_file() {
    let command = golang::file("main.go", []);

    assert_eq!(args(&command), ["run", "main.go"]);
}

#[test]
fn golang_inline_writes_a_temporary_file() {
    let command = golang::inline("package main", []).expect("inline Go");

    let temp = command.temp_file().expect("temp file");
    assert!(temp.to_string_lossy().ends_with(".go"));
    assert_eq!(
        args(&command),
        vec!["run".to_owned(), temp.to_string_lossy().into_owned()]
    );
}

#[test]
fn inline_scripts_use_distinct_private_files() {
    let go = golang::inline("same source", []).expect("inline Go");
    let other = golang::script("same source", []).expect("Go script");
    let cs = dotnet::script("same source", []).expect(".NET script");
    let paths = [
        go.temp_file().unwrap(),
        other.temp_file().unwrap(),
        cs.temp_file().unwrap(),
    ];
    assert_ne!(paths[0], paths[1]);
    assert_ne!(paths[1].parent(), paths[0].parent());
    for path in paths {
        assert_eq!(std::fs::read(path).unwrap(), b"same source");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }
}

#[test]
fn dotnet_file_separates_program_arguments() {
    let command = dotnet::file("app.cs", ["a"]);

    assert_eq!(args(&command), ["run", "--file", "app.cs", "--", "a"]);
    assert_eq!(
        args(&dotnet::file("app.cs", [])),
        ["run", "--file", "app.cs"]
    );
    assert_eq!(args(&dotnet::file("lib.dll", ["a"])), ["lib.dll", "a"]);
    assert_eq!(
        args(&dotnet::file("app.csproj", [])),
        ["run", "--project", "app.csproj"]
    );
}

#[test]
fn powershell_and_pwsh_share_the_shape() {
    let expected = [
        "-NoLogo",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "a.ps1",
    ];
    assert_eq!(args(&powershell::file("a.ps1", [])), expected);
    assert_eq!(args(&pwsh::file("a.ps1", [])), expected);
    assert_eq!(
        args(&pwsh::script("Write-Output hi", [])),
        [
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Write-Output hi"
        ]
    );
}

#[test]
fn bash_inline_passes_the_first_argument_as_dollar_one() {
    if which("bash").is_none() && !std::path::Path::new("/bin/bash").exists() {
        return;
    }

    let output = bash::inline("echo \"$1\"", ["hello"])
        .output()
        .expect("bash");
    assert_eq!(output.text().trim(), "hello");
}

#[test]
fn sh_inline_passes_the_first_argument_as_dollar_one() {
    if which("sh").is_none() && !std::path::Path::new("/bin/sh").exists() {
        return;
    }

    let output = sh::inline("echo \"$1\"", ["hello"]).output().expect("sh");
    assert_eq!(output.text().trim(), "hello");
}

#[test]
fn bash_inline_executes_when_bash_exists() {
    if which("bash").is_none() && !std::path::Path::new("/bin/bash").exists() {
        return;
    }

    let output = bash::inline("echo hi", []).output().expect("bash");
    assert_eq!(output.text().trim(), "hi");
}

#[test]
fn sh_inline_executes_when_sh_exists() {
    if which("sh").is_none() && !std::path::Path::new("/bin/sh").exists() {
        return;
    }

    let output = sh::inline("echo hi", []).output().expect("sh");
    assert_eq!(output.text().trim(), "hi");
}
