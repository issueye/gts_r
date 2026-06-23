use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf, MAIN_SEPARATOR, MAIN_SEPARATOR_STR};
use std::rc::Rc;
use std::sync::OnceLock;
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
    bool_obj, format_number, new_error, num_obj, str_obj, strict_equal, ArrayData,
    AsyncCompletionData, AsyncHttpResponse, Builtin, CallContext, HashData, Object, Promise,
};
#[allow(unused_imports)]
use crate::VERSION;

#[cfg(feature = "tokio")]
use reqwest::Method;

pub(crate) fn http_client_module() -> Object {
    module(vec![
        ("get", native("http.get", http_client_get)),
        ("post", native("http.post", http_client_post)),
        ("request", native("http.request", http_client_request)),
        (
            "requestAsync",
            native("http.requestAsync", http_client_request_async),
        ),
        ("stream", native("http.stream", http_client_stream)),
        (
            "streamAsync",
            native("http.streamAsync", http_client_stream_async),
        ),
        ("fetch", native("http.fetch", http_client_request)),
    ])
}

#[derive(Debug, Clone)]
struct OwnedHttpRequest {
    url: String,
    method: String,
    body: Option<String>,
    headers: Vec<(String, String)>,
    timeout_ms: Option<u64>,
}

#[cfg(feature = "tokio")]
#[derive(Debug)]
struct HttpClientState {
    runtime: tokio::runtime::Runtime,
    client: reqwest::Client,
}

#[cfg(feature = "tokio")]
static HTTP_CLIENT_STATE: OnceLock<HttpClientState> = OnceLock::new();

#[cfg(feature = "tokio")]
fn http_client_state() -> &'static HttpClientState {
    HTTP_CLIENT_STATE.get_or_init(|| HttpClientState {
        runtime: tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("gts-http-client")
            .enable_all()
            .build()
            .expect("build http client runtime"),
        client: reqwest::Client::builder()
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(8)
            .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
            .build()
            .expect("build reqwest client"),
    })
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
    let request = match owned_http_request_from_args(ctx, "http.request", args) {
        Ok(request) => request,
        Err(err) => return err,
    };

    match perform_owned_http_request(request) {
        Ok(response) => async_http_response_to_object(response),
        Err(e) => new_error(ctx.pos.clone(), format!("http.request: {}", e)),
    }
}

pub(crate) fn http_client_request_async(ctx: &mut CallContext, args: &[Object]) -> Object {
    let request = match owned_http_request_from_args(ctx, "http.requestAsync", args) {
        Ok(request) => request,
        Err(err) => {
            let promise = Promise::new();
            promise.reject(err);
            return Object::Promise(promise);
        }
    };

    let vm = ctx.vm();
    let (id, promise) = vm.create_async_completion_promise();
    let sender = vm.async_completion_sender();
    #[cfg(feature = "tokio")]
    {
        let state = http_client_state();
        let client = state.client.clone();
        state.runtime.spawn(async move {
            match perform_owned_http_request_tokio(client, request).await {
                Ok(response) => sender.resolve(id, AsyncCompletionData::HttpResponse(response)),
                Err(e) => sender.reject(id, format!("http.requestAsync: {}", e)),
            }
        });
    }
    #[cfg(not(feature = "tokio"))]
    {
        std::thread::spawn(move || match perform_owned_http_request(request) {
            Ok(response) => sender.resolve(id, AsyncCompletionData::HttpResponse(response)),
            Err(e) => sender.reject(id, format!("http.requestAsync: {}", e)),
        });
    }

    Object::Promise(promise)
}

pub(crate) fn http_client_stream_async(ctx: &mut CallContext, args: &[Object]) -> Object {
    let request = match owned_http_request_from_args(ctx, "http.streamAsync", args) {
        Ok(request) => request,
        Err(err) => {
            let promise = Promise::new();
            promise.reject(err);
            return Object::Promise(promise);
        }
    };

    let vm = ctx.vm();
    let (id, promise) = vm.create_async_completion_promise();
    let sender = vm.async_completion_sender();
    #[cfg(feature = "tokio")]
    {
        let state = http_client_state();
        let client = state.client.clone();
        state.runtime.spawn(async move {
            match perform_owned_http_request_tokio(client, request).await {
                Ok(response) => {
                    sender.resolve(id, AsyncCompletionData::HttpStreamResponse(response))
                }
                Err(e) => sender.reject(id, format!("http.streamAsync: {}", e)),
            }
        });
    }
    #[cfg(not(feature = "tokio"))]
    {
        std::thread::spawn(move || match perform_owned_http_request(request) {
            Ok(response) => sender.resolve(id, AsyncCompletionData::HttpStreamResponse(response)),
            Err(e) => sender.reject(id, format!("http.streamAsync: {}", e)),
        });
    }

    Object::Promise(promise)
}

