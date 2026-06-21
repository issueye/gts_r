use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use gts::object::Object;
use gts::runtime::{RunOptions, Session};

#[test]
fn session_runs_source_expression() {
    let session = Session::new();
    let result = session
        .run_source(
            "let x = 1 + 2 * 3;\nfunction add(a, b) { return a + b; }\nadd(x, 5);",
            "inline.gs",
        )
        .expect("script should run");

    assert_number(result, 12.0);
}

#[test]
fn session_requires_relative_module() {
    let dir = unique_temp_dir("gts-runtime-module");
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(
        dir.join("math.gs"),
        "export function twice(x) { return x * 2; }\nexport const label = \"math\";\n",
    )
    .expect("write module");
    fs::write(
        dir.join("main.gs"),
        "let math = require(\"./math\");\nmath.twice(9);",
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file(dir.join("main.gs"), Vec::new())
        .expect("script should run");

    assert_number(result, 18.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_supports_module_exports_assignment() {
    let dir = unique_temp_dir("gts-runtime-module-exports");
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(
        dir.join("math.gs"),
        "module.exports = { value: 21, double: function(x) { return x * 2; } };\n",
    )
    .expect("write module");
    fs::write(
        dir.join("main.gs"),
        "let math = require(\"./math\");\nmath.double(math.value);",
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file(dir.join("main.gs"), Vec::new())
        .expect("script should run");

    assert_number(result, 42.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_resolves_directory_modules() {
    let dir = unique_temp_dir("gts-runtime-dir-module");
    let pkg = dir.join("pkg");
    fs::create_dir_all(&pkg).expect("create package dir");
    fs::write(pkg.join("index.gs"), "export const value = 31;\n").expect("write index");
    fs::write(
        dir.join("main.gs"),
        "let pkg = require(\"./pkg\");\npkg.value + 11;",
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file(dir.join("main.gs"), Vec::new())
        .expect("script should run");

    assert_number(result, 42.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_resolves_project_entry_modules() {
    let dir = unique_temp_dir("gts-runtime-project-module");
    let pkg = dir.join("pkg");
    fs::create_dir_all(pkg.join("src")).expect("create package src");
    fs::write(
        pkg.join("project.toml"),
        "[project]\nname = \"pkg\"\nentry = \"src/app.gs\"\n",
    )
    .expect("write project");
    fs::write(pkg.join("src/app.gs"), "export const value = 40;\n").expect("write app");
    fs::write(
        dir.join("main.gs"),
        "let pkg = require(\"./pkg\");\npkg.value + 2;",
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file(dir.join("main.gs"), Vec::new())
        .expect("script should run");

    assert_number(result, 42.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_resolves_json_modules() {
    let dir = unique_temp_dir("gts-runtime-json-module");
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(dir.join("config.json"), r#"{"name":"ada","count":3}"#).expect("write json");
    fs::write(
        dir.join("main.gs"),
        r#"
let config = require("./config");
config.name + ":" + String(config.count);
"#,
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file(dir.join("main.gs"), Vec::new())
        .expect("script should run");

    assert_eq!(result.inspect(), "ada:3");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_resolves_project_import_aliases() {
    let dir = unique_temp_dir("gts-runtime-import-alias");
    fs::create_dir_all(dir.join("src/lib")).expect("create src");
    fs::write(
        dir.join("project.toml"),
        "[project]\nname = \"app\"\nentry = \"src/main.gs\"\n\n[imports]\n\"#lib/*\" = \"src/lib/*.gs\"\n",
    )
    .expect("write project");
    fs::write(
        dir.join("src/lib/math.gs"),
        "export function triple(x) { return x * 3; }\n",
    )
    .expect("write module");
    fs::write(
        dir.join("src/main.gs"),
        "let math = require(\"#lib/math\");\nmath.triple(14);",
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file(dir.join("src/main.gs"), Vec::new())
        .expect("script should run");

    assert_number(result, 42.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_resolves_package_exports_from_dependencies() {
    let dir = unique_temp_dir("gts-runtime-package-export");
    fs::create_dir_all(dir.join("app")).expect("create app");
    fs::create_dir_all(dir.join("tools/src/format")).expect("create package");
    fs::write(
        dir.join("app/project.toml"),
        "[project]\nname = \"app\"\nentry = \"main.gs\"\n\n[dependencies]\ntools = \"file:../tools\"\n",
    )
    .expect("write app project");
    fs::write(
        dir.join("tools/project.toml"),
        "[project]\nname = \"tools\"\n\n[package]\nname = \"tools\"\nversion = \"1.0.0\"\nmain = \"src/index.gs\"\n\n[exports]\n\".\" = \"src/index.gs\"\n\"./format/*\" = \"src/format/*.gs\"\n",
    )
    .expect("write tools project");
    fs::write(dir.join("tools/src/index.gs"), "export const value = 40;\n").expect("write index");
    fs::write(
        dir.join("tools/src/format/message.gs"),
        "export function suffix(x) { return x + 2; }\n",
    )
    .expect("write sub export");
    fs::write(
        dir.join("app/main.gs"),
        r#"
let tools = require("tools");
let fmt = require("tools/format/message");
fmt.suffix(tools.value);
"#,
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file(dir.join("app/main.gs"), Vec::new())
        .expect("script should run");

    assert_number(result, 42.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_returns_partial_exports_for_circular_require() {
    let dir = unique_temp_dir("gts-runtime-circular-module");
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(
        dir.join("a.gs"),
        r#"
exports.ready = "loading";
let b = require("./b");
exports.fromB = b.seenA;
exports.ready = "done";
"#,
    )
    .expect("write a");
    fs::write(
        dir.join("b.gs"),
        r#"
let a = require("./a");
exports.seenA = a.ready;
"#,
    )
    .expect("write b");
    fs::write(
        dir.join("main.gs"),
        r#"
let a = require("./a");
a.fromB + ":" + a.ready;
"#,
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file(dir.join("main.gs"), Vec::new())
        .expect("script should run");

    assert_eq!(result.inspect(), "loading:done");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_loads_core_native_stdlib_modules() {
    let dir = unique_temp_dir("gts-runtime-stdlib-core");
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(
        dir.join("main.gs"),
        r#"
let path = require("@std/path");
let os = require("@std/os");
let env = require("@std/env");
env.set("GTS_R_STD_TEST", "42");
let joined = path.toSlash(path.join("alpha", "beta.txt"));
let parsed = path.parse("alpha/beta.txt");
println("stdlib=", joined, ":", parsed.ext, ":", env.getInt("GTS_R_STD_TEST"), ":", os.cpus() > 0);
env.unset("GTS_R_STD_TEST");
"#,
    )
    .expect("write main");

    let session = Session::new();
    session
        .run_file(dir.join("main.gs"), Vec::new())
        .expect("script should run");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_loads_fs_json_and_time_stdlib_modules() {
    let dir = unique_temp_dir("gts-runtime-stdlib-extra");
    fs::create_dir_all(&dir).expect("create temp dir");
    let root = dir.to_string_lossy().replace('\\', "\\\\");
    fs::write(
        dir.join("main.gs"),
        format!(
            r#"
let fs = require("@std/fs");
let path = require("@std/path");
let json = require("@std/json");
let time = require("@std/time");
let file = path.join("{root}", "data.txt");
fs.writeTextSync(file, "alpha");
fs.appendTextSync(file, "-beta");
let doc = json.parse5("{{ items: [{{ name: 'one' }}] }}");
json.set(doc, "/items/0/count", 3);
let duration = time.duration(250);
fs.readTextSync(file) + ":" + String(json.get(doc, "/items/0/count")) + ":" + String(duration.milliseconds);
"#
        ),
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file(dir.join("main.gs"), Vec::new())
        .expect("script should run");

    assert_eq!(result.inspect(), "alpha-beta:3:250");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_binds_caught_error_without_refcell_reentry() {
    let session = Session::new();
    let result = session
        .run_source(
            r#"
let label = "none";
try {
  throw new Error("boom");
} catch (err) {
  label = err.message;
}
label;
"#,
            "try-catch-error.gs",
        )
        .expect("script should run");

    assert_eq!(result.inspect(), "boom");
}

#[test]
fn session_can_call_top_level_main() {
    let dir = unique_temp_dir("gts-runtime-main");
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(
        dir.join("main.gs"),
        "let called = 0;\nfunction main() { called = 42; return called; }\n",
    )
    .expect("write main");

    let session = Session::new();
    let result = session
        .run_file_with_options(
            dir.join("main.gs"),
            RunOptions {
                argv: Vec::new(),
                call_main: true,
                timeout: None,
            },
        )
        .expect("script should run");

    assert_number(result, 42.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_run_uses_project_entry() {
    let dir = unique_temp_dir("gts-cli-project");
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(
        dir.join("project.toml"),
        "[project]\nname = \"cli-test\"\nentry = \"src/app.gs\"\n",
    )
    .expect("write project");
    fs::create_dir_all(dir.join("src")).expect("create src");
    fs::write(dir.join("src/app.gs"), "println(\"project-entry-ok\");\n").expect("write app");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("run gs");

    assert!(
        output.status.success(),
        "gs run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "project-entry-ok\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_run_uses_package_main_as_default_entry() {
    let dir = unique_temp_dir("gts-cli-package-main");
    fs::create_dir_all(dir.join("src")).expect("create src");
    fs::write(
        dir.join("project.toml"),
        "[package]\nname = \"cli-main\"\nmain = \"src/app.gs\"\n",
    )
    .expect("write project");
    fs::write(dir.join("src/app.gs"), "println(\"package-main-ok\");\n").expect("write app");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("run gs");

    assert!(
        output.status.success(),
        "gs run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "package-main-ok\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_supports_gs_agent_runtime_primitives() {
    let session = Session::new();
    let result = session
        .run_source(
            r#"
let compact = JSON.stringify({ a: 1, b: ["x"] }, null, 2).replace(/\r?\n\s*/g, "");
let hasDateNow = String(Date.now()).length > 0;
let sse = require("@std/sse");
let event = sse.reader("event: message\ndata: {\"ok\":true}\n\n").next();
hasDateNow && compact.includes("\"a\": 1") && event.event === "message" && event.data.includes("ok");
"#,
            "gs-agent-primitives.gs",
        )
        .expect("script should run");

    assert_bool(result, true);
}

#[test]
fn session_exec_command_builder_supports_set_dir() {
    let dir = unique_temp_dir("gts-exec-builder");
    let work = dir.join("work");
    fs::create_dir_all(&work).expect("create work dir");
    fs::write(work.join("marker.txt"), "ok").expect("write marker");

    let script = format!(
        r#"
let exec = require("@std/exec");
let cmd = exec.command("powershell", ["-NoProfile", "-Command", "Get-Content marker.txt"]);
let result = cmd.setDir({:?}).run();
result.success && result.stdout.trim() === "ok";
"#,
        work.to_string_lossy().replace('\\', "\\\\")
    );

    let session = Session::new();
    let result = session
        .run_source(&script, "exec-builder.gs")
        .expect("script should run");

    assert_bool(result, true);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn session_http_stream_body_shape_matches_sse_reader() {
    let session = Session::new();
    let result = session
        .run_source(
            r#"
let stream = require("@std/stream");
let sse = require("@std/sse");
let body = stream.fromString("event: message\ndata: {\"ok\":true}\n\n");
body.text = body.readAll();
let event = sse.reader(body).next();
event.event === "message" && event.data === "{\"ok\":true}";
"#,
            "http-stream-shape.gs",
        )
        .expect("script should run");

    assert_bool(result, true);
}

#[test]
fn session_http_body_objects_serialize_as_json() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut data = Vec::new();
        let mut buf = [0_u8; 4096];
        loop {
            let n = stream.read(&mut buf).expect("read request");
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
            let request = String::from_utf8_lossy(&data);
            if let Some(header_end) = request.find("\r\n\r\n") {
                let headers = &request[..header_end];
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        if name.eq_ignore_ascii_case("content-length") {
                            value.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                if data.len() >= header_end + 4 + content_length {
                    break;
                }
            }
        }
        let request = String::from_utf8_lossy(&data).to_string();
        tx.send(request).expect("send request");
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });

    let session = Session::new();
    let script = format!(
        r#"
let http = require("@std/net/http/client");
let response = http.request({{
  method: "POST",
  url: "http://{}",
  headers: {{ "content-type": "application/json" }},
  body: {{ ok: true }}
}});
response.ok;
"#,
        addr
    );
    let result = session
        .run_source(&script, "http-body-json.gs")
        .expect("script should run");

    assert_bool(result, true);
    let request = rx.recv().expect("receive request");
    assert!(
        request.contains(r#""ok": true"#),
        "expected JSON body, got:\n{request}"
    );
    handle.join().expect("server thread should finish");
}

fn assert_number(value: Object, expected: f64) {
    match value {
        Object::Number(actual) => assert_eq!(actual, expected),
        other => panic!("expected number {expected}, got {}", other.inspect()),
    }
}

fn assert_bool(value: Object, expected: bool) {
    match value {
        Object::Boolean(actual) => assert_eq!(actual, expected),
        other => panic!("expected bool {expected}, got {}", other.inspect()),
    }
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}-{}", std::process::id()))
}
