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

pub(crate) fn glob_module() -> Object {
    module(vec![
        ("glob", native("glob.glob", glob_glob)),
        ("globSync", native("glob.globSync", glob_glob)),
        ("match", native("glob.match", glob_match_native)),
        ("hasMagic", native("glob.hasMagic", glob_has_magic)),
    ])
}

pub(crate) fn glob_glob(ctx: &mut CallContext, args: &[Object]) -> Object {
    let pattern = match required_string(ctx, "glob.glob", args, 0, "pattern") {
        Ok(pattern) => pattern,
        Err(err) => return err,
    };
    match glob_paths(&pattern) {
        Ok(matches) => array(
            matches
                .into_iter()
                .map(|path| str_obj(path.to_string_lossy()))
                .collect(),
        ),
        Err(e) => new_error(ctx.pos.clone(), format!("glob.glob: {}", e)),
    }
}

pub(crate) fn glob_match_native(ctx: &mut CallContext, args: &[Object]) -> Object {
    let pattern = match required_string(ctx, "glob.match", args, 0, "pattern") {
        Ok(pattern) => pattern,
        Err(err) => return err,
    };
    let path = match required_string(ctx, "glob.match", args, 1, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    bool_obj(glob_match(&pattern, &path))
}

pub(crate) fn glob_has_magic(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "glob.hasMagic", args, 0, "pattern") {
        Ok(pattern) => bool_obj(pattern.contains('*') || pattern.contains('?')),
        Err(err) => err,
    }
}

// ---------------------------------------------------------------------------
// color: simple ANSI SGR wrappers and escape stripping.
// ---------------------------------------------------------------------------
