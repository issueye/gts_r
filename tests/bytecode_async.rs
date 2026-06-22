use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use gts::object::{Builtin, Object, EXEC_MODE_BYTECODE, EXEC_MODE_TREEWALK};
use gts::runtime::Session;

fn run_source_capturing(source: &str, exec_mode: u8) -> (Object, String) {
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

    let result = session
        .run_source(source, "bytecode_async_contract.gs")
        .unwrap_or_else(|err| panic!("script should run: {}", err.inspect()));
    let output = captured.borrow().clone();
    (result, output)
}

fn assert_tree_and_bytecode_match(name: &str, source: &str) {
    let (tree_result, tree_out) = run_source_capturing(source, EXEC_MODE_TREEWALK);
    let (bytecode_result, bytecode_out) = run_source_capturing(source, EXEC_MODE_BYTECODE);

    assert_eq!(
        bytecode_result.inspect(),
        tree_result.inspect(),
        "{name}: result mismatch"
    );
    assert_eq!(bytecode_out, tree_out, "{name}: stdout mismatch");
}

#[test]
fn bytecode_matches_tree_for_promise_static_methods() {
    assert_tree_and_bytecode_match(
        "promise static methods",
        r#"
        let resolved = await Promise.resolve("ok");
        let all = await Promise.all([
            Promise.resolve("a"),
            Promise.resolve("b"),
            "c"
        ]);
        let race = await Promise.race([
            Promise.resolve("first"),
            Promise.resolve("second")
        ]);
        let rejected = "none";
        try {
            await Promise.reject("bad");
        } catch (err) {
            rejected = err.message;
        }
        print(`promise-static=${resolved}:${all.join("|")}:${race}:${rejected}`);
        "#,
    );
}

#[test]
fn bytecode_matches_tree_for_promise_chains() {
    assert_tree_and_bytecode_match(
        "promise chains",
        r#"
        let marks = [];
        let value = await Promise.resolve("start")
            .then(function(v) {
                marks.push("then");
                return `${v}:then`;
            })
            .finally(function() {
                marks.push("finally");
            });
        let recovered = await Promise.reject("oops")
            .catch(function(err) {
                marks.push("catch");
                return `caught:${err}`;
            });
        print(`promise-chain=${value}:${recovered}:${marks.join("|")}`);
        "#,
    );
}

#[test]
fn bytecode_matches_tree_for_promise_constructor() {
    assert_tree_and_bytecode_match(
        "promise constructor",
        r#"
        let constructed = await new Promise((resolve) => {
            resolve(42);
        });
        let chained = await new Promise((resolve) => {
            resolve(constructed);
        }).then((value) => {
            return value * 2;
        });
        let recovered = await new Promise((_resolve, reject) => {
            reject(new Error("bad"));
        }).catch((err) => {
            return `caught:${err.message}`;
        });
        print(`promise-new=${constructed}:${chained}:${recovered}`);
        "#,
    );
}

#[test]
fn bytecode_matches_tree_for_async_functions_arrows_and_methods() {
    assert_tree_and_bytecode_match(
        "async function forms",
        r#"
        async function add(a, b) {
            return a + b;
        }
        let inc = async (value) => value + 1;
        class Box {
            constructor(value) {
                this.value = value;
            }
            async get() {
                return this.value;
            }
        }
        let sum = await add(20, 1);
        let arrow = await inc(sum);
        let box = new Box(20);
        let method = await box.get();
        print(`async-forms=${sum}:${arrow}:${method}`);
        "#,
    );
}

#[test]
fn bytecode_matches_tree_for_async_try_catch_and_timers() {
    assert_tree_and_bytecode_match(
        "async try/catch and timers",
        r#"
        async function fail() {
            return await Promise.reject("boom");
        }
        let caught = "none";
        try {
            await fail();
            caught = "miss";
        } catch (err) {
            caught = err.message;
        }

        let timeout = "pending";
        setTimeout(function() {
            timeout = "done";
        }, 0);
        await sleepAsync(0);

        let ticks = 0;
        setInterval(function() {
            ticks = ticks + 1;
        }, 0);
        print(`async-contract=${caught}:${timeout}:${ticks}`);
        "#,
    );
}
