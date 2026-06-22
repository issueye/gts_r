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

pub(crate) fn color_module() -> Object {
    module(vec![
        ("ansi", native("color.ansi", color_ansi)),
        ("strip", native("color.strip", color_strip)),
        ("stripAnsi", native("color.stripAnsi", color_strip)),
        ("red", native("color.red", color_red)),
        ("green", native("color.green", color_green)),
        ("yellow", native("color.yellow", color_yellow)),
        ("blue", native("color.blue", color_blue)),
        ("magenta", native("color.magenta", color_magenta)),
        ("cyan", native("color.cyan", color_cyan)),
        ("bold", native("color.bold", color_bold)),
        ("dim", native("color.dim", color_dim)),
        ("underline", native("color.underline", color_underline)),
        ("reset", str_obj("\x1b[0m")),
    ])
}

pub(crate) fn color_ansi(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "color.ansi", args, 0, "text") {
        Ok(text) => text,
        Err(err) => return err,
    };
    let code = match required_number(ctx, "color.ansi", args, 1, "code") {
        Ok(code) => code,
        Err(err) => return err,
    };
    ansi_wrap(&text, code as i64)
}

pub(crate) fn color_strip(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "color.strip", args, 0, "text") {
        Ok(text) => str_obj(strip_ansi(&text)),
        Err(err) => err,
    }
}

pub(crate) fn color_red(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.red", 31)
}

pub(crate) fn color_green(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.green", 32)
}

pub(crate) fn color_yellow(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.yellow", 33)
}

pub(crate) fn color_blue(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.blue", 34)
}

pub(crate) fn color_magenta(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.magenta", 35)
}

pub(crate) fn color_cyan(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.cyan", 36)
}

pub(crate) fn color_bold(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.bold", 1)
}

pub(crate) fn color_dim(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.dim", 2)
}

pub(crate) fn color_underline(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.underline", 4)
}

pub(crate) fn color_named(ctx: &mut CallContext, args: &[Object], name: &str, code: i64) -> Object {
    match required_string(ctx, name, args, 0, "text") {
        Ok(text) => ansi_wrap(&text, code),
        Err(err) => err,
    }
}

pub(crate) fn ansi_wrap(text: &str, code: i64) -> Object {
    str_obj(format!("\x1b[{}m{}\x1b[0m", code, text))
}

// ---------------------------------------------------------------------------
// diff: line-oriented comparison helpers.
// ---------------------------------------------------------------------------
