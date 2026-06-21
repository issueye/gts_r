//! Parity tests for @std/net/http/server.
//!
//! The synchronous VM has no background loop, so the server's `acceptOne`
//! blocks for a single request. A genuine request/response round-trip is
//! exercised by spawning the GTS server script in a child process (the server
//! accepts N requests then exits) and driving HTTP requests at it from the
//! test thread via the same tiny_http client surface.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}-{}", std::process::id()))
}

fn write_script(dir: &PathBuf, script: &str) -> PathBuf {
    let file = dir.join("main.gs");
    fs::write(&file, script).expect("write script");
    file
}

/// Spawn a GTS server script that binds an ephemeral port, prints
/// `GTS_PORT=<port>` to stdout, then accepts `count` requests before exiting.
fn spawn_server_script(dir: &PathBuf, script: &str) -> (std::process::Child, u16) {
    let file = write_script(dir, script);
    let mut child = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gs");

    // Read the first stdout line to discover the bound port.
    let mut stdout = child.stdout.take().expect("take stdout");
    let mut buf = [0u8; 256];
    let mut got = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        // Non-blocking-ish: read available bytes.
        match stdout.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                got.extend_from_slice(&buf[..n]);
                if let Some(nl) = got.iter().position(|&b| b == b'\n') {
                    let line = String::from_utf8_lossy(&got[..nl]).to_string();
                    if let Some(rest) = line.strip_prefix("GTS_PORT=") {
                        let port: u16 = rest.trim().parse().expect("parse port");
                        // Reattach stdout so the child keeps draining.
                        // (We can't reattach; instead just keep the handle alive.)
                        std::mem::forget(stdout);
                        return (child, port);
                    }
                }
            }
            Err(_) => break,
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    panic!(
        "server script did not print GTS_PORT; got: {:?}",
        String::from_utf8_lossy(&got)
    );
}

/// Issue a raw HTTP/1.1 request and read the full response. Returns
/// (status_line, headers, body).
fn http_round_trip(host: &str, port: u16, request: &str) -> (String, String, String) {
    let mut stream = TcpStream::connect((host, port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(request.as_bytes()).expect("write request");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read response");
    let text = String::from_utf8_lossy(&buf).to_string();
    let mut parts = text.splitn(2, "\r\n\r\n");
    let head = parts.next().unwrap_or("").to_string();
    let body = parts.next().unwrap_or("").to_string();
    let status = head.lines().next().unwrap_or("").to_string();
    (status, head, body)
}

#[test]
fn http_server_get_with_text_response() {
    let dir = unique_temp_dir("gts-p9-http-get");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let server = require("@std/net/http/server").createServer(function(req, res) {
    res.send("hello " + req.method);
});
println(`GTS_PORT=${server.port}`);
server.acceptOne();
server.close();
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    let (status, _head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(status.contains("200"), "status was: {}", status);
    assert_eq!(body, "hello GET");
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn http_server_json_response_and_custom_status() {
    let dir = unique_temp_dir("gts-p9-http-json");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let server = require("@std/net/http/server").createServer(function(req, res) {
    res.status(201);
    res.json({ name: "alice", age: 30 });
});
println(`GTS_PORT=${server.port}`);
server.acceptOne();
server.close();
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    let (status, head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET /users HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(status.contains("201"), "status was: {}", status);
    assert!(head
        .to_ascii_lowercase()
        .contains("content-type: application/json"));
    assert_eq!(body, "{\"name\": \"alice\", \"age\": 30}");
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn http_server_query_and_body_parsing() {
    let dir = unique_temp_dir("gts-p9-http-query");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let server = require("@std/net/http/server").createServer(function(req, res) {
    let q = req.query.name;
    let echoed = req.body;
    res.send("name=" + q + " body=" + echoed);
});
println(`GTS_PORT=${server.port}`);
server.acceptOne();
server.close();
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    let body_payload = "payload-data";
    let request = format!(
        "POST /echo?name=bob HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body_payload.len(),
        body_payload
    );
    let (status, _head, body) = http_round_trip("127.0.0.1", port, &request);
    assert!(status.contains("200"), "status was: {}", status);
    assert_eq!(body, "name=bob body=payload-data");
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn http_server_custom_header() {
    let dir = unique_temp_dir("gts-p9-http-hdr");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let server = require("@std/net/http/server").createServer(function(req, res) {
    res.setHeader("X-Custom", "abc");
    res.send("ok");
});
println(`GTS_PORT=${server.port}`);
server.acceptOne();
server.close();
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    let (_status, head, _body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(head.to_ascii_lowercase().contains("x-custom: abc"));
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}
