//! Parity tests for @std/web (Express-style framework).
//!
//! The synchronous VM has no background loop, so `app.listen(port, {count: N})`
//! processes N requests then returns. Tests spawn the GTS server as a
//! subprocess, discover its port via stdout, and drive HTTP requests at it.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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

fn spawn_server_script(dir: &PathBuf, script: &str) -> (std::process::Child, u16) {
    let file = write_script(dir, script);
    let mut child = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gs");

    let mut stdout = child.stdout.take().expect("take stdout");
    let mut buf = [0u8; 256];
    let mut got = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        match stdout.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                got.extend_from_slice(&buf[..n]);
                if let Some(nl) = got.iter().position(|&b| b == b'\n') {
                    let line = String::from_utf8_lossy(&got[..nl]).to_string();
                    if let Some(rest) = line.strip_prefix("GTS_PORT=") {
                        let port: u16 = rest.trim().parse().expect("parse port");
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

fn spawn_delayed_http_upstream(body: &'static str, delay: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind delayed upstream");
    let addr = listener.local_addr().expect("delayed upstream addr");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept delayed upstream request");
        let mut buf = [0; 1024];
        let _ = stream.read(&mut buf);
        std::thread::sleep(delay);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write delayed upstream response");
    });
    format!("http://{addr}/slow")
}

#[test]
fn web_get_route_returns_text() {
    let dir = unique_temp_dir("gts-p9-web-get");
    fs::create_dir_all(&dir).expect("create temp dir");
    // Use a fixed high port with retry to avoid the bind-then-print race.
    let script = r#"
let web = require("@std/web");
let app = web.createApp();
app.get("/hello", function(req, res) {
    res.send("hi from web");
});
let port = 18080;
println(`GTS_PORT=${port}`);
app.listen(port, {count: 1});
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    // Small delay so the server is listening before we connect.
    std::thread::sleep(Duration::from_millis(100));
    let (status, _head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(status.contains("200"), "status was: {}", status);
    assert_eq!(body, "hi from web");
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_route_params_and_json() {
    let dir = unique_temp_dir("gts-p9-web-params");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let web = require("@std/web");
let app = web.createApp();
app.get("/users/:id", function(req, res) {
    res.json({ id: req.params.id });
});
let port = 18081;
println(`GTS_PORT=${port}`);
app.listen(port, {count: 1});
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    std::thread::sleep(Duration::from_millis(100));
    let (status, head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET /users/42 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(status.contains("200"), "status was: {}", status);
    assert!(head
        .to_ascii_lowercase()
        .contains("content-type: application/json"));
    assert_eq!(body, "{\"id\": \"42\"}");
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_express_handler_receives_route_params() {
    let dir = unique_temp_dir("gts-p9-web-express-params");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let web = require("@std/web");
let app = web.createApp();
app.post("/providers/:provider_id/models", function(req, res) {
    res.json({ provider: req.params.provider_id });
});
let port = 18086;
println(`GTS_PORT=${port}`);
app.listen(port, {count: 1});
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    std::thread::sleep(Duration::from_millis(100));
    let (status, head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "POST /providers/perf-openai/models HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
    );
    assert!(status.contains("200"), "status was: {}", status);
    assert!(head
        .to_ascii_lowercase()
        .contains("content-type: application/json"));
    assert_eq!(body, "{\"provider\": \"perf-openai\"}");
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_unmatched_route_returns_404() {
    let dir = unique_temp_dir("gts-p9-web-404");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let web = require("@std/web");
let app = web.createApp();
app.get("/here", function(req, res) { res.send("ok"); });
let port = 18082;
println(`GTS_PORT=${port}`);
app.listen(port, {count: 1});
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    std::thread::sleep(Duration::from_millis(100));
    let (status, _head, _body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET /nope HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(status.contains("404"), "status was: {}", status);
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_middleware_runs_before_route() {
    let dir = unique_temp_dir("gts-p9-web-mw");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let web = require("@std/web");
let app = web.createApp();
let log = [];
app.use(function(req, res, next) { log.push("mw"); });
app.get("/x", function(req, res) {
    res.send(`log=${log.length}`);
});
let port = 18083;
println(`GTS_PORT=${port}`);
app.listen(port, {count: 1});
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    std::thread::sleep(Duration::from_millis(100));
    let (status, _head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET /x HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(status.contains("200"), "status was: {}", status);
    assert_eq!(body, "log=1");
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_json_helper_serializes() {
    let dir = unique_temp_dir("gts-p9-web-jsonhelper");
    fs::create_dir_all(&dir).expect("create temp dir");
    // Pure unit-style check of the web.json helper (no server needed).
    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg("-e")
        .arg(r#"let web = require("@std/web"); println(web.json({a:1,b:"x"}));"#)
        .output()
        .expect("run gs -e");
    // -e may not be supported; fall back to a script file.
    if !output.status.success() {
        let file = dir.join("main.gs");
        fs::write(
            &file,
            r#"let web = require("@std/web"); println(web.json({a:1,b:"x"}));"#,
        )
        .expect("write");
        let out2 = Command::new(env!("CARGO_BIN_EXE_gs"))
            .arg(&file)
            .output()
            .expect("run gs");
        let stdout = String::from_utf8_lossy(&out2.stdout);
        assert!(stdout.contains("\"a\": 1"), "stdout: {}", stdout);
        assert!(stdout.contains("\"b\": \"x\""), "stdout: {}", stdout);
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("\"a\": 1"), "stdout: {}", stdout);
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_json_middleware_parses_request_body() {
    let dir = unique_temp_dir("gts-p9-web-json-middleware");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let web = require("@std/web");
let app = web.createApp();
let port = 18084;
println(`GTS_PORT=${port}`);
app.use(web.json());
app.post("/body", function(req, res) {
  res.json({ name: req.body.name, age: req.body.age });
});
app.listen(port, {count: 1});
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    std::thread::sleep(Duration::from_millis(100));
    let (status, head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "POST /body HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 25\r\nConnection: close\r\n\r\n{\"name\":\"alice\",\"age\":30}",
    );
    assert!(status.contains("200"), "status was: {}", status);
    assert!(
        head.to_ascii_lowercase()
            .contains("content-type: application/json"),
        "head: {}",
        head
    );
    assert!(body.contains("\"name\": \"alice\""), "body: {}", body);
    assert!(body.contains("\"age\": 30"), "body: {}", body);
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_listen_default_serves_multiple_requests() {
    let dir = unique_temp_dir("gts-p9-web-default-listen");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let web = require("@std/web");
let app = web.createApp();
app.get("/hello", function(req, res, next) {
  res.send("world");
});
let port = 18085;
println(`GTS_PORT=${port}`);
app.listen(port);
"#;
    let (mut child, port) = spawn_server_script(&dir, script);
    std::thread::sleep(Duration::from_millis(100));
    for _ in 0..2 {
        let (status, _head, body) = http_round_trip(
            "127.0.0.1",
            port,
            "GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        assert!(status.contains("200"), "status was: {}", status);
        assert_eq!(body, "world");
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_handler_returned_promise_delays_response_until_settled() {
    let dir = unique_temp_dir("gts-p9-web-promise-handler");
    fs::create_dir_all(&dir).expect("create temp dir");
    let upstream = spawn_delayed_http_upstream("upstream ready", Duration::from_millis(150));
    let script = format!(
        r#"
let web = require("@std/web");
let http = require("@std/http");
let app = web.createApp();
app.get("/proxy", function(req, res) {{
  return http.requestAsync({{
    url: "{upstream}",
    method: "GET",
    timeoutMs: 3000
  }});
}});
let port = 18086;
println(`GTS_PORT=${{port}}`);
app.listen(port, {{count: 1}});
"#
    );
    let (mut child, port) = spawn_server_script(&dir, &script);
    std::thread::sleep(Duration::from_millis(100));
    let start = std::time::Instant::now();
    let (status, _head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET /proxy HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    let elapsed = start.elapsed();
    assert!(status.contains("200"), "status was: {}", status);
    assert_eq!(body, "");
    assert!(
        elapsed >= Duration::from_millis(120),
        "response returned before handler promise settled: {:?}",
        elapsed
    );
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_async_handler_can_update_response_after_resume() {
    let dir = unique_temp_dir("gts-p9-web-async-response-state");
    fs::create_dir_all(&dir).expect("create temp dir");
    let upstream = spawn_delayed_http_upstream("upstream body", Duration::from_millis(100));
    let script = format!(
        r#"
let web = require("@std/web");
let http = require("@std/http");
let app = web.createApp();
app.get("/proxy", function(req, res) {{
  return http.requestAsync({{
    url: "{upstream}",
    method: "GET",
    timeoutMs: 3000
  }}).then(function(resp) {{
    res.status(202);
    res.setHeader("X-Upstream-Status", resp.status);
    res.send(resp.body);
  }});
}});
let port = 18094;
println(`GTS_PORT=${{port}}`);
app.listen(port, {{count: 1}});
"#
    );
    let (mut child, port) = spawn_server_script(&dir, &script);
    std::thread::sleep(Duration::from_millis(100));
    let (status, head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET /proxy HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(status.contains("202"), "status was: {}", status);
    assert!(
        head.to_ascii_lowercase().contains("x-upstream-status: 200"),
        "head: {}",
        head
    );
    assert_eq!(body, "upstream body");
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_single_worker_does_not_block_fast_route_while_slow_route_waits() {
    let dir = unique_temp_dir("gts-p9-web-single-worker-async");
    fs::create_dir_all(&dir).expect("create temp dir");
    let upstream = spawn_delayed_http_upstream("slow", Duration::from_millis(500));
    let script = format!(
        r#"
let web = require("@std/web");
let http = require("@std/http");
let app = web.createApp();
app.get("/slow", function(req, res) {{
  return http.requestAsync({{
    url: "{upstream}",
    method: "GET",
    timeoutMs: 3000
  }}).then(function(resp) {{
    res.send(resp.body);
  }});
}});
app.get("/healthz", function(req, res) {{
  res.send("ok");
}});
let port = 18087;
println(`GTS_PORT=${{port}}`);
app.listen(port);
"#
    );
    let (mut child, port) = spawn_server_script(&dir, &script);
    std::thread::sleep(Duration::from_millis(100));

    let slow = std::thread::spawn(move || {
        http_round_trip(
            "127.0.0.1",
            port,
            "GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
    });

    std::thread::sleep(Duration::from_millis(50));
    let health_start = std::time::Instant::now();
    let (status, _head, body) = http_round_trip(
        "127.0.0.1",
        port,
        "GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    let health_elapsed = health_start.elapsed();

    assert!(status.contains("200"), "status was: {}", status);
    assert_eq!(body, "ok");
    assert!(
        health_elapsed < Duration::from_millis(150),
        "single-worker fast route was blocked for {:?}",
        health_elapsed
    );

    let (slow_status, _slow_head, slow_body) = slow.join().expect("join slow request");
    assert!(slow_status.contains("200"), "status was: {}", slow_status);
    assert_eq!(slow_body, "slow");

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// Concurrent (prefork) server tests — `app.listen(port, { workers: N })`.
//
// These verify that multiple requests are served in parallel by independent
// worker VMs sharing one listener. The key correctness property: N slow
// requests complete in ~1× the slow duration, not ~N×.
// ---------------------------------------------------------------------------

/// Spawn a long-running worker server and return the (child, port). The script
/// must print `GTS_PORT=<port>` on its first output line once the main thread
/// has bound the listener.
fn spawn_worker_server(dir: &PathBuf, script: &str) -> (std::process::Child, u16) {
    spawn_server_script(dir, script)
}

/// Fire `n` concurrent requests and return (responses, total_elapsed).
fn concurrent_round_trips(port: u16, path: &str, n: usize) -> (Vec<String>, Duration) {
    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for _ in 0..n {
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            path
        );
        let h = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream.write_all(request.as_bytes()).expect("write");
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).expect("read");
            let text = String::from_utf8_lossy(&buf).to_string();
            text.splitn(2, "\r\n\r\n").nth(1).unwrap_or("").to_string()
        });
        handles.push(h);
    }
    let mut responses = Vec::new();
    for h in handles {
        responses.push(h.join().expect("join client thread"));
    }
    (responses, start.elapsed())
}

#[test]
fn web_concurrent_requests_run_in_parallel() {
    // Handler sleeps 300ms. With 4 workers, 4 concurrent requests should finish
    // in ~300ms (parallel). Serially they'd take ~1200ms. We assert < 800ms to
    // leave headroom for thread startup and OS scheduling.
    let dir = unique_temp_dir("gts-p9-web-concurrent");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let web = require("@std/web");
let timers = require("@std/timers");
let app = web.createApp();
app.get("/slow", function(req, res) {
    timers.sleep(300);
    res.send("ok");
});
let port = 18091;
println(`GTS_PORT=${port}`);
app.listen(port, { workers: 4 });
"#;
    let (mut child, port) = spawn_worker_server(&dir, script);
    std::thread::sleep(Duration::from_millis(300));

    let (responses, elapsed) = concurrent_round_trips(port, "/slow", 4);

    // All four must succeed.
    for (i, r) in responses.iter().enumerate() {
        assert_eq!(r, "ok", "response {} was: {:?}", i, r);
    }
    // Parallelism check: 4 × 300ms serially = 1200ms; in parallel ≈ 300ms.
    // Allow generous headroom (800ms) for worker startup jitter.
    assert!(
        elapsed < Duration::from_millis(800),
        "4 parallel slow requests took {:?} (expected < 800ms; serial would be ~1200ms)",
        elapsed
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_workers_serve_multiple_routes() {
    let dir = unique_temp_dir("gts-p9-web-routes");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let web = require("@std/web");
let app = web.createApp();
app.get("/ping", function(req, res) { res.send("pong"); });
app.get("/echo/:id", function(req, res) { res.json({ id: req.params.id }); });
app.get("/hello", function(req, res) { res.send("world"); });
let port = 18092;
println(`GTS_PORT=${port}`);
app.listen(port, { workers: 2 });
"#;
    let (mut child, port) = spawn_worker_server(&dir, script);
    // Give workers time to reach their accept loops.
    std::thread::sleep(Duration::from_millis(800));

    // Hit three different routes concurrently using the proven round-trip
    // helper (each thread owns its own socket).
    let mk = move |path: &'static str| {
        std::thread::spawn(move || {
            concurrent_round_trips(port, path, 1)
                .0
                .into_iter()
                .next()
                .unwrap_or_default()
        })
    };
    let h1 = mk("/ping");
    let h2 = mk("/echo/42");
    let h3 = mk("/hello");

    let b1 = h1.join().unwrap();
    let b2 = h2.join().unwrap();
    let b3 = h3.join().unwrap();

    assert_eq!(b1, "pong");
    // :id is captured as a string (route params are stringly-typed).
    assert!(
        b2.contains("\"id\": \"42\"") || b2.contains("\"id\":\"42\""),
        "echo body: {:?}",
        b2
    );
    assert_eq!(b3, "world");

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn web_close_shuts_down_workers() {
    // After serving one request, the script calls app.close(), which must
    // cause the long-running listen() to return and the process to exit
    // promptly (no hang waiting for shutdown).
    let dir = unique_temp_dir("gts-p9-web-close");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let web = require("@std/web");
let app = web.createApp();
app.get("/stop", function(req, res) {
    res.send("bye");
    app.close();
});
let port = 18093;
println(`GTS_PORT=${port}`);
app.listen(port, { workers: 2 });
println("SERVER_EXITED");
"#;
    let (mut child, port) = spawn_worker_server(&dir, script);
    std::thread::sleep(Duration::from_millis(800));

    // One request triggers app.close() inside the handler. Use the
    // concurrent helper to avoid read_to_end hanging on keep-alive.
    let body = concurrent_round_trips(port, "/stop", 1)
        .0
        .into_iter()
        .next()
        .unwrap_or_default();
    assert_eq!(body, "bye");

    // The process should exit on its own within a few seconds (listen returns
    // after workers drain). Poll for exit; assert it didn't hang.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut exited = false;
    while std::time::Instant::now() < deadline {
        match child.try_wait().expect("try_wait") {
            Some(_status) => {
                exited = true;
                break;
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    assert!(exited, "server process did not exit after app.close()");

    let _ = fs::remove_dir_all(dir);
}
