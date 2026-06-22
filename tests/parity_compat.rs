use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Clone, Copy)]
enum Invocation {
    RunFile(&'static str),
    ProjectEntry,
}

struct FixtureCase {
    name: &'static str,
    dir: &'static str,
    invocation: Invocation,
    expected_stdout: &'static str,
}

struct CliRunner {
    name: &'static str,
    command: RunnerCommand,
}

enum RunnerCommand {
    Executable(PathBuf),
}

impl CliRunner {
    fn rust() -> Self {
        Self {
            name: "rust-cli",
            command: RunnerCommand::Executable(PathBuf::from(env!("CARGO_BIN_EXE_gs"))),
        }
    }

    fn run(&self, fixture_dir: &Path, invocation: Invocation) -> Output {
        let mut command = match &self.command {
            RunnerCommand::Executable(executable) => Command::new(executable),
        };
        command.arg("run").current_dir(fixture_dir);

        if let Invocation::RunFile(script) = invocation {
            command.arg(script);
        }

        command.output().expect("run parity fixture")
    }
}

#[test]
fn rust_cli_matches_parity_fixtures() {
    let runner = CliRunner::rust();
    for case in expected_fixture_cases() {
        assert_expected(&runner, &case);
    }
    for case in discovered_fixture_cases() {
        assert_success(&runner, &case);
    }
}

#[test]
fn optional_go_cli_matches_rust_cli() {
    let runner = parity_runner_script();
    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&runner)
        .arg("-FixtureRoot")
        .arg(parity_root())
        .arg("-RustGs")
        .arg(env!("CARGO_BIN_EXE_gs"))
        .arg("-AllowSkip")
        .output()
        .expect("run scripts/parity-runner.ps1");

