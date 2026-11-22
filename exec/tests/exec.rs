//! Behaviour tests that run a real process on the machine.

#![cfg(unix)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use neotales_exec::{Command, CommandView, Output, clear_logger, set_logger, which};

fn text(output: &Output) -> String {
    output.text().trim().to_owned()
}

#[test]
fn echo_output_is_captured() {
    let output = Command::new("echo").arg("hello").output().expect("echo");

    assert_eq!(output.code, 0);
    assert!(output.is_ok());
    assert_eq!(text(&output), "hello");
    assert_eq!(output.stages.len(), 1);
    assert_eq!(output.program, "echo");
}

#[test]
fn parse_keeps_a_quoted_argument() {
    let command = Command::parse("echo 'hello world'").expect("parse");

    assert_eq!(command.program(), "echo");
    assert_eq!(command.arguments(), ["hello world"]);

    let output = command.output().expect("echo");
    assert_eq!(text(&output), "hello world");
}

#[test]
fn parse_of_empty_text_is_an_error() {
    assert!(Command::parse("").is_err());
    assert!(Command::new("").output().is_err());
}

#[test]
fn two_stage_pipeline_keeps_the_last_output() {
    if which("grep").is_none() {
        return;
    }

    let output = Command::new("echo")
        .arg("Hello World")
        .pipe(Command::new("grep").arg("Hello"))
        .output()
        .expect("pipeline");

    assert_eq!(output.stages.len(), 2);
    assert_eq!(output.code, 0);
    assert_eq!(output.text(), "Hello World\n");
    assert_eq!(output.stages[0].code, 0);
    assert_eq!(output.stages[0].stdout, [] as [u8; 0]);
}

#[test]
fn three_stage_pipeline_does_not_deadlock() {
    if which("tr").is_none() || which("cat").is_none() {
        return;
    }

    let output = Command::new("echo")
        .arg("hello")
        .pipe(Command::new("tr").args(["a-z", "A-Z"]))
        .pipe(Command::new("cat"))
        .output()
        .expect("pipeline");

    assert_eq!(output.stages.len(), 3);
    assert_eq!(output.text(), "HELLO\n");
}

#[test]
fn logging_hooks_fire_and_can_be_disabled() {
    let _guard = LOG_LOCK.lock().expect("log lock");
    let package_hook = Arc::new(AtomicUsize::new(0));
    let command_hook = Arc::new(AtomicUsize::new(0));

    // Other tests run in parallel and share the package hook, so the counter only counts a
    // command carrying this probe argument.
    let package_counter = Arc::clone(&package_hook);
    set_logger(move |view: &CommandView| {
        if view.program == "echo" && view.args == ["logging-hook-probe"] {
            package_counter.fetch_add(1, Ordering::SeqCst);
        }
    });

    let command_counter = Arc::clone(&command_hook);
    let output = Command::new("echo")
        .arg("logging-hook-probe")
        .logger(move |_: &CommandView| {
            command_counter.fetch_add(1, Ordering::SeqCst);
        })
        .output()
        .expect("echo");
    assert_eq!(text(&output), "logging-hook-probe");
    assert_eq!(command_hook.load(Ordering::SeqCst), 1);
    assert_eq!(package_hook.load(Ordering::SeqCst), 1);

    Command::new("echo")
        .arg("logging-hook-probe")
        .disable_logger()
        .output()
        .expect("echo");
    assert_eq!(command_hook.load(Ordering::SeqCst), 1);
    assert_eq!(package_hook.load(Ordering::SeqCst), 1);

    clear_logger();
}

#[test]
fn which_finds_an_absolute_executable() {
    assert!(which("echo").is_some());
    assert!(which("/bin/echo").is_some());
    assert!(which("there-is-no-such-tool-here").is_none());
}

#[test]
fn error_lines_reads_stderr() {
    if which("sh").is_none() {
        return;
    }

    let output = Command::new("sh")
        .args(["-c", "echo err >&2"])
        .output()
        .expect("sh");

    assert!(output.is_ok());
    assert_eq!(output.error_lines(), ["err"]);
    assert_eq!(output.lines(), [] as [String; 0]);
}

#[test]
fn a_non_zero_exit_is_data_until_to_error_is_called() {
    if which("sh").is_none() {
        return;
    }

    let output = Command::new("sh")
        .args(["-c", "exit 3"])
        .output()
        .expect("sh");

    assert_eq!(output.code, 3);
    assert!(!output.is_ok());
    let error = output.to_error().expect_err("non-zero exit");
    assert!(error.to_string().contains("code 3"));
}

static LOG_LOCK: Mutex<()> = Mutex::new(());
