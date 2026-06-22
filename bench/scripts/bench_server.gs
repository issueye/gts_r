// Benchmark workloads for bytecode VM performance checks and @std/web
// concurrency comparison.
//
// Bytecode performance mode is selected with:
//   GTS_BYTECODE_BENCH = "fib" | "string_concat" | "promise_create"
//   GTS_BYTECODE_BENCH_ITERS = iteration count (optional)
//
// Otherwise this script starts the web benchmark server with routes:
//   GET /io?ms=N    — I/O-bound: sleep N ms (default 50), return "io ok"
//   GET /cpu?n=N    — CPU-bound: sum 1..N (default 1000000), return result
//   GET /fast       — baseline: instant response (framework overhead)
//
// Mode is selected via env vars (set by the bench harness):
//   BENCH_MODE     = "serial" | "workers"   (default: workers)
//   BENCH_WORKERS  = worker count           (default: 4)
let web = require("@std/web");
let timers = require("@std/timers");
let env = require("@std/env");

function fib(n) {
    let a = 0;
    let b = 1;
    let i = 0;
    while (i < n) {
        let next = a + b;
        a = b;
        b = next;
        i = i + 1;
    }
    return a;
}

function benchFib(iterations) {
    let total = 0;
    let i = 0;
    while (i < iterations) {
        total = total + fib(26);
        i = i + 1;
    }
    return total;
}

function benchStringConcat(iterations) {
    let total = 0;
    let i = 0;
    while (i < iterations) {
        let s = "";
        let j = 0;
        while (j < 24) {
            s = s + "gts";
            j = j + 1;
        }
        total = total + s.length;
        i = i + 1;
    }
    return total;
}

function benchPromiseCreate(iterations) {
    let total = 0;
    let i = 0;
    while (i < iterations) {
        let promise = Promise.resolve(i);
        if (promise !== null) {
            total = total + 1;
        }
        i = i + 1;
    }
    return total;
}

let bytecodeBench = env.get("GTS_BYTECODE_BENCH", "");
if (bytecodeBench !== "") {
    let iterations = env.getInt("GTS_BYTECODE_BENCH_ITERS", 10000);
    let result = 0;
    if (bytecodeBench === "fib") {
        result = benchFib(iterations);
    } else if (bytecodeBench === "string_concat") {
        result = benchStringConcat(iterations);
    } else if (bytecodeBench === "promise_create") {
        result = benchPromiseCreate(iterations);
    } else {
        throw new Error(`unknown bytecode benchmark: ${bytecodeBench}`);
    }
    println(`bytecode-bench:${bytecodeBench}:${iterations}:${result}`);
} else {
    let app = web.createApp();

    app.get("/io", function(ctx) {
        let ms = 50;
        let q = ctx.req.query;
        if (typeof q.ms !== "undefined") {
            ms = parseInt(q.ms);
        }
        timers.sleep(ms);
        ctx.res.send("io ok");
    });

    app.get("/cpu", function(ctx) {
        let n = 1000000;
        let q = ctx.req.query;
        if (typeof q.n !== "undefined") {
            n = parseInt(q.n);
        }
        let sum = 0;
        let i = 1;
        while (i <= n) {
            sum = sum + i;
            i = i + 1;
        }
        ctx.res.send(`cpu ${sum}`);
    });

    app.get("/fast", function(ctx) {
        ctx.res.send("fast ok");
    });

    let port = 19000;
    println(`GTS_PORT=${port}`);

    let mode = "workers";
    let m = env.get("BENCH_MODE", "");
    if (m !== "") {
        mode = m;
    }

    let workers = 4;
    let w = env.get("BENCH_WORKERS", "");
    if (w !== "") {
        workers = parseInt(w);
    }

    if (mode === "serial") {
        app.listen(port, { count: 100000000 });
    } else {
        app.listen(port, { workers: workers });
    }
}
