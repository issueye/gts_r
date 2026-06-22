//! Runtime object types.
//!
//! Values are reference-counted (`Rc`) so closures, arrays, and objects can be
//! shared. Mutable structures (arrays, objects, instances) wrap a `RefCell` so
//! the evaluator can mutate them in place, mirroring the JavaScript object
//! model.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::ast::{Param, Position, TypeAnnotation};

use super::environment::Environment;
use super::promise::Promise;
use super::vm::VirtualMachine;

/// A reference to an environment.
pub type EnvRef = Rc<RefCell<Environment>>;

/// The universal runtime value.
#[derive(Clone)]
pub enum Object {
    Number(f64),
    String(Rc<String>),
    Boolean(bool),
    Null,
    Undefined,
    Array(Rc<RefCell<ArrayData>>),
    Hash(Rc<RefCell<HashData>>),
    Function(Rc<Function>),
    Builtin(Rc<Builtin>),
    Class(Rc<RefCell<Class>>),
    Instance(Rc<RefCell<Instance>>),
    Error(Rc<RefCell<ErrorData>>),
    Return(Box<Object>),
    Promise(Rc<Promise>),
    Date(i64), // epoch millis
    Regexp(Rc<RegexpData>),
    Map(Rc<RefCell<MapData>>),
    Set(Rc<RefCell<SetData>>),
    /// A bytecode-VM closure. Coexists with `Function` (tree-walker closures)
    /// until the tree-walker is retired.
    Closure(Rc<crate::bytecode::closure::ClosureData>),
}

/// Backing storage for an array value.
#[derive(Default)]
pub struct ArrayData {
    pub elements: Vec<Object>,
}

/// Backing storage for an object/hash value.
pub struct HashData {
    /// String-keyed entries preserving insertion order.
    pub entries: Vec<(String, Object)>,
    pub proto: Option<Object>,
    pub frozen: bool,
    pub sealed: bool,
}

impl Default for HashData {
    fn default() -> Self {
        HashData {
            entries: Vec::new(),
            proto: None,
            frozen: false,
            sealed: false,
        }
    }
}

impl HashData {
    pub fn get(&self, key: &str) -> Option<&Object> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Object> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    pub fn set(&mut self, key: impl Into<String>, value: Object) {
        let key = key.into();
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn remove(&mut self, key: &str) -> Option<Object> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}

/// A user-defined function (closure) value.
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Rc<crate::ast::BlockStmt>,
    pub env: EnvRef,
    pub is_async: bool,
    pub return_t: Option<TypeAnnotation>,
    pub pos: Position,
    /// When true, `this` is lexically captured from the defining environment
    /// (arrow functions) rather than bound at call time.
    pub lexical_this: bool,
}

/// A native (Rust-implemented) builtin function.
pub struct Builtin {
    pub name: String,
    pub func: BuiltinFn,
    /// Bound receiver for method-style dispatch (e.g. `arr.push`).
    pub extra: Option<Object>,
}

/// The signature of a builtin function.
pub type BuiltinFn = Rc<dyn Fn(&mut CallContext<'_>, &[Object]) -> Object + 'static>;

/// Per-call context handed to builtins: the calling environment (for closures,
/// `this`, and the VM) plus the source position of the call site and the bound
/// method receiver (for builtin method dispatch).
pub struct CallContext<'a> {
    pub env: &'a EnvRef,
    pub pos: Position,
    pub receiver: Option<Object>,
}

impl<'a> CallContext<'a> {
    pub fn new(env: &'a EnvRef, pos: Position) -> Self {
        CallContext {
            env,
            pos,
            receiver: None,
        }
    }

    pub fn vm(&self) -> Rc<VirtualMachine> {
        self.env.borrow().vm.clone()
    }
}

/// A class value.
pub struct Class {
    pub name: String,
    pub super_: Option<Rc<RefCell<Class>>>,
    pub methods: HashMap<String, Object>,
    pub fields: HashMap<String, Object>,
    pub field_types: HashMap<String, TypeAnnotation>,
    pub statics: HashMap<String, Object>,
    pub static_types: HashMap<String, TypeAnnotation>,
    pub native_ctor: Option<NativeCtor>,
    pub pos: Position,
}

