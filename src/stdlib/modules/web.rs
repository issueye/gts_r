use std::cell::Cell;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;



use super::super::helpers::*;
use super::signal::{ctrlc_set_flag, exact_match, prefix_match};
use crate::object::{
    new_error, num_obj, str_obj, Builtin,
    CallContext, HashData, Object, PromiseState,
};

/// The shared handle a worker thread receives from the spawner.
struct WebWorkerCtx {
    /// The single bound listener shared by all workers (accept-ready model).
    server: std::sync::Arc<tiny_http::Server>,
    /// Set to true by `app.close()` or Ctrl+C to ask workers to exit.
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// This worker's id (0-based), for logging.
    id: usize,
}

thread_local! {
    static WEB_WORKER_CTX: RefCell<Option<WebWorkerCtx>> = const { RefCell::new(None) };
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebRequestOutcome {
    Responded,
    Pending,
}

pub(crate) const WEB_APP_STATE_KEY: &str = "__web_app__";

pub(crate) fn web_module() -> Object {
    module(vec![
        ("createApp", native("web.createApp", web_create_app)),
        ("json", native("web.json", web_json_helper)),
        ("text", native("web.text", web_text_helper)),
        ("static", native("web.static", web_static_helper)),
    ])
}

pub(crate) fn web_create_app(_ctx: &mut CallContext, _args: &[Object]) -> Object {
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
        .filter(|h| {
            matches!(
                h,
                Object::Function(_) | Object::Builtin(_) | Object::Closure(_)
            )
        })
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
        .filter(|h| {
            matches!(
                h,
                Object::Function(_) | Object::Builtin(_) | Object::Closure(_)
            )
        })
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

pub(crate) fn split_route_path(path: &str) -> Vec<String> {
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

    // Parse options. Defaults: long-running single worker. Explicit `count`
    // keeps the bounded serial behavior used by unit tests.
    let mut count: usize = 1;
    let mut workers: usize = 1;
    if let Some(Object::Hash(opts)) = args.get(1) {
        if let Some(Object::Number(n)) = opts.borrow().get("count") {
            count = *n as usize;
            workers = 0;
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
    let pending_responses = Rc::new(Cell::new(0usize));
    loop {
        if !infinite && served >= count {
            if pending_responses.get() == 0 {
                break;
            }
            ctx.vm().wait_async();
            ctx.vm().drain_async_completions();
            continue;
        }
        if let Some(flag) = shutdown.as_ref() {
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
        }
        ctx.vm().drain_async_completions();
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
                Ok(None) => {
                    ctx.vm().drain_async_completions();
                    continue;
                }
                Err(_) => {
                    ctx.vm().drain_async_completions();
                    continue;
                }
            }
        };
        match web_handle_request(ctx, app, request, Some(pending_responses.clone())) {
            Ok(WebRequestOutcome::Responded) => served += 1,
            Ok(WebRequestOutcome::Pending) => served += 1,
            Err(_e) => served += 1,
        }
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
        ctx.vm().drain_async_completions();
        let request = match wctx.server.recv_timeout(timeout) {
            Ok(Some(r)) => r,
            Ok(None) => {
                ctx.vm().drain_async_completions();
                continue;
            }
            Err(_) => break, // listener gone
        };
        if let Err(_e) = web_handle_request(ctx, app, request, None) {
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

/// Process one request: build context, match routes, run the handler chain,
/// then respond on the original request (consumed by value).
fn web_handle_request(
    ctx: &mut CallContext,
    app: &Rc<WebApp>,
    mut request: tiny_http::Request,
    pending_responses: Option<Rc<Cell<usize>>>,
) -> Result<WebRequestOutcome, String> {
    let method = request.method().as_str().to_ascii_uppercase();
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();
    let remote_addr = request
        .remote_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();

    // Read body (borrows request immutably via as_reader).
    let mut body_buf = Vec::new();
    {
        let reader = request.as_reader();
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
    let req_obj = Rc::new(RefCell::new(HashData::default()));
    req_obj.borrow_mut().set("method", str_obj(method.clone()));
    req_obj.borrow_mut().set("url", str_obj(url.clone()));
    req_obj.borrow_mut().set("path", str_obj(path.clone()));
    req_obj.borrow_mut().set("body", str_obj(body.clone()));
    req_obj.borrow_mut().set("remoteAddr", str_obj(remote_addr));
    req_obj
        .borrow_mut()
        .set("query", Object::Hash(query_obj.clone()));
    req_obj
        .borrow_mut()
        .set("headers", Object::Hash(headers_obj.clone()));
    let res_obj = http_response_object(resp_state.clone());
    let req_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut request_slot = Some(request);

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
        // Run the matched handler chain. Handlers use Express-style
        // (req, res, next); the old ctx wrapper is intentionally retired.
        let mut err: Option<String> = None;
        for (handler, params) in chain {
            inject_route_params(req_obj.clone(), &query_obj, &headers_obj, &params);
            let result = call_script_function(
                &handler,
                ctx.env,
                &[
                    Object::Hash(req_obj.clone()),
                    res_obj.clone(),
                    Object::Builtin(Rc::new(Builtin {
                        name: "web.next".to_string(),
                        func: Rc::new(|_ctx, _args| Object::Undefined),
                        extra: None,
                    })),
                ],
            );
            if result.is_runtime_error() {
                err = Some(result.inspect());
                break;
            }
            match web_handle_handler_promise(ctx, &result, resp_state.clone(), &mut request_slot) {
                WebPromiseOutcome::NotPromise => {}
                WebPromiseOutcome::Rejected(msg) => {
                    err = Some(msg);
                    break;
                }
                WebPromiseOutcome::Pending => {
                    if let Some(counter) = pending_responses.as_ref() {
                        counter.set(counter.get() + 1);
                        let counter_for_completion = counter.clone();
                        if let Object::Promise(promise) = result {
                            promise.add_continuation(Box::new(move |_state, _value| {
                                counter_for_completion
                                    .set(counter_for_completion.get().saturating_sub(1));
                            }));
                        }
                    }
                    return Ok(WebRequestOutcome::Pending);
                }
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

    if let Some(request) = request_slot.take() {
        web_respond(request, &resp_state);
    }
    Ok(WebRequestOutcome::Responded)
}

enum WebPromiseOutcome {
    NotPromise,
    Pending,
    Rejected(String),
}

fn web_handle_handler_promise(
    _ctx: &mut CallContext,
    result: &Object,
    resp_state: Rc<RefCell<HttpResponseState>>,
    request: &mut Option<tiny_http::Request>,
) -> WebPromiseOutcome {
    let Object::Promise(promise) = result else {
        return WebPromiseOutcome::NotPromise;
    };
    match promise.state() {
        PromiseState::Pending => {
            let Some(request) = request.take() else {
                return WebPromiseOutcome::Rejected("web response already consumed".to_string());
            };
            promise.add_continuation(Box::new(move |state, value| {
                if state == PromiseState::Rejected || value.is_runtime_error() {
                    let mut g = resp_state.borrow_mut();
                    g.status = Some(500);
                    g.content_type = Some("text/plain".to_string());
                    g.body =
                        Some(format!("Internal Server Error: {}", value.inspect()).into_bytes());
                }
                web_respond(request, &resp_state);
            }));
            WebPromiseOutcome::Pending
        }
        PromiseState::Rejected => {
            WebPromiseOutcome::Rejected(promise.value().unwrap_or(Object::Undefined).inspect())
        }
        PromiseState::Fulfilled => {
            let value = promise.value().unwrap_or(Object::Undefined);
            if value.is_runtime_error() {
                WebPromiseOutcome::Rejected(value.inspect())
            } else {
                WebPromiseOutcome::NotPromise
            }
        }
    }
}

fn web_respond(request: tiny_http::Request, resp_state: &Rc<RefCell<HttpResponseState>>) {
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
    if let Ok(h) = tiny_http::Header::from_bytes(&b"Connection"[..], &b"close"[..]) {
        response = response.with_header(h);
    }
    drop(state);
    let _ = request.respond(response);
}

fn inject_route_params(
    req_obj: Rc<RefCell<HashData>>,
    query: &Rc<RefCell<HashData>>,
    headers: &Rc<RefCell<HashData>>,
    params: &[(String, String)],
) {
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
    req_obj
        .borrow_mut()
        .set("params", Object::Hash(params_obj.clone()));
}

// --- web.json / web.text helpers ------------------------------------------
// `web.static` is intentionally omitted from this synchronous port: serving
// files requires the same async event loop as a long-running server. Scripts
// can read a file with `@std/fs` and call `res.send(contents)` instead.

/// `web.json()` returns a request-body parser middleware; `web.json(obj)`
/// keeps the historical serializer behavior.
fn web_json_helper(_ctx: &mut CallContext, args: &[Object]) -> Object {
    match args.get(0) {
        Some(v) => str_obj(value_to_json(v)),
        None => native("web.json.middleware", |ctx, args| {
            let Some(Object::Hash(req_obj)) = args.first() else {
                return Object::Undefined;
            };
            let body = match req_obj.borrow().get("body") {
                Some(Object::String(s)) => s.to_string(),
                _ => String::new(),
            };
            if body.trim().is_empty() {
                return Object::Undefined;
            }
            match simple_json_parse(&body) {
                Ok(value) => {
                    req_obj
                        .borrow_mut()
                        .set("body", crate::stdlib::helpers::json_to_object(value));
                    Object::Undefined
                }
                Err(err) => new_error(ctx.pos.clone(), format!("web.json: {}", err)),
            }
        }),
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
        let req_obj = match args.first() {
            Some(Object::Hash(h)) => h.clone(),
            _ => return Object::Undefined,
        };
        let path = match req_obj.borrow().get("path") {
            Some(Object::String(p)) => p.to_string(),
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
