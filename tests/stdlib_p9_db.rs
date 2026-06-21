//! Parity tests for the @std/db module (sqlite backend). Each test creates a
//! throwaway on-disk sqlite database under a temp dir.

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
fn db_drivers_lists_sqlite() {
    let dir = unique_temp_dir("gts-p9-db-list");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let db = require("@std/db");
println(db.drivers[0]);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "sqlite");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn db_create_insert_query_roundtrip() {
    let dir = unique_temp_dir("gts-p9-db-crud");
    fs::create_dir_all(&dir).expect("create temp dir");
    let db_path = dir.join("test.sqlite");
    let db_path_str = db_path.to_string_lossy().replace('\\', "/");
    let script = format!(
        r#"
let db = require("@std/db");
let conn = db.open("sqlite", "{0}");
let res = conn.exec("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT)");
conn.exec("INSERT INTO notes (body) VALUES (?)", ["hello"]);
conn.exec("INSERT INTO notes (body) VALUES (?)", ["world"]);
let rows = conn.query("SELECT id, body FROM notes ORDER BY id");
println(rows.length);
println(rows[0].body);
println(rows[1].body);
println(conn.ping());
conn.close();
"#,
        db_path_str
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "2");
    assert_eq!(lines[1], "hello");
    assert_eq!(lines[2], "world");
    assert_eq!(lines[3], "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn db_query_one_returns_single_row_or_null() {
    let dir = unique_temp_dir("gts-p9-db-one");
    fs::create_dir_all(&dir).expect("create temp dir");
    let db_path = dir.join("one.sqlite");
    let db_path_str = db_path.to_string_lossy().replace('\\', "/");
    let script = format!(
        r#"
let db = require("@std/db");
let conn = db.open("sqlite", "{0}");
conn.exec("CREATE TABLE kv (k TEXT, v INTEGER)");
conn.exec("INSERT INTO kv VALUES (?, ?)", ["a", 1]);
let hit = conn.queryOne("SELECT k, v FROM kv WHERE k = ?", ["a"]);
println(hit.v);
let miss = conn.queryOne("SELECT k, v FROM kv WHERE k = ?", ["zzz"]);
println(miss);
"#,
        db_path_str
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "null");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn db_transaction_commit_and_rollback() {
    let dir = unique_temp_dir("gts-p9-db-tx");
    fs::create_dir_all(&dir).expect("create temp dir");
    let db_path = dir.join("tx.sqlite");
    let db_path_str = db_path.to_string_lossy().replace('\\', "/");
    let script = format!(
        r#"
let db = require("@std/db");
let conn = db.open("sqlite", "{0}");
conn.exec("CREATE TABLE t (n INTEGER)");
let tx = conn.begin();
tx.exec("INSERT INTO t VALUES (1)");
tx.commit();
println(conn.queryOne("SELECT COUNT(*) AS c FROM t").c);

let tx2 = conn.begin();
tx2.exec("INSERT INTO t VALUES (2)");
tx2.rollback();
println(conn.queryOne("SELECT COUNT(*) AS c FROM t").c);
"#,
        db_path_str
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "1");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn db_unsupported_driver_returns_error() {
    let dir = unique_temp_dir("gts-p9-db-bad");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let db = require("@std/db");
try {
    db.open("postgres", "host=localhost");
    println("opened");
} catch (e) {
    println("errored");
}
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "errored");
    let _ = fs::remove_dir_all(dir);
}
