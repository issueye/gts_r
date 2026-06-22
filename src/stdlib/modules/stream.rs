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

pub(crate) fn stream_module() -> Object {
    module(vec![(
        "fromString",
        native("stream.fromString", stream_from_string),
    )])
}

pub(crate) fn stream_from_string(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "stream.fromString", args, 0, "text") {
        Ok(v) => v,
        Err(e) => return e,
    };
    stream_from_text(text)
}

pub(crate) fn stream_from_text(text: String) -> Object {
    let state = Rc::new(RefCell::new(StreamState {
        text,
        pos: 0,
        closed: false,
    }));
    let instance = Rc::new(RefCell::new(HashData::default()));

    let s = state.clone();
    instance.borrow_mut().set(
        "read",
        native("stream.read", move |ctx, args| stream_read(ctx, args, &s)),
    );
    let s = state.clone();
    instance.borrow_mut().set(
        "readText",
        native("stream.readText", move |ctx, args| {
            stream_read_text(ctx, args, &s)
        }),
    );
    let s = state.clone();
    instance.borrow_mut().set(
        "readLine",
        native("stream.readLine", move |ctx, _args| {
            stream_read_line(ctx, &s)
        }),
    );
    let s = state.clone();
    instance.borrow_mut().set(
        "readAll",
        native("stream.readAll", move |_ctx, _args| stream_read_all(&s)),
    );
    let s = state.clone();
    instance.borrow_mut().set(
        "close",
        native("stream.close", move |_ctx, _args| {
            s.borrow_mut().closed = true;
            Object::Undefined
        }),
    );
    Object::Hash(instance)
}

pub(crate) struct StreamState {
    text: String,
    pos: usize,
    closed: bool,
}

pub(crate) fn stream_read(
    ctx: &mut CallContext,
    args: &[Object],
    state: &Rc<RefCell<StreamState>>,
) -> Object {
    let size = match args.first() {
        Some(Object::Number(n)) => {
            if *n <= 0.0 {
                return new_error(ctx.pos.clone(), "stream.read: size must be positive");
            }
            *n as usize
        }
        Some(_) => return new_error(ctx.pos.clone(), "stream.read: size must be a number"),
        None => 8192,
    };
    let mut s = state.borrow_mut();
    if s.closed || s.pos >= s.text.len() {
        return Object::Null;
    }
    let end = (s.pos + size).min(s.text.len());
    let bytes: Vec<u8> = s.text.as_bytes()[s.pos..end].to_vec();
    s.pos = end;
    array(bytes.iter().map(|b| num_obj(*b as f64)).collect())
}

pub(crate) fn stream_read_text(
    ctx: &mut CallContext,
    args: &[Object],
    state: &Rc<RefCell<StreamState>>,
) -> Object {
    let size = match args.first() {
        Some(Object::Number(n)) => {
            if *n <= 0.0 {
                return new_error(ctx.pos.clone(), "stream.readText: size must be positive");
            }
            *n as usize
        }
        Some(_) => return new_error(ctx.pos.clone(), "stream.readText: size must be a number"),
        None => 8192,
    };
    let mut s = state.borrow_mut();
    if s.closed || s.pos >= s.text.len() {
        return Object::Null;
    }
    let end = (s.pos + size).min(s.text.len());
    let chunk = s.text[s.pos..end].to_string();
    s.pos = end;
    str_obj(chunk)
}

pub(crate) fn stream_read_line(_ctx: &mut CallContext, state: &Rc<RefCell<StreamState>>) -> Object {
    let mut s = state.borrow_mut();
    if s.closed || s.pos >= s.text.len() {
        return Object::Null;
    }
    let rest = &s.text[s.pos..];
    match rest.find('\n') {
        Some(idx) => {
            let line = rest[..idx].trim_end_matches('\r').to_string();
            s.pos += idx + 1;
            str_obj(line)
        }
        None => {
            let line = rest.trim_end_matches('\r').to_string();
            s.pos = s.text.len();
            str_obj(line)
        }
    }
}

pub(crate) fn stream_read_all(state: &Rc<RefCell<StreamState>>) -> Object {
    let s = state.borrow();
    str_obj(s.text[s.pos..].to_string())
}

// ---------------------------------------------------------------------------
// exec: process execution module (@std/exec)
// ---------------------------------------------------------------------------

pub(crate) fn stream_from_text_object(text: String) -> Object {
    let stream = stream_from_text(text.clone());
    if let Object::Hash(h) = &stream {
        h.borrow_mut().set("text", str_obj(text));
    }
    stream
}
