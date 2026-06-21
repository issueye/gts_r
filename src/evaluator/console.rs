//! The `console` global object and its methods.

use std::io::Write;
use std::rc::Rc;
use std::sync::Mutex;

use crate::object::*;

/// Build the `console` global object.
pub fn console_object() -> Object {
    let hash = Rc::new(std::cell::RefCell::new(HashData::default()));
    let make = |name: &str, to_stderr: bool, prefix: &str| -> (String, Object) {
        let prefix = prefix.to_string();
        let func: BuiltinFn = Rc::new(move |_ctx, args| {
            let parts: Vec<String> = args.iter().map(|a| a.inspect()).collect();
            let line = if prefix.is_empty() {
                parts.join(" ")
            } else {
                format!("{}{}", prefix, parts.join(" "))
            };
            print_line(&line, to_stderr);
            Object::Undefined
        });
        (
            name.into(),
            Object::Builtin(Rc::new(Builtin {
                name: format!("console.{}", name),
                func,
                extra: None,
            })),
        )
    };
    {
        let mut h = hash.borrow_mut();
        for (name, val) in [
            make("log", false, ""),
            make("info", false, "[INFO] "),
            make("warn", true, "[WARN] "),
            make("error", true, "[ERROR] "),
            make("debug", false, "[DEBUG] "),
            make("trace", true, ""),
        ] {
            h.set(name, val);
        }
    }
    Object::Hash(hash)
}

static STDOUT_LOCK: Mutex<()> = Mutex::new(());

fn print_line(line: &str, to_stderr: bool) {
    let _g = STDOUT_LOCK.lock().unwrap();
    if to_stderr {
        let _ = writeln!(std::io::stderr(), "{}", line);
    } else {
        let _ = writeln!(std::io::stdout(), "{}", line);
        let _ = std::io::stdout().flush();
    }
}
