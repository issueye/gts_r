//! Focused tests for the P7 batch-2 native stdlib modules:
//! `@std/encoding/csv`, `@std/template`, `@std/compression`,
//! `@std/compress/gzip`, `@std/terminal`, and `@std/cli`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}-{}", std::process::id()))
}

fn run_script(dir: &PathBuf, script: &str) -> std::process::Output {
    let file = dir.join("main.gs");
    fs::write(&file, script).expect("write script");
    Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&file)
        .output()
        .expect("run gs")
}

fn stdout_of(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_of(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn csv_parses_headers_options_and_round_trips_files() {
    let dir = unique_temp_dir("gts-p7b-csv");
    fs::create_dir_all(&dir).expect("create temp dir");
    let csv_path = dir.join("rows.csv").to_string_lossy().replace('\\', "\\\\");
    let script = format!(
        r##"
let csv = require("@std/encoding/csv");
let rows = csv.parse("#ignore\nname;city\n Ada; Paris\nLin; Taipei\n", {{
  comma: ";",
  comment: "#",
  trimLeadingSpace: true,
  fieldsPerRecord: 2
}});
println(rows.length);
println(rows[0].name, ":", rows[0].city);
let plain = csv.parse("a,b\nc,d\n", {{header: false}});
println(plain[1][0], ":", plain[1][1]);
csv.writeFileSync("{csv_path}", rows, {{comma: ";"}});
let read = csv.readFileSync("{csv_path}", {{comma: ";"}});
println(read[1].name, ":", read[1].city);
let text = csv.stringify([{{name:"Ada", city:"Paris"}}, {{name:"Lin", city:"Taipei"}}]);
println(text.indexOf("city,name") >= 0 || text.indexOf("name,city") >= 0);
"##
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let lines: Vec<String> = stdout_of(&output).lines().map(str::to_string).collect();
    assert_eq!(lines[0], "2");
    assert_eq!(lines[1], "Ada:Paris");
    assert_eq!(lines[2], "c:d");
    assert_eq!(lines[3], "Lin:Taipei");
    assert_eq!(lines[4], "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn template_renders_values_funcs_html_and_files() {
    let dir = unique_temp_dir("gts-p7b-template");
    fs::create_dir_all(&dir).expect("create temp dir");
    let tpl_path = dir.join("hello.tmpl");
    fs::write(&tpl_path, "File {{upper .Name}}").expect("write template");
    let tpl_path = tpl_path.to_string_lossy().replace('\\', "\\\\");
    let script = format!(
        r#"
let t = require("@std/template");
let data = {{Name:" Ada ", Tags:["go","rust"], Raw:"<b>"}};
println(t.render("Hi {{{{trim .Name}}}} {{{{join .Tags \"|\"}}}} {{{{lower \"LOUD\"}}}}", data));
println(t.render("{{{{json .Tags}}}}", data));
println(t.renderHTML("{{{{.Raw}}}}", data));
println(t.escapeHTML("<>&\"'"));
println(t.renderFileSync("{tpl_path}", data));
"#
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let lines: Vec<String> = stdout_of(&output).lines().map(str::to_string).collect();
    assert_eq!(lines[0], "Hi Ada go|rust loud");
    assert_eq!(lines[1], "[\"go\",\"rust\"]");
    assert_eq!(lines[2], "&lt;b&gt;");
    assert_eq!(lines[3], "&lt;&gt;&amp;&#34;&#39;");
    assert_eq!(lines[4], "File  ADA ");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn gzip_modules_round_trip_strings_buffers_and_files() {
    let dir = unique_temp_dir("gts-p7b-gzip");
    fs::create_dir_all(&dir).expect("create temp dir");
    let src = dir.join("src.txt");
    let gz = dir.join("src.txt.gz");
    let out = dir.join("out.txt");
    fs::write(&src, "file payload").expect("write source");
    let src = src.to_string_lossy().replace('\\', "\\\\");
    let gz = gz.to_string_lossy().replace('\\', "\\\\");
    let out = out.to_string_lossy().replace('\\', "\\\\");
    let script = format!(
        r#"
let compression = require("@std/compression");
let packedText = compression.gzipCompress("hello gzip");
println(compression.gzipDecompress(packedText));
let gzip = require("@std/compress/gzip");
let packed = gzip.compress("buffer gzip");
println(packed.length > 0);
println(gzip.decompress(packed));
gzip.compressFileSync("{src}", "{gz}");
gzip.decompressFileSync("{gz}", "{out}");
let fs = require("@std/fs");
println(fs.readFileSync("{out}"));
"#
    );
    let output = run_script(&dir, &script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(
        stdout_of(&output),
        "hello gzip\ntrue\nbuffer gzip\nfile payload\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn terminal_returns_deterministic_ansi_helpers_and_stubs() {
    let dir = unique_temp_dir("gts-p7b-terminal");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let term = require("@std/terminal");
let size = term.size();
println(size.cols > 0, ":", size.rows > 0);
println(term.isTTY());
println(term.capabilities().rawMode);
println(term.clearScreen().length > 0);
println(term.clearLine().length > 0);
println(term.moveTo(2, 3).length > 0);
println(term.style("warn", {fg:"yellow", bold:true}).indexOf("warn") >= 0);
println(term.hyperlink("site", "https://example.com").indexOf("site") >= 0);
println(term.read());
let session = term.start();
println(session.active);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let lines: Vec<String> = stdout_of(&output).lines().map(str::to_string).collect();
    assert_eq!(lines[0], "true:true");
    assert_eq!(lines[1], "false");
    assert_eq!(lines[2], "false");
    assert_eq!(lines[3], "true");
    assert_eq!(lines[4], "true");
    assert_eq!(lines[5], "true");
    assert_eq!(lines[6], "true");
    assert_eq!(lines[7], "true");
    assert_eq!(lines[8], "");
    assert_eq!(lines[9], "false");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_defines_flags_executes_and_validates_args() {
    let dir = unique_temp_dir("gts-p7b-cli");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let cli = require("@std/cli");
let cmd = cli.command({use:"serve [path]", short:"Run server", args: cli.exactArgs(1)});
let flags = cmd.flags();
flags.string("host", "H", "127.0.0.1", "host name");
flags.bool("verbose", "v", false, "verbose output");
flags.int("port", "p", 3000, "port");
flags.number("ratio", "r", 1.5, "ratio");
println(flags.get("host"), ":", flags.changed("host"));
println(cmd.execute(["--host", "0.0.0.0", "-v", "-p", "8080", "--ratio=2.25", "app"]));
println(flags.get("host"), ":", flags.changed("host"));
println(flags.get("verbose"), ":", flags.get("port"), ":", flags.get("ratio"));
println(cmd.flag("port"));
println(cmd.usage().indexOf("serve [path]") >= 0);
println(cmd.commandPath());
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let lines: Vec<String> = stdout_of(&output).lines().map(str::to_string).collect();
    assert_eq!(lines[0], "127.0.0.1:false");
    assert_eq!(lines[1], "0");
    assert_eq!(lines[2], "0.0.0.0:true");
    assert_eq!(lines[3], "true:8080:2.25");
    assert_eq!(lines[4], "8080");
    assert_eq!(lines[5], "true");
    assert_eq!(lines[6], "serve");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_arg_validator_reports_errors() {
    let dir = unique_temp_dir("gts-p7b-cli-err");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let cli = require("@std/cli");
let cmd = cli.command({use:"one", args: cli.exactArgs(1)});
println(cmd.execute([]));
"#,
    );
    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains("cli: accepts 1 argument"),
        "got stderr: {}",
        stderr_of(&output)
    );
    let _ = fs::remove_dir_all(dir);
}
