//! Parity tests for the P6 batch-2 native stdlib modules
//! (`@std/crypto`, `@std/text`, `@std/url`, `@std/cache`).
//!
//! SHA/HMAC/PBKDF2 vectors are the NIST/standard test vectors so that the
//! self-contained Rust implementations are verified against ground truth,
//! not just round-tripped.

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

// --- crypto: SHA standard test vectors -------------------------------------

#[test]
fn crypto_sha_matches_nist_vectors() {
    let dir = unique_temp_dir("gts-p6b-crypto-sha");
    fs::create_dir_all(&dir).expect("create temp dir");
    // Known NIST vectors for the 3-byte input "abc":
    //   sha1   = a9993e364706816aba3e25717850c26c9cd0d89d
    //   sha256 = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    //   sha512 = ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a
    //            2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f
    let output = run_script(
        &dir,
        r#"
let c = require("@std/crypto");
println(c.sha1("abc"));
println(c.sha256("abc"));
println(c.sha512("abc"));
println(c.sha256("").length);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "a9993e364706816aba3e25717850c26c9cd0d89d");
    assert_eq!(
        lines[1],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        lines[2],
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
    );
    // Empty input still yields a 64-char hex digest.
    assert_eq!(lines[3], "64");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn crypto_hmac_matches_rfc_4231_vectors() {
    let dir = unique_temp_dir("gts-p6b-crypto-hmac");
    fs::create_dir_all(&dir).expect("create temp dir");
    // RFC 4231 test case 2: key = "Jefe", data = "what do ya want for nothing?"
    //   HMAC-SHA-256 = 5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843
    let output = run_script(
        &dir,
        r#"
let c = require("@std/crypto");
println(c.hmac("sha256", "Jefe", "what do ya want for nothing?"));
println(c.hmac("SHA256", "Jefe", "what do ya want for nothing?"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    let expected = "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";
    assert_eq!(lines[0], expected);
    // Algorithm name is case-insensitive over the two accepted spellings.
    assert_eq!(lines[1], expected);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn crypto_pbkdf2_matches_rfc_vector() {
    let dir = unique_temp_dir("gts-p6b-crypto-pbkdf2");
    fs::create_dir_all(&dir).expect("create temp dir");
    // RFC 6070-style vector using sha256:
    //   pbkdf2("password", "salt", 1, 32, "sha256")
    //   = 120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b
    let output = run_script(
        &dir,
        r#"
let c = require("@std/crypto");
println(c.pbkdf2("password", "salt", 1, 32, "sha256"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(
        stdout_of(&output).trim(),
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn crypto_pbkdf2_rejects_nonpositive_iterations() {
    let dir = unique_temp_dir("gts-p6b-crypto-pbkdf2-err");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let c = require("@std/crypto");
println(c.pbkdf2("password", "salt", 0, 32));
"#,
    );
    assert!(!output.status.success());
    assert!(
        stderr_of(&output).contains("iterations must be positive"),
        "got stderr: {}",
        stderr_of(&output)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn crypto_random_uuid_is_v4_lowercase() {
    let dir = unique_temp_dir("gts-p6b-crypto-uuid");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let c = require("@std/crypto");
let u = c.randomUUID();
println(u.length, ":", u[14], ":", u[19], ":", u.toLowerCase() === u);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let parts: Vec<&str> = stdout.trim().split(':').collect();
    assert_eq!(parts[0], "36");
    assert_eq!(parts[1], "4");
    assert!(matches!(parts[2], "8" | "9" | "a" | "b"));
    assert_eq!(parts[3], "true");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn crypto_random_bytes_range_and_timing_safe_equal() {
    let dir = unique_temp_dir("gts-p6b-crypto-rb-tse");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let c = require("@std/crypto");
let b = c.randomBytes(4);
let ok = b.length === 4;
for (let i = 0; i < b.length; i = i + 1) {
  if (b[i] < 0 || b[i] > 255) { ok = false; }
}
println(ok);
println(c.timingSafeEqual("abc", "abc"));
println(c.timingSafeEqual("abc", "abd"));
println(c.timingSafeEqual("abc", "ab"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "true");
    assert_eq!(lines[2], "false");
    // Unequal lengths return false without error.
    assert_eq!(lines[3], "false");
    let _ = fs::remove_dir_all(dir);
}

// --- text ------------------------------------------------------------------

#[test]
fn text_width_and_strip_ansi() {
    let dir = unique_temp_dir("gts-p6b-text-width");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let t = require("@std/text");
println(t.width("abc"));
println(t.width("\x1b[31mabc\x1b[0m"));
println(t.width("你好"));
println(t.stripAnsi("\x1b[31mred\x1b[0m"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "3");
    // ANSI escapes do not contribute to display width.
    assert_eq!(lines[1], "3");
    // Each CJK ideograph is width 2.
    assert_eq!(lines[2], "4");
    assert_eq!(lines[3], "red");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn text_chars_truncate_pad_and_wrap() {
    let dir = unique_temp_dir("gts-p6b-text-ops");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let t = require("@std/text");
let cs = t.chars("ab");
println(cs.length, ":", cs[0], ":", cs[1]);
println(t.truncateWidth("abcdef", 3));
println(t.padRightWidth("ab", 5).length);
let lines = t.wrapWidth("aaabbbccc", 3);
println(lines.length, ":", lines[0], ":", lines[1], ":", lines[2]);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "2:a:b");
    assert_eq!(lines[1], "abc");
    // padRightWidth("ab", 5) -> "ab   " (length 5).
    assert_eq!(lines[2], "5");
    assert_eq!(lines[3], "3:aaa:bbb:ccc");
    let _ = fs::remove_dir_all(dir);
}

// --- url -------------------------------------------------------------------

#[test]
fn url_parse_extracts_all_components() {
    let dir = unique_temp_dir("gts-p6b-url-parse");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let u = require("@std/url");
let p = u.parse("https://example.com:8080/path/to?x=1#frag");
println(p.protocol);
println(p.host);
println(p.hostname);
println(p.port);
println(p.pathname);
println(p.search);
println(p.hash);
println(p.origin);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "https:");
    assert_eq!(lines[1], "example.com:8080");
    assert_eq!(lines[2], "example.com");
    assert_eq!(lines[3], "8080");
    assert_eq!(lines[4], "/path/to");
    assert_eq!(lines[5], "?x=1");
    assert_eq!(lines[6], "#frag");
    assert_eq!(lines[7], "https://example.com:8080");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn url_format_roundtrips_and_resolve_merges() {
    let dir = unique_temp_dir("gts-p6b-url-format");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let u = require("@std/url");
println(u.format("https://a.test/foo?x=1#h"));
println(u.resolve("https://a.test/dir/page", "next"));
println(u.resolve("https://a.test/dir/page", "/root"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "https://a.test/foo?x=1#h");
    // Relative reference merges against the base directory.
    assert_eq!(lines[1], "https://a.test/dir/next");
    // Root-relative reference replaces the path.
    assert_eq!(lines[2], "https://a.test/root");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn url_origin_is_null_for_relative_urls() {
    let dir = unique_temp_dir("gts-p6b-url-origin");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let u = require("@std/url");
let p = u.parse("/local/path");
println(p.origin);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    assert_eq!(stdout_of(&output).trim(), "null");
    let _ = fs::remove_dir_all(dir);
}

// --- cache -----------------------------------------------------------------

#[test]
fn cache_basic_set_get_has_delete() {
    let dir = unique_temp_dir("gts-p6b-cache-basic");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let cache = require("@std/cache");
let c = cache.create();
c.set("a", 1);
c.set("b", "two");
println(c.get("a"));
println(c.get("b"));
println(c.has("a"));
c.delete("a");
println(c.has("a"));
println(c.size());
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "two");
    assert_eq!(lines[2], "true");
    assert_eq!(lines[3], "false");
    assert_eq!(lines[4], "1");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cache_keys_and_clear() {
    let dir = unique_temp_dir("gts-p6b-cache-clear");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let cache = require("@std/cache");
let c = cache.create();
c.set("x", 1);
c.set("y", 2);
let keys = c.keys();
println(keys.length);
c.clear();
println(c.size());
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "2");
    assert_eq!(lines[1], "0");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cache_ttl_expires_entries() {
    let dir = unique_temp_dir("gts-p6b-cache-ttl");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let cache = require("@std/cache");
let c = cache.create();
c.set("ephemeral", "gone", 1);
println(c.get("ephemeral"));
let time = require("@std/time");
time.sleep(20);
println(c.get("ephemeral"));
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    // Before expiry the value is present.
    assert_eq!(lines[0], "gone");
    // After the TTL elapses, get lazily deletes and returns undefined.
    assert_eq!(lines[1], "undefined");
    let _ = fs::remove_dir_all(dir);
}

// --- env supplements (load/parse/getJson/require-array) --------------------

#[test]
fn env_parse_handles_quotes_comments_and_expansion() {
    let dir = unique_temp_dir("gts-p6b-env-parse");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let env = require("@std/env");
let parsed = env.parse("NAME=Ada\n# comment\nROLE=\"admin\"\nPORT='8080'\nGREETING=${NAME}-admin");
println(parsed.NAME);
println(parsed.ROLE);
println(parsed.PORT);
println(parsed.GREETING);
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "Ada");
    assert_eq!(lines[1], "admin");
    assert_eq!(lines[2], "8080");
    assert_eq!(lines[3], "Ada-admin");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn env_get_json_returns_raw_string_or_undefined() {
    let dir = unique_temp_dir("gts-p6b-env-getjson");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let env = require("@std/env");
env.set("GTS_R_JSON_TEST", "{\"a\":1}");
println(env.getJson("GTS_R_JSON_TEST"));
println(env.getJson("GTS_R_DEFINITELY_MISSING"));
env.unset("GTS_R_JSON_TEST");
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "{\"a\":1}");
    assert_eq!(lines[1], "undefined");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn env_require_array_reports_missing() {
    let dir = unique_temp_dir("gts-p6b-env-require");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let env = require("@std/env");
env.set("GTS_R_HAVE_THIS", "1");
// require throws when keys are missing; the script exits non-zero and the
// error message lists the missing keys.
env.require(["GTS_R_HAVE_THIS", "GTS_R_MISSING_ONE", "GTS_R_MISSING_TWO"]);
"#,
    );
    assert!(!output.status.success());
    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("GTS_R_MISSING_ONE") && stderr.contains("GTS_R_MISSING_TWO"),
        "expected both missing keys in stderr, got: {}",
        stderr
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn env_load_reads_dotenv_into_process_env() {
    let dir = unique_temp_dir("gts-p6b-env-load");
    fs::create_dir_all(&dir).expect("create temp dir");
    // Set DIR via the process environment so the script can read it through
    // @std/env, then load `<DIR>/.env` into the process environment.
    let dotenv_path = dir.join(".env");
    fs::write(
        &dotenv_path,
        "GTS_R_LOADED_KEY=loaded-value\nGTS_R_LOADED_NUM=42\n",
    )
    .expect("write .env");
    let dotenv_str = dotenv_path.to_string_lossy().replace('\\', "\\\\");
    let script = format!(
        r#"
let env = require("@std/env");
env.load("{dotenv_str}");
println(env.get("GTS_R_LOADED_KEY"));
println(env.getInt("GTS_R_LOADED_NUM"));
env.unset("GTS_R_LOADED_KEY");
env.unset("GTS_R_LOADED_NUM");
"#
    );
    let file = dir.join("main.gs");
    fs::write(&file, script).expect("write script");
    let output = Command::new(env!("CARGO_BIN_EXE_gs"))
        .arg(&file)
        .output()
        .expect("run gs");
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "loaded-value");
    assert_eq!(lines[1], "42");
    let _ = fs::remove_dir_all(dir);
}

// --- timers ----------------------------------------------------------------

#[test]
fn timers_sleep_blocks_and_set_timeout_forwards() {
    let dir = unique_temp_dir("gts-p6b-timers");
    fs::create_dir_all(&dir).expect("create temp dir");
    let output = run_script(
        &dir,
        r#"
let timers = require("@std/timers");
let time = require("@std/time");
let before = time.nowMs();
timers.sleep(15);
let after = time.nowMs();
println(after >= before);
let id = timers.setTimeout(function() {}, 0);
println(id >= 0);
timers.clearTimeout(99);
println("clear-ok");
"#,
    );
    assert!(output.status.success(), "stderr: {}", stderr_of(&output));
    let stdout = stdout_of(&output);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "true");
    assert_eq!(lines[1], "true");
    assert_eq!(lines[2], "clear-ok");
    let _ = fs::remove_dir_all(dir);
}
