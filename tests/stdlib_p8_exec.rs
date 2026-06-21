//! Tests for @std/exec module (process execution)

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}-{}", std::process::id()))
}

/// Write `script` to `<dir>/main.gs`, run `gs <file>`, and return the process output.
fn run_script(dir: &PathBuf, script: &str) -> std::process::Output {
    let file = dir.join("main.gs");
    fs::write(&file, script).expect("write script");
    Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&file)
        .output()
        .expect("run gs")
}

#[test]
fn exec_run_captures_exit_code_and_output() {
    let dir = unique_temp_dir("exec_run");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const exec = require("@std/exec");
        const result = exec.run("echo", "hello");
        if (result.exitCode !== 0) {
            throw new Error("expected exitCode 0");
        }
        if (result.stdout.indexOf("hello") === -1) {
            throw new Error("expected stdout to contain hello");
        }
        if (!result.success) {
            throw new Error("expected success true");
        }
        print("test passed");
    "#;
    let out = run_script(&dir, script);
    fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "script failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("test passed"));
}

#[test]
fn exec_output_returns_stdout_as_string() {
    let dir = unique_temp_dir("exec_output");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const exec = require("@std/exec");
        const out = exec.output("echo", "test");
        if (out.indexOf("test") === -1) {
            throw new Error("expected output to contain test");
        }
        print("test passed");
    "#;
    let out = run_script(&dir, script);
    fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "script failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("test passed"));
}

#[test]
fn exec_run_with_array_args() {
    let dir = unique_temp_dir("exec_array");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const exec = require("@std/exec");
        const result = exec.run("echo", ["arg1", "arg2"]);
        if (result.exitCode !== 0) {
            throw new Error("expected exitCode 0");
        }
        if (!result.success) {
            throw new Error("expected success true");
        }
        print("test passed");
    "#;
    let out = run_script(&dir, script);
    fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "script failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("test passed"));
}

#[test]
fn exec_command_builder_pattern() {
    let dir = unique_temp_dir("exec_command");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const exec = require("@std/exec");
        const cmd = exec.command("echo", "builder");
        const result = cmd.run();
        if (result.exitCode !== 0) {
            throw new Error("expected exitCode 0");
        }
        if (result.stdout.indexOf("builder") === -1) {
            throw new Error("expected builder in output");
        }
        if (!result.success) {
            throw new Error("expected success true");
        }
        print("test passed");
    "#;
    let out = run_script(&dir, script);
    fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "script failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("test passed"));
}

#[test]
fn exec_combined_output_merges_stdout_stderr() {
    let dir = unique_temp_dir("exec_combined");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const exec = require("@std/exec");
        const out = exec.combinedOutput("echo", "combined");
        if (out.indexOf("combined") === -1) {
            throw new Error("expected combined in output");
        }
        print("test passed");
    "#;
    let out = run_script(&dir, script);
    fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "script failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("test passed"));
}

#[test]
fn exec_run_non_existent_command_returns_error() {
    let dir = unique_temp_dir("exec_error");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const exec = require("@std/exec");
        try {
            exec.run("nonexistent_command_12345");
            throw new Error("should have thrown");
        } catch (e) {
            if (e.message.indexOf("exec.run") === -1) {
                throw new Error("expected exec.run error");
            }
            print("test passed");
        }
    "#;
    let out = run_script(&dir, script);
    fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "script failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("test passed"));
}
