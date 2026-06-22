use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use gts::object::EXEC_MODE_BYTECODE;
use gts::runtime::Session;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}-{}", std::process::id()))
}

#[test]
fn bytecode_runtime_reuses_module_cache_for_circular_require() {
    let dir = unique_temp_dir("gts-bytecode-circular-module");
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
    session
        .vm()
        .exec_mode
        .store(EXEC_MODE_BYTECODE, Ordering::Relaxed);
    let result = session
        .run_file(dir.join("main.gs"), Vec::new())
        .expect("script should run");

    assert_eq!(result.inspect(), "loading:done");
    let _ = fs::remove_dir_all(dir);
}