/// A native (Rust) constructor for built-in classes like Error.
pub type NativeCtor =
    Rc<dyn Fn(&mut CallContext<'_>, &Rc<RefCell<Instance>>, &[Object]) -> Object + 'static>;

/// A class instance.
pub struct Instance {
    pub class: Rc<RefCell<Class>>,
    pub props: HashMap<String, Object>,
    pub pos: Position,
}

/// An error value (thrown exception).
#[derive(Clone)]
pub struct ErrorData {
    pub message: String,
    pub name: String,
    pub stack: String,
    pub runtime: bool,
    pub pos: Position,
    /// The original thrown value, when the thrown value was not an Error.
    pub thrown: Option<Object>,
}

/// Compiled regular expression data.
pub struct RegexpData {
    pub source: String,
    pub flags: String,
    pub re: regex::Regex,
}

/// Backing storage for a Map value.
/// Maps can have any Object as key, using inspect() for comparison.
#[derive(Default)]
pub struct MapData {
    pub entries: Vec<(String, Object, Object)>, // (key_string, key_obj, value)
}

impl MapData {
    pub fn set(&mut self, key: Object, value: Object) {
        let key_str = key.inspect();
        if let Some(entry) = self.entries.iter_mut().find(|(k, _, _)| k == &key_str) {
            entry.2 = value;
        } else {
            self.entries.push((key_str, key, value));
        }
    }

    pub fn get(&self, key: &Object) -> Option<&Object> {
        let key_str = key.inspect();
        self.entries
            .iter()
            .find(|(k, _, _)| k == &key_str)
            .map(|(_, _, v)| v)
    }

    pub fn has(&self, key: &Object) -> bool {
        let key_str = key.inspect();
        self.entries.iter().any(|(k, _, _)| k == &key_str)
    }

    pub fn delete(&mut self, key: &Object) -> bool {
        let key_str = key.inspect();
        if let Some(pos) = self.entries.iter().position(|(k, _, _)| k == &key_str) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn size(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Backing storage for a Set value.
#[derive(Default)]
pub struct SetData {
    pub entries: Vec<(String, Object)>, // (value_string, value_obj)
}

impl SetData {
    pub fn add(&mut self, value: Object) {
        let value_str = value.inspect();
        if !self.entries.iter().any(|(v, _)| v == &value_str) {
            self.entries.push((value_str, value));
        }
    }

    pub fn has(&self, value: &Object) -> bool {
        let value_str = value.inspect();
        self.entries.iter().any(|(v, _)| v == &value_str)
    }

    pub fn delete(&mut self, value: &Object) -> bool {
        let value_str = value.inspect();
        if let Some(pos) = self.entries.iter().position(|(v, _)| v == &value_str) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn size(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Object {
    /// The type tag name used by `typeof`.
    pub fn type_tag(&self) -> &'static str {
        match self {
            Object::Number(_) => "number",
            Object::String(_) => "string",
            Object::Boolean(_) => "boolean",
            Object::Null => "object",
            Object::Undefined => "undefined",
            Object::Function(_) | Object::Builtin(_) | Object::Class(_) | Object::Closure(_) => {
                "function"
            }
            _ => "object",
        }
    }

    /// Render the value for display (console.log, string concatenation context).
    pub fn inspect(&self) -> String {
        match self {
            Object::Number(n) => format_number(*n),
            Object::String(s) => (**s).clone(),
            Object::Boolean(b) => b.to_string(),
            Object::Null => "null".into(),
            Object::Undefined => "undefined".into(),
            Object::Array(a) => {
                let elems: Vec<String> = a.borrow().elements.iter().map(|e| e.inspect()).collect();
                format!("[{}]", elems.join(", "))
            }
            Object::Hash(h) => {
                let pairs: Vec<String> = h
                    .borrow()
                    .entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.inspect()))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
            Object::Function(f) => {
                let params: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
                if f.name.is_empty() {
                    format!("fn({})", params.join(", "))
                } else {
                    format!("fn {}({})", f.name, params.join(", "))
                }
            }
            Object::Builtin(b) => format!("<builtin {}>", b.name),
            Object::Class(c) => format!("<class {}>", c.borrow().name),
            Object::Instance(i) => format!("<{} instance>", i.borrow().class.borrow().name),
            Object::Error(e) => {
                let e = e.borrow();
                let name = if e.name.is_empty() { "Error" } else { &e.name };
                if e.pos.is_zero() {
                    format!("{}: {}", name, e.message)
                } else {
                    format!("{}: {}: {}", e.pos, name, e.message)
                }
            }
            Object::Return(r) => r.inspect(),
            Object::Promise(p) => p.inspect(),
            Object::Date(ms) => format!("<date {}>", ms),
            Object::Regexp(r) => format!("/{}/{}", r.source, r.flags),
            Object::Map(m) => {
                let entries: Vec<String> = m
                    .borrow()
                    .entries
                    .iter()
                    .map(|(_, k, v)| format!("{} => {}", k.inspect(), v.inspect()))
                    .collect();
                format!("Map({})", entries.len())
            }
            Object::Set(s) => {
                let entries: Vec<String> = s
                    .borrow()
                    .entries
                    .iter()
                    .map(|(_, v)| v.inspect())
                    .collect();
                format!("Set({})", entries.len())
            }
            Object::Function(f) => format!("[Function: {}]", f.name),
            Object::Builtin(b) => format!("[Function: {}]", b.name),
            Object::Class(c) => format!("[Class: {}]", c.borrow().name),
            Object::Closure(c) => {
                let name = &c.proto.name;
                if name.is_empty() {
                    "[Function (anonymous)]".into()
                } else {
                    format!("[Function: {}]", name)
                }
            }
        }
    }

    /// Truthiness, per JS semantics.
    pub fn is_truthy(&self) -> bool {
        match self {
            Object::Null | Object::Undefined => false,
            Object::Boolean(b) => *b,
            Object::Number(n) => *n != 0.0,
            Object::String(s) => !s.is_empty(),
            _ => true,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Object::Error(_))
    }

    pub fn is_runtime_error(&self) -> bool {
        if let Object::Error(e) = self {
            e.borrow().runtime
        } else {
            false
        }
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Object::Number(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Object::String(_))
    }
}

impl PartialEq for Object {
    /// Reference equality for compound values, value equality for primitives.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Object::Number(a), Object::Number(b)) => a == b,
            (Object::String(a), Object::String(b)) => a == b,
            (Object::Boolean(a), Object::Boolean(b)) => a == b,
            (Object::Null, Object::Null) | (Object::Undefined, Object::Undefined) => true,
            (Object::Array(a), Object::Array(b)) => Rc::ptr_eq(a, b),
            (Object::Hash(a), Object::Hash(b)) => Rc::ptr_eq(a, b),
            (Object::Function(a), Object::Function(b)) => Rc::ptr_eq(a, b),
            (Object::Instance(a), Object::Instance(b)) => Rc::ptr_eq(a, b),
            (Object::Class(a), Object::Class(b)) => Rc::ptr_eq(a, b),
            (Object::Promise(a), Object::Promise(b)) => Rc::ptr_eq(a, b),
            (Object::Closure(a), Object::Closure(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inspect())
    }
}

/// Format a number the way JS does (integers without a decimal point).
pub fn format_number(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if n == n.trunc() && n.abs() < 1e21 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

/// Strict equality (===), with NaN === NaN as required by the GoScript spec.
pub fn strict_equal(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::Number(x), Object::Number(y)) => x == y,
        (Object::String(x), Object::String(y)) => x == y,
        (Object::Boolean(x), Object::Boolean(y)) => x == y,
        (Object::Null, Object::Null) => true,
        (Object::Undefined, Object::Undefined) => true,
        // Reference equality for compound types
        (Object::Array(x), Object::Array(y)) => Rc::ptr_eq(x, y),
        (Object::Hash(x), Object::Hash(y)) => Rc::ptr_eq(x, y),
        (Object::Function(x), Object::Function(y)) => Rc::ptr_eq(x, y),
        (Object::Instance(x), Object::Instance(y)) => Rc::ptr_eq(x, y),
        (Object::Class(x), Object::Class(y)) => Rc::ptr_eq(x, y),
        (Object::Promise(x), Object::Promise(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// Convenience constructors.
pub fn str_obj(s: impl Into<String>) -> Object {
    Object::String(Rc::new(s.into()))
}
pub fn num_obj(n: f64) -> Object {
    Object::Number(n)
}
pub fn bool_obj(b: bool) -> Object {
    Object::Boolean(b)
}

/// Construct a runtime error value with the conventional `"Name: message"` prefix.
pub fn new_error(pos: Position, msg: impl Into<String>) -> Object {
    let msg = msg.into();
    let (name, message) = split_error_name(&msg);
    let stack = if pos.is_zero() {
        format!("{}: {}", name, message)
    } else {
        format!("{}: {}\n    at {}", name, message, pos)
    };
    Object::Error(Rc::new(RefCell::new(ErrorData {
        message,
        name,
        stack,
        runtime: true,
        pos,
        thrown: None,
    })))
}

pub fn new_named_error(
    pos: Position,
    name: impl Into<String>,
    message: impl Into<String>,
) -> Object {
    new_named_error_with_runtime(pos, name, message, true)
}

pub fn new_error_object(
    pos: Position,
    name: impl Into<String>,
    message: impl Into<String>,
) -> Object {
    new_named_error_with_runtime(pos, name, message, false)
}

fn new_named_error_with_runtime(
    pos: Position,
    name: impl Into<String>,
    message: impl Into<String>,
    runtime: bool,
) -> Object {
    let name = name.into();
    let message = message.into();
    let stack = if pos.is_zero() {
        format!("{}: {}", name, message)
    } else {
        format!("{}: {}\n    at {}", name, message, pos)
    };
    Object::Error(Rc::new(RefCell::new(ErrorData {
        message,
        name,
        stack,
        runtime,
        pos,
        thrown: None,
    })))
}

fn split_error_name(msg: &str) -> (String, String) {
    for prefix in [
        "TypeError",
        "RangeError",
        "ReferenceError",
        "SyntaxError",
        "ImportError",
        "ExportError",
        "MatchError",
        "PermissionError",
        "TimeoutError",
        "HostError",
        "Error",
    ] {
        if let Some(rest) = msg.strip_prefix(&format!("{}: ", prefix)) {
            return (prefix.into(), rest.into());
        }
    }
    ("Error".into(), msg.into())
}
