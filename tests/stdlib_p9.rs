//! Parity tests for the P9 stdlib modules: rate-limit, prometheus, highlight,
//! and sse. These modules are deterministic / in-process so they fit the same
//! "write a .gs file, run `gs`, assert stdout" pattern used by the other stdlib
//! integration tests.

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

fn stdout_of(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_of(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

// --- rate-limit -----------------------------------------------------------

#[test]
fn rate_limit_burst_within_capacity() {
    let dir = unique_temp_dir("gts-p9-rate");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let rl = require("@std/rate-limit");
let limiter = rl.create({ rate: 100, capacity: 3 });
let allowed = 0;
for (let i = 0; i < 5; i = i + 1) {
    if (limiter.tryAcquire()) {
        allowed = allowed + 1;
    }
}
println(allowed);
println(limiter.remaining());
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    // With capacity 3 and an immediate burst, the first 3 should be allowed.
    assert_eq!(lines[0], "3");
    // After 3 acquisitions out of capacity 3, ~0 remaining (token count is a float).
    let remaining: f64 = lines[1].parse().expect("numeric remaining");
    assert!(
        remaining < 0.5,
        "expected near-zero remaining, got {}",
        remaining
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rate_limit_remaining_reflects_set_capacity() {
    let dir = unique_temp_dir("gts-p9-rate2");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let rl = require("@std/rate-limit");
let limiter = rl.create({ capacity: 5 });
println(limiter.remaining());
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    // A fresh limiter should be at full capacity.
    assert_eq!(stdout.trim(), "5");
    let _ = fs::remove_dir_all(dir);
}

// --- prometheus -----------------------------------------------------------

#[test]
fn prometheus_inc_set_get_snapshot() {
    let dir = unique_temp_dir("gts-p9-prom");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let prom = require("@std/prometheus");
let m = prom.create();
m.inc("hits");
m.inc("hits");
m.inc("hits");
m.set("temp", 42);
println(m.get("hits"));
println(m.get("temp"));
println(m.get("missing"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "3");
    assert_eq!(lines[1], "42");
    assert_eq!(lines[2], "0"); // unknown metric defaults to 0
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn prometheus_snapshot_lists_metrics() {
    let dir = unique_temp_dir("gts-p9-prom2");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let prom = require("@std/prometheus");
let m = prom.create();
m.inc("a");
m.inc("b");
let snap = m.snapshot();
println(snap.length);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    assert_eq!(stdout.trim(), "2");
    let _ = fs::remove_dir_all(dir);
}

// --- highlight ------------------------------------------------------------

#[test]
fn highlight_terminal_diff_coloring() {
    let dir = unique_temp_dir("gts-p9-hi");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let h = require("@std/highlight");
let r = h.terminal("+added\n-removed\n@@ hunk @@", { lang: "diff" });
// When color is on (default), added/removed lines gain ANSI escapes.
println(r.lines[0].includes("\x1b[32m"));
println(r.lines[1].includes("\x1b[31m"));
// No color path returns plain text.
let plain = h.terminal("+added", { lang: "diff", color: false });
println(plain.text);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    // Colored +added line uses green (32), -removed uses red (31).
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "true");
    assert_eq!(lines[2], "+added");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn highlight_terminal_json_wraps_strings_in_color() {
    let dir = unique_temp_dir("gts-p9-hi2");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let h = require("@std/highlight");
let r = h.terminal('{"k": "v"}', { lang: "json" });
// Strings in the JSON line should carry ANSI color codes.
println(r.text.includes("\x1b["));
println(r.lang);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "json");
    let _ = fs::remove_dir_all(dir);
}

// --- sse ------------------------------------------------------------------

#[test]
fn sse_parse_single_event() {
    let dir = unique_temp_dir("gts-p9-sse");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let sse = require("@std/sse");
let events = sse.parse("data: hello world\n\n");
println(events.length);
println(events[0].data);
println(events[0].type);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "hello world");
    assert_eq!(lines[2], "message");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sse_parse_multi_field_and_event_type() {
    let dir = unique_temp_dir("gts-p9-sse2");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let sse = require("@std/sse");
let events = sse.parse("event: update\ndata: line1\ndata: line2\nid: 7\n\n");
println(events.length);
println(events[0].type);
// Multiple data: lines are joined with newlines, mirroring the spec.
println(events[0].data === "line1\nline2");
println(events[0].id);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "update");
    // data fields joined with "\n"
    assert_eq!(lines[2], "true");
    assert_eq!(lines[3], "7");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sse_reader_iterates_events() {
    let dir = unique_temp_dir("gts-p9-sse3");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let sse = require("@std/sse");
let reader = sse.reader("data: a\n\ndata: b\n\ndata: c\n\n");
let collected = "";
let ev = reader.next();
while (ev !== null) {
    collected = collected + ev.data;
    ev = reader.next();
}
println(collected);
let rest = reader.readAll();
println(rest.length);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "abc");
    // After consuming all events, readAll returns an empty array.
    assert_eq!(lines[1], "0");
    let _ = fs::remove_dir_all(dir);
}
