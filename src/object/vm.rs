//! The VirtualMachine: owns the global scope, async coordination, and the
//! evaluator/importer callbacks used to break cycles between the object layer
//! and the evaluator.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use crate::ast::Position;
use crate::async_runtime::{
    AsyncCompletion, AsyncCompletionData, AsyncCompletionId, AsyncCompletionQueue,
    AsyncCompletionResult, AsyncCompletionSender, AsyncHttpResponse,
};

use super::promise::Promise;
use super::value::{bool_obj, new_error, num_obj, str_obj, ArrayData, HashData, Object};

/// The evaluator callback type. Given an AST node and an environment, produce a
/// value. Stored on the VM so runtime objects (closures, Promise continuations)
/// can drive evaluation without a direct dependency on the evaluator module.
pub type EvaluatorFn = dyn Fn(NodeRef, &EnvRef, &Rc<VirtualMachine>) -> Object;

/// An owned AST node reference that can be held by the evaluator callback.
#[derive(Clone)]
pub enum NodeRef {
    Stmt(Rc<crate::ast::Stmt>),
    Expr(Rc<crate::ast::Expr>),
    Program(Rc<crate::ast::Program>),
}

/// Importer callback: load a module by specifier.
pub type ImporterFn = dyn Fn(&EnvRef, &str) -> Result<Object, Object>;

pub use super::value::EnvRef;

/// Execution backend selection. 0 = legacy tree-walker, 1 = bytecode VM (default).
/// Stored as `AtomicU8` so the VM can be queried without borrowing.
pub const EXEC_MODE_TREEWALK: u8 = 0;
pub const EXEC_MODE_BYTECODE: u8 = 1;

/// The runtime for one isolated script execution.
pub struct VirtualMachine {
    globals: RefCell<HashMap<String, Object>>,
    pub argv: RefCell<Vec<String>>,
    pub type_check: AtomicBool,
    /// Which execution backend to use. See `EXEC_MODE_*` constants.
    pub exec_mode: AtomicU8,
    next_timer: AtomicI64,
    next_async_completion: AtomicI64,
    /// Pending async work counter; the host drains this before returning.
    async_pending: RefCell<usize>,
    async_completions: AsyncCompletionQueue,
    async_promises: RefCell<HashMap<AsyncCompletionId, Rc<Promise>>>,
    pub bootstrap_source: RefCell<String>,
    evaluator: RefCell<Option<Rc<EvaluatorFn>>>,
    importer: RefCell<Option<Rc<ImporterFn>>>,
    deadline: RefCell<Option<Instant>>,
}

impl VirtualMachine {
    pub fn new() -> Rc<VirtualMachine> {
        Rc::new(VirtualMachine {
            globals: RefCell::new(HashMap::new()),
            argv: RefCell::new(Vec::new()),
            type_check: AtomicBool::new(false),
            exec_mode: AtomicU8::new(EXEC_MODE_BYTECODE),
            next_timer: AtomicI64::new(0),
            next_async_completion: AtomicI64::new(0),
            async_pending: RefCell::new(0),
            async_completions: AsyncCompletionQueue::new(),
            async_promises: RefCell::new(HashMap::new()),
            bootstrap_source: RefCell::new(String::new()),
            evaluator: RefCell::new(None),
            importer: RefCell::new(None),
            deadline: RefCell::new(None),
        })
    }

    pub fn set_global(&self, name: impl Into<String>, value: Object) {
        self.globals.borrow_mut().insert(name.into(), value);
    }

    pub fn get_global(&self, name: &str) -> Option<Object> {
        self.globals.borrow().get(name).cloned()
    }

    pub fn has_global(&self, name: &str) -> bool {
        self.globals.borrow().contains_key(name)
    }

    pub fn set_argv(&self, argv: Vec<String>) {
        *self.argv.borrow_mut() = argv;
    }

    pub fn set_evaluator(&self, f: Rc<EvaluatorFn>) {
        *self.evaluator.borrow_mut() = Some(f);
    }

    pub fn evaluator(&self) -> Option<Rc<EvaluatorFn>> {
        self.evaluator.borrow().clone()
    }

    pub fn set_importer(&self, f: Rc<ImporterFn>) {
        *self.importer.borrow_mut() = Some(f);
    }

    pub fn importer(&self) -> Option<Rc<ImporterFn>> {
        self.importer.borrow().clone()
    }

