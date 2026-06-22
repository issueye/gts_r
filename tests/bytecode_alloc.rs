//! Allocation smoke test for the stage-2 control-flow hot path.
//!
//! This is ignored by default because it runs a 1,000,000-iteration loop and
//! uses a counting global allocator. Run it explicitly when updating
//! `docs/bytecode-vm-todo.md` stage 2.7.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use gts::bytecode::{compile, interpret};
use gts::evaluator::{builtins::register_globals, eval_program};
use gts::lexer::Lexer;
use gts::object::{Environment, Object, VirtualMachine};
use gts::parser::Parser;

struct CountingAlloc;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNTING.load(Ordering::Relaxed) {
            DEALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

#[derive(Debug, Clone, Copy)]
struct Measure {
    allocs: usize,
    deallocs: usize,
    elapsed: Duration,
}

fn parse(src: &str) -> gts::ast::Program {
    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, "alloc_hot_loop.gs");
    let program = parser.parse_program();
    assert!(
        program.errors.is_empty(),
        "parse errors: {:?}",
        program.errors
    );
    program
}

fn measure(mut f: impl FnMut() -> Object) -> Measure {
    ALLOCS.store(0, Ordering::Relaxed);
    DEALLOCS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::SeqCst);
    let start = Instant::now();
    let result = std::hint::black_box(f());
    let elapsed = start.elapsed();
    COUNTING.store(false, Ordering::SeqCst);
    assert!(
        !result.is_runtime_error(),
        "unexpected runtime error: {}",
        result.inspect()
    );
    Measure {
        allocs: ALLOCS.load(Ordering::Relaxed),
        deallocs: DEALLOCS.load(Ordering::Relaxed),
        elapsed,
    }
}

fn fresh_env() -> gts::object::EnvRef {
    let vm = VirtualMachine::new();
    register_globals(&vm);
    Environment::new_root(vm)
}

#[test]
#[ignore = "explicit stage-2 allocation evidence; run with --ignored --nocapture"]
fn million_empty_for_loop_allocates_far_less_on_bytecode_vm() {
    let src = r#"
let i = 0;
for (; i < 1000000; i = i + 1) {
}
i;
"#;
    let program = parse(src);
    let chunk = compile(&program).expect("bytecode compile");

    let tree_env = fresh_env();
    let vm_env = fresh_env();

    let tree = measure(|| eval_program(&program, &tree_env));
    let vm = measure(|| interpret(&chunk, &vm_env));

    println!(
        "stage2_hot_loop_allocations tree_walk={} bytecode_vm={} ratio={:.1}x tree_elapsed_ms={} vm_elapsed_ms={} tree_deallocs={} vm_deallocs={}",
        tree.allocs,
        vm.allocs,
        tree.allocs as f64 / vm.allocs.max(1) as f64,
        tree.elapsed.as_millis(),
        vm.elapsed.as_millis(),
        tree.deallocs,
        vm.deallocs
    );

    assert_eq!(
        vm_env.borrow().get("i").map(|v| v.inspect()),
        Some("1000000".to_string())
    );
    assert!(
        vm.allocs.saturating_mul(10) < tree.allocs,
        "expected bytecode VM to allocate at least 10x less; tree={:?}, vm={:?}",
        tree,
        vm
    );
}
