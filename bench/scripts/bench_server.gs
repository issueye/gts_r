// Benchmark server for @std/web concurrency comparison.
//
// Routes:
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
