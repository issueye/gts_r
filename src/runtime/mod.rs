//! Runtime session wiring the VM, module cache, and source loader.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::ast::{Position, Program};
use crate::evaluator::builtins::register_globals;
use crate::evaluator::expressions::apply_function;
use crate::evaluator::{eval_node, eval_program};
use crate::lexer::Lexer;
use crate::module::{
    cache_get, cache_insert, new_module_cache, ModuleCache, ModuleKind, ResolveOptions, Resolver,
};
use crate::object::{
    new_error, str_obj, ArrayData, Builtin, BuiltinFn, Environment, HashData, Object,
    VirtualMachine, EXEC_MODE_BYTECODE,
};
use crate::parser::Parser;
use crate::stdlib::load_native_module;

/// Result of running a script.
pub type RuntimeResult<T> = Result<T, Object>;

/// Options for running a script file.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub argv: Vec<String>,
    pub call_main: bool,
    pub timeout: Option<Duration>,
}

/// One isolated GoScript execution session.
pub struct Session {
    vm: Rc<VirtualMachine>,
    root: crate::object::EnvRef,
    module_cache: Rc<ModuleCache>,
    resolver: Rc<Resolver>,
    #[cfg(feature = "tokio")]
    tokio_runtime: Option<crate::async_runtime::tokio_rt::TokioRuntime>,
}

impl Session {
    /// Create a fresh session with standard globals installed.
    pub fn new() -> Session {
        let vm = VirtualMachine::new();
        vm.exec_mode.store(EXEC_MODE_BYTECODE, Ordering::Relaxed);
        register_globals(&vm);
        vm.set_evaluator(Rc::new(eval_node));

        let root = Environment::new_root(vm.clone());
        let module_cache = Rc::new(new_module_cache());
        let resolver = Rc::new(Resolver::new(None));

        let session = Session {
            vm,
            root,
            module_cache,
            resolver,
            #[cfg(feature = "tokio")]
            tokio_runtime: None,
        };
        session.install_host_globals();
        session.install_importer();
        session
    }

    /// Create a session with tokio runtime enabled (requires `tokio` feature)
    ///
    /// This enables multi-threaded async execution for I/O-bound workloads.
    #[cfg(feature = "tokio")]
    pub fn with_tokio() -> Session {
        let vm = VirtualMachine::new();
        vm.exec_mode.store(EXEC_MODE_BYTECODE, Ordering::Relaxed);
        register_globals(&vm);
        vm.set_evaluator(Rc::new(eval_node));

        let root = Environment::new_root(vm.clone());
        let module_cache = Rc::new(new_module_cache());
        let resolver = Rc::new(Resolver::new(None));

        let session = Session {
            vm,
            root,
            module_cache,
            resolver,
            tokio_runtime: Some(crate::async_runtime::tokio_rt::TokioRuntime::new()),
        };
        session.install_host_globals();
        session.install_importer();
        session
    }

    /// Check if tokio runtime is enabled
    #[cfg(feature = "tokio")]
    pub fn has_tokio(&self) -> bool {
        self.tokio_runtime.is_some()
    }

    /// Get a reference to the tokio runtime, if available
    #[cfg(feature = "tokio")]
    pub fn tokio_runtime(&self) -> Option<&crate::async_runtime::tokio_rt::TokioRuntime> {
        self.tokio_runtime.as_ref()
    }

    /// Access the underlying VM.
    pub fn vm(&self) -> Rc<VirtualMachine> {
        self.vm.clone()
    }

    /// Run a source string as the top-level script.
    pub fn run_source(&self, source: &str, file: impl AsRef<Path>) -> RuntimeResult<Object> {
        self.run_source_with_options(source, file, false)
    }

    /// Run a source string, optionally invoking a top-level `main()` after load.
    pub fn run_source_with_options(
        &self,
        source: &str,
        file: impl AsRef<Path>,
        call_main: bool,
    ) -> RuntimeResult<Object> {
        let file = file.as_ref();
        // Record the entry script path so native modules (e.g. @std/web's
        // concurrent workers) can locate and re-run the script.
        *self.vm.bootstrap_source.borrow_mut() = file.to_string_lossy().into_owned();
        let program = parse_source(source, file)?;
        let module_dir = file.parent().unwrap_or_else(|| Path::new("."));
        self.root.borrow_mut().module_dir = module_dir.to_string_lossy().into_owned();
        let exports = Object::Hash(Rc::new(RefCell::new(HashData::default())));
        install_module_bindings(&self.root, exports);
        let mut result = eval_program_for_session(&program, &self.root);
        if !result.is_runtime_error() && call_main {
            result = self.call_main_if_present();
        }
        self.vm.wait_async();
        if result.is_runtime_error() {
            Err(result)
        } else {
            Ok(result)
        }
    }

