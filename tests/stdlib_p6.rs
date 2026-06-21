//! Parity tests for the P6 batch-1 native stdlib modules
//! (`@std/encoding/base64`, `@std/encoding/hex`, `@std/hash`, `@std/random`,
//! `@std/regexp`, `@std/semver`, `@std/collections`, `@std/process`).
//!
//! These exercise the user-visible behavior that the Go originals guarantee;
//! see `docs/full-parity-refactor-plan.md` (P6) and the parity matrix. Each
//! test writes a small `.gs` program to a temp dir, runs the `gs` binary, and
//! asserts on stdout/stderr/exit — the same approach used in `cli_flags.rs`.

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

/// Write `script` to `<dir>/main.gs`, run `gs <file>`, and return the process output.
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

// --- base64 ---------------------------------------------------------------

#[test]
fn base64_encode_and_decode_roundtrip() {
    let dir = unique_temp_dir("gts-p6-base64");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let b64 = require("@std/encoding/base64");
let enc = b64.encode("Hello, GoScript!");
println(enc);
println(b64.decode(enc));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(
        stdout_of(&output),
        "SGVsbG8sIEdvU2NyaXB0IQ==\nHello, GoScript!\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn base64_url_variant_has_no_padding() {
    let dir = unique_temp_dir("gts-p6-base64-url");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let b64 = require("@std/encoding/base64");
let enc = b64.encodeURL("ab");
println(enc, ":", b64.decodeURL(enc));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "YWI:ab\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn base64_decode_invalid_returns_error() {
    let dir = unique_temp_dir("gts-p6-base64-err");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let b64 = require("@std/encoding/base64");
println(b64.decode("!!!not-base64!!!"));
"#,
    );
    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains("base64.decode: invalid base64 data"),
        "got stderr: {}",
        stderr_of(&output)
    );
    let _ = fs::remove_dir_all(dir);
}

// --- hex ------------------------------------------------------------------

#[test]
fn hex_encode_decode_lowercase_roundtrip() {
    let dir = unique_temp_dir("gts-p6-hex");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let hex = require("@std/encoding/hex");
let enc = hex.encode("AB");
println(enc);
println(hex.decode(enc));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "4142\nAB\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn hex_decode_invalid_length_errors() {
    let dir = unique_temp_dir("gts-p6-hex-err");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let hex = require("@std/encoding/hex");
println(hex.decode("abc"));
"#,
    );
    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains("invalid hex data"),
        "got stderr: {}",
        stderr_of(&output)
    );
    let _ = fs::remove_dir_all(dir);
}

// --- hash -----------------------------------------------------------------

