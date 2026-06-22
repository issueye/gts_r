//! Explicit bytecode VM performance gate for `bench/scripts/bench_server.gs`.
//!
//! This test is ignored by default because it runs timing-sensitive workloads
//! several times. Run it when updating `docs/bytecode-vm-todo.md` stage 10.4:
//!
//! `cargo test --release --test bytecode_perf -- --ignored --nocapture`

use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gts::object::{Builtin, Object, EXEC_MODE_BYTECODE, EXEC_MODE_TREEWALK};
use gts::runtime::Session;

#[derive(Clone, Copy, Debug)]
struct Case {
    name: &'static str,
    iterations: usize,
    max_bytecode_over_tree: f64,
}

#[derive(Clone, Debug)]
struct Sample {
    elapsed: Duration,
    output: String,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_case(case: Case, exec_mode: u8) -> Sample {
    let source_path = repo_root().join("bench/scripts/bench_server.gs");
    let source = std::fs::read_to_string(&source_path).expect("read benchmark script");

    env::set_var("GTS_BYTECODE_BENCH", case.name);
    env::set_var("GTS_BYTECODE_BENCH_ITERS", case.iterations.to_string());

    let captured = Rc::new(RefCell::new(String::new()));
    let captured_print = captured.clone();
    let captured_println = captured.clone();

    let session = Session::new();
    session.vm().exec_mode.store(exec_mode, Ordering::Relaxed);
    session.vm().set_global(
        "print",
        Object::Builtin(Rc::new(Builtin {
            name: "print".into(),
            func: Rc::new(move |_ctx, args: &[Object]| {
                for arg in args {
                    captured_print.borrow_mut().push_str(&arg.inspect());
                }
                Object::Undefined
            }),
            extra: None,
        })),
    );
    session.vm().set_global(
        "println",
        Object::Builtin(Rc::new(Builtin {
            name: "println".into(),
            func: Rc::new(move |_ctx, args: &[Object]| {
                let parts: Vec<String> = args.iter().map(|arg| arg.inspect()).collect();
                captured_println.borrow_mut().push_str(&parts.join(""));
                captured_println.borrow_mut().push('\n');
                Object::Undefined
            }),
            extra: None,
        })),
    );

    let start = Instant::now();
    let result = session
        .run_source(&source, &source_path)
        .unwrap_or_else(|err| panic!("{} should run: {}", case.name, err.inspect()));
    let elapsed = start.elapsed();
    assert!(
        !result.is_runtime_error(),
        "{} should not return runtime error: {}",
        case.name,
        result.inspect()
    );

    env::remove_var("GTS_BYTECODE_BENCH");
    env::remove_var("GTS_BYTECODE_BENCH_ITERS");

    let output = captured.borrow().clone();
    Sample { elapsed, output }
}

fn median(samples: &mut [Sample]) -> Sample {
    samples.sort_by_key(|sample| sample.elapsed);
    samples[samples.len() / 2].clone()
}

fn measured_median(case: Case, exec_mode: u8) -> Sample {
    let _ = run_case(case, exec_mode);
    let mut samples: Vec<Sample> = (0..3).map(|_| run_case(case, exec_mode)).collect();
    median(&mut samples)
}

#[test]
#[ignore = "explicit stage-10 performance gate; run with --ignored --nocapture"]
fn bench_server_bytecode_vm_is_not_slower_than_treewalk() {
    if cfg!(debug_assertions) {
        eprintln!("stage10_bench_server skipped: performance gate requires --release");
        return;
    }

    let cases = [
        Case {
            name: "fib",
            iterations: 5000,
            max_bytecode_over_tree: 1.05,
        },
        Case {
            name: "string_concat",
            iterations: 5000,
            max_bytecode_over_tree: 1.05,
        },
        Case {
            name: "promise_create",
            iterations: 5000,
            max_bytecode_over_tree: 1.05,
        },
    ];

    for case in cases {
        let tree = measured_median(case, EXEC_MODE_TREEWALK);
        let bytecode = measured_median(case, EXEC_MODE_BYTECODE);

        assert_eq!(
            bytecode.output, tree.output,
            "{}: output mismatch",
            case.name
        );

        let tree_ms = tree.elapsed.as_secs_f64() * 1000.0;
        let bytecode_ms = bytecode.elapsed.as_secs_f64() * 1000.0;
        let ratio = bytecode_ms / tree_ms.max(0.001);
        println!(
            "stage10_bench_server case={} iterations={} tree_ms={:.3} bytecode_ms={:.3} ratio={:.3}x output={}",
            case.name,
            case.iterations,
            tree_ms,
            bytecode_ms,
            ratio,
            bytecode.output.trim()
        );

        assert!(
            ratio <= case.max_bytecode_over_tree,
            "{} bytecode VM slower than treewalk: tree={:.3}ms bytecode={:.3}ms ratio={:.3}x threshold={:.3}x",
            case.name,
            tree_ms,
            bytecode_ms,
            ratio,
            case.max_bytecode_over_tree
        );
    }
}
