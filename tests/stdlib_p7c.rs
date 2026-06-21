//! Parity tests for the P7 batch-3 stdlib modules: toml, yaml, xml, markdown,
//! schema, test, archive/zip. Each test writes a `.gs` program to a temp dir,
//! runs the `gs` binary, and asserts on stdout/stderr/exit.

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

// --- toml ------------------------------------------------------------------

#[test]
fn toml_parse_and_stringify_roundtrip() {
    let dir = unique_temp_dir("gts-p7c-toml");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let toml = require("@std/toml");
let parsed = toml.parse('title = "TOML Example"\n[owner]\nname = "Ada"\nage = 36\n');
println(parsed.title);
println(parsed.owner.name);
println(parsed.owner.age);
let out = toml.stringify({name: "Z", count: 7});
println(out.indexOf("name = \"Z\"") >= 0);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "TOML Example");
    assert_eq!(lines[1], "Ada");
    assert_eq!(lines[2], "36");
    assert_eq!(lines[3], "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn toml_parse_invalid_reports_error() {
    let dir = unique_temp_dir("gts-p7c-toml-err");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let toml = require("@std/toml");
println(toml.parse("not = valid = toml"));
"#,
    );
    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains("toml.parse:"),
        "got stderr: {}",
        stderr_of(&output)
    );
    let _ = fs::remove_dir_all(dir);
}

// --- yaml ------------------------------------------------------------------

#[test]
fn yaml_parse_handles_scalars_arrays_and_maps() {
    let dir = unique_temp_dir("gts-p7c-yaml");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let yaml = require("@std/yaml");
let doc = yaml.parse("name: Ada\nage: 36\ntags:\n  - a\n  - b\n");
println(doc.name);
println(doc.age);
println(doc.tags[0], ":", doc.tags[1]);
let out = yaml.stringify({x: 1, y: "two"});
println(out.indexOf("x: 1") >= 0);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "Ada");
    assert_eq!(lines[1], "36");
    assert_eq!(lines[2], "a:b");
    assert_eq!(lines[3], "true");
    let _ = fs::remove_dir_all(dir);
}

// --- xml -------------------------------------------------------------------

#[test]
fn xml_parse_builds_dom_tree() {
    let dir = unique_temp_dir("gts-p7c-xml-parse");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let xml = require("@std/xml");
let doc = xml.parse('<book lang="en"><title>GoScript</title><author id="1">Ada</author></book>');
println(doc.name);
println(doc.attributes.lang);
println(doc.children[0].name);
println(doc.children[0].text);
println(doc.children[1].attributes.id);
println(doc.children[1].text);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "book");
    assert_eq!(lines[1], "en");
    assert_eq!(lines[2], "title");
    assert_eq!(lines[3], "GoScript");
    assert_eq!(lines[4], "1");
    assert_eq!(lines[5], "Ada");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn xml_stringify_roundtrips_dom() {
    let dir = unique_temp_dir("gts-p7c-xml-stringify");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let xml = require("@std/xml");
let out = xml.stringify({
  name: "root",
  attributes: { a: "1" },
  text: "hi",
  children: [{ name: "child", text: "c" }]
});
println(out);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    assert_eq!(stdout.trim(), "<root a=\"1\">hi<child>c</child></root>");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn xml_empty_self_closes() {
    let dir = unique_temp_dir("gts-p7c-xml-empty");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let xml = require("@std/xml");
println(xml.stringify({ name: "br" }));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "<br/>");
    let _ = fs::remove_dir_all(dir);
}

// --- markdown --------------------------------------------------------------

#[test]
fn markdown_render_terminal_extracts_headings() {
    let dir = unique_temp_dir("gts-p7c-md");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r##"
let md = require("@std/markdown");
let snap = md.renderTerminal("# Title\n- item one\n- item two\n");
println(snap.headings[0]);
"##,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "Title");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn markdown_from_html_strips_tags() {
    let dir = unique_temp_dir("gts-p7c-md-html");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let md = require("@std/markdown");
let text = md.fromHTML("<h1>Hi</h1><p>Hello <b>world</b></p>");
println(text.indexOf("Hi") >= 0);
println(text.indexOf("world") >= 0);
println(text.indexOf("<") < 0);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "true");
    assert_eq!(lines[2], "true");
    let _ = fs::remove_dir_all(dir);
}

// --- schema ----------------------------------------------------------------

#[test]
fn schema_validate_accepts_valid_and_reports_invalid() {
    let dir = unique_temp_dir("gts-p7c-schema");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let schema = require("@std/schema");
let s = {
  type: "object",
  required: ["name", "age"],
  properties: {
    name: { type: "string", minLength: 1 },
    age: { type: "integer", minimum: 0 }
  }
};
let ok = schema.validate(s, { name: "Ada", age: 36 });
println(ok.valid);
println(ok.errors.length);
let bad = schema.validate(s, { name: "", age: -1 });
println(bad.valid);
println(bad.errors.length >= 2);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "0");
    assert_eq!(lines[2], "false");
    assert_eq!(lines[3], "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn schema_assert_returns_value_or_throws() {
    let dir = unique_temp_dir("gts-p7c-schema-assert");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let schema = require("@std/schema");
let s = { type: "string", minLength: 3 };
let v = schema.assert(s, "hello");
println(v);
println(schema.assert(s, "no"));
"#,
    );
    assert!(!output.status.success());
    let stdout = stdout_of(&output);
    // The first assert succeeds and prints "hello"; the second throws.
    assert_eq!(stdout.trim(), "hello");
    let _ = fs::remove_dir_all(dir);
}

// --- test ------------------------------------------------------------------

#[test]
fn test_runner_reports_passing_and_failing() {
    let dir = unique_temp_dir("gts-p7c-test");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let t = require("@std/test");
t.test("passing case", function() {
  t.expect(1 + 1).toBe(2);
});
let result = t.run();
println(result.total);
println(result.passed);
println(result.failed);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "1");
    assert_eq!(lines[2], "0");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_expect_truthy_and_falsy() {
    let dir = unique_temp_dir("gts-p7c-test-expect");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let t = require("@std/test");
t.test("truthy", function() {
  t.expect("x").toBeTruthy();
  t.expect(0).toBeFalsy();
});
let result = t.run();
println(result.passed);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "1");
    let _ = fs::remove_dir_all(dir);
}

// --- archive/zip -----------------------------------------------------------

#[test]
fn zip_create_list_and_extract_roundtrip() {
    let dir = unique_temp_dir("gts-p7c-zip");
    fs::create_dir_all(&dir).expect("create temp dir");
    let src = dir.join("hello.txt");
    fs::write(&src, "zip-content\n").expect("write src");
    let archive = dir.join("out.zip");
    let extract_dir = dir.join("extracted");
    let archive_str = archive.to_string_lossy().replace('\\', "\\\\");
    let src_str = src.to_string_lossy().replace('\\', "\\\\");
    let extract_str = extract_dir.to_string_lossy().replace('\\', "\\\\");
    let script = format!(
        r#"
let zip = require("@std/archive/zip");
zip.create([{{ path: "{src_str}", name: "hello.txt" }}], "{archive_str}");
let entries = zip.list("{archive_str}");
println(entries.length);
println(entries[0].name);
zip.extract("{archive_str}", "{extract_str}");
"#
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "hello.txt");
    let extracted = extract_dir.join("hello.txt");
    assert_eq!(
        fs::read_to_string(&extracted).unwrap_or_default(),
        "zip-content\n"
    );
    let _ = fs::remove_dir_all(dir);
}
