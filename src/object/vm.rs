//! The VirtualMachine: owns the global scope, async coordination, and the
//! evaluator/importer callbacks used to break cycles between the object layer
//! and the evaluator.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use crate::ast::Position;

use super::value::{new_error, Object};

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
    /// Pending async work counter; the host drains this before returning.
    async_pending: RefCell<usize>,
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
            async_pending: RefCell::new(0),
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
