use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use gts::object::{Object, EXEC_MODE_BYTECODE, EXEC_MODE_TREEWALK};
use gts::packagefile;
use gts::runtime::{RunOptions, Session};
use gts::VERSION;

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let program = args.first().cloned().unwrap_or_else(|| "gs".to_string());
    let cli_args = args.into_iter().skip(1).collect::<Vec<_>>();

    if packagefile::current_executable_has_appended_package() && should_run_embedded_app(&cli_args)
    {
        run_embedded_app(cli_args);
        return;
    }

    match parse_cli(cli_args) {
        Ok(Command::Help) => print_help(&program),
        Ok(Command::Version) => println!("GoScript {}", VERSION),
        Ok(Command::Repl) => run_repl(),
        Ok(Command::Init { dir }) => init_project(dir),
        Ok(Command::Run {
            script,
            script_args,
            call_main,
            timeout,
            default_workers,
            exec_mode,
        }) => run_script(
            script,
            script_args,
            call_main,
            timeout,
            default_workers,
            exec_mode,
        ),
        Ok(Command::RunScript {
            script,
            script_args,
            timeout,
            exec_mode,
        }) => run_script(script, script_args, true, timeout, None, exec_mode),
        Ok(Command::Pack { dir, output }) => pack_project(dir, output),
        Ok(Command::Dist { dir, output }) => dist_project(dir, output),
        Ok(Command::Bundle { entry, output }) => bundle_project(entry, output),
        Ok(Command::Lsp) => run_lsp(),
        Ok(Command::ApiDoc { module }) => print_api_doc(module),
        Ok(Command::CheckTypes) => {
            eprintln!("--check-types is not implemented yet");
            process::exit(1);
        }
        Err(message) => {
            eprintln!("{message}");
            eprintln!("Run '{program} --help' for usage.");
            process::exit(2);
        }
    }
}

fn should_run_embedded_app(args: &[String]) -> bool {
    if args.is_empty() {
        return true;
    }
    if let Some(index) = args.iter().position(|arg| arg == "--") {
        return index == 0;
    }
    !matches!(
        args[0].as_str(),
        "init" | "run" | "run-script" | "pack" | "dist" | "bundle" | "lsp" | "help" | "version"
    ) && !is_existing_script_arg(&args[0])
}

fn is_existing_script_arg(arg: &str) -> bool {
    if arg.starts_with('-') {
        return false;
    }
    PathBuf::from(arg).is_file()
}

fn run_embedded_app(args: Vec<String>) {
    let app_args = if args.first().is_some_and(|arg| arg == "--") {
        args.into_iter().skip(1).collect()
    } else {
        args
    };
    let app_dir = match packagefile::extract_appended_package(
        env::current_exe().unwrap_or_else(|_| PathBuf::from(".")),
    ) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Error extracting embedded package: {e}");
            process::exit(1);
        }
    };
    run_script(
        default_entry_in(&app_dir),
        app_args,
        true,
        None,
        None,
        EXEC_MODE_BYTECODE,
    );
}
#[derive(Debug)]
enum Command {
    Help,
    Version,
    Repl,
    Init {
        dir: PathBuf,
    },
    Run {
        script: PathBuf,
        script_args: Vec<String>,
        call_main: bool,
        timeout: Option<Duration>,
        default_workers: Option<usize>,
        exec_mode: u8,
    },
    RunScript {
        script: PathBuf,
        script_args: Vec<String>,
        timeout: Option<Duration>,
        exec_mode: u8,
    },
    Pack {
        dir: PathBuf,
        output: Option<PathBuf>,
    },
    Dist {
        dir: PathBuf,
        output: Option<PathBuf>,
    },
    Bundle {
        entry: PathBuf,
        output: Option<PathBuf>,
    },
    Lsp,
    ApiDoc {
        module: String,
    },
    CheckTypes,
}

