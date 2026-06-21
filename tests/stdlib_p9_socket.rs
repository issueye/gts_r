//! Parity tests for @std/net/socket/client and @std/net/socket/server.
//!
//! The Rust VM is synchronous with no background event loop, so the server's
//! `acceptOne(handler)` runs inline: we create the listener, connect a client,
//! then call `acceptOne` to drain the pending connection and echo data back
//! within the same script.

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
fn socket_connect_refused_returns_error() {
    let dir = unique_temp_dir("gts-p9-sock-refused");
    fs::create_dir_all(&dir).expect("create temp dir");
    // Port 1 is privileged / unused on most systems → connection refused.
    let output = run_script(
        &dir,
        r#"
let sock = require("@std/net/socket/client");
try {
    sock.connect("127.0.0.1", 1);
    println("connected");
} catch (e) {
    println("errored");
}
"#,
    );
    // Either the connection is refused or it times out; both surface as errors.
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "errored");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn socket_server_listen_returns_bound_port() {
    let dir = unique_temp_dir("gts-p9-sock-listen");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let srv = require("@std/net/socket/server");
let server = srv.listen(0, function(conn) {});
println(server.port > 0);
println(server.address);
server.close();
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    // address is ":<port>".
    assert!(lines[1].starts_with(":"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn socket_echo_roundtrip_with_inline_accept() {
    let dir = unique_temp_dir("gts-p9-sock-echo");
    fs::create_dir_all(&dir).expect("create temp dir");
    // Workflow:
    //   1. listen on an ephemeral port
    //   2. connect a client from the same script
    //   3. acceptOne runs the handler which echoes whatever the client sent
    // Because acceptOne is synchronous, we send BEFORE accepting so the bytes
    // are buffered in the kernel by the time we read inside the handler.
    let output = run_script(
        &dir,
        r#"
let srv = require("@std/net/socket/server");
let client = require("@std/net/socket/client");
let server = srv.listen(0, function(conn) {
    let got = conn.read();
    conn.write(got);
    conn.close();
});
let port = server.port;
let conn = client.connect("127.0.0.1", port);
conn.write("ping");
server.acceptOne();
conn.setDeadline(2000);
let reply = conn.read();
println(reply);
conn.close();
server.close();
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "ping");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn socket_accept_one_without_pending_returns_wouldblock_error() {
    let dir = unique_temp_dir("gts-p9-sock-noblock");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let srv = require("@std/net/socket/server");
let server = srv.listen(0, function(conn) {});
try {
    server.acceptOne();
    println("accepted");
} catch (e) {
    println("errored");
}
server.close();
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    // No pending connection → WouldBlock surfaces as an error.
    assert_eq!(stdout_of(&output).trim(), "errored");
    let _ = fs::remove_dir_all(dir);
}
