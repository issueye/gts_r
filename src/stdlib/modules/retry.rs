use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
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
#[allow(unused_imports)]
use crate::ast::Position;
#[allow(unused_imports)]
use crate::object::{
    bool_obj, format_number, new_error, num_obj, str_obj, strict_equal, ArrayData, Builtin,
    CallContext, HashData, Object,
};
#[allow(unused_imports)]
use crate::VERSION;

pub(crate) fn retry_module() -> Object {
    module(vec![
        ("run", native("retry.run", retry_run)),
        (
            "exponential",
            native("retry.exponential", retry_exponential),
        ),
    ])
}

pub(crate) fn retry_run(ctx: &mut CallContext, args: &[Object]) -> Object {
    let func = match args.first() {
        Some(v @ (Object::Function(_) | Object::Builtin(_))) => v.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "retry.run expects function"),
        None => return new_error(ctx.pos.clone(), "retry.run requires function"),
    };
    let (times, mut delay, backoff) = parse_retry_opts(args.get(1), 3, 0.0, 1.0);
    let mut last_err = Object::Undefined;
    for i in 0..times {
        let result = call_script_function(&func, ctx.env, &[]);
        if !result.is_runtime_error() {
            return result;
        }
        last_err = result;
        if i + 1 < times && delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay as u64));
            delay = (delay as f64 * backoff) as i64;
        }
    }
    last_err
}

pub(crate) fn retry_exponential(ctx: &mut CallContext, args: &[Object]) -> Object {
    let func = match args.first() {
        Some(v @ (Object::Function(_) | Object::Builtin(_))) => v.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "retry.exponential expects function"),
        None => return new_error(ctx.pos.clone(), "retry.exponential requires function"),
    };
    let (times, mut delay) = parse_retry_opts_exp(args.get(1), 5, 1000);
    let mut last_err = Object::Undefined;
    for i in 0..times {
        let result = call_script_function(&func, ctx.env, &[]);
        if !result.is_runtime_error() {
            return result;
        }
        last_err = result;
        if i + 1 < times && delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay as u64));
            delay *= 2;
        }
    }
    last_err
}

pub(crate) fn parse_retry_opts(
    opts: Option<&Object>,
    default_times: usize,
    default_delay: f64,
    default_backoff: f64,
) -> (usize, i64, f64) {
    match opts {
        Some(Object::Hash(h)) => {
            let times = match h.borrow().get("times") {
                Some(Object::Number(n)) => *n as usize,
                _ => default_times,
            };
            let delay = match h.borrow().get("delay") {
                Some(Object::Number(n)) => *n as i64,
                _ => default_delay as i64,
            };
            let backoff = match h.borrow().get("backoff") {
                Some(Object::Number(n)) => *n,
                _ => default_backoff,
            };
            (times, delay, backoff)
        }
        _ => (default_times, default_delay as i64, default_backoff),
    }
}

pub(crate) fn parse_retry_opts_exp(
    opts: Option<&Object>,
    default_times: usize,
    default_delay: i64,
) -> (usize, i64) {
    match opts {
        Some(Object::Hash(h)) => {
            let times = match h.borrow().get("times") {
                Some(Object::Number(n)) => *n as usize,
                _ => default_times,
            };
            let delay = match h.borrow().get("initialDelay") {
                Some(Object::Number(n)) => *n as i64,
                _ => default_delay,
            };
            (times, delay)
        }
        _ => (default_times, default_delay),
    }
}

// ---------------------------------------------------------------------------
// stream: a synchronous readable stream over a string.
// ---------------------------------------------------------------------------
