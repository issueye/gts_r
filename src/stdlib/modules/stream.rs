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
    crate::object::http_stream::stream_from_text(text)
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
