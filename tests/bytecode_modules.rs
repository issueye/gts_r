use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use gts::object::{Builtin, Object, EXEC_MODE_BYTECODE};
use gts::runtime::Session;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}-{}", std::process::id()))
}

fn parity_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity")
}

fn run_fixture_capturing(dir: &str) -> String {
    let captured: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let captured_fn = captured.clone();
    let captured_print = captured.clone();

    let session = Session::new();
    session
        .vm()
        .exec_mode
        .store(EXEC_MODE_BYTECODE, Ordering::Relaxed);

    let println_builtin = Builtin {
        name: "println".into(),
        func: Rc::new(move |_ctx, args: &[Object]| {
            let parts: Vec<String> = args.iter().map(|a| a.inspect()).collect();
            captured_fn.borrow_mut().push_str(&parts.join(""));
            captured_fn.borrow_mut().push('\n');
            Object::Undefined
        }),
        extra: None,
    };
    session
        .vm()
        .set_global("println", Object::Builtin(Rc::new(println_builtin)));

    let print_builtin = Builtin {
        name: "print".into(),
        func: Rc::new(move |_ctx, args: &[Object]| {
            for arg in args {
                captured_print.borrow_mut().push_str(&arg.inspect());
            }
            Object::Undefined
        }),
        extra: None,
    };
    session
        .vm()
        .set_global("print", Object::Builtin(Rc::new(print_builtin)));

    let path = parity_root().join(dir).join("main.gs");
    let result = session
        .run_file(path, Vec::new())
        .unwrap_or_else(|err| panic!("{dir} should run under bytecode VM: {}", err.inspect()));
    assert!(
        !result.is_runtime_error(),
        "{dir} returned runtime error: {}",
        result.inspect()
    );
    let out = captured.borrow().clone();
    out
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

#[test]
fn bytecode_vm_matches_stage_8_module_fixtures() {
    let fixtures = [
        ("relative_require", "relative-require=18\n"),
        ("nested_relative_require", "nested-relative-require=21\n"),
        ("project_module_require", "project-module-require=42\n"),
        ("directory_module_index", "directory-module-index=42\n"),
        ("module_cache", "module-cache=1:1\n"),
        ("module_exports_object", "module-exports-object=42\n"),
        ("import_default_like", "import-default-like=12\n"),
        ("export_const", "export-const=export:42\n"),
        ("export_function_alias", "export-function-alias=18\n"),
    ];

    for (dir, expected) in fixtures {
        let out = run_fixture_capturing(dir);
        assert_eq!(
            out, expected,
            "fixture `{dir}` output mismatch under bytecode VM"
        );
    }
}
