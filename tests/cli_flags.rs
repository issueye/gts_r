use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cli_accepts_short_help_and_version_flags() {
    let help = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg("-h")
        .output()
        .expect("run gs -h");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--timeout <duration>"));

    let version = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg("-v")
        .output()
        .expect("run gs -v");
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("GoScript "));
}

#[test]
fn cli_init_creates_runnable_project() {
    let dir = unique_temp_dir("gts-cli-init").join("hello-app");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .args(["init"])
        .arg(&dir)
        .output()
        .expect("run gs init");

    assert!(
        output.status.success(),
        "gs init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(&dir.to_string_lossy().to_string()));

    let project = fs::read_to_string(dir.join("project.toml")).expect("read project.toml");
    assert!(project.contains("name = \"hello-app\""));
    assert!(project.contains("entry = \"main.gs\""));

    let main = fs::read_to_string(dir.join("main.gs")).expect("read main.gs");
    assert!(main.contains("function main()"));

    let run = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("run initialized project");
    assert!(
        run.status.success(),
        "initialized project failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Hello, GoScript!\n");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_init_does_not_overwrite_existing_project() {
    let dir = unique_temp_dir("gts-cli-init-existing");
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(dir.join("project.toml"), "[project]\nname = \"existing\"\n").expect("write project");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .args(["init"])
        .arg(&dir)
        .output()
        .expect("run gs init");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert_eq!(
        fs::read_to_string(dir.join("project.toml")).expect("read project"),
        "[project]\nname = \"existing\"\n"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_repl_supports_commands_and_persistent_session() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gs"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn gs repl");

    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("child stdin");
        stdin
            .write_all(b".help\nlet value = 40;\nvalue + 2\n.exit\n")
            .expect("write repl input");
    }

    let output = child.wait_with_output().expect("wait for repl");
    assert!(
        output.status.success(),
        "repl failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(".help        Show this help"));
    assert!(stdout.contains(".exit        Exit the REPL"));
    assert!(
        stdout.contains("42"),
        "expected persisted binding result in stdout:\n{stdout}"
    );
}

#[test]
fn cli_accepts_timeout_and_workers_for_direct_file() {
    let dir = unique_temp_dir("gts-cli-flags-direct");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("main.gs");
    fs::write(&script, "println(\"flags-ok\");\n").expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .args(["--timeout", "100ms", "--workers", "2"])
        .arg(&script)
        .output()
        .expect("run gs with flags");

    assert!(
        output.status.success(),
        "gs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "flags-ok\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_accepts_timeout_and_workers_for_run() {
    let dir = unique_temp_dir("gts-cli-flags-run");
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(dir.join("main.gs"), "println(\"run-flags-ok\");\n").expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .args(["run", "--timeout", "1s", "--workers", "1"])
        .current_dir(&dir)
        .output()
        .expect("run gs run with flags");

    assert!(
        output.status.success(),
        "gs run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "run-flags-ok\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_exec_mode_tree_keeps_treewalker_fallback() {
    let dir = unique_temp_dir("gts-cli-exec-mode-tree");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("nullish.gs");
    fs::write(
        &script,
        r#"
let fallback = null ?? 42;
println(`tree=${fallback}`);
"#,
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg("--exec-mode=tree")
        .arg(&script)
        .output()
        .expect("run gs with tree exec mode");

    assert!(
        output.status.success(),
        "gs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "tree=42\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_timeout_stops_infinite_loop() {
    let dir = unique_temp_dir("gts-cli-timeout");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("loop.gs");
    fs::write(&script, "while (true) {}\nprintln(\"unreachable\");\n").expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .args(["--timeout", "10ms"])
        .arg(&script)
        .output()
        .expect("run timeout script");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("TimeoutError"),
        "expected TimeoutError in stderr, got:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_can_require_core_native_stdlib_modules() {
    let dir = unique_temp_dir("gts-cli-stdlib-core");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("stdlib.gs");
    fs::write(
        &script,
        r#"
let path = require("@std/path");
let env = require("@std/env");
env.set("GTS_R_STD_CLI_TEST", "7");
let parsed = path.parse("alpha/beta.txt");
println("stdlib-cli=", path.toSlash(path.join("alpha", "beta.txt")), ":", parsed.name, ":", env.getInt("GTS_R_STD_CLI_TEST"));
env.unset("GTS_R_STD_CLI_TEST");
"#,
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&script)
        .output()
        .expect("run stdlib script");

    assert!(
        output.status.success(),
        "stdlib script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "stdlib-cli=alpha/beta.txt:beta:7\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_can_use_fs_json_and_time_stdlib_modules() {
    let dir = unique_temp_dir("gts-cli-stdlib-extra");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("stdlib-extra.gs");
    let root = dir.to_string_lossy().replace('\\', "\\\\");
    fs::write(
        &script,
        format!(
            r#"
let fs = require("@std/fs");
let path = require("@std/path");
let json = require("@std/json");
let time = require("@std/time");
let root = "{root}";
let file = path.join(root, "note.txt");
fs.writeFileSync(file, "one");
fs.appendFileSync(file, "-two");
let stat = fs.statSync(file);
let doc = json.parse5("{{ user: {{ name: 'Ada', age: 36, tags: ['math'] }}, active: true, }}");
let validation = json.validate(doc.user, {{ type: "object", required: ["name"], properties: {{ age: {{ type: "number", minimum: 30 }} }} }});
json.set(doc, "/user/city", "London");
json.patch(doc, [{{ op: "replace", path: "/user/name", value: "Grace" }}]);
let diff = json.diff({{ a: 1 }}, {{ a: 2, b: true }});
let duration = time.parseDuration("1.5s");
println(
  "stdlib-extra=",
  fs.readFileSync(file),
  ":",
  stat.isFile(),
  ":",
  json.get(doc, "/user/name"),
  ":",
  json.get(doc, "/user/city"),
  ":",
  validation.valid,
  ":",
  diff.length,
  ":",
  duration.milliseconds
);
fs.rmSync(file);
"#
        ),
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&script)
        .output()
        .expect("run stdlib extra script");

    assert!(
        output.status.success(),
        "stdlib extra script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "stdlib-extra=one-two:true:Grace:London:true:2:1500\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_can_use_enhanced_fs_stdlib_module() {
    let dir = unique_temp_dir("gts-cli-stdlib-fs-enhanced");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("fs-enhanced.gs");
    let root = dir.join("work").to_string_lossy().replace('\\', "\\\\");
    fs::write(
        &script,
        format!(
            r#"
let fs = require("@std/fs");
let path = require("@std/path");
let root = "{root}";
fs.mkdirSync(path.join(root, "nested"), {{ recursive: true }});
let file = path.join(root, "nested", "note.txt");
fs.writeFileAtomicSync(file, "one");
fs.appendTextSync(file, "\ntwo");
let tmpDir = fs.mkdtempSync(path.join(root, "tmp-"));
let copy = path.join(root, "copy.txt");
fs.copyFileSync(file, copy);
let stat = fs.lstatSync(copy);
let walked = fs.walkSync(root, {{ includeDirs: false }});
let typed = fs.readdirSync(root, {{ withFileTypes: true }});
let typedKind = "bad";
for (let i = 0; i < typed.length; i = i + 1) {{
  if (typed[i].name === "nested" && typed[i].isDirectory()) {{
    typedKind = "dirent";
  }}
}}
let globbed = fs.globSync(path.join(root, "nested", "*.txt"));
let writerFile = path.join(root, "writer.txt");
let writer = fs.createThrottledWriter(writerFile, {{ flushIntervalMs: 1 }});
writer.write("latest");
writer.flush();
fs.rmSync(path.join(root, "nested"), {{ recursive: true, force: true }});
fs.rmSync(path.join(root, "missing"), {{ recursive: true, force: true }});
println(
  "fs-enhanced=",
  fs.readTextSync(copy),
  ":",
  stat.isFile(),
  ":",
  stat.isSymlink(),
  ":",
  walked.length,
  ":",
  typedKind,
  ":",
  path.basename(globbed[0]),
  ":",
  fs.existsSync(tmpDir),
  ":",
  fs.readTextSync(writerFile),
  ":",
  fs.existsSync(file)
);
"#
        ),
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&script)
        .output()
        .expect("run enhanced fs script");

    assert!(
        output.status.success(),
        "enhanced fs script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "fs-enhanced=one\ntwo:true:false:2:dirent:note.txt:true:latest:false\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_can_use_enhanced_time_stdlib_module() {
    let dir = unique_temp_dir("gts-cli-stdlib-time-enhanced");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("time-enhanced.gs");
    fs::write(
        &script,
        r#"
let time = require("@std/time");
let parsed = time.parse("2020-01-02T03:04:05Z");
let formatted = time.format(parsed, time.RFC3339, "UTC");
let later = time.add(parsed, "2s");
let duration = time.parseDuration("1.5s");
let direct = time.duration("250ms");
let fromUnix = time.unix(1, 500000000);
let fromMs = time.unixMs(0);
let before = time.nowMs();
time.sleep(1);
let after = time.nowMs();
println(
  "time-enhanced=",
  formatted,
  ":",
  later.toISOString(),
  ":",
  duration.milliseconds,
  ":",
  duration.microseconds,
  ":",
  duration.string,
  ":",
  direct.milliseconds,
  ":",
  fromUnix.toISOString(),
  ":",
  fromMs.toISOString(),
  ":",
  after >= before,
  ":",
  time.format(parsed, time.DateOnly)
);
"#,
    )
    .expect("write script");

    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&script)
        .output()
        .expect("run enhanced time script");

    assert!(
        output.status.success(),
        "enhanced time script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "time-enhanced=2020-01-02T03:04:05Z:2020-01-02T03:04:07.000Z:1500:1500000:1500ms:250:1970-01-01T00:00:01.500Z:1970-01-01T00:00:00.000Z:true:2020-01-02\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_check_types_returns_not_implemented_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg("--check-types")
        .output()
        .expect("run gs --check-types");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not implemented"));
}

#[test]
fn cli_run_and_direct_main_call_top_level_main() {
    let dir = unique_temp_dir("gts-cli-main-call");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("main.gs");
    fs::write(&script, "function main() { println(\"main-called\"); }\n").expect("write script");

    let direct = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&script)
        .output()
        .expect("run direct main.gs");
    assert!(
        direct.status.success(),
        "direct main failed: {}",
        String::from_utf8_lossy(&direct.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&direct.stdout), "main-called\n");

    let project = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("run project main");
    assert!(
        project.status.success(),
        "project main failed: {}",
        String::from_utf8_lossy(&project.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&project.stdout), "main-called\n");

    let _ = fs::remove_dir_all(dir);
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}-{}", std::process::id()))
}
