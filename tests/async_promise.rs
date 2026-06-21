//! Tests for Promise methods (then/catch/finally/all/race)

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
fn promise_then_chains_value() {
    let dir = unique_temp_dir("promise_then");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const p = Promise.resolve(10)
        p.then(function(val) {
            if (val !== 10) {
                throw new Error("Expected value 10")
            }
            print("test passed")
        })
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
fn promise_catch_handles_rejection() {
    let dir = unique_temp_dir("promise_catch");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const p = Promise.reject("error message")
        p.catch(function(err) {
            if (err !== "error message") {
                throw new Error("Expected error message")
            }
            print("test passed")
        })
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
fn promise_finally_always_runs() {
    let dir = unique_temp_dir("promise_finally");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        let finallyCalled = false
        const p = Promise.resolve(42)
        p.finally(function() {
            finallyCalled = true
        })
        if (!finallyCalled) {
            throw new Error("finally should have been called")
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
fn promise_all_waits_for_all() {
    let dir = unique_temp_dir("promise_all");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const p1 = Promise.resolve(1)
        const p2 = Promise.resolve(2)
        const p3 = Promise.resolve(3)
        const all = Promise.all([p1, p2, p3])
        if (typeof all !== "object") {
            throw new Error("Promise.all should return a Promise")
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
fn promise_race_returns_first() {
    let dir = unique_temp_dir("promise_race");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const p1 = Promise.resolve(1)
        const p2 = Promise.resolve(2)
        const race = Promise.race([p1, p2])
        if (typeof race !== "object") {
            throw new Error("Promise.race should return a Promise")
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
fn promise_all_empty_array() {
    let dir = unique_temp_dir("promise_all_empty");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const all = Promise.all([])
        if (typeof all !== "object") {
            throw new Error("Promise.all should return a Promise")
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