fn parse_cli(args: Vec<String>) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Repl);
    }

    let mut index = 0;
    let mut options = CliOptions::default();
    match parse_global_flags(&args, &mut index, &mut options)? {
        FlagAction::Continue => {}
        FlagAction::Help => return Ok(Command::Help),
        FlagAction::Version => return Ok(Command::Version),
        FlagAction::CheckTypes => return Ok(Command::CheckTypes),
        FlagAction::ApiDoc(module) => return Ok(Command::ApiDoc { module }),
    }

    if index >= args.len() {
        return Ok(Command::Help);
    }

    match args[index].as_str() {
        "help" => Ok(Command::Help),
        "version" => Ok(Command::Version),
        "init" => {
            index += 1;
            let dir = match args.len() - index {
                0 => PathBuf::from("."),
                1 => PathBuf::from(&args[index]),
                _ => return Err("init expects at most 1 argument: [dir]".to_string()),
            };
            Ok(Command::Init { dir })
        }
        "run" => {
            index += 1;
            match parse_global_flags(&args, &mut index, &mut options)? {
                FlagAction::Continue => {}
                FlagAction::Help => return Ok(Command::Help),
                FlagAction::Version => return Ok(Command::Version),
                FlagAction::CheckTypes => return Ok(Command::CheckTypes),
                FlagAction::ApiDoc(module) => return Ok(Command::ApiDoc { module }),
            }
            let script = args
                .get(index)
                .map(PathBuf::from)
                .unwrap_or_else(default_entry);
            let script_args = if index < args.len() {
                args.iter().skip(index + 1).cloned().collect()
            } else {
                Vec::new()
            };
            Ok(Command::Run {
                script,
                script_args,
                call_main: true,
                timeout: options.timeout,
                default_workers: options.workers,
                exec_mode: options.exec_mode,
            })
        }
        "run-script" => {
            index += 1;
            match parse_global_flags(&args, &mut index, &mut options)? {
                FlagAction::Continue => {}
                FlagAction::Help => return Ok(Command::Help),
                FlagAction::Version => return Ok(Command::Version),
                FlagAction::CheckTypes => return Ok(Command::CheckTypes),
                FlagAction::ApiDoc(module) => return Ok(Command::ApiDoc { module }),
            }
            let script = args
                .get(index)
                .ok_or_else(|| "run-script expects: <script.gs> [args...]".to_string())
                .map(PathBuf::from)?;
            let script_args = if index < args.len() {
                args.iter().skip(index + 1).cloned().collect()
            } else {
                Vec::new()
            };
            Ok(Command::RunScript {
                script,
                script_args,
                timeout: options.timeout,
                exec_mode: options.exec_mode,
            })
        }
        "pack" => {
            index += 1;
            let dir = args
                .get(index)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let output = args.get(index + 1).map(PathBuf::from);
            if args.len() - index > 2 {
                return Err("pack expects at most 2 arguments: [dir] [out.gspkg]".to_string());
            }
            Ok(Command::Pack { dir, output })
        }
        "dist" => {
            index += 1;
            let dir = args
                .get(index)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let output = args.get(index + 1).map(PathBuf::from);
            if args.len() - index > 2 {
                return Err("dist expects at most 2 arguments: [dir] [out]".to_string());
            }
            Ok(Command::Dist { dir, output })
        }
        "bundle" => {
            index += 1;
            let entry = args
                .get(index)
                .ok_or_else(|| "bundle expects: <entry.gs> [out.gs]".to_string())
                .map(PathBuf::from)?;
            let output = args.get(index + 1).map(PathBuf::from);
            if args.len() - index > 2 {
                return Err("bundle expects at most 2 arguments: <entry.gs> [out.gs]".to_string());
            }
            Ok(Command::Bundle { entry, output })
        }
        "lsp" => Ok(Command::Lsp),
        script => {
            let script_args = args.iter().skip(index + 1).cloned().collect();
            let script = PathBuf::from(script);
            let call_main = script
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("main.gs"));
            Ok(Command::Run {
                script,
                script_args,
                call_main,
                timeout: options.timeout,
                default_workers: options.workers,
                exec_mode: options.exec_mode,
            })
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum FlagAction {
    Continue,
    Help,
    Version,
    CheckTypes,
    ApiDoc(String),
}

struct CliOptions {
    timeout: Option<Duration>,
    workers: Option<usize>,
    exec_mode: u8,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            timeout: None,
            workers: None,
            exec_mode: EXEC_MODE_BYTECODE,
        }
    }
}