    /// Read and run a `.gs` file.
    pub fn run_file(&self, file: impl AsRef<Path>, argv: Vec<String>) -> RuntimeResult<Object> {
        self.run_file_with_options(
            file,
            RunOptions {
                argv,
                call_main: false,
                timeout: None,
            },
        )
    }

    /// Read and run a `.gs` file with explicit runtime options.
    pub fn run_file_with_options(
        &self,
        file: impl AsRef<Path>,
        options: RunOptions,
    ) -> RuntimeResult<Object> {
        let file = normalize_path(file.as_ref());
        self.vm.set_argv(options.argv);
        self.vm.set_timeout(options.timeout);
        self.refresh_process_argv();
        let source = fs::read_to_string(&file).map_err(|e| {
            new_error(
                Default::default(),
                format!("IOError: cannot read {}: {}", file.display(), e),
            )
        })?;
        let result = self.run_source_with_options(&source, &file, options.call_main);
        self.vm.clear_timeout();
        result
    }

    fn install_host_globals(&self) {
        let require_fn: BuiltinFn = Rc::new(|ctx, args| {
            let spec = match args.first() {
                Some(Object::String(s)) => s.to_string(),
                Some(other) => other.inspect(),
                None => {
                    return new_error(ctx.pos.clone(), "TypeError: require expects a module path")
                }
            };
            let importer = ctx.env.borrow().vm.importer();
            match importer {
                Some(importer) => match importer(ctx.env, &spec) {
                    Ok(module) => module,
                    Err(err) => err,
                },
                None => new_error(
                    ctx.pos.clone(),
                    "ImportError: module loading is not configured",
                ),
            }
        });
        self.vm.set_global(
            "require",
            Object::Builtin(Rc::new(Builtin {
                name: "require".into(),
                func: require_fn,
                extra: None,
            })),
        );
        self.refresh_process_argv();
    }

    fn refresh_process_argv(&self) {
        let args = Object::Array(Rc::new(RefCell::new(ArrayData {
            elements: self.vm.argv.borrow().iter().cloned().map(str_obj).collect(),
        })));
        let process = Object::Hash(Rc::new(RefCell::new(HashData::default())));
        if let Object::Hash(h) = &process {
            h.borrow_mut().set("argv", args);
        }
        self.vm.set_global("process", process);
    }

    fn call_main_if_present(&self) -> Object {
        let main = self.root.borrow().get("main");
        match main {
            Some(Object::Undefined) | None => Object::Undefined,
            Some(value) => apply_function(&value, &self.root, &[], None, Position::default()),
        }
    }

