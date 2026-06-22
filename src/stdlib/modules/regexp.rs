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

pub(crate) fn regexp_module() -> Object {
    module(vec![
        ("escape", native("regexp.escape", regexp_escape)),
        ("matchAll", native("regexp.matchAll", regexp_match_all)),
        ("split", native("regexp.split", regexp_split)),
    ])
}

pub(crate) fn regexp_escape(ctx: &mut CallContext, args: &[Object]) -> Object {
    match args.first() {
        Some(Object::String(s)) => str_obj(regex::escape(s)),
        Some(_) => new_error(ctx.pos.clone(), "regexp.escape expects string"),
        None => new_error(ctx.pos.clone(), "regexp.escape requires string"),
    }
}

pub(crate) fn regexp_match_all(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(
            ctx.pos.clone(),
            "regexp.matchAll requires pattern and string",
        );
    }
    let pattern = match &args[0] {
        Object::String(s) => s.clone(),
        _ => return new_error(ctx.pos.clone(), "regexp.matchAll expects string pattern"),
    };
    let input = match &args[1] {
        Object::String(s) => s.clone(),
        _ => return new_error(ctx.pos.clone(), "regexp.matchAll expects string"),
    };
    let re = match Regex::new(&pattern) {
        Ok(re) => re,
        Err(e) => return new_error(ctx.pos.clone(), format!("regexp.matchAll: {}", e)),
    };
    let mut groups = Vec::new();
    for caps in re.captures_iter(&input) {
        let mut sub: Vec<Object> = Vec::with_capacity(caps.len());
        for i in 0..caps.len() {
            match caps.get(i) {
                Some(m) => sub.push(str_obj(m.as_str())),
                None => sub.push(Object::Undefined),
            }
        }
        groups.push(array(sub));
    }
    array(groups)
}

pub(crate) fn regexp_split(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(ctx.pos.clone(), "regexp.split requires pattern and string");
    }
    let pattern = match &args[0] {
        Object::String(s) => s.clone(),
        _ => return new_error(ctx.pos.clone(), "regexp.split expects string pattern"),
    };
    let input = match &args[1] {
        Object::String(s) => s.clone(),
        _ => return new_error(ctx.pos.clone(), "regexp.split expects string"),
    };
    let re = match Regex::new(&pattern) {
        Ok(re) => re,
        Err(e) => return new_error(ctx.pos.clone(), format!("regexp.split: {}", e)),
    };
    let limit = match args.get(2) {
        Some(Object::Number(n)) => *n as i64,
        _ => -1,
    };
    let parts: Vec<&str> = if limit < 0 {
        re.split(&input).collect()
    } else if limit == 0 {
        Vec::new()
    } else {
        re.splitn(&input, limit as usize).collect()
    };
    array(parts.into_iter().map(str_obj).collect())
}

// ---------------------------------------------------------------------------
// semver: parse / compare / satisfies / inc.
// ---------------------------------------------------------------------------
