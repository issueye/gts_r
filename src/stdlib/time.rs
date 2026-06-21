/// @std/time module - Time and duration utilities
///
/// This module provides comprehensive time manipulation capabilities including:
/// - Current time retrieval
/// - Unix timestamp conversion
/// - Time parsing and formatting
/// - Duration parsing and arithmetic
/// - Time-based operations (add, since, until)

use crate::ast::Position;
use crate::object::{module, native, new_error, num_obj, str_obj, Object, CallContext};
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current time in milliseconds since Unix epoch
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Create the @std/time module
pub fn time_module() -> Object {
    module(vec![
        (
            "now",
            native("time.now", |_ctx, _args| Object::Date(now_ms())),
        ),
        (
            "nowMs",
            native("time.nowMs", |_ctx, _args| num_obj(now_ms() as f64)),
        ),
        ("unix", native("time.unix", time_unix)),
        ("unixMs", native("time.unixMs", time_unix_ms)),
        ("parse", native("time.parse", time_parse)),
        ("format", native("time.format", time_format)),
        ("add", native("time.add", time_add)),
        ("since", native("time.since", time_since)),
        ("until", native("time.until", time_until)),
        (
            "parseDuration",
            native("time.parseDuration", time_parse_duration),
        ),
        ("duration", native("time.duration", time_duration)),
        ("sleep", native("time.sleep", time_sleep)),
        ("RFC3339", str_obj("2006-01-02T15:04:05Z07:00")),
        (
            "RFC3339Nano",
            str_obj("2006-01-02T15:04:05.999999999Z07:00"),
        ),
        ("RFC1123", str_obj("Mon, 02 Jan 2006 15:04:05 MST")),
        ("RFC1123Z", str_obj("Mon, 02 Jan 2006 15:04:05 -0700")),
        ("UnixDate", str_obj("Mon Jan _2 15:04:05 MST 2006")),
        ("DateTime", str_obj("2006-01-02 15:04:05")),
        ("DateOnly", str_obj("2006-01-02")),
        ("TimeOnly", str_obj("15:04:05")),
        ("Kitchen", str_obj("3:04PM")),
    ])
}

fn time_unix(ctx: &mut CallContext, args: &[Object]) -> Object {
    let seconds = match required_number(ctx, "time.unix", args, 0, "seconds") {
        Ok(seconds) => seconds,
        Err(err) => return err,
    };
    let nanos = match args.get(1) {
        Some(Object::Number(value)) => *value,
        Some(_) => return new_error(ctx.pos.clone(), "time.unix: nanoseconds must be a number"),
        None => 0.0,
    };
    Object::Date((seconds * 1000.0 + nanos / 1_000_000.0) as i64)
}

// Helper function to get required number argument
fn required_number(
    ctx: &CallContext,
    func: &str,
    args: &[Object],
    index: usize,
    name: &str,
) -> Result<f64, Object> {
    match args.get(index) {
        Some(Object::Number(value)) => Ok(*value),
        Some(_) => Err(new_error(
            ctx.pos.clone(),
            &format!("{}: {} must be a number", func, name),
        )),
        None => Err(new_error(
            ctx.pos.clone(),
            &format!("{}: missing required argument: {}", func, name),
        )),
    }
}

// Continue with other time functions...
// (This is a template - actual extraction will happen in next steps)

fn time_unix_ms(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Extract from original
    Object::Null
}

fn time_parse(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Extract from original  
    Object::Null
}

fn time_format(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Extract from original
    Object::Null
}

fn time_add(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Extract from original
    Object::Null
}

fn time_since(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Extract from original
    Object::Null
}

fn time_until(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Extract from original
    Object::Null
}

fn time_parse_duration(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Extract from original
    Object::Null
}

fn time_duration(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Extract from original
    Object::Null
}

fn time_sleep(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // TODO: Extract from original
    Object::Null
}