    fn install_importer(&self) {
        let cache = self.module_cache.clone();
        let resolver = self.resolver.clone();
        self.vm.set_importer(Rc::new(move |env, spec| {
            let base_dir = PathBuf::from(env.borrow().module_dir.clone());
            let resolved = resolver
                .resolve(
                    spec,
                    ResolveOptions {
                        base_dir: Some(base_dir.clone()),
                        ..ResolveOptions::default()
                    },
                )
                .map_err(|e| new_error(Default::default(), format!("ImportError: {e}")))?;

            if resolved.kind == ModuleKind::Native {
                return load_native_module(spec).ok_or_else(|| {
                    new_error(
                        Default::default(),
                        format!("ImportError: unknown native module '{}'", spec),
                    )
                });
            }

            if let Some(module) = cache_get(&cache, &resolved.id) {
                return Ok(module);
            }

            let path = resolved.path.clone().ok_or_else(|| {
                new_error(
                    Default::default(),
                    format!("ImportError: module '{}' has no source path", spec),
                )
            })?;
            if resolved.kind == ModuleKind::Json {
                let module = load_json_module(&path)?;
                cache_insert(&cache, resolved.id, module.clone());
                return Ok(module);
            }
            let source = fs::read_to_string(&path).map_err(|e| {
                new_error(
                    Default::default(),
                    format!("ImportError: cannot read {}: {}", path.display(), e),
                )
            })?;
            let program = parse_source(&source, &path)?;
            let module = Object::Hash(Rc::new(RefCell::new(HashData::default())));
            cache_insert(&cache, resolved.id.clone(), module.clone());

            let scope = Environment::child(env);
            scope.borrow_mut().module_dir = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_string_lossy()
                .into_owned();
            install_module_bindings(&scope, module.clone());

            let result = eval_program_for_session(&program, &scope);
            if result.is_runtime_error() {
                Err(result)
            } else {
                let final_exports = module_exports(&scope).unwrap_or(module);
                cache_insert(&cache, resolved.id, final_exports.clone());
                Ok(final_exports)
            }
        }));
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// Read and run a `.gs` file, then return its final `module.exports`.
    ///
    /// Used by `@std/runtime` to support `runScript`/`callScript`/`runTool`
    /// style helpers that spawn an isolated sub-script and inspect what it
    /// exported. The sub-script runs in a fresh VM with its own argv.
    pub fn run_file_for_exports(
        &self,
        file: impl AsRef<Path>,
        argv: Vec<String>,
        call_main: bool,
    ) -> RuntimeResult<Object> {
        let file = normalize_path(file.as_ref());
        self.vm.set_argv(argv);
        self.refresh_process_argv();
        let source = fs::read_to_string(&file).map_err(|e| {
            new_error(
                Default::default(),
                format!("IOError: cannot read {}: {}", file.display(), e),
            )
        })?;
        let program = parse_source(&source, &file)?;
        let module_dir = file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy()
            .into_owned();
        self.root.borrow_mut().module_dir = module_dir;
        let exports = Object::Hash(Rc::new(RefCell::new(HashData::default())));
        install_module_bindings(&self.root, exports);
        let mut result = eval_program_for_session(&program, &self.root);
        if !result.is_runtime_error() && call_main {
            result = self.call_main_if_present();
        }
        self.vm.wait_async();
        if result.is_runtime_error() {
            return Err(result);
        }
        Ok(module_exports(&self.root).unwrap_or(Object::Undefined))
    }

    /// Look up a named export on the root environment of the last-run script.
    pub fn root_export(&self, name: &str) -> Option<Object> {
        module_exports(&self.root).and_then(|exports| match exports {
            Object::Hash(h) => h.borrow().get(name).cloned(),
            _ => None,
        })
    }
}

fn parse_source(source: &str, file: &Path) -> RuntimeResult<Program> {
    let lex = Lexer::new(source);
    let mut parser = Parser::new(lex, file.to_string_lossy());
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        let message = program
            .errors
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        Err(new_error(
            Default::default(),
            format!("SyntaxError: {}", message),
        ))
    } else {
        Ok(program)
    }
}

fn eval_program_for_session(program: &Program, env: &crate::object::EnvRef) -> Object {
    let vm = env.borrow().vm.clone();
    if vm.exec_mode.load(Ordering::Relaxed) == EXEC_MODE_BYTECODE {
        match crate::bytecode::compile(program) {
            Ok(chunk) => crate::bytecode::interpret(&chunk, env),
            Err(error) => error,
        }
    } else {
        eval_program(program, env)
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn install_module_bindings(env: &crate::object::EnvRef, exports: Object) {
    let module = Object::Hash(Rc::new(RefCell::new(HashData::default())));
    if let Object::Hash(h) = &module {
        h.borrow_mut().set("exports", exports.clone());
    }
    let mut env = env.borrow_mut();
    env.set_here("exports", exports);
    env.set_here("module", module);
}

fn module_exports(env: &crate::object::EnvRef) -> Option<Object> {
    let module = env.borrow().get("module")?;
    match module {
        Object::Hash(h) => h.borrow().get("exports").cloned(),
        _ => None,
    }
}

fn load_json_module(path: &Path) -> RuntimeResult<Object> {
    let source = fs::read_to_string(path).map_err(|e| {
        new_error(
            Default::default(),
            format!("ImportError: cannot read {}: {}", path.display(), e),
        )
    })?;
    json_to_object(
        serde_json::from_str::<serde_json::Value>(&source).map_err(|e| {
            new_error(
                Default::default(),
                format!(
                    "ImportError: cannot parse JSON module {}: {}",
                    path.display(),
                    e
                ),
            )
        })?,
    )
    .map_err(|e| new_error(Default::default(), format!("ImportError: {e}")))
}

fn json_to_object(value: serde_json::Value) -> Result<Object, String> {
    Ok(match value {
        serde_json::Value::Null => Object::Null,
        serde_json::Value::Bool(value) => Object::Boolean(value),
        serde_json::Value::Number(value) => Object::Number(
            value
                .as_f64()
                .ok_or_else(|| format!("JSON number {} is not representable", value))?,
        ),
        serde_json::Value::String(value) => str_obj(value),
        serde_json::Value::Array(values) => Object::Array(Rc::new(RefCell::new(ArrayData {
            elements: values
                .into_iter()
                .map(json_to_object)
                .collect::<Result<Vec<_>, _>>()?,
        }))),
        serde_json::Value::Object(values) => {
            let hash = Rc::new(RefCell::new(HashData::default()));
            for (key, value) in values {
                hash.borrow_mut().set(key, json_to_object(value)?);
            }
            Object::Hash(hash)
        }
    })
}
