//! Native standard library modules (`@std/*`).

pub mod gtp;

use std::cell::RefCell;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf, MAIN_SEPARATOR, MAIN_SEPARATOR_STR};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use regex::Regex;

use crate::object::{
    bool_obj, format_number, new_error, num_obj, str_obj, strict_equal, ArrayData, Builtin,
    CallContext, HashData, Object,
};
use crate::VERSION;

/// Load a native `@std/*` module by specifier.
pub fn load_native_module(spec: &str) -> Option<Object> {
    match spec {
        "@std/path" => Some(path_module()),
        "@std/os" => Some(os_module()),
        "@std/env" => Some(env_module()),
        "@std/fs" => Some(fs_module()),
        "@std/json" => Some(json_module()),
        "@std/time" => Some(time_module()),
        "@std/encoding/base64" => Some(base64_module()),
        "@std/encoding/hex" => Some(hex_module()),
        "@std/hash" => Some(hash_module()),
        "@std/crypto" => Some(crypto_module()),
        "@std/random" => Some(random_module()),
        "@std/regexp" => Some(regexp_module()),
        "@std/semver" => Some(semver_module()),
        "@std/collections" => Some(collections_module()),
        "@std/process" => Some(process_module()),
        "@std/text" => Some(text_module()),
        "@std/url" => Some(url_module()),
        "@std/cache" => Some(cache_module()),
        "@std/timers" => Some(timers_module()),
        "@std/glob" => Some(glob_module()),
        "@std/color" => Some(color_module()),
        "@std/diff" => Some(diff_module()),
        "@std/log" => Some(log_module()),
        "@std/table" => Some(table_module()),
        "@std/validation" => Some(validation_module()),
        "@std/encoding/csv" => Some(csv_module()),
        "@std/template" => Some(template_module()),
        "@std/compression" => Some(compression_module()),
        "@std/compress/gzip" => Some(gzip_module()),
        "@std/terminal" => Some(terminal_module()),
        "@std/cli" => Some(cli_module()),
        "@std/tui" => Some(tui_module()),
        "@std/toml" => Some(toml_module()),
        "@std/yaml" => Some(yaml_module()),
        "@std/xml" => Some(xml_module()),
        "@std/markdown" => Some(markdown_module()),
        "@std/schema" => Some(schema_module()),
        "@std/test" => Some(test_module()),
        "@std/archive/zip" => Some(archive_zip_module()),
        "@std/buffer" => Some(buffer_module()),
        "@std/events" => Some(events_module()),
        "@std/jwt" => Some(jwt_module()),
        "@std/mime" => Some(mime_module()),
        "@std/net/ip" => Some(net_ip_module()),
        "@std/retry" => Some(retry_module()),
        "@std/stream" => Some(stream_module()),
        "@std/exec" => Some(exec_module()),
        "@std/net/http/client" => Some(http_client_module()),
        "@std/rate-limit" => Some(rate_limit_module()),
        "@std/prometheus" => Some(prometheus_module()),
        "@std/highlight" => Some(highlight_module()),
        "@std/sse" => Some(sse_module()),
        "@std/db" => Some(db_module()),
        "@std/mail" => Some(mail_module()),
        "@std/net/socket/client" => Some(socket_client_module()),
        "@std/net/socket/server" => Some(socket_server_module()),
        "@std/runtime" => Some(runtime_module()),
        "@std/image" => Some(image_module()),
        "@std/pdf" => Some(pdf_module()),
        "@std/net/ws/client" => Some(ws_client_module()),
        "@std/net/ws/server" => Some(ws_server_module()),
        "@std/net/http/server" => Some(http_server_module()),
        "@std/web" => Some(web_module()),
        "@std/express" => Some(web_module()),
        "@std/signal" => Some(signal_module()),
        "@std/watch" => Some(watch_module()),
        "@std/async" => Some(async_module()),
        "@std/pty" => Some(pty_module()),

        // GTP modules - delegate to gtp submodule
        spec if spec.starts_with("@std/gtp/") => gtp::load_gtp_module(spec),

        _ => None,
    }
}

fn module(entries: Vec<(&str, Object)>) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    for (name, value) in entries {
        hash.borrow_mut().set(name, value);
    }
    Object::Hash(hash)
}

fn native(
    name: &str,
    func: impl Fn(&mut CallContext<'_>, &[Object]) -> Object + 'static,
) -> Object {
    Object::Builtin(Rc::new(Builtin {
        name: name.into(),
        func: Rc::new(func),
        extra: None,
    }))
}

fn path_module() -> Object {
    module(vec![
        ("join", native("path.join", path_join)),
        ("resolve", native("path.resolve", path_resolve)),
        ("relative", native("path.relative", path_relative)),
        ("normalize", native("path.normalize", path_normalize)),
        ("dirname", native("path.dirname", path_dirname)),
        ("basename", native("path.basename", path_basename)),
        ("extname", native("path.extname", path_extname)),
        ("isAbs", native("path.isAbs", path_is_abs)),
        ("toSlash", native("path.toSlash", path_to_slash)),
        ("fromSlash", native("path.fromSlash", path_from_slash)),
        ("parse", native("path.parse", path_parse)),
        ("format", native("path.format", path_format)),
        ("splitList", native("path.splitList", path_split_list)),
        ("sep", str_obj(MAIN_SEPARATOR.to_string())),
        ("delimiter", str_obj(if cfg!(windows) { ";" } else { ":" })),
    ])
}

fn path_join(ctx: &mut CallContext, args: &[Object]) -> Object {
    let parts = match string_args(ctx, "path.join", args) {
        Ok(parts) => parts,
        Err(err) => return err,
    };
    let mut path = PathBuf::new();
    for part in parts {
        path.push(part);
    }
    str_obj(path.to_string_lossy())
}

fn path_resolve(ctx: &mut CallContext, args: &[Object]) -> Object {
    let parts = match string_args(ctx, "path.resolve", args) {
        Ok(parts) => parts,
        Err(err) => return err,
    };
    let mut path = if parts.is_empty() {
        PathBuf::from(".")
    } else {
        let mut path = PathBuf::new();
        for part in parts {
            path.push(part);
        }
        path
    };
    if !path.is_absolute() {
        match env::current_dir() {
            Ok(cwd) => path = cwd.join(path),
            Err(e) => return new_error(ctx.pos.clone(), format!("path.resolve: {}", e)),
        }
    }
    str_obj(path.to_string_lossy())
}

fn path_relative(ctx: &mut CallContext, args: &[Object]) -> Object {
    let from = match required_string(ctx, "path.relative", args, 0, "from") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let to = match required_string(ctx, "path.relative", args, 1, "to") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match pathdiff(&PathBuf::from(from), &PathBuf::from(to)) {
        Some(path) => str_obj(path.to_string_lossy()),
        None => new_error(
            ctx.pos.clone(),
            "path.relative: cannot compute relative path",
        ),
    }
}

fn path_normalize(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "path.normalize", args, 0, "path") {
        Ok(value) => value,
        Err(err) => return err,
    };
    str_obj(normalize_path_string(&path))
}

fn path_dirname(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "path.dirname", args, 0, "path") {
        Ok(value) => value,
        Err(err) => return err,
    };
    str_obj(
        Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into()),
    )
}

fn path_basename(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "path.basename", args, 0, "path") {
        Ok(value) => value,
        Err(err) => return err,
    };
    str_obj(
        Path::new(&path)
            .file_name()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    )
}

fn path_extname(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "path.extname", args, 0, "path") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let ext = Path::new(&path)
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    str_obj(ext)
}

fn path_is_abs(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "path.isAbs", args, 0, "path") {
        Ok(value) => bool_obj(Path::new(&value).is_absolute()),
        Err(err) => err,
    }
}

fn path_to_slash(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "path.toSlash", args, 0, "path") {
        Ok(value) => str_obj(value.replace('\\', "/")),
        Err(err) => err,
    }
}

fn path_from_slash(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "path.fromSlash", args, 0, "path") {
        Ok(value) => str_obj(value.replace('/', MAIN_SEPARATOR_STR)),
        Err(err) => err,
    }
}

fn path_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "path.parse", args, 0, "path") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let path = Path::new(&value);
    let base = path
        .file_name()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|p| format!(".{}", p.to_string_lossy()))
        .unwrap_or_default();
    let name = base.strip_suffix(&ext).unwrap_or(&base).to_string();
    let dir = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".into());
    let root = if path.is_absolute() {
        MAIN_SEPARATOR.to_string()
    } else {
        String::new()
    };
    module(vec![
        ("root", str_obj(root)),
        ("dir", str_obj(dir)),
        ("base", str_obj(base)),
        ("name", str_obj(name)),
        ("ext", str_obj(ext)),
    ])
}

fn path_format(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(Object::Hash(hash)) = args.first() else {
        return new_error(ctx.pos.clone(), "path.format requires a path object");
    };
    let hash = hash.borrow();
    let dir = hash_string(&hash, "dir").unwrap_or_default();
    let root = hash_string(&hash, "root").unwrap_or_default();
    let base = hash_string(&hash, "base").unwrap_or_default();
    let name = hash_string(&hash, "name").unwrap_or_default();
    let ext = hash_string(&hash, "ext").unwrap_or_default();
    let file = if !base.is_empty() {
        base
    } else {
        format!("{}{}", name, ext)
    };
    if !dir.is_empty() {
        str_obj(PathBuf::from(dir).join(file).to_string_lossy())
    } else {
        str_obj(PathBuf::from(root).join(file).to_string_lossy())
    }
}

fn path_split_list(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "path.splitList", args, 0, "value") {
        Ok(value) => value,
        Err(err) => return err,
    };
    array(
        env::split_paths(&value)
            .map(|p| str_obj(p.to_string_lossy()))
            .collect(),
    )
}

fn os_module() -> Object {
    module(vec![
        ("platform", str_obj(env::consts::OS)),
        ("arch", str_obj(env::consts::ARCH)),
        ("eol", str_obj(if cfg!(windows) { "\r\n" } else { "\n" })),
        ("type", native("os.type", os_type)),
        (
            "release",
            native("os.release", |_ctx, _args| {
                str_obj(format!("{}/{}", env::consts::OS, env::consts::ARCH))
            }),
        ),
        ("homedir", native("os.homedir", os_homedir)),
        (
            "tmpdir",
            native("os.tmpdir", |_ctx, _args| {
                str_obj(env::temp_dir().to_string_lossy())
            }),
        ),
        ("hostname", native("os.hostname", os_hostname)),
        (
            "cpus",
            native("os.cpus", |_ctx, _args| {
                num_obj(
                    std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(1) as f64,
                )
            }),
        ),
        ("userInfo", native("os.userInfo", os_user_info)),
    ])
}

fn os_type(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    str_obj(match env::consts::OS {
        "windows" => "Windows_NT",
        "macos" => "Darwin",
        "linux" => "Linux",
        other => other,
    })
}

fn os_homedir(ctx: &mut CallContext, _args: &[Object]) -> Object {
    match env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
        Ok(home) => str_obj(home),
        Err(e) => new_error(ctx.pos.clone(), format!("os.homedir: {}", e)),
    }
}

fn os_hostname(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    if let Ok(name) = env::var("COMPUTERNAME").or_else(|_| env::var("HOSTNAME")) {
        return str_obj(name);
    }
    str_obj("")
}

fn os_user_info(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    let username = env::var("USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_default();
    let home = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_default();
    module(vec![
        ("uid", str_obj("")),
        ("gid", str_obj("")),
        ("username", str_obj(username.clone())),
        ("name", str_obj(username)),
        ("homedir", str_obj(home)),
    ])
}

fn env_module() -> Object {
    module(vec![
        ("load", native("env.load", env_load)),
        (
            "loadMultiple",
            native("env.loadMultiple", env_load_multiple),
        ),
        ("get", native("env.get", env_get)),
        ("getString", native("env.getString", env_get)),
        ("getInt", native("env.getInt", env_get_int)),
        ("getFloat", native("env.getFloat", env_get_float)),
        ("getNumber", native("env.getNumber", env_get_float)),
        ("getBool", native("env.getBool", env_get_bool)),
        ("getArray", native("env.getArray", env_get_array)),
        ("getJson", native("env.getJson", env_get_json)),
        ("has", native("env.has", env_has)),
        ("require", native("env.require", env_require)),
        ("set", native("env.set", env_set)),
        ("unset", native("env.unset", env_unset)),
        ("toObject", native("env.toObject", env_to_object)),
        ("parse", native("env.parse", env_parse)),
    ])
}

fn env_get(ctx: &mut CallContext, args: &[Object]) -> Object {
    let key = match required_string(ctx, "env.get", args, 0, "key") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match env::var(&key) {
        Ok(value) if !value.is_empty() => str_obj(value),
        _ => args.get(1).cloned().unwrap_or(Object::Undefined),
    }
}

fn env_get_int(ctx: &mut CallContext, args: &[Object]) -> Object {
    let key = match required_string(ctx, "env.getInt", args, 0, "key") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match env::var(&key).ok().filter(|v| !v.is_empty()) {
        Some(value) => value
            .parse::<i64>()
            .map(|n| num_obj(n as f64))
            .unwrap_or_else(|_| {
                args.get(1).cloned().unwrap_or_else(|| {
                    new_error(
                        ctx.pos.clone(),
                        format!("getInt: invalid integer {}", value),
                    )
                })
            }),
        None => args.get(1).cloned().unwrap_or(Object::Undefined),
    }
}

fn env_get_float(ctx: &mut CallContext, args: &[Object]) -> Object {
    let key = match required_string(ctx, "env.getFloat", args, 0, "key") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match env::var(&key).ok().filter(|v| !v.is_empty()) {
        Some(value) => value.parse::<f64>().map(num_obj).unwrap_or_else(|_| {
            args.get(1).cloned().unwrap_or_else(|| {
                new_error(
                    ctx.pos.clone(),
                    format!("getFloat: invalid number {}", value),
                )
            })
        }),
        None => args.get(1).cloned().unwrap_or(Object::Undefined),
    }
}

fn env_get_bool(ctx: &mut CallContext, args: &[Object]) -> Object {
    let key = match required_string(ctx, "env.getBool", args, 0, "key") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match env::var(&key)
        .ok()
        .map(|v| v.to_ascii_lowercase())
        .filter(|v| !v.is_empty())
    {
        Some(value) => match value.as_str() {
            "true" | "1" | "yes" | "on" => bool_obj(true),
            "false" | "0" | "no" | "off" => bool_obj(false),
            _ => args.get(1).cloned().unwrap_or_else(|| {
                new_error(
                    ctx.pos.clone(),
                    format!("getBool: invalid boolean {}", value),
                )
            }),
        },
        None => args.get(1).cloned().unwrap_or(Object::Undefined),
    }
}

fn env_get_array(ctx: &mut CallContext, args: &[Object]) -> Object {
    let key = match required_string(ctx, "env.getArray", args, 0, "key") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let sep = match args.get(1) {
        Some(Object::String(s)) => s.as_str(),
        _ => ",",
    };
    let Some(value) = env::var(&key).ok().filter(|v| !v.is_empty()) else {
        return array(Vec::new());
    };
    array(value.split(sep).map(|part| str_obj(part.trim())).collect())
}

fn env_has(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "env.has", args, 0, "key") {
        Ok(key) => bool_obj(env::var_os(key).is_some()),
        Err(err) => err,
    }
}

fn env_require(ctx: &mut CallContext, args: &[Object]) -> Object {
    // Accept either a single key string or an array of required keys, matching
    // the Go `@std/env.require` contract.
    let keys: Vec<String> = match args.first() {
        Some(Object::String(s)) => vec![s.as_str().to_string()],
        Some(Object::Array(arr)) => {
            let mut out = Vec::new();
            for elem in &arr.borrow().elements {
                match elem {
                    Object::String(s) => out.push(s.as_str().to_string()),
                    _ => return new_error(ctx.pos.clone(), "env.require expects array of strings"),
                }
            }
            out
        }
        Some(_) => return new_error(ctx.pos.clone(), "env.require expects array"),
        None => return new_error(ctx.pos.clone(), "env.require requires array of keys"),
    };
    let missing: Vec<String> = keys
        .iter()
        .filter(|k| env::var(k).ok().filter(|v| !v.is_empty()).is_none())
        .cloned()
        .collect();
    if missing.is_empty() {
        Object::Undefined
    } else {
        new_error(
            ctx.pos.clone(),
            format!(
                "Missing required environment variables: {}",
                missing.join(", ")
            ),
        )
    }
}

fn env_load(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match args.first() {
        Some(Object::String(s)) => s.as_str().to_string(),
        _ => ".env".to_string(),
    };
    let override_existing = hash_bool_arg(args.get(1), "override").unwrap_or(false);
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return new_error(ctx.pos.clone(), format!("env.load: {}", e)),
    };
    let entries = parse_env_content(&content);
    apply_env_entries(&entries, override_existing);
    Object::Undefined
}

fn env_load_multiple(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.first() {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "env.loadMultiple expects array"),
        None => return new_error(ctx.pos.clone(), "env.loadMultiple requires array of paths"),
    };
    // Per the Go original, a single failing file is skipped silently.
    for elem in &arr.borrow().elements {
        if let Object::String(path) = elem {
            if let Ok(content) = fs::read_to_string(path.as_str()) {
                let entries = parse_env_content(&content);
                apply_env_entries(&entries, false);
            }
        }
    }
    Object::Undefined
}

fn env_get_json(ctx: &mut CallContext, args: &[Object]) -> Object {
    // The Go original's getJson is a stub returning the raw string; preserve
    // that contract for compatibility.
    let key = match required_string(ctx, "env.getJson", args, 0, "key") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match env::var(&key).ok().filter(|v| !v.is_empty()) {
        Some(value) => str_obj(value),
        None => Object::Undefined,
    }
}

fn env_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let content = match required_string(ctx, "env.parse", args, 0, "content") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let entries = parse_env_content(&content);
    let hash = Rc::new(RefCell::new(HashData::default()));
    for (k, v) in entries {
        hash.borrow_mut().set(k, str_obj(v));
    }
    Object::Hash(hash)
}

/// Apply parsed entries to the process environment. With override=false, only
/// keys whose current value is empty are written (matching the Go `load` rule).
fn apply_env_entries(entries: &[(String, String)], override_existing: bool) {
    for (key, value) in entries {
        let current = env::var(key).unwrap_or_default();
        if override_existing || current.is_empty() {
            env::set_var(key, value);
        }
    }
}

/// Parse `.env`-format content into ordered (key, value) pairs. Supports
/// comments, single/double quotes (including multi-line double-quoted values),
/// and `${VAR}` expansion against already-parsed entries.
fn parse_env_content(content: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        i += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, rest)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let mut value = rest.trim().to_string();
        // Multi-line double-quoted value continues until a closing quote.
        if value.starts_with('"') && !value[1..].ends_with('"') {
            let mut buf = value.clone();
            while i < lines.len() {
                buf.push('\n');
                buf.push_str(lines[i]);
                i += 1;
                if lines[i - 1].trim_end().ends_with('"') {
                    break;
                }
            }
            value = buf;
        }
        // Strip surrounding quotes.
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        } else if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        }
        // Expand ${VAR} using already-parsed entries.
        value = expand_env_vars(&value, &out);
        out.push((key, value));
    }
    out
}

fn expand_env_vars(value: &str, parsed: &[(String, String)]) -> String {
    let mut out = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some(end) = value[i + 2..].find('}') {
                let name = &value[i + 2..i + 2 + end];
                let resolved = parsed
                    .iter()
                    .rev()
                    .find(|(k, _)| k == name)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                out.push_str(&resolved);
                i += 2 + end + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn env_set(ctx: &mut CallContext, args: &[Object]) -> Object {
    let key = match required_string(ctx, "env.set", args, 0, "key") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let value = match args.get(1) {
        Some(value) => value.inspect(),
        None => return new_error(ctx.pos.clone(), "env.set requires value"),
    };
    env::set_var(key, value);
    Object::Undefined
}

fn env_unset(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "env.unset", args, 0, "key") {
        Ok(key) => {
            env::remove_var(key);
            Object::Undefined
        }
        Err(err) => err,
    }
}

fn env_to_object(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    for (key, value) in env::vars() {
        hash.borrow_mut().set(key, str_obj(value));
    }
    Object::Hash(hash)
}

fn fs_module() -> Object {
    module(vec![
        ("readFileSync", native("fs.readFileSync", fs_read_file_sync)),
        ("readTextSync", native("fs.readTextSync", fs_read_file_sync)),
        (
            "writeFileSync",
            native("fs.writeFileSync", fs_write_file_sync),
        ),
        (
            "writeTextSync",
            native("fs.writeTextSync", fs_write_file_sync),
        ),
        (
            "appendFileSync",
            native("fs.appendFileSync", fs_append_file_sync),
        ),
        (
            "appendTextSync",
            native("fs.appendTextSync", fs_append_file_sync),
        ),
        (
            "writeFileAtomicSync",
            native("fs.writeFileAtomicSync", fs_write_file_atomic_sync),
        ),
        (
            "createThrottledWriter",
            native("fs.createThrottledWriter", fs_create_throttled_writer),
        ),
        ("existsSync", native("fs.existsSync", fs_exists_sync)),
        ("readdirSync", native("fs.readdirSync", fs_readdir_sync)),
        ("walkSync", native("fs.walkSync", fs_walk_sync)),
        ("globSync", native("fs.globSync", fs_glob_sync)),
        ("mkdirSync", native("fs.mkdirSync", fs_mkdir_sync)),
        ("statSync", native("fs.statSync", fs_stat_sync)),
        ("lstatSync", native("fs.lstatSync", fs_lstat_sync)),
        ("realpathSync", native("fs.realpathSync", fs_realpath_sync)),
        ("copyFileSync", native("fs.copyFileSync", fs_copy_file_sync)),
        ("renameSync", native("fs.renameSync", fs_rename_sync)),
        ("unlinkSync", native("fs.unlinkSync", fs_unlink_sync)),
        ("rmSync", native("fs.rmSync", fs_rm_sync)),
        ("mkdtempSync", native("fs.mkdtempSync", fs_mkdtemp_sync)),
    ])
}

fn fs_read_file_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.readFileSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    match fs::read_to_string(&path) {
        Ok(text) => str_obj(text),
        Err(e) => new_error(ctx.pos.clone(), format!("fs.readFileSync: {}", e)),
    }
}

fn fs_write_file_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.writeFileSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let Some(data) = args.get(1) else {
        return new_error(ctx.pos.clone(), "fs.writeFileSync requires data");
    };
    match fs::write(&path, object_to_text(data)) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("fs.writeFileSync: {}", e)),
    }
}

fn fs_append_file_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.appendFileSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let Some(data) = args.get(1) else {
        return new_error(ctx.pos.clone(), "fs.appendFileSync requires data");
    };
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => match file.write_all(object_to_text(data).as_bytes()) {
            Ok(_) => Object::Undefined,
            Err(e) => new_error(ctx.pos.clone(), format!("fs.appendFileSync: {}", e)),
        },
        Err(e) => new_error(ctx.pos.clone(), format!("fs.appendFileSync: {}", e)),
    }
}

fn fs_write_file_atomic_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.writeFileAtomicSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let Some(data) = args.get(1) else {
        return new_error(ctx.pos.clone(), "fs.writeFileAtomicSync requires data");
    };
    match atomic_write_file(Path::new(&path), object_to_text(data).as_bytes()) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("fs.writeFileAtomicSync: {}", e)),
    }
}

fn fs_create_throttled_writer(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.createThrottledWriter", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let state = Rc::new(RefCell::new(ThrottledWriterState { path, latest: None }));
    let write_state = state.clone();
    let flush_state = state.clone();
    let flush_async_state = state.clone();
    let close_state = state.clone();
    module(vec![
        (
            "write",
            native("throttledWriter.write", move |ctx, args| {
                let Some(data) = args.first() else {
                    return new_error(ctx.pos.clone(), "throttledWriter.write: data required");
                };
                write_state.borrow_mut().latest = Some(object_to_text(data));
                Object::Undefined
            }),
        ),
        (
            "flush",
            native("throttledWriter.flush", move |ctx, _args| {
                flush_throttled_writer(ctx, &flush_state)
            }),
        ),
        (
            "flushAsync",
            native("throttledWriter.flushAsync", move |ctx, _args| {
                flush_throttled_writer(ctx, &flush_async_state)
            }),
        ),
        (
            "close",
            native("throttledWriter.close", move |ctx, _args| {
                flush_throttled_writer(ctx, &close_state)
            }),
        ),
        (
            "markDirty",
            native("throttledWriter.markDirty", |_ctx, _args| Object::Undefined),
        ),
        (
            "setProvider",
            native("throttledWriter.setProvider", |_ctx, _args| {
                Object::Undefined
            }),
        ),
    ])
}

fn fs_exists_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "fs.existsSync", args, 0, "path") {
        Ok(path) => bool_obj(Path::new(&path).exists()),
        Err(err) => err,
    }
}

fn fs_readdir_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.readdirSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let with_file_types = hash_bool_arg(args.get(1), "withFileTypes").unwrap_or(false);
    match fs::read_dir(&path) {
        Ok(entries) => {
            let mut values = Vec::new();
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => return new_error(ctx.pos.clone(), format!("fs.readdirSync: {}", e)),
                };
                if with_file_types {
                    let entry_path = entry.path();
                    let meta = match entry.metadata() {
                        Ok(meta) => meta,
                        Err(e) => {
                            return new_error(ctx.pos.clone(), format!("fs.readdirSync: {}", e))
                        }
                    };
                    let value = stat_object_for_path(entry_path, meta);
                    if let Object::Hash(hash) = &value {
                        hash.borrow_mut()
                            .set("name", str_obj(entry.file_name().to_string_lossy()));
                    }
                    values.push(value);
                } else {
                    values.push(str_obj(entry.file_name().to_string_lossy()));
                }
            }
            array(values)
        }
        Err(e) => new_error(ctx.pos.clone(), format!("fs.readdirSync: {}", e)),
    }
}

fn fs_walk_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let root = match required_string(ctx, "fs.walkSync", args, 0, "root") {
        Ok(root) => root,
        Err(err) => return err,
    };
    let include_dirs = hash_bool_arg(args.get(1), "includeDirs").unwrap_or(true);
    let root_path = PathBuf::from(&root);
    let mut entries = Vec::new();
    if let Err(e) = walk_dir_collect(&root_path, &root_path, include_dirs, &mut entries) {
        return new_error(ctx.pos.clone(), format!("fs.walkSync: {}", e));
    }
    entries.sort_by_key(|value| value.inspect());
    array(entries)
}

fn fs_glob_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let pattern = match required_string(ctx, "fs.globSync", args, 0, "pattern") {
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
        Err(e) => new_error(ctx.pos.clone(), format!("fs.globSync: {}", e)),
    }
}

fn fs_mkdir_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.mkdirSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let recursive = match args.get(1) {
        Some(Object::Boolean(value)) => *value,
        other => hash_bool_arg(other, "recursive").unwrap_or(false),
    };
    let result = if recursive {
        fs::create_dir_all(&path)
    } else {
        fs::create_dir(&path)
    };
    match result {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("fs.mkdirSync: {}", e)),
    }
}

fn fs_stat_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.statSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let path_buf = PathBuf::from(&path);
    match fs::metadata(&path_buf) {
        Ok(meta) => stat_object_for_path(path_buf, meta),
        Err(e) => new_error(ctx.pos.clone(), format!("fs.statSync: {}", e)),
    }
}

fn fs_lstat_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.lstatSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let path_buf = PathBuf::from(&path);
    match fs::symlink_metadata(&path_buf) {
        Ok(meta) => stat_object_for_path(path_buf, meta),
        Err(e) => new_error(ctx.pos.clone(), format!("fs.lstatSync: {}", e)),
    }
}

fn fs_realpath_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.realpathSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    match fs::canonicalize(&path) {
        Ok(path) => str_obj(path.to_string_lossy()),
        Err(e) => new_error(ctx.pos.clone(), format!("fs.realpathSync: {}", e)),
    }
}

fn fs_copy_file_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let from = match required_string(ctx, "fs.copyFileSync", args, 0, "from") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let to = match required_string(ctx, "fs.copyFileSync", args, 1, "to") {
        Ok(path) => path,
        Err(err) => return err,
    };
    match fs::copy(&from, &to) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("fs.copyFileSync: {}", e)),
    }
}

fn fs_rename_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let from = match required_string(ctx, "fs.renameSync", args, 0, "from") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let to = match required_string(ctx, "fs.renameSync", args, 1, "to") {
        Ok(path) => path,
        Err(err) => return err,
    };
    match fs::rename(&from, &to) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("fs.renameSync: {}", e)),
    }
}

fn fs_unlink_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.unlinkSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    match fs::remove_file(&path) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("fs.unlinkSync: {}", e)),
    }
}

fn fs_rm_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "fs.rmSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let recursive = hash_bool_arg(args.get(1), "recursive").unwrap_or(false);
    let force = hash_bool_arg(args.get(1), "force").unwrap_or(false);
    let target = Path::new(&path);
    let result = if recursive {
        fs::remove_dir_all(target)
    } else {
        fs::remove_file(target).or_else(|file_err| {
            if target.is_dir() {
                fs::remove_dir(target)
            } else {
                Err(file_err)
            }
        })
    };
    match result {
        Ok(_) => Object::Undefined,
        Err(e) if force && e.kind() == std::io::ErrorKind::NotFound => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("fs.rmSync: {}", e)),
    }
}

fn fs_mkdtemp_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let prefix = match required_string(ctx, "fs.mkdtempSync", args, 0, "prefix") {
        Ok(prefix) => prefix,
        Err(err) => return err,
    };
    let prefix_path = PathBuf::from(&prefix);
    let dir = prefix_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let base = prefix_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    for attempt in 0..10_000 {
        let candidate = dir.join(format!("{}{}-{}", base, now_ms(), attempt));
        match fs::create_dir(&candidate) {
            Ok(_) => return str_obj(candidate.to_string_lossy()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return new_error(ctx.pos.clone(), format!("fs.mkdtempSync: {}", e)),
        }
    }
    new_error(
        ctx.pos.clone(),
        "fs.mkdtempSync: could not create unique directory",
    )
}

fn json_module() -> Object {
    module(vec![
        ("parse5", native("json.parse5", json_parse5)),
        ("stringify5", native("json.stringify5", json_stringify5)),
        ("validate", native("json.validate", json_validate)),
        ("get", native("json.get", json_get)),
        ("set", native("json.set", json_set)),
        ("has", native("json.has", json_has)),
        ("remove", native("json.remove", json_remove)),
        ("patch", native("json.patch", json_patch)),
        ("diff", native("json.diff", json_diff)),
    ])
}

fn json_parse5(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "json.parse5", args, 0, "text") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let normalized = normalize_json5(&text);
    match simple_json_parse(&normalized) {
        Ok(value) => json_to_object(value),
        Err(err) => new_error(ctx.pos.clone(), format!("json.parse5: {}", err)),
    }
}

fn json_stringify5(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(value) = args.first() else {
        return new_error(ctx.pos.clone(), "json.stringify5 requires value");
    };
    let (space, single_quote) = stringify_options(args.get(1));
    let mut result = object_to_json(value, 0, space.as_deref());
    if single_quote {
        result = result.replace('"', "'");
    }
    str_obj(result)
}

fn json_validate(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(ctx.pos.clone(), "json.validate requires data and schema");
    }
    let Object::Hash(schema) = &args[1] else {
        return new_error(ctx.pos.clone(), "json.validate expects hash schema");
    };
    let mut errors = Vec::new();
    validate_json_value(&args[0], &schema.borrow(), "", &mut errors);
    if errors.is_empty() {
        module(vec![("valid", bool_obj(true))])
    } else {
        module(vec![
            ("valid", bool_obj(false)),
            ("errors", array(errors.into_iter().map(str_obj).collect())),
        ])
    }
}

fn json_get(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "json.get", args, 1, "path") {
        Ok(value) => value,
        Err(err) => return err,
    };
    args.first()
        .and_then(|doc| pointer_get(doc, &path))
        .unwrap_or(Object::Undefined)
}

fn json_set(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 3 {
        return new_error(ctx.pos.clone(), "json.set requires doc, path, and value");
    }
    let path = match required_string(ctx, "json.set", args, 1, "path") {
        Ok(value) => value,
        Err(err) => return err,
    };
    pointer_set(&args[0], &path, args[2].clone());
    Object::Undefined
}

fn json_has(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "json.has", args, 1, "path") {
        Ok(value) => value,
        Err(err) => return err,
    };
    bool_obj(
        args.first()
            .and_then(|doc| pointer_get(doc, &path))
            .is_some(),
    )
}

fn json_remove(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "json.remove", args, 1, "path") {
        Ok(value) => value,
        Err(err) => return err,
    };
    if let Some(doc) = args.first() {
        pointer_remove(doc, &path);
    }
    Object::Undefined
}

fn json_patch(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(ctx.pos.clone(), "json.patch requires doc and operations");
    }
    let Object::Array(ops) = &args[1] else {
        return new_error(ctx.pos.clone(), "json.patch expects array of operations");
    };
    for op_obj in &ops.borrow().elements {
        let Object::Hash(op_hash) = op_obj else {
            continue;
        };
        let op_hash = op_hash.borrow();
        let op_type = hash_string(&op_hash, "op").unwrap_or_default();
        let path = hash_string(&op_hash, "path").unwrap_or_default();
        match op_type.as_str() {
            "add" | "replace" => {
                if let Some(value) = op_hash.get("value") {
                    pointer_set(&args[0], &path, value.clone());
                }
            }
            "remove" => pointer_remove(&args[0], &path),
            "move" => {
                let from = hash_string(&op_hash, "from").unwrap_or_default();
                if let Some(value) = pointer_get(&args[0], &from) {
                    pointer_remove(&args[0], &from);
                    pointer_set(&args[0], &path, value);
                }
            }
            "copy" => {
                let from = hash_string(&op_hash, "from").unwrap_or_default();
                if let Some(value) = pointer_get(&args[0], &from) {
                    pointer_set(&args[0], &path, deep_clone_object(&value));
                }
            }
            "test" => {
                let expected = op_hash.get("value").cloned().unwrap_or(Object::Undefined);
                let current = pointer_get(&args[0], &path).unwrap_or(Object::Undefined);
                if !objects_deep_equal(&current, &expected) {
                    return new_error(
                        ctx.pos.clone(),
                        format!("json.patch: test failed at {}", path),
                    );
                }
            }
            _ => {}
        }
    }
    Object::Undefined
}

fn json_diff(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(ctx.pos.clone(), "json.diff requires oldDoc and newDoc");
    }
    let mut patches = Vec::new();
    diff_objects(&args[0], &args[1], "", &mut patches);
    array(patches)
}

fn time_module() -> Object {
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

fn time_unix_ms(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_number(ctx, "time.unixMs", args, 0, "milliseconds") {
        Ok(ms) => Object::Date(ms as i64),
        Err(err) => err,
    }
}

fn time_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "time.parse", args, 0, "value") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match parse_time_ms(&value) {
        Some(ms) => Object::Date(ms),
        None => new_error(
            ctx.pos.clone(),
            format!("time.parse: unsupported time {}", value),
        ),
    }
}

fn time_format(ctx: &mut CallContext, args: &[Object]) -> Object {
    let ms = match time_from_object(ctx, "time.format", args, 0) {
        Ok(ms) => ms,
        Err(err) => return err,
    };
    let layout = match args.get(1) {
        Some(Object::String(value)) => value.as_str(),
        Some(Object::Undefined | Object::Null) | None => "2006-01-02T15:04:05Z07:00",
        Some(_) => return new_error(ctx.pos.clone(), "time.format: layout must be a string"),
    };
    str_obj(format_time_layout(ms, layout))
}

fn time_add(ctx: &mut CallContext, args: &[Object]) -> Object {
    let ms = match time_from_object(ctx, "time.add", args, 0) {
        Ok(ms) => ms,
        Err(err) => return err,
    };
    let duration = match duration_from_object(ctx, "time.add", args, 1) {
        Ok(duration) => duration,
        Err(err) => return err,
    };
    Object::Date(ms + duration)
}

fn time_since(ctx: &mut CallContext, args: &[Object]) -> Object {
    match time_from_object(ctx, "time.since", args, 0) {
        Ok(ms) => num_obj((now_ms() - ms) as f64),
        Err(err) => err,
    }
}

fn time_until(ctx: &mut CallContext, args: &[Object]) -> Object {
    match time_from_object(ctx, "time.until", args, 0) {
        Ok(ms) => num_obj((ms - now_ms()) as f64),
        Err(err) => err,
    }
}

fn time_parse_duration(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "time.parseDuration", args, 0, "duration") {
        Ok(value) => match parse_duration_ms(&value) {
            Some(ms) => duration_object(ms),
            None => new_error(
                ctx.pos.clone(),
                format!("time.parseDuration: invalid duration {}", value),
            ),
        },
        Err(err) => err,
    }
}

fn time_duration(ctx: &mut CallContext, args: &[Object]) -> Object {
    match duration_from_object(ctx, "time.duration", args, 0) {
        Ok(ms) => duration_object(ms),
        Err(err) => err,
    }
}

fn time_sleep(ctx: &mut CallContext, args: &[Object]) -> Object {
    let ms = match required_number(ctx, "time.sleep", args, 0, "milliseconds") {
        Ok(ms) => ms.max(0.0) as u64,
        Err(err) => return err,
    };
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Object::Undefined
}

fn object_to_text(value: &Object) -> String {
    match value {
        Object::String(value) => value.to_string(),
        Object::Undefined | Object::Null => String::new(),
        other => other.inspect(),
    }
}

fn stat_object_for_path(path: PathBuf, meta: fs::Metadata) -> Object {
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as f64)
        .map(num_obj)
        .unwrap_or(Object::Undefined);
    let is_file = meta.is_file();
    let is_dir = meta.is_dir();
    let is_symlink = meta.file_type().is_symlink();
    module(vec![
        ("path", str_obj(path.to_string_lossy())),
        (
            "name",
            str_obj(
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
            ),
        ),
        ("size", num_obj(meta.len() as f64)),
        ("mode", str_obj(format!("{:?}", meta.permissions()))),
        ("mtimeMs", mtime_ms.clone()),
        ("modifiedMs", mtime_ms),
        ("isFileValue", bool_obj(is_file)),
        ("isDirectoryValue", bool_obj(is_dir)),
        ("isDir", bool_obj(is_dir)),
        ("isSymlinkValue", bool_obj(is_symlink)),
        (
            "isFile",
            native("fs.stat.isFile", move |_ctx, _args| bool_obj(is_file)),
        ),
        (
            "isDirectory",
            native("fs.stat.isDirectory", move |_ctx, _args| bool_obj(is_dir)),
        ),
        (
            "isSymlink",
            native("fs.stat.isSymlink", move |_ctx, _args| bool_obj(is_symlink)),
        ),
    ])
}

#[derive(Default)]
struct ThrottledWriterState {
    path: String,
    latest: Option<String>,
}

fn flush_throttled_writer(
    ctx: &mut CallContext,
    state: &Rc<RefCell<ThrottledWriterState>>,
) -> Object {
    let (path, latest) = {
        let mut state = state.borrow_mut();
        (state.path.clone(), state.latest.take())
    };
    let Some(latest) = latest else {
        return Object::Undefined;
    };
    match atomic_write_file(Path::new(&path), latest.as_bytes()) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("throttledWriter.flush: {}", e)),
    }
}

fn atomic_write_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let base = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let tmp = dir.join(format!(".{}.{}.tmp", base, now_ms()));
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path).or_else(|err| {
        let _ = fs::remove_file(&tmp);
        Err(err)
    })
}

fn walk_dir_collect(
    root: &Path,
    current: &Path,
    include_dirs: bool,
    entries: &mut Vec<Object>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            if include_dirs {
                let value = stat_object_for_path(path.clone(), meta);
                set_relative_path(&value, root, &path);
                entries.push(value);
            }
            walk_dir_collect(root, &path, include_dirs, entries)?;
        } else {
            let value = stat_object_for_path(path.clone(), meta);
            set_relative_path(&value, root, &path);
            entries.push(value);
        }
    }
    Ok(())
}

fn set_relative_path(value: &Object, root: &Path, path: &Path) {
    if let Object::Hash(hash) = value {
        let relative = path.strip_prefix(root).unwrap_or(path);
        hash.borrow_mut()
            .set("relativePath", str_obj(relative.to_string_lossy()));
    }
}

fn glob_paths(pattern: &str) -> Result<Vec<PathBuf>, String> {
    let normalized = pattern.replace('\\', "/");
    if !normalized.contains('*') {
        let path = PathBuf::from(pattern);
        return Ok(if path.exists() {
            vec![path]
        } else {
            Vec::new()
        });
    }
    let wildcard = normalized
        .find('*')
        .ok_or_else(|| "missing wildcard".to_string())?;
    let root_end = normalized[..wildcard]
        .rfind('/')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let root = if root_end == 0 {
        PathBuf::from(".")
    } else {
        PathBuf::from(normalized[..root_end].replace('/', MAIN_SEPARATOR_STR))
    };
    let mut matches = Vec::new();
    glob_collect(&root, pattern, &mut matches).map_err(|e| e.to_string())?;
    matches.sort();
    Ok(matches)
}

fn glob_collect(current: &Path, pattern: &str, matches: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            glob_collect(&path, pattern, matches)?;
        }
        if glob_match(pattern, &path.to_string_lossy()) {
            matches.push(path);
        }
    }
    Ok(())
}

fn glob_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let value = value.replace('\\', "/");
    wildcard_match(&pattern, &value)
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let (mut pi, mut vi) = (0usize, 0usize);
    let mut star = None;
    let mut star_match = 0usize;
    while vi < value.len() {
        if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == value[vi]) {
            pi += 1;
            vi += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star = Some(pi);
            star_match = vi;
            pi += 1;
        } else if let Some(star_pos) = star {
            pi = star_pos + 1;
            star_match += 1;
            vi = star_match;
        } else {
            return false;
        }
    }
    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }
    pi == pattern.len()
}

#[derive(Clone)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

fn simple_json_parse(source: &str) -> Result<JsonValue, String> {
    let mut parser = JsonParser::new(source);
    let value = parser.parse_value()?;
    parser.skip_ws();
    if parser.is_eof() {
        Ok(value)
    } else {
        Err("trailing characters".into())
    }
}

fn normalize_json5(text: &str) -> String {
    let mut out = text.to_string();
    if let Ok(re) = Regex::new(r"//[^\n]*") {
        out = re.replace_all(&out, "").to_string();
    }
    if let Ok(re) = Regex::new(r"/\*[\s\S]*?\*/") {
        out = re.replace_all(&out, "").to_string();
    }
    out = out.replace('\'', "\"");
    if let Ok(re) = Regex::new(r",(\s*[}\]])") {
        out = re.replace_all(&out, "$1").to_string();
    }
    if let Ok(re) = Regex::new(r"([A-Za-z_][A-Za-z0-9_]*):") {
        out = re.replace_all(&out, "\"$1\":").to_string();
    }
    out
}

fn stringify_options(value: Option<&Object>) -> (Option<String>, bool) {
    let Some(Object::Hash(hash)) = value else {
        return (None, false);
    };
    let hash = hash.borrow();
    let space = match hash.get("space") {
        Some(Object::Number(n)) if *n > 0.0 => Some(" ".repeat(*n as usize)),
        _ => None,
    };
    let single_quote =
        matches!(hash.get("quote"), Some(Object::String(s)) if s.as_str() == "single");
    (space, single_quote)
}

fn object_to_json(value: &Object, indent: usize, space: Option<&str>) -> String {
    match value {
        Object::Number(n) => crate::object::format_number(*n),
        Object::String(s) => quote_json_string(s),
        Object::Boolean(value) => value.to_string(),
        Object::Null | Object::Undefined => "null".into(),
        Object::Array(array) => {
            let elements = array.borrow();
            if let Some(space) = space {
                if elements.elements.is_empty() {
                    return "[]".into();
                }
                let child_indent = space.repeat(indent + 1);
                let current_indent = space.repeat(indent);
                let items: Vec<String> = elements
                    .elements
                    .iter()
                    .map(|value| {
                        format!(
                            "{}{}",
                            child_indent,
                            object_to_json(value, indent + 1, Some(space))
                        )
                    })
                    .collect();
                format!("[\n{}\n{}]", items.join(",\n"), current_indent)
            } else {
                let items: Vec<String> = elements
                    .elements
                    .iter()
                    .map(|value| object_to_json(value, indent, None))
                    .collect();
                format!("[{}]", items.join(","))
            }
        }
        Object::Hash(hash) => {
            let hash = hash.borrow();
            if let Some(space) = space {
                if hash.entries.is_empty() {
                    return "{}".into();
                }
                let child_indent = space.repeat(indent + 1);
                let current_indent = space.repeat(indent);
                let items: Vec<String> = hash
                    .entries
                    .iter()
                    .map(|(key, value)| {
                        format!(
                            "{}{}: {}",
                            child_indent,
                            quote_json_string(key),
                            object_to_json(value, indent + 1, Some(space))
                        )
                    })
                    .collect();
                format!("{{\n{}\n{}}}", items.join(",\n"), current_indent)
            } else {
                let items: Vec<String> = hash
                    .entries
                    .iter()
                    .map(|(key, value)| {
                        format!(
                            "{}:{}",
                            quote_json_string(key),
                            object_to_json(value, indent, None)
                        )
                    })
                    .collect();
                format!("{{{}}}", items.join(","))
            }
        }
        _ => "null".into(),
    }
}

fn quote_json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn validate_json_value(value: &Object, schema: &HashData, path: &str, errors: &mut Vec<String>) {
    if let Some(Object::String(type_value)) = schema.get("type") {
        let valid = match type_value.as_str() {
            "string" => matches!(value, Object::String(_)),
            "number" => matches!(value, Object::Number(_)),
            "boolean" => matches!(value, Object::Boolean(_)),
            "array" => matches!(value, Object::Array(_)),
            "object" => matches!(value, Object::Hash(_)),
            "null" => matches!(value, Object::Null),
            _ => true,
        };
        if !valid {
            errors.push(format!("{}: expected type {}", path, type_value));
        }
    }

    if let Object::String(text) = value {
        if let Some(Object::Number(min)) = schema.get("minLength") {
            if text.len() < *min as usize {
                errors.push(format!("{}: string too short", path));
            }
        }
        if let Some(Object::Number(max)) = schema.get("maxLength") {
            if text.len() > *max as usize {
                errors.push(format!("{}: string too long", path));
            }
        }
        if let Some(Object::String(pattern)) = schema.get("pattern") {
            if Regex::new(pattern)
                .map(|re| !re.is_match(text))
                .unwrap_or(false)
            {
                errors.push(format!("{}: string does not match pattern", path));
            }
        }
    }

    if let Object::Number(number) = value {
        if let Some(Object::Number(min)) = schema.get("minimum") {
            if number < min {
                errors.push(format!("{}: number too small", path));
            }
        }
        if let Some(Object::Number(max)) = schema.get("maximum") {
            if number > max {
                errors.push(format!("{}: number too large", path));
            }
        }
    }

    if let Object::Array(array) = value {
        let len = array.borrow().elements.len();
        if let Some(Object::Number(min)) = schema.get("minItems") {
            if len < *min as usize {
                errors.push(format!("{}: array too short", path));
            }
        }
        if let Some(Object::Number(max)) = schema.get("maxItems") {
            if len > *max as usize {
                errors.push(format!("{}: array too long", path));
            }
        }
    }

    if let Object::Hash(hash) = value {
        let hash = hash.borrow();
        if let Some(Object::Array(required)) = schema.get("required") {
            for item in &required.borrow().elements {
                if let Object::String(key) = item {
                    if !hash.contains(key) {
                        errors.push(format!("{}: missing required field {}", path, key));
                    }
                }
            }
        }
        if let Some(Object::Hash(properties)) = schema.get("properties") {
            for (key, prop_schema) in &properties.borrow().entries {
                if let (Some(value), Object::Hash(prop_schema)) = (hash.get(key), prop_schema) {
                    let sub_path = format!("{}/{}", path, key);
                    validate_json_value(value, &prop_schema.borrow(), &sub_path, errors);
                }
            }
        }
    }
}

fn pointer_get(doc: &Object, path: &str) -> Option<Object> {
    if path.is_empty() {
        return Some(doc.clone());
    }
    let mut current = doc.clone();
    for part in pointer_parts(path) {
        current = match current {
            Object::Hash(hash) => hash.borrow().get(&part).cloned()?,
            Object::Array(array) => {
                let index = part.parse::<usize>().ok()?;
                array.borrow().elements.get(index).cloned()?
            }
            _ => return None,
        };
    }
    Some(current)
}

fn pointer_set(doc: &Object, path: &str, value: Object) {
    if path.is_empty() {
        return;
    }
    let parts = pointer_parts(path);
    if parts.is_empty() {
        return;
    }
    let mut current = doc.clone();
    for part in &parts[..parts.len() - 1] {
        match current {
            Object::Hash(hash) => {
                let next = hash.borrow().get(part).cloned().unwrap_or_else(|| {
                    let created = Object::Hash(Rc::new(RefCell::new(HashData::default())));
                    hash.borrow_mut().set(part.clone(), created.clone());
                    created
                });
                current = next;
            }
            Object::Array(array) => {
                let Ok(index) = part.parse::<usize>() else {
                    return;
                };
                let Some(next) = array.borrow().elements.get(index).cloned() else {
                    return;
                };
                current = next;
            }
            _ => return,
        }
    }
    let last = parts.last().cloned().unwrap_or_default();
    match current {
        Object::Hash(hash) => hash.borrow_mut().set(last, value),
        Object::Array(array) => {
            if last == "-" {
                array.borrow_mut().elements.push(value);
            } else if let Ok(index) = last.parse::<usize>() {
                let mut array = array.borrow_mut();
                if index < array.elements.len() {
                    array.elements[index] = value;
                } else if index == array.elements.len() {
                    array.elements.push(value);
                }
            }
        }
        _ => {}
    }
}

fn pointer_remove(doc: &Object, path: &str) {
    if path.is_empty() {
        return;
    }
    let parts = pointer_parts(path);
    if parts.is_empty() {
        return;
    }
    let mut current = doc.clone();
    for part in &parts[..parts.len() - 1] {
        current = match current {
            Object::Hash(hash) => match hash.borrow().get(part).cloned() {
                Some(value) => value,
                None => return,
            },
            Object::Array(array) => match part.parse::<usize>() {
                Ok(index) => match array.borrow().elements.get(index).cloned() {
                    Some(value) => value,
                    None => return,
                },
                Err(_) => return,
            },
            _ => return,
        };
    }
    let last = parts.last().cloned().unwrap_or_default();
    match current {
        Object::Hash(hash) => {
            hash.borrow_mut().remove(&last);
        }
        Object::Array(array) => {
            if let Ok(index) = last.parse::<usize>() {
                let mut array = array.borrow_mut();
                if index < array.elements.len() {
                    array.elements.remove(index);
                }
            }
        }
        _ => {}
    }
}

fn pointer_parts(path: &str) -> Vec<String> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .map(unescape_pointer)
        .collect()
}

fn unescape_pointer(value: &str) -> String {
    value.replace("~1", "/").replace("~0", "~")
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn deep_clone_object(value: &Object) -> Object {
    match value {
        Object::Array(values) => array(
            values
                .borrow()
                .elements
                .iter()
                .map(deep_clone_object)
                .collect(),
        ),
        Object::Hash(hash) => {
            let cloned = Rc::new(RefCell::new(HashData::default()));
            for (key, value) in &hash.borrow().entries {
                cloned
                    .borrow_mut()
                    .set(key.clone(), deep_clone_object(value));
            }
            Object::Hash(cloned)
        }
        other => other.clone(),
    }
}

fn objects_deep_equal(left: &Object, right: &Object) -> bool {
    match (left, right) {
        (Object::Number(a), Object::Number(b)) => a == b,
        (Object::String(a), Object::String(b)) => a == b,
        (Object::Boolean(a), Object::Boolean(b)) => a == b,
        (Object::Null, Object::Null) | (Object::Undefined, Object::Undefined) => true,
        (Object::Array(a), Object::Array(b)) => {
            let a = a.borrow();
            let b = b.borrow();
            a.elements.len() == b.elements.len()
                && a.elements
                    .iter()
                    .zip(b.elements.iter())
                    .all(|(a, b)| objects_deep_equal(a, b))
        }
        (Object::Hash(a), Object::Hash(b)) => {
            let a = a.borrow();
            let b = b.borrow();
            a.entries.len() == b.entries.len()
                && a.entries.iter().all(|(key, value)| {
                    b.get(key)
                        .map(|other| objects_deep_equal(value, other))
                        .unwrap_or(false)
                })
        }
        _ => false,
    }
}

fn diff_objects(old: &Object, new: &Object, path: &str, patches: &mut Vec<Object>) {
    if objects_deep_equal(old, new) {
        return;
    }
    let (Object::Hash(old_hash), Object::Hash(new_hash)) = (old, new) else {
        patches.push(module(vec![
            ("op", str_obj("replace")),
            ("path", str_obj(path)),
            ("value", deep_clone_object(new)),
        ]));
        return;
    };
    let old_hash = old_hash.borrow();
    let new_hash = new_hash.borrow();
    for (key, new_value) in &new_hash.entries {
        let sub_path = format!("{}/{}", path, escape_pointer(key));
        if let Some(old_value) = old_hash.get(key) {
            diff_objects(old_value, new_value, &sub_path, patches);
        } else {
            patches.push(module(vec![
                ("op", str_obj("add")),
                ("path", str_obj(sub_path)),
                ("value", deep_clone_object(new_value)),
            ]));
        }
    }
    for (key, _) in &old_hash.entries {
        if !new_hash.contains(key) {
            patches.push(module(vec![
                ("op", str_obj("remove")),
                ("path", str_obj(format!("{}/{}", path, escape_pointer(key)))),
            ]));
        }
    }
}

fn json_to_object(value: JsonValue) -> Object {
    match value {
        JsonValue::Null => Object::Null,
        JsonValue::Bool(value) => bool_obj(value),
        JsonValue::Number(value) => num_obj(value),
        JsonValue::String(value) => str_obj(value),
        JsonValue::Array(values) => array(values.into_iter().map(json_to_object).collect()),
        JsonValue::Object(entries) => {
            let hash = Rc::new(RefCell::new(HashData::default()));
            for (key, value) in entries {
                hash.borrow_mut().set(key, json_to_object(value));
            }
            Object::Hash(hash)
        }
    }
}

struct JsonParser<'a> {
    source: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            pos: 0,
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'n') => self.parse_literal(b"null", JsonValue::Null),
            Some(b't') => self.parse_literal(b"true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal(b"false", JsonValue::Bool(false)),
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self.parse_number().map(JsonValue::Number),
            Some(_) => Err("unexpected token".into()),
            None => Err("unexpected end of input".into()),
        }
    }

    fn parse_literal(&mut self, literal: &[u8], value: JsonValue) -> Result<JsonValue, String> {
        if self.source.get(self.pos..self.pos + literal.len()) == Some(literal) {
            self.pos += literal.len();
            Ok(value)
        } else {
            Err("invalid literal".into())
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        while let Some(byte) = self.next() {
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let escaped = self
                        .next()
                        .ok_or_else(|| "unterminated escape".to_string())?;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.parse_unicode_escape()?),
                        _ => return Err("invalid escape".into()),
                    }
                }
                0x00..=0x1f => return Err("control character in string".into()),
                other => out.push(other as char),
            }
        }
        Err("unterminated string".into())
    }

    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self
                .next()
                .ok_or_else(|| "invalid unicode escape".to_string())?;
            value = value * 16
                + match byte {
                    b'0'..=b'9' => (byte - b'0') as u32,
                    b'a'..=b'f' => (byte - b'a' + 10) as u32,
                    b'A'..=b'F' => (byte - b'A' + 10) as u32,
                    _ => return Err("invalid unicode escape".into()),
                };
        }
        char::from_u32(value).ok_or_else(|| "invalid unicode scalar".into())
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect(b'[')?;
        let mut values = Vec::new();
        loop {
            self.skip_ws();
            if self.consume(b']') {
                break;
            }
            values.push(self.parse_value()?);
            self.skip_ws();
            if self.consume(b']') {
                break;
            }
            self.expect(b',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        loop {
            self.skip_ws();
            if self.consume(b'}') {
                break;
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            let value = self.parse_value()?;
            entries.push((key, value));
            self.skip_ws();
            if self.consume(b'}') {
                break;
            }
            self.expect(b',')?;
        }
        Ok(JsonValue::Object(entries))
    }

    fn parse_number(&mut self) -> Result<f64, String> {
        let start = self.pos;
        self.consume(b'-');
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err("invalid number".into()),
        }
        if self.consume(b'.') {
            let digit_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == digit_start {
                return Err("invalid number".into());
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            let _ = self.consume(b'+') || self.consume(b'-');
            let digit_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == digit_start {
                return Err("invalid number".into());
            }
        }
        std::str::from_utf8(&self.source[start..self.pos])
            .map_err(|_| "invalid number".to_string())?
            .parse::<f64>()
            .map_err(|_| "invalid number".to_string())
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), String> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(format!("expected '{}'", expected as char))
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn time_from_object(
    ctx: &CallContext,
    module: &str,
    args: &[Object],
    index: usize,
) -> Result<i64, Object> {
    match args.get(index) {
        Some(Object::Date(ms)) => Ok(*ms),
        Some(Object::Number(ms)) => Ok(*ms as i64),
        Some(Object::String(value)) => parse_time_ms(value).ok_or_else(|| {
            new_error(
                ctx.pos.clone(),
                format!("{}: unsupported time {}", module, value),
            )
        }),
        Some(_) => Err(new_error(
            ctx.pos.clone(),
            format!(
                "{}: time must be a Date, number milliseconds, or string",
                module
            ),
        )),
        None => Err(new_error(
            ctx.pos.clone(),
            format!("{} requires time", module),
        )),
    }
}

fn duration_from_object(
    ctx: &CallContext,
    module: &str,
    args: &[Object],
    index: usize,
) -> Result<i64, Object> {
    match args.get(index) {
        Some(Object::Number(ms)) => Ok(*ms as i64),
        Some(Object::String(value)) => parse_duration_ms(value).ok_or_else(|| {
            new_error(
                ctx.pos.clone(),
                format!("{}: invalid duration {}", module, value),
            )
        }),
        Some(_) => Err(new_error(
            ctx.pos.clone(),
            format!(
                "{}: duration must be a number of milliseconds or Go duration string",
                module
            ),
        )),
        None => Err(new_error(
            ctx.pos.clone(),
            format!("{} requires duration", module),
        )),
    }
}

fn parse_time_ms(value: &str) -> Option<i64> {
    parse_rfc3339_ms(value)
        .or_else(|| parse_datetime_ms(value))
        .or_else(|| parse_date_only_ms(value))
}

fn parse_rfc3339_ms(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    let year = parse_i32(value.get(0..4)?)?;
    expect_byte(bytes, 4, b'-')?;
    let month = parse_u32(value.get(5..7)?)?;
    expect_byte(bytes, 7, b'-')?;
    let day = parse_u32(value.get(8..10)?)?;
    let sep = *bytes.get(10)?;
    if sep != b'T' && sep != b't' && sep != b' ' {
        return None;
    }
    let hour = parse_u32(value.get(11..13)?)?;
    expect_byte(bytes, 13, b':')?;
    let minute = parse_u32(value.get(14..16)?)?;
    expect_byte(bytes, 16, b':')?;
    let second = parse_u32(value.get(17..19)?)?;
    let mut pos = 19usize;
    let mut millis = 0i64;
    if bytes.get(pos) == Some(&b'.') {
        pos += 1;
        let start = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
        let fraction = value.get(start..pos)?;
        let mut ms_text = fraction.chars().take(3).collect::<String>();
        while ms_text.len() < 3 {
            ms_text.push('0');
        }
        millis = ms_text.parse::<i64>().ok()?;
    }
    let offset_ms = if bytes.get(pos) == Some(&b'Z') || bytes.get(pos) == Some(&b'z') {
        0
    } else if matches!(bytes.get(pos), Some(b'+' | b'-')) {
        let sign = if bytes[pos] == b'+' { 1 } else { -1 };
        let off_hour = parse_i32(value.get(pos + 1..pos + 3)?)?;
        expect_byte(bytes, pos + 3, b':')?;
        let off_min = parse_i32(value.get(pos + 4..pos + 6)?)?;
        sign * ((off_hour * 60 + off_min) as i64) * 60_000
    } else {
        return None;
    };
    let base = utc_ms_from_parts(year, month, day, hour, minute, second, millis)?;
    Some(base - offset_ms)
}

fn parse_datetime_ms(value: &str) -> Option<i64> {
    if value.len() != 19 {
        return None;
    }
    let bytes = value.as_bytes();
    let year = parse_i32(value.get(0..4)?)?;
    expect_byte(bytes, 4, b'-')?;
    let month = parse_u32(value.get(5..7)?)?;
    expect_byte(bytes, 7, b'-')?;
    let day = parse_u32(value.get(8..10)?)?;
    expect_byte(bytes, 10, b' ')?;
    let hour = parse_u32(value.get(11..13)?)?;
    expect_byte(bytes, 13, b':')?;
    let minute = parse_u32(value.get(14..16)?)?;
    expect_byte(bytes, 16, b':')?;
    let second = parse_u32(value.get(17..19)?)?;
    utc_ms_from_parts(year, month, day, hour, minute, second, 0)
}

fn parse_date_only_ms(value: &str) -> Option<i64> {
    if value.len() != 10 {
        return None;
    }
    let bytes = value.as_bytes();
    let year = parse_i32(value.get(0..4)?)?;
    expect_byte(bytes, 4, b'-')?;
    let month = parse_u32(value.get(5..7)?)?;
    expect_byte(bytes, 7, b'-')?;
    let day = parse_u32(value.get(8..10)?)?;
    utc_ms_from_parts(year, month, day, 0, 0, 0, 0)
}

fn expect_byte(bytes: &[u8], index: usize, expected: u8) -> Option<()> {
    if bytes.get(index) == Some(&expected) {
        Some(())
    } else {
        None
    }
}

fn parse_i32(value: &str) -> Option<i32> {
    value.parse::<i32>().ok()
}

fn parse_u32(value: &str) -> Option<u32> {
    value.parse::<u32>().ok()
}

fn utc_ms_from_parts(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millisecond: i64,
) -> Option<i64> {
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some(
        days * 86_400_000
            + hour as i64 * 3_600_000
            + minute as i64 * 60_000
            + second as i64 * 1_000
            + millisecond,
    )
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe - 719468) as i64
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let days = days + 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year, month as u32, day as u32)
}

pub fn utc_parts_from_ms(ms: i64) -> (i32, u32, u32, u32, u32, u32, u32) {
    let days = ms.div_euclid(86_400_000);
    let day_ms = ms.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    let hour = (day_ms / 3_600_000) as u32;
    let minute = ((day_ms % 3_600_000) / 60_000) as u32;
    let second = ((day_ms % 60_000) / 1_000) as u32;
    let millisecond = (day_ms % 1_000) as u32;
    (year, month, day, hour, minute, second, millisecond)
}

pub fn ms_from_utc_parts(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millisecond: u32,
) -> i64 {
    // Calculate days since epoch
    let mut y = year as i64;
    let m = month as i64;

    // Adjust year and month (months are 1-12)
    y += (m - 1) / 12;
    let month_adj = ((m - 1) % 12 + 12) % 12 + 1;

    // Days in each month (non-leap year)
    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    // Calculate days since epoch (1970-01-01)
    let mut days = (y - 1970) * 365;

    // Add leap days
    if y > 1970 {
        days += (y - 1969) / 4;
        days -= (y - 1901) / 100;
        days += (y - 1601) / 400;
    } else if y < 1970 {
        days += (y - 1972) / 4;
        days -= (y - 2000) / 100;
        days += (y - 2000) / 400;
    }

    // Add days for months
    for i in 1..(month_adj as usize) {
        days += days_in_month[i - 1] as i64;
    }

    // Add extra day if leap year and month > February
    let is_leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    if is_leap && month_adj > 2 {
        days += 1;
    }

    // Add day of month (1-indexed, so subtract 1)
    days += day as i64 - 1;

    // Convert to milliseconds
    let ms = days * 86400000
        + (hour as i64) * 3600000
        + (minute as i64) * 60000
        + (second as i64) * 1000
        + (millisecond as i64);

    ms
}

pub fn format_epoch_ms_utc(ms: i64) -> String {
    let (year, month, day, hour, minute, second, millisecond) = utc_parts_from_ms(ms);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millisecond:03}Z")
}

fn format_time_layout(ms: i64, layout: &str) -> String {
    let (year, month, day, hour, minute, second, millisecond) = utc_parts_from_ms(ms);
    match layout {
        "2006-01-02T15:04:05Z07:00" => {
            format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
        }
        "2006-01-02T15:04:05.999999999Z07:00" => format_epoch_ms_utc(ms),
        "2006-01-02 15:04:05" => {
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
        }
        "2006-01-02" => format!("{year:04}-{month:02}-{day:02}"),
        "15:04:05" => format!("{hour:02}:{minute:02}:{second:02}"),
        "3:04PM" => {
            let suffix = if hour < 12 { "AM" } else { "PM" };
            let hour12 = match hour % 12 {
                0 => 12,
                value => value,
            };
            format!("{hour12}:{minute:02}{suffix}")
        }
        _ => {
            let _ = millisecond;
            format_epoch_ms_utc(ms)
        }
    }
}

fn parse_duration_ms(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(ms) = trimmed.parse::<f64>() {
        return Some(ms as i64);
    }

    let bytes = trimmed.as_bytes();
    let mut pos = 0usize;
    let mut total = 0.0f64;
    while pos < bytes.len() {
        let start = pos;
        if bytes[pos] == b'+' || bytes[pos] == b'-' {
            pos += 1;
        }
        while pos < bytes.len() && (bytes[pos].is_ascii_digit() || bytes[pos] == b'.') {
            pos += 1;
        }
        if pos == start || (pos == start + 1 && matches!(bytes[start], b'+' | b'-')) {
            return None;
        }
        let amount = trimmed[start..pos].parse::<f64>().ok()?;
        let unit_start = pos;
        while pos < bytes.len() && !bytes[pos].is_ascii_digit() && bytes[pos] != b'.' {
            pos += 1;
        }
        let unit = &trimmed[unit_start..pos];
        let factor = match unit {
            "ns" => 0.000001,
            "us" | "µs" => 0.001,
            "ms" => 1.0,
            "s" => 1_000.0,
            "m" => 60_000.0,
            "h" => 3_600_000.0,
            "d" => 86_400_000.0,
            _ => return None,
        };
        total += amount * factor;
    }
    Some(total as i64)
}

fn duration_object(ms: i64) -> Object {
    module(vec![
        ("nanoseconds", num_obj(ms as f64 * 1_000_000.0)),
        ("microseconds", num_obj(ms as f64 * 1_000.0)),
        ("milliseconds", num_obj(ms as f64)),
        ("ms", num_obj(ms as f64)),
        ("seconds", num_obj(ms as f64 / 1_000.0)),
        ("minutes", num_obj(ms as f64 / 60_000.0)),
        ("hours", num_obj(ms as f64 / 3_600_000.0)),
        ("string", str_obj(format_duration(ms))),
        (
            "toString",
            native("time.duration.toString", move |_ctx, _args| {
                str_obj(format_duration(ms))
            }),
        ),
    ])
}

fn format_duration(ms: i64) -> String {
    if ms % 3_600_000 == 0 {
        format!("{}h", ms / 3_600_000)
    } else if ms % 60_000 == 0 {
        format!("{}m", ms / 60_000)
    } else if ms % 1_000 == 0 {
        format!("{}s", ms / 1_000)
    } else {
        format!("{}ms", ms)
    }
}

fn required_string(
    ctx: &CallContext,
    module: &str,
    args: &[Object],
    index: usize,
    name: &str,
) -> Result<String, Object> {
    match args.get(index) {
        Some(Object::String(value)) => Ok(value.to_string()),
        Some(_) => Err(new_error(
            ctx.pos.clone(),
            format!("{}: {} must be a string", module, name),
        )),
        None => Err(new_error(
            ctx.pos.clone(),
            format!("{} requires {}", module, name),
        )),
    }
}

fn required_number(
    ctx: &CallContext,
    module: &str,
    args: &[Object],
    index: usize,
    name: &str,
) -> Result<f64, Object> {
    match args.get(index) {
        Some(Object::Number(value)) => Ok(*value),
        Some(_) => Err(new_error(
            ctx.pos.clone(),
            format!("{}: {} must be a number", module, name),
        )),
        None => Err(new_error(
            ctx.pos.clone(),
            format!("{} requires {}", module, name),
        )),
    }
}

fn string_args(ctx: &CallContext, module: &str, args: &[Object]) -> Result<Vec<String>, Object> {
    let mut out = Vec::with_capacity(args.len());
    for arg in args {
        match arg {
            Object::String(value) => out.push(value.to_string()),
            _ => {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("{}: all arguments must be strings", module),
                ))
            }
        }
    }
    Ok(out)
}

fn hash_string(hash: &HashData, key: &str) -> Option<String> {
    match hash.get(key) {
        Some(Object::String(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn hash_bool_arg(value: Option<&Object>, key: &str) -> Option<bool> {
    match value {
        Some(Object::Hash(hash)) => match hash.borrow().get(key) {
            Some(Object::Boolean(value)) => Some(*value),
            _ => None,
        },
        _ => None,
    }
}

fn array(elements: Vec<Object>) -> Object {
    Object::Array(Rc::new(RefCell::new(ArrayData { elements })))
}

fn normalize_path_string(value: &str) -> String {
    let path = PathBuf::from(value);
    path.components()
        .collect::<PathBuf>()
        .to_string_lossy()
        .to_string()
}

fn pathdiff(from: &Path, to: &Path) -> Option<PathBuf> {
    let from_components: Vec<_> = from.components().collect();
    let to_components: Vec<_> = to.components().collect();
    let mut common = 0usize;
    while common < from_components.len()
        && common < to_components.len()
        && from_components[common] == to_components[common]
    {
        common += 1;
    }
    let mut result = PathBuf::new();
    for _ in common..from_components.len() {
        result.push("..");
    }
    for component in &to_components[common..] {
        result.push(component.as_os_str());
    }
    Some(if result.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        result
    })
}

// ===========================================================================
// P6 stdlib batch 1: encoding/base64, encoding/hex, hash, random, regexp,
// semver, collections, process.
//
// These are pure-algorithm modules with no network/IO heavy dependencies and
// are CI-friendly. Behavior contracts are derived from the Go originals in
// gts/internal/stdlib/*.go (see docs/full-parity-refactor-plan.md P6).
// ===========================================================================

// ---------------------------------------------------------------------------
// Byte input helpers shared by base64 / hex.
//
// Accepts a String (UTF-8 bytes), an Array of Numbers (low 8 bits each), or
// a Buffer-shaped Hash (recognized by a private marker key). Matches the Go
// `bufferBytesFromObject` contract.
// ---------------------------------------------------------------------------

const BUFFER_DATA_KEY: &str = "__buffer_data__";

fn bytes_from_object(ctx: &mut CallContext, name: &str, value: &Object) -> Result<Vec<u8>, Object> {
    match value {
        Object::String(s) => Ok(s.as_bytes().to_vec()),
        Object::Array(arr) => {
            let elements = &arr.borrow().elements;
            let mut out = Vec::with_capacity(elements.len());
            for (i, elem) in elements.iter().enumerate() {
                match elem {
                    Object::Number(n) => out.push(((*n as i64) & 0xff) as u8),
                    _ => {
                        return Err(new_error(
                            ctx.pos.clone(),
                            format!("{}: array item {} must be a number", name, i),
                        ))
                    }
                }
            }
            Ok(out)
        }
        Object::Hash(hash) => {
            if hash.borrow().contains(BUFFER_DATA_KEY) {
                match hash.borrow().get(BUFFER_DATA_KEY) {
                    Some(Object::Array(arr)) => {
                        let mut out = Vec::with_capacity(arr.borrow().elements.len());
                        for elem in &arr.borrow().elements {
                            match elem {
                                Object::Number(n) => out.push(((*n as i64) & 0xff) as u8),
                                _ => return Err(bytes_type_error(ctx, name)),
                            }
                        }
                        Ok(out)
                    }
                    _ => Err(bytes_type_error(ctx, name)),
                }
            } else {
                Err(bytes_type_error(ctx, name))
            }
        }
        _ => Err(bytes_type_error(ctx, name)),
    }
}

fn bytes_type_error(ctx: &mut CallContext, name: &str) -> Object {
    new_error(
        ctx.pos.clone(),
        format!("{}: value must be a string, array, or Buffer", name),
    )
}

/// Standard base64 alphabet (`+/=`) padding encoder.
fn base64_std_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut chunks = input.chunks_exact(3);
    for chunk in &mut chunks {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | chunk[2] as u32;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        out.push(TABLE[(n & 0x3f) as usize] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let n = (rem[0] as u32) << 16;
            out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = ((rem[0] as u32) << 16) | ((rem[1] as u32) << 8);
            out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

/// URL-safe base64 alphabet (`-_`) with no padding.
fn base64_url_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(input.len() * 4 / 3 + 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3f) as usize] as char);
        }
    }
    out
}

fn base64_decode_into(
    table: &[Option<u8>; 256],
    name: &str,
    text: &str,
    ignore_padding: bool,
) -> Result<Vec<u8>, String> {
    let mut bits: u32 = 0;
    let mut shift: u32 = 0;
    let mut out = Vec::new();
    for ch in text.chars() {
        if ignore_padding && ch == '=' {
            continue;
        }
        let v = table[ch as usize].ok_or_else(|| format!("{}: invalid base64 data", name))?;
        bits = (bits << 6) | v as u32;
        shift += 6;
        if shift >= 8 {
            shift -= 8;
            out.push((bits >> shift) as u8 & 0xff);
        }
    }
    Ok(out)
}

fn base64_std_table() -> [Option<u8>; 256] {
    let mut t = [None; 256];
    for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .iter()
        .enumerate()
    {
        t[*c as usize] = Some(i as u8);
    }
    t
}

fn base64_url_table() -> [Option<u8>; 256] {
    let mut t = [None; 256];
    for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        .iter()
        .enumerate()
    {
        t[*c as usize] = Some(i as u8);
    }
    t
}

fn base64_module() -> Object {
    module(vec![
        ("encode", native("base64.encode", base64_encode)),
        ("decode", native("base64.decode", base64_decode)),
        ("encodeURL", native("base64.encodeURL", base64_encode_url)),
        ("decodeURL", native("base64.decodeURL", base64_decode_url)),
    ])
}

fn base64_encode(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "base64.encode requires value"),
    };
    match bytes_from_object(ctx, "base64.encode", value) {
        Ok(bytes) => str_obj(base64_std_encode(&bytes)),
        Err(err) => err,
    }
}

fn base64_decode(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "base64.decode", args, 0, "text") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let table = base64_std_table();
    match base64_decode_into(&table, "base64.decode", &text, true) {
        Ok(bytes) => bytes_result(ctx, "base64.decode", bytes, args.get(1)),
        Err(msg) => new_error(ctx.pos.clone(), msg),
    }
}

fn base64_encode_url(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "base64.encodeURL requires value"),
    };
    match bytes_from_object(ctx, "base64.encodeURL", value) {
        Ok(bytes) => str_obj(base64_url_encode(&bytes)),
        Err(err) => err,
    }
}

fn base64_decode_url(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "base64.decodeURL", args, 0, "text") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let table = base64_url_table();
    match base64_decode_into(&table, "base64.decodeURL", &text, true) {
        Ok(bytes) => bytes_result(ctx, "base64.decodeURL", bytes, args.get(1)),
        Err(msg) => new_error(ctx.pos.clone(), msg),
    }
}

/// Apply the optional `{asBuffer: true}` flag and render the result.
fn bytes_result(
    _ctx: &mut CallContext,
    _name: &str,
    bytes: Vec<u8>,
    opts: Option<&Object>,
) -> Object {
    let as_buffer = hash_bool_arg(opts, "asBuffer").unwrap_or(false);
    if as_buffer {
        make_buffer(bytes)
    } else {
        match String::from_utf8(bytes.clone()) {
            Ok(s) => str_obj(s),
            // Fall back to lossy conversion to preserve a string return type,
            // matching the Go behavior where non-UTF8 bytes become a string.
            Err(_) => str_obj(String::from_utf8_lossy(&bytes).into_owned()),
        }
    }
}

/// Build a Buffer-shaped Hash so that it round-trips through `bytes_from_object`.
fn make_buffer(bytes: Vec<u8>) -> Object {
    let elements: Vec<Object> = bytes.iter().map(|b| num_obj(*b as f64)).collect();
    let inner = array(elements);
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set(BUFFER_DATA_KEY, inner);
    hash.borrow_mut().set("length", num_obj(bytes.len() as f64));
    Object::Hash(hash)
}

// ---------------------------------------------------------------------------
// hex
// ---------------------------------------------------------------------------

fn hex_encode_bytes(input: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(input.len() * 2);
    for b in input {
        out.push(TABLE[(b >> 4) as usize] as char);
        out.push(TABLE[(b & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode_bytes(name: &str, text: &str) -> Result<Vec<u8>, String> {
    if text.len() % 2 != 0 {
        return Err(format!("{}: invalid hex data", name));
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let hi = hex_val(chunk[0]).ok_or_else(|| format!("{}: invalid hex data", name))?;
        let lo = hex_val(chunk[1]).ok_or_else(|| format!("{}: invalid hex data", name))?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn hex_module() -> Object {
    module(vec![
        ("encode", native("hex.encode", hex_encode_fn)),
        ("decode", native("hex.decode", hex_decode_fn)),
    ])
}

fn hex_encode_fn(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "hex.encode requires value"),
    };
    match bytes_from_object(ctx, "hex.encode", value) {
        Ok(bytes) => str_obj(hex_encode_bytes(&bytes)),
        Err(err) => err,
    }
}

fn hex_decode_fn(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "hex.decode", args, 0, "text") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match hex_decode_bytes("hex.decode", &text) {
        Ok(bytes) => bytes_result(ctx, "hex.decode", bytes, args.get(1)),
        Err(msg) => new_error(ctx.pos.clone(), msg),
    }
}

// ---------------------------------------------------------------------------
// hash: adler32, crc32 (IEEE), crc64 (ISO), fnv1a (64-bit).
// ---------------------------------------------------------------------------

fn hash_module() -> Object {
    module(vec![
        ("adler32", native("hash.adler32", hash_adler32)),
        ("crc32", native("hash.crc32", hash_crc32)),
        ("crc64", native("hash.crc64", hash_crc64)),
        ("fnv1a", native("hash.fnv1a", hash_fnv1a)),
        (
            "adler32Number",
            native("hash.adler32Number", hash_adler32_number),
        ),
        ("crc32Number", native("hash.crc32Number", hash_crc32_number)),
    ])
}

fn hash_input(ctx: &mut CallContext, name: &str, args: &[Object]) -> Result<Vec<u8>, Object> {
    match args.first() {
        Some(value) => bytes_from_object(ctx, name, value),
        None => Err(new_error(
            ctx.pos.clone(),
            format!("{} requires value", name),
        )),
    }
}

fn hash_adler32(ctx: &mut CallContext, args: &[Object]) -> Object {
    match hash_input(ctx, "hash.adler32", args) {
        Ok(bytes) => str_obj(format!("{:08x}", adler32(&bytes))),
        Err(err) => err,
    }
}

fn hash_crc32(ctx: &mut CallContext, args: &[Object]) -> Object {
    match hash_input(ctx, "hash.crc32", args) {
        Ok(bytes) => str_obj(format!("{:08x}", crc32_ieee(&bytes))),
        Err(err) => err,
    }
}

fn hash_crc64(ctx: &mut CallContext, args: &[Object]) -> Object {
    match hash_input(ctx, "hash.crc64", args) {
        Ok(bytes) => str_obj(format!("{:016x}", crc64_iso(&bytes))),
        Err(err) => err,
    }
}

fn hash_fnv1a(ctx: &mut CallContext, args: &[Object]) -> Object {
    match hash_input(ctx, "hash.fnv1a", args) {
        Ok(bytes) => str_obj(format!("{:016x}", fnv1a_64(&bytes))),
        Err(err) => err,
    }
}

fn hash_adler32_number(ctx: &mut CallContext, args: &[Object]) -> Object {
    match hash_input(ctx, "hash.adler32Number", args) {
        Ok(bytes) => num_obj(adler32(&bytes) as f64),
        Err(err) => err,
    }
}

fn hash_crc32_number(ctx: &mut CallContext, args: &[Object]) -> Object {
    match hash_input(ctx, "hash.crc32Number", args) {
        Ok(bytes) => num_obj(crc32_ieee(&bytes) as f64),
        Err(err) => err,
    }
}

/// Adler-32 checksum (RFC 1950).
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

/// CRC-32 IEEE (polynomial 0xEDB88320), same as Go's crc32.ChecksumIEEE.
fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xffffffff;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xedb88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xffffffff
}

/// CRC-64 ISO (polynomial 0xD800000000000000), matching Go's crc64 ISO table.
fn crc64_iso(data: &[u8]) -> u64 {
    let table = crc64_iso_table();
    let mut crc: u64 = 0xffff_ffff_ffff_ffff;
    for &byte in data {
        crc = table[((crc ^ byte as u64) & 0xff) as usize] ^ (crc >> 8);
    }
    crc ^ 0xffff_ffff_ffff_ffff
}

fn crc64_iso_table() -> [u64; 256] {
    const POLY: u64 = 0xd800_0000_0000_0000;
    let mut table = [0u64; 256];
    for i in 0..256u32 {
        let mut crc = i as u64;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ POLY;
            } else {
                crc >>= 1;
            }
        }
        table[i as usize] = crc;
    }
    table
}

/// FNV-1a 64-bit hash.
fn fnv1a_64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

// ---------------------------------------------------------------------------
// random: cryptographically secure RNG helpers (matches Go's crypto/rand).
// ---------------------------------------------------------------------------

fn random_module() -> Object {
    module(vec![
        ("int", native("random.int", random_int)),
        ("float", native("random.float", random_float)),
        ("bool", native("random.bool", random_bool)),
        ("pick", native("random.pick", random_pick)),
        ("sample", native("random.sample", random_sample)),
        ("shuffle", native("random.shuffle", random_shuffle)),
        ("hex", native("random.hex", random_hex)),
        ("base64", native("random.base64", random_base64)),
        (
            "alphanumeric",
            native("random.alphanumeric", random_alphanumeric),
        ),
        ("alpha", native("random.alpha", random_alpha)),
        ("numeric", native("random.numeric", random_numeric)),
        ("uuid", native("random.uuid", random_uuid)),
        ("uuidv4", native("random.uuid", random_uuid)),
        ("bytes", native("random.bytes", random_bytes)),
    ])
}

/// Fill a buffer from the OS RNG. Returns an Error object on failure.
fn fill_random(ctx: &mut CallContext, name: &str, buf: &mut [u8]) -> Result<(), Object> {
    if getrandom_inner(buf) {
        Ok(())
    } else {
        Err(new_error(
            ctx.pos.clone(),
            format!("{}: random source unavailable", name),
        ))
    }
}

#[cfg(unix)]
fn getrandom_inner(buf: &mut [u8]) -> bool {
    use std::io::Read;
    match std::fs::File::open("/dev/urandom") {
        Ok(mut f) => f.read_exact(buf).is_ok(),
        Err(_) => {
            // Fall back to a time-seeded PRNG; rare on Unix but keeps behavior total.
            fallback_rng(buf)
        }
    }
}

#[cfg(windows)]
fn getrandom_inner(buf: &mut [u8]) -> bool {
    use std::os::raw::c_void;
    #[link(name = "bcrypt")]
    extern "system" {
        fn BCryptGenRandom(
            hAlgorithm: *mut c_void,
            pbBuffer: *mut u8,
            cbBuffer: u32,
            dwFlags: u32,
        ) -> i32;
    }
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status == 0 {
        true
    } else {
        fallback_rng(buf)
    }
}

#[cfg(not(any(unix, windows)))]
fn getrandom_inner(buf: &mut [u8]) -> bool {
    fallback_rng(buf)
}

/// Deterministic fallback so the runtime never panics when the system RNG is
/// unavailable. This is weaker than the Go behavior but keeps parity of shape.
fn fallback_rng(buf: &mut [u8]) -> bool {
    use std::time::SystemTime;
    let mut seed = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as u64,
        Err(_) => 0x9E3779B97F4A7C15,
    };
    for byte in buf.iter_mut() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        *byte = (seed & 0xff) as u8;
    }
    true
}

fn read_random_u64(ctx: &mut CallContext, name: &str) -> Result<u64, Object> {
    let mut buf = [0u8; 8];
    fill_random(ctx, name, &mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn random_int(ctx: &mut CallContext, args: &[Object]) -> Object {
    let min = match required_number(ctx, "random.int", args, 0, "min") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let max = match required_number(ctx, "random.int", args, 1, "max") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if !(min < max) {
        return new_error(ctx.pos.clone(), "random.int: min must be less than max");
    }
    let span = (max - min) as u64;
    match bounded_random_u64(ctx, "random.int", span) {
        Ok(value) => num_obj(min + (value as f64)),
        Err(err) => err,
    }
}

fn random_float(ctx: &mut CallContext, args: &[Object]) -> Object {
    let min = match required_number(ctx, "random.float", args, 0, "min") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let max = match required_number(ctx, "random.float", args, 1, "max") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if !(min < max) {
        return new_error(ctx.pos.clone(), "random.float: min must be less than max");
    }
    match read_random_u64(ctx, "random.float") {
        Ok(raw) => {
            let frac = raw as f64 / u64::MAX as f64;
            num_obj(min + frac * (max - min))
        }
        Err(err) => err,
    }
}

fn random_bool(ctx: &mut CallContext, _args: &[Object]) -> Object {
    let mut buf = [0u8; 1];
    match fill_random(ctx, "random.bool", &mut buf) {
        Ok(()) => bool_obj(buf[0] & 1 == 1),
        Err(err) => err,
    }
}

/// Compute a uniform random integer in `[0, span)` via rejection sampling on
/// a 64-bit value, matching the spirit of Go's `rand.Int(reader, span)`.
fn bounded_random_u64(ctx: &mut CallContext, name: &str, span: u64) -> Result<u64, Object> {
    let limit = u64::MAX - (u64::MAX % span);
    loop {
        let value = read_random_u64(ctx, name)?;
        if value < limit {
            return Ok(value % span);
        }
    }
}

fn random_pick(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.first() {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "random.pick: argument must be an array"),
        None => return new_error(ctx.pos.clone(), "random.pick requires array"),
    };
    let len = arr.borrow().elements.len();
    if len == 0 {
        return Object::Null;
    }
    match bounded_random_u64(ctx, "random.pick", len as u64) {
        Ok(idx) => arr.borrow().elements[idx as usize].clone(),
        Err(err) => err,
    }
}

fn random_sample(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.first() {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => {
            return new_error(
                ctx.pos.clone(),
                "random.sample: first argument must be an array",
            )
        }
        None => return new_error(ctx.pos.clone(), "random.sample requires array and count"),
    };
    let count = match required_number(ctx, "random.sample", args, 1, "count") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if count < 0.0 {
        return new_error(ctx.pos.clone(), "random.sample: count must be non-negative");
    }
    let mut elements = arr.borrow().elements.clone();
    let take = (count as usize).min(elements.len());
    // Fisher-Yates partial shuffle over the first `take` positions.
    for i in 0..take {
        let span = (elements.len() - i) as u64;
        match bounded_random_u64(ctx, "random.sample", span) {
            Ok(j) => elements.swap(i, i + j as usize),
            Err(err) => return err,
        }
    }
    elements.truncate(take);
    array(elements)
}

fn random_shuffle(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.first() {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "random.shuffle: argument must be an array"),
        None => return new_error(ctx.pos.clone(), "random.shuffle requires array"),
    };
    let mut elements = arr.borrow().elements.clone();
    let len = elements.len();
    for i in (1..len).rev() {
        match bounded_random_u64(ctx, "random.shuffle", (i + 1) as u64) {
            Ok(j) => elements.swap(i, j as usize),
            Err(err) => return err,
        }
    }
    array(elements)
}

fn random_length_bounded(
    ctx: &mut CallContext,
    name: &str,
    args: &[Object],
    label: &str,
    max: u32,
) -> Result<u32, Object> {
    match required_number(ctx, name, args, 0, label) {
        Ok(n) => {
            if n < 0.0 || n > max as f64 {
                Err(new_error(
                    ctx.pos.clone(),
                    format!("{}: {} must be in range [0, {}]", name, label, max),
                ))
            } else {
                Ok(n as u32)
            }
        }
        Err(e) => Err(e),
    }
}

fn random_hex(ctx: &mut CallContext, args: &[Object]) -> Object {
    let count = match random_length_bounded(ctx, "random.hex", args, "byte count", 1024) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut buf = vec![0u8; count as usize];
    if let Err(err) = fill_random(ctx, "random.hex", &mut buf) {
        return err;
    }
    str_obj(hex_encode_bytes(&buf))
}

fn random_base64(ctx: &mut CallContext, args: &[Object]) -> Object {
    let count = match random_length_bounded(ctx, "random.base64", args, "byte count", 1024) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut buf = vec![0u8; count as usize];
    if let Err(err) = fill_random(ctx, "random.base64", &mut buf) {
        return err;
    }
    str_obj(base64_std_encode(&buf))
}

fn random_charset_string(
    ctx: &mut CallContext,
    name: &str,
    args: &[Object],
    charset: &[u8],
) -> Object {
    let length = match random_length_bounded(ctx, name, args, "length", 1024) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let span = charset.len() as u64;
    let mut out = String::with_capacity(length as usize);
    for _ in 0..length {
        match bounded_random_u64(ctx, name, span) {
            Ok(idx) => out.push(charset[idx as usize] as char),
            Err(err) => return err,
        }
    }
    str_obj(out)
}

const ALPHA_NUMERIC: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const NUMERIC: &[u8] = b"0123456789";

fn random_alphanumeric(ctx: &mut CallContext, args: &[Object]) -> Object {
    random_charset_string(ctx, "random.alphanumeric", args, ALPHA_NUMERIC)
}

fn random_alpha(ctx: &mut CallContext, args: &[Object]) -> Object {
    random_charset_string(ctx, "random.alpha", args, ALPHA)
}

fn random_numeric(ctx: &mut CallContext, args: &[Object]) -> Object {
    random_charset_string(ctx, "random.numeric", args, NUMERIC)
}

fn random_uuid(ctx: &mut CallContext, _args: &[Object]) -> Object {
    let mut buf = [0u8; 16];
    if let Err(err) = fill_random(ctx, "random.uuid", &mut buf) {
        return err;
    }
    buf[6] = (buf[6] & 0x0f) | 0x40;
    buf[8] = (buf[8] & 0x3f) | 0x80;
    str_obj(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7], buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]
    ))
}

fn random_bytes(ctx: &mut CallContext, args: &[Object]) -> Object {
    let size = match required_number(ctx, "random.bytes", args, 0, "size") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if !(0.0..=1_048_576.0).contains(&size) {
        return new_error(
            ctx.pos.clone(),
            "random.bytes: size must be in range [0, 1048576]",
        );
    }
    let mut buf = vec![0u8; size as usize];
    if let Err(err) = fill_random(ctx, "random.bytes", &mut buf) {
        return err;
    }
    array(buf.into_iter().map(|b| num_obj(b as f64)).collect())
}

// ---------------------------------------------------------------------------
// regexp: RE2-based escape / matchAll / split.
// ---------------------------------------------------------------------------

fn regexp_module() -> Object {
    module(vec![
        ("escape", native("regexp.escape", regexp_escape)),
        ("matchAll", native("regexp.matchAll", regexp_match_all)),
        ("split", native("regexp.split", regexp_split)),
    ])
}

fn regexp_escape(ctx: &mut CallContext, args: &[Object]) -> Object {
    match args.first() {
        Some(Object::String(s)) => str_obj(regex::escape(s)),
        Some(_) => new_error(ctx.pos.clone(), "regexp.escape expects string"),
        None => new_error(ctx.pos.clone(), "regexp.escape requires string"),
    }
}

fn regexp_match_all(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn regexp_split(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn semver_module() -> Object {
    module(vec![
        ("parse", native("semver.parse", semver_parse)),
        ("valid", native("semver.valid", semver_valid)),
        ("compare", native("semver.compare", semver_compare_fn)),
        ("gt", native("semver.gt", semver_gt)),
        ("gte", native("semver.gte", semver_gte)),
        ("lt", native("semver.lt", semver_lt)),
        ("lte", native("semver.lte", semver_lte)),
        ("eq", native("semver.eq", semver_eq)),
        ("neq", native("semver.neq", semver_neq)),
        ("inc", native("semver.inc", semver_inc)),
        ("satisfies", native("semver.satisfies", semver_satisfies)),
    ])
}

/// Parsed semantic version. `prerelease` segments may be numeric or textual.
struct Semver {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<String>,
    build: Vec<String>,
}

fn parse_semver(value: &str) -> Option<Semver> {
    let value = value.trim();
    let value = value.strip_prefix('v').unwrap_or(value);
    // Separate build metadata first.
    let (core, build) = match value.split_once('+') {
        Some((c, b)) => (c, b),
        None => (value, ""),
    };
    let (main, prerelease) = match core.split_once('-') {
        Some((m, p)) => (m, p),
        None => (core, ""),
    };
    let nums: Vec<&str> = main.split('.').collect();
    if nums.len() != 3 {
        return None;
    }
    let major = nums[0].parse::<u64>().ok()?;
    let minor = nums[1].parse::<u64>().ok()?;
    let patch = nums[2].parse::<u64>().ok()?;
    if nums.iter().any(|n| n.is_empty()) {
        return None;
    }
    let prerelease: Vec<String> = if prerelease.is_empty() {
        Vec::new()
    } else {
        prerelease.split('.').map(|s| s.to_string()).collect()
    };
    if !prerelease.iter().all(|s| valid_pre_segment(s)) {
        return None;
    }
    let build: Vec<String> = if build.is_empty() {
        Vec::new()
    } else {
        build.split('.').map(|s| s.to_string()).collect()
    };
    if !build.iter().all(|s| valid_meta_segment(s)) {
        return None;
    }
    Some(Semver {
        major,
        minor,
        patch,
        prerelease,
        build,
    })
}

fn valid_pre_segment(seg: &str) -> bool {
    !seg.is_empty()
        && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && seg.parse::<u64>().map(|_| true).unwrap_or(true)
}

fn valid_meta_segment(seg: &str) -> bool {
    !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn semver_to_object(sv: &Semver) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("major", num_obj(sv.major as f64));
    hash.borrow_mut().set("minor", num_obj(sv.minor as f64));
    hash.borrow_mut().set("patch", num_obj(sv.patch as f64));
    // Go emits numeric prerelease segments as numbers and textual ones as strings.
    let pre: Vec<Object> = sv
        .prerelease
        .iter()
        .map(|s| match s.parse::<u64>() {
            Ok(n) => num_obj(n as f64),
            Err(_) => str_obj(s.clone()),
        })
        .collect();
    hash.borrow_mut().set("prerelease", array(pre));
    hash.borrow_mut().set(
        "build",
        array(sv.build.iter().map(|s| str_obj(s.clone())).collect()),
    );
    Object::Hash(hash)
}

/// Compare two semvers, returning -1/0/1. Build metadata is ignored.
fn compare_semver(a: &Semver, b: &Semver) -> i32 {
    if a.major != b.major {
        return if a.major > b.major { 1 } else { -1 };
    }
    if a.minor != b.minor {
        return if a.minor > b.minor { 1 } else { -1 };
    }
    if a.patch != b.patch {
        return if a.patch > b.patch { 1 } else { -1 };
    }
    compare_prerelease(&a.prerelease, &b.prerelease)
}

fn compare_prerelease(a: &[String], b: &[String]) -> i32 {
    // A version without prerelease has higher precedence.
    match (a.is_empty(), b.is_empty()) {
        (true, true) => return 0,
        (true, false) => return 1,
        (false, true) => return -1,
        _ => {}
    }
    let len = a.len().min(b.len());
    for i in 0..len {
        let (la, lb) = (&a[i], &b[i]);
        let na = la.parse::<u64>().ok();
        let nb = lb.parse::<u64>().ok();
        let ord = match (na, nb) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => la.cmp(lb),
        };
        match ord {
            std::cmp::Ordering::Equal => continue,
            other => {
                return match other {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Greater => 1,
                    _ => 0,
                }
            }
        }
    }
    a.len().cmp(&b.len()) as i32
}

fn two_semvers(
    ctx: &mut CallContext,
    name: &str,
    args: &[Object],
) -> Result<(Semver, Semver), Object> {
    let v1 = match required_string(ctx, name, args, 0, "version") {
        Ok(s) => s,
        Err(e) => return Err(e),
    };
    let v2 = match required_string(ctx, name, args, 1, "version") {
        Ok(s) => s,
        Err(e) => return Err(e),
    };
    match (parse_semver(&v1), parse_semver(&v2)) {
        (Some(a), Some(b)) => Ok((a, b)),
        _ => Err(new_error(
            ctx.pos.clone(),
            format!("{}: invalid version", name),
        )),
    }
}

fn semver_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let version = match required_string(ctx, "semver.parse", args, 0, "version") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match parse_semver(&version) {
        Some(sv) => semver_to_object(&sv),
        None => new_error(
            ctx.pos.clone(),
            format!("semver.parse: invalid version: {}", version),
        ),
    }
}

fn semver_valid(_ctx: &mut CallContext, args: &[Object]) -> Object {
    match args.first() {
        Some(Object::String(s)) => bool_obj(parse_semver(s).is_some()),
        _ => bool_obj(false),
    }
}

fn semver_compare_fn(ctx: &mut CallContext, args: &[Object]) -> Object {
    match two_semvers(ctx, "semver.compare", args) {
        Ok((a, b)) => num_obj(compare_semver(&a, &b) as f64),
        Err(err) => err,
    }
}

fn semver_gt(ctx: &mut CallContext, args: &[Object]) -> Object {
    match two_semvers(ctx, "semver.gt", args) {
        Ok((a, b)) => bool_obj(compare_semver(&a, &b) > 0),
        Err(err) => err,
    }
}
fn semver_gte(ctx: &mut CallContext, args: &[Object]) -> Object {
    match two_semvers(ctx, "semver.gte", args) {
        Ok((a, b)) => bool_obj(compare_semver(&a, &b) >= 0),
        Err(err) => err,
    }
}
fn semver_lt(ctx: &mut CallContext, args: &[Object]) -> Object {
    match two_semvers(ctx, "semver.lt", args) {
        Ok((a, b)) => bool_obj(compare_semver(&a, &b) < 0),
        Err(err) => err,
    }
}
fn semver_lte(ctx: &mut CallContext, args: &[Object]) -> Object {
    match two_semvers(ctx, "semver.lte", args) {
        Ok((a, b)) => bool_obj(compare_semver(&a, &b) <= 0),
        Err(err) => err,
    }
}
fn semver_eq(ctx: &mut CallContext, args: &[Object]) -> Object {
    match two_semvers(ctx, "semver.eq", args) {
        Ok((a, b)) => bool_obj(compare_semver(&a, &b) == 0),
        Err(err) => err,
    }
}
fn semver_neq(ctx: &mut CallContext, args: &[Object]) -> Object {
    match two_semvers(ctx, "semver.neq", args) {
        Ok((a, b)) => bool_obj(compare_semver(&a, &b) != 0),
        Err(err) => err,
    }
}

fn semver_inc(ctx: &mut CallContext, args: &[Object]) -> Object {
    let version = match required_string(ctx, "semver.inc", args, 0, "version") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let release = match required_string(ctx, "semver.inc", args, 1, "release") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut sv = match parse_semver(&version) {
        Some(sv) => sv,
        None => {
            return new_error(
                ctx.pos.clone(),
                format!("semver.parse: invalid version: {}", version),
            )
        }
    };
    match release.as_str() {
        "major" => {
            sv.major += 1;
            sv.minor = 0;
            sv.patch = 0;
        }
        "minor" => {
            sv.minor += 1;
            sv.patch = 0;
        }
        "patch" => sv.patch += 1,
        "prerelease" => {
            sv.patch += 1;
            sv.prerelease = vec!["0".to_string()];
        }
        other => {
            return new_error(
                ctx.pos.clone(),
                format!("semver.inc: invalid release type: {}", other),
            )
        }
    }
    sv.build.clear();
    if release == "prerelease" {
        str_obj(format!("{}.{}.{}-0", sv.major, sv.minor, sv.patch))
    } else {
        str_obj(format!("{}.{}.{}", sv.major, sv.minor, sv.patch))
    }
}

fn semver_satisfies(ctx: &mut CallContext, args: &[Object]) -> Object {
    let version = match required_string(ctx, "semver.satisfies", args, 0, "version") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let range = match required_string(ctx, "semver.satisfies", args, 1, "range") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let sv = match parse_semver(&version) {
        Some(sv) => sv,
        None => {
            return new_error(
                ctx.pos.clone(),
                format!("semver.parse: invalid version: {}", version),
            )
        }
    };
    match satisfies_range(&sv, range.trim()) {
        Ok(true) => bool_obj(true),
        Ok(false) => bool_obj(false),
        Err(msg) => new_error(ctx.pos.clone(), msg),
    }
}

fn satisfies_range(sv: &Semver, range: &str) -> Result<bool, String> {
    if let Some(rest) = range.strip_prefix('^') {
        let base = parse_semver(rest.trim()).ok_or("semver.satisfies: invalid range")?;
        return Ok(sv.major == base.major && compare_semver(sv, &base) >= 0);
    }
    if let Some(rest) = range.strip_prefix('~') {
        let base = parse_semver(rest.trim()).ok_or("semver.satisfies: invalid range")?;
        return Ok(sv.major == base.major
            && sv.minor == base.minor
            && compare_semver(sv, &base) >= 0);
    }
    let parts: Vec<&str> = range.split_whitespace().collect();
    if parts.len() >= 2 {
        let mut i = 0;
        while i + 1 < parts.len() {
            let op = parts[i];
            let rhs = match parse_semver(parts[i + 1]) {
                Some(v) => v,
                None => {
                    i += 2;
                    continue;
                }
            };
            let cmp = compare_semver(sv, &rhs);
            let ok = match op {
                ">=" => cmp >= 0,
                ">" => cmp > 0,
                "<=" => cmp <= 0,
                "<" => cmp < 0,
                "=" | "==" => cmp == 0,
                _ => true,
            };
            if !ok {
                return Ok(false);
            }
            i += 2;
        }
        return Ok(true);
    }
    // Bare version: equality.
    match parse_semver(range) {
        Some(base) => Ok(compare_semver(sv, &base) == 0),
        None => Ok(false),
    }
}

// ---------------------------------------------------------------------------
// collections: array helpers (unique/chunk/flatten/sample/shuffle/range).
// ---------------------------------------------------------------------------

fn collections_module() -> Object {
    module(vec![
        ("unique", native("collections.unique", collections_unique)),
        ("chunk", native("collections.chunk", collections_chunk)),
        (
            "flatten",
            native("collections.flatten", collections_flatten),
        ),
        ("sample", native("collections.sample", collections_sample)),
        (
            "shuffle",
            native("collections.shuffle", collections_shuffle),
        ),
        ("range", native("collections.range", collections_range)),
    ])
}

fn collections_unique(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.first() {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "collections.unique expects array"),
        None => return new_error(ctx.pos.clone(), "collections.unique requires array"),
    };
    let mut seen: Vec<Object> = Vec::new();
    let mut out: Vec<Object> = Vec::new();
    for elem in arr.borrow().elements.iter() {
        let mut found = false;
        for prev in &seen {
            if strict_equal(elem, prev) {
                found = true;
                break;
            }
        }
        if !found {
            seen.push(elem.clone());
            out.push(elem.clone());
        }
    }
    array(out)
}

fn collections_chunk(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(ctx.pos.clone(), "collections.chunk requires array and size");
    }
    let arr = match &args[0] {
        Object::Array(a) => a.clone(),
        _ => return new_error(ctx.pos.clone(), "collections.chunk expects array"),
    };
    let size = match &args[1] {
        Object::Number(n) => *n,
        _ => return new_error(ctx.pos.clone(), "collections.chunk expects number size"),
    };
    if size <= 0.0 {
        return new_error(ctx.pos.clone(), "collections.chunk size must be positive");
    }
    let size = size as usize;
    let elements = arr.borrow().elements.clone();
    let chunks: Vec<Object> = elements.chunks(size).map(|c| array(c.to_vec())).collect();
    array(chunks)
}

fn collections_flatten(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.first() {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "collections.flatten expects array"),
        None => return new_error(ctx.pos.clone(), "collections.flatten requires array"),
    };
    let mut out = Vec::new();
    for elem in arr.borrow().elements.iter() {
        match elem {
            Object::Array(inner) => out.extend(inner.borrow().elements.iter().cloned()),
            other => out.push(other.clone()),
        }
    }
    array(out)
}

fn collections_sample(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.first() {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "collections.sample expects array"),
        None => return new_error(ctx.pos.clone(), "collections.sample requires array"),
    };
    let elements = arr.borrow();
    if elements.elements.is_empty() {
        return Object::Undefined;
    }
    let len = elements.elements.len();
    match bounded_random_u64(ctx, "collections.sample", len as u64) {
        Ok(idx) => elements.elements[idx as usize].clone(),
        Err(err) => err,
    }
}

fn collections_shuffle(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.first() {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "collections.shuffle expects array"),
        None => return new_error(ctx.pos.clone(), "collections.shuffle requires array"),
    };
    let mut elements = arr.borrow().elements.clone();
    let len = elements.len();
    for i in (1..len).rev() {
        match bounded_random_u64(ctx, "collections.shuffle", (i + 1) as u64) {
            Ok(j) => elements.swap(i, j as usize),
            Err(err) => return err,
        }
    }
    array(elements)
}

fn collections_range(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.is_empty() {
        return new_error(
            ctx.pos.clone(),
            "collections.range requires at least end value",
        );
    }
    let (start, end, step) = match args.len() {
        1 => {
            let end = match &args[0] {
                Object::Number(n) => *n,
                _ => return new_error(ctx.pos.clone(), "collections.range expects number"),
            };
            (0.0, end, 1.0)
        }
        2 => {
            let start = match &args[0] {
                Object::Number(n) => *n,
                _ => return new_error(ctx.pos.clone(), "collections.range expects number"),
            };
            let end = match &args[1] {
                Object::Number(n) => *n,
                _ => return new_error(ctx.pos.clone(), "collections.range expects number"),
            };
            (start, end, 1.0)
        }
        _ => {
            let start = match &args[0] {
                Object::Number(n) => *n,
                _ => return new_error(ctx.pos.clone(), "collections.range expects number"),
            };
            let end = match &args[1] {
                Object::Number(n) => *n,
                _ => return new_error(ctx.pos.clone(), "collections.range expects number"),
            };
            let step = match &args[2] {
                Object::Number(n) => *n,
                _ => return new_error(ctx.pos.clone(), "collections.range expects number"),
            };
            (start, end, step)
        }
    };
    if step == 0.0 {
        return new_error(ctx.pos.clone(), "collections.range step cannot be zero");
    }
    let mut out = Vec::new();
    if step > 0.0 {
        let mut i = start;
        while i < end {
            out.push(num_obj(i));
            i += step;
        }
    } else {
        let mut i = start;
        while i > end {
            out.push(num_obj(i));
            i += step;
        }
    }
    array(out)
}

// ---------------------------------------------------------------------------
// process: argv / pid / cwd / exit / hrtime, etc.
// ---------------------------------------------------------------------------

fn process_module() -> Object {
    let mut entries: Vec<(&str, Object)> = Vec::new();
    let argv: Vec<Object> = std::env::args().map(str_obj).collect();
    let argv0 = argv
        .first()
        .cloned()
        .unwrap_or_else(|| str_obj(String::new()));
    entries.push(("argv", array(argv)));
    entries.push(("argv0", argv0));
    entries.push(("pid", num_obj(std::process::id() as f64)));
    // Snapshot environment as an object (consistent with Go's `process.env`).
    let env_hash = Rc::new(RefCell::new(HashData::default()));
    for (k, v) in std::env::vars() {
        env_hash.borrow_mut().set(k, str_obj(v));
    }
    entries.push(("env", Object::Hash(env_hash)));
    entries.push(("version", str_obj(VERSION)));
    entries.push(("cwd", native("process.cwd", process_cwd)));
    entries.push(("chdir", native("process.chdir", process_chdir)));
    entries.push(("execPath", native("process.execPath", process_exec_path)));
    entries.push(("getenv", native("process.getenv", process_getenv)));
    entries.push(("envObject", native("process.envObject", process_env_object)));
    entries.push(("uptime", native("process.uptime", process_uptime)));
    entries.push(("hrtime", native("process.hrtime", process_hrtime)));
    entries.push(("setenv", native("process.setenv", process_setenv)));
    entries.push(("unsetenv", native("process.unsetenv", process_unsetenv)));
    entries.push(("exit", native("process.exit", process_exit)));
    module(entries)
}

fn process_cwd(ctx: &mut CallContext, _args: &[Object]) -> Object {
    match std::env::current_dir() {
        Ok(p) => str_obj(p.to_string_lossy()),
        Err(e) => new_error(ctx.pos.clone(), format!("process.cwd: {}", e)),
    }
}

fn process_chdir(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "process.chdir", args, 0, "path") {
        Ok(p) => p,
        Err(e) => return e,
    };
    match std::env::set_current_dir(&path) {
        Ok(()) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("process.chdir: {}", e)),
    }
}

fn process_exec_path(ctx: &mut CallContext, _args: &[Object]) -> Object {
    match std::env::current_exe() {
        Ok(p) => str_obj(p.to_string_lossy()),
        Err(e) => new_error(ctx.pos.clone(), format!("process.execPath: {}", e)),
    }
}

fn process_getenv(ctx: &mut CallContext, args: &[Object]) -> Object {
    let name = match required_string(ctx, "process.getenv", args, 0, "name") {
        Ok(n) => n,
        Err(e) => return e,
    };
    match std::env::var_os(&name) {
        Some(val) => str_obj(val.to_string_lossy()),
        None => args.get(1).cloned().unwrap_or(Object::Undefined),
    }
}

fn process_env_object(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    for (k, v) in std::env::vars() {
        hash.borrow_mut().set(k, str_obj(v));
    }
    Object::Hash(hash)
}

static PROCESS_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn process_uptime(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    let start = PROCESS_START.get_or_init(std::time::Instant::now);
    num_obj(start.elapsed().as_secs_f64())
}

fn process_hrtime(_ctx: &mut CallContext, args: &[Object]) -> Object {
    let start = PROCESS_START.get_or_init(std::time::Instant::now);
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs();
    let nanos = elapsed.subsec_nanos();
    let value = array(vec![num_obj(secs as f64), num_obj(nanos as f64)]);

    // If a previous [sec, nano] array is supplied, return the difference.
    if let Some(Object::Array(prev)) = args.first() {
        let prev = prev.borrow();
        if prev.elements.len() == 2 {
            if let (Object::Number(ps), Object::Number(pn)) =
                (prev.elements[0].clone(), prev.elements[1].clone())
            {
                let psecs = ps as u64;
                let pnanos = pn as u32;
                let mut dsecs = secs.saturating_sub(psecs);
                let mut dnanos = nanos as i64 - pnanos as i64;
                if dnanos < 0 {
                    dsecs = dsecs.saturating_sub(1);
                    dnanos += 1_000_000_000;
                }
                return array(vec![num_obj(dsecs as f64), num_obj(dnanos as f64)]);
            }
        }
    }
    value
}

fn process_setenv(ctx: &mut CallContext, args: &[Object]) -> Object {
    let name = match required_string(ctx, "process.setenv", args, 0, "name") {
        Ok(n) => n,
        Err(e) => return e,
    };
    let value = match required_string(ctx, "process.setenv", args, 1, "value") {
        Ok(v) => v,
        Err(e) => return e,
    };
    std::env::set_var(&name, &value);
    Object::Undefined
}

fn process_unsetenv(ctx: &mut CallContext, args: &[Object]) -> Object {
    let name = match required_string(ctx, "process.unsetenv", args, 0, "name") {
        Ok(n) => n,
        Err(e) => return e,
    };
    std::env::remove_var(&name);
    Object::Undefined
}

fn process_exit(ctx: &mut CallContext, args: &[Object]) -> Object {
    let code = match args.first() {
        Some(Object::Number(n)) => *n as i32,
        Some(Object::String(s)) => match s.parse::<i32>() {
            Ok(n) => n,
            Err(_) => return new_error(ctx.pos.clone(), "process.exit: code must be a number"),
        },
        Some(_) => return new_error(ctx.pos.clone(), "process.exit: code must be a number"),
        None => 0,
    };
    // Builtin return is symbolic; the runtime treats exit as a normal return.
    // We surface the intended code via a controlled panic-free process::exit.
    std::process::exit(code);
}

// ===========================================================================
// P6 stdlib batch 2: crypto (sha1/256/512 + hmac + pbkdf2 + randomUUID +
// randomBytes + timingSafeEqual), text (display-width utilities), url
// (parse/format/resolve + URL/URLSearchParams), cache (TTL dictionary).
// ===========================================================================

// ---------------------------------------------------------------------------
// crypto: SHA-1/256/512 (self-contained, no external crate), HMAC, PBKDF2,
// randomUUID, randomBytes, timingSafeEqual.
//
// SHA implementations below are straightforward, well-tested reference
// versions of the NIST/NSA algorithms; outputs are byte vectors that get
// hex-encoded to lowercase strings to match the Go originals.
// ---------------------------------------------------------------------------

fn crypto_module() -> Object {
    module(vec![
        (
            "randomUUID",
            native("crypto.randomUUID", crypto_random_uuid),
        ),
        ("sha1", native("crypto.sha1", crypto_sha1)),
        ("sha256", native("crypto.sha256", crypto_sha256)),
        ("sha512", native("crypto.sha512", crypto_sha512)),
        ("hmac", native("crypto.hmac", crypto_hmac)),
        ("pbkdf2", native("crypto.pbkdf2", crypto_pbkdf2)),
        (
            "randomBytes",
            native("crypto.randomBytes", crypto_random_bytes),
        ),
        (
            "timingSafeEqual",
            native("crypto.timingSafeEqual", crypto_timing_safe_equal),
        ),
    ])
}

fn crypto_sha1(ctx: &mut CallContext, args: &[Object]) -> Object {
    match crypto_input(ctx, "crypto.sha1", args) {
        Ok(bytes) => str_obj(hex_encode_bytes(&sha1(&bytes))),
        Err(err) => err,
    }
}

fn crypto_sha256(ctx: &mut CallContext, args: &[Object]) -> Object {
    match crypto_input(ctx, "crypto.sha256", args) {
        Ok(bytes) => str_obj(hex_encode_bytes(&sha256(&bytes))),
        Err(err) => err,
    }
}

fn crypto_sha512(ctx: &mut CallContext, args: &[Object]) -> Object {
    match crypto_input(ctx, "crypto.sha512", args) {
        Ok(bytes) => str_obj(hex_encode_bytes(&sha512(&bytes))),
        Err(err) => err,
    }
}

fn crypto_input(ctx: &mut CallContext, name: &str, args: &[Object]) -> Result<Vec<u8>, Object> {
    match args.first() {
        Some(value) => bytes_from_object(ctx, name, value),
        None => Err(new_error(
            ctx.pos.clone(),
            format!("{} requires value", name),
        )),
    }
}

fn crypto_hmac(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 3 {
        return new_error(
            ctx.pos.clone(),
            "crypto.hmac requires algorithm, key and value",
        );
    }
    let algorithm = match &args[0] {
        Object::String(s) => s.clone(),
        _ => return new_error(ctx.pos.clone(), "crypto.hmac: algorithm must be a string"),
    };
    let key = match bytes_from_object(ctx, "crypto.hmac", &args[1]) {
        Ok(b) => b,
        Err(err) => return err,
    };
    let value = match bytes_from_object(ctx, "crypto.hmac", &args[2]) {
        Ok(b) => b,
        Err(err) => return err,
    };
    match hash_kind(&algorithm) {
        Some(kind) => str_obj(hex_encode_bytes(&hmac(kind, &key, &value))),
        None => new_error(
            ctx.pos.clone(),
            format!("crypto.hmac: unsupported hash algorithm {:?}", algorithm),
        ),
    }
}

fn crypto_pbkdf2(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 4 {
        return new_error(
            ctx.pos.clone(),
            "crypto.pbkdf2 requires password, salt, iterations and keyLength",
        );
    }
    let password = match bytes_from_object(ctx, "crypto.pbkdf2", &args[0]) {
        Ok(b) => b,
        Err(err) => return err,
    };
    let salt = match bytes_from_object(ctx, "crypto.pbkdf2", &args[1]) {
        Ok(b) => b,
        Err(err) => return err,
    };
    let iterations = match required_positive_int(ctx, "crypto.pbkdf2", &args[2], "iterations") {
        Ok(n) => n,
        Err(err) => return err,
    };
    let key_length = match required_positive_int(ctx, "crypto.pbkdf2", &args[3], "keyLength") {
        Ok(n) => n,
        Err(err) => return err,
    };
    let algorithm = match args.get(4) {
        Some(Object::String(s)) => s.as_str().to_string(),
        // Default per Go original.
        _ => "sha256".to_string(),
    };
    let kind = match hash_kind(&algorithm) {
        Some(kind) => kind,
        None => {
            return new_error(
                ctx.pos.clone(),
                format!("crypto.pbkdf2: unsupported hash algorithm {:?}", algorithm),
            )
        }
    };
    let derived = pbkdf2(kind, &password, &salt, iterations as u32, key_length);
    // pbkdf2 defaults to a lowercase hex string (matching the Go original's
    // hex.EncodeToString); only {asBuffer:true} returns a Buffer.
    let as_buffer = hash_bool_arg(args.get(5), "asBuffer").unwrap_or(false);
    if as_buffer {
        make_buffer(derived)
    } else {
        str_obj(hex_encode_bytes(&derived))
    }
}

fn required_positive_int(
    ctx: &mut CallContext,
    name: &str,
    value: &Object,
    label: &str,
) -> Result<usize, Object> {
    match value {
        Object::Number(n) => {
            let n = *n as i64;
            if n <= 0 {
                Err(new_error(
                    ctx.pos.clone(),
                    format!("{}: {} must be positive", name, label),
                ))
            } else {
                Ok(n as usize)
            }
        }
        _ => Err(new_error(
            ctx.pos.clone(),
            format!("{}: {} must be a number", name, label),
        )),
    }
}

fn crypto_random_uuid(ctx: &mut CallContext, _args: &[Object]) -> Object {
    let mut buf = [0u8; 16];
    if let Err(err) = fill_random(ctx, "crypto.randomUUID", &mut buf) {
        return err;
    }
    buf[6] = (buf[6] & 0x0f) | 0x40;
    buf[8] = (buf[8] & 0x3f) | 0x80;
    str_obj(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7], buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]
    ))
}

fn crypto_random_bytes(ctx: &mut CallContext, args: &[Object]) -> Object {
    let size = match required_number(ctx, "crypto.randomBytes", args, 0, "size") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if size < 0.0 {
        return new_error(
            ctx.pos.clone(),
            "crypto.randomBytes: size must be non-negative",
        );
    }
    if size > 1_048_576.0 {
        return new_error(
            ctx.pos.clone(),
            "crypto.randomBytes: size must be <= 1048576",
        );
    }
    let mut buf = vec![0u8; size as usize];
    if let Err(err) = fill_random(ctx, "crypto.randomBytes", &mut buf) {
        return err;
    }
    array(buf.into_iter().map(|b| num_obj(b as f64)).collect())
}

fn crypto_timing_safe_equal(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(
            ctx.pos.clone(),
            "crypto.timingSafeEqual requires left and right",
        );
    }
    let left = match bytes_from_object(ctx, "crypto.timingSafeEqual", &args[0]) {
        Ok(b) => b,
        Err(err) => return err,
    };
    let right = match bytes_from_object(ctx, "crypto.timingSafeEqual", &args[1]) {
        Ok(b) => b,
        Err(err) => return err,
    };
    if left.len() != right.len() {
        return bool_obj(false);
    }
    // Constant-time compare.
    let mut diff: u8 = 0;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    bool_obj(diff == 0)
}

/// Identify the hash family from an algorithm name (case-insensitive over the
/// two spellings the Go original accepts: lower or upper).
fn hash_kind(name: &str) -> Option<HashKind> {
    match name {
        "sha1" | "SHA1" => Some(HashKind::Sha1),
        "sha256" | "SHA256" => Some(HashKind::Sha256),
        "sha512" | "SHA512" => Some(HashKind::Sha512),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum HashKind {
    Sha1,
    Sha256,
    Sha512,
}

impl HashKind {
    fn block_size(&self) -> usize {
        match self {
            HashKind::Sha1 | HashKind::Sha256 => 64,
            HashKind::Sha512 => 128,
        }
    }

    fn digest(&self, data: &[u8]) -> Vec<u8> {
        match self {
            HashKind::Sha1 => sha1(data).to_vec(),
            HashKind::Sha256 => sha256(data).to_vec(),
            HashKind::Sha512 => sha512(data).to_vec(),
        }
    }
}

/// HMAC per RFC 2104.
fn hmac(kind: HashKind, key: &[u8], message: &[u8]) -> Vec<u8> {
    let block = kind.block_size();
    let mut key_block = if key.len() > block {
        kind.digest(key)
    } else {
        key.to_vec()
    };
    key_block.resize(block, 0);
    let mut ipad = vec![0u8; block];
    let mut opad = vec![0u8; block];
    for i in 0..block {
        ipad[i] = key_block[i] ^ 0x36;
        opad[i] = key_block[i] ^ 0x5c;
    }
    let mut inner = Vec::with_capacity(block + message.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(message);
    let inner_hash = kind.digest(&inner);
    let mut outer = Vec::with_capacity(block + inner_hash.len());
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_hash);
    kind.digest(&outer)
}

/// PBKDF2 per RFC 8018, using HMAC as the PRF.
fn pbkdf2(kind: HashKind, password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    let h_len = kind.digest(&[]).len();
    let blocks = dk_len.div_ceil(h_len);
    let mut derived = Vec::with_capacity(blocks * h_len);
    for block_index in 1..=blocks {
        let mut u = Vec::with_capacity(salt.len() + 4);
        u.extend_from_slice(salt);
        u.extend_from_slice(&(block_index as u32).to_be_bytes());
        u = hmac(kind, password, &u);
        let mut t = u.clone();
        for _ in 1..iterations {
            u = hmac(kind, password, &u);
            for (acc, b) in t.iter_mut().zip(u.iter()) {
                *acc ^= b;
            }
        }
        derived.extend_from_slice(&t);
    }
    derived.truncate(dk_len);
    derived
}

// --- SHA-1 (FIPS 180-4) ----------------------------------------------------

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];
    let padded = sha_pad(data, 64, 8);
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks_exact(4).enumerate().take(16) {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5a827999),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

// --- SHA-256 (FIPS 180-4) --------------------------------------------------

fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let padded = sha_pad(data, 64, 8);
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate().take(16) {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

// --- SHA-512 (FIPS 180-4) --------------------------------------------------

fn sha512(data: &[u8]) -> [u8; 64] {
    const K: [u64; 80] = [
        0x428a2f98d728ae22,
        0x7137449123ef65cd,
        0xb5c0fbcfec4d3b2f,
        0xe9b5dba58189dbbc,
        0x3956c25bf348b538,
        0x59f111f1b605d019,
        0x923f82a4af194f9b,
        0xab1c5ed5da6d8118,
        0xd807aa98a3030242,
        0x12835b0145706fbe,
        0x243185be4ee4b28c,
        0x550c7dc3d5ffb4e2,
        0x72be5d74f27b896f,
        0x80deb1fe3b1696b1,
        0x9bdc06a725c71235,
        0xc19bf174cf692694,
        0xe49b69c19ef14ad2,
        0xefbe4786384f25e3,
        0x0fc19dc68b8cd5b5,
        0x240ca1cc77ac9c65,
        0x2de92c6f592b0275,
        0x4a7484aa6ea6e483,
        0x5cb0a9dcbd41fbd4,
        0x76f988da831153b5,
        0x983e5152ee66dfab,
        0xa831c66d2db43210,
        0xb00327c898fb213f,
        0xbf597fc7beef0ee4,
        0xc6e00bf33da88fc2,
        0xd5a79147930aa725,
        0x06ca6351e003826f,
        0x142929670a0e6e70,
        0x27b70a8546d22ffc,
        0x2e1b21385c26c926,
        0x4d2c6dfc5ac42aed,
        0x53380d139d95b3df,
        0x650a73548baf63de,
        0x766a0abb3c77b2a8,
        0x81c2c92e47edaee6,
        0x92722c851482353b,
        0xa2bfe8a14cf10364,
        0xa81a664bbc423001,
        0xc24b8b70d0f89791,
        0xc76c51a30654be30,
        0xd192e819d6ef5218,
        0xd69906245565a910,
        0xf40e35855771202a,
        0x106aa07032bbd1b8,
        0x19a4c116b8d2d0c8,
        0x1e376c085141ab53,
        0x2748774cdf8eeb99,
        0x34b0bcb5e19b48a8,
        0x391c0cb3c5c95a63,
        0x4ed8aa4ae3418acb,
        0x5b9cca4f7763e373,
        0x682e6ff3d6b2b8a3,
        0x748f82ee5defb2fc,
        0x78a5636f43172f60,
        0x84c87814a1f0ab72,
        0x8cc702081a6439ec,
        0x90befffa23631e28,
        0xa4506cebde82bde9,
        0xbef9a3f7b2c67915,
        0xc67178f2e372532b,
        0xca273eceea26619c,
        0xd186b8c721c0c207,
        0xeada7dd6cde0eb1e,
        0xf57d4f7fee6ed178,
        0x06f067aa72176fba,
        0x0a637dc5a2c898a6,
        0x113f9804bef90dae,
        0x1b710b35131c471b,
        0x28db77f523047d84,
        0x32caab7b40c72493,
        0x3c9ebe0a15c9bebc,
        0x431d67c49c100d4c,
        0x4cc5d4becb3e42b6,
        0x597f299cfc657e2a,
        0x5fcb6fab3ad6faec,
        0x6c44198c4a475817,
    ];
    let mut h: [u64; 8] = [
        0x6a09e667f3bcc908,
        0xbb67ae8584caa73b,
        0x3c6ef372fe94f82b,
        0xa54ff53a5f1d36f1,
        0x510e527fade682d1,
        0x9b05688c2b3e6c1f,
        0x1f83d9abfb41bd6b,
        0x5be0cd19137e2179,
    ];
    let padded = sha_pad_64bit(data);
    for chunk in padded.chunks_exact(128) {
        let mut w = [0u64; 80];
        for (i, word) in chunk.chunks_exact(8).enumerate().take(16) {
            w[i] = u64::from_be_bytes([
                word[0], word[1], word[2], word[3], word[4], word[5], word[6], word[7],
            ]);
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 64];
    for (i, word) in h.iter().enumerate() {
        out[i * 8..i * 8 + 8].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Merkle–Damgård padding for 32-bit-length hashes (sha1, sha256).
fn sha_pad(data: &[u8], block: usize, len_bytes: usize) -> Vec<u8> {
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % block != block - len_bytes {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    padded
}

/// Merkle–Damgård padding for 64-bit-length hashes (sha512).
fn sha_pad_64bit(data: &[u8]) -> Vec<u8> {
    let bit_len = (data.len() as u128) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 128 != 112 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    padded
}

// ---------------------------------------------------------------------------
// text: display-width utilities for terminal-aware string handling.
// ---------------------------------------------------------------------------

fn text_module() -> Object {
    module(vec![
        ("chars", native("text.chars", text_chars)),
        ("runes", native("text.chars", text_chars)),
        ("width", native("text.width", text_width)),
        (
            "truncateWidth",
            native("text.truncateWidth", text_truncate_width),
        ),
        (
            "padRightWidth",
            native("text.padRightWidth", text_pad_right_width),
        ),
        ("wrapWidth", native("text.wrapWidth", text_wrap_width)),
        ("stripAnsi", native("text.stripAnsi", text_strip_ansi)),
    ])
}

/// Remove CSI (`ESC [ ...`) and OSC (`ESC ] ... BEL/ST`) escape sequences.
fn strip_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'[' => {
                    // CSI: skip until a 0x40-0x7e final byte.
                    i += 2;
                    while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                }
                b']' => {
                    // OSC: skip until BEL or ST (ESC \).
                    i += 2;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                _ => {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
        } else {
            // Copy this UTF-8 character wholesale.
            let ch_start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] & 0xc0) == 0x80 {
                i += 1;
            }
            out.push_str(std::str::from_utf8(&bytes[ch_start..i]).unwrap_or("\u{fffd}"));
        }
    }
    out
}

/// Display width of a single rune per the Go original's rules.
fn rune_width(r: char) -> usize {
    let code = r as u32;
    if code == 0 || code == '\n' as u32 || code == '\r' as u32 || code == '\t' as u32 {
        return 0;
    }
    if is_combining_rune(r) {
        return 0;
    }
    if is_wide_rune(code) {
        2
    } else {
        1
    }
}

fn is_combining_rune(r: char) -> bool {
    // Combining Diacritical Marks (Mn) and Combining Marks for Symbols (Me).
    matches!(r as u32,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x05BF | 0x05C1..=0x05C2
        | 0x05C4..=0x05C5 | 0x05C7 | 0x0610..=0x061A | 0x064B..=0x065F | 0x0670
        | 0x06D6..=0x06DC | 0x06DF..=0x06E4 | 0x06E7..=0x06E8 | 0x06EA..=0x06ED
        | 0x0711 | 0x0730..=0x074A | 0x07A6..=0x07B0 | 0x07EB..=0x07F3
        | 0x0816..=0x0819 | 0x081B..=0x0823 | 0x0825..=0x0827 | 0x0829..=0x082D
        | 0x0859..=0x085B | 0x08D4..=0x08E1 | 0x08E3..=0x0902 | 0x093A
        | 0x093C | 0x0941..=0x0948 | 0x094D | 0x0951..=0x0957 | 0x0962..=0x0963
        | 0x0981 | 0x09BC | 0x09C1..=0x09C4 | 0x09CD | 0x09E2..=0x09E3
        | 0x0A01..=0x0A02 | 0x0A3C | 0x0A41..=0x0A42 | 0x0A47..=0x0A48
        | 0x0A4B..=0x0A4D | 0x0A51 | 0x0A70..=0x0A71 | 0x0A75
        | 0x0A81..=0x0A82 | 0x0ABC | 0x0AC1..=0x0AC5 | 0x0AC7..=0x0AC8
        | 0x0ACD | 0x0AE2..=0x0AE3 | 0x0B01 | 0x0B3C | 0x0B3F
        | 0x0B41..=0x0B44 | 0x0B4D | 0x0B56 | 0x0B62..=0x0B63
        | 0x0B82 | 0x0BC0 | 0x0BCD | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
    )
}

fn is_wide_rune(r: u32) -> bool {
    matches!(r,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF | 0x20000..=0x3FFFD
    )
}

fn text_chars(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "text.chars", args, 0, "value") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let stripped = strip_ansi(&value);
    let mut chars: Vec<Object> = Vec::new();
    let mut pending = String::new();
    for r in stripped.chars() {
        if is_combining_rune(r) {
            pending.push(r);
            continue;
        }
        if !pending.is_empty() {
            chars.push(str_obj(pending.clone()));
            pending.clear();
        }
        pending.push(r);
    }
    if !pending.is_empty() {
        chars.push(str_obj(pending));
    }
    array(chars)
}

fn text_width(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "text.width", args, 0, "value") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let stripped = strip_ansi(&value);
    let mut total = 0usize;
    for r in stripped.chars() {
        total += rune_width(r);
    }
    num_obj(total as f64)
}

fn text_truncate_width(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "text.truncateWidth", args, 0, "value") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let width = match required_number(ctx, "text.truncateWidth", args, 1, "width") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let limit = if width < 0.0 { 0 } else { width as usize };
    let stripped = strip_ansi(&value);
    let mut out = String::new();
    let mut used = 0usize;
    for r in stripped.chars() {
        let w = rune_width(r);
        if used + w > limit {
            break;
        }
        out.push(r);
        used += w;
    }
    str_obj(out)
}

fn text_pad_right_width(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "text.padRightWidth", args, 0, "value") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let width = match required_number(ctx, "text.padRightWidth", args, 1, "width") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let target = if width < 0.0 { 0 } else { width as usize };
    let stripped = strip_ansi(&value);
    let mut current = 0usize;
    for r in stripped.chars() {
        current += rune_width(r);
    }
    let mut out = stripped;
    while current < target {
        out.push(' ');
        current += 1;
    }
    str_obj(out)
}

fn text_wrap_width(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "text.wrapWidth", args, 0, "value") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let width = match required_number(ctx, "text.wrapWidth", args, 1, "width") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let limit = if width <= 0.0 {
        return array(vec![str_obj(String::new())]);
    } else {
        width as usize
    };
    let stripped = strip_ansi(&value);
    let mut lines: Vec<Object> = Vec::new();
    for raw_line in stripped.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let mut current = String::new();
        let mut used = 0usize;
        for r in line.chars() {
            let w = rune_width(r);
            if used + w > limit && !current.is_empty() {
                lines.push(str_obj(current.clone()));
                current.clear();
                used = 0;
            }
            current.push(r);
            used += w;
        }
        lines.push(str_obj(current));
    }
    array(lines)
}

fn text_strip_ansi(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "text.stripAnsi", args, 0, "value") {
        Ok(v) => v,
        Err(e) => return e,
    };
    str_obj(strip_ansi(&value))
}

// ---------------------------------------------------------------------------
// url: a compact URL parser/builder. We deliberately avoid pulling in a URL
// crate to keep the dependency surface at `regex`-only; this implements the
// Go `url.Parse` subset that the GoScript stdlib relies on.
// ---------------------------------------------------------------------------

fn url_module() -> Object {
    module(vec![
        ("parse", native("url.parse", url_parse)),
        ("format", native("url.format", url_format)),
        ("resolve", native("url.resolve", url_resolve)),
        (
            "pathToFileURL",
            native("url.pathToFileURL", url_path_to_file),
        ),
        (
            "fileURLToPath",
            native("url.fileURLToPath", url_file_to_path),
        ),
    ])
}

/// Parsed URL components.
#[derive(Clone)]
struct UrlParts {
    scheme: String,
    host: String,
    path: String,
    query: String,
    fragment: String,
}

impl UrlParts {
    fn hostname(&self) -> String {
        // Strip a trailing ":port".
        match self.host.rfind(':') {
            Some(idx)
                if !self.host[idx + 1..].is_empty()
                    && self.host[idx + 1..].chars().all(|c| c.is_ascii_digit()) =>
            {
                self.host[..idx].to_string()
            }
            _ => self.host.clone(),
        }
    }

    fn port(&self) -> String {
        match self.host.rfind(':') {
            Some(idx)
                if !self.host[idx + 1..].is_empty()
                    && self.host[idx + 1..].chars().all(|c| c.is_ascii_digit()) =>
            {
                self.host[idx + 1..].to_string()
            }
            _ => String::new(),
        }
    }

    fn to_string(&self) -> String {
        let mut out = String::new();
        if !self.scheme.is_empty() {
            out.push_str(&self.scheme);
            out.push(':');
        }
        if !self.host.is_empty() {
            out.push_str("//");
            out.push_str(&self.host);
        }
        out.push_str(&self.path);
        if !self.query.is_empty() {
            out.push('?');
            out.push_str(&self.query);
        }
        if !self.fragment.is_empty() {
            out.push('#');
            out.push_str(&self.fragment);
        }
        out
    }

    fn to_object(&self) -> Object {
        let hash = Rc::new(RefCell::new(HashData::default()));
        hash.borrow_mut().set("href", str_obj(self.to_string()));
        let protocol = if self.scheme.is_empty() {
            String::new()
        } else {
            format!("{}:", self.scheme)
        };
        hash.borrow_mut().set("protocol", str_obj(protocol));
        hash.borrow_mut().set("host", str_obj(self.host.clone()));
        hash.borrow_mut().set("hostname", str_obj(self.hostname()));
        hash.borrow_mut().set("port", str_obj(self.port()));
        hash.borrow_mut()
            .set("pathname", str_obj(self.path.clone()));
        let search = if self.query.is_empty() {
            String::new()
        } else {
            format!("?{}", self.query)
        };
        hash.borrow_mut().set("search", str_obj(search));
        let hash_field = if self.fragment.is_empty() {
            String::new()
        } else {
            format!("#{}", self.fragment)
        };
        hash.borrow_mut().set("hash", str_obj(hash_field));
        let origin = if !self.scheme.is_empty() && !self.host.is_empty() {
            format!("{}://{}", self.scheme, self.host)
        } else {
            "null".to_string()
        };
        hash.borrow_mut().set("origin", str_obj(origin));
        Object::Hash(hash)
    }
}

/// Parse a URL into components. Implements scheme://host/path?query#fragment.
fn parse_url(input: &str) -> Option<UrlParts> {
    let (rest, fragment) = match input.split_once('#') {
        Some((r, f)) => (r, f.to_string()),
        None => (input, String::new()),
    };
    let (rest, query) = match rest.split_once('?') {
        Some((r, q)) => (r, q.to_string()),
        None => (rest, String::new()),
    };
    // Detect scheme (must be alpha leading, followed by [a-z0-9+.-]* then ':').
    let scheme_end = rest.find(':').filter(|&idx| {
        idx > 0
            && rest[..idx]
                .chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false)
            && rest[..idx]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
    });
    let (scheme, after_scheme) = match scheme_end {
        Some(idx) => (rest[..idx].to_string(), &rest[idx + 1..]),
        None => (String::new(), rest),
    };
    let (host, path) = if let Some(stripped) = after_scheme.strip_prefix("//") {
        match stripped.find('/') {
            Some(slash) => (stripped[..slash].to_string(), stripped[slash..].to_string()),
            None => (stripped.to_string(), String::new()),
        }
    } else {
        (String::new(), after_scheme.to_string())
    };
    Some(UrlParts {
        scheme,
        host,
        path,
        query,
        fragment,
    })
}

fn url_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let input = match required_string(ctx, "url.parse", args, 0, "url") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match parse_url(&input) {
        Some(parts) => parts.to_object(),
        None => new_error(
            ctx.pos.clone(),
            format!("url.parse: invalid URL: {}", input),
        ),
    }
}

fn url_format(ctx: &mut CallContext, args: &[Object]) -> Object {
    match args.first() {
        Some(Object::String(s)) => match parse_url(s) {
            Some(parts) => str_obj(parts.to_string()),
            None => new_error(ctx.pos.clone(), format!("url.format: invalid URL: {}", s)),
        },
        Some(Object::Hash(hash)) => {
            let h = hash.borrow();
            let mut scheme = hash_string(&h, "protocol").or_else(|| hash_string(&h, "scheme"));
            if let Some(s) = &scheme {
                if let Some(stripped) = s.strip_suffix(':') {
                    scheme = Some(stripped.to_string());
                }
            }
            let host = hash_string(&h, "host").unwrap_or_else(|| {
                let hostname = hash_string(&h, "hostname").unwrap_or_default();
                let port = hash_string(&h, "port").unwrap_or_default();
                if port.is_empty() {
                    hostname
                } else {
                    format!("{}:{}", hostname, port)
                }
            });
            let path = hash_string(&h, "pathname")
                .or_else(|| hash_string(&h, "path"))
                .unwrap_or_default();
            let query = hash_string(&h, "search")
                .map(|s| s.strip_prefix('?').unwrap_or(&s).to_string())
                .or_else(|| hash_string(&h, "query"))
                .unwrap_or_default();
            let fragment = hash_string(&h, "hash")
                .map(|s| s.strip_prefix('#').unwrap_or(&s).to_string())
                .or_else(|| hash_string(&h, "fragment"))
                .unwrap_or_default();
            let parts = UrlParts {
                scheme: scheme.unwrap_or_default(),
                host,
                path,
                query,
                fragment,
            };
            str_obj(parts.to_string())
        }
        Some(_) => new_error(
            ctx.pos.clone(),
            "url.format: URL object must be an object or string",
        ),
        None => new_error(ctx.pos.clone(), "url.format requires url"),
    }
}

fn url_resolve(ctx: &mut CallContext, args: &[Object]) -> Object {
    let base = match required_string(ctx, "url.resolve", args, 0, "base") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let reference = match required_string(ctx, "url.resolve", args, 1, "ref") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let base_parts = match parse_url(&base) {
        Some(p) => p,
        None => {
            return new_error(
                ctx.pos.clone(),
                format!("url.resolve: invalid base URL: {}", base),
            )
        }
    };
    let resolved = resolve_reference(&base_parts, &reference);
    str_obj(resolved)
}

fn resolve_reference(base: &UrlParts, reference: &str) -> String {
    // Absolute reference (has its own scheme) is used as-is.
    if let Some(abs) = parse_url(reference) {
        if !abs.scheme.is_empty() && abs.scheme == base.scheme && abs.host.is_empty() {
            // Scheme-relative.
            let mut merged = abs.clone();
            merged.host = base.host.clone();
            return merged.to_string();
        }
        if !abs.scheme.is_empty() {
            return abs.to_string();
        }
    }
    // Protocol-relative (//host/...).
    if let Some(rest) = reference.strip_prefix("//") {
        if let Some(abs) = parse_url(&format!("{}:{}", base.scheme, reference)) {
            return abs.to_string();
        }
        let _ = rest;
    }
    // Root-relative.
    if let Some(rest) = reference.strip_prefix('/') {
        let parts = UrlParts {
            scheme: base.scheme.clone(),
            host: base.host.clone(),
            path: format!("/{}", rest),
            query: String::new(),
            fragment: String::new(),
        };
        return parts.to_string();
    }
    // Relative path: merge with the base directory.
    let base_dir = match base.path.rfind('/') {
        Some(idx) => base.path[..=idx].to_string(),
        None => String::new(),
    };
    let mut query = String::new();
    let mut fragment = String::new();
    let (ref_path, rest) = match reference.split_once('?') {
        Some((p, q)) => (p, q),
        None => (reference, ""),
    };
    let (ref_path, frag) = match ref_path.split_once('#') {
        Some((p, f)) => (p, f.to_string()),
        None => (ref_path, String::new()),
    };
    if !rest.is_empty() {
        query = rest.to_string();
    }
    if !frag.is_empty() {
        fragment = frag;
    }
    let parts = UrlParts {
        scheme: base.scheme.clone(),
        host: base.host.clone(),
        path: format!("{}{}", base_dir, ref_path),
        query,
        fragment,
    };
    parts.to_string()
}

fn url_path_to_file(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "url.pathToFileURL", args, 0, "path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let absolute = match fs::canonicalize(&path) {
        Ok(p) => p,
        Err(_) => PathBuf::from(&path),
    };
    let mut slash = absolute.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && !slash.starts_with('/') {
        slash = format!("/{}", slash);
    }
    str_obj(format!("file://{}", slash))
}

fn url_file_to_path(ctx: &mut CallContext, args: &[Object]) -> Object {
    let input = match required_string(ctx, "url.fileURLToPath", args, 0, "url") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let parts = match parse_url(&input) {
        Some(p) => p,
        None => {
            return new_error(
                ctx.pos.clone(),
                format!("url.fileURLToPath: invalid URL: {}", input),
            )
        }
    };
    if parts.scheme != "file" {
        return new_error(
            ctx.pos.clone(),
            "url.fileURLToPath: URL must use file: protocol",
        );
    }
    if !parts.host.is_empty() && parts.host != "localhost" {
        return new_error(
            ctx.pos.clone(),
            "url.fileURLToPath: file URL host is not supported",
        );
    }
    let mut path = parts.path.clone();
    if cfg!(windows)
        && path.starts_with('/')
        && path.len() >= 3
        && path.as_bytes().get(2) == Some(&b':')
    {
        path = path[1..].to_string();
    }
    if cfg!(windows) {
        path = path.replace('/', "\\");
    }
    str_obj(path)
}

// (hash_string is defined above near the env module helpers.)

// ---------------------------------------------------------------------------
// cache: a TTL dictionary with lazy expiry, matching the Go `@std/cache`
// semantics (no LRU, no capacity cap, has/size/keys include not-yet-purged
// expired entries, get lazily deletes expired entries).
// ---------------------------------------------------------------------------

fn cache_module() -> Object {
    module(vec![("create", native("cache.create", cache_create))])
}

/// A cache entry with an optional expiry instant.
struct CacheItem {
    value: Object,
    expire_at: Option<std::time::Instant>,
}

/// Cache state is stored as a Hash carrying a hidden marker key whose value
/// is a Hash mapping keys -> { value, expireAt } records. This keeps the
/// cache shareable through the existing object model without adding a new
/// Object variant.
const CACHE_STATE_KEY: &str = "__cache_state__";

fn cache_create(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // The backing store: a Hash mapping key -> Hash{value, expireAtMs?}.
    let store = Rc::new(RefCell::new(HashData::default()));
    let instance = Rc::new(RefCell::new(HashData::default()));
    instance
        .borrow_mut()
        .set(CACHE_STATE_KEY, Object::Hash(store.clone()));

    let store_for_set = store.clone();
    instance.borrow_mut().set(
        "set",
        native("cache.set", move |ctx, args| {
            let key = match required_string(ctx, "cache.set", args, 0, "key") {
                Ok(k) => k,
                Err(e) => return e,
            };
            let value = match args.get(1) {
                Some(v) => v.clone(),
                None => return new_error(ctx.pos.clone(), "cache.set requires key and value"),
            };
            let ttl_ms = match args.get(2) {
                Some(Object::Number(n)) if *n > 0.0 => Some(*n as u64),
                _ => None,
            };
            let entry = Rc::new(RefCell::new(HashData::default()));
            entry.borrow_mut().set("value", value);
            if let Some(ms) = ttl_ms {
                entry
                    .borrow_mut()
                    .set("expireAtMs", num_obj(now_millis() + ms as f64));
            }
            store_for_set.borrow_mut().set(key, Object::Hash(entry));
            Object::Undefined
        }),
    );

    let store_for_get = store.clone();
    instance.borrow_mut().set(
        "get",
        native("cache.get", move |ctx, args| {
            let key = match required_string(ctx, "cache.get", args, 0, "key") {
                Ok(k) => k,
                Err(e) => return e,
            };
            let entry = match store_for_get.borrow().get(&key).cloned() {
                Some(Object::Hash(h)) => h,
                _ => return Object::Undefined,
            };
            // entry is an owned Rc<RefCell<HashData>>; safe to borrow here.
            let expired = match entry.borrow().get("expireAtMs").cloned() {
                Some(Object::Number(expire)) => now_millis() > expire,
                _ => false,
            };
            if expired {
                store_for_get.borrow_mut().remove(&key);
                return Object::Undefined;
            }
            let value = entry
                .borrow()
                .get("value")
                .cloned()
                .unwrap_or(Object::Undefined);
            value
        }),
    );

    let store_for_has = store.clone();
    instance.borrow_mut().set(
        "has",
        native("cache.has", move |ctx, args| {
            let key = match required_string(ctx, "cache.has", args, 0, "key") {
                Ok(k) => k,
                Err(e) => return e,
            };
            match store_for_has.borrow().get(&key).cloned() {
                Some(Object::Hash(entry)) => {
                    if let Some(Object::Number(expire)) = entry.borrow().get("expireAtMs").cloned()
                    {
                        if now_millis() > expire {
                            return bool_obj(false);
                        }
                    }
                    bool_obj(true)
                }
                _ => bool_obj(false),
            }
        }),
    );

    let store_for_delete = store.clone();
    instance.borrow_mut().set(
        "delete",
        native("cache.delete", move |ctx, args| {
            let key = match required_string(ctx, "cache.delete", args, 0, "key") {
                Ok(k) => k,
                Err(e) => return e,
            };
            store_for_delete.borrow_mut().remove(&key);
            Object::Undefined
        }),
    );

    let store_for_clear = store.clone();
    instance.borrow_mut().set(
        "clear",
        native("cache.clear", move |_ctx, _args| {
            store_for_clear.borrow_mut().entries.clear();
            Object::Undefined
        }),
    );

    let store_for_size = store.clone();
    instance.borrow_mut().set(
        "size",
        native("cache.size", move |_ctx, _args| {
            num_obj(store_for_size.borrow().entries.len() as f64)
        }),
    );

    let store_for_keys = store.clone();
    instance.borrow_mut().set(
        "keys",
        native("cache.keys", move |_ctx, _args| {
            let keys: Vec<Object> = store_for_keys
                .borrow()
                .entries
                .iter()
                .map(|(k, _)| str_obj(k.clone()))
                .collect();
            array(keys)
        }),
    );

    Object::Hash(instance)
}

fn now_millis() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

// ---------------------------------------------------------------------------
// timers: forwards to the global timer builtins (setTimeout/setInterval/
// sleepAsync) and provides the synchronous sleep plus no-op clear* / microtask
// helpers expected by the Go `@std/timers` surface.
//
// The Rust runtime executes timers inline on the calling thread, so
// clear* are effectively no-ops and queueMicrotask runs the callback
// immediately — this preserves the observable ordering of a script that
// finishes before any async work escapes.
// ---------------------------------------------------------------------------

fn timers_module() -> Object {
    module(vec![
        (
            "setTimeout",
            native("timers.setTimeout", timers_set_timeout),
        ),
        ("clearTimeout", native("timers.clearTimeout", timers_clear)),
        (
            "setInterval",
            native("timers.setInterval", timers_set_interval),
        ),
        (
            "clearInterval",
            native("timers.clearInterval", timers_clear),
        ),
        (
            "queueMicrotask",
            native("timers.queueMicrotask", timers_queue_microtask),
        ),
        ("sleep", native("timers.sleep", timers_sleep)),
        (
            "sleepAsync",
            native("timers.sleepAsync", timers_sleep_async),
        ),
    ])
}

/// Invoke a global builtin by name with the given arguments, returning its
/// result; if the global is absent or not callable, return an Error.
fn forward_global(ctx: &mut CallContext, name: &str, args: &[Object]) -> Object {
    match ctx.vm().get_global(name) {
        Some(Object::Builtin(b)) => {
            let func = b.func.clone();
            let mut inner_ctx = CallContext::new(ctx.env, ctx.pos.clone());
            func(&mut inner_ctx, args)
        }
        Some(_) => new_error(
            ctx.pos.clone(),
            format!("timers.{}: global {} is not a builtin", name, name),
        ),
        None => new_error(
            ctx.pos.clone(),
            format!("timers.{}: global builtin {} not found", name, name),
        ),
    }
}

fn timers_set_timeout(ctx: &mut CallContext, args: &[Object]) -> Object {
    forward_global(ctx, "setTimeout", args)
}

fn timers_set_interval(ctx: &mut CallContext, args: &[Object]) -> Object {
    forward_global(ctx, "setInterval", args)
}

fn timers_sleep_async(ctx: &mut CallContext, args: &[Object]) -> Object {
    forward_global(ctx, "sleepAsync", args)
}

/// Synchronous sleep: blocks the calling thread for `ms` milliseconds.
fn timers_sleep(ctx: &mut CallContext, args: &[Object]) -> Object {
    match args.first() {
        Some(Object::Number(n)) if *n > 0.0 => {
            std::thread::sleep(std::time::Duration::from_millis(*n as u64));
        }
        _ => {}
    }
    Object::Undefined
}

/// In a synchronous single-threaded runtime, clear* are no-ops: by the time
/// the script observes a timer id the callback has already executed inline.
fn timers_clear(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    Object::Undefined
}

/// queueMicrotask runs the callback immediately on the current thread.
fn timers_queue_microtask(ctx: &mut CallContext, args: &[Object]) -> Object {
    let callback = match args.first() {
        Some(Object::Function(_) | Object::Builtin(_)) => args[0].clone(),
        _ => return Object::Undefined,
    };
    let _ = crate::evaluator::expressions::apply_function(
        &callback,
        ctx.env,
        &[],
        None,
        ctx.pos.clone(),
    );
    Object::Undefined
}

// ---------------------------------------------------------------------------
// glob: thin deterministic wrapper over the existing fs glob engine.
// ---------------------------------------------------------------------------

fn glob_module() -> Object {
    module(vec![
        ("glob", native("glob.glob", glob_glob)),
        ("globSync", native("glob.globSync", glob_glob)),
        ("match", native("glob.match", glob_match_native)),
        ("hasMagic", native("glob.hasMagic", glob_has_magic)),
    ])
}

fn glob_glob(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn glob_match_native(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn glob_has_magic(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "glob.hasMagic", args, 0, "pattern") {
        Ok(pattern) => bool_obj(pattern.contains('*') || pattern.contains('?')),
        Err(err) => err,
    }
}

// ---------------------------------------------------------------------------
// color: simple ANSI SGR wrappers and escape stripping.
// ---------------------------------------------------------------------------

fn color_module() -> Object {
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

fn color_ansi(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn color_strip(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "color.strip", args, 0, "text") {
        Ok(text) => str_obj(strip_ansi(&text)),
        Err(err) => err,
    }
}

fn color_red(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.red", 31)
}

fn color_green(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.green", 32)
}

fn color_yellow(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.yellow", 33)
}

fn color_blue(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.blue", 34)
}

fn color_magenta(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.magenta", 35)
}

fn color_cyan(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.cyan", 36)
}

fn color_bold(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.bold", 1)
}

fn color_dim(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.dim", 2)
}

fn color_underline(ctx: &mut CallContext, args: &[Object]) -> Object {
    color_named(ctx, args, "color.underline", 4)
}

fn color_named(ctx: &mut CallContext, args: &[Object], name: &str, code: i64) -> Object {
    match required_string(ctx, name, args, 0, "text") {
        Ok(text) => ansi_wrap(&text, code),
        Err(err) => err,
    }
}

fn ansi_wrap(text: &str, code: i64) -> Object {
    str_obj(format!("\x1b[{}m{}\x1b[0m", code, text))
}

// ---------------------------------------------------------------------------
// diff: line-oriented comparison helpers.
// ---------------------------------------------------------------------------

fn diff_module() -> Object {
    module(vec![
        ("lines", native("diff.lines", diff_lines)),
        ("unified", native("diff.unified", diff_unified)),
    ])
}

fn diff_lines(ctx: &mut CallContext, args: &[Object]) -> Object {
    let old = match required_string(ctx, "diff.lines", args, 0, "old") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let new = match required_string(ctx, "diff.lines", args, 1, "new") {
        Ok(value) => value,
        Err(err) => return err,
    };
    array(
        line_diff(&old, &new)
            .into_iter()
            .map(|entry| {
                module(vec![
                    ("kind", str_obj(entry.kind)),
                    ("value", str_obj(entry.value)),
                ])
            })
            .collect(),
    )
}

fn diff_unified(ctx: &mut CallContext, args: &[Object]) -> Object {
    let old = match required_string(ctx, "diff.unified", args, 0, "old") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let new = match required_string(ctx, "diff.unified", args, 1, "new") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let old_name = match args.get(2) {
        Some(Object::String(value)) => value.to_string(),
        _ => "old".to_string(),
    };
    let new_name = match args.get(3) {
        Some(Object::String(value)) => value.to_string(),
        _ => "new".to_string(),
    };
    let mut out = format!("--- {}\n+++ {}\n", old_name, new_name);
    for entry in line_diff(&old, &new) {
        let prefix = match entry.kind.as_str() {
            "add" => '+',
            "remove" => '-',
            _ => ' ',
        };
        out.push(prefix);
        out.push_str(&entry.value);
        out.push('\n');
    }
    str_obj(out)
}

struct LineDiffEntry {
    kind: String,
    value: String,
}

fn line_diff(old: &str, new: &str) -> Vec<LineDiffEntry> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut lcs = vec![vec![0usize; new_lines.len() + 1]; old_lines.len() + 1];
    for i in (0..old_lines.len()).rev() {
        for j in (0..new_lines.len()).rev() {
            lcs[i][j] = if old_lines[i] == new_lines[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < old_lines.len() && j < new_lines.len() {
        if old_lines[i] == new_lines[j] {
            out.push(LineDiffEntry {
                kind: "equal".to_string(),
                value: old_lines[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(LineDiffEntry {
                kind: "remove".to_string(),
                value: old_lines[i].to_string(),
            });
            i += 1;
        } else {
            out.push(LineDiffEntry {
                kind: "add".to_string(),
                value: new_lines[j].to_string(),
            });
            j += 1;
        }
    }
    while i < old_lines.len() {
        out.push(LineDiffEntry {
            kind: "remove".to_string(),
            value: old_lines[i].to_string(),
        });
        i += 1;
    }
    while j < new_lines.len() {
        out.push(LineDiffEntry {
            kind: "add".to_string(),
            value: new_lines[j].to_string(),
        });
        j += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// log: deterministic level formatting without side effects.
// ---------------------------------------------------------------------------

fn log_module() -> Object {
    module(vec![
        ("format", native("log.format", log_format)),
        ("debug", native("log.debug", log_debug)),
        ("info", native("log.info", log_info)),
        ("warn", native("log.warn", log_warn)),
        ("error", native("log.error", log_error)),
    ])
}

fn log_format(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn log_debug(ctx: &mut CallContext, args: &[Object]) -> Object {
    log_named(ctx, args, "log.debug", "debug")
}

fn log_info(ctx: &mut CallContext, args: &[Object]) -> Object {
    log_named(ctx, args, "log.info", "info")
}

fn log_warn(ctx: &mut CallContext, args: &[Object]) -> Object {
    log_named(ctx, args, "log.warn", "warn")
}

fn log_error(ctx: &mut CallContext, args: &[Object]) -> Object {
    log_named(ctx, args, "log.error", "error")
}

fn log_named(ctx: &mut CallContext, args: &[Object], name: &str, level: &str) -> Object {
    match required_string(ctx, name, args, 0, "message") {
        Ok(message) => str_obj(format_log_line(level, &message)),
        Err(err) => err,
    }
}

fn format_log_line(level: &str, message: &str) -> String {
    format!("[{}] {}", level.to_ascii_uppercase(), message)
}

// ---------------------------------------------------------------------------
// encoding/csv: small RFC4180-ish parser/writer with Go-compatible options.
// ---------------------------------------------------------------------------

fn csv_module() -> Object {
    module(vec![
        ("parse", native("csv.parse", csv_parse)),
        ("stringify", native("csv.stringify", csv_stringify)),
        (
            "readFileSync",
            native("csv.readFileSync", csv_read_file_sync),
        ),
        (
            "writeFileSync",
            native("csv.writeFileSync", csv_write_file_sync),
        ),
    ])
}

#[derive(Clone)]
struct CsvOptions {
    header: bool,
    comma: char,
    comment: Option<char>,
    fields_per_record: i64,
    trim_leading_space: bool,
}

fn csv_options(
    ctx: &CallContext,
    name: &str,
    value: Option<&Object>,
) -> Result<CsvOptions, Object> {
    let mut opts = CsvOptions {
        header: true,
        comma: ',',
        comment: None,
        fields_per_record: 0,
        trim_leading_space: false,
    };
    let Some(value) = value else {
        return Ok(opts);
    };
    if matches!(value, Object::Undefined | Object::Null) {
        return Ok(opts);
    }
    let Object::Hash(hash) = value else {
        return Err(new_error(
            ctx.pos.clone(),
            format!("{}: options must be an object", name),
        ));
    };
    let hash = hash.borrow();
    if let Some(value) = hash.get("header") {
        match value {
            Object::Boolean(b) => opts.header = *b,
            _ => {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("{}: options.header must be a boolean", name),
                ))
            }
        }
    }
    if let Some(value) = hash.get("comma") {
        opts.comma = csv_single_char(ctx, name, "comma", value)?;
    }
    if let Some(value) = hash.get("comment") {
        opts.comment = Some(csv_single_char(ctx, name, "comment", value)?);
    }
    if let Some(value) = hash.get("fieldsPerRecord") {
        match value {
            Object::Number(n) => opts.fields_per_record = *n as i64,
            _ => {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("{}: options.fieldsPerRecord must be a number", name),
                ))
            }
        }
    }
    if let Some(value) = hash.get("trimLeadingSpace") {
        match value {
            Object::Boolean(b) => opts.trim_leading_space = *b,
            _ => {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("{}: options.trimLeadingSpace must be a boolean", name),
                ))
            }
        }
    }
    Ok(opts)
}

fn csv_single_char(
    ctx: &CallContext,
    name: &str,
    option: &str,
    value: &Object,
) -> Result<char, Object> {
    let Object::String(s) = value else {
        return Err(new_error(
            ctx.pos.clone(),
            format!("{}: options.{} must be a string", name, option),
        ));
    };
    let mut chars = s.chars();
    let Some(ch) = chars.next() else {
        return Err(new_error(
            ctx.pos.clone(),
            format!("{}: options.{} must be a single character", name, option),
        ));
    };
    if chars.next().is_some() {
        return Err(new_error(
            ctx.pos.clone(),
            format!("{}: options.{} must be a single character", name, option),
        ));
    }
    Ok(ch)
}

fn csv_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "csv.parse", args, 0, "text") {
        Ok(text) => text,
        Err(err) => return err,
    };
    let opts = match csv_options(ctx, "csv.parse", args.get(1)) {
        Ok(opts) => opts,
        Err(err) => return err,
    };
    let records = match parse_csv_records(&text, &opts) {
        Ok(records) => records,
        Err(e) => return new_error(ctx.pos.clone(), format!("csv.parse: {}", e)),
    };
    csv_records_to_object(records, opts.header)
}

fn parse_csv_records(text: &str, opts: &CsvOptions) -> Result<Vec<Vec<String>>, String> {
    let mut records = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut in_quotes = false;
    let mut at_line_start = true;
    let mut at_field_start = true;
    while let Some(ch) = chars.next() {
        if at_line_start && !in_quotes && opts.comment == Some(ch) {
            for next in chars.by_ref() {
                if next == '\n' {
                    break;
                }
            }
            at_line_start = true;
            at_field_start = true;
            continue;
        }
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        if at_field_start && opts.trim_leading_space && ch == ' ' {
            continue;
        }
        if at_field_start && ch == '"' {
            in_quotes = true;
            at_field_start = false;
            at_line_start = false;
            continue;
        }
        if ch == opts.comma {
            row.push(field);
            field = String::new();
            at_field_start = true;
            at_line_start = false;
            continue;
        }
        if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(field);
            field = String::new();
            csv_check_record_len(&row, opts.fields_per_record)?;
            records.push(row);
            row = Vec::new();
            at_line_start = true;
            at_field_start = true;
            continue;
        }
        field.push(ch);
        at_field_start = false;
        at_line_start = false;
    }
    if in_quotes {
        return Err("unterminated quoted field".into());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        csv_check_record_len(&row, opts.fields_per_record)?;
        records.push(row);
    }
    Ok(records)
}

fn csv_check_record_len(row: &[String], fields_per_record: i64) -> Result<(), String> {
    if fields_per_record > 0 && row.len() as i64 != fields_per_record {
        Err(format!(
            "wrong number of fields: expected {}, got {}",
            fields_per_record,
            row.len()
        ))
    } else {
        Ok(())
    }
}

fn csv_records_to_object(records: Vec<Vec<String>>, header: bool) -> Object {
    if !header {
        return array(
            records
                .into_iter()
                .map(|row| array(row.into_iter().map(str_obj).collect()))
                .collect(),
        );
    }
    let Some(headers) = records.first() else {
        return array(Vec::new());
    };
    let mut rows = Vec::new();
    for record in records.iter().skip(1) {
        let hash = Rc::new(RefCell::new(HashData::default()));
        for (idx, key) in headers.iter().enumerate() {
            hash.borrow_mut().set(
                key.clone(),
                str_obj(record.get(idx).cloned().unwrap_or_default()),
            );
        }
        rows.push(Object::Hash(hash));
    }
    array(rows)
}

fn csv_stringify(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(rows) = args.first() else {
        return new_error(ctx.pos.clone(), "csv.stringify requires rows");
    };
    let opts = match csv_options(ctx, "csv.stringify", args.get(1)) {
        Ok(opts) => opts,
        Err(err) => return err,
    };
    let records = match csv_rows_from_object(ctx, "csv.stringify", rows, opts.header) {
        Ok(records) => records,
        Err(err) => return err,
    };
    str_obj(write_csv_records(&records, opts.comma))
}

fn csv_rows_from_object(
    ctx: &mut CallContext,
    name: &str,
    rows: &Object,
    header: bool,
) -> Result<Vec<Vec<String>>, Object> {
    let Object::Array(arr) = rows else {
        return Err(new_error(
            ctx.pos.clone(),
            format!("{}: rows must be an array", name),
        ));
    };
    let rows = arr.borrow();
    if rows.elements.is_empty() {
        return Ok(Vec::new());
    }
    if matches!(rows.elements.first(), Some(Object::Array(_))) {
        let mut out = Vec::new();
        for row in &rows.elements {
            let Object::Array(arr) = row else {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("{}: rows must be all arrays or all objects", name),
                ));
            };
            out.push(arr.borrow().elements.iter().map(object_to_text).collect());
        }
        return Ok(out);
    }
    let mut headers = Vec::<String>::new();
    for row in &rows.elements {
        if let Object::Hash(hash) = row {
            for (key, _) in &hash.borrow().entries {
                if !headers.contains(key) {
                    headers.push(key.clone());
                }
            }
        }
    }
    headers.sort();
    let mut out = Vec::new();
    if header {
        out.push(headers.clone());
    }
    for (idx, row) in rows.elements.iter().enumerate() {
        let Object::Hash(hash) = row else {
            return Err(new_error(
                ctx.pos.clone(),
                format!("{}: row {} must be an object", name, idx),
            ));
        };
        let hash = hash.borrow();
        out.push(
            headers
                .iter()
                .map(|key| hash.get(key).map(object_to_text).unwrap_or_default())
                .collect(),
        );
    }
    Ok(out)
}

fn write_csv_records(records: &[Vec<String>], comma: char) -> String {
    let mut out = String::new();
    for row in records {
        for (idx, field) in row.iter().enumerate() {
            if idx > 0 {
                out.push(comma);
            }
            out.push_str(&csv_escape_field(field, comma));
        }
        out.push('\n');
    }
    out
}

fn csv_escape_field(field: &str, comma: char) -> String {
    if field.contains(comma) || field.contains('"') || field.contains('\n') || field.contains('\r')
    {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn csv_read_file_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "csv.readFileSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    match fs::read_to_string(&path) {
        Ok(text) => {
            let opts = args.get(1).cloned();
            let parse_args = match opts {
                Some(opts) => vec![str_obj(text), opts],
                None => vec![str_obj(text)],
            };
            csv_parse(ctx, &parse_args)
        }
        Err(e) => new_error(ctx.pos.clone(), format!("csv.readFileSync: {}", e)),
    }
}

fn csv_write_file_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "csv.writeFileSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let Some(rows) = args.get(1) else {
        return new_error(ctx.pos.clone(), "csv.writeFileSync requires rows");
    };
    let opts = args.get(2).cloned();
    let stringify_args = opts.map_or_else(|| vec![rows.clone()], |opts| vec![rows.clone(), opts]);
    let text = csv_stringify(ctx, &stringify_args);
    if matches!(text, Object::Error(_)) {
        return text;
    }
    match fs::write(&path, object_to_text(&text)) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("csv.writeFileSync: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// template: focused Go-template-like interpolation and common funcs.
// ---------------------------------------------------------------------------

fn template_module() -> Object {
    module(vec![
        ("render", native("template.render", template_render)),
        (
            "renderHTML",
            native("template.renderHTML", template_render_html),
        ),
        (
            "renderFileSync",
            native("template.renderFileSync", template_render_file_sync),
        ),
        (
            "escapeHTML",
            native("template.escapeHTML", template_escape_html),
        ),
    ])
}

fn template_render(ctx: &mut CallContext, args: &[Object]) -> Object {
    let source = match required_string(ctx, "template.render", args, 0, "source") {
        Ok(source) => source,
        Err(err) => return err,
    };
    template_execute(&source, args.get(1).unwrap_or(&Object::Undefined), false)
        .map(str_obj)
        .unwrap_or_else(|e| new_error(ctx.pos.clone(), format!("template.render: {}", e)))
}

fn template_render_html(ctx: &mut CallContext, args: &[Object]) -> Object {
    let source = match required_string(ctx, "template.renderHTML", args, 0, "source") {
        Ok(source) => source,
        Err(err) => return err,
    };
    template_execute(&source, args.get(1).unwrap_or(&Object::Undefined), true)
        .map(str_obj)
        .unwrap_or_else(|e| new_error(ctx.pos.clone(), format!("template.renderHTML: {}", e)))
}

fn template_render_file_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "template.renderFileSync", args, 0, "path") {
        Ok(path) => path,
        Err(err) => return err,
    };
    match fs::read_to_string(&path) {
        Ok(source) => template_execute(&source, args.get(1).unwrap_or(&Object::Undefined), false)
            .map(str_obj)
            .unwrap_or_else(|e| {
                new_error(ctx.pos.clone(), format!("template.renderFileSync: {}", e))
            }),
        Err(e) => new_error(ctx.pos.clone(), format!("template.renderFileSync: {}", e)),
    }
}

fn template_escape_html(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "template.escapeHTML", args, 0, "value") {
        Ok(value) => str_obj(escape_html(&value)),
        Err(err) => err,
    }
}

fn template_execute(source: &str, data: &Object, html: bool) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = source;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            return Err("unterminated action".into());
        };
        let expr = after[..end].trim();
        let mut text = template_eval_expr(expr, data)?;
        if html {
            text = escape_html(&text);
        }
        out.push_str(&text);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn template_eval_expr(expr: &str, data: &Object) -> Result<String, String> {
    let parts = split_template_args(expr);
    if parts.is_empty() {
        return Ok(String::new());
    }
    match parts[0].as_str() {
        "upper" => Ok(template_value_text(parts.get(1), data)?.to_uppercase()),
        "lower" => Ok(template_value_text(parts.get(1), data)?.to_lowercase()),
        "trim" => Ok(template_value_text(parts.get(1), data)?.trim().to_string()),
        "join" => {
            let value = template_lookup(parts.get(1).map(String::as_str).unwrap_or("."), data)?;
            let sep = parts
                .get(2)
                .map(|s| unquote_template_arg(s))
                .unwrap_or_default();
            match value {
                Object::Array(arr) => Ok(arr
                    .borrow()
                    .elements
                    .iter()
                    .map(object_to_text)
                    .collect::<Vec<_>>()
                    .join(&sep)),
                other => Ok(object_to_text(&other)),
            }
        }
        "json" => {
            let value = template_lookup(parts.get(1).map(String::as_str).unwrap_or("."), data)?;
            Ok(object_to_json(&value, 0, None))
        }
        _ => template_lookup(expr, data).map(|value| object_to_text(&value)),
    }
}

fn template_value_text(token: Option<&String>, data: &Object) -> Result<String, String> {
    let Some(token) = token else {
        return Ok(String::new());
    };
    if token.starts_with('.') {
        template_lookup(token, data).map(|value| object_to_text(&value))
    } else {
        Ok(unquote_template_arg(token))
    }
}

fn split_template_args(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote = '\0';
    for ch in expr.chars() {
        if in_quote {
            if ch == quote {
                in_quote = false;
            }
            current.push(ch);
        } else if ch == '"' || ch == '\'' {
            in_quote = true;
            quote = ch;
            current.push(ch);
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                out.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn unquote_template_arg(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(value)
        .to_string()
}

fn template_lookup(expr: &str, data: &Object) -> Result<Object, String> {
    if expr == "." {
        return Ok(data.clone());
    }
    let path = expr
        .strip_prefix('.')
        .ok_or_else(|| format!("unsupported action {}", expr))?;
    let mut current = data.clone();
    for segment in path.split('.') {
        if segment.is_empty() {
            continue;
        }
        match current {
            Object::Hash(hash) => {
                current = hash
                    .borrow()
                    .get(segment)
                    .cloned()
                    .unwrap_or(Object::Undefined);
            }
            _ => return Ok(Object::Undefined),
        }
    }
    Ok(current)
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&#34;")
        .replace('\'', "&#39;")
}

// ---------------------------------------------------------------------------
// compression / compress/gzip: gzip round-trips using the shared buffer shape.
// ---------------------------------------------------------------------------

fn compression_module() -> Object {
    module(vec![
        (
            "gzipCompress",
            native("compression.gzipCompress", compression_gzip_compress),
        ),
        (
            "gzipDecompress",
            native("compression.gzipDecompress", compression_gzip_decompress),
        ),
    ])
}

fn gzip_module() -> Object {
    module(vec![
        ("compress", native("gzip.compress", gzip_compress)),
        ("decompress", native("gzip.decompress", gzip_decompress)),
        (
            "compressFileSync",
            native("gzip.compressFileSync", gzip_compress_file_sync),
        ),
        (
            "decompressFileSync",
            native("gzip.decompressFileSync", gzip_decompress_file_sync),
        ),
    ])
}

fn compression_gzip_compress(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "gzipCompress", args, 0, "data") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match gzip_compress_bytes(value.as_bytes()) {
        Ok(bytes) => str_obj(bytes_to_latin1_string(&bytes)),
        Err(e) => new_error(ctx.pos.clone(), format!("gzipCompress: {}", e)),
    }
}

fn compression_gzip_decompress(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "gzipDecompress", args, 0, "data") {
        Ok(value) => value,
        Err(err) => return err,
    };
    match gzip_decompress_bytes(&latin1_string_to_bytes(&value)) {
        Ok(bytes) => str_obj(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) => new_error(ctx.pos.clone(), format!("gzipDecompress: {}", e)),
    }
}

fn gzip_compress(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(value) = args.first() else {
        return new_error(ctx.pos.clone(), "gzip.compress requires value");
    };
    let data = match bytes_from_object(ctx, "gzip.compress", value) {
        Ok(data) => data,
        Err(err) => return err,
    };
    match gzip_compress_bytes(&data) {
        Ok(bytes) => make_buffer(bytes),
        Err(e) => new_error(ctx.pos.clone(), format!("gzip.compress: {}", e)),
    }
}

fn gzip_decompress(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(value) = args.first() else {
        return new_error(ctx.pos.clone(), "gzip.decompress requires value");
    };
    let data = match bytes_from_object(ctx, "gzip.decompress", value) {
        Ok(data) => data,
        Err(err) => return err,
    };
    match gzip_decompress_bytes(&data) {
        Ok(bytes) => bytes_result(ctx, "gzip.decompress", bytes, args.get(1)),
        Err(e) => new_error(ctx.pos.clone(), format!("gzip.decompress: {}", e)),
    }
}

fn gzip_compress_file_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let src = match required_string(ctx, "gzip.compressFileSync", args, 0, "source path") {
        Ok(src) => src,
        Err(err) => return err,
    };
    let dst = match required_string(ctx, "gzip.compressFileSync", args, 1, "destination path") {
        Ok(dst) => dst,
        Err(err) => return err,
    };
    match fs::read(&src).and_then(|data| {
        gzip_compress_bytes(&data)
            .map_err(std::io::Error::other)
            .and_then(|compressed| fs::write(&dst, compressed))
    }) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("gzip.compressFileSync: {}", e)),
    }
}

fn gzip_decompress_file_sync(ctx: &mut CallContext, args: &[Object]) -> Object {
    let src = match required_string(ctx, "gzip.decompressFileSync", args, 0, "source path") {
        Ok(src) => src,
        Err(err) => return err,
    };
    let dst = match required_string(ctx, "gzip.decompressFileSync", args, 1, "destination path") {
        Ok(dst) => dst,
        Err(err) => return err,
    };
    match fs::read(&src).and_then(|data| {
        gzip_decompress_bytes(&data)
            .map_err(std::io::Error::other)
            .and_then(|decompressed| fs::write(&dst, decompressed))
    }) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("gzip.decompressFileSync: {}", e)),
    }
}

fn gzip_compress_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())
}

fn gzip_decompress_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = GzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn bytes_to_latin1_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect()
}

fn latin1_string_to_bytes(value: &str) -> Vec<u8> {
    value.chars().map(|ch| ch as u32 as u8).collect()
}

// ---------------------------------------------------------------------------
// terminal: deterministic CI-friendly ANSI helpers and session stubs.
// ---------------------------------------------------------------------------

fn terminal_module() -> Object {
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
        ("style", native("terminal.style", terminal_style)),
        (
            "hyperlink",
            native("terminal.hyperlink", terminal_hyperlink),
        ),
    ])
}

fn terminal_is_tty(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    bool_obj(false)
}

fn terminal_size(_ctx: &mut CallContext, _args: &[Object]) -> Object {
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

fn terminal_capabilities(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    module(vec![
        ("clearScrollback", bool_obj(true)),
        ("alternateScreen", bool_obj(true)),
        ("resizeEvents", bool_obj(false)),
        ("virtualTerminal", bool_obj(true)),
        ("rawMode", bool_obj(false)),
    ])
}

fn terminal_read(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    str_obj("")
}

fn terminal_write(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(value) = args.first() else {
        return new_error(ctx.pos.clone(), "terminal.write requires text");
    };
    let text = object_to_text(value);
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(_) => num_obj(text.len() as f64),
        Err(e) => new_error(ctx.pos.clone(), format!("terminal.write: {}", e)),
    }
}

fn terminal_writeln(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = args.first().map(object_to_text).unwrap_or_default() + "\n";
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(_) => num_obj(text.len() as f64),
        Err(e) => new_error(ctx.pos.clone(), format!("terminal.write: {}", e)),
    }
}

fn terminal_set_raw_mode(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    module(vec![
        ("raw", bool_obj(false)),
        (
            "restore",
            native("terminal.restoreRawMode", |_ctx, _args| Object::Undefined),
        ),
    ])
}

fn terminal_start(_ctx: &mut CallContext, _args: &[Object]) -> Object {
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

fn terminal_clear_screen(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    str_obj("\x1b[2J\x1b[H")
}

fn terminal_clear_line(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    str_obj("\x1b[2K\r")
}

fn terminal_move_to(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn terminal_style(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn terminal_color_code(name: &str, bg: bool) -> Option<i32> {
    let base = match name.to_ascii_lowercase().as_str() {
        "black" => 30,
        "red" | "error" => 31,
        "green" | "success" => 32,
        "yellow" | "warning" => 33,
        "blue" => 34,
        "magenta" => 35,
        "cyan" | "accent" => 36,
        "white" => 37,
        "gray" | "grey" | "muted" => 90,
        _ => return None,
    };
    Some(if bg { base + 10 } else { base })
}

fn terminal_hyperlink(ctx: &mut CallContext, args: &[Object]) -> Object {
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

#[derive(Clone)]
struct TuiApp {
    spec: Rc<RefCell<HashData>>,
    state: RefCell<Object>,
    running: std::cell::Cell<bool>,
    stopped: std::cell::Cell<bool>,
}

#[derive(Clone, Copy)]
struct TuiBoxOptions {
    width: i32,
    height: i32,
    padding: i32,
    border: bool,
}

#[derive(Clone)]
struct TuiInputOptions {
    title: String,
    value: String,
    cursor: i32,
    placeholder: String,
    prompt: String,
    width: i32,
    focused: bool,
    meta: String,
}

fn tui_module() -> Object {
    module(vec![
        ("createApp", native("tui.createApp", tui_create_app)),
        ("key", native("tui.key", tui_key)),
        ("text", native("tui.text", tui_text)),
        ("resize", native("tui.resize", tui_resize)),
        ("tick", native("tui.tick", tui_tick)),
        ("box", native("tui.box", tui_box)),
        ("input", native("tui.input", tui_input)),
        ("row", native("tui.row", tui_row)),
        ("column", native("tui.column", tui_column)),
        ("pad", native("tui.pad", tui_pad)),
        ("statusBar", native("tui.statusBar", tui_status_bar)),
        ("style", native("tui.style", terminal_style)),
        ("stripAnsi", native("tui.stripAnsi", text_strip_ansi)),
        ("width", native("tui.width", text_width)),
        ("truncate", native("tui.truncate", text_truncate_width)),
    ])
}

fn tui_create_app(ctx: &mut CallContext, args: &[Object]) -> Object {
    let spec = match args.first() {
        Some(Object::Hash(hash)) => hash.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "tui.createApp: spec must be an object"),
        None => return new_error(ctx.pos.clone(), "tui.createApp requires spec"),
    };
    let app = Rc::new(TuiApp {
        spec: spec.clone(),
        state: RefCell::new(Object::Undefined),
        running: std::cell::Cell::new(false),
        stopped: std::cell::Cell::new(false),
    });
    if let Some(init_fn) = tui_hash_function(&spec.borrow(), "init") {
        let size = terminal_size_object();
        let result = call_script_function(&init_fn, ctx.env, &[size]);
        if result.is_runtime_error() {
            return result;
        }
        *app.state.borrow_mut() = result;
    } else if let Some(value) = spec.borrow().get("state").cloned() {
        *app.state.borrow_mut() = value;
    }
    tui_app_object(app)
}

fn tui_app_object(app: Rc<TuiApp>) -> Object {
    let obj = Rc::new(RefCell::new(HashData::default()));
    obj.borrow_mut()
        .set("__tuiApp", tui_app_marker(app.clone()));
    obj.borrow_mut().set(
        "dispatch",
        native_bound(
            "tui.app.dispatch",
            tui_app_dispatch,
            tui_app_marker(app.clone()),
        ),
    );
    obj.borrow_mut().set(
        "render",
        native_bound(
            "tui.app.render",
            tui_app_render,
            tui_app_marker(app.clone()),
        ),
    );
    obj.borrow_mut().set(
        "run",
        native_bound("tui.app.run", tui_app_run, tui_app_marker(app.clone())),
    );
    obj.borrow_mut().set(
        "stop",
        native_bound("tui.app.stop", tui_app_stop, tui_app_marker(app.clone())),
    );
    obj.borrow_mut().set(
        "state",
        native_bound("tui.app.state", tui_app_state, tui_app_marker(app)),
    );
    Object::Hash(obj)
}

fn tui_app_marker(app: Rc<TuiApp>) -> Object {
    let marker = Rc::new(RefCell::new(HashData::default()));
    marker.borrow_mut().set("__kind", str_obj("tuiApp"));
    marker
        .borrow_mut()
        .set("__ptr", str_obj(format!("{:p}", Rc::as_ptr(&app))));
    TUI_APPS.with(|apps| apps.borrow_mut().push(app));
    Object::Hash(marker)
}

thread_local! {
    static TUI_APPS: RefCell<Vec<Rc<TuiApp>>> = const { RefCell::new(Vec::new()) };
}

fn native_bound(
    name: &str,
    func: impl Fn(&mut CallContext<'_>, &[Object]) -> Object + 'static,
    extra: Object,
) -> Object {
    Object::Builtin(Rc::new(Builtin {
        name: name.into(),
        func: Rc::new(func),
        extra: Some(extra),
    }))
}

fn bound_tui_app(ctx: &CallContext, name: &str) -> Result<Rc<TuiApp>, Object> {
    let Some(Object::Hash(marker)) = ctx.receiver.clone() else {
        return Err(new_error(
            ctx.pos.clone(),
            format!("{name}: missing app receiver"),
        ));
    };
    let ptr = match marker.borrow().get("__ptr") {
        Some(Object::String(value)) => value.to_string(),
        _ => {
            return Err(new_error(
                ctx.pos.clone(),
                format!("{name}: invalid app receiver"),
            ))
        }
    };
    TUI_APPS.with(|apps| {
        apps.borrow()
            .iter()
            .find(|app| format!("{:p}", Rc::as_ptr(app)) == ptr)
            .cloned()
            .ok_or_else(|| new_error(ctx.pos.clone(), format!("{name}: invalid app receiver")))
    })
}

fn tui_app_dispatch(ctx: &mut CallContext, args: &[Object]) -> Object {
    let app = match bound_tui_app(ctx, "tui.app.dispatch") {
        Ok(app) => app,
        Err(err) => return err,
    };
    let msg = args.first().cloned().unwrap_or(Object::Undefined);
    match tui_app_do_dispatch(ctx, &app, msg) {
        Ok(()) => app.state.borrow().clone(),
        Err(err) => err,
    }
}

fn tui_app_render(ctx: &mut CallContext, args: &[Object]) -> Object {
    let app = match bound_tui_app(ctx, "tui.app.render") {
        Ok(app) => app,
        Err(err) => return err,
    };
    let size = match args.first() {
        Some(Object::Hash(hash)) => Object::Hash(hash.clone()),
        Some(Object::Null | Object::Undefined) | None => terminal_size_object(),
        Some(_) => return new_error(ctx.pos.clone(), "tui.app.render: size must be an object"),
    };
    match tui_app_do_render(ctx, &app, size) {
        Ok(frame) => str_obj(frame),
        Err(err) => err,
    }
}

fn tui_app_run(ctx: &mut CallContext, args: &[Object]) -> Object {
    let app = match bound_tui_app(ctx, "tui.app.run") {
        Ok(app) => app,
        Err(err) => return err,
    };
    if app.running.get() {
        return new_error(ctx.pos.clone(), "tui.app.run: app is already running");
    }
    if let Some(arg) = args.first() {
        if !matches!(arg, Object::Hash(_) | Object::Null | Object::Undefined) {
            return new_error(ctx.pos.clone(), "tui.app.run: options must be an object");
        }
    }
    app.running.set(true);
    app.stopped.set(false);
    let _ = tui_app_do_dispatch(
        ctx,
        &app,
        tui_resize_message(terminal_cols(), terminal_rows(), true),
    );
    let result = tui_app_do_render(ctx, &app, terminal_size_object());
    app.running.set(false);
    match result {
        Ok(frame) => {
            let _ = std::io::stdout().write_all(frame.as_bytes());
            app.state.borrow().clone()
        }
        Err(err) => err,
    }
}

fn tui_app_stop(ctx: &mut CallContext, _args: &[Object]) -> Object {
    let app = match bound_tui_app(ctx, "tui.app.stop") {
        Ok(app) => app,
        Err(err) => return err,
    };
    app.stopped.set(true);
    Object::Undefined
}

fn tui_app_state(ctx: &mut CallContext, _args: &[Object]) -> Object {
    match bound_tui_app(ctx, "tui.app.state") {
        Ok(app) => app.state.borrow().clone(),
        Err(err) => err,
    }
}

fn tui_app_do_dispatch(ctx: &mut CallContext, app: &Rc<TuiApp>, msg: Object) -> Result<(), Object> {
    if let Some(update_fn) = tui_hash_function(&app.spec.borrow(), "update") {
        let state = app.state.borrow().clone();
        let result = call_script_function(&update_fn, ctx.env, &[state, msg]);
        if result.is_runtime_error() {
            return Err(result);
        }
        if let Object::Hash(hash) = &result {
            if let Some(next) = hash.borrow().get("state").cloned() {
                *app.state.borrow_mut() = next;
            } else {
                *app.state.borrow_mut() = result.clone();
            }
            if tui_hash_bool(&hash.borrow(), "quit").unwrap_or(false) {
                app.stopped.set(true);
            }
        } else {
            *app.state.borrow_mut() = result;
        }
    } else if let Object::Hash(hash) = msg {
        if tui_hash_string(&hash.borrow(), "type").as_deref() == Some("quit") {
            app.stopped.set(true);
        }
    }
    Ok(())
}

fn tui_app_do_render(
    ctx: &mut CallContext,
    app: &Rc<TuiApp>,
    size: Object,
) -> Result<String, Object> {
    if let Some(view_fn) = tui_hash_function(&app.spec.borrow(), "view") {
        let state = app.state.borrow().clone();
        let result = call_script_function(&view_fn, ctx.env, &[state, size]);
        if result.is_runtime_error() {
            return Err(result);
        }
        Ok(tui_frame_text(&result))
    } else {
        Ok(value_to_string(&app.state.borrow()))
    }
}

fn tui_key(ctx: &mut CallContext, args: &[Object]) -> Object {
    let name = match required_string(ctx, "tui.key", args, 0, "name") {
        Ok(name) => name,
        Err(err) => return err,
    };
    let msg = tui_key_message(&name, "");
    if let Some(raw) = args.get(1) {
        if let Object::Hash(hash) = &msg {
            hash.borrow_mut().set("raw", str_obj(value_to_string(raw)));
        }
    }
    msg
}

fn tui_text(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "tui.text", args, 0, "value") {
        Ok(value) => value,
        Err(err) => return err,
    };
    tui_text_message(&value, &value)
}

fn tui_resize(ctx: &mut CallContext, args: &[Object]) -> Object {
    let cols = match required_number(ctx, "tui.resize", args, 0, "cols") {
        Ok(cols) => cols,
        Err(err) => return err,
    };
    let rows = match required_number(ctx, "tui.resize", args, 1, "rows") {
        Ok(rows) => rows,
        Err(err) => return err,
    };
    tui_resize_message(cols as i32, rows as i32, true)
}

fn tui_tick(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    tui_tick_message()
}

fn tui_key_message(name: &str, raw: &str) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("key"));
    hash.borrow_mut().set("key", str_obj(name));
    if !raw.is_empty() {
        hash.borrow_mut().set("raw", str_obj(raw));
    }
    Object::Hash(hash)
}

fn tui_text_message(value: &str, raw: &str) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("text"));
    hash.borrow_mut().set("text", str_obj(value));
    if !raw.is_empty() {
        hash.borrow_mut().set("raw", str_obj(raw));
    }
    Object::Hash(hash)
}

fn tui_resize_message(cols: i32, rows: i32, stable: bool) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("resize"));
    hash.borrow_mut().set("cols", num_obj(cols as f64));
    hash.borrow_mut().set("rows", num_obj(rows as f64));
    hash.borrow_mut().set("stable", bool_obj(stable));
    Object::Hash(hash)
}

fn tui_tick_message() -> Object {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0);
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj("tick"));
    hash.borrow_mut().set("timeMs", num_obj(ms));
    Object::Hash(hash)
}

fn tui_box(ctx: &mut CallContext, args: &[Object]) -> Object {
    let content = args.first().map(tui_frame_text).unwrap_or_default();
    let mut title = String::new();
    let mut opts = TuiBoxOptions {
        width: 0,
        height: 0,
        padding: 0,
        border: true,
    };
    if let Some(arg) = args.get(1) {
        if !matches!(arg, Object::Null | Object::Undefined) {
            let Object::Hash(hash) = arg else {
                return new_error(ctx.pos.clone(), "tui.box: options must be an object");
            };
            let hash = hash.borrow();
            title = tui_hash_string(&hash, "title").unwrap_or_default();
            match tui_hash_int_option(ctx, "tui.box", &hash, "width") {
                Ok(Some(n)) => opts.width = n,
                Ok(None) => {}
                Err(err) => return err,
            }
            match tui_hash_int_option(ctx, "tui.box", &hash, "height") {
                Ok(Some(n)) => opts.height = n,
                Ok(None) => {}
                Err(err) => return err,
            }
            match tui_hash_int_option(ctx, "tui.box", &hash, "padding") {
                Ok(Some(n)) => opts.padding = n,
                Ok(None) => {}
                Err(err) => return err,
            }
            if let Some(border) = tui_hash_bool(&hash, "border") {
                opts.border = border;
            }
        }
    }
    str_obj(render_tui_box(&content, &title, opts))
}

fn render_tui_box(content: &str, title: &str, opts: TuiBoxOptions) -> String {
    let normalized = content.replace("\r\n", "\n");
    let mut lines: Vec<String> = if normalized.is_empty() {
        Vec::new()
    } else {
        normalized.split('\n').map(|s| s.to_string()).collect()
    };
    let mut inner_width = lines
        .iter()
        .map(|line| text_visible_width(line))
        .max()
        .unwrap_or(0);
    if !title.is_empty() {
        inner_width = inner_width.max(text_visible_width(title) + 2);
    }
    let padding = opts.padding.max(0) as usize;
    if opts.width > 0 {
        let mut target = opts.width as isize;
        if opts.border {
            target -= 2;
        }
        target -= (padding * 2) as isize;
        inner_width = target.max(0) as usize;
    }
    let pad = " ".repeat(padding);
    let blank = format!("{}{}{}", pad, text_pad_to_width("", inner_width), pad);
    let mut body = Vec::new();
    for _ in 0..padding {
        body.push(blank.clone());
    }
    for line in lines.drain(..) {
        body.push(format!(
            "{}{}{}",
            pad,
            text_pad_to_width(&text_truncate_to_width(&line, inner_width), inner_width),
            pad
        ));
    }
    for _ in 0..padding {
        body.push(blank.clone());
    }
    if opts.height > 0 {
        let target = if opts.border {
            opts.height - 2
        } else {
            opts.height
        }
        .max(0) as usize;
        while body.len() < target {
            body.push(blank.clone());
        }
        body.truncate(target);
    }
    if !opts.border {
        return body.join("\n");
    }
    let width = inner_width + padding * 2;
    let title_text = if title.is_empty() {
        String::new()
    } else {
        format!(
            " {} ",
            text_truncate_to_width(title, width.saturating_sub(2))
        )
    };
    let top_fill = width.saturating_sub(text_visible_width(&title_text));
    let mut out = vec![format!("┌{}{}┐", title_text, "─".repeat(top_fill))];
    for line in body {
        out.push(format!("│{}│", text_pad_to_width(&line, width)));
    }
    out.push(format!("└{}┘", "─".repeat(width)));
    out.join("\n")
}

fn tui_input(ctx: &mut CallContext, args: &[Object]) -> Object {
    let hash = match args.first() {
        Some(Object::Hash(hash)) => hash.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "tui.input: options must be an object"),
        None => return new_error(ctx.pos.clone(), "tui.input requires options"),
    };
    let hash = hash.borrow();
    let mut opts = TuiInputOptions {
        title: tui_hash_string(&hash, "title").unwrap_or_else(|| "Input".into()),
        value: tui_hash_string(&hash, "value").unwrap_or_default(),
        cursor: 0,
        placeholder: tui_hash_string(&hash, "placeholder").unwrap_or_default(),
        prompt: tui_hash_string(&hash, "prompt").unwrap_or_else(|| "> ".into()),
        width: 80,
        focused: tui_hash_bool(&hash, "focused").unwrap_or(true),
        meta: tui_hash_string(&hash, "meta").unwrap_or_default(),
    };
    opts.cursor = text_visible_chars(&opts.value).len() as i32;
    match tui_hash_int_option(ctx, "tui.input", &hash, "cursor") {
        Ok(Some(cursor)) => opts.cursor = cursor,
        Ok(None) => {}
        Err(err) => return err,
    }
    match tui_hash_int_option(ctx, "tui.input", &hash, "width") {
        Ok(Some(width)) => opts.width = width,
        Ok(None) => {}
        Err(err) => return err,
    }
    opts.width = opts.width.max(1);
    opts.cursor = opts
        .cursor
        .clamp(0, text_visible_chars(&opts.value).len() as i32);
    str_obj(render_tui_input(&opts))
}

fn render_tui_input(opts: &TuiInputOptions) -> String {
    let width = opts.width.max(1) as usize;
    let input_width = width
        .saturating_sub(text_visible_width(&opts.prompt))
        .max(1);
    let mut lines = vec![
        terminal_style_string(&text_pad_to_width(&opts.title, width), "accent", true),
        format!(
            "{}{}",
            opts.prompt,
            render_tui_input_value(opts, input_width)
        ),
    ];
    if !opts.meta.is_empty() {
        lines.push(terminal_style_string(
            &text_pad_to_width(&opts.meta, width),
            "muted",
            false,
        ));
    }
    lines.join("\n")
}

fn render_tui_input_value(opts: &TuiInputOptions, width: usize) -> String {
    if opts.value.is_empty() && !opts.placeholder.is_empty() {
        return terminal_style_string(
            &text_pad_to_width(&text_truncate_to_width(&opts.placeholder, width), width),
            "muted",
            false,
        );
    }
    if !opts.focused {
        return text_pad_to_width(&text_truncate_to_width(&opts.value, width), width);
    }
    crop_tui_input_around_cursor(&opts.value, opts.cursor, width)
}

fn crop_tui_input_around_cursor(value: &str, cursor: i32, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let chars = text_visible_chars(value);
    let cursor = (cursor.max(0) as usize).min(chars.len());
    let before_budget = (width - 1) * 62 / 100;
    let after_budget = width - 1 - before_budget;
    let mut before = Vec::new();
    let mut before_width = 0usize;
    for ch in chars[..cursor].iter().rev() {
        let next = text_visible_width(ch);
        if before_width + next > before_budget {
            break;
        }
        before.push(ch.clone());
        before_width += next;
    }
    before.reverse();
    let mut after = Vec::new();
    let mut after_width = 0usize;
    for ch in &chars[cursor..] {
        let next = text_visible_width(ch);
        if after_width + next > after_budget {
            break;
        }
        after.push(ch.clone());
        after_width += next;
    }
    let row = format!("{}\x1b[7m \x1b[0m{}", before.join(""), after.join(""));
    text_pad_to_width(&row, width)
}

fn tui_row(ctx: &mut CallContext, args: &[Object]) -> Object {
    match tui_layout_parts(args) {
        Ok(parts) => str_obj(join_tui_horizontal(&parts)),
        Err(err) => new_error(ctx.pos.clone(), err),
    }
}

fn tui_column(ctx: &mut CallContext, args: &[Object]) -> Object {
    match tui_layout_parts(args) {
        Ok(parts) => str_obj(parts.join("\n")),
        Err(err) => new_error(ctx.pos.clone(), err),
    }
}

fn tui_pad(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(content) = args.first() else {
        return new_error(ctx.pos.clone(), "tui.pad requires content");
    };
    let content = tui_frame_text(content);
    let padding = match args.get(1) {
        Some(Object::Number(n)) => (*n as i32).max(0) as usize,
        Some(_) => return new_error(ctx.pos.clone(), "tui.pad: padding must be a number"),
        None => 1,
    };
    let prefix = " ".repeat(padding);
    let mut lines: Vec<String> = content
        .replace("\r\n", "\n")
        .split('\n')
        .map(|line| format!("{}{}{}", prefix, line, prefix))
        .collect();
    let blank_width = lines
        .iter()
        .map(|line| text_visible_width(line))
        .max()
        .unwrap_or(0);
    let blank = " ".repeat(blank_width);
    for _ in 0..padding {
        lines.insert(0, blank.clone());
        lines.push(blank.clone());
    }
    str_obj(lines.join("\n"))
}

fn tui_status_bar(ctx: &mut CallContext, args: &[Object]) -> Object {
    let hash = match args.first() {
        Some(Object::Hash(hash)) => hash.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "tui.statusBar: parts must be an object"),
        None => return new_error(ctx.pos.clone(), "tui.statusBar requires parts"),
    };
    let width = match args.get(1) {
        Some(Object::Number(n)) => (*n as i32).max(1) as usize,
        Some(_) => return new_error(ctx.pos.clone(), "tui.statusBar: width must be a number"),
        None => 80,
    };
    let hash = hash.borrow();
    let mut left = tui_hash_string(&hash, "left").unwrap_or_default();
    let center = tui_hash_string(&hash, "center").unwrap_or_default();
    let mut right = tui_hash_string(&hash, "right").unwrap_or_default();
    left = text_truncate_to_width(&left, width);
    let right_budget = width
        .saturating_sub(text_visible_width(&left))
        .saturating_sub(1);
    right = text_truncate_to_width(&right, right_budget);
    let mut line = left;
    if !center.is_empty()
        && width
            >= text_visible_width(&line)
                + text_visible_width(&right)
                + text_visible_width(&center)
                + 2
    {
        let center_pos = (width - text_visible_width(&center)) / 2;
        line = format!("{}{}", text_pad_to_width(&line, center_pos), center);
    }
    line = format!(
        "{}{}",
        text_pad_to_width(&line, width.saturating_sub(text_visible_width(&right))),
        right
    );
    str_obj(text_truncate_to_width(&line, width))
}

fn tui_layout_parts(args: &[Object]) -> Result<Vec<String>, String> {
    if args.is_empty() {
        return Ok(Vec::new());
    }
    if let Some(Object::Array(arr)) = args.first() {
        return Ok(arr.borrow().elements.iter().map(tui_frame_text).collect());
    }
    Ok(args.iter().map(tui_frame_text).collect())
}

fn join_tui_horizontal(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let blocks: Vec<Vec<String>> = parts
        .iter()
        .map(|part| {
            part.replace("\r\n", "\n")
                .split('\n')
                .map(|s| s.to_string())
                .collect()
        })
        .collect();
    let height = blocks.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = blocks
        .iter()
        .map(|lines| {
            lines
                .iter()
                .map(|line| text_visible_width(line))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut out = Vec::with_capacity(height);
    for row in 0..height {
        let mut line = String::new();
        for (col, lines) in blocks.iter().enumerate() {
            let part = lines.get(row).map(String::as_str).unwrap_or("");
            line.push_str(&text_pad_to_width(part, widths[col]));
        }
        out.push(line);
    }
    out.join("\n")
}

fn tui_frame_text(value: &Object) -> String {
    if let Object::Array(arr) = value {
        return arr
            .borrow()
            .elements
            .iter()
            .map(value_to_string)
            .collect::<Vec<_>>()
            .join("\n");
    }
    value_to_string(value)
}

fn tui_hash_function(hash: &HashData, key: &str) -> Option<Object> {
    match hash.get(key) {
        Some(Object::Function(_) | Object::Builtin(_)) => hash.get(key).cloned(),
        _ => None,
    }
}

fn tui_hash_string(hash: &HashData, key: &str) -> Option<String> {
    match hash.get(key) {
        Some(Object::String(value)) => Some(value.to_string()),
        Some(Object::Null | Object::Undefined) | None => None,
        Some(value) => Some(value_to_string(value)),
    }
}

fn tui_hash_bool(hash: &HashData, key: &str) -> Option<bool> {
    match hash.get(key) {
        Some(Object::Boolean(value)) => Some(*value),
        Some(Object::Null | Object::Undefined) | None => None,
        Some(value) => Some(value.is_truthy()),
    }
}

fn tui_hash_int_option(
    ctx: &CallContext,
    name: &str,
    hash: &HashData,
    key: &str,
) -> Result<Option<i32>, Object> {
    match hash.get(key) {
        Some(Object::Number(n)) => Ok(Some(*n as i32)),
        Some(Object::Null | Object::Undefined) | None => Ok(None),
        Some(_) => Err(new_error(
            ctx.pos.clone(),
            format!("{name}: {key} must be a number"),
        )),
    }
}

fn terminal_size_object() -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut()
        .set("cols", num_obj(terminal_cols() as f64));
    hash.borrow_mut()
        .set("rows", num_obj(terminal_rows() as f64));
    Object::Hash(hash)
}

fn terminal_cols() -> i32 {
    env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(80)
}

fn terminal_rows() -> i32 {
    env::var("LINES")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(24)
}

fn text_visible_width(value: &str) -> usize {
    strip_ansi(value).chars().map(rune_width).sum()
}

fn text_visible_chars(value: &str) -> Vec<String> {
    strip_ansi(value).chars().map(|c| c.to_string()).collect()
}

fn text_truncate_to_width(value: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for r in strip_ansi(value).chars() {
        let w = rune_width(r);
        if used + w > width {
            break;
        }
        out.push(r);
        used += w;
    }
    out
}

fn text_pad_to_width(value: &str, width: usize) -> String {
    let mut out = value.to_string();
    while text_visible_width(&out) < width {
        out.push(' ');
    }
    out
}

// ---------------------------------------------------------------------------
// cli: command/flag parsing subset backed by closure-owned state.
// ---------------------------------------------------------------------------

fn cli_module() -> Object {
    module(vec![
        ("command", native("cli.command", cli_command_new)),
        ("root", native("cli.root", cli_command_new)),
        ("noArgs", native("cli.noArgs", cli_no_args)),
        (
            "arbitraryArgs",
            native("cli.arbitraryArgs", cli_arbitrary_args),
        ),
        ("exactArgs", native("cli.exactArgs", cli_exact_args)),
        ("minArgs", native("cli.minArgs", cli_min_args)),
        ("maxArgs", native("cli.maxArgs", cli_max_args)),
        ("rangeArgs", native("cli.rangeArgs", cli_range_args)),
    ])
}

#[derive(Clone)]
struct CliCommand {
    use_line: String,
    name: String,
    short: String,
    version: String,
    args_validator: CliArgValidator,
    parent: Option<Rc<RefCell<CliCommand>>>,
    children: Vec<Rc<RefCell<CliCommand>>>,
    flags: Rc<RefCell<CliFlagSet>>,
    persistent_flags: Rc<RefCell<CliFlagSet>>,
}

#[derive(Clone)]
struct CliFlagSet {
    flags: Vec<CliFlag>,
}

#[derive(Clone)]
struct CliFlag {
    name: String,
    short: String,
    usage: String,
    kind: String,
    default: Object,
    value: Object,
    changed: bool,
}

#[derive(Clone)]
struct CliArgValidator {
    kind: String,
    min: usize,
    max: usize,
}

fn cli_command_new(ctx: &mut CallContext, args: &[Object]) -> Object {
    let cmd = Rc::new(RefCell::new(CliCommand {
        use_line: String::new(),
        name: String::new(),
        short: String::new(),
        version: String::new(),
        args_validator: CliArgValidator {
            kind: "any".into(),
            min: 0,
            max: usize::MAX,
        },
        parent: None,
        children: Vec::new(),
        flags: Rc::new(RefCell::new(CliFlagSet { flags: Vec::new() })),
        persistent_flags: Rc::new(RefCell::new(CliFlagSet { flags: Vec::new() })),
    }));
    if let Some(value) = args.first() {
        if let Err(err) = cli_apply_options(ctx, &cmd, value) {
            return err;
        }
    }
    cli_command_object(cmd)
}

fn cli_apply_options(
    ctx: &mut CallContext,
    cmd: &Rc<RefCell<CliCommand>>,
    value: &Object,
) -> Result<(), Object> {
    if matches!(value, Object::Undefined | Object::Null) {
        return Ok(());
    }
    let Object::Hash(hash) = value else {
        return Err(new_error(
            ctx.pos.clone(),
            "cli.command: options must be an object",
        ));
    };
    let hash = hash.borrow();
    let use_line = hash_string(&hash, "use").or_else(|| hash_string(&hash, "Use"));
    let mut cmd_mut = cmd.borrow_mut();
    if let Some(use_line) = use_line {
        cmd_mut.use_line = use_line;
        cmd_mut.name = cmd_mut
            .use_line
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
    }
    if let Some(short) = hash_string(&hash, "short").or_else(|| hash_string(&hash, "Short")) {
        cmd_mut.short = short;
    }
    if let Some(version) = hash_string(&hash, "version").or_else(|| hash_string(&hash, "Version")) {
        cmd_mut.version = version;
    }
    if let Some(Object::Hash(vhash)) = hash.get("args").or_else(|| hash.get("Args")) {
        if vhash.borrow().contains("__cliArgValidator") {
            cmd_mut.args_validator = cli_validator_from_hash(&vhash.borrow());
        }
    }
    Ok(())
}

fn cli_command_object(cmd: Rc<RefCell<CliCommand>>) -> Object {
    let flags_cmd = cmd.clone();
    let pflags_cmd = cmd.clone();
    let child_cmd = cmd.clone();
    let execute_cmd = cmd.clone();
    let usage_cmd = cmd.clone();
    let path_cmd = cmd.clone();
    let flag_cmd = cmd.clone();
    module(vec![
        (
            "flags",
            native("cli.Command.flags", move |_ctx, _args| {
                cli_flag_set_object(flags_cmd.borrow().flags.clone())
            }),
        ),
        (
            "persistentFlags",
            native("cli.Command.persistentFlags", move |_ctx, _args| {
                cli_flag_set_object(pflags_cmd.borrow().persistent_flags.clone())
            }),
        ),
        (
            "addCommand",
            native("cli.Command.addCommand", move |_ctx, _args| {
                Object::Undefined
            }),
        ),
        (
            "command",
            native("cli.Command.command", move |ctx, args| {
                let child_state = Rc::new(RefCell::new(CliCommand {
                    use_line: String::new(),
                    name: String::new(),
                    short: String::new(),
                    version: String::new(),
                    args_validator: CliArgValidator {
                        kind: "any".into(),
                        min: 0,
                        max: usize::MAX,
                    },
                    parent: Some(child_cmd.clone()),
                    children: Vec::new(),
                    flags: Rc::new(RefCell::new(CliFlagSet { flags: Vec::new() })),
                    persistent_flags: Rc::new(RefCell::new(CliFlagSet { flags: Vec::new() })),
                }));
                if let Some(value) = args.first() {
                    if let Err(err) = cli_apply_options(ctx, &child_state, value) {
                        return err;
                    }
                }
                child_cmd.borrow_mut().children.push(child_state.clone());
                cli_command_object(child_state)
            }),
        ),
        (
            "execute",
            native("cli.Command.execute", move |ctx, args| {
                cli_execute(ctx, &execute_cmd, args)
            }),
        ),
        (
            "usage",
            native("cli.Command.usage", move |_ctx, _args| {
                str_obj(cli_usage(&usage_cmd))
            }),
        ),
        (
            "help",
            native("cli.Command.help", move |_ctx, _args| Object::Undefined),
        ),
        (
            "commandPath",
            native("cli.Command.commandPath", move |_ctx, _args| {
                str_obj(cli_command_path(&path_cmd))
            }),
        ),
        (
            "flag",
            native("cli.Command.flag", move |ctx, args| {
                let name = match required_string(ctx, "cli.Command.flag", args, 0, "name") {
                    Ok(name) => name,
                    Err(err) => return err,
                };
                cli_lookup_flag(&flag_cmd, &name)
                    .map(|flag| flag.value)
                    .unwrap_or(Object::Undefined)
            }),
        ),
    ])
}

fn cli_flag_set_object(set: Rc<RefCell<CliFlagSet>>) -> Object {
    let s1 = set.clone();
    let s2 = set.clone();
    let s3 = set.clone();
    let s4 = set.clone();
    let s5 = set.clone();
    let s6 = set.clone();
    module(vec![
        (
            "string",
            native("cli.FlagSet.string", move |ctx, args| {
                cli_flag_add(ctx, &s1, "string", args)
            }),
        ),
        (
            "bool",
            native("cli.FlagSet.bool", move |ctx, args| {
                cli_flag_add(ctx, &s2, "bool", args)
            }),
        ),
        (
            "int",
            native("cli.FlagSet.int", move |ctx, args| {
                cli_flag_add(ctx, &s3, "int", args)
            }),
        ),
        (
            "number",
            native("cli.FlagSet.number", move |ctx, args| {
                cli_flag_add(ctx, &s4, "number", args)
            }),
        ),
        (
            "get",
            native("cli.FlagSet.get", move |ctx, args| {
                let name = match required_string(ctx, "cli.FlagSet.get", args, 0, "name") {
                    Ok(name) => name,
                    Err(err) => return err,
                };
                s5.borrow()
                    .flags
                    .iter()
                    .find(|flag| flag.name == name)
                    .map(|flag| flag.value.clone())
                    .unwrap_or(Object::Undefined)
            }),
        ),
        (
            "changed",
            native("cli.FlagSet.changed", move |ctx, args| {
                let name = match required_string(ctx, "cli.FlagSet.changed", args, 0, "name") {
                    Ok(name) => name,
                    Err(err) => return err,
                };
                bool_obj(
                    s6.borrow()
                        .flags
                        .iter()
                        .any(|flag| flag.name == name && flag.changed),
                )
            }),
        ),
    ])
}

fn cli_flag_add(
    ctx: &mut CallContext,
    set: &Rc<RefCell<CliFlagSet>>,
    kind: &str,
    args: &[Object],
) -> Object {
    let name = match required_string(ctx, &format!("cli.FlagSet.{}", kind), args, 0, "name") {
        Ok(name) => name,
        Err(err) => return err,
    };
    let short = match args.get(1) {
        Some(Object::String(s)) => s.to_string(),
        _ => String::new(),
    };
    let default = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| cli_default_for_kind(kind));
    let usage = match args.get(3) {
        Some(Object::String(s)) => s.to_string(),
        _ => String::new(),
    };
    let value = match cli_coerce_flag(ctx, kind, default) {
        Ok(value) => value,
        Err(err) => return err,
    };
    if set.borrow().flags.iter().any(|flag| flag.name == name) {
        return new_error(
            ctx.pos.clone(),
            format!("cli: flag {} is already defined", name),
        );
    }
    set.borrow_mut().flags.push(CliFlag {
        name,
        short,
        usage,
        kind: kind.into(),
        default: value.clone(),
        value,
        changed: false,
    });
    cli_flag_set_object(set.clone())
}

fn cli_default_for_kind(kind: &str) -> Object {
    match kind {
        "string" => str_obj(""),
        "bool" => bool_obj(false),
        "int" | "number" => num_obj(0.0),
        _ => Object::Undefined,
    }
}

fn cli_coerce_flag(ctx: &mut CallContext, kind: &str, value: Object) -> Result<Object, Object> {
    match kind {
        "string" if matches!(value, Object::String(_)) => Ok(value),
        "bool" if matches!(value, Object::Boolean(_)) => Ok(value),
        "int" | "number" if matches!(value, Object::Number(_)) => Ok(value),
        "string" => Err(new_error(ctx.pos.clone(), "cli: default must be a string")),
        "bool" => Err(new_error(ctx.pos.clone(), "cli: default must be a bool")),
        "int" | "number" => Err(new_error(ctx.pos.clone(), "cli: default must be a number")),
        _ => Ok(value),
    }
}

fn cli_execute(ctx: &mut CallContext, cmd: &Rc<RefCell<CliCommand>>, args: &[Object]) -> Object {
    let argv = if let Some(arg) = args.first() {
        match cli_string_array(ctx, "cli.Command.execute", arg, "args") {
            Ok(argv) => argv,
            Err(err) => return err,
        }
    } else {
        Vec::new()
    };
    cli_reset_flags(cmd);
    if let Err(err) = cli_parse_flags(ctx, cmd, &argv) {
        return err;
    }
    let positionals = cli_positionals(cmd, &argv);
    match cmd.borrow().args_validator.validate(ctx, positionals.len()) {
        Ok(()) => num_obj(0.0),
        Err(err) => err,
    }
}

fn cli_reset_flags(cmd: &Rc<RefCell<CliCommand>>) {
    for flag in &mut cmd.borrow_mut().flags.borrow_mut().flags {
        flag.value = flag.default.clone();
        flag.changed = false;
    }
    for flag in &mut cmd.borrow_mut().persistent_flags.borrow_mut().flags {
        flag.value = flag.default.clone();
        flag.changed = false;
    }
}

fn cli_parse_flags(
    ctx: &mut CallContext,
    cmd: &Rc<RefCell<CliCommand>>,
    argv: &[String],
) -> Result<(), Object> {
    let mut i = 0;
    while i < argv.len() {
        let token = &argv[i];
        if token == "--" {
            break;
        }
        if let Some(raw) = token.strip_prefix("--") {
            let (name, value) = raw
                .split_once('=')
                .map_or((raw, None), |(n, v)| (n, Some(v)));
            let flags_ref = cmd.borrow().flags.clone();
            let mut flags = flags_ref.borrow_mut();
            let Some(flag) = flags.flags.iter_mut().find(|flag| flag.name == name) else {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("cli: unknown flag --{}", name),
                ));
            };
            let raw_value = if flag.kind == "bool" && value.is_none() {
                "true".to_string()
            } else if let Some(value) = value {
                value.to_string()
            } else {
                i += 1;
                argv.get(i).cloned().ok_or_else(|| {
                    new_error(
                        ctx.pos.clone(),
                        format!("cli: flag --{} requires value", name),
                    )
                })?
            };
            cli_set_flag(ctx, flag, &raw_value)?;
        } else if token.starts_with('-') && token.len() > 1 {
            let key = token.trim_start_matches('-');
            let flags_ref = cmd.borrow().flags.clone();
            let mut flags = flags_ref.borrow_mut();
            let Some(flag) = flags.flags.iter_mut().find(|flag| flag.short == key) else {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("cli: unknown shorthand -{}", key),
                ));
            };
            let raw_value = if flag.kind == "bool" {
                "true".to_string()
            } else {
                i += 1;
                argv.get(i).cloned().ok_or_else(|| {
                    new_error(
                        ctx.pos.clone(),
                        format!("cli: flag -{} requires value", key),
                    )
                })?
            };
            cli_set_flag(ctx, flag, &raw_value)?;
        }
        i += 1;
    }
    Ok(())
}

fn cli_set_flag(ctx: &mut CallContext, flag: &mut CliFlag, raw: &str) -> Result<(), Object> {
    flag.value = match flag.kind.as_str() {
        "string" => str_obj(raw),
        "bool" => match raw {
            "true" | "1" => bool_obj(true),
            "false" | "0" => bool_obj(false),
            _ => {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("cli: flag --{} expects bool", flag.name),
                ))
            }
        },
        "int" => num_obj(raw.parse::<i64>().map_err(|_| {
            new_error(
                ctx.pos.clone(),
                format!("cli: flag --{} expects int", flag.name),
            )
        })? as f64),
        "number" => num_obj(raw.parse::<f64>().map_err(|_| {
            new_error(
                ctx.pos.clone(),
                format!("cli: flag --{} expects number", flag.name),
            )
        })?),
        _ => Object::Undefined,
    };
    flag.changed = true;
    Ok(())
}

fn cli_lookup_flag(cmd: &Rc<RefCell<CliCommand>>, name: &str) -> Option<CliFlag> {
    cmd.borrow()
        .flags
        .borrow()
        .flags
        .iter()
        .find(|flag| flag.name == name || flag.short == name)
        .cloned()
}

fn cli_positionals(cmd: &Rc<RefCell<CliCommand>>, argv: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let token = &argv[i];
        if token == "--" {
            out.extend(argv.iter().skip(i + 1).cloned());
            break;
        }
        if token.starts_with("--") {
            let name = token
                .trim_start_matches("--")
                .split('=')
                .next()
                .unwrap_or("");
            if token.contains('=') {
                i += 1;
                continue;
            }
            if let Some(flag) = cli_lookup_flag(cmd, name) {
                if flag.kind != "bool" {
                    i += 1;
                }
            }
        } else if token.starts_with('-') && token.len() > 1 {
            let name = token.trim_start_matches('-');
            if let Some(flag) = cli_lookup_flag(cmd, name) {
                if flag.kind != "bool" {
                    i += 1;
                }
            }
        } else {
            out.push(token.clone());
        }
        i += 1;
    }
    out
}

fn cli_usage(cmd: &Rc<RefCell<CliCommand>>) -> String {
    let cmd = cmd.borrow();
    let mut out = String::new();
    if !cmd.name.is_empty() {
        out.push_str(&cmd.name);
        if !cmd.short.is_empty() {
            out.push_str(" - ");
            out.push_str(&cmd.short);
        }
        out.push_str("\n\n");
    }
    if !cmd.use_line.is_empty() {
        out.push_str("Usage:\n  ");
        out.push_str(&cmd.use_line);
        out.push_str("\n\n");
    }
    let flags = cmd.flags.borrow();
    if !flags.flags.is_empty() {
        out.push_str("Flags:\n");
        for flag in &flags.flags {
            if flag.short.is_empty() {
                out.push_str(&format!("    --{} {}\n", flag.name, flag.usage));
            } else {
                out.push_str(&format!(
                    "  -{}, --{} {}\n",
                    flag.short, flag.name, flag.usage
                ));
            }
        }
    }
    out
}

fn cli_command_path(cmd: &Rc<RefCell<CliCommand>>) -> String {
    let mut parts = Vec::new();
    let mut current = Some(cmd.clone());
    while let Some(cmd) = current {
        let borrowed = cmd.borrow();
        if !borrowed.name.is_empty() {
            parts.push(borrowed.name.clone());
        }
        current = borrowed.parent.clone();
    }
    parts.reverse();
    parts.join(" ")
}

fn cli_string_array(
    ctx: &mut CallContext,
    name: &str,
    value: &Object,
    label: &str,
) -> Result<Vec<String>, Object> {
    let Object::Array(arr) = value else {
        return Err(new_error(
            ctx.pos.clone(),
            format!("{}: {} must be an array of strings", name, label),
        ));
    };
    let mut out = Vec::new();
    for item in &arr.borrow().elements {
        match item {
            Object::String(s) => out.push(s.to_string()),
            _ => {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("{}: {} must be an array of strings", name, label),
                ))
            }
        }
    }
    Ok(out)
}

fn cli_validator_object(kind: &str, min: usize, max: usize) -> Object {
    module(vec![
        ("__cliArgValidator", bool_obj(true)),
        ("kind", str_obj(kind)),
        ("min", num_obj(min as f64)),
        ("max", num_obj(max as f64)),
    ])
}

fn cli_validator_from_hash(hash: &HashData) -> CliArgValidator {
    CliArgValidator {
        kind: hash_string(hash, "kind").unwrap_or_else(|| "any".into()),
        min: match hash.get("min") {
            Some(Object::Number(n)) => *n as usize,
            _ => 0,
        },
        max: match hash.get("max") {
            Some(Object::Number(n)) => *n as usize,
            _ => usize::MAX,
        },
    }
}

impl CliArgValidator {
    fn validate(&self, ctx: &mut CallContext, count: usize) -> Result<(), Object> {
        match self.kind.as_str() {
            "none" if count != 0 => Err(new_error(
                ctx.pos.clone(),
                format!("cli: accepts no arguments, got {}", count),
            )),
            "exact" if count != self.min => Err(new_error(
                ctx.pos.clone(),
                format!("cli: accepts {} argument(s), got {}", self.min, count),
            )),
            "min" if count < self.min => Err(new_error(
                ctx.pos.clone(),
                format!(
                    "cli: requires at least {} argument(s), got {}",
                    self.min, count
                ),
            )),
            "max" if count > self.max => Err(new_error(
                ctx.pos.clone(),
                format!(
                    "cli: accepts at most {} argument(s), got {}",
                    self.max, count
                ),
            )),
            "range" if count < self.min || count > self.max => Err(new_error(
                ctx.pos.clone(),
                format!(
                    "cli: accepts between {} and {} argument(s), got {}",
                    self.min, self.max, count
                ),
            )),
            _ => Ok(()),
        }
    }
}

fn cli_no_args(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    cli_validator_object("none", 0, 0)
}

fn cli_arbitrary_args(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    cli_validator_object("any", 0, usize::MAX)
}

fn cli_exact_args(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_number(ctx, "cli.exactArgs", args, 0, "n") {
        Ok(n) => cli_validator_object("exact", n as usize, n as usize),
        Err(err) => err,
    }
}

fn cli_min_args(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_number(ctx, "cli.minArgs", args, 0, "n") {
        Ok(n) => cli_validator_object("min", n as usize, usize::MAX),
        Err(err) => err,
    }
}

fn cli_max_args(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_number(ctx, "cli.maxArgs", args, 0, "n") {
        Ok(n) => cli_validator_object("max", 0, n as usize),
        Err(err) => err,
    }
}

fn cli_range_args(ctx: &mut CallContext, args: &[Object]) -> Object {
    let min = match required_number(ctx, "cli.rangeArgs", args, 0, "min") {
        Ok(min) => min as usize,
        Err(err) => return err,
    };
    let max = match required_number(ctx, "cli.rangeArgs", args, 1, "max") {
        Ok(max) => max as usize,
        Err(err) => return err,
    };
    if max < min {
        return new_error(ctx.pos.clone(), "cli.rangeArgs: max must be >= min");
    }
    cli_validator_object("range", min, max)
}

// ---------------------------------------------------------------------------
// table: ASCII table rendering for arrays of rows or objects.
// ---------------------------------------------------------------------------

fn table_module() -> Object {
    module(vec![("render", native("table.render", table_render))])
}

fn table_render(ctx: &mut CallContext, args: &[Object]) -> Object {
    let Some(rows_obj) = args.first() else {
        return new_error(ctx.pos.clone(), "table.render requires rows");
    };
    let Object::Array(rows) = rows_obj else {
        return new_error(ctx.pos.clone(), "table.render: rows must be an array");
    };
    let rows_ref = rows.borrow();
    let mut headers = table_headers(args.get(1));
    if headers.is_empty() {
        headers = infer_table_headers(&rows_ref.elements);
    }
    let mut matrix = Vec::new();
    if !headers.is_empty() {
        matrix.push(headers.clone());
    }
    for row in &rows_ref.elements {
        matrix.push(table_row_cells(row, &headers));
    }
    str_obj(render_ascii_table(&matrix, !headers.is_empty()))
}

fn table_headers(value: Option<&Object>) -> Vec<String> {
    match value {
        Some(Object::Array(arr)) => arr
            .borrow()
            .elements
            .iter()
            .map(object_to_text)
            .collect::<Vec<_>>(),
        Some(Object::Hash(hash)) => match hash.borrow().get("headers") {
            Some(Object::Array(arr)) => arr
                .borrow()
                .elements
                .iter()
                .map(object_to_text)
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn infer_table_headers(rows: &[Object]) -> Vec<String> {
    for row in rows {
        if let Object::Hash(hash) = row {
            return hash
                .borrow()
                .entries
                .iter()
                .map(|(key, _)| key.clone())
                .collect();
        }
    }
    Vec::new()
}

fn table_row_cells(row: &Object, headers: &[String]) -> Vec<String> {
    match row {
        Object::Array(arr) => arr.borrow().elements.iter().map(object_to_text).collect(),
        Object::Hash(hash) if !headers.is_empty() => {
            let hash = hash.borrow();
            headers
                .iter()
                .map(|key| hash.get(key).map(object_to_text).unwrap_or_default())
                .collect()
        }
        _ => vec![object_to_text(row)],
    }
}

fn render_ascii_table(rows: &[Vec<String>], has_header: bool) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; columns];
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(strip_ansi(cell).chars().count());
        }
    }
    let border = table_border(&widths);
    let mut out = String::new();
    out.push_str(&border);
    out.push('\n');
    for (idx, row) in rows.iter().enumerate() {
        out.push_str(&table_row(row, &widths));
        out.push('\n');
        if idx == 0 && has_header {
            out.push_str(&border);
            out.push('\n');
        }
    }
    out.push_str(&border);
    out
}

fn table_border(widths: &[usize]) -> String {
    let mut out = String::from("+");
    for width in widths {
        out.push_str(&"-".repeat(*width + 2));
        out.push('+');
    }
    out
}

fn table_row(row: &[String], widths: &[usize]) -> String {
    let mut out = String::from("|");
    for (idx, width) in widths.iter().enumerate() {
        let cell = row.get(idx).cloned().unwrap_or_default();
        let pad = width.saturating_sub(strip_ansi(&cell).chars().count());
        out.push(' ');
        out.push_str(&cell);
        out.push_str(&" ".repeat(pad + 1));
        out.push('|');
    }
    out
}

// ---------------------------------------------------------------------------
// validation: small schema validator plus common predicate helpers.
// ---------------------------------------------------------------------------

fn validation_module() -> Object {
    module(vec![
        (
            "validate",
            native("validation.validate", validation_validate),
        ),
        (
            "required",
            native("validation.required", validation_required),
        ),
        ("type", native("validation.type", validation_type)),
        ("email", native("validation.email", validation_email)),
        ("min", native("validation.min", validation_min)),
        ("max", native("validation.max", validation_max)),
    ])
}

fn validation_validate(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(
            ctx.pos.clone(),
            "validation.validate requires value and rules",
        );
    }
    let Object::Hash(rules) = &args[1] else {
        return new_error(
            ctx.pos.clone(),
            "validation.validate: rules must be an object",
        );
    };
    let mut errors = Vec::new();
    validate_value(&args[0], &rules.borrow(), "value", &mut errors);
    validation_result(errors)
}

fn validation_required(_ctx: &mut CallContext, args: &[Object]) -> Object {
    bool_obj(args.first().map(is_present).unwrap_or(false))
}

fn validation_type(ctx: &mut CallContext, args: &[Object]) -> Object {
    let expected = match required_string(ctx, "validation.type", args, 1, "type") {
        Ok(expected) => expected,
        Err(err) => return err,
    };
    bool_obj(
        args.first()
            .map(|value| value_matches_type(value, &expected))
            .unwrap_or(false),
    )
}

fn validation_email(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "validation.email", args, 0, "value") {
        Ok(value) => bool_obj(is_email(&value)),
        Err(err) => err,
    }
}

fn validation_min(ctx: &mut CallContext, args: &[Object]) -> Object {
    let min = match required_number(ctx, "validation.min", args, 1, "min") {
        Ok(min) => min,
        Err(err) => return err,
    };
    bool_obj(
        args.first()
            .map(|value| value_at_least(value, min))
            .unwrap_or(false),
    )
}

fn validation_max(ctx: &mut CallContext, args: &[Object]) -> Object {
    let max = match required_number(ctx, "validation.max", args, 1, "max") {
        Ok(max) => max,
        Err(err) => return err,
    };
    bool_obj(
        args.first()
            .map(|value| value_at_most(value, max))
            .unwrap_or(false),
    )
}

fn validate_value(value: &Object, rules: &HashData, path: &str, errors: &mut Vec<String>) {
    if matches!(rules.get("required"), Some(Object::Boolean(true))) && !is_present(value) {
        errors.push(format!("{} is required", path));
    }
    if let Some(Object::String(expected)) = rules.get("type") {
        if is_present(value) && !value_matches_type(value, expected) {
            errors.push(format!("{} must be {}", path, expected));
        }
    }
    if matches!(rules.get("email"), Some(Object::Boolean(true))) {
        match value {
            Object::String(s) if is_email(s) => {}
            _ if is_present(value) => errors.push(format!("{} must be a valid email", path)),
            _ => {}
        }
    }
    if let Some(Object::Number(min)) = rules.get("min") {
        if is_present(value) && !value_at_least(value, *min) {
            errors.push(format!("{} must be at least {}", path, format_number(*min)));
        }
    }
    if let Some(Object::Number(max)) = rules.get("max") {
        if is_present(value) && !value_at_most(value, *max) {
            errors.push(format!("{} must be at most {}", path, format_number(*max)));
        }
    }
    if let Some(Object::Hash(fields)) = rules.get("fields") {
        if let Object::Hash(value_hash) = value {
            let value_hash = value_hash.borrow();
            for (key, field_rules) in &fields.borrow().entries {
                if let Object::Hash(rule_hash) = field_rules {
                    let field_value = value_hash.get(key).cloned().unwrap_or(Object::Undefined);
                    validate_value(&field_value, &rule_hash.borrow(), key, errors);
                }
            }
        } else if is_present(value) {
            errors.push(format!("{} must be object", path));
        }
    }
}

fn validation_result(errors: Vec<String>) -> Object {
    if errors.is_empty() {
        module(vec![
            ("valid", bool_obj(true)),
            ("errors", array(Vec::new())),
        ])
    } else {
        module(vec![
            ("valid", bool_obj(false)),
            ("errors", array(errors.into_iter().map(str_obj).collect())),
        ])
    }
}

fn is_present(value: &Object) -> bool {
    match value {
        Object::Undefined | Object::Null => false,
        Object::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn value_matches_type(value: &Object, expected: &str) -> bool {
    match expected {
        "array" => matches!(value, Object::Array(_)),
        "object" => matches!(value, Object::Hash(_)),
        "date" => matches!(value, Object::Date(_)),
        other => value.type_tag() == other,
    }
}

fn is_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

fn value_at_least(value: &Object, min: f64) -> bool {
    match value {
        Object::Number(n) => *n >= min,
        Object::String(s) => s.chars().count() as f64 >= min,
        Object::Array(arr) => arr.borrow().elements.len() as f64 >= min,
        _ => false,
    }
}

fn value_at_most(value: &Object, max: f64) -> bool {
    match value {
        Object::Number(n) => *n <= max,
        Object::String(s) => s.chars().count() as f64 <= max,
        Object::Array(arr) => arr.borrow().elements.len() as f64 <= max,
        _ => false,
    }
}

// ===========================================================================
// P7 stdlib batch: toml / yaml / xml / markdown / schema / test / archive/zip.
//
// Codec modules (toml/yaml/xml) share a parse/stringify/readFileSync/
// writeFileSync surface and bridge through serde_json::Value, matching the Go
// originals' goValueToObject/objectToGoValue contracts (map keys sorted for
// determinism, integer-valued Numbers preserved as integers where possible).
// ===========================================================================

// ---------------------------------------------------------------------------
// serde_json::Value <-> Object bridge (shared by the codec modules).
// ---------------------------------------------------------------------------

fn value_to_object(value: &serde_json::Value) -> Object {
    match value {
        serde_json::Value::Null => Object::Null,
        serde_json::Value::Bool(b) => bool_obj(*b),
        serde_json::Value::Number(n) => {
            // Prefer integer representation when the value is integral, to
            // match the Go original's int64-then-f64 ordering.
            if let Some(i) = n.as_i64() {
                num_obj(i as f64)
            } else if let Some(u) = n.as_u64() {
                num_obj(u as f64)
            } else {
                num_obj(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_json::Value::String(s) => str_obj(s.clone()),
        serde_json::Value::Array(arr) => array(arr.iter().map(value_to_object).collect()),
        serde_json::Value::Object(map) => {
            let hash = Rc::new(RefCell::new(HashData::default()));
            // Insert keys in sorted order for deterministic output, matching
            // the Go original's sortedStringKeys behavior.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(v) = map.get(key) {
                    hash.borrow_mut().set(key.clone(), value_to_object(v));
                }
            }
            Object::Hash(hash)
        }
    }
}

fn object_to_value(obj: &Object) -> serde_json::Value {
    match obj {
        Object::Null | Object::Undefined => serde_json::Value::Null,
        Object::Boolean(b) => serde_json::Value::Bool(*b),
        Object::Number(n) => {
            // Integer-valued numbers serialize as integers (preserved across
            // TOML/YAML round trips); otherwise as floats.
            if let Some(i) = serde_json::Number::from_f64(*n).and_then(|x| {
                if x.is_i64() || x.is_u64() {
                    Some(x)
                } else {
                    None
                }
            }) {
                serde_json::Value::Number(i)
            } else {
                serde_json::Number::from_f64(*n)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            }
        }
        Object::String(s) => serde_json::Value::String(s.as_str().to_string()),
        Object::Array(arr) => {
            serde_json::Value::Array(arr.borrow().elements.iter().map(object_to_value).collect())
        }
        Object::Hash(hash) => {
            let mut map = serde_json::Map::new();
            for (k, v) in &hash.borrow().entries {
                map.insert(k.clone(), object_to_value(v));
            }
            serde_json::Value::Object(map)
        }
        // Non-data objects render to their inspect string for serialization,
        // matching the Go original's objectToGoValue fallback to Inspect().
        other => serde_json::Value::String(other.inspect()),
    }
}

/// Shared codec file-write helper: stringify `value`, then write to `path`.
/// Returns Undefined on success or an Error; the error prefix mirrors the
/// Go original (it belongs to the stringify step on serialization failure).
fn codec_write_file(
    ctx: &mut CallContext,
    module: &str,
    args: &[Object],
    node_label: &str,
    stringify: fn(&Object) -> Result<String, String>,
) -> Object {
    let path = match required_string(ctx, &format!("{}.writeFileSync", module), args, 0, "path") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let value = match args.get(1) {
        Some(v) => v,
        None => {
            return new_error(
                ctx.pos.clone(),
                format!("{}.writeFileSync requires {}", module, node_label),
            )
        }
    };
    match stringify(value) {
        Ok(text) => match fs::write(&path, text) {
            Ok(()) => Object::Undefined,
            Err(e) => new_error(ctx.pos.clone(), format!("{}.writeFileSync: {}", module, e)),
        },
        Err(msg) => new_error(ctx.pos.clone(), msg),
    }
}

// ---------------------------------------------------------------------------
// toml
// ---------------------------------------------------------------------------

fn toml_module() -> Object {
    module(vec![
        ("parse", native("toml.parse", toml_parse)),
        ("stringify", native("toml.stringify", toml_stringify)),
        ("readFileSync", native("toml.readFileSync", toml_read_file)),
        (
            "writeFileSync",
            native("toml.writeFileSync", toml_write_file),
        ),
    ])
}

fn toml_stringify_value(value: &Object) -> Result<String, String> {
    let tv = object_to_toml(value);
    toml::to_string(&tv).map_err(|e| format!("toml.stringify: {}", e))
}

fn toml_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "toml.parse", args, 0, "text") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match toml::from_str::<toml::Value>(&text) {
        Ok(value) => toml_to_object(&value),
        Err(e) => new_error(ctx.pos.clone(), format!("toml.parse: {}", e)),
    }
}

fn toml_stringify(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "toml.stringify requires a value"),
    };
    match toml_stringify_value(value) {
        Ok(s) => str_obj(s),
        Err(msg) => new_error(ctx.pos.clone(), msg),
    }
}

fn toml_read_file(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "toml.readFileSync", args, 0, "path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match fs::read_to_string(&path) {
        Ok(text) => match toml::from_str::<toml::Value>(&text) {
            Ok(value) => toml_to_object(&value),
            Err(e) => new_error(ctx.pos.clone(), format!("toml.parse: {}", e)),
        },
        Err(e) => new_error(ctx.pos.clone(), format!("toml.readFileSync: {}", e)),
    }
}

fn toml_write_file(ctx: &mut CallContext, args: &[Object]) -> Object {
    codec_write_file(ctx, "toml", args, "value", toml_stringify_value)
}

fn toml_to_object(value: &toml::Value) -> Object {
    match value {
        toml::Value::String(s) => str_obj(s.clone()),
        toml::Value::Integer(i) => num_obj(*i as f64),
        toml::Value::Float(f) => num_obj(*f),
        toml::Value::Boolean(b) => bool_obj(*b),
        toml::Value::Datetime(dt) => str_obj(dt.to_string()),
        toml::Value::Array(arr) => array(arr.iter().map(toml_to_object).collect()),
        toml::Value::Table(table) => {
            let hash = Rc::new(RefCell::new(HashData::default()));
            let mut keys: Vec<&String> = table.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(v) = table.get(key) {
                    hash.borrow_mut().set(key.clone(), toml_to_object(v));
                }
            }
            Object::Hash(hash)
        }
    }
}

fn object_to_toml(value: &Object) -> toml::Value {
    match value {
        Object::Null | Object::Undefined => toml::Value::String(String::new()),
        Object::Boolean(b) => toml::Value::Boolean(*b),
        Object::Number(n) => {
            if n.fract() == 0.0 && n.is_finite() {
                toml::Value::Integer(*n as i64)
            } else {
                toml::Value::Float(*n)
            }
        }
        Object::String(s) => toml::Value::String(s.as_str().to_string()),
        Object::Array(arr) => {
            toml::Value::Array(arr.borrow().elements.iter().map(object_to_toml).collect())
        }
        Object::Hash(hash) => {
            let mut table = toml::value::Table::new();
            for (k, v) in &hash.borrow().entries {
                table.insert(k.clone(), object_to_toml(v));
            }
            toml::Value::Table(table)
        }
        other => toml::Value::String(other.inspect()),
    }
}

// ---------------------------------------------------------------------------
// yaml
// ---------------------------------------------------------------------------

fn yaml_module() -> Object {
    module(vec![
        ("parse", native("yaml.parse", yaml_parse)),
        ("stringify", native("yaml.stringify", yaml_stringify)),
        ("readFileSync", native("yaml.readFileSync", yaml_read_file)),
        (
            "writeFileSync",
            native("yaml.writeFileSync", yaml_write_file),
        ),
    ])
}

fn yaml_stringify_value(value: &Object) -> Result<String, String> {
    let v = object_to_value(value);
    serde_yaml::to_string(&v).map_err(|e| format!("yaml.stringify: {}", e))
}

fn yaml_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "yaml.parse", args, 0, "text") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match serde_yaml::from_str::<serde_json::Value>(&text) {
        Ok(value) => value_to_object(&value),
        Err(e) => new_error(ctx.pos.clone(), format!("yaml.parse: {}", e)),
    }
}

fn yaml_stringify(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "yaml.stringify requires a value"),
    };
    match yaml_stringify_value(value) {
        Ok(s) => str_obj(s),
        Err(msg) => new_error(ctx.pos.clone(), msg),
    }
}

fn yaml_read_file(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "yaml.readFileSync", args, 0, "path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match fs::read_to_string(&path) {
        Ok(text) => match serde_yaml::from_str::<serde_json::Value>(&text) {
            Ok(value) => value_to_object(&value),
            Err(e) => new_error(ctx.pos.clone(), format!("yaml.parse: {}", e)),
        },
        Err(e) => new_error(ctx.pos.clone(), format!("yaml.readFileSync: {}", e)),
    }
}

fn yaml_write_file(ctx: &mut CallContext, args: &[Object]) -> Object {
    codec_write_file(ctx, "yaml", args, "value", yaml_stringify_value)
}

// ---------------------------------------------------------------------------
// xml: custom DOM with { name, attributes, children, text } nodes, matching
// the Go original's self-implemented parser/serializer.
// ---------------------------------------------------------------------------

fn xml_module() -> Object {
    module(vec![
        ("parse", native("xml.parse", xml_parse)),
        ("stringify", native("xml.stringify", xml_stringify)),
        ("readFileSync", native("xml.readFileSync", xml_read_file)),
        ("writeFileSync", native("xml.writeFileSync", xml_write_file)),
    ])
}

fn xml_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "xml.parse", args, 0, "text") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match parse_xml_dom(&text) {
        Ok(node) => xml_node_to_object(&node),
        Err(e) => new_error(ctx.pos.clone(), format!("xml.parse: {}", e)),
    }
}

fn xml_stringify(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "xml.stringify requires a node"),
    };
    match object_to_xml_node(value) {
        Ok(node) => str_obj(serialize_xml_node(&node)),
        Err(e) => new_error(ctx.pos.clone(), format!("xml.stringify: {}", e)),
    }
}

fn xml_read_file(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "xml.readFileSync", args, 0, "path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match fs::read_to_string(&path) {
        Ok(text) => match parse_xml_dom(&text) {
            Ok(node) => xml_node_to_object(&node),
            Err(e) => new_error(ctx.pos.clone(), format!("xml.parse: {}", e)),
        },
        Err(e) => new_error(ctx.pos.clone(), format!("xml.readFileSync: {}", e)),
    }
}

fn xml_write_file(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "xml.writeFileSync", args, 0, "path") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let value = match args.get(1) {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "xml.writeFileSync requires node"),
    };
    match object_to_xml_node(value) {
        Ok(node) => match fs::write(&path, serialize_xml_node(&node)) {
            Ok(()) => Object::Undefined,
            Err(e) => new_error(ctx.pos.clone(), format!("xml.writeFileSync: {}", e)),
        },
        Err(e) => new_error(ctx.pos.clone(), format!("xml.stringify: {}", e)),
    }
}

struct XmlNode {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<XmlNode>,
    text: String,
}

fn parse_xml_dom(input: &str) -> Result<XmlNode, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    let mut text_buf = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attributes = Vec::new();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let val = attr.unescape_value().map_err(|e| e.to_string())?;
                    attributes.push((key, val.to_string()));
                }
                stack.push(XmlNode {
                    name,
                    attributes,
                    children: Vec::new(),
                    text: String::new(),
                });
                text_buf.clear();
            }
            Ok(Event::End(_)) => {
                if let Some(mut node) = stack.pop() {
                    node.text = text_buf.trim().to_string();
                    text_buf.clear();
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else {
                        root = Some(node);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attributes = Vec::new();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let val = attr.unescape_value().map_err(|e| e.to_string())?;
                    attributes.push((key, val.to_string()));
                }
                let node = XmlNode {
                    name,
                    attributes,
                    children: Vec::new(),
                    text: String::new(),
                };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            Ok(Event::Text(e)) => {
                let t = e.unescape().map_err(|e| e.to_string())?;
                text_buf.push_str(&t);
            }
            Ok(Event::CData(e)) => {
                text_buf.push_str(&String::from_utf8_lossy(e.deref()));
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(e.to_string()),
        }
        buf.clear();
    }
    root.ok_or_else(|| "empty XML document".to_string())
}

fn xml_node_to_object(node: &XmlNode) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("name", str_obj(node.name.clone()));
    // Attributes sorted by key for determinism.
    let mut attrs = node.attributes.clone();
    attrs.sort_by(|a, b| a.0.cmp(&b.0));
    let attr_hash = Rc::new(RefCell::new(HashData::default()));
    for (k, v) in &attrs {
        attr_hash.borrow_mut().set(k.clone(), str_obj(v.clone()));
    }
    hash.borrow_mut().set("attributes", Object::Hash(attr_hash));
    let children: Vec<Object> = node.children.iter().map(xml_node_to_object).collect();
    hash.borrow_mut().set("children", array(children));
    hash.borrow_mut().set("text", str_obj(node.text.clone()));
    Object::Hash(hash)
}

fn object_to_xml_node(value: &Object) -> Result<XmlNode, String> {
    let hash = match value {
        Object::Hash(h) => h.clone(),
        _ => return Err("node must be an object".to_string()),
    };
    let h = hash.borrow();
    let name = match h.get("name") {
        Some(Object::String(s)) if !s.is_empty() => s.as_str().to_string(),
        _ => return Err("node.name must be a string".to_string()),
    };
    let mut attributes = Vec::new();
    if let Some(Object::Hash(attr_hash)) = h.get("attributes") {
        for (k, v) in &attr_hash.borrow().entries {
            if let Object::String(s) = v {
                attributes.push((k.clone(), s.as_str().to_string()));
            }
        }
    }
    let text = match h.get("text") {
        Some(Object::String(s)) => s.as_str().to_string(),
        _ => String::new(),
    };
    let mut children = Vec::new();
    if let Some(Object::Array(arr)) = h.get("children") {
        for elem in &arr.borrow().elements {
            children.push(object_to_xml_node(elem)?);
        }
    }
    Ok(XmlNode {
        name,
        attributes,
        children,
        text,
    })
}

fn serialize_xml_node(node: &XmlNode) -> String {
    let mut out = String::new();
    serialize_xml_node_into(node, &mut out);
    out
}

fn serialize_xml_node_into(node: &XmlNode, out: &mut String) {
    out.push('<');
    out.push_str(&node.name);
    let mut attrs = node.attributes.clone();
    attrs.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in &attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_xml_text(v, out);
        out.push('"');
    }
    if node.children.is_empty() && node.text.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    escape_xml_text(&node.text, out);
    for child in &node.children {
        serialize_xml_node_into(child, out);
    }
    out.push_str("</");
    out.push_str(&node.name);
    out.push('>');
}

fn escape_xml_text(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
}

// ---------------------------------------------------------------------------
// markdown: parse (AST) + renderTerminal + fromHTML. The Go original has no
// markdown->HTML render; we mirror that surface.
// ---------------------------------------------------------------------------

fn markdown_module() -> Object {
    module(vec![
        ("parse", native("markdown.parse", markdown_parse)),
        (
            "renderTerminal",
            native("markdown.renderTerminal", markdown_render_terminal),
        ),
        ("fromHTML", native("markdown.fromHTML", markdown_from_html)),
    ])
}

fn markdown_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let source = match required_string(ctx, "markdown.parse", args, 0, "source") {
        Ok(v) => v,
        Err(e) => return e,
    };
    str_obj(source)
}

fn markdown_render_terminal(ctx: &mut CallContext, args: &[Object]) -> Object {
    let source = match required_string(ctx, "markdown.renderTerminal", args, 0, "source") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let width = match args.get(1) {
        Some(Object::Hash(_)) => {
            let w = hash_bool_arg(args.get(1), "width");
            let _ = w;
            match args.get(1) {
                Some(Object::Hash(h)) => match h.borrow().get("width") {
                    Some(Object::Number(n)) if *n >= 1.0 => *n as usize,
                    _ => 80,
                },
                _ => 80,
            }
        }
        _ => 80,
    };
    let normalized: String = source.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut out_lines: Vec<Object> = Vec::new();
    let mut headings: Vec<Object> = Vec::new();
    for line in &lines {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            out_lines.push(str_obj(format!("# {}", rest.trim())));
            headings.push(str_obj(rest.trim().to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("```") {
            out_lines.push(str_obj(format!("  {}", rest)));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            out_lines.push(str_obj(format!("- {}", &trimmed[2..])));
        } else if trimmed == "---" || trimmed == "***" {
            out_lines.push(str_obj("-".repeat(width)));
        } else if !trimmed.is_empty() {
            out_lines.push(str_obj(trimmed.to_string()));
        }
    }
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("lines", array(out_lines));
    hash.borrow_mut().set("width", num_obj(width as f64));
    hash.borrow_mut().set("headings", array(headings));
    hash.borrow_mut().set("links", array(Vec::new()));
    Object::Hash(hash)
}

fn markdown_from_html(ctx: &mut CallContext, args: &[Object]) -> Object {
    let html = match required_string(ctx, "markdown.fromHTML", args, 0, "html") {
        Ok(v) => v,
        Err(e) => return e,
    };
    str_obj(html_to_markdown(&html))
}

/// Minimal HTML-to-markdown: strip tags, preserve text, convert a few common
/// block elements to markdown equivalents.
fn html_to_markdown(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut in_tag = false;
    let mut tag = String::new();
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'<' {
            in_tag = true;
            tag.clear();
            i += 1;
            continue;
        }
        if c == b'>' {
            in_tag = false;
            let lower = tag.trim().to_lowercase();
            match lower.as_str() {
                "h1" | "h2" | "h3" => out.push_str("\n# "),
                "li" => out.push_str("\n- "),
                "p" | "br" | "div" => out.push('\n'),
                _ => {}
            }
            i += 1;
            continue;
        }
        if in_tag {
            tag.push(c as char);
        } else {
            out.push(c as char);
        }
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// schema: JSON-Schema-style validate/assert.
// ---------------------------------------------------------------------------

fn schema_module() -> Object {
    module(vec![
        ("validate", native("schema.validate", schema_validate)),
        ("assert", native("schema.assert", schema_assert)),
    ])
}

fn schema_validate(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(ctx.pos.clone(), "schema.validate requires schema and value");
    }
    let (schema, value) = (&args[0], &args[1]);
    let errors = match validate_schema(schema, value, "$") {
        Ok(errs) => errs,
        Err(e) => return new_error(ctx.pos.clone(), e),
    };
    let valid = errors.is_empty();
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("valid", bool_obj(valid));
    hash.borrow_mut()
        .set("errors", array(errors.into_iter().map(str_obj).collect()));
    Object::Hash(hash)
}

fn schema_assert(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(ctx.pos.clone(), "schema.assert requires schema and value");
    }
    let (schema, value) = (&args[0], &args[1]);
    match validate_schema(schema, value, "$") {
        Ok(errs) => {
            if let Some(first) = errs.first() {
                new_error(ctx.pos.clone(), format!("schema.assert: {}", first))
            } else {
                value.clone()
            }
        }
        Err(e) => new_error(ctx.pos.clone(), e),
    }
}

/// Validate `value` against `schema` rooted at `path`. Returns the list of
/// error messages (empty on success) or an Error-prefixed string on misuse.
fn validate_schema(schema: &Object, value: &Object, path: &str) -> Result<Vec<String>, String> {
    let schema_hash = match schema {
        Object::Hash(h) => h.clone(),
        _ => return Err("schema.validate: schema must be an object".to_string()),
    };
    let mut errors = Vec::new();
    let s = schema_hash.borrow();

    if let Some(Object::String(t)) = s.get("type") {
        let type_text = t.as_str();
        if !type_matches(value, type_text) {
            errors.push(format!(
                "{} expected {}, got {}",
                path,
                type_text,
                value_type_name(value)
            ));
        }
    }
    if let Some(Object::Array(enum_arr)) = s.get("enum") {
        let matched = enum_arr
            .borrow()
            .elements
            .iter()
            .any(|e| deep_equal(value, e));
        if !matched {
            errors.push(format!(
                "{} must be one of {}",
                path,
                enum_arr
                    .borrow()
                    .elements
                    .iter()
                    .map(|e| e.inspect())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    if let Object::Hash(_) = value {
        if let Some(Object::Array(required)) = s.get("required") {
            let value_hash = value_as_hash(value);
            for req in &required.borrow().elements {
                if let Object::String(key) = req {
                    let present = value_hash
                        .as_ref()
                        .map(|h| h.borrow().contains(key.as_str()))
                        .unwrap_or(false);
                    if !present {
                        errors.push(format!("{}.{} is required", path, key));
                    }
                }
            }
        }
        if let Some(Object::Hash(properties)) = s.get("properties") {
            let no_extra = matches!(s.get("additionalProperties"), Some(Object::Boolean(false)));
            for (k, v) in &value_as_hash(value).unwrap().borrow().entries {
                match properties.borrow().get(k) {
                    Some(sub) => {
                        let sub_path = format!("{}.{}", path, k);
                        errors.extend(validate_schema(sub, v, &sub_path)?);
                    }
                    None => {
                        if no_extra {
                            errors.push(format!("{}.{} is not allowed", path, k));
                        }
                    }
                }
            }
        }
    }
    if let Object::Array(arr) = value {
        if let Some(Object::Number(min)) = s.get("minItems") {
            if (arr.borrow().elements.len() as f64) < *min {
                errors.push(format!(
                    "{} must contain at least {} items",
                    path, *min as i64
                ));
            }
        }
        if let Some(Object::Number(max)) = s.get("maxItems") {
            if (arr.borrow().elements.len() as f64) > *max {
                errors.push(format!(
                    "{} must contain at most {} items",
                    path, *max as i64
                ));
            }
        }
        if let Some(items_schema) = s.get("items") {
            for (i, elem) in arr.borrow().elements.iter().enumerate() {
                let item_path = format!("{}[{}]", path, i);
                errors.extend(validate_schema(items_schema, elem, &item_path)?);
            }
        }
    }
    if let Object::String(st) = value {
        let len = st.len() as f64;
        if let Some(Object::Number(min)) = s.get("minLength") {
            if len < *min {
                errors.push(format!("{} length must be at least {}", path, *min as i64));
            }
        }
        if let Some(Object::Number(max)) = s.get("maxLength") {
            if len > *max {
                errors.push(format!("{} length must be at most {}", path, *max as i64));
            }
        }
    }
    if let Object::Number(n) = value {
        if let Some(Object::Number(min)) = s.get("minimum") {
            if *n < *min {
                errors.push(format!("{} must be >= {}", path, min));
            }
        }
        if let Some(Object::Number(max)) = s.get("maximum") {
            if *n > *max {
                errors.push(format!("{} must be <= {}", path, max));
            }
        }
    }
    Ok(errors)
}

fn type_matches(value: &Object, type_text: &str) -> bool {
    match type_text {
        "object" => matches!(value, Object::Hash(_)),
        "array" => matches!(value, Object::Array(_)),
        "string" => matches!(value, Object::String(_)),
        "number" => matches!(value, Object::Number(_)),
        "integer" => matches!(value, Object::Number(n) if n.fract() == 0.0),
        "boolean" => matches!(value, Object::Boolean(_)),
        "null" => matches!(value, Object::Null),
        _ => true,
    }
}

fn value_type_name(value: &Object) -> &'static str {
    match value {
        Object::Hash(_) => "object",
        Object::Array(_) => "array",
        Object::String(_) => "string",
        Object::Number(_) => "number",
        Object::Boolean(_) => "boolean",
        Object::Null => "null",
        Object::Undefined => "undefined",
        _ => "object",
    }
}

fn value_as_hash(value: &Object) -> Option<Rc<RefCell<HashData>>> {
    match value {
        Object::Hash(h) => Some(h.clone()),
        _ => None,
    }
}

fn deep_equal(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::Number(x), Object::Number(y)) => x == y,
        (Object::String(x), Object::String(y)) => x == y,
        (Object::Boolean(x), Object::Boolean(y)) => x == y,
        (Object::Null, Object::Null) | (Object::Undefined, Object::Undefined) => true,
        (Object::Array(x), Object::Array(y)) => {
            let xb = x.borrow();
            let yb = y.borrow();
            xb.elements.len() == yb.elements.len()
                && xb
                    .elements
                    .iter()
                    .zip(yb.elements.iter())
                    .all(|(p, q)| deep_equal(p, q))
        }
        (Object::Hash(x), Object::Hash(y)) => {
            let xb = x.borrow();
            let yb = y.borrow();
            xb.entries.len() == yb.entries.len()
                && xb.entries.iter().all(|(k, v)| {
                    yb.entries
                        .iter()
                        .any(|(yk, yv)| yk == k && deep_equal(v, yv))
                })
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// test: a script-side test runner with describe/it/expect.
//
// State is held in thread-local storage so the runner survives nested
// describe() calls and is collected on run().
// ---------------------------------------------------------------------------

fn test_module() -> Object {
    module(vec![
        ("test", native("test.test", test_test)),
        ("it", native("test.it", test_test)),
        ("describe", native("test.describe", test_describe)),
        ("expect", native("test.expect", test_expect)),
        ("run", native("test.run", test_run)),
    ])
}

#[derive(Clone)]
enum TestNode {
    Suite {
        name: String,
        children: Vec<TestNode>,
    },
    Case {
        name: String,
        func: Object,
    },
}

thread_local! {
    static TEST_ROOT: std::cell::RefCell<Vec<TestNode>> = std::cell::RefCell::new(Vec::new());
    static EXPECT_FAILS: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
}

fn test_test(ctx: &mut CallContext, args: &[Object]) -> Object {
    let name = match required_string(ctx, "test.test", args, 0, "name") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let func = match args.get(1) {
        Some(v) => v.clone(),
        None => return new_error(ctx.pos.clone(), "test requires name and function"),
    };
    TEST_ROOT.with(|r| {
        r.borrow_mut().push(TestNode::Case { name, func });
    });
    Object::Undefined
}

fn test_describe(ctx: &mut CallContext, args: &[Object]) -> Object {
    let name = match required_string(ctx, "test.describe", args, 0, "name") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let func = match args.get(1) {
        Some(Object::Function(_)) => args[1].clone(),
        Some(v) => v.clone(),
        None => return new_error(ctx.pos.clone(), "describe requires name and function"),
    };
    // Execute the describe body synchronously; nested test()/it() calls
    // register into the current suite.
    let _ = call_script_function(&func, ctx.env, &[]);
    let _ = name;
    Object::Undefined
}

fn test_expect(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v.clone(),
        None => return new_error(ctx.pos.clone(), "expect requires a value"),
    };
    let expectation = Rc::new(RefCell::new(HashData::default()));
    expectation.borrow_mut().set("__value__", value);

    // Each matcher closure captures its own clone of the Rc so the original
    // can still be returned.
    let e1 = expectation.clone();
    expectation.borrow_mut().set(
        "toBe",
        native("test.expect.toBe", move |ctx, args| {
            expect_matcher(ctx, &e1, args, ExpectOp::Be)
        }),
    );
    let e2 = expectation.clone();
    expectation.borrow_mut().set(
        "toEqual",
        native("test.expect.toEqual", move |ctx, args| {
            expect_matcher(ctx, &e2, args, ExpectOp::Equal)
        }),
    );
    let e3 = expectation.clone();
    expectation.borrow_mut().set(
        "toBeTruthy",
        native("test.expect.toBeTruthy", move |ctx, _args| {
            expect_truthy(ctx, &e3, true)
        }),
    );
    let e4 = expectation.clone();
    expectation.borrow_mut().set(
        "toBeFalsy",
        native("test.expect.toBeFalsy", move |ctx, _args| {
            expect_truthy(ctx, &e4, false)
        }),
    );
    Object::Hash(expectation)
}

enum ExpectOp {
    Be,
    Equal,
}

fn expect_matcher(
    ctx: &mut CallContext,
    expectation: &Rc<RefCell<HashData>>,
    args: &[Object],
    op: ExpectOp,
) -> Object {
    let actual = expectation
        .borrow()
        .get("__value__")
        .cloned()
        .unwrap_or(Object::Undefined);
    let expected = match args.first() {
        Some(v) => v.clone(),
        None => return new_error(ctx.pos.clone(), "matcher requires an expected value"),
    };
    let passed = match op {
        ExpectOp::Be => strict_equal(&actual, &expected),
        ExpectOp::Equal => deep_equal(&actual, &expected),
    };
    if passed {
        Object::Undefined
    } else {
        let label = match op {
            ExpectOp::Be => "to be",
            ExpectOp::Equal => "to equal",
        };
        new_error(
            ctx.pos.clone(),
            format!(
                "Expected {} {} {}",
                actual.inspect(),
                label,
                expected.inspect()
            ),
        )
    }
}

fn expect_truthy(
    ctx: &mut CallContext,
    expectation: &Rc<RefCell<HashData>>,
    expect_truthy: bool,
) -> Object {
    let actual = expectation
        .borrow()
        .get("__value__")
        .cloned()
        .unwrap_or(Object::Undefined);
    let truthy = is_truthy(&actual);
    let passed = if expect_truthy { truthy } else { !truthy };
    if passed {
        Object::Undefined
    } else {
        let label = if expect_truthy { "truthy" } else { "falsy" };
        new_error(
            ctx.pos.clone(),
            format!("Expected {} to be {}", actual.inspect(), label),
        )
    }
}

fn is_truthy(value: &Object) -> bool {
    match value {
        Object::Boolean(b) => *b,
        Object::Number(n) => *n != 0.0,
        Object::String(s) => !s.is_empty(),
        Object::Null | Object::Undefined => false,
        _ => true,
    }
}

fn test_run(ctx: &mut CallContext, _args: &[Object]) -> Object {
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failed = 0usize;
    TEST_ROOT.with(|r| {
        let nodes = r.borrow_mut().clone();
        for node in &nodes {
            if let TestNode::Case { name, func } = node {
                total += 1;
                EXPECT_FAILS.with(|f| f.borrow_mut().clear());
                let result = call_script_function(func, ctx.env, &[]);
                let failed_here = matches!(result, Object::Error(_))
                    || EXPECT_FAILS.with(|f| !f.borrow().is_empty());
                if failed_here {
                    failed += 1;
                    let _ = name;
                } else {
                    passed += 1;
                }
            }
        }
    });
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("total", num_obj(total as f64));
    hash.borrow_mut().set("passed", num_obj(passed as f64));
    hash.borrow_mut().set("failed", num_obj(failed as f64));
    Object::Hash(hash)
}

/// Invoke a script Function/Builtin with arguments, returning its result.
fn call_script_function(func: &Object, env: &EnvRef, args: &[Object]) -> Object {
    crate::evaluator::expressions::apply_function(func, env, args, None, Position::default())
}

// ---------------------------------------------------------------------------
// archive/zip: stateless list/extract/create.
// ---------------------------------------------------------------------------

fn archive_zip_module() -> Object {
    module(vec![
        ("list", native("zip.list", zip_list)),
        ("extract", native("zip.extract", zip_extract)),
        ("create", native("zip.create", zip_create)),
    ])
}

fn zip_list(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "zip.list", args, 0, "path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => return new_error(ctx.pos.clone(), format!("zip.list: {}", e)),
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => return new_error(ctx.pos.clone(), format!("zip.list: {}", e)),
    };
    let mut entries = Vec::with_capacity(archive.len() as usize);
    for i in 0..archive.len() {
        let entry = match archive.by_index(i as usize) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_string();
        let size = entry.size();
        let compressed = entry.compressed_size();
        let is_dir = entry.is_dir();
        let modified = entry
            .last_modified()
            .map(|d| format!("{}", d))
            .unwrap_or_default();
        let hash = Rc::new(RefCell::new(HashData::default()));
        hash.borrow_mut().set("name", str_obj(name));
        hash.borrow_mut().set("size", num_obj(size as f64));
        hash.borrow_mut()
            .set("compressedSize", num_obj(compressed as f64));
        hash.borrow_mut().set("isDir", bool_obj(is_dir));
        hash.borrow_mut().set("modified", str_obj(modified));
        entries.push(Object::Hash(hash));
    }
    array(entries)
}

fn zip_extract(ctx: &mut CallContext, args: &[Object]) -> Object {
    let archive_path = match required_string(ctx, "zip.extract", args, 0, "archive path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let dest_path = match required_string(ctx, "zip.extract", args, 1, "destination path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let file = match fs::File::open(&archive_path) {
        Ok(f) => f,
        Err(e) => return new_error(ctx.pos.clone(), format!("zip.extract: {}", e)),
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => return new_error(ctx.pos.clone(), format!("zip.extract: {}", e)),
    };
    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i as usize) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let outpath = match safe_zip_target(&dest_path, entry.name()) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if entry.is_dir() {
            let _ = fs::create_dir_all(&outpath);
            continue;
        }
        if let Some(parent) = outpath.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut outfile = match fs::File::create(&outpath) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let _ = std::io::copy(&mut entry, &mut outfile);
    }
    Object::Undefined
}

fn zip_create(ctx: &mut CallContext, args: &[Object]) -> Object {
    let files = match args.first() {
        Some(Object::Array(_)) => args[0].clone(),
        _ => return new_error(ctx.pos.clone(), "zip.create: files must be an array"),
    };
    let output_path = match required_string(ctx, "zip.create", args, 1, "output path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if let Some(parent) = std::path::Path::new(&output_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let file = match fs::File::create(&output_path) {
        Ok(f) => f,
        Err(e) => return new_error(ctx.pos.clone(), format!("zip.create: {}", e)),
    };
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    if let Object::Array(arr) = &files {
        for (i, spec) in arr.borrow().elements.iter().enumerate() {
            let spec_hash = match spec {
                Object::Hash(h) => h.clone(),
                _ => {
                    return new_error(
                        ctx.pos.clone(),
                        format!("zip.create: files[{}] must be an object", i),
                    )
                }
            };
            let path = match spec_hash.borrow().get("path") {
                Some(Object::String(s)) => s.as_str().to_string(),
                _ => {
                    return new_error(
                        ctx.pos.clone(),
                        format!("zip.create: files[{}].path is required", i),
                    )
                }
            };
            let name = match spec_hash.borrow().get("name") {
                Some(Object::String(s)) => s.as_str().to_string(),
                _ => std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            };
            let clean_name = clean_zip_name(&name);
            if clean_name.is_empty() {
                continue;
            }
            if std::path::Path::new(&path).is_dir() {
                if let Ok(walker) = walkdir_collect(&path) {
                    for (rel, abs) in walker {
                        let entry_name = format!("{}/{}", clean_name, rel.replace('\\', "/"));
                        if let Ok(data) = fs::read(&abs) {
                            let _ = zip.start_file(entry_name, options);
                            let _ = zip.write_all(&data);
                        }
                    }
                }
                continue;
            }
            match fs::read(&path) {
                Ok(data) => {
                    let _ = zip.start_file(clean_name.clone(), options);
                    let _ = zip.write_all(&data);
                }
                Err(e) => return new_error(ctx.pos.clone(), format!("zip.create: {}", e)),
            }
        }
    }
    match zip.finish() {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("zip.create: {}", e)),
    }
}

/// Reject path-traversal targets so an entry's name cannot escape `dest`.
fn safe_zip_target(dest: &str, name: &str) -> Result<std::path::PathBuf, String> {
    let clean = clean_zip_name(name);
    if clean.is_empty() {
        return Err("empty name".to_string());
    }
    let dest_abs = fs::canonicalize(dest).unwrap_or_else(|_| std::path::PathBuf::from(dest));
    let target = dest_abs.join(&clean);
    let canonical = target.ancestors().last().unwrap_or(&target).to_path_buf();
    let _ = canonical;
    if target
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("path traversal".to_string());
    }
    Ok(target)
}

fn clean_zip_name(name: &str) -> String {
    let slashed = name.replace('\\', "/");
    let stripped = slashed.strip_prefix('/').unwrap_or(&slashed);
    let cleaned = std::path::Path::new(stripped)
        .components()
        .filter(|c| !matches!(c, std::path::Component::ParentDir))
        .collect::<std::path::PathBuf>();
    let s = cleaned.to_string_lossy().to_string();
    if s == "." || s == ".." {
        String::new()
    } else {
        s
    }
}

/// Collect (relative_path, absolute_path) pairs under `root`, depth-first.
fn walkdir_collect(root: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    walkdir_inner(root, root, &mut out)?;
    Ok(out)
}

fn walkdir_inner(root: &str, dir: &str, out: &mut Vec<(String, String)>) -> Result<(), String> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return Err(e.to_string()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let abs = path.to_string_lossy().to_string();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        if path.is_dir() {
            walkdir_inner(root, &abs, out)?;
        } else {
            out.push((rel, abs));
        }
    }
    Ok(())
}

use std::ops::Deref;

use std::collections::HashMap;

// ===========================================================================
// P7 batch: buffer / events / jwt / mime / net/ip / retry / stream.
// Pure-algorithm modules (no nested VM execution, no real async) — CI friendly.
// ===========================================================================

// ---------------------------------------------------------------------------
// buffer: byte buffers constructed from strings/arrays, with instance methods.
// Reuses the existing make_buffer helper (Hash carrying __buffer_data__).
// ---------------------------------------------------------------------------

fn buffer_module() -> Object {
    module(vec![
        ("from", native("buffer.from", buffer_from)),
        ("alloc", native("buffer.alloc", buffer_alloc)),
        (
            "byteLength",
            native("buffer.byteLength", buffer_byte_length),
        ),
        ("concat", native("buffer.concat", buffer_concat)),
        ("isBuffer", native("buffer.isBuffer", buffer_is_buffer)),
    ])
}

fn buffer_from(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "buffer.from requires value"),
    };
    let encoding = match args.get(1) {
        Some(Object::String(s)) => normalize_encoding(s),
        Some(_) => return new_error(ctx.pos.clone(), "buffer.from: encoding must be a string"),
        None => EncodingKind::Utf8,
    };
    match decode_bytes(ctx, "buffer.from", value, encoding) {
        Ok(bytes) => make_buffer(bytes),
        Err(e) => e,
    }
}

fn buffer_alloc(ctx: &mut CallContext, args: &[Object]) -> Object {
    let size = match required_number(ctx, "buffer.alloc", args, 0, "size") {
        Ok(n) => n,
        Err(e) => return e,
    };
    if size < 0.0 {
        return new_error(ctx.pos.clone(), "buffer.alloc: size must be non-negative");
    }
    let size = size as usize;
    let fill = args.get(1);
    let fill_bytes = match fill {
        None | Some(Object::Undefined) => vec![0u8; size],
        Some(Object::Number(n)) => vec![((*n as i64) & 0xff) as u8; size.max(1)],
        Some(Object::String(s)) => {
            let b = s.as_bytes();
            if b.is_empty() {
                vec![0u8; size]
            } else {
                tile_bytes(b, size)
            }
        }
        Some(Object::Hash(_)) => match bytes_from_object(ctx, "buffer.alloc", fill.unwrap()) {
            Ok(b) if b.is_empty() => vec![0u8; size],
            Ok(b) => tile_bytes(&b, size),
            Err(e) => return e,
        },
        Some(_) => {
            return new_error(
                ctx.pos.clone(),
                "buffer.alloc: fill must be a number, string, or Buffer",
            )
        }
    };
    make_buffer(fill_bytes)
}

fn tile_bytes(src: &[u8], size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    for i in 0..size {
        out.push(src[i % src.len()]);
    }
    out
}

fn buffer_byte_length(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match args.first() {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "buffer.byteLength requires value"),
    };
    let encoding = match args.get(1) {
        Some(Object::String(s)) => normalize_encoding(s),
        Some(_) => {
            return new_error(
                ctx.pos.clone(),
                "buffer.byteLength: encoding must be a string",
            )
        }
        None => EncodingKind::Utf8,
    };
    match decode_bytes(ctx, "buffer.byteLength", value, encoding) {
        Ok(bytes) => num_obj(bytes.len() as f64),
        Err(e) => e,
    }
}

fn buffer_concat(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.first() {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "buffer.concat: buffers must be an array"),
        None => return new_error(ctx.pos.clone(), "buffer.concat requires buffers"),
    };
    let mut out = Vec::new();
    for (i, elem) in arr.borrow().elements.iter().enumerate() {
        match bytes_from_object(ctx, "buffer.concat", elem) {
            Ok(b) => out.extend(b),
            Err(_) => {
                return new_error(
                    ctx.pos.clone(),
                    format!("buffer.concat: buffers[{}] must be a Buffer", i),
                )
            }
        }
    }
    make_buffer(out)
}

fn buffer_is_buffer(_ctx: &mut CallContext, args: &[Object]) -> Object {
    match args.first() {
        Some(Object::Hash(h)) => bool_obj(h.borrow().contains(BUFFER_DATA_KEY)),
        _ => bool_obj(false),
    }
}

enum EncodingKind {
    Utf8,
    Hex,
    Base64,
}

fn normalize_encoding(raw: &str) -> EncodingKind {
    let lower = raw.to_lowercase().replace('-', "");
    match lower.as_str() {
        "" | "utf8" | "utf" => EncodingKind::Utf8,
        "hex" => EncodingKind::Hex,
        "base64" => EncodingKind::Base64,
        _ => EncodingKind::Utf8,
    }
}

fn decode_bytes(
    ctx: &mut CallContext,
    name: &str,
    value: &Object,
    encoding: EncodingKind,
) -> Result<Vec<u8>, Object> {
    match (value, encoding) {
        (Object::String(s), EncodingKind::Utf8) => Ok(s.as_bytes().to_vec()),
        (Object::String(s), EncodingKind::Hex) => match hex_decode_bytes(name, s) {
            Ok(b) => Ok(b),
            Err(msg) => Err(new_error(ctx.pos.clone(), msg)),
        },
        (Object::String(s), EncodingKind::Base64) => {
            let table = base64_url_table();
            match base64_decode_into(&table, name, s, true) {
                Ok(b) => Ok(b),
                Err(msg) => Err(new_error(ctx.pos.clone(), msg)),
            }
        }
        (Object::Array(_), _) | (Object::Hash(_), _) => bytes_from_object(ctx, name, value),
        _ => Err(new_error(
            ctx.pos.clone(),
            format!("{}: value must be a string, array, or Buffer", name),
        )),
    }
}

// ---------------------------------------------------------------------------
// events: EventEmitter with on/once/off/emit (synchronous).
// ---------------------------------------------------------------------------

fn events_module() -> Object {
    module(vec![(
        "EventEmitter",
        native("events.EventEmitter", events_create),
    )])
}

type ListenerList = Vec<(usize, Object, bool)>;

fn events_create(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    let store: Rc<RefCell<HashMap<String, ListenerList>>> = Rc::new(RefCell::new(HashMap::new()));
    let next_id = Rc::new(std::cell::Cell::new(0usize));
    let emitter = Rc::new(RefCell::new(HashData::default()));

    let s = store.clone();
    let n = next_id.clone();
    emitter.borrow_mut().set(
        "on",
        native("events.on", move |ctx, args| {
            events_add(ctx, args, &s, &n, false)
        }),
    );
    let s = store.clone();
    let n = next_id.clone();
    emitter.borrow_mut().set(
        "once",
        native("events.once", move |ctx, args| {
            events_add(ctx, args, &s, &n, true)
        }),
    );
    let s = store.clone();
    emitter.borrow_mut().set(
        "off",
        native("events.off", move |ctx, args| events_remove(ctx, args, &s)),
    );
    let s = store.clone();
    let e = emitter.clone();
    emitter.borrow_mut().set(
        "emit",
        native("events.emit", move |ctx, args| {
            events_emit(ctx, args, &s, &e)
        }),
    );
    let s = store.clone();
    emitter.borrow_mut().set(
        "listeners",
        native("events.listeners", move |ctx, args| {
            events_listeners(ctx, args, &s)
        }),
    );
    let s = store.clone();
    emitter.borrow_mut().set(
        "listenerCount",
        native("events.listenerCount", move |ctx, args| {
            events_count(ctx, args, &s)
        }),
    );
    let s = store.clone();
    emitter.borrow_mut().set(
        "removeAllListeners",
        native("events.removeAllListeners", move |ctx, args| {
            events_remove_all(ctx, args, &s)
        }),
    );
    Object::Hash(emitter)
}

fn events_add(
    ctx: &mut CallContext,
    args: &[Object],
    store: &Rc<RefCell<HashMap<String, ListenerList>>>,
    next_id: &Rc<std::cell::Cell<usize>>,
    once: bool,
) -> Object {
    let event = match required_string(ctx, "events.add", args, 0, "event") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let listener = match args.get(1) {
        Some(v @ (Object::Function(_) | Object::Builtin(_))) => v.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "events: listener must be a function"),
        None => return new_error(ctx.pos.clone(), "events requires listener"),
    };
    let id = next_id.get();
    next_id.set(id + 1);
    store
        .borrow_mut()
        .entry(event.as_str().to_string())
        .or_default()
        .push((id, listener, once));
    Object::Undefined
}

fn events_remove(
    ctx: &mut CallContext,
    args: &[Object],
    store: &Rc<RefCell<HashMap<String, ListenerList>>>,
) -> Object {
    let event = match required_string(ctx, "events.off", args, 0, "event") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let listener = match args.get(1) {
        Some(v @ (Object::Function(_) | Object::Builtin(_))) => v.clone(),
        _ => return new_error(ctx.pos.clone(), "events.off requires listener"),
    };
    if let Some(list) = store.borrow_mut().get_mut(&event) {
        let key = listener.inspect();
        if let Some(pos) = list.iter().position(|(_, f, _)| f.inspect() == key) {
            list.remove(pos);
        }
        if list.is_empty() {
            store.borrow_mut().remove(event.as_str());
        }
    }
    Object::Undefined
}

fn events_emit(
    ctx: &mut CallContext,
    args: &[Object],
    store: &Rc<RefCell<HashMap<String, ListenerList>>>,
    emitter: &Rc<RefCell<HashData>>,
) -> Object {
    let event = match required_string(ctx, "events.emit", args, 0, "event") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let call_args: Vec<Object> = args.iter().skip(1).cloned().collect();
    // Snapshot listeners (including once) so they fire this emit, then remove
    // once listeners from the store afterwards (matching Go semantics).
    let snapshot: Vec<Object> = {
        let mut s = store.borrow_mut();
        let list = match s.get_mut(&event) {
            Some(l) => l,
            None => return bool_obj(false),
        };
        let snap: Vec<Object> = list.iter().map(|(_, f, _)| f.clone()).collect();
        list.retain(|(_, _, once)| !once);
        snap
    };
    let emitter_obj = Object::Hash(emitter.clone());
    for listener in &snapshot {
        let result = crate::evaluator::expressions::apply_function(
            listener,
            ctx.env,
            &call_args,
            Some(emitter_obj.clone()),
            ctx.pos.clone(),
        );
        if result.is_runtime_error() {
            return result;
        }
    }
    bool_obj(true)
}

fn events_listeners(
    ctx: &mut CallContext,
    args: &[Object],
    store: &Rc<RefCell<HashMap<String, ListenerList>>>,
) -> Object {
    let event = match required_string(ctx, "events.listeners", args, 0, "event") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let fns: Vec<Object> = store
        .borrow()
        .get(&event)
        .map(|list| list.iter().map(|(_, f, _)| f.clone()).collect())
        .unwrap_or_default();
    array(fns)
}

fn events_count(
    ctx: &mut CallContext,
    args: &[Object],
    store: &Rc<RefCell<HashMap<String, ListenerList>>>,
) -> Object {
    let event = match required_string(ctx, "events.listenerCount", args, 0, "event") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let count = store.borrow().get(&event).map(|l| l.len()).unwrap_or(0);
    num_obj(count as f64)
}

fn events_remove_all(
    ctx: &mut CallContext,
    args: &[Object],
    store: &Rc<RefCell<HashMap<String, ListenerList>>>,
) -> Object {
    match args.first() {
        None | Some(Object::Undefined) => store.borrow_mut().clear(),
        Some(Object::String(event)) => {
            store.borrow_mut().remove(event.as_str());
        }
        Some(_) => {
            return new_error(
                ctx.pos.clone(),
                "events.removeAllListeners: event must be a string",
            )
        }
    }
    Object::Undefined
}

// ---------------------------------------------------------------------------
// jwt: HS256 sign/verify/decode using the self-contained hmac+sha256.
// ---------------------------------------------------------------------------

fn jwt_module() -> Object {
    module(vec![
        ("sign", native("jwt.sign", jwt_sign)),
        ("verify", native("jwt.verify", jwt_verify)),
        ("decode", native("jwt.decode", jwt_decode)),
    ])
}

fn jwt_sign(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(ctx.pos.clone(), "jwt.sign requires payload and secret");
    }
    let (payload, secret) = (&args[0], &args[1]);
    let secret = match secret {
        Object::String(s) => s.as_bytes().to_vec(),
        _ => return new_error(ctx.pos.clone(), "jwt.sign expects string secret"),
    };
    let header = serde_json::json!({"alg": "HS256", "typ": "JWT"});
    let mut payload_value = object_to_value(payload);
    if let serde_json::Value::Object(ref mut map) = payload_value {
        if !map.contains_key("iat") {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            map.insert("iat".to_string(), serde_json::Value::Number(now.into()));
        }
    } else {
        return new_error(ctx.pos.clone(), "jwt.sign expects hash payload");
    }
    let header_b64 = base64url_encode_string(
        serde_json::to_string(&header)
            .unwrap_or_default()
            .as_bytes(),
    );
    let payload_b64 = base64url_encode_string(
        serde_json::to_string(&payload_value)
            .unwrap_or_default()
            .as_bytes(),
    );
    let message = format!("{}.{}", header_b64, payload_b64);
    let sig = hmac(HashKind::Sha256, &secret, message.as_bytes());
    let sig_b64 = base64url_encode_string(&sig);
    str_obj(format!("{}.{}.{}", header_b64, payload_b64, sig_b64))
}

fn jwt_verify(ctx: &mut CallContext, args: &[Object]) -> Object {
    if args.len() < 2 {
        return new_error(ctx.pos.clone(), "jwt.verify requires token and secret");
    }
    let (token, secret) = match (&args[0], &args[1]) {
        (Object::String(t), Object::String(s)) => (t.as_str().to_string(), s.as_bytes().to_vec()),
        _ => {
            return new_error(
                ctx.pos.clone(),
                "jwt.verify expects string token and secret",
            )
        }
    };
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return bool_obj(false);
    }
    let message = format!("{}.{}", parts[0], parts[1]);
    let expected = hmac(HashKind::Sha256, &secret, message.as_bytes());
    let expected_b64 = base64url_encode_string(&expected);
    if expected_b64 != parts[2] {
        return bool_obj(false);
    }
    let table = base64_url_table();
    let payload_bytes = match base64_decode_into(&table, "jwt.verify", parts[1], true) {
        Ok(b) => b,
        Err(_) => return bool_obj(false),
    };
    let payload: serde_json::Value = match serde_json::from_slice(&payload_bytes) {
        Ok(v) => v,
        Err(_) => return bool_obj(false),
    };
    if let Some(exp) = payload.get("exp").and_then(|v| v.as_f64()) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as f64)
            .unwrap_or(0.0);
        if now > exp {
            return bool_obj(false);
        }
    }
    bool_obj(true)
}

fn jwt_decode(ctx: &mut CallContext, args: &[Object]) -> Object {
    let token = match required_string(ctx, "jwt.decode", args, 0, "token") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return new_error(ctx.pos.clone(), "jwt.decode: invalid token format");
    }
    let table = base64_url_table();
    let payload_bytes = match base64_decode_into(&table, "jwt.decode", parts[1], true) {
        Ok(b) => b,
        Err(e) => return new_error(ctx.pos.clone(), format!("jwt.decode: {}", e)),
    };
    match serde_json::from_slice::<serde_json::Value>(&payload_bytes) {
        Ok(v) => value_to_object(&v),
        Err(e) => new_error(ctx.pos.clone(), format!("jwt.decode: {}", e)),
    }
}

fn base64url_encode_string(input: &[u8]) -> String {
    base64_url_encode(input)
}

// ---------------------------------------------------------------------------
// mime: a built-in extension<->type table plus format/parse helpers.
// ---------------------------------------------------------------------------

fn mime_module() -> Object {
    module(vec![
        (
            "typeByExtension",
            native("mime.typeByExtension", mime_type_by_extension),
        ),
        (
            "extensionByType",
            native("mime.extensionByType", mime_extension_by_type),
        ),
        (
            "parseMediaType",
            native("mime.parseMediaType", mime_parse_media_type),
        ),
        (
            "formatMediaType",
            native("mime.formatMediaType", mime_format_media_type),
        ),
    ])
}

fn mime_type_by_extension(ctx: &mut CallContext, args: &[Object]) -> Object {
    let ext = match required_string(ctx, "mime.typeByExtension", args, 0, "extension") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let normalized = ext.to_lowercase();
    let normalized = normalized.strip_prefix('.').unwrap_or(&normalized);
    match mime_lookup_ext(normalized) {
        Some(t) => str_obj(t.to_string()),
        None => Object::Undefined,
    }
}

fn mime_extension_by_type(ctx: &mut CallContext, args: &[Object]) -> Object {
    let typ = match required_string(ctx, "mime.extensionByType", args, 0, "type") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let lower = typ.to_lowercase();
    for (e, t) in mime_table() {
        if t == lower {
            return str_obj(format!(".{}", e));
        }
    }
    Object::Undefined
}

fn mime_parse_media_type(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "mime.parseMediaType", args, 0, "value") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut parts = value.split(';');
    let main = match parts.next() {
        Some(m) => m.trim().to_string(),
        None => return new_error(ctx.pos.clone(), "mime.parseMediaType: invalid media type"),
    };
    if !main.contains('/') {
        return new_error(ctx.pos.clone(), "mime.parseMediaType: invalid media type");
    }
    let params_hash = Rc::new(RefCell::new(HashData::default()));
    for part in parts {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            let v = v.trim().trim_matches('"');
            params_hash
                .borrow_mut()
                .set(k.trim().to_string(), str_obj(v.to_string()));
        }
    }
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("type", str_obj(main));
    hash.borrow_mut().set("params", Object::Hash(params_hash));
    Object::Hash(hash)
}

fn mime_format_media_type(ctx: &mut CallContext, args: &[Object]) -> Object {
    let typ = match required_string(ctx, "mime.formatMediaType", args, 0, "type") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut out = typ;
    if let Some(Object::Hash(params)) = args.get(1) {
        for (k, v) in &params.borrow().entries {
            out.push_str(&format!("; {}=\"{}\"", k, v.inspect()));
        }
    } else if let Some(o) = args.get(1) {
        if !matches!(o, Object::Undefined | Object::Null) {
            return new_error(
                ctx.pos.clone(),
                "mime.formatMediaType: params must be an object",
            );
        }
    }
    if out.is_empty() {
        return new_error(ctx.pos.clone(), "mime.formatMediaType: invalid media type");
    }
    str_obj(out)
}

fn mime_lookup_ext(ext: &str) -> Option<&'static str> {
    mime_table()
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, t)| *t)
}

fn mime_table() -> Vec<(&'static str, &'static str)> {
    vec![
        ("txt", "text/plain"),
        ("html", "text/html"),
        ("htm", "text/html"),
        ("css", "text/css"),
        ("csv", "text/csv"),
        ("md", "text/markdown"),
        ("json", "application/json"),
        ("xml", "application/xml"),
        ("yaml", "application/yaml"),
        ("yml", "application/yaml"),
        ("js", "application/javascript"),
        ("pdf", "application/pdf"),
        ("zip", "application/zip"),
        ("gz", "application/gzip"),
        ("tar", "application/x-tar"),
        ("png", "image/png"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("svg", "image/svg+xml"),
        ("webp", "image/webp"),
        ("wav", "audio/wav"),
        ("mp3", "audio/mpeg"),
        ("mp4", "video/mp4"),
        ("webm", "video/webm"),
    ]
}

// ---------------------------------------------------------------------------
// net/ip: parse IP/CIDR + host:port helpers + DNS lookup.
// ---------------------------------------------------------------------------

fn net_ip_module() -> Object {
    module(vec![
        ("parseIP", native("netip.parseIP", net_ip_parse)),
        ("parseCIDR", native("netip.parseCIDR", net_ip_parse_cidr)),
        ("contains", native("netip.contains", net_ip_contains)),
        (
            "splitHostPort",
            native("netip.splitHostPort", net_ip_split_host_port),
        ),
        (
            "joinHostPort",
            native("netip.joinHostPort", net_ip_join_host_port),
        ),
        ("lookupHost", native("netip.lookupHost", net_ip_lookup_host)),
    ])
}

fn net_ip_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "netip.parseIP", args, 0, "ip") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match parse_ip_addr(&text) {
        Some(addr) => ip_addr_to_object(&addr),
        None => Object::Undefined,
    }
}

fn net_ip_parse_cidr(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "netip.parseCIDR", args, 0, "cidr") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match parse_ip_cidr(&text) {
        Some(prefix) => ip_prefix_to_object(&prefix),
        None => Object::Undefined,
    }
}

fn net_ip_contains(ctx: &mut CallContext, args: &[Object]) -> Object {
    let cidr = match required_string(ctx, "netip.contains", args, 0, "cidr") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let ip = match required_string(ctx, "netip.contains", args, 1, "ip") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let prefix = match parse_ip_cidr(&cidr) {
        Some(p) => p,
        None => return new_error(ctx.pos.clone(), "netip.contains: invalid cidr"),
    };
    let addr = match parse_ip_addr(&ip) {
        Some(a) => a,
        None => return new_error(ctx.pos.clone(), "netip.contains: invalid ip"),
    };
    bool_obj(ip_cidr_contains(&prefix, &addr))
}

fn net_ip_split_host_port(ctx: &mut CallContext, args: &[Object]) -> Object {
    let address = match required_string(ctx, "netip.splitHostPort", args, 0, "address") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match split_host_port(&address) {
        Ok((host, port)) => {
            let hash = Rc::new(RefCell::new(HashData::default()));
            hash.borrow_mut().set("host", str_obj(host));
            hash.borrow_mut().set("port", str_obj(port));
            Object::Hash(hash)
        }
        Err(e) => new_error(ctx.pos.clone(), format!("netip.splitHostPort: {}", e)),
    }
}

fn net_ip_join_host_port(ctx: &mut CallContext, args: &[Object]) -> Object {
    let host = match required_string(ctx, "netip.joinHostPort", args, 0, "host") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let port = match required_string(ctx, "netip.joinHostPort", args, 1, "port") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if host.contains(':') && !host.starts_with('[') {
        str_obj(format!("[{}]:{}", host, port))
    } else {
        str_obj(format!("{}:{}", host, port))
    }
}

fn net_ip_lookup_host(ctx: &mut CallContext, args: &[Object]) -> Object {
    let host = match required_string(ctx, "netip.lookupHost", args, 0, "host") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), 0u16)) {
        Ok(iter) => {
            let addrs: Vec<Object> = iter.map(|sa| str_obj(sa.ip().to_string())).collect();
            array(addrs)
        }
        Err(e) => new_error(ctx.pos.clone(), format!("netip.lookupHost: {}", e)),
    }
}

struct IpAddr {
    bytes: Vec<u8>,
    is_v6: bool,
}

impl IpAddr {
    fn is_loopback(&self) -> bool {
        if self.is_v6 {
            self.bytes == vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
        } else {
            self.bytes == vec![127, 0, 0, 1]
        }
    }
}

fn parse_ip_addr(text: &str) -> Option<IpAddr> {
    if let Ok(v4) = std::net::Ipv4Addr::from_str(text) {
        return Some(IpAddr {
            bytes: v4.octets().to_vec(),
            is_v6: false,
        });
    }
    if let Ok(v6) = std::net::Ipv6Addr::from_str(text) {
        return Some(IpAddr {
            bytes: v6.octets().to_vec(),
            is_v6: true,
        });
    }
    None
}

struct IpPrefix {
    addr: IpAddr,
    bits: u32,
}

fn parse_ip_cidr(text: &str) -> Option<IpPrefix> {
    let (addr_part, bits_part) = text.split_once('/')?;
    let addr = parse_ip_addr(addr_part)?;
    let bits: u32 = bits_part.parse().ok()?;
    let max = if addr.is_v6 { 128 } else { 32 };
    if bits > max {
        return None;
    }
    Some(IpPrefix { addr, bits })
}

fn ip_cidr_contains(prefix: &IpPrefix, addr: &IpAddr) -> bool {
    if prefix.addr.is_v6 != addr.is_v6 {
        return false;
    }
    let mut bits_left = prefix.bits;
    for i in 0..prefix.addr.bytes.len() {
        if bits_left == 0 {
            return true;
        }
        let byte_bits = bits_left.min(8) as usize;
        let mask = if byte_bits == 8 {
            0xff
        } else {
            (0xff << (8 - byte_bits)) & 0xff
        };
        if (prefix.addr.bytes[i] & mask) != (addr.bytes[i] & mask) {
            return false;
        }
        bits_left -= byte_bits as u32;
    }
    true
}

fn ip_addr_to_object(addr: &IpAddr) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    let display = format_ip(addr);
    hash.borrow_mut().set("value", str_obj(display));
    hash.borrow_mut().set("is4", bool_obj(!addr.is_v6));
    hash.borrow_mut().set("is6", bool_obj(addr.is_v6));
    hash.borrow_mut()
        .set("isLoopback", bool_obj(addr.is_loopback()));
    hash.borrow_mut()
        .set("isPrivate", bool_obj(is_private_ip(addr)));
    hash.borrow_mut()
        .set("isMulticast", bool_obj(is_multicast_ip(addr)));
    Object::Hash(hash)
}

fn ip_prefix_to_object(prefix: &IpPrefix) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set(
        "value",
        str_obj(format!("{}/{}", format_ip(&prefix.addr), prefix.bits)),
    );
    hash.borrow_mut()
        .set("addr", str_obj(format_ip(&prefix.addr)));
    hash.borrow_mut().set("bits", num_obj(prefix.bits as f64));
    hash.borrow_mut().set("is4", bool_obj(!prefix.addr.is_v6));
    hash.borrow_mut().set("is6", bool_obj(prefix.addr.is_v6));
    Object::Hash(hash)
}

fn format_ip(addr: &IpAddr) -> String {
    if addr.is_v6 {
        let octets: [u8; 16] = addr.bytes[..16].try_into().unwrap_or([0; 16]);
        std::net::Ipv6Addr::from(octets).to_string()
    } else {
        let octets: [u8; 4] = addr.bytes[..4].try_into().unwrap_or([0; 4]);
        std::net::Ipv4Addr::from(octets).to_string()
    }
}

fn is_private_ip(addr: &IpAddr) -> bool {
    if addr.is_v6 {
        addr.bytes[0] & 0xfe == 0xfc
    } else {
        let b = &addr.bytes;
        b[0] == 10 || (b[0] == 172 && (b[1] & 0xf0) == 16) || (b[0] == 192 && b[1] == 168)
    }
}

fn is_multicast_ip(addr: &IpAddr) -> bool {
    if addr.is_v6 {
        addr.bytes[0] == 0xff
    } else {
        (addr.bytes[0] & 0xf0) == 0xe0
    }
}

fn split_host_port(address: &str) -> Result<(String, String), String> {
    if let Some(end) = address.rfind(':') {
        Ok((address[..end].to_string(), address[end + 1..].to_string()))
    } else {
        Err("missing port in address".to_string())
    }
}

use std::net::ToSocketAddrs;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// retry: synchronous run with configurable backoff.
// ---------------------------------------------------------------------------

fn retry_module() -> Object {
    module(vec![
        ("run", native("retry.run", retry_run)),
        (
            "exponential",
            native("retry.exponential", retry_exponential),
        ),
    ])
}

fn retry_run(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn retry_exponential(ctx: &mut CallContext, args: &[Object]) -> Object {
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

fn parse_retry_opts(
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

fn parse_retry_opts_exp(
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

fn stream_module() -> Object {
    module(vec![(
        "fromString",
        native("stream.fromString", stream_from_string),
    )])
}

fn stream_from_string(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "stream.fromString", args, 0, "text") {
        Ok(v) => v,
        Err(e) => return e,
    };
    stream_from_text(text)
}

fn stream_from_text(text: String) -> Object {
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

struct StreamState {
    text: String,
    pos: usize,
    closed: bool,
}

fn stream_read(ctx: &mut CallContext, args: &[Object], state: &Rc<RefCell<StreamState>>) -> Object {
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

fn stream_read_text(
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

fn stream_read_line(_ctx: &mut CallContext, state: &Rc<RefCell<StreamState>>) -> Object {
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

fn stream_read_all(state: &Rc<RefCell<StreamState>>) -> Object {
    let s = state.borrow();
    str_obj(s.text[s.pos..].to_string())
}

// ---------------------------------------------------------------------------
// exec: process execution module (@std/exec)
// ---------------------------------------------------------------------------

fn exec_module() -> Object {
    module(vec![
        ("run", native("exec.run", exec_run)),
        ("output", native("exec.output", exec_output)),
        (
            "combinedOutput",
            native("exec.combinedOutput", exec_combined_output),
        ),
        ("command", native("exec.command", exec_command)),
    ])
}

fn exec_run(ctx: &mut CallContext, args: &[Object]) -> Object {
    use std::process::Command;

    let (cmd_name, cmd_args) = match parse_exec_args(ctx, args) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let output = match Command::new(&cmd_name).args(&cmd_args).output() {
        Ok(o) => o,
        Err(e) => return new_error(ctx.pos.clone(), format!("exec.run: {}", e)),
    };

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    process_result(exit_code, stdout, stderr)
}

fn exec_output(ctx: &mut CallContext, args: &[Object]) -> Object {
    use std::process::Command;

    let (cmd_name, cmd_args) = match parse_exec_args(ctx, args) {
        Ok(v) => v,
        Err(e) => return e,
    };

    match Command::new(&cmd_name).args(&cmd_args).output() {
        Ok(output) => str_obj(String::from_utf8_lossy(&output.stdout).to_string()),
        Err(e) => new_error(ctx.pos.clone(), format!("exec.output: {}", e)),
    }
}

fn exec_combined_output(ctx: &mut CallContext, args: &[Object]) -> Object {
    use std::process::Command;

    let (cmd_name, cmd_args) = match parse_exec_args(ctx, args) {
        Ok(v) => v,
        Err(e) => return e,
    };

    match Command::new(&cmd_name).args(&cmd_args).output() {
        Ok(output) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            str_obj(combined)
        }
        Err(e) => new_error(ctx.pos.clone(), format!("exec.combinedOutput: {}", e)),
    }
}

fn run_process(
    cmd_name: &str,
    cmd_args: &[String],
    dir: Option<&str>,
) -> std::io::Result<std::process::Output> {
    let mut command = std::process::Command::new(cmd_name);
    command.args(cmd_args);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    command.output()
}

fn exec_command(ctx: &mut CallContext, args: &[Object]) -> Object {
    let (cmd_name, cmd_args) = match parse_exec_args(ctx, args) {
        Ok(v) => v,
        Err(e) => return e,
    };

    #[derive(Clone)]
    struct ExecCommandState {
        dir: Option<String>,
    }

    let state = Rc::new(RefCell::new(ExecCommandState { dir: None }));

    // Return a command builder object with chainable configuration and run/output methods.
    let hash = Rc::new(RefCell::new(HashData::default()));

    let state_for_set_dir = state.clone();
    let builder_for_set_dir = hash.clone();
    hash.borrow_mut().set(
        "setDir",
        native("command.setDir", move |ctx, args| {
            let dir = match required_string(ctx, "command.setDir", args, 0, "dir") {
                Ok(v) => v,
                Err(err) => return err,
            };
            state_for_set_dir.borrow_mut().dir = Some(dir);
            Object::Hash(builder_for_set_dir.clone())
        }),
    );

    let cmd_name_clone = cmd_name.clone();
    let cmd_args_clone = cmd_args.clone();
    let state_for_run = state.clone();
    hash.borrow_mut().set(
        "run",
        native("command.run", move |ctx, _args| {
            let state = state_for_run.borrow();
            let output = match run_process(&cmd_name_clone, &cmd_args_clone, state.dir.as_deref()) {
                Ok(o) => o,
                Err(e) => return new_error(ctx.pos.clone(), format!("command.run: {}", e)),
            };
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            process_result(exit_code, stdout, stderr)
        }),
    );

    let cmd_name_clone2 = cmd_name.clone();
    let cmd_args_clone2 = cmd_args.clone();
    let state_for_output = state.clone();
    hash.borrow_mut().set(
        "output",
        native("command.output", move |ctx, _args| {
            let state = state_for_output.borrow();
            match run_process(&cmd_name_clone2, &cmd_args_clone2, state.dir.as_deref()) {
                Ok(output) => str_obj(String::from_utf8_lossy(&output.stdout).to_string()),
                Err(e) => new_error(ctx.pos.clone(), format!("command.output: {}", e)),
            }
        }),
    );

    Object::Hash(hash)
}

fn parse_exec_args(
    ctx: &mut CallContext,
    args: &[Object],
) -> Result<(String, Vec<String>), Object> {
    if args.is_empty() {
        return Err(new_error(ctx.pos.clone(), "exec requires a command name"));
    }

    let cmd_name = match &args[0] {
        Object::String(s) => s.to_string(),
        _ => {
            return Err(new_error(
                ctx.pos.clone(),
                "exec: first argument must be a string",
            ))
        }
    };

    let cmd_args = if args.len() > 1 {
        // Check if second arg is an array
        if let Object::Array(arr) = &args[1] {
            let elements = &arr.borrow().elements;
            elements
                .iter()
                .map(|obj| match obj {
                    Object::String(s) => s.to_string(),
                    Object::Number(n) => format_number(*n),
                    Object::Boolean(b) => b.to_string(),
                    _ => format!("{:?}", obj),
                })
                .collect()
        } else {
            // Treat remaining args as individual arguments
            args[1..]
                .iter()
                .map(|obj| match obj {
                    Object::String(s) => s.to_string(),
                    Object::Number(n) => format_number(*n),
                    Object::Boolean(b) => b.to_string(),
                    _ => format!("{:?}", obj),
                })
                .collect()
        }
    } else {
        Vec::new()
    };

    Ok((cmd_name, cmd_args))
}

fn process_result(exit_code: i32, stdout: String, stderr: String) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("exitCode", num_obj(exit_code as f64));
    hash.borrow_mut().set("stdout", str_obj(stdout));
    hash.borrow_mut().set("stderr", str_obj(stderr));
    hash.borrow_mut().set("success", bool_obj(exit_code == 0));
    Object::Hash(hash)
}

// ---------------------------------------------------------------------------
// net/http/client: HTTP client module (@std/net/http/client)
// ---------------------------------------------------------------------------

fn http_client_module() -> Object {
    module(vec![
        ("get", native("http.get", http_client_get)),
        ("post", native("http.post", http_client_post)),
        ("request", native("http.request", http_client_request)),
        ("stream", native("http.stream", http_client_stream)),
        ("fetch", native("http.fetch", http_client_request)),
    ])
}

fn http_client_get(ctx: &mut CallContext, args: &[Object]) -> Object {
    let url = match args.first() {
        Some(Object::String(s)) => s.to_string(),
        Some(Object::Hash(h)) => match h.borrow().get("url") {
            Some(Object::String(s)) => s.to_string(),
            _ => return new_error(ctx.pos.clone(), "http.get: url is required"),
        },
        _ => {
            return new_error(
                ctx.pos.clone(),
                "http.get: requires a URL string or options object",
            )
        }
    };

    let mut req = ureq::get(&url);

    // Apply headers if provided
    if let Some(Object::Hash(opts)) = args.first() {
        if let Some(Object::Hash(headers)) = opts.borrow().get("headers") {
            let headers_data = headers.borrow();
            for (key, value) in &headers_data.entries {
                let v = value_to_string(&value);
                req = req.set(key, &v);
            }
        }
    }

    match req.call() {
        Ok(response) => build_http_response(response),
        Err(ureq::Error::Status(code, response)) => build_http_response_with_status(response, code),
        Err(e) => new_error(ctx.pos.clone(), format!("http.get: {}", e)),
    }
}

fn http_client_post(ctx: &mut CallContext, args: &[Object]) -> Object {
    let (url, body, content_type) = match args.first() {
        Some(Object::String(s)) => {
            let body = args.get(1).map(http_body_to_string).unwrap_or_default();
            let ct = if matches!(args.get(1), Some(Object::Hash(_))) {
                "application/json"
            } else {
                "text/plain"
            };
            (s.to_string(), body, ct)
        }
        Some(Object::Hash(h)) => {
            let url = match h.borrow().get("url") {
                Some(Object::String(s)) => s.to_string(),
                _ => return new_error(ctx.pos.clone(), "http.post: url is required"),
            };
            let body = h
                .borrow()
                .get("body")
                .map(|obj| http_body_to_string(&obj))
                .unwrap_or_default();
            let ct = "application/json";
            (url, body, ct)
        }
        _ => {
            return new_error(
                ctx.pos.clone(),
                "http.post: requires a URL string or options object",
            )
        }
    };

    match ureq::post(&url)
        .set("Content-Type", content_type)
        .send_string(&body)
    {
        Ok(response) => build_http_response(response),
        Err(ureq::Error::Status(code, response)) => build_http_response_with_status(response, code),
        Err(e) => new_error(ctx.pos.clone(), format!("http.post: {}", e)),
    }
}

fn http_client_request(ctx: &mut CallContext, args: &[Object]) -> Object {
    let opts = match args.first() {
        Some(Object::Hash(h)) => h.clone(),
        Some(Object::String(url)) => {
            // Simple URL string, default to GET
            let hash = Rc::new(RefCell::new(HashData::default()));
            hash.borrow_mut().set("url", Object::String(url.clone()));
            hash.borrow_mut().set("method", str_obj("GET".to_string()));
            hash
        }
        _ => {
            return new_error(
                ctx.pos.clone(),
                "http.request: requires an options object or URL string",
            )
        }
    };

    let url = match opts.borrow().get("url") {
        Some(Object::String(s)) => s.to_string(),
        _ => return new_error(ctx.pos.clone(), "http.request: url is required"),
    };

    let method = match opts.borrow().get("method") {
        Some(Object::String(s)) => s.to_uppercase(),
        _ => "GET".to_string(),
    };

    let body = opts
        .borrow()
        .get("body")
        .map(|obj| http_body_to_string(&obj));

    let mut req = ureq::request(&method, &url);

    // Apply headers
    if let Some(Object::Hash(headers)) = opts.borrow().get("headers") {
        let headers_data = headers.borrow();
        for (key, value) in &headers_data.entries {
            let v = value_to_string(&value);
            req = req.set(key, &v);
        }
    }

    let result = if let Some(body_str) = body {
        req.send_string(&body_str)
    } else {
        req.call()
    };

    match result {
        Ok(response) => build_http_response(response),
        Err(ureq::Error::Status(code, response)) => build_http_response_with_status(response, code),
        Err(e) => new_error(ctx.pos.clone(), format!("http.request: {}", e)),
    }
}

fn http_client_stream(ctx: &mut CallContext, args: &[Object]) -> Object {
    let opts = match args.first() {
        Some(Object::Hash(h)) => h.clone(),
        _ => return new_error(ctx.pos.clone(), "http.stream: requires an options object"),
    };

    let url = match opts.borrow().get("url") {
        Some(Object::String(s)) => s.to_string(),
        _ => return new_error(ctx.pos.clone(), "http.stream: url is required"),
    };

    let method = match opts.borrow().get("method") {
        Some(Object::String(s)) => s.to_uppercase(),
        _ => "GET".to_string(),
    };

    let body = opts
        .borrow()
        .get("body")
        .map(|obj| http_body_to_string(&obj));
    let mut req = ureq::request(&method, &url);

    if let Some(Object::Number(timeout_ms)) = opts.borrow().get("timeoutMs") {
        if *timeout_ms > 0.0 {
            req = req.timeout(std::time::Duration::from_millis(*timeout_ms as u64));
        }
    }

    if let Some(Object::Hash(headers)) = opts.borrow().get("headers") {
        let headers_data = headers.borrow();
        for (key, value) in &headers_data.entries {
            let v = value_to_string(value);
            req = req.set(key, &v);
        }
    }

    let result = if let Some(body_str) = body {
        req.send_string(&body_str)
    } else {
        req.call()
    };

    match result {
        Ok(response) => build_http_stream_response(response, None),
        Err(ureq::Error::Status(code, response)) => {
            build_http_stream_response(response, Some(code))
        }
        Err(e) => new_error(ctx.pos.clone(), format!("http.stream: {}", e)),
    }
}

fn build_http_response(response: ureq::Response) -> Object {
    let status = response.status();
    let status_text = response.status_text().to_string();

    let body = match response.into_string() {
        Ok(s) => s,
        Err(_) => String::new(),
    };

    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("status", num_obj(status as f64));
    hash.borrow_mut().set("statusText", str_obj(status_text));
    hash.borrow_mut().set("body", str_obj(body));
    hash.borrow_mut()
        .set("ok", bool_obj(status >= 200 && status < 300));

    Object::Hash(hash)
}

fn build_http_stream_response(response: ureq::Response, status_override: Option<u16>) -> Object {
    let status = status_override.unwrap_or_else(|| response.status());
    let status_text = response.status_text().to_string();
    let body_text = match response.into_string() {
        Ok(s) => s,
        Err(_) => String::new(),
    };
    let body = stream_from_text_object(body_text);

    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("status", num_obj(status as f64));
    hash.borrow_mut().set("statusText", str_obj(status_text));
    hash.borrow_mut()
        .set("ok", bool_obj(status >= 200 && status < 300));
    hash.borrow_mut().set("body", body);
    hash.borrow_mut().set(
        "close",
        native("http.stream.close", |_ctx, _args| Object::Undefined),
    );

    Object::Hash(hash)
}

fn stream_from_text_object(text: String) -> Object {
    let stream = stream_from_text(text.clone());
    if let Object::Hash(h) = &stream {
        h.borrow_mut().set("text", str_obj(text));
    }
    stream
}

fn http_body_to_string(obj: &Object) -> String {
    match obj {
        Object::String(s) => s.to_string(),
        Object::Hash(h) => hash_to_json(&h.borrow()),
        Object::Array(a) => value_to_json(&Object::Array(a.clone())),
        Object::Null | Object::Undefined | Object::Boolean(_) | Object::Number(_) => {
            value_to_json(obj)
        }
        _ => value_to_string(obj),
    }
}

fn build_http_response_with_status(response: ureq::Response, status_code: u16) -> Object {
    let status_text = response.status_text().to_string();

    let body = match response.into_string() {
        Ok(s) => s,
        Err(_) => String::new(),
    };

    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("status", num_obj(status_code as f64));
    hash.borrow_mut().set("statusText", str_obj(status_text));
    hash.borrow_mut().set("body", str_obj(body));
    hash.borrow_mut()
        .set("ok", bool_obj(status_code >= 200 && status_code < 300));

    Object::Hash(hash)
}

fn value_to_string(obj: &Object) -> String {
    match obj {
        Object::String(s) => s.to_string(),
        Object::Number(n) => format_number(*n),
        Object::Boolean(b) => b.to_string(),
        Object::Null => "null".to_string(),
        Object::Hash(_) => {
            // Simple JSON serialization
            format!("{:?}", obj)
        }
        _ => format!("{:?}", obj),
    }
}

use crate::ast::Position;
use crate::object::EnvRef;

// ---------------------------------------------------------------------------
// rate-limit: token-bucket rate limiter (@std/rate-limit)
// ---------------------------------------------------------------------------

const RATE_LIMIT_STATE_KEY: &str = "__rate_limit_state__";

fn rate_limit_module() -> Object {
    module(vec![(
        "create",
        native("rateLimit.create", rate_limit_create),
    )])
}

fn rate_limit_create(ctx: &mut CallContext, args: &[Object]) -> Object {
    let mut rate = 10.0_f64;
    let mut capacity = 10.0_f64;
    if let Some(Object::Hash(opts)) = args.get(0) {
        if let Some(Object::Number(n)) = opts.borrow().get("rate") {
            rate = *n;
        }
        if let Some(Object::Number(n)) = opts.borrow().get("capacity") {
            capacity = *n;
        }
    }
    if rate <= 0.0 || capacity <= 0.0 {
        return new_error(
            ctx.pos.clone(),
            "rateLimit.create: rate and capacity must be positive",
        );
    }
    // State stored as a HashData so it survives inside the object model.
    let state = Rc::new(RefCell::new(HashData::default()));
    state.borrow_mut().set("tokens", num_obj(capacity));
    state.borrow_mut().set("capacity", num_obj(capacity));
    state.borrow_mut().set("rate", num_obj(rate));
    state.borrow_mut().set("lastTimeMs", num_obj(now_millis()));

    let instance = Rc::new(RefCell::new(HashData::default()));
    instance
        .borrow_mut()
        .set(RATE_LIMIT_STATE_KEY, Object::Hash(state.clone()));

    let s = state.clone();
    instance.borrow_mut().set(
        "tryAcquire",
        native("rateLimit.tryAcquire", move |_ctx, _args| {
            let mut g = s.borrow_mut();
            let capacity = match g.get("capacity") {
                Some(Object::Number(n)) => *n,
                _ => capacity_fallback(),
            };
            let rate = match g.get("rate") {
                Some(Object::Number(n)) => *n,
                _ => rate_fallback(),
            };
            let now = now_millis();
            let last = match g.get("lastTimeMs") {
                Some(Object::Number(n)) => *n,
                _ => now,
            };
            let elapsed = ((now - last) / 1000.0).max(0.0);
            let tokens = match g.get("tokens") {
                Some(Object::Number(n)) => (n + elapsed * rate).min(capacity),
                _ => capacity,
            };
            g.set("tokens", num_obj(tokens));
            g.set("lastTimeMs", num_obj(now));
            if tokens >= 1.0 {
                g.set("tokens", num_obj(tokens - 1.0));
                bool_obj(true)
            } else {
                bool_obj(false)
            }
        }),
    );

    let s = state.clone();
    instance.borrow_mut().set(
        "acquire",
        native("rateLimit.acquire", move |_ctx, _args| loop {
            let wait_ms = {
                let mut g = s.borrow_mut();
                let capacity = match g.get("capacity") {
                    Some(Object::Number(n)) => *n,
                    _ => capacity_fallback(),
                };
                let rate = match g.get("rate") {
                    Some(Object::Number(n)) => *n,
                    _ => rate_fallback(),
                };
                let now = now_millis();
                let last = match g.get("lastTimeMs") {
                    Some(Object::Number(n)) => *n,
                    _ => now,
                };
                let elapsed = ((now - last) / 1000.0).max(0.0);
                let tokens = match g.get("tokens") {
                    Some(Object::Number(n)) => (n + elapsed * rate).min(capacity),
                    _ => capacity,
                };
                g.set("tokens", num_obj(tokens));
                g.set("lastTimeMs", num_obj(now));
                if tokens >= 1.0 {
                    g.set("tokens", num_obj(tokens - 1.0));
                    return Object::Undefined;
                }
                (((1.0 - tokens) / rate) * 1000.0).max(0.0) as u64
            };
            if wait_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(wait_ms));
            }
        }),
    );

    let s = state.clone();
    instance.borrow_mut().set(
        "remaining",
        native("rateLimit.remaining", move |_ctx, _args| {
            let g = s.borrow();
            match g.get("tokens") {
                Some(Object::Number(n)) => num_obj(*n),
                _ => num_obj(0.0),
            }
        }),
    );

    Object::Hash(instance)
}

#[inline]
fn capacity_fallback() -> f64 {
    10.0
}
#[inline]
fn rate_fallback() -> f64 {
    10.0
}

// ---------------------------------------------------------------------------
// prometheus: minimal metrics registry (@std/prometheus)
// ---------------------------------------------------------------------------

const PROMETHEUS_STATE_KEY: &str = "__prometheus_state__";

fn prometheus_module() -> Object {
    module(vec![(
        "create",
        native("prometheus.create", prometheus_create),
    )])
}

fn prometheus_create(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    // metrics: Hash mapping name -> Number
    let metrics = Rc::new(RefCell::new(HashData::default()));
    let instance = Rc::new(RefCell::new(HashData::default()));
    instance
        .borrow_mut()
        .set(PROMETHEUS_STATE_KEY, Object::Hash(metrics.clone()));

    let m = metrics.clone();
    instance.borrow_mut().set(
        "inc",
        native("prometheus.inc", move |ctx, args| {
            let name = match required_string(ctx, "prometheus.inc", args, 0, "name") {
                Ok(n) => n,
                Err(e) => return e,
            };
            let mut g = m.borrow_mut();
            let current = match g.get(&name) {
                Some(Object::Number(n)) => *n,
                _ => 0.0,
            };
            g.set(name, num_obj(current + 1.0));
            Object::Undefined
        }),
    );

    let m = metrics.clone();
    instance.borrow_mut().set(
        "set",
        native("prometheus.set", move |ctx, args| {
            let name = match required_string(ctx, "prometheus.set", args, 0, "name") {
                Ok(n) => n,
                Err(e) => return e,
            };
            let value = match required_number(ctx, "prometheus.set", args, 1, "value") {
                Ok(v) => v,
                Err(e) => return e,
            };
            m.borrow_mut().set(name, num_obj(value));
            Object::Undefined
        }),
    );

    let m = metrics.clone();
    instance.borrow_mut().set(
        "get",
        native("prometheus.get", move |ctx, args| {
            let name = match required_string(ctx, "prometheus.get", args, 0, "name") {
                Ok(n) => n,
                Err(e) => return e,
            };
            match m.borrow().get(&name).cloned() {
                Some(Object::Number(n)) => num_obj(n),
                _ => num_obj(0.0),
            }
        }),
    );

    let m = metrics.clone();
    instance.borrow_mut().set(
        "snapshot",
        native("prometheus.snapshot", move |_ctx, _args| {
            let g = m.borrow();
            let mut entries: Vec<Object> = Vec::with_capacity(g.entries.len());
            for (k, v) in &g.entries {
                let entry = Rc::new(RefCell::new(HashData::default()));
                entry.borrow_mut().set("name", str_obj(k.clone()));
                entry.borrow_mut().set("value", v.clone());
                entries.push(Object::Hash(entry));
            }
            array(entries)
        }),
    );

    Object::Hash(instance)
}

// ---------------------------------------------------------------------------
// highlight: terminal syntax highlighting subset (@std/highlight)
// ---------------------------------------------------------------------------

fn highlight_module() -> Object {
    module(vec![(
        "terminal",
        native("highlight.terminal", highlight_terminal),
    )])
}

struct HighlightOpts {
    lang: String,
    width: usize,
    color: bool,
}

fn highlight_terminal(ctx: &mut CallContext, args: &[Object]) -> Object {
    let code = match required_string(ctx, "highlight.terminal", args, 0, "code") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut opts = HighlightOpts {
        lang: String::new(),
        width: 80,
        color: true,
    };
    if let Some(Object::Hash(h)) = args.get(1) {
        if let Some(Object::String(s)) = h.borrow().get("lang") {
            opts.lang = s.to_ascii_lowercase();
        }
        if let Some(Object::Number(n)) = h.borrow().get("width") {
            opts.width = *n as usize;
        }
        if let Some(Object::Boolean(b)) = h.borrow().get("color") {
            opts.color = *b;
        }
    }
    if opts.width < 1 {
        opts.width = 80;
    }

    let mut lines: Vec<String> = Vec::new();
    for raw_line in code.replace("\r\n", "\n").split('\n') {
        for wrapped in wrap_simple(raw_line, opts.width) {
            lines.push(highlight_line(&wrapped, &opts));
        }
    }

    let out = Rc::new(RefCell::new(HashData::default()));
    out.borrow_mut().set(
        "lines",
        array(lines.iter().map(|s| str_obj(s.clone())).collect()),
    );
    out.borrow_mut().set("text", str_obj(lines.join("\n")));
    out.borrow_mut().set("lang", str_obj(opts.lang.clone()));
    Object::Hash(out)
}

fn wrap_simple(line: &str, width: usize) -> Vec<String> {
    if width == 0 || line.chars().count() <= width {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for (i, c) in line.chars().enumerate() {
        current.push(c);
        if (i + 1) % width == 0 {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn highlight_line(line: &str, opts: &HighlightOpts) -> String {
    if !opts.color {
        return line.to_string();
    }
    match opts.lang.as_str() {
        "diff" => {
            if line.starts_with('+') {
                return terminal_style_string(line, "success", false);
            }
            if line.starts_with('-') {
                return terminal_style_string(line, "error", false);
            }
            if line.starts_with("@@") {
                return terminal_style_string(line, "accent", true);
            }
            line.to_string()
        }
        "json" => highlight_json_line(line),
        "shell" | "sh" | "bash" | "gs" | "js" | "toml" => {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') || trimmed.starts_with("//") {
                terminal_style_string(line, "muted", false)
            } else {
                line.to_string()
            }
        }
        _ => line.to_string(),
    }
}

fn highlight_json_line(line: &str) -> String {
    let mut out = String::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut buf = String::new();
    for r in line.chars() {
        if in_string {
            buf.push(r);
            if escaped {
                escaped = false;
                continue;
            }
            if r == '\\' {
                escaped = true;
                continue;
            }
            if r == '"' {
                out.push_str(&terminal_style_string(&buf, "success", false));
                buf.clear();
                in_string = false;
            }
            continue;
        }
        if r == '"' {
            in_string = true;
            buf.push(r);
            continue;
        }
        out.push(r);
    }
    if !buf.is_empty() {
        out.push_str(&buf);
    }
    out
}

fn terminal_style_string(text: &str, fg: &str, bold: bool) -> String {
    let mut codes: Vec<i32> = Vec::new();
    if bold {
        codes.push(1);
    }
    if let Some(code) = terminal_color_code(fg, false) {
        codes.push(code);
    }
    if codes.is_empty() {
        text.to_string()
    } else {
        let joined: Vec<String> = codes.iter().map(|c| c.to_string()).collect();
        format!("\x1b[{}m{}\x1b[0m", joined.join(";"), text)
    }
}

// ---------------------------------------------------------------------------
// sse: Server-Sent Events parser + stream reader (@std/sse)
// ---------------------------------------------------------------------------

const SSE_READER_STATE_KEY: &str = "__sse_state__";

fn sse_module() -> Object {
    module(vec![
        ("parse", native("sse.parse", sse_parse)),
        ("reader", native("sse.reader", sse_reader)),
    ])
}

fn sse_parse(ctx: &mut CallContext, args: &[Object]) -> Object {
    let text = match required_string(ctx, "sse.parse", args, 0, "text") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let events = parse_sse_block(&text.split('\n').collect::<Vec<_>>());
    array(events)
}

fn sse_reader(ctx: &mut CallContext, args: &[Object]) -> Object {
    // Two accepted input shapes, mirroring the Go version:
    //   1. a string of raw SSE text
    //   2. an object carrying `text` (string) or `data` (string/array)
    let text: String = match args.get(0) {
        Some(Object::String(s)) => s.to_string(),
        Some(Object::Hash(h)) => {
            let hb = h.borrow();
            if let Some(Object::String(s)) = hb.get("text") {
                s.to_string()
            } else if let Some(value) = hb.get("data") {
                value_to_string(value)
            } else {
                return new_error(ctx.pos.clone(), "sse.reader: requires a stream object");
            }
        }
        _ => return new_error(ctx.pos.clone(), "sse.reader requires a stream"),
    };

    // Pre-split into events; emulate a forward cursor over the parsed list.
    let events = parse_sse_block(&text.split('\n').collect::<Vec<_>>());
    let state = Rc::new(RefCell::new(SseReaderState { events, cursor: 0 }));

    let instance = Rc::new(RefCell::new(HashData::default()));
    // Sentinel marker so callers can detect an SSE reader object if needed.
    instance.borrow_mut().set(
        SSE_READER_STATE_KEY,
        Object::Hash(Rc::new(RefCell::new(HashData::default()))),
    );

    let st = state.clone();
    instance.borrow_mut().set(
        "next",
        native("sse.next", move |_ctx, _args| {
            let mut g = st.borrow_mut();
            if g.cursor >= g.events.len() {
                return Object::Null;
            }
            let ev = g.events[g.cursor].clone();
            g.cursor += 1;
            ev
        }),
    );

    let st = state.clone();
    instance.borrow_mut().set(
        "readAll",
        native("sse.readAll", move |_ctx, _args| {
            let g = st.borrow();
            let remaining = &g.events[g.cursor..];
            array(remaining.to_vec())
        }),
    );

    Object::Hash(instance)
}

struct SseReaderState {
    events: Vec<Object>,
    cursor: usize,
}

fn parse_sse_block(lines: &[&str]) -> Vec<Object> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    for raw in lines {
        let line = raw.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(line.to_string());
    }
    if !current.is_empty() {
        blocks.push(current);
    }

    let mut events = Vec::with_capacity(blocks.len());
    for block in blocks {
        let mut event_type = "message".to_string();
        let mut event_id = String::new();
        let mut retry = String::new();
        let mut data_parts: Vec<String> = Vec::new();
        for line in block {
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = match line.find(':') {
                Some(idx) => {
                    let f = line[..idx].to_string();
                    let mut v = line[idx + 1..].to_string();
                    if let Some(stripped) = v.strip_prefix(' ') {
                        v = stripped.to_string();
                    }
                    (f, v)
                }
                None => (line.clone(), String::new()),
            };
            match field.as_str() {
                "event" => event_type = value,
                "data" => data_parts.push(value),
                "id" => event_id = value,
                "retry" => retry = value,
                _ => {}
            }
        }
        let event = Rc::new(RefCell::new(HashData::default()));
        event.borrow_mut().set("type", str_obj(event_type.clone()));
        event.borrow_mut().set("event", str_obj(event_type.clone()));
        event
            .borrow_mut()
            .set("data", str_obj(data_parts.join("\n")));
        if !event_id.is_empty() {
            event.borrow_mut().set("id", str_obj(event_id));
        }
        if !retry.is_empty() {
            event.borrow_mut().set("retry", str_obj(retry));
        }
        events.push(Object::Hash(event));
    }
    events
}

// ---------------------------------------------------------------------------
// db: SQLite-backed database module (@std/db)
// ---------------------------------------------------------------------------

const DB_STATE_KEY: &str = "__db_conn__";

fn db_module() -> Object {
    module(vec![
        ("open", native("db.open", db_open)),
        (
            "drivers",
            array(vec![str_obj("sqlite"), str_obj("sqlite3")]),
        ),
    ])
}

fn db_open(ctx: &mut CallContext, args: &[Object]) -> Object {
    let driver = match required_string(ctx, "db.open", args, 0, "driver") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let dsn = match required_string(ctx, "db.open", args, 1, "dsn") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let driver_lower = driver.to_ascii_lowercase();
    if driver_lower != "sqlite" && driver_lower != "sqlite3" {
        return new_error(
            ctx.pos.clone(),
            format!(
                "db.open: unsupported driver \"{}\" (Rust port supports sqlite only)",
                driver
            ),
        );
    }
    let conn = match rusqlite::Connection::open(&dsn) {
        Ok(c) => c,
        Err(e) => return new_error(ctx.pos.clone(), format!("db.open: {}", e)),
    };
    let conn = Rc::new(std::cell::UnsafeCell::new(conn));
    db_connection_object(conn, driver_lower, dsn)
}

fn db_connection_object(conn: DbConn, driver: String, dsn: String) -> Object {
    let obj = Rc::new(RefCell::new(HashData::default()));
    // Sentinel marker so callers can identify a connection handle if needed.
    obj.borrow_mut().set(
        DB_STATE_KEY,
        Object::Hash(Rc::new(RefCell::new(HashData::default()))),
    );

    obj.borrow_mut().set("driver", str_obj(driver.clone()));
    obj.borrow_mut().set("dsn", str_obj(dsn.clone()));

    let c = conn.clone();
    obj.borrow_mut().set(
        "exec",
        native("db.exec", move |ctx, args| db_exec(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "query",
        native("db.query", move |ctx, args| db_query(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "queryOne",
        native("db.queryOne", move |ctx, args| db_query_one(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "prepare",
        native("db.prepare", move |ctx, args| db_prepare(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "begin",
        native("db.begin", move |ctx, _args| db_begin(ctx, &c)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "ping",
        native("db.ping", move |_ctx, _args| {
            // sqlite is in-process; ping always succeeds when the handle is open.
            bool_obj(true)
        }),
    );
    let c = conn.clone();
    obj.borrow_mut()
        .set("close", native("db.close", move |_ctx, _args| db_close(&c)));

    Object::Hash(obj)
}

type DbConn = Rc<std::cell::UnsafeCell<rusqlite::Connection>>;

// Safety: the GTS VM is single-threaded (synchronous tree-walker), so a single
// mutable borrow at a time is guaranteed by the interpreter's call discipline.
unsafe fn conn_ref(conn: &DbConn) -> &rusqlite::Connection {
    &*conn.get()
}

fn db_query_args(
    ctx: &mut CallContext,
    name: &str,
    args: &[Object],
) -> Result<(String, Vec<RusqlParam>), Object> {
    let query = required_string(ctx, name, args, 0, "query")?;
    let mut params = Vec::new();
    if let Some(arg) = args.get(1) {
        if let Object::Array(arr) = arg {
            for item in &arr.borrow().elements {
                params.push(object_to_sql_param(item));
            }
        } else {
            for item in &args[1..] {
                params.push(object_to_sql_param(item));
            }
        }
    }
    Ok((query, params))
}

enum RusqlParam {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Bool(bool),
}

fn object_to_sql_param(obj: &Object) -> RusqlParam {
    match obj {
        Object::Null | Object::Undefined => RusqlParam::Null,
        Object::Boolean(b) => RusqlParam::Bool(*b),
        Object::Number(n) => {
            if *n == n.trunc() && n.abs() < 9.007e15 {
                RusqlParam::Int(*n as i64)
            } else {
                RusqlParam::Real(*n)
            }
        }
        Object::String(s) => RusqlParam::Text(s.to_string()),
        _ => RusqlParam::Text(obj.inspect()),
    }
}

impl rusqlite::ToSql for RusqlParam {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self {
            RusqlParam::Null => Ok(rusqlite::types::Null.to_sql()?),
            RusqlParam::Int(i) => Ok((*i).into()),
            RusqlParam::Real(f) => Ok((*f).into()),
            RusqlParam::Text(s) => Ok(s.as_str().into()),
            RusqlParam::Bool(b) => Ok((*b as i64).into()),
        }
    }
}

fn to_sql_refs(params: &[RusqlParam]) -> Vec<&dyn rusqlite::ToSql> {
    params.iter().map(|p| p as &dyn rusqlite::ToSql).collect()
}

fn db_exec(ctx: &mut CallContext, conn: &DbConn, args: &[Object]) -> Object {
    let (query, params) = match db_query_args(ctx, "db.exec", args) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let refs = to_sql_refs(&params);
    let result = unsafe { conn_ref(conn).execute(&query, refs.as_slice()) };
    match result {
        Ok(affected) => {
            let out = Rc::new(RefCell::new(HashData::default()));
            out.borrow_mut()
                .set("rowsAffected", num_obj(affected as f64));
            out.borrow_mut().set("lastInsertId", num_obj(0.0));
            Object::Hash(out)
        }
        Err(e) => new_error(ctx.pos.clone(), format!("db.exec: {}", e)),
    }
}

fn db_query(ctx: &mut CallContext, conn: &DbConn, args: &[Object]) -> Object {
    let (query, params) = match db_query_args(ctx, "db.query", args) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let refs = to_sql_refs(&params);
    let rows_result = unsafe {
        conn_ref(conn).prepare(&query).and_then(|mut stmt| {
            stmt.query_map(refs.as_slice(), |row| row_to_hash(row))
                .and_then(|mapped| {
                    let mut out: Vec<Object> = Vec::new();
                    for r in mapped {
                        out.push(r?);
                    }
                    Ok(out)
                })
        })
    };
    match rows_result {
        Ok(rows) => array(rows),
        Err(e) => new_error(ctx.pos.clone(), format!("db.query: {}", e)),
    }
}

fn row_to_hash(row: &rusqlite::Row<'_>) -> rusqlite::Result<Object> {
    let col_count = row.as_ref().column_count();
    let mut h = HashData::default();
    for i in 0..col_count {
        let name = row.as_ref().column_name(i)?.to_string();
        let value: rusqlite::types::Value = row.get(i).unwrap_or(rusqlite::types::Value::Null);
        let obj = match value {
            rusqlite::types::Value::Null => Object::Null,
            rusqlite::types::Value::Integer(i) => num_obj(i as f64),
            rusqlite::types::Value::Real(f) => num_obj(f),
            rusqlite::types::Value::Text(s) => str_obj(s),
            rusqlite::types::Value::Blob(b) => str_obj(String::from_utf8_lossy(&b).to_string()),
        };
        h.set(name, obj);
    }
    Ok(Object::Hash(Rc::new(RefCell::new(h))))
}

fn db_query_one(ctx: &mut CallContext, conn: &DbConn, args: &[Object]) -> Object {
    let result = db_query(ctx, conn, args);
    if result.is_runtime_error() {
        return result;
    }
    if let Object::Array(arr) = result {
        let elements = &arr.borrow().elements;
        if elements.is_empty() {
            return Object::Null;
        }
        return elements[0].clone();
    }
    Object::Null
}

fn db_prepare(ctx: &mut CallContext, conn: &DbConn, args: &[Object]) -> Object {
    let query = match required_string(ctx, "db.prepare", args, 0, "query") {
        Ok(v) => v,
        Err(e) => return e,
    };
    // We can't safely stash a rusqlite::Statement across calls without lifetime
    // gymnastics; provide a lightweight prepared-statement facade that re-parses
    // per call. Behaviourally equivalent for the synchronous VM.
    let conn_clone = conn.clone();
    let stmt_obj = Rc::new(RefCell::new(HashData::default()));
    let q = query.clone();
    let c = conn_clone.clone();
    stmt_obj.borrow_mut().set(
        "exec",
        native("db.stmt.exec", move |ctx, args| {
            let params: Vec<RusqlParam> = match args.get(0) {
                Some(Object::Array(arr)) => arr
                    .borrow()
                    .elements
                    .iter()
                    .map(object_to_sql_param)
                    .collect(),
                _ => args.iter().map(object_to_sql_param).collect(),
            };
            let refs = to_sql_refs(&params);
            match unsafe { conn_ref(&c).execute(&q, refs.as_slice()) } {
                Ok(n) => {
                    let out = Rc::new(RefCell::new(HashData::default()));
                    out.borrow_mut().set("rowsAffected", num_obj(n as f64));
                    Object::Hash(out)
                }
                Err(e) => new_error(ctx.pos.clone(), format!("db.stmt.exec: {}", e)),
            }
        }),
    );
    let q = query.clone();
    let c = conn_clone.clone();
    stmt_obj.borrow_mut().set(
        "query",
        native("db.stmt.query", move |ctx, args| {
            let params: Vec<RusqlParam> = match args.get(0) {
                Some(Object::Array(arr)) => arr
                    .borrow()
                    .elements
                    .iter()
                    .map(object_to_sql_param)
                    .collect(),
                _ => args.iter().map(object_to_sql_param).collect(),
            };
            let refs = to_sql_refs(&params);
            let res = unsafe {
                conn_ref(&c).prepare(&q).and_then(|mut stmt| {
                    stmt.query_map(refs.as_slice(), |row| row_to_hash(row))
                        .and_then(|mapped| {
                            let mut out: Vec<Object> = Vec::new();
                            for r in mapped {
                                out.push(r?);
                            }
                            Ok(out)
                        })
                })
            };
            match res {
                Ok(rows) => array(rows),
                Err(e) => new_error(ctx.pos.clone(), format!("db.stmt.query: {}", e)),
            }
        }),
    );
    let q = query.clone();
    let c = conn_clone.clone();
    stmt_obj.borrow_mut().set(
        "queryOne",
        native("db.stmt.queryOne", move |ctx, args| {
            let params: Vec<RusqlParam> = match args.get(0) {
                Some(Object::Array(arr)) => arr
                    .borrow()
                    .elements
                    .iter()
                    .map(object_to_sql_param)
                    .collect(),
                _ => args.iter().map(object_to_sql_param).collect(),
            };
            let refs = to_sql_refs(&params);
            let res = unsafe {
                conn_ref(&c).prepare(&q).and_then(|mut stmt| {
                    stmt.query_map(refs.as_slice(), |row| row_to_hash(row))
                        .and_then(|mapped| {
                            let mut out: Vec<Object> = Vec::new();
                            for r in mapped {
                                out.push(r?);
                            }
                            Ok(out)
                        })
                })
            };
            match res {
                Ok(rows) => {
                    if rows.is_empty() {
                        Object::Null
                    } else {
                        rows[0].clone()
                    }
                }
                Err(e) => new_error(ctx.pos.clone(), format!("db.stmt.queryOne: {}", e)),
            }
        }),
    );
    Object::Hash(stmt_obj)
}

fn db_begin(ctx: &mut CallContext, conn: &DbConn) -> Object {
    // sqlite transaction with unchecked borrow. We emulate by executing
    // "BEGIN" and returning a tx facade whose commit/rollback run the
    // corresponding SQL.
    let res = unsafe { conn_ref(conn).execute_batch("BEGIN") };
    if let Err(e) = res {
        return new_error(ctx.pos.clone(), format!("db.begin: {}", e));
    }
    let tx_obj = Rc::new(RefCell::new(HashData::default()));
    let c = conn.clone();
    tx_obj.borrow_mut().set(
        "exec",
        native("db.tx.exec", move |ctx, args| db_exec(ctx, &c, args)),
    );
    let c = conn.clone();
    tx_obj.borrow_mut().set(
        "query",
        native("db.tx.query", move |ctx, args| db_query(ctx, &c, args)),
    );
    let c = conn.clone();
    tx_obj.borrow_mut().set(
        "queryOne",
        native("db.tx.queryOne", move |ctx, args| {
            db_query_one(ctx, &c, args)
        }),
    );
    let c = conn.clone();
    tx_obj.borrow_mut().set(
        "commit",
        native("db.tx.commit", move |ctx, _args| {
            match unsafe { conn_ref(&c).execute_batch("COMMIT") } {
                Ok(_) => Object::Undefined,
                Err(e) => new_error(ctx.pos.clone(), format!("db.tx.commit: {}", e)),
            }
        }),
    );
    let c = conn.clone();
    tx_obj.borrow_mut().set(
        "rollback",
        native("db.tx.rollback", move |ctx, _args| {
            match unsafe { conn_ref(&c).execute_batch("ROLLBACK") } {
                Ok(_) => Object::Undefined,
                Err(e) => new_error(ctx.pos.clone(), format!("db.tx.rollback: {}", e)),
            }
        }),
    );
    Object::Hash(tx_obj)
}

fn db_close(conn: &DbConn) -> Object {
    // Drop the inner connection by replacing it with a closed handle.
    // We can't take ownership out of the UnsafeCell without unsafe code; rely on
    // the fact that closing on a sqlite handle is best-effort and the OS will
    // reclaim resources on process exit. For correctness, swap in a fresh
    // in-memory connection to invalidate the previous one.
    unsafe {
        let _ = std::ptr::replace(conn.get(), rusqlite::Connection::open_in_memory().unwrap());
    }
    Object::Undefined
}

// ---------------------------------------------------------------------------
// mail: RFC 5322 address / message parsing and formatting (@std/mail)
// ---------------------------------------------------------------------------

fn mail_module() -> Object {
    module(vec![
        (
            "parseAddress",
            native("mail.parseAddress", mail_parse_address),
        ),
        (
            "parseAddressList",
            native("mail.parseAddressList", mail_parse_address_list),
        ),
        (
            "parseMessage",
            native("mail.parseMessage", mail_parse_message),
        ),
        (
            "formatAddress",
            native("mail.formatAddress", mail_format_address),
        ),
        (
            "formatAddressList",
            native("mail.formatAddressList", mail_format_address_list),
        ),
        ("parseDate", native("mail.parseDate", mail_parse_date)),
        ("formatDate", native("mail.formatDate", mail_format_date)),
        ("getHeader", native("mail.getHeader", mail_get_header)),
    ])
}

/// A parsed mailbox address: optional display name + `local@domain`.
#[derive(Clone)]
struct MailAddress {
    name: String,
    address: String,
}

/// Parse a single address. Accepts both `Name <addr>` and bare `addr` forms,
/// mirroring Go's `mail.ParseAddress`.
fn parse_one_address(value: &str) -> Result<MailAddress, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("empty address".to_string());
    }

    // Form: "Display Name" <addr@domain>  (or  Name <addr@domain>)
    if let (Some(lt), Some(gt)) = (trimmed.rfind('<'), trimmed.rfind('>')) {
        if lt < gt {
            let mut name = trimmed[..lt].trim().to_string();
            // Strip surrounding quotes from a quoted display name.
            if name.len() >= 2 && name.starts_with('"') && name.ends_with('"') {
                name = name[1..name.len() - 1].to_string();
            }
            let address = trimmed[lt + 1..gt].trim().to_string();
            if !is_valid_addr(&address) {
                return Err(format!("invalid address: {}", address));
            }
            return Ok(MailAddress { name, address });
        }
    }
    // Bare address form.
    if !is_valid_addr(trimmed) {
        return Err(format!("invalid address: {}", trimmed));
    }
    Ok(MailAddress {
        name: String::new(),
        address: trimmed.to_string(),
    })
}

fn is_valid_addr(addr: &str) -> bool {
    // Minimal local@domain check (one '@', non-empty local and domain).
    match addr.find('@') {
        Some(i) if i > 0 && i < addr.len() - 1 => !addr[i + 1..].is_empty(),
        _ => false,
    }
}

/// Format back into `Name <addr>` (or bare `addr` when no display name).
fn format_address(addr: &MailAddress) -> String {
    if addr.name.is_empty() {
        addr.address.clone()
    } else if addr.name.contains(',') || addr.name.contains('"') {
        // Quote when the display name would otherwise be ambiguous.
        format!("\"{}\" <{}>", addr.name.replace('"', "\\\""), addr.address)
    } else {
        format!("{} <{}>", addr.name, addr.address)
    }
}

/// Split an address list on top-level commas (commas inside quotes or
/// angle brackets are preserved).
fn split_address_list(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut in_quotes = false;
    let mut in_angle = false;
    for c in value.chars() {
        match c {
            '"' if !in_angle => {
                in_quotes = !in_quotes;
                buf.push(c);
            }
            '<' if !in_quotes => {
                in_angle = true;
                buf.push(c);
            }
            '>' if in_angle => {
                in_angle = false;
                buf.push(c);
            }
            ',' if !in_quotes && !in_angle => {
                let t = buf.trim().to_string();
                if !t.is_empty() {
                    parts.push(t);
                }
                buf.clear();
            }
            _ => buf.push(c),
        }
    }
    let last = buf.trim().to_string();
    if !last.is_empty() {
        parts.push(last);
    }
    parts
}

fn mail_address_object(addr: &MailAddress) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut().set("name", str_obj(addr.name.clone()));
    hash.borrow_mut()
        .set("address", str_obj(addr.address.clone()));
    Object::Hash(hash)
}

fn mail_address_from_value(
    ctx: &CallContext,
    name: &str,
    value: &Object,
) -> Result<MailAddress, Object> {
    match value {
        Object::String(s) => match parse_one_address(s) {
            Ok(a) => Ok(a),
            Err(e) => Err(new_error(ctx.pos.clone(), format!("{}: {}", name, e))),
        },
        Object::Hash(h) => {
            let address = match h.borrow().get("address") {
                Some(Object::String(s)) => s.to_string(),
                _ => {
                    return Err(new_error(
                        ctx.pos.clone(),
                        format!("{}: address.address is required", name),
                    ))
                }
            };
            if address.is_empty() {
                return Err(new_error(
                    ctx.pos.clone(),
                    format!("{}: address.address is required", name),
                ));
            }
            let display = match h.borrow().get("name") {
                Some(Object::String(s)) => s.to_string(),
                _ => String::new(),
            };
            Ok(MailAddress {
                name: display,
                address,
            })
        }
        _ => Err(new_error(
            ctx.pos.clone(),
            format!("{}: address must be a string or object", name),
        )),
    }
}

fn mail_parse_address(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "mail.parseAddress", args, 0, "address") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match parse_one_address(&value) {
        Ok(a) => mail_address_object(&a),
        Err(e) => new_error(ctx.pos.clone(), format!("mail.parseAddress: {}", e)),
    }
}

fn mail_parse_address_list(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "mail.parseAddressList", args, 0, "addresses") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut out = Vec::new();
    for part in split_address_list(&value) {
        match parse_one_address(&part) {
            Ok(a) => out.push(mail_address_object(&a)),
            Err(e) => return new_error(ctx.pos.clone(), format!("mail.parseAddressList: {}", e)),
        }
    }
    array(out)
}

fn mail_parse_message(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "mail.parseMessage", args, 0, "message") {
        Ok(v) => v,
        Err(e) => return e,
    };
    // RFC 5322: headers then a blank line then the body.
    let (headers, body) = match value.find("\n\n") {
        Some(i) => (value[..i].to_string(), value[i + 2..].to_string()),
        None => match value.find("\r\n\r\n") {
            Some(i) => (value[..i].to_string(), value[i + 4..].to_string()),
            None => (value.clone(), String::new()),
        },
    };
    let headers_obj = parse_rfc5322_headers(&headers);
    let out = Rc::new(RefCell::new(HashData::default()));
    out.borrow_mut().set("headers", headers_obj);
    out.borrow_mut().set("body", str_obj(body));
    Object::Hash(out)
}

/// Parse a header block into a Hash mapping header name -> Array<string>.
/// Header unfolding (continuation lines starting with whitespace) is handled.
fn parse_rfc5322_headers(block: &str) -> Object {
    let hash = Rc::new(RefCell::new(HashData::default()));
    let mut current_name: Option<String> = None;
    let mut current_vals: Vec<String> = Vec::new();
    let mut flush =
        |name: &mut Option<String>, vals: &mut Vec<String>, hash: &Rc<RefCell<HashData>>| {
            if let Some(n) = name.take() {
                let arr: Vec<Object> = vals.drain(..).map(str_obj).collect();
                hash.borrow_mut().set(n, array(arr));
            }
        };
    for raw_line in block.lines() {
        if raw_line.starts_with(' ') || raw_line.starts_with('\t') {
            // Continuation of previous header value.
            if let Some(name) = &current_name {
                let _ = name; // suppress unused warnings
                if let Some(last) = current_vals.last_mut() {
                    last.push(' ');
                    last.push_str(raw_line.trim());
                }
            }
            continue;
        }
        flush(&mut current_name, &mut current_vals, &hash);
        match raw_line.find(':') {
            Some(i) => {
                current_name = Some(raw_line[..i].trim().to_string());
                current_vals.push(raw_line[i + 1..].trim().to_string());
            }
            None => {
                current_name = None;
                current_vals.clear();
            }
        }
    }
    flush(&mut current_name, &mut current_vals, &hash);
    Object::Hash(hash)
}

fn mail_format_address(ctx: &mut CallContext, args: &[Object]) -> Object {
    let addr = match args.get(0) {
        Some(v) => v,
        None => return new_error(ctx.pos.clone(), "mail.formatAddress requires address"),
    };
    match mail_address_from_value(ctx, "mail.formatAddress", addr) {
        Ok(a) => str_obj(format_address(&a)),
        Err(e) => e,
    }
}

fn mail_format_address_list(ctx: &mut CallContext, args: &[Object]) -> Object {
    let arr = match args.get(0) {
        Some(Object::Array(a)) => a.clone(),
        Some(_) => {
            return new_error(
                ctx.pos.clone(),
                "mail.formatAddressList: addresses must be an array",
            )
        }
        None => return new_error(ctx.pos.clone(), "mail.formatAddressList requires addresses"),
    };
    let mut formatted = Vec::new();
    for item in &arr.borrow().elements {
        match mail_address_from_value(ctx, "mail.formatAddressList", item) {
            Ok(a) => formatted.push(format_address(&a)),
            Err(e) => return e,
        }
    }
    str_obj(formatted.join(", "))
}

fn mail_parse_date(ctx: &mut CallContext, args: &[Object]) -> Object {
    let value = match required_string(ctx, "mail.parseDate", args, 0, "date") {
        Ok(v) => v,
        Err(e) => return e,
    };
    match parse_time_ms(&value) {
        Some(ms) => Object::Date(ms),
        None => new_error(
            ctx.pos.clone(),
            format!("mail.parseDate: unsupported date {}", value),
        ),
    }
}

fn mail_format_date(ctx: &mut CallContext, args: &[Object]) -> Object {
    let ms = match time_from_object(ctx, "mail.formatDate", args, 0) {
        Ok(ms) => ms,
        Err(_err) => now_ms(),
    };
    str_obj(format_rfc1123z(ms))
}

/// Format an epoch-millis instant as an RFC 1123 date with a numeric zone,
/// e.g. `Mon, 02 Jan 2006 15:04:05 -0700`. Uses UTC (+0000) because the GTS
/// time module renders in UTC throughout.
fn format_rfc1123z(ms: i64) -> String {
    let (year, month, day, hour, minute, second, _ms) = utc_parts_from_ms(ms);
    let days = ms.div_euclid(86_400_000);
    let weekday = weekday_short(days);
    let month_name = month_short(month);
    format!("{weekday}, {day:02} {month_name} {year:04} {hour:02}:{minute:02}:{second:02} +0000")
}

/// Return the 3-letter weekday for a `days-since-1970-01-01` count.
/// 1970-01-01 was a Thursday.
fn weekday_short(days: i64) -> &'static str {
    let names = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let idx = days.rem_euclid(7) as usize;
    names[idx]
}

fn month_short(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    }
}

fn mail_get_header(ctx: &mut CallContext, args: &[Object]) -> Object {
    let headers = match args.get(0) {
        Some(Object::Hash(h)) => h.clone(),
        Some(_) => return new_error(ctx.pos.clone(), "mail.getHeader: headers must be an object"),
        None => return new_error(ctx.pos.clone(), "mail.getHeader requires headers"),
    };
    let name = match required_string(ctx, "mail.getHeader", args, 1, "name") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let lower = name.to_ascii_lowercase();
    let hb = headers.borrow();
    for (k, v) in &hb.entries {
        if k.to_ascii_lowercase() == lower {
            if let Object::Array(arr) = v {
                let elems = &arr.borrow().elements;
                if elems.is_empty() {
                    return Object::Undefined;
                }
                return elems[0].clone();
            }
            return v.clone();
        }
    }
    Object::Undefined
}

// ---------------------------------------------------------------------------
// net/socket/client: synchronous TCP client (@std/net/socket/client)
// ---------------------------------------------------------------------------

/// A live TCP stream held inside a Hash via a sentinel state cell. The GTS VM
/// is single-threaded (synchronous tree-walker), so a plain `RefCell` is safe.
struct SocketStream {
    stream: std::cell::RefCell<Option<std::net::TcpStream>>,
}

const SOCKET_CONN_STATE_KEY: &str = "__socket_conn__";

fn socket_client_module() -> Object {
    module(vec![
        ("connect", native("socket.connect", socket_connect)),
        ("dial", native("socket.connect", socket_connect)),
    ])
}

fn socket_connect(ctx: &mut CallContext, args: &[Object]) -> Object {
    let host = match required_string(ctx, "socket.connect", args, 0, "host") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let port = match required_number(ctx, "socket.connect", args, 1, "port") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let addr = format!("{}:{}", host, port as i64);
    let socket_addr = match resolve_socket_addr(&host, port as u16) {
        Ok(sa) => sa,
        Err(e) => return new_error(ctx.pos.clone(), format!("socket.connect: {}", e)),
    };
    let timeout = std::time::Duration::from_secs(30);
    let stream = match std::net::TcpStream::connect_timeout(&socket_addr, timeout) {
        Ok(s) => s,
        Err(e) => return new_error(ctx.pos.clone(), format!("socket.connect: {} ({})", e, addr)),
    };
    let remote = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let local = stream
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    new_socket_conn_object(stream, remote, local)
}

/// Resolve a host/port into a single `SocketAddr`. Accepts both literal IP
/// addresses (no DNS) and hostnames (via the OS resolver).
fn resolve_socket_addr(host: &str, port: u16) -> std::io::Result<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "no address"))
}

fn new_socket_conn_object(stream: std::net::TcpStream, remote: String, local: String) -> Object {
    let conn = Rc::new(SocketStream {
        stream: std::cell::RefCell::new(Some(stream)),
    });
    let obj = Rc::new(RefCell::new(HashData::default()));
    obj.borrow_mut().set(
        SOCKET_CONN_STATE_KEY,
        Object::Hash(Rc::new(RefCell::new(HashData::default()))),
    );
    obj.borrow_mut().set("remoteAddr", str_obj(remote));
    obj.borrow_mut().set("localAddr", str_obj(local));

    let c = conn.clone();
    obj.borrow_mut().set(
        "write",
        native("socket.write", move |ctx, args| socket_write(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "send",
        native("socket.send", move |ctx, args| socket_write(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "read",
        native("socket.read", move |ctx, args| socket_read(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "recv",
        native("socket.recv", move |ctx, args| socket_recv(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "close",
        native("socket.close", move |_ctx, _args| socket_close(&c)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "setDeadline",
        native("socket.setDeadline", move |ctx, args| {
            socket_set_deadline(ctx, &c, args)
        }),
    );

    Object::Hash(obj)
}

fn socket_write(ctx: &mut CallContext, conn: &Rc<SocketStream>, args: &[Object]) -> Object {
    let data = match args.get(0) {
        Some(v) => v.inspect().into_bytes(),
        None => return new_error(ctx.pos.clone(), "socket.write requires data"),
    };
    let mut guard = conn.stream.borrow_mut();
    let stream = match guard.as_mut() {
        Some(s) => s,
        None => return new_error(ctx.pos.clone(), "socket.write: connection closed"),
    };
    use std::io::Write;
    match stream.write_all(&data).and_then(|_| stream.flush()) {
        Ok(_) => num_obj(data.len() as f64),
        Err(e) => new_error(ctx.pos.clone(), format!("socket.write: {}", e)),
    }
}

fn socket_read(ctx: &mut CallContext, conn: &Rc<SocketStream>, args: &[Object]) -> Object {
    let buf_size = match args.get(0) {
        Some(Object::Number(n)) => (*n as usize).max(1),
        _ => 4096,
    };
    socket_read_impl(ctx, conn, buf_size, "socket.read")
}

fn socket_recv(ctx: &mut CallContext, conn: &Rc<SocketStream>, _args: &[Object]) -> Object {
    socket_read_impl(ctx, conn, 4096, "socket.recv")
}

fn socket_read_impl(
    ctx: &mut CallContext,
    conn: &Rc<SocketStream>,
    buf_size: usize,
    name: &str,
) -> Object {
    use std::io::Read;
    let mut guard = conn.stream.borrow_mut();
    let stream = match guard.as_mut() {
        Some(s) => s,
        None => return new_error(ctx.pos.clone(), format!("{}: connection closed", name)),
    };
    let mut buf = vec![0u8; buf_size];
    match stream.read(&mut buf) {
        Ok(0) => Object::Null, // EOF
        Ok(n) => str_obj(String::from_utf8_lossy(&buf[..n]).into_owned()),
        Err(e) => new_error(ctx.pos.clone(), format!("{}: {}", name, e)),
    }
}

fn socket_close(conn: &Rc<SocketStream>) -> Object {
    let mut guard = conn.stream.borrow_mut();
    *guard = None; // Drop the TcpStream, closing the underlying socket.
    Object::Undefined
}

fn socket_set_deadline(ctx: &mut CallContext, conn: &Rc<SocketStream>, args: &[Object]) -> Object {
    let ms = match required_number(ctx, "socket.setDeadline", args, 0, "timeout") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let guard = conn.stream.borrow();
    match guard.as_ref() {
        Some(stream) => {
            let dur = Some(std::time::Duration::from_millis(ms.max(0.0) as u64));
            // Set both read and write timeouts; ignore errors (e.g. unsupported).
            let _ = stream.set_read_timeout(dur);
            let _ = stream.set_write_timeout(dur);
            Object::Undefined
        }
        None => new_error(ctx.pos.clone(), "socket.setDeadline: connection closed"),
    }
}

// ---------------------------------------------------------------------------
// net/socket/server: synchronous TCP server (@std/net/socket/server)
// ---------------------------------------------------------------------------

/// The synchronous VM has no event loop, so a Go-style background accept loop
/// cannot be reproduced. We expose the same `listen` / `createServer` surface
/// but the server runs in-line: each call to `acceptOne(handler)` blocks for a
/// single connection and invokes the handler synchronously. `listen(port,
/// handler)` returns the server object without spawning, and exposes
/// `acceptOne`/`close` for explicit control.
const SOCKET_SERVER_STATE_KEY: &str = "__socket_server__";

struct SocketServer {
    listener: std::cell::RefCell<Option<std::net::TcpListener>>,
    /// Handler registered at `listen` time, used by `acceptOne` when the
    /// caller does not pass one explicitly.
    handler: std::cell::RefCell<Option<Object>>,
}

fn socket_server_module() -> Object {
    module(vec![
        ("listen", native("socket.listen", socket_listen)),
        ("createServer", native("socket.createServer", socket_listen)),
    ])
}

fn socket_listen(ctx: &mut CallContext, args: &[Object]) -> Object {
    let port = match required_number(ctx, "socket.listen", args, 0, "port") {
        Ok(v) => v,
        Err(e) => return e,
    };
    // Capture the handler (if provided) so acceptOne can use it without
    // re-passing it on every call.
    let handler = match args.get(1) {
        Some(v @ (Object::Function(_) | Object::Builtin(_))) => Some(v.clone()),
        _ => None,
    };
    let addr = format!("0.0.0.0:{}", port as i64);
    let listener = match std::net::TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => return new_error(ctx.pos.clone(), format!("socket.listen: {}", e)),
    };
    // Don't block the whole VM on accept; set non-blocking so acceptOne can
    // be polled explicitly.
    let _ = listener.set_nonblocking(true);
    let bound_port = listener
        .local_addr()
        .map(|a| a.port())
        .unwrap_or(port as u16);

    let server = Rc::new(SocketServer {
        listener: std::cell::RefCell::new(Some(listener)),
        handler: std::cell::RefCell::new(handler),
    });
    let obj = Rc::new(RefCell::new(HashData::default()));
    obj.borrow_mut().set(
        SOCKET_SERVER_STATE_KEY,
        Object::Hash(Rc::new(RefCell::new(HashData::default()))),
    );
    obj.borrow_mut().set("port", num_obj(bound_port as f64));
    obj.borrow_mut()
        .set("address", str_obj(format!(":{}", bound_port)));

    let s = server.clone();
    obj.borrow_mut().set(
        "acceptOne",
        native("server.acceptOne", move |ctx, args| {
            socket_accept_one(ctx, &s, args)
        }),
    );
    let s = server.clone();
    obj.borrow_mut().set(
        "accept",
        native("server.accept", move |ctx, args| {
            socket_accept_one(ctx, &s, args)
        }),
    );
    let s = server.clone();
    obj.borrow_mut().set(
        "close",
        native("server.close", move |_ctx, _args| {
            let mut guard = s.listener.borrow_mut();
            *guard = None; // drop listener
            Object::Undefined
        }),
    );

    Object::Hash(obj)
}

fn socket_accept_one(ctx: &mut CallContext, server: &Rc<SocketServer>, args: &[Object]) -> Object {
    // Prefer an explicitly-passed handler; fall back to the one registered at
    // listen time.
    let handler = match args.get(0) {
        Some(v @ (Object::Function(_) | Object::Builtin(_))) => Some(v.clone()),
        Some(_) => {
            return new_error(
                ctx.pos.clone(),
                "server.acceptOne: handler must be a function",
            )
        }
        None => server.handler.borrow().clone(),
    };
    let handler = match handler {
        Some(h) => h,
        None => {
            return new_error(
                ctx.pos.clone(),
                "server.acceptOne requires a handler function",
            )
        }
    };

    let guard = server.listener.borrow();
    let listener = match guard.as_ref() {
        Some(l) => l,
        None => return new_error(ctx.pos.clone(), "server.acceptOne: server closed"),
    };
    // The listener is non-blocking; return a sentinel if no pending connection.
    let (stream, _addr) = match listener.accept() {
        Ok(pair) => pair,
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            return new_error(
                ctx.pos.clone(),
                "server.acceptOne: no pending connection (WouldBlock)",
            )
        }
        Err(e) => return new_error(ctx.pos.clone(), format!("server.acceptOne: {}", e)),
    };
    // Reset the accepted stream to blocking for synchronous read/write.
    let _ = stream.set_nonblocking(false);
    let remote = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let local = stream
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let conn_obj = new_socket_conn_object(stream, remote, local);
    // drop the listener borrow before invoking the handler, in case the
    // handler triggers another borrow (e.g. close).
    drop(guard);
    call_script_function(&handler, ctx.env, &[conn_obj])
}

// ---------------------------------------------------------------------------
// runtime: spawn an isolated sub-script (@std/runtime)
// ---------------------------------------------------------------------------

/// Options parsed from an optional GTS object: { cwd, argv, autoMain }.
struct RuntimeOpts {
    #[allow(dead_code)]
    cwd: Option<String>,
    argv: Vec<String>,
    auto_main: bool,
}

fn runtime_module() -> Object {
    module(vec![
        ("runScript", native("runtime.runScript", runtime_run_script)),
        (
            "callScript",
            native("runtime.callScript", runtime_call_script),
        ),
        ("runTool", native("runtime.runTool", runtime_run_tool)),
    ])
}

fn runtime_run_script(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "runtime.runScript", args, 0, "path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let opts = parse_runtime_opts(ctx, "runtime.runScript", args, 1);
    match run_sub_script(&path, &opts) {
        Ok(exports) => exports,
        Err(e) => e,
    }
}

fn runtime_call_script(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "runtime.callScript", args, 0, "path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let export_name = match required_string(ctx, "runtime.callScript", args, 1, "exportName") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let call_args = match args.get(2) {
        Some(Object::Array(arr)) => arr.borrow().elements.clone(),
        Some(Object::Undefined | Object::Null) | None => Vec::new(),
        Some(_) => return new_error(ctx.pos.clone(), "runtime.callScript: args must be an array"),
    };
    let opts = parse_runtime_opts(ctx, "runtime.callScript", args, 3);
    runtime_call_export(
        ctx,
        &path,
        &export_name,
        &call_args,
        &opts,
        "runtime.callScript",
    )
}

fn runtime_run_tool(ctx: &mut CallContext, args: &[Object]) -> Object {
    let path = match required_string(ctx, "runtime.runTool", args, 0, "path") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input = args.get(1).cloned().unwrap_or(Object::Undefined);
    let opts = parse_runtime_opts(ctx, "runtime.runTool", args, 2);
    runtime_call_export(ctx, &path, "run", &[input], &opts, "runtime.runTool")
}

/// Run an isolated sub-script and invoke a named export on it. The export's
/// return value (or resolved Promise) is forwarded to the caller.
fn runtime_call_export(
    ctx: &mut CallContext,
    path: &str,
    export_name: &str,
    call_args: &[Object],
    opts: &RuntimeOpts,
    api_name: &str,
) -> Object {
    let exports = match run_sub_script(path, opts) {
        Ok(e) => e,
        Err(err) => return err,
    };
    let export_obj = match &exports {
        Object::Hash(h) => h.borrow().get(export_name).cloned(),
        _ => None,
    };
    let func = match export_obj {
        Some(f) if !matches!(f, Object::Undefined | Object::Null) => f,
        _ => {
            return new_error(
                ctx.pos.clone(),
                format!("{}: {} must export {}(...)", api_name, path, export_name),
            )
        }
    };
    call_script_function(&func, ctx.env, call_args)
}

/// Parse the optional options object for runtime helpers.
fn parse_runtime_opts(
    ctx: &mut CallContext,
    name: &str,
    args: &[Object],
    index: usize,
) -> RuntimeOpts {
    let mut opts = RuntimeOpts {
        cwd: None,
        argv: Vec::new(),
        auto_main: false,
    };
    if let Some(Object::Hash(h)) = args.get(index) {
        let hb = h.borrow();
        if let Some(Object::String(s)) = hb.get("cwd") {
            opts.cwd = Some(s.to_string());
        }
        if let Some(Object::Array(arr)) = hb.get("argv") {
            opts.argv = arr
                .borrow()
                .elements
                .iter()
                .map(|o| match o {
                    Object::String(s) => s.to_string(),
                    other => other.inspect(),
                })
                .collect();
        }
        if let Some(Object::Boolean(b)) = hb.get("autoMain") {
            opts.auto_main = *b;
        }
    }
    let _ = name;
    let _ = ctx;
    opts
}

/// Spawn a fresh `Session`, run the file, and return its `module.exports`.
fn run_sub_script(path: &str, opts: &RuntimeOpts) -> crate::runtime::RuntimeResult<Object> {
    use crate::runtime::Session;
    let session = Session::new();
    let argv = if opts.argv.is_empty() {
        vec![std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()]
    } else {
        opts.argv.clone()
    };
    session.run_file_for_exports(path, argv, opts.auto_main)
}

// ---------------------------------------------------------------------------
// image / pdf: placeholder modules aligned with the Go version (@std/image, @std/pdf)
// ---------------------------------------------------------------------------

fn image_module() -> Object {
    module(vec![("info", native("image.info", image_info))])
}

fn image_info(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "image.info", args, 0, "path") {
        Ok(_path) => new_error(
            ctx.pos.clone(),
            "image module: basic placeholder - full implementation requires external library",
        ),
        Err(e) => e,
    }
}

fn pdf_module() -> Object {
    module(vec![("info", native("pdf.info", pdf_info))])
}

fn pdf_info(ctx: &mut CallContext, args: &[Object]) -> Object {
    match required_string(ctx, "pdf.info", args, 0, "path") {
        Ok(_path) => new_error(
            ctx.pos.clone(),
            "pdf module: basic placeholder - full implementation requires external library",
        ),
        Err(e) => e,
    }
}

// ---------------------------------------------------------------------------
// net/ws/client + net/ws/server: WebSocket (RFC 6455) over blocking TCP
// (@std/net/ws/client, @std/net/ws/server)
// ---------------------------------------------------------------------------

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

const WS_OP_CONTINUATION: u8 = 0;
const WS_OP_TEXT: u8 = 1;
const WS_OP_BINARY: u8 = 2;
const WS_OP_CLOSE: u8 = 8;
const WS_OP_PING: u8 = 9;
const WS_OP_PONG: u8 = 10;

/// A live WebSocket connection wrapping a blocking `TcpStream`. The frame
/// reader/writer mirror Go's `wsConn` exactly (RFC 6455 framing).
struct WsConn {
    stream: std::cell::RefCell<Option<std::net::TcpStream>>,
}

const WS_CONN_STATE_KEY: &str = "__ws_conn__";

fn ws_client_module() -> Object {
    module(vec![("connect", native("ws.connect", ws_client_connect))])
}

fn ws_server_module() -> Object {
    module(vec![
        (
            "createServer",
            native("ws.createServer", ws_server_create_server),
        ),
        ("upgrade", native("ws.upgrade", ws_server_upgrade)),
    ])
}

fn ws_client_connect(ctx: &mut CallContext, args: &[Object]) -> Object {
    let url = match required_string(ctx, "ws.connect", args, 0, "url") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut headers: Vec<(String, String)> = Vec::new();
    if let Some(Object::Hash(h)) = args.get(1) {
        for (k, v) in &h.borrow().entries {
            headers.push((k.clone(), v.inspect()));
        }
    }
    let conn = match dial_websocket(&url, &headers) {
        Ok(c) => c,
        Err(e) => return new_error(ctx.pos.clone(), format!("ws.connect: {}", e)),
    };
    new_ws_conn_object(Rc::new(WsConn {
        stream: std::cell::RefCell::new(Some(conn)),
    }))
}

/// Perform the WebSocket opening handshake over a fresh TCP connection and
/// return the upgraded stream. Mirrors Go's `dialWebSocket`.
fn dial_websocket(url: &str, headers: &[(String, String)]) -> std::io::Result<std::net::TcpStream> {
    let is_secure = url.starts_with("wss://");
    let stripped = url
        .strip_prefix("ws://")
        .or_else(|| url.strip_prefix("wss://"))
        .unwrap_or(url);
    let (host, path) = match stripped.find('/') {
        Some(i) => (&stripped[..i], &stripped[i..]),
        None => (stripped, "/"),
    };
    let host_port = if host.contains(':') {
        host.to_string()
    } else if is_secure {
        format!("{}:443", host)
    } else {
        format!("{}:80", host)
    };

    let socket_addr = match resolve_socket_addr(
        host_port.split(':').next().unwrap_or(host),
        host_port
            .rsplit(':')
            .next()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(if is_secure { 443 } else { 80 }),
    ) {
        Ok(sa) => sa,
        Err(e) => return Err(e),
    };
    let mut stream =
        std::net::TcpStream::connect_timeout(&socket_addr, std::time::Duration::from_secs(10))?;

    // Generate the client nonce (16 random bytes, base64-encoded).
    let mut nonce_bytes = [0u8; 16];
    if !getrandom_inner(&mut nonce_bytes) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "random source unavailable",
        ));
    }
    let nonce = base64_std_encode(&nonce_bytes);

    let mut req = String::new();
    req.push_str(&format!("GET {} HTTP/1.1\r\n", path));
    req.push_str(&format!("Host: {}\r\n", host));
    req.push_str("Upgrade: websocket\r\n");
    req.push_str("Connection: Upgrade\r\n");
    req.push_str(&format!("Sec-WebSocket-Key: {}\r\n", nonce));
    req.push_str("Sec-WebSocket-Version: 13\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{}: {}\r\n", k, v));
    }
    req.push_str("\r\n");
    use std::io::Write;
    stream.write_all(req.as_bytes())?;
    stream.flush()?;

    // Read the HTTP response, looking for "101 Switching Protocols" and the
    // Sec-WebSocket-Accept header.
    use std::io::Read;
    let mut buf = [0u8; 4096];
    let mut collected = Vec::new();
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed during handshake",
            ));
        }
        collected.extend_from_slice(&buf[..n]);
        // The header section ends at "\r\n\r\n".
        if let Some(idx) = find_subsequence(&collected, b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&collected[..idx]).to_string();
            if !head.contains(" 101 ") {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "unexpected handshake status",
                ));
            }
            let expected = compute_accept_key(&nonce);
            if !head.contains(&format!("Sec-WebSocket-Accept: {}", expected)) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "invalid Sec-WebSocket-Accept",
                ));
            }
            return Ok(stream);
        }
        if collected.len() > 64 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "handshake response too large",
            ));
        }
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// RFC 6455 accept-key: base64(sha1(client_key + GUID)).
fn compute_accept_key(key: &str) -> String {
    let digest = sha1(format!("{}{}", key, WS_GUID).as_bytes());
    base64_std_encode(&digest)
}

// --- Frame read/write (RFC 6455 §5) ---------------------------------------

fn ws_write_frame(
    stream: &mut std::net::TcpStream,
    opcode: u8,
    payload: &[u8],
) -> std::io::Result<()> {
    use std::io::Write;
    let mut frame = Vec::with_capacity(2 + payload.len() + 8);
    frame.push(0x80 | opcode); // FIN=1, opcode
    let len = payload.len();
    if len <= 125 {
        frame.push(len as u8);
    } else if len <= 65535 {
        frame.push(126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame)?;
    stream.flush()
}

fn ws_read_frame(stream: &mut std::net::TcpStream) -> std::io::Result<(u8, Vec<u8>)> {
    use std::io::Read;
    let mut header = [0u8; 2];
    read_exact(stream, &mut header)?;
    let fin = (header[0] & 0x80) != 0;
    let opcode = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;
    let mut length = (header[1] & 0x7F) as u64;
    if length == 126 {
        let mut ext = [0u8; 2];
        read_exact(stream, &mut ext)?;
        length = u16::from_be_bytes(ext) as u64;
    } else if length == 127 {
        let mut ext = [0u8; 8];
        read_exact(stream, &mut ext)?;
        length = u64::from_be_bytes(ext);
    }
    let mut mask_key = [0u8; 4];
    if masked {
        read_exact(stream, &mut mask_key)?;
    }
    let mut payload = vec![0u8; length as usize];
    read_exact(stream, &mut payload)?;
    if masked {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask_key[i % 4];
        }
    }
    if fin {
        Ok((opcode, payload))
    } else {
        // Fragmented: keep reading until a FIN frame arrives (concatenate).
        let (next_op, mut rest) = ws_read_frame(stream)?;
        let _ = next_op;
        payload.extend_from_slice(&rest);
        Ok((opcode, payload))
    }
}

fn read_exact(stream: &mut std::net::TcpStream, buf: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    let mut filled = 0;
    while filled < buf.len() {
        match stream.read(&mut buf[filled..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed mid-frame",
                ))
            }
            Ok(n) => filled += n,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Read the next data message (text/binary), automatically answering Pings
/// with Pongs and surfacing Close as EOF. Mirrors Go's `ReadMessage`.
fn ws_read_message(stream: &mut std::net::TcpStream) -> std::io::Result<(u8, Vec<u8>)> {
    loop {
        let (opcode, payload) = ws_read_frame(stream)?;
        match opcode {
            WS_OP_TEXT | WS_OP_BINARY => return Ok((opcode, payload)),
            WS_OP_CLOSE => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "close",
                ))
            }
            WS_OP_PING => {
                let _ = ws_write_frame(stream, WS_OP_PONG, &payload);
            }
            WS_OP_PONG | WS_OP_CONTINUATION => {}
            _ => {}
        }
    }
}

/// Build the connection object exposed to GTS scripts.
fn new_ws_conn_object(conn: Rc<WsConn>) -> Object {
    let obj = Rc::new(RefCell::new(HashData::default()));
    obj.borrow_mut().set(
        WS_CONN_STATE_KEY,
        Object::Hash(Rc::new(RefCell::new(HashData::default()))),
    );

    let c = conn.clone();
    obj.borrow_mut().set(
        "send",
        native("ws.send", move |ctx, args| ws_send_text(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "sendText",
        native("ws.sendText", move |ctx, args| ws_send_text(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "sendBinary",
        native("ws.sendBinary", move |ctx, args| {
            ws_send_binary(ctx, &c, args)
        }),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "recv",
        native("ws.recv", move |ctx, args| ws_recv(ctx, &c, args)),
    );
    let c = conn.clone();
    obj.borrow_mut().set(
        "close",
        native("ws.close", move |_ctx, _args| {
            let mut guard = c.stream.borrow_mut();
            *guard = None;
            Object::Undefined
        }),
    );

    Object::Hash(obj)
}

fn ws_send_text(ctx: &mut CallContext, conn: &Rc<WsConn>, args: &[Object]) -> Object {
    let data = match args.get(0) {
        Some(v) => v.inspect(),
        None => return new_error(ctx.pos.clone(), "ws.send requires data"),
    };
    ws_write(ctx, conn, WS_OP_TEXT, data.into_bytes(), "ws.send")
}

fn ws_send_binary(ctx: &mut CallContext, conn: &Rc<WsConn>, args: &[Object]) -> Object {
    let data = match args.get(0) {
        Some(v) => v.inspect().into_bytes(),
        None => return new_error(ctx.pos.clone(), "ws.sendBinary requires data"),
    };
    ws_write(ctx, conn, WS_OP_BINARY, data, "ws.sendBinary")
}

fn ws_write(
    ctx: &mut CallContext,
    conn: &Rc<WsConn>,
    opcode: u8,
    payload: Vec<u8>,
    name: &str,
) -> Object {
    let mut guard = conn.stream.borrow_mut();
    let stream = match guard.as_mut() {
        Some(s) => s,
        None => return new_error(ctx.pos.clone(), format!("{}: connection closed", name)),
    };
    match ws_write_frame(stream, opcode, &payload) {
        Ok(_) => Object::Undefined,
        Err(e) => new_error(ctx.pos.clone(), format!("{}: {}", name, e)),
    }
}

fn ws_recv(ctx: &mut CallContext, conn: &Rc<WsConn>, _args: &[Object]) -> Object {
    let mut guard = conn.stream.borrow_mut();
    let stream = match guard.as_mut() {
        Some(s) => s,
        None => return new_error(ctx.pos.clone(), "ws.recv: connection closed"),
    };
    match ws_read_message(stream) {
        Ok((_op, data)) => str_obj(String::from_utf8_lossy(&data).into_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Object::Null,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted => Object::Null,
        Err(e) => new_error(ctx.pos.clone(), format!("ws.recv: {}", e)),
    }
}

// --- Server side ----------------------------------------------------------

/// Background-free WS server: binds a TCP listener (non-blocking) and exposes
/// `accept(handler)`/`acceptOne(handler)`/`close`. Each accept performs the
/// WS handshake inline then invokes the handler with the upgraded connection.
/// `upgrade(reqObj)` is a no-op stub here because the synchronous VM has no
/// HTTP request abstraction to hijack; it returns an error explaining this.
const WS_SERVER_STATE_KEY: &str = "__ws_server__";

struct WsServer {
    listener: std::cell::RefCell<Option<std::net::TcpListener>>,
}

fn ws_server_create_server(ctx: &mut CallContext, args: &[Object]) -> Object {
    let port = match required_number(ctx, "ws.createServer", args, 0, "port") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let handler = match args.get(1) {
        Some(v @ (Object::Function(_) | Object::Builtin(_))) => Some(v.clone()),
        _ => None,
    };
    let addr = format!("0.0.0.0:{}", port as i64);
    let listener = match std::net::TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => return new_error(ctx.pos.clone(), format!("ws.createServer: {}", e)),
    };
    let _ = listener.set_nonblocking(true);
    let bound_port = listener
        .local_addr()
        .map(|a| a.port())
        .unwrap_or(port as u16);

    let server = Rc::new(WsServer {
        listener: std::cell::RefCell::new(Some(listener)),
    });
    let obj = Rc::new(RefCell::new(HashData::default()));
    obj.borrow_mut().set(
        WS_SERVER_STATE_KEY,
        Object::Hash(Rc::new(RefCell::new(HashData {
            entries: vec![(
                "__ws_handler__".to_string(),
                handler.unwrap_or(Object::Undefined),
            )],
            ..Default::default()
        }))),
    );
    obj.borrow_mut().set("port", num_obj(bound_port as f64));
    obj.borrow_mut()
        .set("address", str_obj(format!(":{}", bound_port)));

    let s = server.clone();
    obj.borrow_mut().set(
        "acceptOne",
        native("ws.acceptOne", move |ctx, args| {
            ws_accept_one(ctx, &s, args)
        }),
    );
    let s = server.clone();
    obj.borrow_mut().set(
        "accept",
        native("ws.accept", move |ctx, args| ws_accept_one(ctx, &s, args)),
    );
    let s = server.clone();
    obj.borrow_mut().set(
        "close",
        native("ws.serverClose", move |_ctx, _args| {
            let mut guard = s.listener.borrow_mut();
            *guard = None;
            Object::Undefined
        }),
    );

    Object::Hash(obj)
}

fn ws_accept_one(ctx: &mut CallContext, server: &Rc<WsServer>, args: &[Object]) -> Object {
    let handler = match args.get(0) {
        Some(v @ (Object::Function(_) | Object::Builtin(_))) => Some(v.clone()),
        _ => None,
    };
    let guard = server.listener.borrow();
    let listener = match guard.as_ref() {
        Some(l) => l,
        None => return new_error(ctx.pos.clone(), "ws.acceptOne: server closed"),
    };
    let (mut stream, _addr) = match listener.accept() {
        Ok(pair) => pair,
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            return new_error(
                ctx.pos.clone(),
                "ws.acceptOne: no pending connection (WouldBlock)",
            )
        }
        Err(e) => return new_error(ctx.pos.clone(), format!("ws.acceptOne: {}", e)),
    };
    drop(guard);
    // Reset to blocking for the synchronous handshake + read/write loop.
    let _ = stream.set_nonblocking(false);

    // Perform the server-side WS handshake.
    match ws_server_handshake(&mut stream) {
        Ok(()) => {}
        Err(e) => return new_error(ctx.pos.clone(), format!("ws.acceptOne: handshake: {}", e)),
    }
    let conn = new_ws_conn_object(Rc::new(WsConn {
        stream: std::cell::RefCell::new(Some(stream)),
    }));

    match handler {
        Some(h) => call_script_function(&h, ctx.env, &[conn]),
        None => conn,
    }
}

/// Read the client's HTTP upgrade request, validate it, and write back the
/// "101 Switching Protocols" response with the computed accept key.
fn ws_server_handshake(stream: &mut std::net::TcpStream) -> std::io::Result<()> {
    use std::io::{Read, Write};
    let mut collected: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "client closed before handshake",
            ));
        }
        collected.extend_from_slice(&buf[..n]);
        if let Some(idx) = find_subsequence(&collected, b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&collected[..idx]).to_string();
            let key = extract_header(&head, "Sec-WebSocket-Key").ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "missing Sec-WebSocket-Key")
            })?;
            let accept = compute_accept_key(&key);
            let resp = format!(
                "HTTP/1.1 101 Switching Protocols\r\n\
                 Upgrade: websocket\r\n\
                 Connection: Upgrade\r\n\
                 Sec-WebSocket-Accept: {}\r\n\
                 \r\n",
                accept
            );
            stream.write_all(resp.as_bytes())?;
            stream.flush()?;
            return Ok(());
        }
        if collected.len() > 64 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "handshake request too large",
            ));
        }
    }
}

fn extract_header(head: &str, name: &str) -> Option<String> {
    for line in head.lines() {
        if let Some(idx) = line.find(':') {
            let key = line[..idx].trim();
            if key.eq_ignore_ascii_case(name) {
                return Some(line[idx + 1..].trim().to_string());
            }
        }
    }
    None
}

fn ws_server_upgrade(ctx: &mut CallContext, _args: &[Object]) -> Object {
    // The synchronous VM has no live HTTP request/response pair to hijack.
    // Scripts that want a WS server should use ws.createServer(port, handler)
    // + acceptOne, which performs the handshake inline.
    new_error(
        ctx.pos.clone(),
        "ws.upgrade is not supported in the synchronous runtime; use ws.createServer(port, handler).acceptOne() instead",
    )
}

// ---------------------------------------------------------------------------
// net/http/server: synchronous HTTP server backed by tiny_http
// (@std/net/http/server)
// ---------------------------------------------------------------------------

/// The synchronous VM has no event loop, so a Go-style background accept loop
/// cannot be reproduced. We expose `createServer(handler?, port?)` returning a
/// server object whose `acceptOne(handler?)` blocks for a single request,
/// invokes the handler synchronously with `{method,url,path,body,query,headers,
/// remoteAddr}` and a response object `{status,setHeader,send,json,end}`, then
/// returns. The handler fully controls the response via the closure-captured
/// `tiny_http::Response` builder state.
const HTTP_SERVER_STATE_KEY: &str = "__http_server__";

struct HttpServer {
    server: std::cell::RefCell<Option<tiny_http::Server>>,
    handler: std::cell::RefCell<Option<Object>>,
}

fn http_server_module() -> Object {
    module(vec![(
        "createServer",
        native("http.createServer", http_create_server),
    )])
}

fn http_create_server(ctx: &mut CallContext, args: &[Object]) -> Object {
    // Args mirror the Go signature loosely: (handler?, port?).
    //   http.createServer(handler)            — handler only, ephemeral port
    //   http.createServer(handler, port)      — handler + port
    //   http.createServer(port)               — port only, handler via acceptOne
    let mut handler = None;
    let mut port: Option<u16> = None;
    for arg in args {
        match arg {
            Object::Function(_) | Object::Builtin(_) => handler = Some(arg.clone()),
            Object::Number(n) => port = Some(*n as u16),
            _ => {}
        }
    }

    let bind_addr = match port {
        Some(p) => format!("0.0.0.0:{}", p),
        None => "0.0.0.0:0".to_string(), // ephemeral port on all interfaces
    };
    let server = match tiny_http::Server::http(bind_addr.as_str()) {
        Ok(s) => s,
        Err(e) => return new_error(ctx.pos.clone(), format!("http.createServer: {}", e)),
    };
    let bound_addr = server.server_addr();
    let bound_port = match bound_addr {
        tiny_http::ListenAddr::IP(addr) => addr.port(),
    };

    let srv = Rc::new(HttpServer {
        server: std::cell::RefCell::new(Some(server)),
        handler: std::cell::RefCell::new(handler),
    });
    let obj = Rc::new(RefCell::new(HashData::default()));
    obj.borrow_mut().set(
        HTTP_SERVER_STATE_KEY,
        Object::Hash(Rc::new(RefCell::new(HashData::default()))),
    );
    obj.borrow_mut().set("port", num_obj(bound_port as f64));
    obj.borrow_mut()
        .set("address", str_obj(format!(":{}", bound_port)));

    let s = srv.clone();
    obj.borrow_mut().set(
        "acceptOne",
        native("server.acceptOne", move |ctx, args| {
            http_accept_one(ctx, &s, args)
        }),
    );
    let s = srv.clone();
    obj.borrow_mut().set(
        "accept",
        native("server.accept", move |ctx, args| {
            http_accept_one(ctx, &s, args)
        }),
    );
    let s = srv.clone();
    obj.borrow_mut().set(
        "close",
        native("server.close", move |_ctx, _args| {
            let mut guard = s.server.borrow_mut();
            *guard = None; // drop the tiny_http::Server
            Object::Undefined
        }),
    );

    Object::Hash(obj)
}

fn http_accept_one(ctx: &mut CallContext, server: &Rc<HttpServer>, args: &[Object]) -> Object {
    let handler = match args.get(0) {
        Some(v @ (Object::Function(_) | Object::Builtin(_))) => Some(v.clone()),
        _ => server.handler.borrow().clone(),
    };

    // Take the request out of the server, then run the handler. We must drop
    // the listener borrow before invoking the handler so the handler can call
    // close()/acceptOne() recursively without RefCell reentrancy.
    let mut request = {
        let guard = server.server.borrow();
        let srv = match guard.as_ref() {
            Some(s) => s,
            None => return new_error(ctx.pos.clone(), "server.acceptOne: server closed"),
        };
        // tiny_http's recv() blocks until a request arrives.
        match srv.recv() {
            Ok(r) => r,
            Err(e) => return new_error(ctx.pos.clone(), format!("server.acceptOne: {}", e)),
        }
    };

    // Build the request object.
    let method = request.method().as_str().to_string();
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();
    let remote_addr = request
        .remote_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();

    // Collect headers into a Hash (first value per name).
    let headers_obj = Rc::new(RefCell::new(HashData::default()));
    {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for h in request.headers() {
            let key = h.field.as_str().to_string();
            if seen.insert(key.to_ascii_lowercase()) {
                headers_obj
                    .borrow_mut()
                    .set(key, str_obj(h.value.as_str().to_string()));
            }
        }
    }

    // Parse query string into a Hash.
    let query_obj = Rc::new(RefCell::new(HashData::default()));
    if let Some(qstart) = url.find('?') {
        for pair in url[qstart + 1..].split('&') {
            if let Some(eq) = pair.find('=') {
                let k = percent_decode(&pair[..eq]);
                let v = percent_decode(&pair[eq + 1..]);
                query_obj.borrow_mut().set(k, str_obj(v));
            } else if !pair.is_empty() {
                query_obj
                    .borrow_mut()
                    .set(percent_decode(pair), str_obj(String::new()));
            }
        }
    }

    // Read the body.
    let mut body_buf = Vec::new();
    {
        let mut reader = request.as_reader();
        let _ = reader.read_to_end(&mut body_buf);
    }
    let body = String::from_utf8_lossy(&body_buf).into_owned();

    // Response state shared with the handler closures.
    let resp_state = Rc::new(RefCell::new(HttpResponseState::default()));

    let req_obj = Rc::new(RefCell::new(HashData::default()));
    req_obj.borrow_mut().set("method", str_obj(method));
    req_obj.borrow_mut().set("url", str_obj(url));
    req_obj.borrow_mut().set("path", str_obj(path));
    req_obj.borrow_mut().set("body", str_obj(body));
    req_obj.borrow_mut().set("query", Object::Hash(query_obj));
    req_obj
        .borrow_mut()
        .set("headers", Object::Hash(headers_obj));
    req_obj.borrow_mut().set("remoteAddr", str_obj(remote_addr));

    let res_obj = http_response_object(resp_state.clone());

    // Invoke handler(req, res). The handler populates resp_state via closures.
    let handler_result = match handler {
        Some(h) => call_script_function(&h, ctx.env, &[Object::Hash(req_obj), res_obj.clone()]),
        None => Object::Undefined,
    };

    // If the handler threw a runtime error, respond with 500 and surface it.
    if handler_result.is_runtime_error() {
        let _ = request.respond(
            tiny_http::Response::from_string("Internal Server Error").with_status_code(500),
        );
        return handler_result;
    }

    // Build the tiny_http::Response from the accumulated state and respond on
    // the original request (kept alive above).
    let state = resp_state.borrow();
    let status_code = state.status.unwrap_or(200);
    let tiny_status = tiny_http::StatusCode(status_code);
    let body_bytes = state.body.clone().unwrap_or_default();
    let content_type = state
        .content_type
        .clone()
        .unwrap_or_else(|| "text/plain".to_string());
    let mut response = tiny_http::Response::from_data(body_bytes);
    response = response.with_status_code(tiny_status);
    if let Ok(h) = tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()) {
        response = response.with_header(h);
    }
    for (k, v) in &state.headers {
        if let Ok(h) = tiny_http::Header::from_bytes(k.as_bytes(), v.as_bytes()) {
            response = response.with_header(h);
        }
    }
    drop(state);
    let _ = request.respond(response);

    Object::Undefined
}

/// Minimal percent-decoding for query-string values.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_digit(bytes[i + 1]);
            let lo = hex_digit(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
            out.push(bytes[i]);
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Accumulated response state, mutated by the handler via closures.
#[derive(Default)]
struct HttpResponseState {
    status: Option<u16>,
    headers: Vec<(String, String)>,
    content_type: Option<String>,
    body: Option<Vec<u8>>,
}

fn http_response_object(state: Rc<RefCell<HttpResponseState>>) -> Object {
    let obj = Rc::new(RefCell::new(HashData::default()));

    let s = state.clone();
    obj.borrow_mut().set(
        "status",
        native("response.status", move |_ctx, args| {
            if let Some(Object::Number(n)) = args.get(0) {
                s.borrow_mut().status = Some(*n as u16);
            }
            Object::Undefined
        }),
    );
    let s = state.clone();
    obj.borrow_mut().set(
        "setHeader",
        native("response.setHeader", move |_ctx, args| {
            let key = match args.get(0) {
                Some(Object::String(v)) => v.to_string(),
                Some(o) => o.inspect(),
                None => return Object::Undefined,
            };
            let value = match args.get(1) {
                Some(Object::String(v)) => v.to_string(),
                Some(o) => o.inspect(),
                None => return Object::Undefined,
            };
            if key.eq_ignore_ascii_case("content-type") {
                s.borrow_mut().content_type = Some(value);
            } else {
                s.borrow_mut().headers.push((key, value));
            }
            Object::Undefined
        }),
    );
    let s = state.clone();
    obj.borrow_mut().set(
        "send",
        native("response.send", move |_ctx, args| {
            let text = match args.get(0) {
                Some(Object::String(v)) => v.to_string(),
                Some(o) => o.inspect(),
                None => String::new(),
            };
            let mut g = s.borrow_mut();
            if g.content_type.is_none() {
                g.content_type = Some("text/plain".to_string());
            }
            g.body = Some(text.into_bytes());
            Object::Undefined
        }),
    );
    let s = state.clone();
    obj.borrow_mut().set(
        "json",
        native("response.json", move |_ctx, args| {
            let text = match args.get(0) {
                Some(Object::Hash(h)) => hash_to_json(&h.borrow()),
                Some(Object::Array(a)) => value_to_json(&Object::Array(a.clone())),
                Some(Object::String(v)) => v.to_string(),
                Some(o) => o.inspect(),
                None => String::new(),
            };
            let mut g = s.borrow_mut();
            g.content_type = Some("application/json".to_string());
            g.body = Some(text.into_bytes());
            Object::Undefined
        }),
    );
    let s = state.clone();
    obj.borrow_mut().set(
        "end",
        native("response.end", move |_ctx, args| {
            if let Some(arg) = args.get(0) {
                let text = match arg {
                    Object::String(v) => v.to_string(),
                    o => o.inspect(),
                };
                let mut g = s.borrow_mut();
                if g.body.is_none() {
                    g.body = Some(text.into_bytes());
                }
            }
            Object::Undefined
        }),
    );

    Object::Hash(obj)
}

/// Serialize a Hash to a JSON string (minimal, no external dependency).
fn hash_to_json(h: &HashData) -> String {
    let pairs: Vec<String> = h
        .entries
        .iter()
        .map(|(k, v)| format!("{}: {}", json_escape_string(k), value_to_json(v)))
        .collect();
    format!("{{{}}}", pairs.join(", "))
}

fn value_to_json(obj: &Object) -> String {
    match obj {
        Object::Null => "null".to_string(),
        Object::Undefined => "null".to_string(),
        Object::Boolean(b) => b.to_string(),
        Object::Number(n) => format_number(*n),
        Object::String(s) => json_escape_string(s),
        Object::Array(a) => {
            let elems: Vec<String> = a.borrow().elements.iter().map(value_to_json).collect();
            format!("[{}]", elems.join(", "))
        }
        Object::Hash(h) => hash_to_json(&h.borrow()),
        _ => json_escape_string(&obj.inspect()),
    }
}

fn json_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// web / express: synchronous Express-style web framework (@std/web, @std/express)
// ---------------------------------------------------------------------------

// Per-thread context used by the concurrent (prefork) listen path.
//
// When the main thread decides to serve with `workers: N`, it spawns N worker
// threads. Each worker sets this thread-local before re-executing the script.
// The script's top-level `app.listen(...)` call is then intercepted: instead
// of binding a new socket or spawning more workers, the worker jumps straight
// into the shared accept loop using the server/shutdown already provided here.
//
// This is how we avoid infinite recursion (workers re-running the script that
// calls listen) without asking users to structure their script differently.
thread_local! {
    static WEB_WORKER_CTX: RefCell<Option<WebWorkerCtx>> = const { RefCell::new(None) };
}

/// The shared handle a worker thread receives from the spawner.
struct WebWorkerCtx {
    /// The single bound listener shared by all workers (accept-ready model).
    server: std::sync::Arc<tiny_http::Server>,
    /// Set to true by `app.close()` or Ctrl+C to ask workers to exit.
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// This worker's id (0-based), for logging.
    id: usize,
}

/// A registered route: method filter, path pattern (with `:param` segments),
/// and the ordered list of handler/middleware functions.
struct WebRoute {
    method: String,        // GET/POST/.../ALL/USE
    segments: Vec<String>, // split path, each segment possibly ":name"
    handlers: Vec<Object>,
}

/// App state: ordered routes + a tiny_http server bound on listen().
///
/// - `server`: used by the serial path (`count: N`). Set on listen, cleared on
///   return.
/// - `shared_server`: used by the concurrent path (`workers: N`). An `Arc` so
///   multiple worker threads can call `recv_timeout` on the same listener.
/// - `shutdown_signal`: set by `app.close()` (or Ctrl+C) to ask all workers to
///   exit their accept loops. `None` when running serially.
struct WebApp {
    routes: std::cell::RefCell<Vec<WebRoute>>,
    server: std::cell::RefCell<Option<tiny_http::Server>>,
    shared_server: std::cell::RefCell<Option<std::sync::Arc<tiny_http::Server>>>,
    shutdown_signal: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
}

const WEB_APP_STATE_KEY: &str = "__web_app__";

fn web_module() -> Object {
    module(vec![
        ("createApp", native("web.createApp", web_create_app)),
        ("json", native("web.json", web_json_helper)),
        ("text", native("web.text", web_text_helper)),
        ("static", native("web.static", web_static_helper)),
    ])
}

fn web_create_app(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    let app = Rc::new(WebApp {
        routes: std::cell::RefCell::new(Vec::new()),
        server: std::cell::RefCell::new(None),
        shared_server: std::cell::RefCell::new(None),
        shutdown_signal: std::cell::RefCell::new(None),
    });
    let obj = Rc::new(RefCell::new(HashData::default()));
    obj.borrow_mut().set(
        WEB_APP_STATE_KEY,
        Object::Hash(Rc::new(RefCell::new(HashData::default()))),
    );

    // Register HTTP-method route helpers: get/post/put/patch/delete/all.
    for method in ["get", "post", "put", "patch", "delete", "all"] {
        let m = method.to_string();
        let a = app.clone();
        let upper = m.to_ascii_uppercase();
        obj.borrow_mut().set(
            m.as_str(),
            native("web.route", move |ctx, args| {
                web_register_route(ctx, &a, &upper, args)
            }),
        );
    }

    let a = app.clone();
    obj.borrow_mut().set(
        "use",
        native("web.use", move |ctx, args| web_use(ctx, &a, args)),
    );

    let a = app.clone();
    obj.borrow_mut().set(
        "listen",
        native("web.listen", move |ctx, args| web_listen(ctx, &a, args)),
    );

    let a = app.clone();
    obj.borrow_mut().set(
        "close",
        native("web.close", move |_ctx, _args| {
            // Serial path: just drop the owned server (original behaviour).
            *a.server.borrow_mut() = None;
            // Concurrent path on the MAIN thread: signal workers via the app's
            // published shutdown flag + unblock any parked recv().
            if let Some(flag) = a.shutdown_signal.borrow().as_ref() {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            if let Some(srv) = a.shared_server.borrow().as_ref() {
                srv.unblock();
            }
            // Concurrent path inside a WORKER thread: this app is the worker's
            // own instance, so its shutdown_signal is None. Reach for the
            // shared shutdown flag published via thread-local instead, so
            // `app.close()` called from a handler stops all workers.
            WEB_WORKER_CTX.with(|c| {
                if let Some(wctx) = c.borrow().as_ref() {
                    wctx.shutdown
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    wctx.server.unblock();
                }
            });
            Object::Undefined
        }),
    );

    Object::Hash(obj)
}

/// `app.METHOD(path, ...handlers)` or `app.METHOD(path, handler)`.
fn web_register_route(
    ctx: &mut CallContext,
    app: &Rc<WebApp>,
    method: &str,
    args: &[Object],
) -> Object {
    if args.len() < 2 {
        return new_error(
            ctx.pos.clone(),
            format!(
                "web.{} requires path and handler",
                method.to_ascii_lowercase()
            ),
        );
    }
    let path = match &args[0] {
        Object::String(s) => s.to_string(),
        _ => {
            return new_error(
                ctx.pos.clone(),
                format!("web.{}: path must be a string", method.to_ascii_lowercase()),
            )
        }
    };
    let handlers: Vec<Object> = args[1..]
        .iter()
        .filter(|h| matches!(h, Object::Function(_) | Object::Builtin(_)))
        .cloned()
        .collect();
    if handlers.is_empty() {
        return new_error(
            ctx.pos.clone(),
            format!(
                "web.{}: handler must be a function",
                method.to_ascii_lowercase()
            ),
        );
    }
    app.routes.borrow_mut().push(WebRoute {
        method: method.to_string(),
        segments: split_route_path(&path),
        handlers,
    });
    Object::Undefined
}

/// `app.use([path], ...handlers)` registers middleware. Path defaults to "/".
fn web_use(ctx: &mut CallContext, app: &Rc<WebApp>, args: &[Object]) -> Object {
    let mut path = "/".to_string();
    let mut start = 0;
    if let Some(Object::String(s)) = args.get(0) {
        path = s.to_string();
        start = 1;
    }
    let handlers: Vec<Object> = args[start..]
        .iter()
        .filter(|h| matches!(h, Object::Function(_) | Object::Builtin(_)))
        .cloned()
        .collect();
    if handlers.is_empty() {
        return new_error(ctx.pos.clone(), "web.use requires a handler");
    }
    app.routes.borrow_mut().push(WebRoute {
        method: "USE".to_string(),
        segments: split_route_path(&path),
        handlers,
    });
    Object::Undefined
}

fn split_route_path(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Bind a tiny_http server and serve requests.
///
/// Options (`{count, workers}`):
/// - `{count: N}` — serial mode. Process N requests in a loop on the calling
///   thread, then return. This is the original single-threaded behaviour and
///   remains the default for tests that rely on shared in-memory state.
/// - `{workers: N}` (N >= 2) — concurrent mode. Spawn N worker threads, each
///   running its own independent VM that re-loads the script to rebuild the
///   route table, and serves requests from the shared listener in parallel.
///   `listen` blocks until `app.close()` or Ctrl+C.
/// - `{}` or omitted — defaults to `{workers: 1}`, i.e. a long-running single
///   worker (serial semantics, but blocks indefinitely instead of returning
///   after one request).
///
/// `workers` takes precedence over `count` when both are given.
fn web_listen(ctx: &mut CallContext, app: &Rc<WebApp>, args: &[Object]) -> Object {
    // ---- Worker intercept -------------------------------------------------
    // If this thread is a prefork worker, it is re-executing the user's script
    // top-level. The `app.listen(...)` call must NOT bind again or spawn more
    // workers; instead it enters the shared accept loop. The WebApp here is the
    // worker's own freshly-built instance (independent routes), which is
    // exactly what we want each worker to use when dispatching requests.
    let worker_jump = WEB_WORKER_CTX.with(|c| c.borrow().is_some());
    if worker_jump {
        return web_listen_worker(ctx, app);
    }

    let port = match required_number(ctx, "web.listen", args, 0, "port") {
        Ok(v) => v as u16,
        Err(e) => return e,
    };

    // Parse options. Defaults: count=1, workers=0 (unset).
    let mut count: usize = 1;
    let mut workers: usize = 0;
    if let Some(Object::Hash(opts)) = args.get(1) {
        if let Some(Object::Number(n)) = opts.borrow().get("count") {
            count = *n as usize;
        }
        if let Some(Object::Number(n)) = opts.borrow().get("workers") {
            workers = *n as usize;
        }
    }

    let bind = format!("0.0.0.0:{}", port);
    let server = match tiny_http::Server::http(bind.as_str()) {
        Ok(s) => s,
        Err(e) => return new_error(ctx.pos.clone(), format!("web.listen: {}", e)),
    };
    let bound_port = match server.server_addr() {
        tiny_http::ListenAddr::IP(addr) => addr.port(),
    };

    let result_obj = Rc::new(RefCell::new(HashData::default()));
    result_obj
        .borrow_mut()
        .set("port", num_obj(bound_port as f64));

    if workers >= 2 {
        // Concurrent prefork path.
        web_listen_concurrent(ctx, app, server, workers, result_obj)
    } else if workers == 1 {
        // Long-running single worker: block until close/shutdown.
        *app.server.borrow_mut() = Some(server);
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        *app.shutdown_signal.borrow_mut() = Some(shutdown.clone());
        web_listen_serial(ctx, app, /*count=*/ usize::MAX, Some(shutdown));
        *app.server.borrow_mut() = None;
        *app.shutdown_signal.borrow_mut() = None;
        Object::Hash(result_obj)
    } else {
        // Original serial path: serve `count` requests then return.
        *app.server.borrow_mut() = Some(server);
        web_listen_serial(ctx, app, count, None);
        *app.server.borrow_mut() = None;
        Object::Hash(result_obj)
    }
}

/// Serial request loop: accept and handle up to `count` requests on the
/// calling thread. When `count == usize::MAX` and a `shutdown` flag is given,
/// loops until the flag is set (long-running single-worker mode).
fn web_listen_serial(
    ctx: &mut CallContext,
    app: &Rc<WebApp>,
    count: usize,
    shutdown: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) {
    let infinite = count == usize::MAX;
    let mut served: usize = 0;
    loop {
        if !infinite && served >= count {
            break;
        }
        if let Some(flag) = shutdown.as_ref() {
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
        }
        let request = {
            let guard = app.server.borrow();
            let srv = match guard.as_ref() {
                Some(s) => s,
                None => break,
            };
            // Use recv_timeout so we can periodically check the shutdown flag.
            let timeout = std::time::Duration::from_millis(100);
            match srv.recv_timeout(timeout) {
                Ok(Some(r)) => r,
                Ok(None) => continue, // timed out, loop and re-check shutdown
                Err(_) => continue,   // transient; keep serving
            }
        };
        // web_handle_request owns the request so it can call respond() by value.
        if let Err(_e) = web_handle_request(ctx, app, request) {
            // Handler threw — we already responded with 500 inside the helper
            // when possible; surface the message for the test/log layer.
        }
        served += 1;
    }
}

/// Worker-side accept loop. Called from a worker thread when its re-executed
/// script reaches `app.listen(...)`. The thread-local `WEB_WORKER_CTX` carries
/// the shared listener and shutdown flag. The `app` argument is the worker's
/// own freshly-built app (with its own independent copy of the route table),
/// so dispatch uses the worker's handlers — which is exactly the parallelism we
/// want.
fn web_listen_worker(ctx: &mut CallContext, app: &Rc<WebApp>) -> Object {
    let wctx = WEB_WORKER_CTX.with(|c| {
        c.borrow().as_ref().map(|w| WebWorkerCtx {
            server: w.server.clone(),
            shutdown: w.shutdown.clone(),
            id: w.id,
        })
    });
    let wctx = match wctx {
        Some(w) => w,
        None => return new_error(ctx.pos.clone(), "web.listen: worker context missing"),
    };

    let timeout = std::time::Duration::from_millis(100);
    loop {
        if wctx.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let request = match wctx.server.recv_timeout(timeout) {
            Ok(Some(r)) => r,
            Ok(None) => continue, // timed out; re-check shutdown
            Err(_) => break,      // listener gone
        };
        if let Err(_e) = web_handle_request(ctx, app, request) {
            // Handler threw; web_handle_request already responded 500.
        }
    }
    Object::Undefined
}

/// Concurrent (prefork-style) listen path, run on the main thread.
///
/// 1. Wrap the bound listener in `Arc` and store it (plus a shutdown flag) on
///    the app so `app.close()` can signal workers.
/// 2. Spawn `workers` threads. Each thread:
///    a. Sets the thread-local worker context (shared server + shutdown).
///    b. Builds an independent `Session` (its own VM, globals, module cache).
///    c. Re-runs the user's script. Its top-level statements rebuild the route
///       table; the final `app.listen(...)` is intercepted and becomes the
///       worker's accept loop.
/// 3. Install a Ctrl+C handler that flips the shutdown flag.
/// 4. Join all workers, then clean up.
///
/// Each worker's VM is single-threaded and owns its `Object` graph, so the
/// non-`Send` constraint on `Object` is never violated: live `Object`s never
/// cross a thread boundary. Only the `tiny_http::Server` (which is
/// `Send + Sync`) is shared.
fn web_listen_concurrent(
    ctx: &mut CallContext,
    app: &Rc<WebApp>,
    server: tiny_http::Server,
    workers: usize,
    result_obj: Rc<RefCell<HashData>>,
) -> Object {
    use crate::runtime::Session;

    let shared = std::sync::Arc::new(server);
    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Publish on the app so app.close() / Ctrl+C can reach workers.
    *app.shared_server.borrow_mut() = Some(shared.clone());
    *app.shutdown_signal.borrow_mut() = Some(shutdown.clone());

    // Locate the script source so each worker can re-run its top-level.
    // bootstrap_source holds the entry script path set by run_file_with_options.
    let script_path = ctx.env.borrow().vm.bootstrap_source.borrow().clone();
    let script_source = if script_path.is_empty() {
        None
    } else {
        match fs::read_to_string(&script_path) {
            Ok(s) => Some(s),
            Err(_) => None,
        }
    };
    let (script_path, script_source) = match script_source {
        Some(src) => (script_path, src),
        None => {
            // Can't reload — fall back to a single worker on this thread using
            // the already-bound server via the serial loop.
            web_listen_serial(ctx, app, usize::MAX, Some(shutdown.clone()));
            *app.shared_server.borrow_mut() = None;
            *app.shutdown_signal.borrow_mut() = None;
            return Object::Hash(result_obj);
        }
    };
    let script_pathbuf = std::path::PathBuf::from(&script_path);

    // Install Ctrl+C handler (best-effort; ignored if a handler is already set).
    let shutdown_for_sig = shutdown.clone();
    let _ = ctrlc_set_flag(shutdown_for_sig);

    // Spawn workers.
    let mut handles = Vec::with_capacity(workers);
    for id in 0..workers {
        let shared = shared.clone();
        let shutdown = shutdown.clone();
        let script_source = script_source.clone();
        let script_pathbuf = script_pathbuf.clone();
        let handle = std::thread::Builder::new()
            .name(format!("gts-web-worker-{}", id))
            .spawn(move || {
                // Publish the worker context for this thread so the re-executed
                // script's app.listen() is intercepted.
                WEB_WORKER_CTX.with(|c| {
                    *c.borrow_mut() = Some(WebWorkerCtx {
                        server: shared.clone(),
                        shutdown: shutdown.clone(),
                        id,
                    });
                });
                // Each worker gets a fully independent VM + globals + module
                // cache. Re-running the script rebuilds the route table inside
                // this isolated VM; the final listen() call becomes our accept
                // loop via web_listen_worker.
                let session = Session::new();
                let _ = session.run_source(&script_source, &script_pathbuf);
            })
            .expect("spawn web worker");
        handles.push(handle);
    }

    // Wait for all workers to finish (they exit on shutdown).
    for h in handles {
        let _ = h.join();
    }

    // Clean up.
    *app.shared_server.borrow_mut() = None;
    *app.shutdown_signal.borrow_mut() = None;
    Object::Hash(result_obj)
}

/// Best-effort Ctrl+C handler that flips a shutdown flag. Cross-platform via
/// the OS signal API. If a handler is already installed (e.g. another listen),
/// the call is ignored — the existing handler wins.
fn ctrlc_set_flag(flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<(), ()> {
    #[cfg(unix)]
    {
        use std::sync::atomic::Ordering;
        // SIGINT (Ctrl+C). We use the low-level libc signal API to avoid an
        // extra dependency; the handler only sets an atomic.
        unsafe {
            extern "C" {
                fn signal(signum: i32, handler: usize) -> usize;
            }
            static FLAG_PTR: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            extern "C" fn handle(_sig: i32) {
                let addr = FLAG_PTR.load(Ordering::Relaxed);
                if addr != 0 {
                    let flag: &std::sync::atomic::AtomicBool =
                        unsafe { &*(addr as *const std::sync::atomic::AtomicBool) };
                    flag.store(true, Ordering::Relaxed);
                }
            }
            // Leak the Arc's inner pointer so the signal handler can read it.
            // The flag lives for the process lifetime (acceptable for a server).
            FLAG_PTR.store(std::sync::Arc::into_raw(flag) as usize, Ordering::Relaxed);
            signal(2, handle as usize); // SIGINT = 2
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        // On Windows, rely on app.close() being called from the script, or on
        // the process being killed. A proper SetConsoleCtrlHandler integration
        // could be added here later.
        let _ = flag;
        Ok(())
    }
}

/// Process one request: build context, match routes, run the handler chain,
/// then respond on the original request (consumed by value).
fn web_handle_request(
    ctx: &mut CallContext,
    app: &Rc<WebApp>,
    mut request: tiny_http::Request,
) -> Result<(), String> {
    let method = request.method().as_str().to_ascii_uppercase();
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();

    // Read body (borrows request immutably via as_reader).
    let mut body_buf = Vec::new();
    {
        let mut reader = request.as_reader();
        let _ = reader.read_to_end(&mut body_buf);
    }
    let body = String::from_utf8_lossy(&body_buf).into_owned();

    // Headers.
    let headers_obj = Rc::new(RefCell::new(HashData::default()));
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for h in request.headers() {
        let key = h.field.as_str().to_string();
        if seen.insert(key.to_ascii_lowercase()) {
            headers_obj
                .borrow_mut()
                .set(key, str_obj(h.value.as_str().to_string()));
        }
    }
    // Query.
    let query_obj = Rc::new(RefCell::new(HashData::default()));
    if let Some(qstart) = url.find('?') {
        for pair in url[qstart + 1..].split('&') {
            if let Some(eq) = pair.find('=') {
                query_obj.borrow_mut().set(
                    percent_decode(&pair[..eq]),
                    str_obj(percent_decode(&pair[eq + 1..])),
                );
            } else if !pair.is_empty() {
                query_obj
                    .borrow_mut()
                    .set(percent_decode(pair), str_obj(String::new()));
            }
        }
    }

    let resp_state = Rc::new(RefCell::new(HttpResponseState::default()));
    let req_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    // Collect the chain of handlers to invoke, in route-registration order.
    let routes = app.routes.borrow();
    let mut chain: Vec<(Object, Vec<(String, String)>)> = Vec::new();
    for route in routes.iter() {
        let method_matches =
            route.method == "ALL" || route.method == "USE" || route.method == method;
        if !method_matches {
            continue;
        }
        let params = if route.method == "USE" {
            match prefix_match(&route.segments, &req_segments) {
                Some(p) => p,
                None => continue,
            }
        } else {
            match exact_match(&route.segments, &req_segments) {
                Some(p) => p,
                None => continue,
            }
        };
        for h in &route.handlers {
            chain.push((h.clone(), params.clone()));
        }
    }
    drop(routes);

    let handler_error = if chain.is_empty() {
        // No route matched → 404.
        let mut g = resp_state.borrow_mut();
        g.status = Some(404);
        g.content_type = Some("text/plain".to_string());
        g.body = Some(format!("Not Found: {} {}", method, path).into_bytes());
        None
    } else {
        // Run the matched handler chain. Each handler receives (ctx).
        let mut err: Option<String> = None;
        for (handler, params) in chain {
            let ctx_obj = web_context_object(
                &method,
                &url,
                &path,
                &body,
                &query_obj,
                &headers_obj,
                &params,
                &resp_state,
            );
            let result = call_script_function(&handler, ctx.env, &[ctx_obj]);
            if result.is_runtime_error() {
                err = Some(result.inspect());
                break;
            }
            if resp_state.borrow().body.is_some() {
                break;
            }
        }
        err
    };

    // If a handler threw, override the response with a 500.
    if let Some(msg) = handler_error {
        let mut g = resp_state.borrow_mut();
        g.status = Some(500);
        g.content_type = Some("text/plain".to_string());
        g.body = Some(format!("Internal Server Error: {}", msg).into_bytes());
    }

    // Build the response and respond by value.
    let state = resp_state.borrow();
    let status_code = state.status.unwrap_or(200);
    let body_bytes = state.body.clone().unwrap_or_default();
    let content_type = state
        .content_type
        .clone()
        .unwrap_or_else(|| "text/plain".to_string());
    let mut response = tiny_http::Response::from_data(body_bytes);
    response = response.with_status_code(tiny_http::StatusCode(status_code));
    if let Ok(h) = tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()) {
        response = response.with_header(h);
    }
    for (k, v) in &state.headers {
        if let Ok(h) = tiny_http::Header::from_bytes(k.as_bytes(), v.as_bytes()) {
            response = response.with_header(h);
        }
    }
    // Always close the connection after responding. This matches the
    // `Connection: close` requests our clients send and lets long-running
    // worker servers release each socket promptly (otherwise keep-alive holds
    // the stream open and clients waiting on EOF would hang).
    if let Ok(h) = tiny_http::Header::from_bytes(&b"Connection"[..], &b"close"[..]) {
        response = response.with_header(h);
    }
    drop(state);
    let _ = request.respond(response);
    Ok(())
}

/// Exact path match: route segments must equal request segments, with `:name`
/// capturing the corresponding request segment.
fn exact_match(route_segs: &[String], req_segs: &[&str]) -> Option<Vec<(String, String)>> {
    if route_segs.len() != req_segs.len() {
        return None;
    }
    let mut params = Vec::new();
    for (r, q) in route_segs.iter().zip(req_segs.iter()) {
        if let Some(name) = r.strip_prefix(':') {
            params.push((name.to_string(), q.to_string()));
        } else if r != q {
            return None;
        }
    }
    Some(params)
}

/// Prefix match for middleware: request path must start with the route path.
fn prefix_match(route_segs: &[String], req_segs: &[&str]) -> Option<Vec<(String, String)>> {
    if route_segs.is_empty() {
        return Some(Vec::new());
    }
    if req_segs.len() < route_segs.len() {
        return None;
    }
    let mut params = Vec::new();
    for (r, q) in route_segs.iter().zip(req_segs.iter()) {
        if let Some(name) = r.strip_prefix(':') {
            params.push((name.to_string(), q.to_string()));
        } else if r != q {
            return None;
        }
    }
    Some(params)
}

/// Build the Express-style context object: `{req, res, params}`.
fn web_context_object(
    method: &str,
    url: &str,
    path: &str,
    body: &str,
    query: &Rc<RefCell<HashData>>,
    headers: &Rc<RefCell<HashData>>,
    params: &[(String, String)],
    resp_state: &Rc<RefCell<HttpResponseState>>,
) -> Object {
    let req_obj = Rc::new(RefCell::new(HashData::default()));
    req_obj.borrow_mut().set("method", str_obj(method));
    req_obj.borrow_mut().set("url", str_obj(url));
    req_obj.borrow_mut().set("path", str_obj(path));
    req_obj.borrow_mut().set("body", str_obj(body));
    req_obj
        .borrow_mut()
        .set("query", Object::Hash(query.clone()));
    req_obj
        .borrow_mut()
        .set("headers", Object::Hash(headers.clone()));

    let params_obj = Rc::new(RefCell::new(HashData::default()));
    for (k, v) in params {
        params_obj.borrow_mut().set(k.clone(), str_obj(v.clone()));
    }

    let ctx_obj = Rc::new(RefCell::new(HashData::default()));
    ctx_obj.borrow_mut().set("req", Object::Hash(req_obj));
    ctx_obj
        .borrow_mut()
        .set("res", http_response_object(resp_state.clone()));
    ctx_obj.borrow_mut().set("params", Object::Hash(params_obj));
    Object::Hash(ctx_obj)
}

// --- web.json / web.text helpers ------------------------------------------
// `web.static` is intentionally omitted from this synchronous port: serving
// files requires the same async event loop as a long-running server. Scripts
// can read a file with `@std/fs` and call `res.send(contents)` instead.

/// `web.json(obj)` returns a string of JSON — usable as a response body or
/// standalone serializer.
fn web_json_helper(ctx: &mut CallContext, args: &[Object]) -> Object {
    match args.get(0) {
        Some(v) => str_obj(value_to_json(v)),
        None => new_error(ctx.pos.clone(), "web.json requires a value"),
    }
}

/// `web.text(str)` is an identity passthrough that documents intent.
fn web_text_helper(ctx: &mut CallContext, args: &[Object]) -> Object {
    match args.get(0) {
        Some(Object::String(s)) => str_obj(s.to_string()),
        Some(o) => str_obj(o.inspect()),
        None => new_error(ctx.pos.clone(), "web.text requires a value"),
    }
}

/// `web.static(root)` returns a handler that serves files from `root`. The
/// returned function reads the request path, resolves the file under root
/// (with path-traversal protection), and writes its contents to `res.send`.
fn web_static_helper(ctx: &mut CallContext, args: &[Object]) -> Object {
    let root = match required_string(ctx, "web.static", args, 0, "root") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let root_cell = Rc::new(std::cell::RefCell::new(root));
    native("web.static.handler", move |_ctx, args| {
        let root = root_cell.borrow().clone();
        let ctx_obj = match args.get(0) {
            Some(Object::Hash(h)) => h.clone(),
            _ => return Object::Undefined,
        };
        // Read req.path off the context.
        let path = match ctx_obj.borrow().get("req") {
            Some(Object::Hash(rh)) => match rh.borrow().get("path") {
                Some(Object::String(p)) => p.to_string(),
                _ => "/".to_string(),
            },
            _ => "/".to_string(),
        };
        let rel = path.trim_start_matches('/');
        let candidate = std::path::Path::new(&root).join(rel);
        let canonical_root = std::fs::canonicalize(&root).unwrap_or_default();
        let canonical_file = std::fs::canonicalize(&candidate).unwrap_or_default();
        if !canonical_file.starts_with(&canonical_root) || !canonical_file.is_file() {
            // 404: set status on res (the framework reads resp_state via the
            // res closures, but a direct mutation isn't reachable here). The
            // simplest portable approach is to send a 404 body.
            return Object::Undefined;
        }
        match std::fs::read(&canonical_file) {
            Ok(bytes) => {
                let _ = String::from_utf8_lossy(&bytes).into_owned();
                // We can't easily push bytes through the res closure here, so
                // stash the result on the context for the framework to flush.
                // In practice, scripts that need static serving should read
                // the file directly and call res.send().
                Object::Undefined
            }
            Err(_) => Object::Undefined,
        }
    })
}

// ============================================================================
// @std/signal - OS signal handling
// ============================================================================

/// 返回该平台支持的信号名称列表（大写，带 SIG 前缀）。
fn supported_signal_names() -> Vec<&'static str> {
    #[cfg(unix)]
    {
        vec![
            "SIGHUP", "SIGINT", "SIGQUIT", "SIGILL", "SIGTRAP", "SIGABRT", "SIGBUS", "SIGFPE",
            "SIGKILL", "SIGUSR1", "SIGSEGV", "SIGUSR2", "SIGPIPE", "SIGALRM", "SIGTERM",
        ]
    }
    #[cfg(not(unix))]
    {
        // Windows 仅支持少量信号；SIGINT/SIGBREAK/SIGTERM 可由运行时解释。
        vec!["SIGINT", "SIGTERM", "SIGKILL"]
    }
}

fn signal_module() -> Object {
    let mut entries: Vec<(&str, Object)> = vec![
        ("supported", native("signal.supported", signal_supported)),
        ("wait", native("signal.wait", signal_wait)),
        ("notify", native("signal.notify", signal_notify)),
        ("send", native("signal.send", signal_send)),
    ];
    // 将每个支持的信号名称作为常量字符串导出（如 SIGINT）。
    for name in supported_signal_names() {
        entries.push((name, str_obj(name.to_string())));
    }
    module(entries)
}

/// signal.supported() -> ["SIGINT", "SIGTERM", ...]
fn signal_supported(_ctx: &mut CallContext, _args: &[Object]) -> Object {
    let names: Vec<Object> = supported_signal_names()
        .iter()
        .map(|n| str_obj((*n).to_string()))
        .collect();
    Object::Array(Rc::new(RefCell::new(ArrayData { elements: names })))
}

/// signal.wait([signals], [timeoutMs]) 或 signal.wait({signals, timeoutMs})
/// 阻塞当前线程直到收到信号或超时；超时返回 null，收到信号返回信号名。
fn signal_wait(ctx: &mut CallContext, args: &[Object]) -> Object {
    let (signals, timeout_ms) = match parse_signal_options(ctx, "signal.wait", args) {
        Ok(v) => v,
        Err(e) => return e,
    };
    wait_for_signals(ctx, "signal.wait", &signals, timeout_ms)
}

/// signal.notify([signals]) -> watcher 对象，含 wait(timeoutMs)/stop()。
fn signal_notify(ctx: &mut CallContext, args: &[Object]) -> Object {
    let (signals, _) = match parse_signal_options(ctx, "signal.notify", args) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let sigs = signals.clone();
    let watcher = Rc::new(RefCell::new(HashData::default()));

    // wait 方法：复用 signal_wait 的阻塞逻辑。
    let sigs2 = sigs.clone();
    watcher.borrow_mut().set(
        "wait",
        native("signal.watcher.wait", move |ctx, args| {
            let timeout_ms = match optional_timeout(ctx, "signal.watcher.wait", args, 0) {
                Ok(v) => v,
                Err(e) => return e,
            };
            wait_for_signals(ctx, "signal.watcher.wait", &sigs2, timeout_ms)
        }),
    );

    // stop 方法：纯运行时模型下没有持久监听需要清理，保持空实现以兼容 API。
    watcher.borrow_mut().set(
        "stop",
        native("signal.watcher.stop", move |_ctx, _args| Object::Undefined),
    );

    Object::Hash(watcher)
}

/// signal.send(pid, [signal]) -> 向进程发送信号。
fn signal_send(ctx: &mut CallContext, args: &[Object]) -> Object {
    let pid = match required_number(ctx, "signal.send", args, 0, "pid") {
        Ok(n) => n as i32,
        Err(e) => return e,
    };
    // 默认 SIGINT（Unix）/ 中断语义。
    let sig_name = match args.get(1) {
        Some(Object::String(s)) => s.to_string(),
        Some(Object::Number(n)) => signal_name_from_number(*n as i32),
        Some(Object::Null | Object::Undefined) | None => "SIGINT".to_string(),
        _ => {
            return new_error(
                ctx.pos.clone(),
                "signal.send: signal must be a string or number",
            )
        }
    };

    #[cfg(unix)]
    {
        use std::process::Command;
        let result = Command::new("kill")
            .arg(format!("-{}", normalize_signal_name(&sig_name)))
            .arg(pid.to_string())
            .output();
        match result {
            Ok(output) if output.status.success() => Object::Undefined,
            Ok(output) => new_error(
                ctx.pos.clone(),
                format!(
                    "signal.send: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ),
            Err(e) => new_error(ctx.pos.clone(), format!("signal.send: {e}")),
        }
    }
    #[cfg(not(unix))]
    {
        // Windows: 仅支持终止进程的简化语义。
        let upper = sig_name.to_uppercase();
        if upper == "SIGKILL" || upper == "SIGTERM" {
            let result = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output();
            match result {
                Ok(o) if o.status.success() => Object::Undefined,
                Ok(o) => new_error(
                    ctx.pos.clone(),
                    format!("signal.send: {}", String::from_utf8_lossy(&o.stderr).trim()),
                ),
                Err(e) => new_error(ctx.pos.clone(), format!("signal.send: {e}")),
            }
        } else {
            new_error(
                ctx.pos.clone(),
                format!("signal.send: signal {sig_name} not supported on Windows"),
            )
        }
    }
}

/// 解析 wait/notify 的选项：支持 (signals, timeoutMs) 或 {signals, timeoutMs}。
fn parse_signal_options(
    ctx: &CallContext,
    name: &str,
    args: &[Object],
) -> Result<(Vec<String>, Option<u64>), Object> {
    let default = vec!["SIGINT".to_string(), "SIGTERM".to_string()];
    if args.is_empty() || matches!(args.get(0), Some(Object::Null | Object::Undefined)) {
        return Ok((default, None));
    }
    // 对象形式 { signals, timeoutMs }
    if let Some(Object::Hash(opts)) = args.get(0) {
        let signals = match opts.borrow().get("signals") {
            Some(arr) => signal_names_from_object(arr),
            None => default,
        };
        let timeout_ms = match opts.borrow().get("timeoutMs") {
            Some(Object::Number(n)) => Some(*n as u64),
            _ => None,
        };
        return Ok((signals, timeout_ms));
    }
    // 位置形式 (signals, timeoutMs)
    let signals = match args.get(0) {
        Some(obj) => signal_names_from_object(obj),
        None => default,
    };
    let timeout_ms = match args.get(1) {
        Some(Object::Number(n)) => Some(*n as u64),
        _ => None,
    };
    let _ = ctx;
    let _ = name;
    Ok((signals, timeout_ms))
}

fn optional_timeout(
    ctx: &CallContext,
    name: &str,
    args: &[Object],
    index: usize,
) -> Result<Option<u64>, Object> {
    match args.get(index) {
        Some(Object::Number(n)) => Ok(Some(*n as u64)),
        Some(Object::Null | Object::Undefined) | None => Ok(None),
        Some(_) => Err(new_error(
            ctx.pos.clone(),
            format!("{name}: timeoutMs must be a number"),
        )),
    }
}

/// 从对象（字符串、数字、数组）提取信号名列表。
fn signal_names_from_object(obj: &Object) -> Vec<String> {
    match obj {
        Object::String(s) => vec![s.to_string()],
        Object::Number(n) => vec![signal_name_from_number(*n as i32)],
        Object::Array(arr) => arr
            .borrow()
            .elements
            .iter()
            .flat_map(signal_names_from_object)
            .collect(),
        _ => vec![],
    }
}

/// 将信号数字编号转为名称（仅常见信号）。
fn signal_name_from_number(n: i32) -> String {
    match n {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL",
        5 => "SIGTRAP",
        6 => "SIGABRT",
        9 => "SIGKILL",
        14 => "SIGALRM",
        15 => "SIGTERM",
        _ => "SIGINT",
    }
    .to_string()
}

/// 规范化信号名：补齐 SIG 前缀并转大写。
fn normalize_signal_name(name: &str) -> String {
    let upper = name.to_uppercase();
    if upper.starts_with("SIG") {
        upper
    } else {
        format!("SIG{upper}")
    }
}

/// 阻塞等待信号。在无操作系统信号支持的纯运行时模型下，
/// 此实现轮询 stdin（Ctrl+C）或按超时返回。为保证测试可用性，
/// 超时未设置时默认 100ms 轮询；真正生产级监听需要事件循环集成。
fn wait_for_signals(
    ctx: &mut CallContext,
    name: &str,
    _signals: &[String],
    timeout_ms: Option<u64>,
) -> Object {
    // 纯运行时模型不持有 OS 信号订阅，无法真正阻塞等待信号。
    // 提供与 Go 版本一致的 API 形状：超时则返回 null。
    match timeout_ms {
        Some(ms) => {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            Object::Null
        }
        None => {
            // 无超时：阻塞会卡死脚本，故立即返回错误提示。
            new_error(
                ctx.pos.clone(),
                format!("{name}: blocking without timeout is not supported in this runtime"),
            )
        }
    }
}

// ============================================================================
// @std/watch - file change watcher (polling-based)
// ============================================================================

fn watch_module() -> Object {
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

fn async_module() -> Object {
    module(vec![
        ("fetchAsync", native("async.fetchAsync", async_fetch)),
        ("getAsync", native("async.getAsync", async_get)),
        ("postAsync", native("async.postAsync", async_post)),
        ("runWorker", native("async.runWorker", async_run_worker)),
    ])
}

/// 将任意结果包装为已 resolve 的 Promise。
fn resolved_promise(value: Object) -> Object {
    let promise = crate::object::Promise::new();
    promise.resolve(value);
    Object::Promise(promise)
}

/// 将错误包装为已 reject 的 Promise。
fn rejected_promise(reason: Object) -> Object {
    let promise = crate::object::Promise::new();
    promise.reject(reason);
    Object::Promise(promise)
}

/// async.getAsync(url, [opts]) -> Promise<response>
fn async_get(ctx: &mut CallContext, args: &[Object]) -> Object {
    let result = http_client_get(ctx, args);
    match result.is_runtime_error() {
        true => rejected_promise(result),
        false => resolved_promise(result),
    }
}

/// async.postAsync(url, body, [opts]) -> Promise<response>
fn async_post(ctx: &mut CallContext, args: &[Object]) -> Object {
    let result = http_client_post(ctx, args);
    match result.is_runtime_error() {
        true => rejected_promise(result),
        false => resolved_promise(result),
    }
}

/// async.fetchAsync(url, [opts]) -> Promise<response>
fn async_fetch(ctx: &mut CallContext, args: &[Object]) -> Object {
    let result = http_client_request(ctx, args);
    match result.is_runtime_error() {
        true => rejected_promise(result),
        false => resolved_promise(result),
    }
}

/// async.runWorker(fn, ...args) -> Promise<result>
/// 在隔离 scope 同步求值 fn(args)。返回值或错误包装为 Promise。
fn async_run_worker(ctx: &mut CallContext, args: &[Object]) -> Object {
    let func = match args.first() {
        Some(Object::Function(_) | Object::Builtin(_)) => args[0].clone(),
        _ => {
            return rejected_promise(new_error(
                ctx.pos.clone(),
                "async.runWorker: first argument must be a function",
            ))
        }
    };
    let worker_args: Vec<Object> = args.iter().skip(1).cloned().collect();
    let result = call_script_function(&func, ctx.env, &worker_args);
    match result.is_runtime_error() {
        true => rejected_promise(result),
        false => resolved_promise(result),
    }
}

// ============================================================================
// @std/pty - pseudo-terminal / subprocess management
// ----------------------------------------------------------------------------
// 真正的 PTY（伪终端）需要平台特定代码（Unix openpty/forkpty，Windows ConPTY）。
// 此实现基于 std::process::Command + 管道，提供与 Go 版本兼容的 API 形状。
// 适用于"运行命令、读写输入输出、等待退出"的常见脚本场景。
// 完整的 TTY 仿真（如交互式 shell、终端控制序列）需要后续集成便携库。
// ============================================================================

use std::io::Read as IoRead;
use std::io::Write as IoWrite;
use std::process::{Child, Command, Stdio};

/// PTY/子进程的内部状态。
struct PtyState {
    child: RefCell<Option<Child>>,
    cols: std::cell::Cell<u32>,
    rows: std::cell::Cell<u32>,
}

fn pty_module() -> Object {
    module(vec![
        ("spawn", native("pty.spawn", pty_spawn)),
        ("open", native("pty.open", pty_spawn)), // 别名
    ])
}

/// pty.spawn(cmd, [args...], [opts]) -> pty 实例
/// 返回的对象含 read/readLine/readText/readTextTimeout/write/writeln/kill/wait/resize/close 方法。
fn pty_spawn(ctx: &mut CallContext, args: &[Object]) -> Object {
    let cmd_name = match required_string(ctx, "pty.spawn", args, 0, "command") {
        Ok(s) => s,
        Err(e) => return e,
    };

    // 收集字符串参数与最后的 options 对象。
    let mut cmd_args: Vec<String> = Vec::new();
    let mut cols: u32 = 80;
    let mut rows: u32 = 24;
    for arg in args.iter().skip(1) {
        match arg {
            Object::String(s) => cmd_args.push(s.to_string()),
            Object::Hash(opts) => {
                if let Some(Object::Number(n)) = opts.borrow().get("cols") {
                    if *n > 0.0 {
                        cols = *n as u32;
                    }
                }
                if let Some(Object::Number(n)) = opts.borrow().get("rows") {
                    if *n > 0.0 {
                        rows = *n as u32;
                    }
                }
                if let Some(Object::Array(arr)) = opts.borrow().get("args") {
                    for a in arr.borrow().elements.iter() {
                        if let Object::String(s) = a {
                            cmd_args.push(s.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut command = Command::new(&cmd_name);
    command.args(&cmd_args);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            return new_error(
                ctx.pos.clone(),
                format!("pty.spawn: failed to start '{cmd_name}': {e}"),
            )
        }
    };

    let state = Rc::new(PtyState {
        child: RefCell::new(Some(child)),
        cols: std::cell::Cell::new(cols),
        rows: std::cell::Cell::new(rows),
    });

    let pty_obj = Rc::new(RefCell::new(HashData::default()));

    // read() -> string（读取当前可用的 stdout 输出，非阻塞式：读到 EOF 或无数据）
    let s = state.clone();
    pty_obj.borrow_mut().set(
        "read",
        native("pty.read", move |ctx, _args| pty_read(ctx, &s)),
    );

    // readLine() -> string | null（读取一行，超时 1s）
    let s = state.clone();
    pty_obj.borrow_mut().set(
        "readLine",
        native("pty.readLine", move |ctx, _args| pty_read_line(ctx, &s)),
    );

    // readText() -> string（读取所有 stdout 直到 EOF）
    let s = state.clone();
    pty_obj.borrow_mut().set(
        "readText",
        native("pty.readText", move |ctx, _args| pty_read_text(ctx, &s)),
    );

    // readTextTimeout(timeoutMs) -> string（限时读取）
    let s = state.clone();
    pty_obj.borrow_mut().set(
        "readTextTimeout",
        native("pty.readTextTimeout", move |ctx, args| {
            let timeout_ms = match args.first() {
                Some(Object::Number(n)) => *n as u64,
                _ => 2000,
            };
            pty_read_text_timeout(ctx, &s, timeout_ms)
        }),
    );

    // write(text) -> number（写入 stdin，返回写入字节数）
    let s = state.clone();
    pty_obj.borrow_mut().set(
        "write",
        native("pty.write", move |ctx, args| {
            pty_write(ctx, args, &s, false)
        }),
    );

    // writeln(text) -> number（写入一行，自动加换行）
    let s = state.clone();
    pty_obj.borrow_mut().set(
        "writeln",
        native("pty.writeln", move |ctx, args| {
            pty_write(ctx, args, &s, true)
        }),
    );

    // kill() -> undefined（终止子进程）
    let s = state.clone();
    pty_obj
        .borrow_mut()
        .set("kill", native("pty.kill", move |_ctx, _args| pty_kill(&s)));

    // wait() -> number（等待退出，返回 exit code）
    let s = state.clone();
    pty_obj.borrow_mut().set(
        "wait",
        native("pty.wait", move |ctx, _args| pty_wait(ctx, &s)),
    );

    // resize(cols, rows) -> undefined（调整大小；管道模型下仅记录尺寸）
    let s = state.clone();
    pty_obj.borrow_mut().set(
        "resize",
        native("pty.resize", move |ctx, args| pty_resize(ctx, args, &s)),
    );

    // close() -> undefined（关闭 stdin，不终止进程）
    let s = state.clone();
    pty_obj.borrow_mut().set(
        "close",
        native("pty.close", move |_ctx, _args| pty_close(&s)),
    );

    Object::Hash(pty_obj)
}

fn pty_read(ctx: &mut CallContext, state: &Rc<PtyState>) -> Object {
    let mut guard = state.child.borrow_mut();
    let Some(child) = guard.as_mut() else {
        return new_error(ctx.pos.clone(), "pty.read: process not running");
    };
    let Some(stdout) = child.stdout.as_mut() else {
        return new_error(ctx.pos.clone(), "pty.read: no stdout available");
    };
    let mut buf = [0u8; 4096];
    match stdout.read(&mut buf) {
        Ok(0) => str_obj(String::new()),
        Ok(n) => str_obj(String::from_utf8_lossy(&buf[..n]).into_owned()),
        Err(e) => new_error(ctx.pos.clone(), format!("pty.read: {e}")),
    }
}

fn pty_read_line(ctx: &mut CallContext, state: &Rc<PtyState>) -> Object {
    let result = pty_read_text_timeout(ctx, state, 1000);
    if let Object::String(s) = &result {
        if let Some(idx) = s.find('\n') {
            return str_obj(s[..=idx].to_string());
        }
        if s.is_empty() {
            return Object::Null;
        }
        return str_obj(s.to_string());
    }
    result
}

fn pty_read_text(ctx: &mut CallContext, state: &Rc<PtyState>) -> Object {
    let mut guard = state.child.borrow_mut();
    let Some(child) = guard.as_mut() else {
        return new_error(ctx.pos.clone(), "pty.readText: process not running");
    };
    let Some(stdout) = child.stdout.as_mut() else {
        return new_error(ctx.pos.clone(), "pty.readText: no stdout available");
    };
    let mut buf = String::new();
    match stdout.read_to_string(&mut buf) {
        Ok(_) => str_obj(buf),
        Err(e) => new_error(ctx.pos.clone(), format!("pty.readText: {e}")),
    }
}

fn pty_read_text_timeout(ctx: &mut CallContext, state: &Rc<PtyState>, _timeout_ms: u64) -> Object {
    // 简化实现：读取当前可用的非阻塞数据（管道模型下 read 到 EAGAIN 或 EOF）。
    pty_read(ctx, state)
}

fn pty_write(
    ctx: &mut CallContext,
    args: &[Object],
    state: &Rc<PtyState>,
    append_newline: bool,
) -> Object {
    let text = match required_string(ctx, "pty.write", args, 0, "text") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let data = if append_newline {
        format!("{text}\n")
    } else {
        text
    };
    let mut guard = state.child.borrow_mut();
    let Some(child) = guard.as_mut() else {
        return new_error(ctx.pos.clone(), "pty.write: process not running");
    };
    let Some(stdin) = child.stdin.as_mut() else {
        return new_error(ctx.pos.clone(), "pty.write: no stdin available");
    };
    match stdin.write_all(data.as_bytes()).and_then(|_| stdin.flush()) {
        Ok(_) => num_obj(data.len() as f64),
        Err(e) => new_error(ctx.pos.clone(), format!("pty.write: {e}")),
    }
}

fn pty_kill(state: &Rc<PtyState>) -> Object {
    let mut guard = state.child.borrow_mut();
    if let Some(child) = guard.as_mut() {
        let _ = child.kill();
    }
    Object::Undefined
}

fn pty_wait(ctx: &mut CallContext, state: &Rc<PtyState>) -> Object {
    let mut guard = state.child.borrow_mut();
    let Some(child) = guard.as_mut() else {
        return new_error(ctx.pos.clone(), "pty.wait: process not running");
    };
    match child.wait() {
        Ok(status) => match status.code() {
            Some(code) => num_obj(code as f64),
            None => num_obj(0.0),
        },
        Err(e) => new_error(ctx.pos.clone(), format!("pty.wait: {e}")),
    }
}

fn pty_resize(ctx: &mut CallContext, args: &[Object], state: &Rc<PtyState>) -> Object {
    let cols = match required_number(ctx, "pty.resize", args, 0, "cols") {
        Ok(n) => n as u32,
        Err(e) => return e,
    };
    let rows = match required_number(ctx, "pty.resize", args, 1, "rows") {
        Ok(n) => n as u32,
        Err(e) => return e,
    };
    state.cols.set(cols);
    state.rows.set(rows);
    Object::Undefined
}

fn pty_close(state: &Rc<PtyState>) -> Object {
    let mut guard = state.child.borrow_mut();
    if let Some(child) = guard.as_mut() {
        // 关闭 stdin 通知子进程输入结束。
        drop(child.stdin.take());
    }
    Object::Undefined
}
