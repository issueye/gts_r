use std::cell::Cell;
use std::cell::RefCell;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf, MAIN_SEPARATOR, MAIN_SEPARATOR_STR};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

#[allow(unused_imports)]
use std::process::Command;
#[allow(unused_imports)]
use std::process::Stdio;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
#[allow(unused_imports)]
use regex::Regex;

use super::super::helpers::*;
use super::net_http_client::{http_client_get, http_client_post, http_client_request};
#[allow(unused_imports)]
use crate::ast::Position;
#[allow(unused_imports)]
use crate::object::{
    bool_obj, format_number, new_error, num_obj, str_obj, strict_equal, ArrayData, Builtin,
    CallContext, HashData, Object,
};
#[allow(unused_imports)]
use crate::VERSION;

pub(crate) fn async_module() -> Object {
    module(vec![
        ("fetchAsync", native("async.fetchAsync", async_fetch)),
        ("getAsync", native("async.getAsync", async_get)),
        ("postAsync", native("async.postAsync", async_post)),
        ("runWorker", native("async.runWorker", async_run_worker)),
    ])
}

fn resolved_promise(value: Object) -> Object {
    let promise = crate::object::Promise::new();
    promise.resolve(value);
    Object::Promise(promise)
}

fn rejected_promise(reason: Object) -> Object {
    let promise = crate::object::Promise::new();
    promise.reject(reason);
    Object::Promise(promise)
}

fn async_get(ctx: &mut CallContext, args: &[Object]) -> Object {
    let result = http_client_get(ctx, args);
    match result.is_runtime_error() {
        true => rejected_promise(result),
        false => resolved_promise(result),
    }
}

fn async_post(ctx: &mut CallContext, args: &[Object]) -> Object {
    let result = http_client_post(ctx, args);
    match result.is_runtime_error() {
        true => rejected_promise(result),
        false => resolved_promise(result),
    }
}

fn async_fetch(ctx: &mut CallContext, args: &[Object]) -> Object {
    let result = http_client_request(ctx, args);
    match result.is_runtime_error() {
        true => rejected_promise(result),
        false => resolved_promise(result),
    }
}

fn async_run_worker(ctx: &mut CallContext, args: &[Object]) -> Object {
    let func = match args.first() {
        Some(Object::Function(_) | Object::Builtin(_)) => args[0].clone(),
        _ => {
            return rejected_promise(new_error(
                ctx.pos.clone(),
                "async.runWorker: first argument must be a function",
            ))
        }
    };
    let worker_args: Vec<Object> = args.iter().skip(1).cloned().collect();
    let result = call_script_function(&func, ctx.env, &worker_args);
    match result.is_runtime_error() {
        true => rejected_promise(result),
        false => resolved_promise(result),
    }
}