fn parse_global_flags(
    args: &[String],
    index: &mut usize,
    options: &mut CliOptions,
) -> Result<FlagAction, String> {
    let mut saw_check_types = false;
    while *index < args.len() {
        match args[*index].as_str() {
            "-h" | "--help" => return Ok(FlagAction::Help),
            "-v" | "--version" => return Ok(FlagAction::Version),
            "--check-types" => {
                saw_check_types = true;
                *index += 1;
            }
            "--api_doc" => {
                *index += 1;
                let module = args
                    .get(*index)
                    .ok_or_else(|| "--api_doc requires a module name or 'all'".to_string())?
                    .clone();
                *index += 1;
                return Ok(FlagAction::ApiDoc(module));
            }
            "--timeout" => {
                *index += 1;
                let value = args
                    .get(*index)
                    .ok_or_else(|| "--timeout requires a duration".to_string())?;
                let timeout = parse_duration(value)?;
                if !timeout.is_zero() {
                    options.timeout = Some(timeout);
                }
                *index += 1;
            }
            "--workers" => {
                *index += 1;
                let value = args
                    .get(*index)
                    .ok_or_else(|| "--workers requires a positive integer".to_string())?;
                options.workers = Some(parse_workers(value)?);
                *index += 1;
            }
            "--exec-mode" => {
                *index += 1;
                let value = args
                    .get(*index)
                    .ok_or_else(|| "--exec-mode requires 'bytecode' or 'tree'".to_string())?;
                options.exec_mode = parse_exec_mode(value)?;
                *index += 1;
            }
            value if value.starts_with("--exec-mode=") => {
                let mode = value
                    .strip_prefix("--exec-mode=")
                    .expect("prefix checked above");
                if mode.is_empty() {
                    return Err("--exec-mode requires 'bytecode' or 'tree'".to_string());
                }
                options.exec_mode = parse_exec_mode(mode)?;
                *index += 1;
            }
            "--" => {
                *index += 1;
                break;
            }
            _ => break,
        }
    }

    if saw_check_types {
        Ok(FlagAction::CheckTypes)
    } else {
        Ok(FlagAction::Continue)
    }
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let (number, unit) = split_duration(value)?;
    let amount = number
        .parse::<u64>()
        .map_err(|_| format!("invalid --timeout duration '{value}'"))?;

    match unit {
        "" | "s" => Ok(Duration::from_secs(amount)),
        "ms" => Ok(Duration::from_millis(amount)),
        "m" => Ok(Duration::from_secs(amount.saturating_mul(60))),
        "h" => Ok(Duration::from_secs(amount.saturating_mul(60 * 60))),
        _ => Err(format!("invalid --timeout duration '{value}'")),
    }
}

fn split_duration(value: &str) -> Result<(&str, &str), String> {
    let unit_start = value
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(value.len());
    if unit_start == 0 {
        return Err(format!("invalid --timeout duration '{value}'"));
    }
    Ok(value.split_at(unit_start))
}

fn parse_workers(value: &str) -> Result<usize, String> {
    let workers = value
        .parse::<usize>()
        .map_err(|_| format!("invalid --workers value '{value}'"))?;
    if workers == 0 {
        Err("--workers requires a positive integer".to_string())
    } else {
        Ok(workers)
    }
}

fn parse_exec_mode(value: &str) -> Result<u8, String> {
    match value {
        "bytecode" | "vm" => Ok(EXEC_MODE_BYTECODE),
        "tree" | "treewalk" | "tree-walk" => Ok(EXEC_MODE_TREEWALK),
        _ => Err(format!(
            "invalid --exec-mode value '{value}' (expected 'bytecode' or 'tree')"
        )),
    }
}