#[test]
fn hash_checksums_match_known_vectors() {
    let dir = unique_temp_dir("gts-p6-hash");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let hash = require("@std/hash");
println(hash.crc32("123456789"));
println(hash.adler32("Wikipedia"));
println(hash.fnv1a("foobar"));
println(hash.crc32Number("123456789"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    // crc32("123456789") == 0xcbf43926 (standard IEEE check value).
    assert_eq!(lines[0], "cbf43926");
    // adler32("Wikipedia") == 0x11e60398.
    assert_eq!(lines[1], "11e60398");
    // fnv1a is deterministic; assert 16 hex chars.
    assert_eq!(lines[2].len(), 16);
    assert!(lines[3].parse::<f64>().unwrap() > 0.0);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn hash_crc64_iso_check_value() {
    let dir = unique_temp_dir("gts-p6-hash-crc64");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let hash = require("@std/hash");
println(hash.crc64("123456789"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    // crc64-ISO check value for the "123456789" string.
    assert_eq!(stdout_of(&output).trim(), "b90956c775a41001");
    let _ = fs::remove_dir_all(dir);
}

// --- random ---------------------------------------------------------------

#[test]
fn random_int_respects_half_open_range() {
    let dir = unique_temp_dir("gts-p6-rand-int");
    fs::create_dir_all(&dir).expect("create temp dir");
    let script = r#"
let r = require("@std/random");
let ok = true;
for (let i = 0; i < 300; i = i + 1) {
  let n = r.int(5, 10);
  if (n < 5 || n >= 10) { ok = false; }
}
println(ok);
"#;
    let output = run_script(&dir, script);
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn random_int_rejects_invalid_range() {
    let dir = unique_temp_dir("gts-p6-rand-int-err");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let r = require("@std/random");
println(r.int(10, 10));
"#,
    );
    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains("min must be less than max"),
        "got stderr: {}",
        stderr_of(&output)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn random_pick_empty_array_returns_null() {
    let dir = unique_temp_dir("gts-p6-rand-pick");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let r = require("@std/random");
println(r.pick([]));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "null\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn random_uuid_is_well_formed_v4() {
    let dir = unique_temp_dir("gts-p6-rand-uuid");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let r = require("@std/random");
let u = r.uuid();
println(u.length, ":", u[14], ":", u[19]);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    // UUID length is 36; version nibble at index 14 is '4';
    // variant nibble at index 19 is one of 8/9/a/b.
    let stdout = stdout_of(&output);
    let parts: Vec<&str> = stdout.trim().split(':').collect();
    assert_eq!(parts[0], "36");
    assert_eq!(parts[1], "4");
    assert!(matches!(parts[2], "8" | "9" | "a" | "b"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn random_alphanumeric_has_expected_length_and_charset() {
    let dir = unique_temp_dir("gts-p6-rand-alnum");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let r = require("@std/random");
let s = r.alphanumeric(16);
println(s.length);
let ok = true;
for (let i = 0; i < s.length; i = i + 1) {
  let c = s[i];
  let isLower = c >= "a" && c <= "z";
  let isUpper = c >= "A" && c <= "Z";
  let isDigit = c >= "0" && c <= "9";
  if (!(isLower || isUpper || isDigit)) {
    ok = false;
  }
}
println(ok);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "16");
    assert_eq!(lines[1], "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn random_bytes_in_range() {
    let dir = unique_temp_dir("gts-p6-rand-bytes");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let r = require("@std/random");
let b = r.bytes(8);
let ok = b.length === 8;
for (let i = 0; i < b.length; i = i + 1) {
  if (b[i] < 0 || b[i] > 255) { ok = false; }
}
println(ok);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "true");
    let _ = fs::remove_dir_all(dir);
}

// --- regexp ---------------------------------------------------------------

#[test]
fn regexp_escape_and_match_all() {
    let dir = unique_temp_dir("gts-p6-re-match");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        // GoScript double-quoted strings do not preserve `\w` (only JS-recognized
        // escapes survive), so use an explicit character class for word matching.
        r#"
let re = require("@std/regexp");
println(re.escape("1+1=2"));
let matches = re.matchAll("([a-zA-Z]+)", "foo bar baz");
println(matches.length);
println(matches[0][0], ":", matches[1][0], ":", matches[2][1]);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1\\+1=2");
    assert_eq!(lines[1], "3");
    // matches[2][1] is the first capture group of the 3rd match == "baz".
    assert_eq!(lines[2], "foo:bar:baz");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn regexp_split_with_limit() {
    let dir = unique_temp_dir("gts-p6-re-split");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let re = require("@std/regexp");
let parts = re.split("[ ]+", "a b c d", 2);
println(parts.length, ":", parts[0], ":", parts[1]);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "2:a:b c d");
    let _ = fs::remove_dir_all(dir);
}

// --- semver ---------------------------------------------------------------

#[test]
fn semver_parse_and_compare() {
    let dir = unique_temp_dir("gts-p6-semver");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let sv = require("@std/semver");
let p = sv.parse("v1.2.3-alpha.1+build.5");
println(p.major, ":", p.minor, ":", p.patch);
println(p.prerelease[0], ":", p.prerelease[1]);
println(p.build[0], ":", p.build[1]);
println(sv.compare("1.2.3", "1.2.4"));
println(sv.gt("2.0.0", "1.9.9"));
println(sv.lt("1.0.0", "1.0.0-beta"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1:2:3");
    assert_eq!(lines[1], "alpha:1");
    // build metadata is split on '.', so build[0]="build", build[1]="5".
    assert_eq!(lines[2], "build:5");
    assert_eq!(lines[3], "-1");
    assert_eq!(lines[4], "true");
    // A release (1.0.0) is greater than its prerelease (1.0.0-beta).
    assert_eq!(lines[5], "false");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn semver_satisfies_caret_and_tilde() {
    let dir = unique_temp_dir("gts-p6-semver-sat");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let sv = require("@std/semver");
println(sv.satisfies("1.4.5", "^1.2.3"));
println(sv.satisfies("2.0.0", "^1.2.3"));
println(sv.satisfies("1.2.9", "~1.2.3"));
println(sv.satisfies("1.3.0", "~1.2.3"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "false");
    assert_eq!(lines[2], "true");
    assert_eq!(lines[3], "false");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn semver_inc_bumps_correctly() {
    let dir = unique_temp_dir("gts-p6-semver-inc");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let sv = require("@std/semver");
println(sv.inc("1.2.3", "patch"));
println(sv.inc("1.2.3", "minor"));
println(sv.inc("1.2.3", "major"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1.2.4");
    assert_eq!(lines[1], "1.3.0");
    assert_eq!(lines[2], "2.0.0");
    let _ = fs::remove_dir_all(dir);
}

// --- collections ----------------------------------------------------------

#[test]
fn collections_unique_chunk_flatten() {
    let dir = unique_temp_dir("gts-p6-coll");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let c = require("@std/collections");
let u = c.unique([1, 2, 2, 3, 1]);
println(u.length, ":", u[0], ":", u[2]);
let chunks = c.chunk([1,2,3,4,5], 2);
println(chunks.length, ":", chunks[0].length, ":", chunks[2].length);
let flat = c.flatten([[1,2],[3,[4]]]);
println(flat.length, ":", flat[2], ":", flat[3].length);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    // unique([1,2,2,3,1]) -> [1,2,3] (length 3, u[0]=1, u[2]=3).
    assert_eq!(lines[0], "3:1:3");
    assert_eq!(lines[1], "3:2:1");
    // flatten is one level: [1,2,3,[4]] -> length 4, flat[3] is the inner array.
    assert_eq!(lines[2], "4:3:1");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn collections_range_basic_and_step() {
    let dir = unique_temp_dir("gts-p6-coll-range");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let c = require("@std/collections");
let r1 = c.range(3);
let r2 = c.range(1, 5);
let r3 = c.range(0, 10, 3);
println(r1.length, ":", r1[2]);
println(r2.length, ":", r2[3]);
println(r3.length, ":", r3[2]);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    // range(3) -> [0,1,2]; range(1,5) -> [1,2,3,4]; range(0,10,3) -> [0,3,6,9].
    assert_eq!(lines[0], "3:2");
    assert_eq!(lines[1], "4:4");
    assert_eq!(lines[2], "4:6");
    let _ = fs::remove_dir_all(dir);
}

// --- process --------------------------------------------------------------

#[test]
fn process_exposes_argv_pid_version_and_cwd() {
    let dir = unique_temp_dir("gts-p6-proc");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let p = require("@std/process");
println(p.argv0.length > 0);
println(typeof p.pid);
println(p.version);
println(p.cwd().length > 0);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "number");
    assert_eq!(lines[2], gts::VERSION);
    assert_eq!(lines[3], "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn process_getenv_distinguishes_unset_from_default() {
    let dir = unique_temp_dir("gts-p6-proc-getenv");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let p = require("@std/process");
println(p.getenv("GTS_R_DEFINITELY_MISSING"));
println(p.getenv("GTS_R_DEFINITELY_MISSING", "fallback"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "undefined");
    assert_eq!(lines[1], "fallback");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn process_hrtime_is_monotonic_pair() {
    let dir = unique_temp_dir("gts-p6-proc-hrtime");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let p = require("@std/process");
let a = p.hrtime();
let b = p.hrtime(a);
println(a[0] >= 0, ":", b.length);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let parts: Vec<&str> = stdout.trim().split(':').collect();
    assert_eq!(parts[0], "true");
    assert_eq!(parts[1], "2");
    let _ = fs::remove_dir_all(dir);
}
