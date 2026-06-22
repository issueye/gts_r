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

pub(crate) fn watch_module() -> Object {
    module(vec![("file", native("watch.file", watch_file))])
}

/// watch.file(path, callback, [options]) -> 同步轮询直到文件修改。
///
/// 在纯单线程运行时模型下无法启动后台 goroutine 回调，因此采用同步语义：
/// 阻塞当前脚本，轮询文件的修改时间，一旦变化立即同步调用回调函数。
/// 可通过 options.duration（毫秒，默认 1000）和 options.timeout（毫秒，默认无限）控制。
fn watch_file(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "watch.file", args, 0, "path") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let callback = match args.get(1) {
        Some(Object::Function(_) | Object::Builtin(_)) => args.get(1).cloned(),
        _ => return new_error(ctx.pos.clone(), "watch.file expects function callback"),
    };
    let callback = match callback {
        Some(c) => c,
        None => return new_error(ctx.pos.clone(), "watch.file expects function callback"),
    };

    let mut interval_ms: u64 = 1000;
    let mut timeout_ms: Option<u64> = None;
    if let Some(Object::Hash(opts)) = args.get(2) {
        if let Some(Object::Number(n)) = opts.borrow().get("interval") {
            interval_ms = *n as u64;
        }
        if let Some(Object::Number(n)) = opts.borrow().get("duration") {
            interval_ms = *n as u64;
        }
        if let Some(Object::Number(n)) = opts.borrow().get("timeout") {
            timeout_ms = Some(*n as u64);
        }
    }
    if interval_ms == 0 {
        interval_ms = 1000;
    }

    // 记录初始修改时间。
    let mut last_mod = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());

    let start = std::time::Instant::now();
    let interval = std::time::Duration::from_millis(interval_ms);
    loop {
        if let Some(t) = timeout_ms {
            if start.elapsed().as_millis() as u64 >= t {
                // 超时：返回 false 表示未检测到变化。
                return Object::Boolean(false);
            }
        }
        std::thread::sleep(interval);

        let current_mod = match std::fs::metadata(&path) {
            Ok(m) => m.modified().ok(),
            Err(_) => continue,
        };

        let changed = match (last_mod, current_mod) {
            (Some(prev), Some(cur)) => cur > prev,
            (None, Some(_)) => true,
            _ => false,
        };

        if changed {
            last_mod = current_mod;
            // 同步调用回调。
            let _ = call_script_function(&callback, ctx.env, &[]);
            return Object::Boolean(true);
        }
    }
}

// ============================================================================
// @std/async - async concurrency primitives
// ----------------------------------------------------------------------------
// Rust 版本是单线程 Rc<RefCell> 模型，无法跨线程执行用户函数（借用检查器
// 禁止跨线程共享 Rc）。此模块提供与 Go 版本 API 兼容的语义：
//   - fetchAsync/getAsync/postAsync：同步执行 HTTP 请求，返回已 resolve 的 Promise。
//     与 Go 版本一样返回 Promise，便于 await/then 链式调用。
//   - runWorker：在隔离 scope 同步求值 fn(args)，返回已 resolve 的 Promise。
// 虽然不是真正的并行，但保持了 API 形状一致，迁移代码无需改动。
// ============================================================================
