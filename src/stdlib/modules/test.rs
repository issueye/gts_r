use std::cell::RefCell;
use std::rc::Rc;

use super::super::helpers::*;
use crate::object::{new_error, num_obj, strict_equal, CallContext, HashData, Object};

pub(crate) fn test_module() -> Object {
    module(vec![
        ("test", native("test.test", test_test)),
        ("it", native("test.it", test_test)),
        ("describe", native("test.describe", test_describe)),
        ("expect", native("test.expect", test_expect)),
        ("run", native("test.run", test_run)),
    ])
}

#[derive(Clone)]
enum TestNode {
    Suite {
        name: String,
        children: Vec<TestNode>,
    },
    Case {
        name: String,
        func: Object,
    },
}

thread_local! {
    static TEST_ROOT: std::cell::RefCell<Vec<TestNode>> = std::cell::RefCell::new(Vec::new());
    static EXPECT_FAILS: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
}

pub(crate) fn test_test(ctx: &mut CallContext, args: &[Object]) -> Object {
    let name = match required_string(ctx, "test.test", args, 0, "name") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let func = match args.get(1) {
        Some(v) => v.clone(),
        None => return new_error(ctx.pos.clone(), "test requires name and function"),
    };
    TEST_ROOT.with(|r| {
        r.borrow_mut().push(TestNode::Case { name, func });
    });
    Object::Undefined
}

pub(crate) fn test_describe(ctx: &mut CallContext, args: &[Object]) -> Object {
    let name = match required_string(ctx, "test.describe", args, 0, "name") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let func = match args.get(1) {
        Some(Object::Function(_)) => args[1].clone(),
        Some(v) => v.clone(),
        None => return new_error(ctx.pos.clone(), "describe requires name and function"),
    };
    // Execute the describe body synchronously; nested test()/it() calls
    // register into the current suite.
    let _ = call_script_function(&func, ctx.env, &[]);
    let _ = name;
    Object::Undefined
}

pub(crate) fn test_expect(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v.clone(),
        None => return new_error(ctx.pos.clone(), "expect requires a value"),
    };
    let expectation = Rc::new(RefCell::new(HashData::default()));
    expectation.borrow_mut().set("__value__", value);

    // Each matcher closure captures its own clone of the Rc so the original
    // can still be returned.
    let e1 = expectation.clone();
    expectation.borrow_mut().set(
        "toBe",
        native("test.expect.toBe", move |ctx, args| {
            expect_matcher(ctx, &e1, args, ExpectOp::Be)
        }),
    );
    let e2 = expectation.clone();
    expectation.borrow_mut().set(
        "toEqual",
        native("test.expect.toEqual", move |ctx, args| {
            expect_matcher(ctx, &e2, args, ExpectOp::Equal)
        }),
    );
    let e3 = expectation.clone();
    expectation.borrow_mut().set(
        "toBeTruthy",
        native("test.expect.toBeTruthy", move |ctx, _args| {
            expect_truthy(ctx, &e3, true)
        }),
    );
    let e4 = expectation.clone();
    expectation.borrow_mut().set(
        "toBeFalsy",
        native("test.expect.toBeFalsy", move |ctx, _args| {
            expect_truthy(ctx, &e4, false)
        }),
    );
    Object::Hash(expectation)
}

pub(crate) enum ExpectOp {
    Be,
    Equal,
}

pub(crate) fn expect_matcher(
    ctx: &mut CallContext,
    expectation: &Rc<RefCell<HashData>>,
    args: &[Object],
    op: ExpectOp,
) -> Object {
    let actual = expectation
        .borrow()
        .get("__value__")
        .cloned()
        .unwrap_or(Object::Undefined);
    let expected = match args.first() {
        Some(v) => v.clone(),
        None => return new_error(ctx.pos.clone(), "matcher requires an expected value"),
    };
    let passed = match op {
        ExpectOp::Be => strict_equal(&actual, &expected),
        ExpectOp::Equal => deep_equal(&actual, &expected),
    };
    if passed {
        Object::Undefined
    } else {
        let label = match op {
            ExpectOp::Be => "to be",
            ExpectOp::Equal => "to equal",
        };
        new_error(
            ctx.pos.clone(),
            format!(
                "Expected {} {} {}",
                actual.inspect(),
                label,
                expected.inspect()
            ),
        )
    }
}

pub(crate) fn expect_truthy(
    ctx: &mut CallContext,
    expectation: &Rc<RefCell<HashData>>,
    expect_truthy: bool,
) -> Object {
    let actual = expectation
        .borrow()
        .get("__value__")
        .cloned()
        .unwrap_or(Object::Undefined);
    let truthy = is_truthy(&actual);
    let passed = if expect_truthy { truthy } else { !truthy };
    if passed {
        Object::Undefined
    } else {
        let label = if expect_truthy { "truthy" } else { "falsy" };
        new_error(
            ctx.pos.clone(),
            format!("Expected {} to be {}", actual.inspect(), label),
        )
    }
}

pub(crate) fn test_run(ctx: &mut CallContext, _args: &[Object]) -> Object {
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failed = 0usize;
    TEST_ROOT.with(|r| {
        let nodes = r.borrow_mut().clone();
        for node in &nodes {
            if let TestNode::Case { name, func } = node {
                total += 1;
                EXPECT_FAILS.with(|f| f.borrow_mut().clear());
                let result = call_script_function(func, ctx.env, &[]);
                let failed_here = matches!(result, Object::Error(_))
                    || EXPECT_FAILS.with(|f| !f.borrow().is_empty());
                if failed_here {
                    failed += 1;
                    let _ = name;
                } else {
                    passed += 1;
                }
            }
        }
    });
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("total", num_obj(total as f64));
    hash.borrow_mut().set("passed", num_obj(passed as f64));
    hash.borrow_mut().set("failed", num_obj(failed as f64));
    Object::Hash(hash)
}
