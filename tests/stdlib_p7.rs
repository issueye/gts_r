//! Focused tests for the P7 low-risk native stdlib modules:
//! `@std/glob`, `@std/color`, `@std/diff`, `@std/log`, `@std/table`,
//! and `@std/validation`.

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
fn glob_matches_files_and_checks_patterns() {
    let dir = unique_temp_dir("gts-p7-glob");
    fs::create_dir_all(dir.join("src/nested")).expect("create temp dir");
    fs::write(dir.join("src/a.gs"), "").expect("write file");
    fs::write(dir.join("src/nested/b.gs"), "").expect("write file");
    fs::write(dir.join("src/readme.txt"), "").expect("write file");
    let pattern = dir
        .join("src")
        .join("*.gs")
        .to_string_lossy()
        .replace('\\', "\\\\");
    let script = format!(
        r#"
let glob = require("@std/glob");
let matches = glob.glob("{pattern}");
println(matches.length);
println(glob.match("src/*.gs", "src/a.gs"));
println(glob.hasMagic("src/*.gs"));
println(glob.hasMagic("src/a.gs"));
"#
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "2");
    assert_eq!(lines[1], "true");
    assert_eq!(lines[2], "true");
    assert_eq!(lines[3], "false");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn color_wraps_and_strips_ansi() {
    let dir = unique_temp_dir("gts-p7-color");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let color = require("@std/color");
let red = color.red("fail");
println(red.length > "fail".length);
println(color.strip(red));
println(color.stripAnsi(color.bold("ok")));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "true\nfail\nok\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn diff_reports_line_changes_and_unified_text() {
    let dir = unique_temp_dir("gts-p7-diff");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let diff = require("@std/diff");
let d = diff.lines("a\nb\nc", "a\nx\nc");
println(d.length);
println(d[0].kind, ":", d[1].kind, ":", d[2].kind);
let u = diff.unified("a\nb", "a\nc", "left", "right");
println(u[0], u[4], u[5]);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "4");
    assert_eq!(lines[1], "equal:remove:add");
    assert_eq!(lines[2], "-le");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn log_formats_levels_without_writing() {
    let dir = unique_temp_dir("gts-p7-log");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let log = require("@std/log");
println(log.format("info", "ready"));
println(log.warn("careful"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "[INFO] ready\n[WARN] careful\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn table_renders_arrays_and_objects() {
    let dir = unique_temp_dir("gts-p7-table");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let table = require("@std/table");
let rendered = table.render([{name:"Ada", age:36}, {name:"Lin", age:29}]);
println(rendered[0]);
println(rendered.indexOf("Ada") >= 0);
println(rendered.indexOf("age") >= 0);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "+");
    assert_eq!(lines[1], "true");
    assert_eq!(lines[2], "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn validation_checks_basic_rules_and_fields() {
    let dir = unique_temp_dir("gts-p7-validation");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let v = require("@std/validation");
println(v.required("x"));
println(v.type([1], "array"));
println(v.email("ada@example.com"));
println(v.min("abcd", 3));
println(v.max(5, 4));
let result = v.validate({name:"Ada", email:"ada@example.com", age:36}, {
  fields: {
    name: {required: true, type: "string", min: 2},
    email: {required: true, email: true},
    age: {type: "number", min: 18, max: 99}
  }
});
println(result.valid, ":", result.errors.length);
let bad = v.validate({name:"", email:"nope", age:12}, {
  fields: {
    name: {required: true, min: 2},
    email: {email: true},
    age: {min: 18}
  }
});
println(bad.valid, ":", bad.errors.length);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "true");
    assert_eq!(lines[2], "true");
    assert_eq!(lines[3], "true");
    assert_eq!(lines[4], "false");
    assert_eq!(lines[5], "true:0");
    assert_eq!(lines[6], "false:3");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn validation_validate_rejects_non_object_rules() {
    let dir = unique_temp_dir("gts-p7-validation-err");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let v = require("@std/validation");
println(v.validate("x", "required"));
"#,
    );
    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains("validation.validate: rules must be an object"),
        "got stderr: {}",
        stderr_of(&output)
    );
    let _ = fs::remove_dir_all(dir);
}
