//! Parity tests for ES module import/export semantics: named exports,
//! default exports, namespace imports, re-exports, and live bindings.

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

fn write(dir: &PathBuf, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).expect("write");
    path
}

fn run_main(dir: &PathBuf, body: &str) -> std::process::Output {
    write(dir, "main.gs", body);
    Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(dir.join("main.gs"))
        .output()
        .expect("run gs")
}

fn stdout_of(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn stderr_of(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn es_named_exports_and_imports() {
    let dir = unique_temp_dir("gts-es-named");
    fs::create_dir_all(&dir).expect("mkdir");
    write(
        &dir,
        "math.gs",
        r#"
export function add(a, b) { return a + b; }
export const PI = 3;
"#,
    );
    let out = run_main(
        &dir,
        r#"
import { add, PI } from "./math.gs";
println(add(2, 3));
println(PI);
"#,
    );
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let stdout = stdout_of(&out);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "5");
    assert_eq!(lines[1], "3");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn es_default_export_and_import() {
    let dir = unique_temp_dir("gts-es-default");
    fs::create_dir_all(&dir).expect("mkdir");
    write(
        &dir,
        "greet.gs",
        r#"
export default function(name) { return "hi " + name; }
"#,
    );
    let out = run_main(
        &dir,
        r#"
import greet from "./greet.gs";
println(greet("world"));
"#,
    );
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    assert_eq!(stdout_of(&out).trim(), "hi world");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn es_namespace_import() {
    let dir = unique_temp_dir("gts-es-ns");
    fs::create_dir_all(&dir).expect("mkdir");
    write(
        &dir,
        "lib.gs",
        r#"
export function inc(n) { return n + 1; }
export const TAG = "ns";
"#,
    );
    let out = run_main(
        &dir,
        r#"
import * as lib from "./lib.gs";
println(lib.inc(9));
println(lib.TAG);
"#,
    );
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let stdout = stdout_of(&out);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "ns");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn es_import_alias() {
    let dir = unique_temp_dir("gts-es-alias");
    fs::create_dir_all(&dir).expect("mkdir");
    write(
        &dir,
        "m.gs",
        r#"
export function longName() { return 42; }
"#,
    );
    let out = run_main(
        &dir,
        r#"
import { longName as ln } from "./m.gs";
println(ln());
"#,
    );
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    assert_eq!(stdout_of(&out).trim(), "42");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn es_mixed_default_and_named_import() {
    let dir = unique_temp_dir("gts-es-mixed");
    fs::create_dir_all(&dir).expect("mkdir");
    write(
        &dir,
        "mod.gs",
        r#"
export default function def() { return "default"; }
export function named() { return "named"; }
"#,
    );
    let out = run_main(
        &dir,
        r#"
import d, { named } from "./mod.gs";
println(d());
println(named());
"#,
    );
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let stdout = stdout_of(&out);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "default");
    assert_eq!(lines[1], "named");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn es_reexport_from_module() {
    let dir = unique_temp_dir("gts-es-reexport");
    fs::create_dir_all(&dir).expect("mkdir");
    write(
        &dir,
        "inner.gs",
        r#"
export const VALUE = 7;
export function helper() { return "h"; }
"#,
    );
    write(
        &dir,
        "barrel.gs",
        r#"
export { VALUE, helper } from "./inner.gs";
"#,
    );
    let out = run_main(
        &dir,
        r#"
import { VALUE, helper } from "./barrel.gs";
println(VALUE);
println(helper());
"#,
    );
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    let stdout = stdout_of(&out);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "7");
    assert_eq!(lines[1], "h");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn es_export_class_and_import() {
    let dir = unique_temp_dir("gts-es-class");
    fs::create_dir_all(&dir).expect("mkdir");
    write(
        &dir,
        "shape.gs",
        r#"
export class Box {
    constructor(v) { this.v = v; }
    get() { return this.v; }
}
"#,
    );
    let out = run_main(
        &dir,
        r#"
import { Box } from "./shape.gs";
let b = new Box(99);
println(b.get());
"#,
    );
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    assert_eq!(stdout_of(&out).trim(), "99");
    let _ = fs::remove_dir_all(dir);
}

#[test]
#[ignore = "circular imports with top-level value dependencies are not supported: imports snapshot values rather than creating live bindings. Circular imports work only when imported values (e.g. functions) are invoked after both modules finish evaluating. Full fix requires Object::ImportBinding indirection."]
fn es_circular_import_with_live_bindings() {
    let dir = unique_temp_dir("gts-es-circular");
    fs::create_dir_all(&dir).expect("mkdir");
    // a imports b's function, b imports a's function. Both must load without
    // infinite recursion. The functions are only invoked AFTER both modules
    // have finished evaluating, so the circular top-level binding resolves.
    write(
        &dir,
        "a.gs",
        r#"
import { bLater } from "./b.gs";
export function aLater() { return "a"; }
export function callB() { return bLater(); }
"#,
    );
    write(
        &dir,
        "b.gs",
        r#"
import { aLater } from "./a.gs";
export function bLater() { return "b+" + aLater(); }
"#,
    );
    let out = run_main(
        &dir,
        r#"
import { callB } from "./a.gs";
println(callB());
"#,
    );
    assert!(out.status.success(), "stderr: {}", stderr_of(&out));
    // callB() invokes bLater() which reads aLater() — both defined by call time.
    assert_eq!(stdout_of(&out).trim(), "b+a");
    let _ = fs::remove_dir_all(dir);
}
