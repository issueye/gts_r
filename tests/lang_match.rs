//! Tests for match expression functionality

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

fn run_script(dir: &PathBuf, script: &str) -> std::process::Output {
    let file = dir.join("main.gs");
    fs::write(&file, script).expect("write script");
    Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&file)
        .output()
        .expect("run gs")
}

#[test]
fn match_basic_value_matching() {
    let dir = unique_temp_dir("match_basic");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const x = 2
        const result = match x {
            1 => "one",
            2 => "two",
            3 => "three",
            _ => "other"
        }
        if (result !== "two") {
            throw new Error("Expected two")
        }
        print("test passed")
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
fn match_or_patterns() {
    let dir = unique_temp_dir("match_or");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const day = 6
        const isWeekend = match day {
            6 | 7 => true,
            _ => false
        }
        if (!isWeekend) {
            throw new Error("Expected weekend")
        }
        print("test passed")
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
fn match_wildcard_default() {
    let dir = unique_temp_dir("match_wildcard");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const x = 999
        const result = match x {
            1 => "one",
            2 => "two",
            _ => "default"
        }
        if (result !== "default") {
            throw new Error("Expected default")
        }
        print("test passed")
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
fn match_string_patterns() {
    let dir = unique_temp_dir("match_string");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const cmd = "start"
        const action = match cmd {
            "start" => "starting",
            "stop" => "stopping",
            "pause" => "pausing",
            _ => "unknown"
        }
        if (action !== "starting") {
            throw new Error("Expected starting")
        }
        print("test passed")
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
fn match_variable_binding() {
    let dir = unique_temp_dir("match_binding");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const x = 42
        const result = match x {
            0 => "zero",
            n => n * 2
        }
        if (result !== 84) {
            throw new Error("Expected 84")
        }
        print("test passed")
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
fn match_in_return_statement() {
    let dir = unique_temp_dir("match_return");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        function getDayName(day) {
            return match day {
                1 => "Monday",
                2 => "Tuesday",
                3 => "Wednesday",
                _ => "Other"
            }
        }
        const name = getDayName(2)
        if (name !== "Tuesday") {
            throw new Error("Expected Tuesday")
        }
        print("test passed")
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
