//! Tests for @std/net/http/client module

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

fn spawn_mock_http_server(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("mock server addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut buf = [0; 2048];
        let _ = stream.read(&mut buf);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nX-Test: async\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });
    format!("http://{addr}/async")
}

fn read_http_head(stream: &mut TcpStream) {
    let mut head = Vec::new();
    let mut byte = [0; 1];
    while !head.ends_with(b"\r\n\r\n") {
        let n = stream.read(&mut byte).expect("read request head");
        assert!(n != 0, "client closed before sending a complete request");
        head.push(byte[0]);
    }
}

fn spawn_keepalive_http_server(expected: usize) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind keepalive mock server");
    let addr = listener.local_addr().expect("mock server addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept pooled request");
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("set read timeout");
        for i in 0..expected {
            read_http_head(&mut stream);
            let body = format!("pooled-{i}");
            let connection = if i + 1 == expected {
                "close"
            } else {
                "keep-alive"
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: {}\r\n\r\n{}",
                body.len(),
                connection,
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write keepalive response");
        }
    });
    (format!("http://{addr}/pooled"), handle)
}

#[test]
fn http_client_get_returns_response() {
    let dir = unique_temp_dir("http_get");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const http = require("@std/net/http/client");
        const resp = http.get("https://httpbin.org/get");
        if (typeof resp.status !== "number") {
            throw new Error("expected status to be a number");
        }
        if (typeof resp.body !== "string") {
            throw new Error("expected body to be a string");
        }
        if (typeof resp.ok !== "boolean") {
            throw new Error("expected ok to be a boolean");
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
fn http_client_post_sends_data() {
    let dir = unique_temp_dir("http_post");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const http = require("@std/net/http/client");
        const resp = http.post("https://httpbin.org/post", "test data");
        if (resp.status !== 200) {
            throw new Error("expected status 200");
        }
        if (!resp.ok) {
            throw new Error("expected ok to be true");
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
fn http_client_request_with_options() {
    let dir = unique_temp_dir("http_request");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const http = require("@std/net/http/client");
        const resp = http.request({
            url: "https://httpbin.org/get",
            method: "GET"
        });
        if (resp.status !== 200) {
            throw new Error("expected status 200");
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
fn http_client_fetch_alias_works() {
    let dir = unique_temp_dir("http_fetch");
    fs::create_dir_all(&dir).unwrap();
    let script = r#"
        const http = require("@std/net/http/client");
        const resp = http.fetch("https://httpbin.org/get");
        if (resp.status !== 200) {
            throw new Error("expected status 200");
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
fn http_client_request_async_returns_promise_response() {
    let dir = unique_temp_dir("http_request_async");
    fs::create_dir_all(&dir).unwrap();
    let url = spawn_mock_http_server("async ok");
    let script = format!(
        r#"
        const http = require("@std/http");
        const resp = await http.requestAsync({{
            url: "{url}",
            method: "GET",
            headers: {{ "X-Client": "gts" }}
        }});
        if (resp.status !== 200) {{
            throw new Error("expected status 200, got " + resp.status);
        }}
        if (!resp.ok) {{
            throw new Error("expected ok true");
        }}
        if (resp.body !== "async ok") {{
            throw new Error("unexpected body " + resp.body);
        }}
        if (typeof resp.headers !== "object") {{
            throw new Error("expected headers object");
        }}
        print("async test passed");
    "#
    );
    let out = run_script(&dir, &script);
    fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "script failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("async test passed"));
}

#[test]
fn http_client_request_async_reuses_keepalive_connection() {
    let dir = unique_temp_dir("http_request_async_pool");
    fs::create_dir_all(&dir).unwrap();
    let request_count = 64;
    let (url, server) = spawn_keepalive_http_server(request_count);
    let expected = (0..request_count)
        .map(|i| format!("pooled-{i}"))
        .collect::<Vec<_>>()
        .join("|");
    let script = format!(
        r#"
        const http = require("@std/http");
        let bodies = "";
        for (let i = 0; i < {request_count}; i = i + 1) {{
            const resp = await http.requestAsync({{
                url: "{url}",
                method: "GET",
                timeoutMs: 2000
            }});
            if (resp.status !== 200) {{
                throw new Error("expected status 200, got " + resp.status);
            }}
            if (i > 0) {{
                bodies = bodies + "|";
            }}
            bodies = bodies + resp.body;
        }}
        if (bodies !== "{expected}") {{
            throw new Error("unexpected pooled bodies " + bodies);
        }}
        print("pooled async test passed");
    "#
    );
    let out = run_script(&dir, &script);
    fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "script failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    server.join().expect("keepalive server");
    assert!(String::from_utf8_lossy(&out.stdout).contains("pooled async test passed"));
}