fn run_script(
    script: PathBuf,
    script_args: Vec<String>,
    call_main: bool,
    timeout: Option<Duration>,
    default_workers: Option<usize>,
    exec_mode: u8,
) {
    // Expose --workers as an environment default the script can read via
    // @std/env (e.g. `app.listen(port, { workers: process.env.GTS_DEFAULT_WORKERS })`).
    // app.listen({ workers: N }) in the script itself always takes precedence.
    if let Some(n) = default_workers {
        // SAFETY: single-threaded CLI startup before the VM runs; the env var
        // is read by worker threads but only after being set here.
        std::env::set_var("GTS_DEFAULT_WORKERS", n.to_string());
    }
    let session = Session::new();
    session
        .vm()
        .exec_mode
        .store(exec_mode, std::sync::atomic::Ordering::Relaxed);
    match session.run_file_with_options(
        &script,
        RunOptions {
            argv: script_args,
            call_main,
            timeout,
        },
    ) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e.inspect());
            process::exit(1);
        }
    }
}

fn init_project(dir: PathBuf) {
    match write_project_files(dir) {
        Ok(path) => println!("{}", path.display()),
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

fn write_project_files(dir: PathBuf) -> Result<PathBuf, String> {
    let abs_dir = if dir.is_absolute() {
        dir
    } else {
        env::current_dir().map_err(|e| e.to_string())?.join(dir)
    };
    fs::create_dir_all(&abs_dir).map_err(|e| e.to_string())?;

    let name = abs_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("goscript-app");

    let files = [
        ("project.toml", init_project_template(name)),
        ("main.gs", INIT_MAIN_TEMPLATE.to_string()),
    ];

    for (file, _) in &files {
        let path = abs_dir.join(file);
        if path.exists() {
            return Err(format!("{} already exists", path.display()));
        }
    }

    for (file, contents) in files {
        let path = abs_dir.join(file);
        fs::write(&path, contents).map_err(|e| e.to_string())?;
    }

    Ok(abs_dir)
}

fn init_project_template(name: &str) -> String {
    format!(
        "[project]\nname = \"{}\"\nversion = \"0.1.0\"\nentry = \"main.gs\"\n",
        name.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

const INIT_MAIN_TEMPLATE: &str = "function main() {\n  println(\"Hello, GoScript!\");\n}\n";

fn run_repl() {
    if let Err(e) = repl_loop() {
        eprintln!("{e}");
        process::exit(1);
    }
}

fn repl_loop() -> io::Result<()> {
    let session = Session::new();
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut stdout = io::stdout();
    let mut line = String::new();

    writeln!(stdout, "GoScript {}", VERSION)?;
    writeln!(stdout, "Type .help for commands, .exit to quit.")?;

    loop {
        write!(stdout, "gs> ")?;
        stdout.flush()?;

        line.clear();
        if input.read_line(&mut line)? == 0 {
            break;
        }

        let source = line.trim();
        if source.is_empty() {
            continue;
        }
        if source.starts_with('.') {
            match source {
                ".exit" | ".quit" => break,
                ".help" => {
                    writeln!(stdout, "{REPL_HELP}")?;
                }
                _ => eprintln!("unknown command: {source}"),
            }
            continue;
        }

        match session.run_source(source, "<repl>") {
            Ok(Object::Undefined) => {}
            Ok(result) => writeln!(stdout, "{}", result.inspect())?,
            Err(e) => eprintln!("{}", e.inspect()),
        }
    }

    Ok(())
}

const REPL_HELP: &str = ".help        Show this help\n.exit        Exit the REPL";

// ============================================================================
// New command implementations
// ============================================================================

fn pack_project(dir: PathBuf, output: Option<PathBuf>) {
    match packagefile::pack_directory(&dir, output.as_ref()) {
        Ok(path) => {
            println!("Package created: {}", path.display());
        }
        Err(e) => {
            eprintln!("Error packing project: {}", e);
            process::exit(1);
        }
    }
}

fn dist_project(dir: PathBuf, output: Option<PathBuf>) {
    // 第一步：打包项目
    let temp_pkg = match packagefile::pack_directory(&dir, None::<&PathBuf>) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error packing project: {}", e);
            process::exit(1);
        }
    };

    // 第二步：确定输出路径
    let output_path = match output {
        Some(out) => out,
        None => {
            let name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("goscript-app");
            let mut path = dir.join("dist").join(name);
            if cfg!(windows) {
                path.set_extension("exe");
            }
            path
        }
    };

    // 创建 dist 目录
    if let Some(parent) = output_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("Error creating output directory: {}", e);
            process::exit(1);
        }
    }

    // 第三步：获取当前可执行文件作为 stub
    let stub = match env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("Error getting current executable: {}", e);
            process::exit(1);
        }
    };

    // 第四步：附加包到可执行文件
    match packagefile::append_package_to_executable(&stub, &temp_pkg, &output_path) {
        Ok(_) => {
            // 清理临时包文件
            let _ = fs::remove_file(&temp_pkg);
            println!("Standalone executable created: {}", output_path.display());
        }
        Err(e) => {
            eprintln!("Error creating standalone executable: {}", e);
            let _ = fs::remove_file(&temp_pkg);
            process::exit(1);
        }
    }
}

