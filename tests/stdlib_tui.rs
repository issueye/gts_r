//! Focused parity tests for the @std/tui module.

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
fn tui_constructs_messages_and_layouts() {
    let dir = unique_temp_dir("gts-tui-layout");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let tui = require("@std/tui");
let key = tui.key("ctrl+c", "\x03");
let text = tui.text("hello");
let resize = tui.resize(12, 3);
println(key.type + ":" + key.key + ":" + key.raw);
println(text.type + ":" + text.text + ":" + text.raw);
println(resize.type + ":" + String(resize.cols) + ":" + String(resize.rows) + ":" + String(resize.stable));

let boxA = tui.box("A", { width: 7, title: "one" });
let boxB = tui.box("B", { width: 5 });
let row = tui.row(boxA, boxB);
println(row.indexOf("one") >= 0);
println(row.indexOf("A") >= 0);
println(row.indexOf("B") >= 0);

let status = tui.statusBar({ left: "L", center: "C", right: "R" }, 9);
println(status.length);
println(tui.stripAnsi(tui.style("ok", { fg: "accent", bold: true })));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "key:ctrl+c:\x03");
    assert_eq!(lines[1], "text:hello:hello");
    assert_eq!(lines[2], "resize:12:3:true");
    assert_eq!(lines[3], "true");
    assert_eq!(lines[4], "true");
    assert_eq!(lines[5], "true");
    assert_eq!(lines[6], "9");
    assert_eq!(lines[7], "ok");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn tui_input_renders_cursor_and_respects_width() {
    let dir = unique_temp_dir("gts-tui-input");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let tui = require("@std/tui");
let rendered = tui.input({
  width: 18,
  title: "Prompt",
  value: "hello",
  cursor: 2,
  meta: "meta",
});
println(tui.stripAnsi(rendered).indexOf("Prompt") >= 0);
println(rendered.indexOf("\x1b[7m \x1b[0m") >= 0);
let lines = rendered.split("\n");
let ok = true;
for (let i = 0; i < lines.length; i = i + 1) {
  if (tui.width(lines[i]) > 18) {
    ok = false;
  }
}
println(ok);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "true\ntrue\ntrue\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn tui_app_dispatches_and_renders_script_state() {
    let dir = unique_temp_dir("gts-tui-app");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let tui = require("@std/tui");
let app = tui.createApp({
  init: function(size) {
    return { count: 0, cols: size.cols };
  },
  update: function(state, msg) {
    if (msg.type === "text") {
      state.count = state.count + 1;
    }
    if (msg.type === "key" && msg.key === "ctrl+c") {
      return { state: state, quit: true };
    }
    return state;
  },
  view: function(state, size) {
    return "count=" + String(state.count) + " cols=" + String(size.cols);
  },
});
app.dispatch(tui.text("a"));
app.dispatch(tui.text("b"));
println(app.render({ cols: 12, rows: 3 }));
app.dispatch(tui.key("ctrl+c"));
println(app.state().count);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "count=2 cols=12\n2\n");
    let _ = fs::remove_dir_all(dir);
}
