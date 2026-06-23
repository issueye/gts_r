use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{IsTerminal, Read, Write};
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

pub(crate) fn terminal_module() -> Object {
    module(vec![
        ("isTTY", native("terminal.isTTY", terminal_is_tty)),
        ("size", native("terminal.size", terminal_size)),
        (
            "capabilities",
            native("terminal.capabilities", terminal_capabilities),
        ),
        ("read", native("terminal.read", terminal_read)),
        ("write", native("terminal.write", terminal_write)),
        ("writeln", native("terminal.writeln", terminal_writeln)),
        (
            "renderFrame",
            native("terminal.renderFrame", terminal_render_frame),
        ),
        (
            "setRawMode",
            native("terminal.setRawMode", terminal_set_raw_mode),
        ),
        ("start", native("terminal.start", terminal_start)),
        ("clear", native("terminal.clear", terminal_clear_screen)),
        (
            "clearScreen",
            native("terminal.clearScreen", terminal_clear_screen),
        ),
        (
            "clearLine",
            native("terminal.clearLine", terminal_clear_line),
        ),
        ("moveTo", native("terminal.moveTo", terminal_move_to)),
        ("setTitle", native("terminal.setTitle", terminal_set_title)),
        ("style", native("terminal.style", terminal_style)),
        (
            "hyperlink",
            native("terminal.hyperlink", terminal_hyperlink),
        ),
    ])
}

pub(crate) fn terminal_is_tty(_ctx: &mut CallContext, args: &[Object]) -> Object {
    let stream = match args.first() {
        Some(Object::String(value)) => value.to_ascii_lowercase(),
        _ => "stdout".to_string(),
    };
    let interactive = match stream.as_str() {
        "stdin" | "in" => std::io::stdin().is_terminal(),
        "stderr" | "err" => std::io::stderr().is_terminal(),
        _ => std::io::stdout().is_terminal(),
    };
    bool_obj(interactive)
}

pub(crate) fn terminal_size(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    let cols = env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(80);
    let rows = env::var("LINES")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(24);
    module(vec![
        ("cols", num_obj(cols as f64)),
        ("rows", num_obj(rows as f64)),
    ])
}

pub(crate) fn terminal_capabilities(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    module(vec![
        ("clearScrollback", bool_obj(true)),
        ("alternateScreen", bool_obj(true)),
        ("resizeEvents", bool_obj(false)),
        ("virtualTerminal", bool_obj(true)),
        ("rawMode", bool_obj(false)),
    ])
}

pub(crate) fn terminal_read(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    str_obj("")
}

pub(crate) fn terminal_write(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(value) = args.first() else {
        return new_error(ctx.pos.clone(), "terminal.write requires text");
    };
    let text = object_to_text(value);
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(_) => num_obj(text.len() as f64),
        Err(e) => new_error(ctx.pos.clone(), format!("terminal.write: {}", e)),
    }
}

pub(crate) fn terminal_writeln(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = args.first().map(object_to_text).unwrap_or_default() + "\n";
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(_) => num_obj(text.len() as f64),
        Err(e) => new_error(ctx.pos.clone(), format!("terminal.write: {}", e)),
    }
}

pub(crate) fn terminal_render_frame(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(frame) = args.first() else {
        return new_error(ctx.pos.clone(), "terminal.renderFrame requires frame");
    };
    let mut text = String::new();
    let full = hash_bool_arg(args.get(1), "full").unwrap_or(false);
    if full {
        text.push_str("\x1b[2J");
    }
    text.push_str("\x1b[H");
    text.push_str(&object_to_text(frame));
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(_) => {
            let _ = std::io::stdout().flush();
            num_obj(text.len() as f64)
        }
        Err(e) => new_error(ctx.pos.clone(), format!("terminal.renderFrame: {}", e)),
    }
}

pub(crate) fn terminal_set_raw_mode(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    module(vec![
        ("raw", bool_obj(false)),
        (
            "restore",
            native("terminal.restoreRawMode", |_ctx, _args| Object::Undefined),
        ),
    ])
}

