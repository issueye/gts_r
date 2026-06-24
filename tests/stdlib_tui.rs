//! Integration tests for the redesigned @std/tui module.
//!
//! These run scripts via the `gs` binary (non-TTY subprocess), which exercises
//! the node-tree → flexbox → render pipeline through `app.render` (the
//! non-interactive single-frame path) rather than `app.run`'s event loop.

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

/// Strip ANSI CSI/OSC escape sequences for layout-content comparisons.
fn strip_ansi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        let start = i;
        i += 1;
        while i < bytes.len() && (bytes[i] & 0xc0) == 0x80 {
            i += 1;
        }
        if let Ok(ch) = std::str::from_utf8(&bytes[start..i]) {
            out.push_str(ch);
        }
    }
    out
}

// --- node constructors return markers -------------------------------------

#[test]
fn text_node_is_a_tui_node_marker() {
    let dir = unique_temp_dir("gts-tui-text-node");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let tui = require("@std/tui");
let n = tui.text("hi");
println(typeof n);
println(n.__kind);
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "object");
    assert_eq!(lines[1], "tuiNode");
    let _ = fs::remove_dir_all(dir);
}

// --- app.render produces a text frame from a node tree --------------------

#[test]
fn app_render_renders_text_node() {
    let dir = unique_temp_dir("gts-tui-render-text");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let tui = require("@std/tui");
let app = tui.createApp({
  state: "Hello",
  view: (state, size) => tui.text(state),
});
let frame = app.render({cols: 10, rows: 1});
println(frame);
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    // "Hello" left-aligned, padded to 10 cols on a single line (no trim: the
    // trailing spaces are part of the frame).
    assert_eq!(out, "Hello     \n");
    let _ = fs::remove_dir_all(dir);
}

// --- flexbox: row lays children side by side ------------------------------

#[test]
fn row_layout_places_children_horizontally() {
    let dir = unique_temp_dir("gts-tui-row");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let tui = require("@std/tui");
let app = tui.createApp({
  state: null,
  view: (_s, _size) => tui.row({
    children: [tui.text("AB"), tui.text("CD")],
  }),
});
let frame = app.render({cols: 8, rows: 1});
println(frame);
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    assert_eq!(out, "ABCD    \n");
    let _ = fs::remove_dir_all(dir);
}

// --- flexbox: column stacks children vertically ---------------------------

#[test]
fn column_layout_stacks_children_vertically() {
    let dir = unique_temp_dir("gts-tui-column");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let tui = require("@std/tui");
let app = tui.createApp({
  state: null,
  view: (_s, _size) => tui.column({
    children: [tui.text("A"), tui.text("B")],
  }),
});
let frame = app.render({cols: 3, rows: 2});
println("FRAME");
println(frame);
println("END");
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    let lines: Vec<&str> = out.lines().collect();
    let frame_start = lines.iter().position(|l| *l == "FRAME").unwrap() + 1;
    assert_eq!(lines[frame_start], "A  ");
    assert_eq!(lines[frame_start + 1], "B  ");
    let _ = fs::remove_dir_all(dir);
}

// --- box with border draws box-drawing characters -------------------------

#[test]
fn box_with_border_renders_frame() {
    let dir = unique_temp_dir("gts-tui-border");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let tui = require("@std/tui");
let app = tui.createApp({
  state: null,
  view: (_s, _size) => tui.box({
    width: 5, height: 3, border: true,
    children: [tui.text("X")],
  }),
});
let frame = app.render({cols: 5, rows: 3});
println("FRAME");
println(frame);
println("END");
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    let lines: Vec<&str> = out.lines().collect();
    let frame_start = lines.iter().position(|l| *l == "FRAME").unwrap() + 1;
    assert_eq!(lines[frame_start], "┌───┐");
    assert_eq!(lines[frame_start + 1], "│X  │");
    assert_eq!(lines[frame_start + 2], "└───┘");
    let _ = fs::remove_dir_all(dir);
}

