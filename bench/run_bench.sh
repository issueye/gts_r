#!/usr/bin/env bash
# Benchmark harness: serial vs concurrent @std/web server.
#
# Runs the bench server under several configurations and drives the Rust
# bench_client against /fast, /io (I/O-bound), and /cpu (CPU-bound) routes.
# Results are printed as a table and also appended to bench/results.txt.
#
# Usage: ./bench/run_bench.sh
set -u
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL="*"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

GS="./target/release/gs.exe"
CLIENT="./target/release/bench_client.exe"
SCRIPT="bench/scripts/bench_server.gs"
PORT=19000
RESULTS="bench/results.txt"

mkdir -p bench
echo "=== @std/web benchmark run @ $(date) ===" | tee "$RESULTS"

# Kill any leftover server.
taskkill //F //IM gs.exe >/dev/null 2>&1 || true
sleep 1

run_config() {
    local label="$1"
    local mode="$2"
    local workers="$3"

    echo "" | tee -a "$RESULTS"
    echo ">>> CONFIG: $label  (mode=$mode workers=$workers)" | tee -a "$RESULTS"

    BENCH_MODE="$mode" BENCH_WORKERS="$workers" "$GS" "$SCRIPT" > /tmp/bench_srv.log 2>&1 &
    local srv_pid=$!
    sleep 2

    # Quick liveness check.
    if ! curl -s --max-time 3 "http://127.0.0.1:$PORT/fast" >/dev/null 2>&1; then
        echo "  SERVER FAILED TO START; log:" | tee -a "$RESULTS"
        cat /tmp/bench_srv.log | tee -a "$RESULTS"
        taskkill //F //IM gs.exe >/dev/null 2>&1 || true
        return
    fi

    # --- /fast : baseline overhead, concurrency 16, 4000 reqs ---
    echo "  --- /fast (c=16, n=4000) ---" | tee -a "$RESULTS"
    "$CLIENT" "$PORT" "/fast" 16 4000 2>&1 | sed 's/^/    /' | tee -a "$RESULTS"

    # --- /io?ms=50 : I/O-bound 50ms sleep, concurrency 32, 320 reqs ---
    # This is the scenario where concurrency wins biggest: serial throughput
    # is capped at ~1/0.05s = 20 req/s; with N workers it approaches N*20.
    echo "  --- /io?ms=50 (c=32, n=320) ---" | tee -a "$RESULTS"
    "$CLIENT" "$PORT" "/io?ms=50" 32 320 2>&1 | sed 's/^/    /' | tee -a "$RESULTS"

    # --- /cpu?n=200000 : CPU-bound (~0.3-0.5s each in the interpreter) ---
    # Use a modest request count so even serial completes within the timeout.
    echo "  --- /cpu?n=200000 (c=16, n=160) ---" | tee -a "$RESULTS"
    "$CLIENT" "$PORT" "/cpu?n=200000" 16 160 2>&1 | sed 's/^/    /' | tee -a "$RESULTS"

    taskkill //F //IM gs.exe >/dev/null 2>&1 || true
    # Windows needs time to release the listening socket (TIME_WAIT etc.).
    sleep 4
}

# Baseline: serial mode (count-limited loop, effectively single-threaded).
run_config "serial (count loop)" serial 1

# Concurrent: increasing worker counts.
run_config "workers=1 (long-run single)" workers 1
run_config "workers=2" workers 2
run_config "workers=4" workers 4
run_config "workers=8" workers 8

echo "" | tee -a "$RESULTS"
echo "=== DONE ===" | tee -a "$RESULTS"
