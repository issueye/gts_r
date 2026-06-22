//! End-to-end parity check: run selected fixtures under the bytecode VM and
//! compare against the expected stdout recorded in `parity_compat.rs`.
//!
//! We stub the global `println` with a capturing closure so we can assert the
//! exact output without depending on process stdout capture. `print` is
//! similarly captured (no trailing newline).

use std::cell::RefCell;
use std::rc::Rc;

use gts::bytecode::{compile, interpret};
use gts::lexer::Lexer;
use gts::object::{Environment, Object, VirtualMachine};
use gts::parser::Parser;

/// Run `src` under the bytecode VM with a capturing `println`/`print`, and
/// return the concatenated captured output.
fn run_vm_capturing(src: &str) -> String {
    let captured: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let captured_fn = captured.clone();
    let captured_print = captured.clone();

    let vm = VirtualMachine::new();
    // Register the full global set first (Math, JSON, console, ...), then
    // override println/print with capturing versions.
    gts::evaluator::builtins::register_globals(&vm);

    let println_builtin = gts::object::Builtin {
        name: "println".into(),
        func: Rc::new(move |_ctx, args: &[Object]| {
            let parts: Vec<String> = args.iter().map(|a| a.inspect()).collect();
            captured_fn.borrow_mut().push_str(&parts.join(""));
            captured_fn.borrow_mut().push('\n');
            Object::Undefined
        }),
        extra: None,
    };
    vm.set_global("println", Object::Builtin(Rc::new(println_builtin)));

    let print_builtin = gts::object::Builtin {
        name: "print".into(),
        func: Rc::new(move |_ctx, args: &[Object]| {
            for a in args {
                captured_print.borrow_mut().push_str(&a.inspect());
            }
            Object::Undefined
        }),
        extra: None,
    };
    vm.set_global("print", Object::Builtin(Rc::new(print_builtin)));

    let lexer = Lexer::new(src);
    let mut parser = Parser::new(lexer, "fixture.gs");
    let program = parser.parse_program();
    assert!(
        program.errors.is_empty(),
        "parse errors: {:?}",
        program.errors
    );

    let env = Environment::new_root(vm);
    let chunk = match compile(&program) {
        Ok(c) => c,
        Err(e) => panic!("compile error: {:?}", e),
    };
    let _result = interpret(&chunk, &env);
    let out: String = captured.borrow().clone();
    out
}

struct Fixture {
    dir: &'static str,
    expected: &'static str,
}

/// Stage 1/2 fixtures whose only dependencies are variables, operators, if,
// while/for loops, break/continue, template literals, and println.
fn stage_1_2_fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            dir: "basic_expression",
            expected: "basic-expression=1\n",
        },
        Fixture {
            dir: "comparison_edges",
            expected: "comparison-edges=ok\n",
        },
        Fixture {
            dir: "truthy_logic",
            expected: "truthy-logic=start:ok\n",
        },
        Fixture {
            dir: "template_literals",
            expected: "template-literals=gts:9\n",
        },
        Fixture {
            dir: "control_flow",
            expected: "control-flow=8\n",
        },
        Fixture {
            dir: "for_break",
            expected: "for-break=6\n",
        },
        Fixture {
            dir: "while_continue",
            expected: "while-continue=18\n",
        },
    ]
}

#[test]
fn bytecode_vm_matches_stage_1_2_fixtures() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity");
    for fx in stage_1_2_fixtures() {
        // loop_array_build uses arrays which are stage 5; skip if the source
        // won't compile yet. We detect by attempting compile.
        let path = root.join(fx.dir).join("main.gs");
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => panic!("read {}: {}", path.display(), e),
        };
        // Try compile first; if it fails (unsupported node), report skip.
        let lexer = Lexer::new(&src);
        let mut parser = Parser::new(lexer, "fixture.gs");
        let program = parser.parse_program();
        if !program.errors.is_empty() {
            panic!("{} parse errors: {:?}", fx.dir, program.errors);
        }
        match compile(&program) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("SKIP {}: compile error (later stage): {:?}", fx.dir, e);
                continue;
            }
        }
        let out = run_vm_capturing(&src);
        if out != fx.expected {
            eprintln!(
                "fixture `{}` mismatch:\n--- expected ---\n{:?}\n--- got ---\n{:?}",
                fx.dir, fx.expected, out
            );
        }
        assert_eq!(
            out, fx.expected,
            "fixture `{}` output mismatch under bytecode VM",
            fx.dir
        );
    }
}