fn owned_http_request_from_args(
    ctx: &mut CallContext,
    name: &str,
    args: &[Object],
) -> Result<OwnedHttpRequest, Object> {
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
            return Err(new_error(
                ctx.pos.clone(),
                format!("{name}: requires an options object or URL string"),
            ))
        }
    };

    let url = match opts.borrow().get("url") {
        Some(Object::String(s)) => s.to_string(),
        _ => {
            return Err(new_error(
                ctx.pos.clone(),
                format!("{name}: url is required"),
            ))
        }
    };

    let method = match opts.borrow().get("method") {
        Some(Object::String(s)) => s.to_uppercase(),
        _ => "GET".to_string(),
    };

    let body = opts
        .borrow()
        .get("body")
        .map(|obj| http_body_to_string(&obj));

    let timeout_ms = match opts.borrow().get("timeoutMs") {
        Some(Object::Number(ms)) if *ms > 0.0 => Some(*ms as u64),
        _ => None,
    };

    let mut request_headers = Vec::new();
    if let Some(Object::Hash(headers_obj)) = opts.borrow().get("headers") {
        let headers_data = headers_obj.borrow();
        for (key, value) in &headers_data.entries {
            request_headers.push((key.clone(), value_to_string(value)));
        }
    }

    Ok(OwnedHttpRequest {
        url,
        method,
        body,
        headers: request_headers,
        timeout_ms,
    })
}

fn perform_owned_http_request(request: OwnedHttpRequest) -> Result<AsyncHttpResponse, String> {
    #[cfg(feature = "tokio")]
    {
        let state = http_client_state();
        let client = state.client.clone();
        return state
            .runtime
            .block_on(perform_owned_http_request_tokio(client, request));
    }
    #[cfg(not(feature = "tokio"))]
    {
        return perform_owned_http_request_ureq(request);
    }
}

#[cfg(not(feature = "tokio"))]
fn perform_owned_http_request_ureq(request: OwnedHttpRequest) -> Result<AsyncHttpResponse, String> {
    let mut req = ureq::request(&request.method, &request.url);
    for (key, value) in request.headers {
        req = req.set(&key, &value);
    }
    if let Some(timeout_ms) = request.timeout_ms {
        req = req.timeout(std::time::Duration::from_millis(timeout_ms));
    }

    let result = if let Some(body_str) = request.body {
        req.send_string(&body_str)
    } else {
        req.call()
    };

    match result {
        Ok(response) => Ok(async_http_response_from_ureq(response, None)),
        Err(ureq::Error::Status(code, response)) => {
            Ok(async_http_response_from_ureq(response, Some(code)))
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(feature = "tokio")]
async fn perform_owned_http_request_tokio(
    client: reqwest::Client,
    request: OwnedHttpRequest,
) -> Result<AsyncHttpResponse, String> {
    let method = Method::from_bytes(request.method.as_bytes()).map_err(|e| e.to_string())?;
    let mut builder = client.request(method, &request.url);

    for (key, value) in request.headers {
        builder = builder.header(key, value);
    }
    if let Some(timeout_ms) = request.timeout_ms {
        builder = builder.timeout(std::time::Duration::from_millis(timeout_ms));
    }
    if let Some(body) = request.body {
        builder = builder.body(body);
    }

    let response = builder.send().await.map_err(|e| e.to_string())?;
    async_http_response_from_reqwest(response).await
}

#[cfg(feature = "tokio")]
async fn async_http_response_from_reqwest(
    response: reqwest::Response,
) -> Result<AsyncHttpResponse, String> {
    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("")
        .to_string();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                value.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let body = response.bytes().await.map_err(|e| e.to_string())?.to_vec();

    Ok(AsyncHttpResponse {
        status,
        status_text,
        headers,
        body,
    })
}

#[cfg(not(feature = "tokio"))]
fn async_http_response_from_ureq(
    response: ureq::Response,
    status_override: Option<u16>,
) -> AsyncHttpResponse {
    let status = status_override.unwrap_or_else(|| response.status());
    let status_text = response.status_text().to_string();
    let headers = response
        .headers_names()
        .into_iter()
        .filter_map(|name| {
            response
                .header(&name)
                .map(|value| (name, value.to_string()))
        })
        .collect();
    let body = match response.into_string() {
        Ok(s) => s.into_bytes(),
        Err(_) => Vec::new(),
    };

    AsyncHttpResponse {
        status,
        status_text,
        headers,
        body,
    }
}

fn async_http_response_to_object(response: AsyncHttpResponse) -> Object {
    let headers = Rc::new(RefCell::new(HashData::default()));
    for (name, value) in response.headers {
        headers.borrow_mut().set(name, str_obj(value));
    }
    let body = String::from_utf8_lossy(&response.body).into_owned();
    let hash = Rc::new(RefCell::new(HashData::default()));
    hash.borrow_mut()
        .set("status", num_obj(response.status as f64));
    hash.borrow_mut()
        .set("statusText", str_obj(response.status_text));
    hash.borrow_mut().set("headers", Object::Hash(headers));
    hash.borrow_mut().set("body", str_obj(body));
    hash.borrow_mut().set(
        "ok",
        bool_obj(response.status >= 200 && response.status < 300),
    );

    Object::Hash(hash)
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
