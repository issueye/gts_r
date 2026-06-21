//! Parity tests for the @std/mail module: address parsing/formatting,
//! message parsing, date helpers, and header lookup.

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

#[test]
fn mail_parse_bare_address() {
    let dir = unique_temp_dir("gts-p9-mail-bare");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let mail = require("@std/mail");
let a = mail.parseAddress("alice@example.com");
println(a.name);
println(a.address);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "");
    assert_eq!(lines[1], "alice@example.com");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn mail_parse_named_address() {
    let dir = unique_temp_dir("gts-p9-mail-named");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let mail = require("@std/mail");
let a = mail.parseAddress("Alice Doe <alice@example.com>");
println(a.name);
println(a.address);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "Alice Doe");
    assert_eq!(lines[1], "alice@example.com");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn mail_parse_invalid_returns_error() {
    let dir = unique_temp_dir("gts-p9-mail-bad");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let mail = require("@std/mail");
try {
    mail.parseAddress("not-an-address");
    println("ok");
} catch (e) {
    println("errored");
}
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "errored");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn mail_parse_address_list_and_format_roundtrip() {
    let dir = unique_temp_dir("gts-p9-mail-list");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let mail = require("@std/mail");
let list = mail.parseAddressList("Alice <alice@example.com>, bob@example.com");
println(list.length);
println(list[0].name);
println(list[1].address);
// Format the parsed list back into a single header value.
let formatted = mail.formatAddressList(list);
println(formatted);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "2");
    assert_eq!(lines[1], "Alice");
    assert_eq!(lines[2], "bob@example.com");
    assert_eq!(lines[3], "Alice <alice@example.com>, bob@example.com");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn mail_format_address_from_object() {
    let dir = unique_temp_dir("gts-p9-mail-fmt");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let mail = require("@std/mail");
println(mail.formatAddress({ name: "Bob", address: "bob@example.com" }));
println(mail.formatAddress("carol@example.com"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "Bob <bob@example.com>");
    assert_eq!(lines[1], "carol@example.com");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn mail_parse_message_splits_headers_and_body() {
    let dir = unique_temp_dir("gts-p9-mail-msg");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let mail = require("@std/mail");
let msg = mail.parseMessage("From: alice@example.com\nTo: bob@example.com\nSubject: Hello\n\nThis is the body.");
println(msg.headers.From[0]);
println(msg.headers.To[0]);
println(msg.headers.Subject[0]);
println(msg.body);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "alice@example.com");
    assert_eq!(lines[1], "bob@example.com");
    assert_eq!(lines[2], "Hello");
    assert_eq!(lines[3], "This is the body.");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn mail_get_header_case_insensitive() {
    let dir = unique_temp_dir("gts-p9-mail-hdr");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let mail = require("@std/mail");
let msg = mail.parseMessage("Subject: Hi\n\nbody");
println(mail.getHeader(msg.headers, "subject"));
println(mail.getHeader(msg.headers, "SUBJECT"));
println(mail.getHeader(msg.headers, "missing"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "Hi");
    assert_eq!(lines[1], "Hi");
    assert_eq!(lines[2], "undefined");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn mail_format_date_returns_rfc1123z_shape() {
    let dir = unique_temp_dir("gts-p9-mail-date");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let mail = require("@std/mail");
let s = mail.formatDate();
// RFC 1123Z shape: "Day, DD Mon YYYY HH:MM:SS +ZZZZ"
println(s.length > 25);
println(s.includes(","));
// parseDate should accept an RFC3339-style timestamp.
let d = mail.parseDate("2024-01-15T10:30:00Z");
println(d);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "true");
    // d renders as a date object placeholder; just assert non-empty.
    assert!(lines[2].len() > 0);
    let _ = fs::remove_dir_all(dir);
}
