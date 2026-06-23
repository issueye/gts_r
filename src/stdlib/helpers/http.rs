use super::*;

/// Accumulated response state, mutated by web/http handlers via closures.
#[derive(Default)]
pub(crate) struct HttpResponseState {
    pub(crate) status: Option<u16>,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) content_type: Option<String>,
    pub(crate) body: Option<Vec<u8>>,
}

pub(crate) fn http_response_object(state: Rc<RefCell<HttpResponseState>>) -> Object {
    let obj = Rc::new(RefCell::new(HashData::default()));

    let s = state.clone();
    let out = obj.clone();
    obj.borrow_mut().set(
        "status",
        native("response.status", move |_ctx, args| {
            if let Some(Object::Number(n)) = args.get(0) {
                s.borrow_mut().status = Some(*n as u16);
            }
            Object::Hash(out.clone())
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
        "write",
        native("response.write", move |_ctx, args| {
            let text = match args.get(0) {
                Some(Object::String(v)) => v.to_string(),
                Some(o) => o.inspect(),
                None => String::new(),
            };
            let mut g = s.borrow_mut();
            if g.content_type.is_none() {
                g.content_type = Some("text/plain".to_string());
            }
            g.body
                .get_or_insert_with(Vec::new)
                .extend_from_slice(text.as_bytes());
            Object::Undefined
        }),
    );
    let s = state.clone();
    obj.borrow_mut().set(
        "stream",
        native("response.stream", move |ctx, args| {
            let Some(stream) = args.get(0).cloned() else {
                return Object::Undefined;
            };
            let mut text = String::new();
            if let Object::Hash(h) = &stream {
                if let Some(read_all) = h.borrow().get("readAll").cloned() {
                    let result = call_script_function(&read_all, ctx.env, &[]);
                    if result.is_runtime_error() {
                        return result;
                    }
                    text = match result {
                        Object::String(v) => v.to_string(),
                        Object::Null | Object::Undefined => String::new(),
                        other => other.inspect(),
                    };
                } else if let Some(read_text) = h.borrow().get("readText").cloned() {
                    loop {
                        let result = call_script_function(&read_text, ctx.env, &[]);
                        if result.is_runtime_error() {
                            return result;
                        }
                        match result {
                            Object::String(v) => text.push_str(&v),
                            Object::Null | Object::Undefined => break,
                            other => {
                                text.push_str(&other.inspect());
                                break;
                            }
                        }
                    }
                } else if let Some(Object::String(v)) = h.borrow().get("text") {
                    text = v.to_string();
                }
            } else {
                text = stream.inspect();
            }
            let mut g = s.borrow_mut();
            if g.content_type.is_none() {
                g.content_type = Some("application/octet-stream".to_string());
            }
            g.body
                .get_or_insert_with(Vec::new)
                .extend_from_slice(text.as_bytes());
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
pub(crate) fn hash_to_json(h: &HashData) -> String {
    let pairs: Vec<String> = h
        .entries
        .iter()
        .map(|(k, v)| format!("{}: {}", json_escape_string(k), value_to_json(v)))
        .collect();
    format!("{{{}}}", pairs.join(", "))
}

pub(crate) fn value_to_json(obj: &Object) -> String {
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

pub(crate) fn json_escape_string(s: &str) -> String {
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
