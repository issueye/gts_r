//! Parity tests for the @std/runtime module: spawning isolated sub-scripts and
//! inspecting their exports or invoking named exports.

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
fn runtime_run_script_returns_exports() {
    let dir = unique_temp_dir("gts-p9-rt-export");
    fs::create_dir_all(&dir).expect("create temp dir");
    // Child script defines module.exports.value; parent runs it and prints.
    let child_path = dir.join("child.gs");
    fs::write(&child_path, "module.exports.value = 42;\n").expect("write child");
    let child_path_str = child_path.to_string_lossy().replace('\\', "/");
    let script = format!(
        r#"
let runtime = require("@std/runtime");
let child = runtime.runScript("{0}");
println(child.value);
"#,
        child_path_str
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "42");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn runtime_call_script_invokes_named_export() {
    let dir = unique_temp_dir("gts-p9-rt-call");
    fs::create_dir_all(&dir).expect("create temp dir");
    let child_path = dir.join("calc.gs");
    fs::write(
        &child_path,
        "module.exports.add = function(a, b) { return a + b; };\n",
    )
    .expect("write child");
    let child_path_str = child_path.to_string_lossy().replace('\\', "/");
    let script = format!(
        r#"
let runtime = require("@std/runtime");
let sum = runtime.callScript("{0}", "add", [3, 4]);
println(sum);
"#,
        child_path_str
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "7");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn runtime_run_tool_calls_run_export() {
    let dir = unique_temp_dir("gts-p9-rt-tool");
    fs::create_dir_all(&dir).expect("create temp dir");
    let child_path = dir.join("tool.gs");
    fs::write(
        &child_path,
        "module.exports.run = function(input) { return \"echo:\" + input; };\n",
    )
    .expect("write child");
    let child_path_str = child_path.to_string_lossy().replace('\\', "/");
    let script = format!(
        r#"
let runtime = require("@std/runtime");
let result = runtime.runTool("{0}", "hello");
println(result);
"#,
        child_path_str
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "echo:hello");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn runtime_call_script_missing_export_errors() {
    let dir = unique_temp_dir("gts-p9-rt-missing");
    fs::create_dir_all(&dir).expect("create temp dir");
    let child_path = dir.join("empty.gs");
    fs::write(&child_path, "let x = 1;\n").expect("write child");
    let child_path_str = child_path.to_string_lossy().replace('\\', "/");
    // Use .replace (not format!) so the GTS braces aren't parsed as placeholders.
    let script = r#"
let runtime = require("@std/runtime");
try {
    runtime.callScript("__CHILD_PATH__", "nope", []);
    println("ok");
} catch (e) {
    println("errored");
}
"#
    .replace("__CHILD_PATH__", &child_path_str);
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "errored");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn runtime_missing_file_errors() {
    let dir = unique_temp_dir("gts-p9-rt-nofile");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let runtime = require("@std/runtime");
try {
    runtime.runScript("definitely-does-not-exist.gs");
    println("ok");
} catch (e) {
    println("errored");
}
"#;
    let output = run_script(&dir, script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "errored");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn image_and_pdf_placeholders_return_descriptive_error() {
    let dir = unique_temp_dir("gts-p9-rt-img");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let image = require("@std/image");
let pdf = require("@std/pdf");
try { image.info("x.png"); println("img-ok"); } catch (e) { println("img-err"); }
try { pdf.info("x.pdf"); println("pdf-ok"); } catch (e) { println("pdf-err"); }
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    // Both modules are aligned placeholders that surface an error.
    assert_eq!(lines[0], "img-err");
    assert_eq!(lines[1], "pdf-err");
    let _ = fs::remove_dir_all(dir);
}