    assert!(
        output.status.success(),
        "parity runner failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_expected(runner: &CliRunner, case: &FixtureCase) {
    let fixture_dir = parity_root().join(case.dir);
    let output = runner.run(&fixture_dir, case.invocation);

    assert_success_output(runner, case, &output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        case.expected_stdout,
        "{} stdout mismatch for fixture {}",
        runner.name,
        case.name
    );
}

fn assert_success(runner: &CliRunner, case: &FixtureCase) {
    let fixture_dir = parity_root().join(case.dir);
    let output = runner.run(&fixture_dir, case.invocation);

    assert_success_output(runner, case, &output);
}

fn assert_success_output(runner: &CliRunner, case: &FixtureCase, output: &Output) {
    assert!(
        output.status.success(),
        "{} failed fixture {}:\n{}",
        runner.name,
        case.name,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{} stderr should be empty for fixture {}:\n{}",
        runner.name,
        case.name,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn expected_fixture_cases() -> Vec<FixtureCase> {
    vec![
        FixtureCase {
            name: "basic expression",
            dir: "basic_expression",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "basic-expression=1\n",
        },
        FixtureCase {
            name: "function call",
            dir: "function_call",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "function-call=14\n",
        },
        FixtureCase {
            name: "relative require",
            dir: "relative_require",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "relative-require=18\n",
        },
        FixtureCase {
            name: "project.toml entry",
            dir: "project_entry",
            invocation: Invocation::ProjectEntry,
            expected_stdout: "project-entry=ok\n",
        },
        FixtureCase {
            name: "control flow",
            dir: "control_flow",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "control-flow=8\n",
        },
        FixtureCase {
            name: "for break",
            dir: "for_break",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "for-break=6\n",
        },
        FixtureCase {
            name: "while continue",
            dir: "while_continue",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "while-continue=18\n",
        },
        FixtureCase {
            name: "nested loops",
            dir: "nested_loops",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "nested-loops=111213212223\n",
        },
        FixtureCase {
            name: "loop array build",
            dir: "loop_array_build",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "loop-array-build=0|1|4|9\n",
        },
        FixtureCase {
            name: "arrays and objects",
            dir: "arrays_objects",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "arrays-objects=3:3:8:gts:1\n",
        },
        FixtureCase {
            name: "array reduce",
            dir: "array_reduce",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "array-reduce=10\n",
        },
        FixtureCase {
            name: "array slice join",
            dir: "array_slice_join",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "array-slice-join=one:two:4\n",
        },
        FixtureCase {
            name: "array index assignment",
            dir: "array_index_assignment",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "array-index-assignment=1,4,3\n",
        },
        FixtureCase {
            name: "array shift unshift",
            dir: "array_shift_unshift",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "array-shift-unshift=1:2|3\n",
        },
        FixtureCase {
            name: "array find index",
            dir: "array_find_index",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "array-find-index=8:3\n",
        },
        FixtureCase {
            name: "object nested access",
            dir: "object_nested_access",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "object-nested-access=ada:12\n",
        },
        FixtureCase {
            name: "object method call",
            dir: "object_method_call",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "object-method-call=10:10\n",
        },
        FixtureCase {
            name: "object computed key",
            dir: "object_computed_key",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "object-computed-key=14:14\n",
        },
        FixtureCase {
            name: "string methods",
            dir: "string_methods",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "string-methods=ALPHA:4\n",
        },
        FixtureCase {
            name: "template literals",
            dir: "template_literals",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "template-literals=gts:9\n",
        },
        FixtureCase {
            name: "truthy logic",
            dir: "truthy_logic",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "truthy-logic=start:ok\n",
        },
        FixtureCase {
            name: "comparison edges",
            dir: "comparison_edges",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "comparison-edges=ok\n",
        },
        FixtureCase {
            name: "function closure",
            dir: "function_closure",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "function-closure=13\n",
        },
        FixtureCase {
            name: "closure counter",
            dir: "closure_counter",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "closure-counter=1:2:3\n",
        },
        FixtureCase {
            name: "closure iife",
            dir: "closure_iife",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "closure-iife=goscript\n",
        },
        FixtureCase {
            name: "closure returned frame",
            dir: "closure_returned_frame",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "closure-returned-frame=42\n",
        },
        FixtureCase {
            name: "recursive function",
            dir: "recursive_function",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "recursive-function=120\n",
        },
        FixtureCase {
            name: "class basic",
            dir: "class_basic",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "class-basic=7:7\n",
        },
        FixtureCase {
            name: "class inheritance method",
            dir: "class_inheritance_method",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "class-inheritance-method=10\n",
        },
        FixtureCase {
            name: "class inheritance constructor",
            dir: "class_inheritance_constructor",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "class-inheritance-constructor=12\n",
        },
        FixtureCase {
            name: "class implicit super",
            dir: "class_implicit_super",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "class-implicit-super=10\n",
        },
        FixtureCase {
            name: "class super method override",
            dir: "class_super_method_override",
            invocation: Invocation::RunFile("main.gs"),
            // Exercises the override + super.method() call dispatch path,
            // which previously failed with "undefined is not a function".
            expected_stdout: "class-super-method-override=child:base:106\n",
        },
        FixtureCase {
            name: "class method this",
            dir: "class_method_this",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "class-method-this=2:ab\n",
        },
        FixtureCase {
            name: "class field update",
            dir: "class_field_update",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "class-field-update=12\n",
        },
        FixtureCase {
            name: "try catch finally",
            dir: "try_catch",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "try-catch=boom:finally\n",
        },
        FixtureCase {
            name: "try finally no throw",
            dir: "try_finally_no_throw",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "try-finally-no-throw=body:try:finally\n",
        },
        FixtureCase {
            name: "throw catch string",
            dir: "throw_catch_string",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "throw-catch-string=boom\n",
        },
        FixtureCase {
            name: "throw catch error",
            dir: "throw_catch_error",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "throw-catch-error=boom\n",
        },
        FixtureCase {
            name: "catch finally order",
            dir: "catch_finally_order",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "catch-finally-order=start:catch:finally\n",
        },
        FixtureCase {
            name: "match basic",
            dir: "match_basic",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "match-basic=two\n",
        },
        FixtureCase {
            name: "match default only",
            dir: "match_default_only",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "match-default-only=fallback\n",
        },
        FixtureCase {
            name: "match string",
            dir: "match_string",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "match-string=go\n",
        },
        FixtureCase {
            name: "match boolean",
            dir: "match_boolean",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "match-boolean=no\n",
        },
        FixtureCase {
            name: "match block body",
            dir: "match_block_body",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "match-block-body=hit:6\n",
        },
        FixtureCase {
            name: "match no arm catch",
            dir: "match_no_arm_catch",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "match-no-arm-catch=MatchError\n",
        },
        FixtureCase {
            name: "match null",
            dir: "match_null",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "match-null=nil\n",
        },
        FixtureCase {
            name: "export const",
            dir: "export_const",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "export-const=export:42\n",
        },
        FixtureCase {
            name: "export function alias",
            dir: "export_function_alias",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "export-function-alias=18\n",
        },
        FixtureCase {
            name: "module exports object",
            dir: "module_exports_object",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "module-exports-object=42\n",
        },
        FixtureCase {
            name: "module cache",
            dir: "module_cache",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "module-cache=1:1\n",
        },
        FixtureCase {
            name: "import default like",
            dir: "import_default_like",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "import-default-like=12\n",
        },
        FixtureCase {
            name: "directory module index",
            dir: "directory_module_index",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "directory-module-index=42\n",
        },
        FixtureCase {
            name: "project module require",
            dir: "project_module_require",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "project-module-require=42\n",
        },
        FixtureCase {
            name: "nested relative require",
            dir: "nested_relative_require",
            invocation: Invocation::RunFile("main.gs"),
            expected_stdout: "nested-relative-require=21\n",
        },
    ]
}

fn discovered_fixture_cases() -> Vec<FixtureCase> {
    let mut cases = Vec::new();
    let entries = std::fs::read_dir(parity_root()).expect("read parity fixtures");
    for entry in entries {
        let entry = entry.expect("read fixture entry");
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry
            .file_name()
            .into_string()
            .expect("fixture names must be utf-8");
        let dir: &'static str = Box::leak(dir_name.into_boxed_str());
        let invocation = if path.join("project.toml").exists() {
            Invocation::ProjectEntry
        } else {
            Invocation::RunFile("main.gs")
        };
        cases.push(FixtureCase {
            name: dir,
            dir,
            invocation,
            expected_stdout: "",
        });
    }
    cases.sort_by_key(|case| case.dir);
    cases
}

fn parity_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity")
}

fn parity_runner_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/parity-runner.ps1")
}
