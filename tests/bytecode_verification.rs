use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use gts::object::{Object, EXEC_MODE_BYTECODE};
use gts::runtime::Session;

fn verification_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gts_r has a workspace parent")
        .join("gts")
        .join("verification")
}

fn run_script(script: &Path) {
    eprintln!("bytecode verification: {}", script.display());
    let session = Session::new();
    session
        .vm()
        .exec_mode
        .store(EXEC_MODE_BYTECODE, Ordering::Relaxed);

    let result = session
        .run_file(script, Vec::new())
        .unwrap_or_else(|err| panic!("{} failed: {}", script.display(), err.inspect()));
    assert!(
        !matches!(result, Object::Error(_)),
        "{} returned runtime error: {}",
        script.display(),
        result.inspect()
    );
}

#[test]
fn bytecode_vm_runs_verification_basics_and_async() {
    let root = verification_root();
    let scripts = [
        "01_basics/01_variables.gs",
        "01_basics/02_operators.gs",
        "01_basics/03_control_flow.gs",
        "01_basics/04_functions.gs",
        "01_basics/05_closures.gs",
        "01_basics/06_arrays.gs",
        "01_basics/07_objects.gs",
        "01_basics/08_classes.gs",
        "01_basics/09_errors.gs",
        "01_basics/10_typeof.gs",
        "02_async/11_promises.gs",
        "02_async/12_async_await.gs",
    ];

    for script in scripts {
        run_script(&root.join(script));
    }
}