    pub fn next_timer_id(&self) -> i64 {
        self.next_timer.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn next_async_completion_id(&self) -> AsyncCompletionId {
        (self.next_async_completion.fetch_add(1, Ordering::Relaxed) + 1) as AsyncCompletionId
    }

    pub fn set_timeout(&self, timeout: Option<Duration>) {
        *self.deadline.borrow_mut() = timeout.map(|duration| Instant::now() + duration);
    }

    pub fn clear_timeout(&self) {
        *self.deadline.borrow_mut() = None;
    }

    pub fn check_timeout(&self, pos: Position) -> Option<Object> {
        let deadline = *self.deadline.borrow();
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                return Some(new_error(pos, "TimeoutError: script execution timed out"));
            }
        }
        None
    }

    /// Register outstanding async work.
    pub fn async_add(&self, n: usize) {
        *self.async_pending.borrow_mut() += n;
    }

    /// Clone a thread-safe sender for Tokio/background workers.
    pub fn async_completion_sender(&self) -> AsyncCompletionSender {
        self.async_completions.sender()
    }

    /// Allocate an async completion id, register a Promise on the VM thread,
    /// and count it as pending async work.
    pub fn create_async_completion_promise(&self) -> (AsyncCompletionId, Rc<Promise>) {
        let id = self.next_async_completion_id();
        let promise = Promise::new();
        self.register_async_completion_promise(id, promise.clone());
        (id, promise)
    }

    /// Register a Promise that will be settled when the matching completion is
    /// drained on the VM thread.
    pub fn register_async_completion_promise(&self, id: AsyncCompletionId, promise: Rc<Promise>) {
        self.async_promises.borrow_mut().insert(id, promise);
        self.async_add(1);
    }

    /// Queue a completion from the VM thread or tests.
    pub fn enqueue_async_completion(&self, completion: AsyncCompletion) {
        self.async_completions.enqueue(completion);
    }

    /// Convenience helper to resolve an async operation with owned data.
    pub fn enqueue_async_resolve(&self, id: AsyncCompletionId, data: AsyncCompletionData) {
        self.enqueue_async_completion(AsyncCompletion::resolve(id, data));
    }

    /// Convenience helper to reject an async operation with an owned error.
    pub fn enqueue_async_reject(&self, id: AsyncCompletionId, error: impl Into<String>) {
        self.enqueue_async_completion(AsyncCompletion::reject(id, error));
    }

    /// Drain queued completions on the VM thread.
    ///
    /// Matching registered Promises are resolved/rejected here so Object work
    /// remains on the VM thread. The returned completions are pure data for
    /// diagnostics and low-level tests.
    pub fn drain_async_completions(&self) -> Vec<AsyncCompletion> {
        let completions = self.async_completions.drain();
        for completion in &completions {
            if let Some(promise) = self.async_promises.borrow_mut().remove(&completion.id) {
                match &completion.result {
                    AsyncCompletionResult::Resolve(data) => {
                        promise.resolve(async_completion_data_to_object(data.clone()));
                    }
                    AsyncCompletionResult::Reject(error) => {
                        promise.reject(new_error(Position::default(), error.clone()));
                    }
                }
            }
            self.async_done();
        }
        completions
    }

    pub fn async_completion_len(&self) -> usize {
        self.async_completions.len()
    }

    pub fn async_registered_promise_len(&self) -> usize {
        self.async_promises.borrow().len()
    }

    /// Mark async work complete.
    pub fn async_done(&self) {
        let mut g = self.async_pending.borrow_mut();
        if *g > 0 {
            *g -= 1;
        }
    }

    /// Block until all outstanding async tasks complete. In the single-threaded
    /// model this simply polls; the event loop on the host thread drives the
    /// resolution.
    pub fn wait_async(&self) {
        while *self.async_pending.borrow() > 0 {
            let drained = self.drain_async_completions();
            if !drained.is_empty() {
                continue;
            }
            if self.check_timeout(Position::default()).is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

/// Helper to create an error positioned at the given call site.
pub fn vm_error(pos: Position, msg: impl Into<String>) -> Object {
    new_error(pos, msg)
}

fn async_completion_data_to_object(data: AsyncCompletionData) -> Object {
    match data {
        AsyncCompletionData::Undefined => Object::Undefined,
        AsyncCompletionData::Text(text) | AsyncCompletionData::JsonText(text) => str_obj(text),
        AsyncCompletionData::Bytes(bytes) => Object::Array(Rc::new(RefCell::new(ArrayData {
            elements: bytes.into_iter().map(|byte| num_obj(byte as f64)).collect(),
        }))),
        AsyncCompletionData::HttpResponse(response) => async_http_response_to_object(response),
    }
}

fn async_http_response_to_object(response: AsyncHttpResponse) -> Object {
    let headers = Rc::new(RefCell::new(HashData::default()));
    for (name, value) in response.headers {
        headers.borrow_mut().set(name, str_obj(value));
    }

    let body = String::from_utf8_lossy(&response.body).into_owned();
    let obj = Rc::new(RefCell::new(HashData::default()));
    obj.borrow_mut()
        .set("status", num_obj(response.status as f64));
    obj.borrow_mut()
        .set("statusText", str_obj(response.status_text));
    obj.borrow_mut().set("headers", Object::Hash(headers));
    obj.borrow_mut().set("body", str_obj(body));
    obj.borrow_mut()
        .set("ok", bool_obj((200..300).contains(&response.status)));
    Object::Hash(obj)
}