fn bundle_project(entry: PathBuf, output: Option<PathBuf>) {
    match gts::bundler::bundle_modules(&entry, output.as_deref()) {
        Ok(content) => {
            if let Some(out) = &output {
                println!("Bundle created: {}", out.display());
            } else {
                // 如果没有指定输出，打印到标准输出
                print!("{}", content);
            }
        }
        Err(e) => {
            eprintln!("Error bundling modules: {}", e);
            process::exit(1);
        }
    }
}

fn run_lsp() {
    eprintln!("lsp command is not yet implemented");
    eprintln!("This will start the Language Server Protocol server");
    process::exit(1);
}

fn print_api_doc(module: String) {
    if module == "all" {
        // 列出所有模块
        println!("Available standard library modules:\n");
        for mod_name in gts::apidoc::list_all_modules() {
            println!("  {}", mod_name);
        }
        println!("\nUse 'gs --api_doc <module>' to see documentation for a specific module.");
    } else {
        // 显示特定模块的文档
        let docs = gts::apidoc::get_all_stdlib_docs();
        if let Some(doc) = docs.get(&module) {
            print!("{}", gts::apidoc::format_module_doc(doc));
        } else {
            eprintln!("Module '{}' not found.", module);
            eprintln!("\nAvailable modules:");
            for mod_name in gts::apidoc::list_all_modules() {
                eprintln!("  {}", mod_name);
            }
            process::exit(1);
        }
    }
}

fn print_help(program: &str) {
    println!("GoScript {}", VERSION);
    println!();
    println!("Usage:");
    println!("  {program}                          Start REPL");
    println!("  {program} init [dir]               Initialize a new project");
    println!("  {program} <file.gs> [args...]      Run a script file");
    println!("  {program} run [file.gs] [args...]  Run project or file");
    println!("  {program} run-script <script.gs> [args...] Run script");
    println!("  {program} pack [dir] [out.gspkg]   Package project");
    println!("  {program} dist [dir] [out]         Create standalone executable");
    println!("  {program} bundle <entry.gs> [out.gs] Bundle modules into one file");
    println!("  {program} lsp                      Start LSP server (not implemented)");
    println!("  {program} --version                Show version");
    println!();
    println!("Options:");
    println!("  -h, --help                Show this help message");
    println!("  -v, --version             Show version information");
    println!("      --timeout <duration>  Execution timeout (e.g., 10s, 1m, 0 to disable)");
    println!("      --workers <n>         Maximum async worker count");
    println!("      --exec-mode <mode>    Execution backend: bytecode (default) or tree");
    println!("      --check-types         Enable type checking (not implemented)");
    println!("      --api_doc <module>    Print API documentation (module name or 'all')");
    println!();
    println!("Examples:");
    println!("  {program}                   # Start interactive REPL");
    println!("  {program} script.gs         # Run a script");
    println!("  {program} init my-project   # Create new project");
    println!("  {program} pack . app.gspkg  # Package current directory");
    println!("  {program} dist . myapp      # Create standalone executable");
}

fn default_entry() -> PathBuf {
    if let Ok(cwd) = env::current_dir() {
        return default_entry_in(&cwd);
    }
    PathBuf::from("main.gs")
}

fn default_entry_in(dir: &std::path::Path) -> PathBuf {
    if let Some(entry) = gts::module::resolve_entry_in_dir(dir) {
        return entry;
    }
    dir.join("main.gs")
}