pub(crate) fn terminal_start(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    module(vec![
        ("active", bool_obj(false)),
        ("write", native("terminal.session.write", terminal_write)),
        (
            "writeln",
            native("terminal.session.writeln", terminal_writeln),
        ),
        ("size", native("terminal.session.size", terminal_size)),
        (
            "restore",
            native("terminal.session.restore", |_ctx, _args| Object::Undefined),
        ),
        (
            "stop",
            native("terminal.session.stop", |_ctx, _args| Object::Undefined),
        ),
    ])
}

pub(crate) fn terminal_clear_screen(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    str_obj("\x1b[2J\x1b[H")
}

pub(crate) fn terminal_clear_line(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    str_obj("\x1b[2K\r")
}

pub(crate) fn terminal_move_to(ctx: &mut CallContext, args: &[Object]) -> Object {
    let row = match required_number(ctx, "terminal.moveTo", args, 0, "row") {
        Ok(row) => row.max(1.0) as i64,
        Err(err) => return err,
    };
    let col = match required_number(ctx, "terminal.moveTo", args, 1, "col") {
        Ok(col) => col.max(1.0) as i64,
        Err(err) => return err,
    };
    str_obj(format!("\x1b[{};{}H", row, col))
}

pub(crate) fn terminal_set_title(ctx: &mut CallContext, args: &[Object]) -> Object {
    let title = match required_string(ctx, "terminal.setTitle", args, 0, "title") {
        Ok(title) => title,
        Err(err) => return err,
    };
    if !std::io::stdout().is_terminal() {
        return Object::Undefined;
    }
    let text = format!("\x1b]0;{}\x07", title);
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(_) => num_obj(text.len() as f64),
        Err(e) => new_error(ctx.pos.clone(), format!("terminal.setTitle: {}", e)),
    }
}

pub(crate) fn terminal_style(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "terminal.style", args, 0, "text") {
        Ok(text) => text,
        Err(err) => return err,
    };
    let Some(Object::Hash(hash)) = args.get(1) else {
        return str_obj(text);
    };
    let hash = hash.borrow();
    let mut codes = Vec::<String>::new();
    for (key, code) in [
        ("bold", "1"),
        ("dim", "2"),
        ("underline", "4"),
        ("inverse", "7"),
    ] {
        if matches!(hash.get(key), Some(Object::Boolean(true))) {
            codes.push(code.to_string());
        }
    }
    if let Some(fg) = hash_string(&hash, "fg").or_else(|| hash_string(&hash, "color")) {
        if let Some(code) = terminal_color_code(&fg, false) {
            codes.push(code.to_string());
        }
    }
    if let Some(bg) = hash_string(&hash, "bg") {
        if let Some(code) = terminal_color_code(&bg, true) {
            codes.push(code.to_string());
        }
    }
    if codes.is_empty() {
        str_obj(text)
    } else {
        str_obj(format!("\x1b[{}m{}\x1b[0m", codes.join(";"), text))
    }
}

pub(crate) fn terminal_hyperlink(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "terminal.hyperlink", args, 0, "text") {
        Ok(text) => text,
        Err(err) => return err,
    };
    let url = match required_string(ctx, "terminal.hyperlink", args, 1, "url") {
        Ok(url) => url,
        Err(err) => return err,
    };
    str_obj(format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text))
}

// ---------------------------------------------------------------------------
// tui: lightweight script-driven terminal UI helpers.
// ---------------------------------------------------------------------------

pub(crate) fn terminal_size_object() -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut()
        .set("cols", num_obj(terminal_cols() as f64));
    hash.borrow_mut()
        .set("rows", num_obj(terminal_rows() as f64));
    Object::Hash(hash)
}

pub(crate) fn terminal_cols() -> i32 {
    env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(80)
}

pub(crate) fn terminal_rows() -> i32 {
    env::var("LINES")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(24)
}
