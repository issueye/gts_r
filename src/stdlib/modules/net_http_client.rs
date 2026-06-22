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
use super::stream::stream_from_text_object;
#[allow(unused_imports)]
use crate::ast::Position;
#[allow(unused_imports)]
use crate::object::{
    bool_obj, format_number, new_error, num_obj, str_obj, strict_equal, ArrayData, Builtin,
    CallContext, HashData, Object,
};
#[allow(unused_imports)]
use crate::VERSION;

pub(crate) fn http_client_module() -> Object {
    module(vec![
        ("get", native("http.get", http_client_get)),
        ("post", native("http.post", http_client_post)),
        ("request", native("http.request", http_client_request)),
        ("stream", native("http.stream", http_client_stream)),
        ("fetch", native("http.fetch", http_client_request)),
    ])
}

pub(crate) fn http_client_get(ctx: &mut CallContext, args: &[Object]) -> Object {
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

pub(crate) fn http_client_post(ctx: &mut CallContext, args: &[Object]) -> Object {
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

pub(crate) fn http_client_request(ctx: &mut CallContext, args: &[Object]) -> Object {
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

pub(crate) fn http_client_stream(ctx: &mut CallContext, args: &[Object]) -> Object {
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

pub(crate) fn build_http_response(response: ureq::Response) -> Object {
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

pub(crate) fn build_http_stream_response(
    response: ureq::Response,
    status_override: Option<u16>,
) -> Object {
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

pub(crate) fn http_body_to_string(obj: &Object) -> String {
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

pub(crate) fn build_http_response_with_status(
    response: ureq::Response,
    status_code: u16,
) -> Object {
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