// --- progress bar renders filled/empty cells ------------------------------

#[test]
fn progress_bar_renders_half_filled() {
    let dir = unique_temp_dir("gts-tui-progress");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let tui = require("@std/tui");
let app = tui.createApp({
  state: null,
  view: (_s, _size) => tui.progress({ value: 50, total: 100, width: 8 }),
});
let frame = app.render({cols: 8, rows: 1});
println(frame);
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    assert_eq!(out.trim_end(), "[███░░░]");
    let _ = fs::remove_dir_all(dir);
}

// --- checkbox renders checked/unchecked markers ---------------------------

#[test]
fn checkbox_renders_checked_and_unchecked() {
    let dir = unique_temp_dir("gts-tui-checkbox");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let tui = require("@std/tui");
let app = tui.createApp({
  state: null,
  view: (_s, _size) => tui.column({
    children: [
      tui.checkbox({ checked: true, label: "done" }),
      tui.checkbox({ checked: false, label: "todo" }),
    ],
  }),
});
let frame = app.render({cols: 12, rows: 2});
println("FRAME");
println(frame);
println("END");
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    let lines: Vec<&str> = out.lines().collect();
    let frame_start = lines.iter().position(|l| *l == "FRAME").unwrap() + 1;
    // Compare ANSI-stripped: styling may place reset codes in padding cells.
    assert_eq!(strip_ansi(&lines[frame_start]).trim_end(), "[x] done");
    assert_eq!(strip_ansi(&lines[frame_start + 1]).trim_end(), "[ ] todo");
    let _ = fs::remove_dir_all(dir);
}

// --- list renders selection marker ----------------------------------------

#[test]
fn list_renders_selected_marker() {
    let dir = unique_temp_dir("gts-tui-list");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let tui = require("@std/tui");
let app = tui.createApp({
  state: null,
  view: (_s, _size) => tui.list({ items: ["alpha", "beta"], selected: 1, focused: false }),
});
let frame = app.render({cols: 10, rows: 2});
println("FRAME");
println(frame);
println("END");
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    let lines: Vec<&str> = out.lines().collect();
    let frame_start = lines.iter().position(|l| *l == "FRAME").unwrap() + 1;
    assert_eq!(strip_ansi(&lines[frame_start]).trim_end(), "  alpha");
    // selected item (index 1) is bolded; marker "› ".
    let sel = strip_ansi(&lines[frame_start + 1]);
    assert!(sel.contains("›") && sel.contains("beta"), "got: {:?}", sel);
    let _ = fs::remove_dir_all(dir);
}

// --- Elm dispatch: update + quit ------------------------------------------

#[test]
fn app_dispatch_updates_state_and_quit() {
    let dir = unique_temp_dir("gts-tui-dispatch");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let tui = require("@std/tui");
let app = tui.createApp({
  init: () => 0,
  update: (state, msg) => {
    if (msg.type === "tick") {
      let next = state + 1;
      return { state: next, quit: next >= 3 };
    }
    return { state: state };
  },
  view: (state, _size) => tui.text(String(state)),
});
app.dispatch(tui.tick());
app.dispatch(tui.tick());
let before = app.state();
app.dispatch(tui.tick());
let after = app.state();
println(before);
println(after);
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "2");
    assert_eq!(lines[1], "3");
    let _ = fs::remove_dir_all(dir);
}

// --- terminal capabilities report real rawMode ----------------------------

#[test]
fn terminal_capabilities_report_raw_mode() {
    let dir = unique_temp_dir("gts-tui-termcaps");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let term = require("@std/terminal");
let caps = term.capabilities();
println(caps.rawMode);
"#;
    let output = run_script(&dir, script);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = stdout_of(&output);
    assert_eq!(out.trim_end(), "true");
    let _ = fs::remove_dir_all(dir);
}
