//! End-to-end parity check: run all parity fixtures under the bytecode runtime
//! and compare against the expected stdout recorded in `parity_compat.rs`.
//!
//! We stub the global `println` with a capturing closure so we can assert the
//! exact output without depending on process stdout capture. `print` is
//! similarly captured (no trailing newline).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::Ordering;

use gts::object::{Builtin, Object, EXEC_MODE_BYTECODE};
use gts::runtime::Session;

/// Run a fixture through the bytecode runtime with capturing `println`/`print`.
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
            for a in args {
                captured_print.borrow_mut().push_str(&a.inspect());
            }
            Object::Undefined
        }),
        extra: None,
    };
    session
        .vm()
        .set_global("print", Object::Builtin(Rc::new(print_builtin)));

    let fixture_dir = parity_root().join(dir);
    let entry = fixture_entry(&fixture_dir);
    let result = session
        .run_file(entry, Vec::new())
        .unwrap_or_else(|err| panic!("{dir} should run under bytecode VM: {}", err.inspect()));
    assert!(
        !result.is_runtime_error(),
        "{dir} returned runtime error: {}",
        result.inspect()
    );
    let out: String = captured.borrow().clone();
    out
}

fn fixture_entry(fixture_dir: &Path) -> PathBuf {
    gts::module::resolve_entry_in_dir(fixture_dir).unwrap_or_else(|| fixture_dir.join("main.gs"))
}

struct Fixture {
    dir: &'static str,
    expected: &'static str,
}

fn all_parity_fixtures() -> Vec<Fixture> {
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
            dir: "nullish_coalescing",
            expected: "nullish-coalescing=42:7:0:false\n",
        },
        Fixture {
            dir: "ternary_expression",
            expected: "ternary-expression=yes:hit\n",
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
            dir: "symbol_iterator",
            expected: "symbol-iterator=1:2:true:go:true:xy\n",
        },
        Fixture {
            dir: "custom_symbol_iterator",
            expected: "custom-symbol-iterator=60:10|20|30\n",
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
            dir: "array_splice",
            expected: "array-splice-a=hello:he|llo|world\narray-splice-b=llo|world:he|lloworld\narray-splice-c=0:1|2|3|4|5\narray-splice-d=4|5:1|2|3\n",
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
            dir: "optional_chain",
            expected: "optional-chain=gts:7:undefined:5\n",
        },
        Fixture {
            dir: "regexp_literal",
            expected: "regexp-literal=/go+/i\n",
        },
        Fixture {
            dir: "class_basic",
            expected: "class-basic=7:7\n",
        },
        Fixture {
            dir: "class_expression",
            expected: "class-expression=9\n",
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
        Fixture {
            dir: "typeof_values",
            expected: "typeof-values=number:string:boolean:object:undefined:object:object:function:function\n",
        },
        Fixture {
            dir: "relative_require",
            expected: "relative-require=18\n",
        },
        Fixture {
            dir: "nested_relative_require",
            expected: "nested-relative-require=21\n",
        },
        Fixture {
            dir: "project_module_require",
            expected: "project-module-require=42\n",
        },
        Fixture {
            dir: "directory_module_index",
            expected: "directory-module-index=42\n",
        },
        Fixture {
            dir: "module_cache",
            expected: "module-cache=1:1\n",
        },
        Fixture {
            dir: "module_exports_object",
            expected: "module-exports-object=42\n",
        },
        Fixture {
            dir: "import_default_like",
            expected: "import-default-like=12\n",
        },
        Fixture {
            dir: "export_const",
            expected: "export-const=export:42\n",
        },
        Fixture {
            dir: "export_function_alias",
            expected: "export-function-alias=18\n",
        },
        Fixture {
            dir: "project_entry",
            expected: "project-entry=ok\n",
        },
    ]
}

#[test]
fn bytecode_vm_matches_all_parity_fixtures() {
    assert_all_fixture_dirs_are_covered();
    for fx in all_parity_fixtures() {
        let out = run_fixture_capturing(fx.dir);
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

fn assert_all_fixture_dirs_are_covered() {
    let expected: BTreeSet<&'static str> = all_parity_fixtures()
        .into_iter()
        .map(|fixture| fixture.dir)
        .collect();
    let discovered: BTreeSet<String> = std::fs::read_dir(parity_root())
        .expect("read parity fixtures")
        .filter_map(|entry| {
            let entry = entry.expect("read fixture entry");
            if entry.path().is_dir() {
                Some(
                    entry
                        .file_name()
                        .into_string()
                        .expect("fixture names must be utf-8"),
                )
            } else {
                None
            }
        })
        .collect();
    let expected_strings: BTreeSet<String> = expected.into_iter().map(str::to_string).collect();
    assert_eq!(
        discovered, expected_strings,
        "bytecode parity fixture list must cover every directory under tests/fixtures/parity"
    );
}

fn parity_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity")
}
