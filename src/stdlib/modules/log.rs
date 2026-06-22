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

pub(crate) fn log_module() -> Object {
    module(vec![
        ("format", native("log.format", log_format)),
        ("debug", native("log.debug", log_debug)),
        ("info", native("log.info", log_info)),
        ("warn", native("log.warn", log_warn)),
        ("error", native("log.error", log_error)),
    ])
}

pub(crate) fn log_format(ctx: &mut CallContext, args: &[Object]) -> Object {
    let level = match required_string(ctx, "log.format", args, 0, "level") {
        Ok(level) => level,
        Err(err) => return err,
    };
    let message = match required_string(ctx, "log.format", args, 1, "message") {
        Ok(message) => message,
        Err(err) => return err,
    };
    str_obj(format_log_line(&level, &message))
}

pub(crate) fn log_debug(ctx: &mut CallContext, args: &[Object]) -> Object {
    log_named(ctx, args, "log.debug", "debug")
}

pub(crate) fn log_info(ctx: &mut CallContext, args: &[Object]) -> Object {
    log_named(ctx, args, "log.info", "info")
}

pub(crate) fn log_warn(ctx: &mut CallContext, args: &[Object]) -> Object {
    log_named(ctx, args, "log.warn", "warn")
}

pub(crate) fn log_error(ctx: &mut CallContext, args: &[Object]) -> Object {
    log_named(ctx, args, "log.error", "error")
}

pub(crate) fn log_named(ctx: &mut CallContext, args: &[Object], name: &str, level: &str) -> Object {
    match required_string(ctx, name, args, 0, "message") {
        Ok(message) => str_obj(format_log_line(level, &message)),
        Err(err) => err,
    }
}

pub(crate) fn format_log_line(level: &str, message: &str) -> String {
    format!("[{}] {}", level.to_ascii_uppercase(), message)
}

// ---------------------------------------------------------------------------
// encoding/csv: small RFC4180-ish parser/writer with Go-compatible options.
// ---------------------------------------------------------------------------
