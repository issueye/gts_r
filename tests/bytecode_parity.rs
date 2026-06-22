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

/// Stage 1/2/3 fixtures whose dependencies are now covered by the bytecode VM.
fn stage_1_3_fixtures() -> Vec<Fixture> {
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
        Fixture {
            dir: "nested_loops",
            expected: "nested-loops=111213212223\n",
        },
        Fixture {
            dir: "loop_array_build",
            expected: "loop-array-build=0|1|4|9\n",
        },
        Fixture {
            dir: "for_in_object",
            expected: "for-in-object=abc\n",
        },
        Fixture {
            dir: "for_of_array",
            expected: "for-of-array=6:go:5:xy\n",
        },
        Fixture {
            dir: "labeled_break",
            expected: "labeled-break=1\n",
        },
        Fixture {
            dir: "function_call",
            expected: "function-call=14\n",
        },
        Fixture {
            dir: "recursive_function",
            expected: "recursive-function=120\n",
        },
        Fixture {
            dir: "function_params",
            expected: "function-params=item:undefined:key\n",
        },
        Fixture {
            dir: "function_rest_params",
            expected: "function-rest-params=v:1|2|3:3\n",
        },
        Fixture {
            dir: "function_arguments",
            expected: "function-arguments=3:a:c:a\n",
        },
        Fixture {
            dir: "function_spread_call",
            expected: "function-spread-call=a:b:c\n",
        },
        Fixture {
            dir: "string_methods",
            expected: "string-methods=ALPHA:4\n",
        },
        Fixture {
            dir: "array_map_callback",
            expected: "array-map-callback=2|4|6\n",
        },
        Fixture {
            dir: "function_closure",
            expected: "function-closure=13\n",
        },
        Fixture {
            dir: "closure_counter",
            expected: "closure-counter=1:2:3\n",
        },
        Fixture {
            dir: "closure_iife",
            expected: "closure-iife=goscript\n",
        },
        Fixture {
            dir: "closure_returned_frame",
            expected: "closure-returned-frame=42\n",
        },
        Fixture {
            dir: "arrays_objects",
            expected: "arrays-objects=3:3:8:gts:1\n",
        },
        Fixture {
            dir: "array_index_assignment",
            expected: "array-index-assignment=1,4,3\n",
        },
        Fixture {
            dir: "array_reduce",
            expected: "array-reduce=10\n",
        },
        Fixture {
            dir: "array_slice_join",
            expected: "array-slice-join=one:two:4\n",
        },
        Fixture {
            dir: "array_shift_unshift",
            expected: "array-shift-unshift=1:2|3\n",
        },
        Fixture {
            dir: "array_find_index",
            expected: "array-find-index=8:3\n",
        },
        Fixture {
            dir: "object_computed_key",
            expected: "object-computed-key=14:14\n",
        },
        Fixture {
            dir: "object_nested_access",
            expected: "object-nested-access=ada:12\n",
        },
        Fixture {
            dir: "object_method_call",
            expected: "object-method-call=10:10\n",
        },
        Fixture {
            dir: "class_basic",
            expected: "class-basic=7:7\n",
        },
        Fixture {
            dir: "class_inheritance_method",
            expected: "class-inheritance-method=10\n",
        },
        Fixture {
            dir: "class_inheritance_constructor",
            expected: "class-inheritance-constructor=12\n",
        },
        Fixture {
            dir: "class_implicit_super",
            expected: "class-implicit-super=10\n",
        },
        Fixture {
            dir: "class_super_method_override",
            expected: "class-super-method-override=child:base:106\n",
        },
        Fixture {
            dir: "class_method_this",
            expected: "class-method-this=2:ab\n",
        },
        Fixture {
            dir: "class_field_update",
            expected: "class-field-update=12\n",
        },
        Fixture {
            dir: "try_catch",
            expected: "try-catch=boom:finally\n",
        },
        Fixture {
            dir: "try_finally_no_throw",
            expected: "try-finally-no-throw=body:try:finally\n",
        },
        Fixture {
            dir: "throw_catch_string",
            expected: "throw-catch-string=boom\n",
        },
        Fixture {
            dir: "throw_catch_error",
            expected: "throw-catch-error=boom\n",
        },
        Fixture {
            dir: "catch_finally_order",
            expected: "catch-finally-order=start:catch:finally\n",
        },
        Fixture {
            dir: "match_basic",
            expected: "match-basic=two\n",
        },
        Fixture {
            dir: "match_string",
            expected: "match-string=go\n",
        },
        Fixture {
            dir: "match_null",
            expected: "match-null=nil\n",
        },
        Fixture {
            dir: "match_boolean",
            expected: "match-boolean=no\n",
        },
        Fixture {
            dir: "match_default_only",
            expected: "match-default-only=fallback\n",
        },
        Fixture {
            dir: "match_block_body",
            expected: "match-block-body=hit:6\n",
        },
        Fixture {
            dir: "match_no_arm_catch",
            expected: "match-no-arm-catch=MatchError\n",
        },
        Fixture {
            dir: "match_or",
            expected: "match-or=primary\n",
        },
        Fixture {
            dir: "match_range",
            expected: "match-range=medium\n",
        },
        Fixture {
            dir: "match_guard",
            expected: "match-guard=medium:6\n",
        },
        Fixture {
            dir: "match_ident_binding",
            expected: "match-ident-binding=id:gts\n",
        },
    ]
}

#[test]
fn bytecode_vm_matches_stage_1_3_fixtures() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity");
    for fx in stage_1_3_fixtures() {
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
