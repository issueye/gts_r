//! Parity tests for @std/net/ws/client and @std/net/ws/server.
//!
//! The synchronous VM has no background loop, so the server's `acceptOne`
//! performs the WS handshake inline and invokes the handler synchronously.
//! A genuine client/server round-trip is exercised against a small Rust-side
//! WS echo server spawned in a thread (the GTS server cannot run concurrently
//! with the GTS client inside one synchronous script).

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
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
fn ws_connect_refused_returns_error() {
    let dir = unique_temp_dir("gts-p9-ws-refused");
    fs::create_dir_all(&dir).expect("create temp dir");
    // Port 1 is unused → connection refused during the WS handshake.
    let output = run_script(
        &dir,
        r#"
let ws = require("@std/net/ws/client");
try {
    ws.connect("ws://127.0.0.1:1/");
    println("connected");
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
fn ws_server_listen_returns_bound_port() {
    let dir = unique_temp_dir("gts-p9-ws-listen");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let srv = require("@std/net/ws/server");
let server = srv.createServer(0, function(conn) {});
println(server.port > 0);
println(server.address);
server.close();
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert!(lines[1].starts_with(":"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn ws_client_module_exposes_connect_api() {
    let dir = unique_temp_dir("gts-p9-ws-api");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let ws = require("@std/net/ws/client");
// The module object should expose a callable connect function.
println(typeof ws.connect === "function");
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn ws_server_accept_one_without_pending_returns_wouldblock_error() {
    let dir = unique_temp_dir("gts-p9-ws-noblock");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let srv = require("@std/net/ws/server");
let server = srv.createServer(0, function(conn) {});
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
    assert_eq!(stdout_of(&output).trim(), "errored");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn ws_upgrade_returns_unsupported_error() {
    let dir = unique_temp_dir("gts-p9-ws-upgrade");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let srv = require("@std/net/ws/server");
try {
    srv.upgrade({});
    println("upgraded");
} catch (e) {
    println("errored");
}
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "errored");
    let _ = fs::remove_dir_all(dir);
}

/// Minimal SHA-1 + base64 for the server-side accept-key, mirroring the
/// interpreter's own helpers (kept self-contained in the test binary).
fn sha1_hex_accept(client_key: &str) -> String {
    const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let msg = format!("{}{}", client_key, GUID);
    let bytes = msg.as_bytes();

    // SHA-1 padding.
    let bit_len = (bytes.len() as u64) * 8;
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut h: [u32; 5] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5a827999),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let digest: Vec<u8> = h.iter().flat_map(|w| w.to_be_bytes()).collect();

    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in digest.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Spawn a thread running a one-shot WS echo server on an ephemeral port.
/// Returns the bound port. The server accepts a single connection, completes
/// the WS handshake, echoes one text frame, and exits.
fn spawn_echo_ws_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind echo server");
    let port = listener.local_addr().expect("local addr").port();
    std::thread::spawn(move || {
        let (mut stream, _) = match listener.accept() {
            Ok(p) => p,
            Err(_) => return,
        };
        // Read the client handshake.
        let mut buf = [0u8; 4096];
        let mut got = Vec::new();
        loop {
            let n = match stream.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };
            got.extend_from_slice(&buf[..n]);
            if got.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let head = String::from_utf8_lossy(&got);
        let key = head
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("sec-websocket-key:"))
            .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
            .unwrap_or_default();
        let accept = sha1_hex_accept(&key);
        let resp = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            accept
        );
        if stream.write_all(resp.as_bytes()).is_err() {
            return;
        }
        // Read one frame (client→server frames are masked).
        let mut hdr = [0u8; 2];
        if stream.read_exact(&mut hdr).is_err() {
            return;
        }
        let opcode = hdr[0] & 0x0f;
        let masked = hdr[1] & 0x80 != 0;
        let mut len = (hdr[1] & 0x7f) as usize;
        if len == 126 {
            let mut ext = [0u8; 2];
            let _ = stream.read_exact(&mut ext);
            len = u16::from_be_bytes(ext) as usize;
        }
        let mut mask = [0u8; 4];
        if masked {
            let _ = stream.read_exact(&mut mask);
        }
        let mut payload = vec![0u8; len];
        if stream.read_exact(&mut payload).is_err() {
            return;
        }
        if masked {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
        // Echo back as a server→client text frame (unmasked).
        let mut frame = vec![0x80 | opcode];
        if payload.len() <= 125 {
            frame.push(payload.len() as u8);
        } else if payload.len() <= 65535 {
            frame.push(126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        } else {
            frame.push(127);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
        frame.extend_from_slice(&payload);
        let _ = stream.write_all(&frame);
        let _ = stream.flush();
        // Keep the socket briefly so the client can read the echo.
        std::thread::sleep(std::time::Duration::from_millis(200));
    });
    port
}

#[test]
fn ws_client_echo_roundtrip_against_rust_server() {
    let dir = unique_temp_dir("gts-p9-ws-client-echo");
    fs::create_dir_all(&dir).expect("create temp dir");
    let port = spawn_echo_ws_server();

    let script = format!(
        r#"
let ws = require("@std/net/ws/client");
let conn = ws.connect("ws://127.0.0.1:{port}/");
conn.send("hello");
let reply = conn.recv();
println(reply);
conn.close();
"#
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "hello");
    let _ = fs::remove_dir_all(dir);
}
