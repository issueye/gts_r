//! Benchmark client for the @std/web concurrent server.
//!
//! Sends N concurrent HTTP requests across C client threads, measures per-
//! request latency, and reports throughput (RPS) + latency percentiles.
//!
//! Usage:
//!   bench_client <port> <path> <concurrency> <total_requests>
//!
//! Example:
//!   bench_client 19000 /fast 16 5000
//!
//! Run via: cargo run --release --bin bench_client -- 19000 /fast 16 5000

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!(
            "usage: {} <port> <path> <concurrency> <total_requests>",
            args[0]
        );
        eprintln!("example: {} 19000 /fast 16 5000", args[0]);
        std::process::exit(2);
    }
    let port: u16 = args[1].parse().expect("port");
    let path = args[2].as_str();
    let concurrency: usize = args[3].parse().expect("concurrency");
    let total: usize = args[4].parse().expect("total_requests");

    let addr = format!("127.0.0.1:{}", port);
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        path
    );
    let request = Arc::new(request);

    // Warm up: one request to ensure the server (and its workers) are hot.
    let _ = do_request(&addr, &request);

    let counter = Arc::new(AtomicUsize::new(0));
    let latencies = Arc::new(std::sync::Mutex::new(Vec::<u64>::with_capacity(total)));
    let errors = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(concurrency);
    let start = Instant::now();

    for _ in 0..concurrency {
        let addr = addr.clone();
        let request = Arc::clone(&request);
        let counter = Arc::clone(&counter);
        let latencies = Arc::clone(&latencies);
        let errors = Arc::clone(&errors);
        handles.push(thread::spawn(move || loop {
            let i = counter.fetch_add(1, Ordering::Relaxed);
            if i >= total {
                break;
            }
            let t0 = Instant::now();
            match do_request(&addr, &request) {
                Ok(()) => {
                    let us = t0.elapsed().as_micros() as u64;
                    latencies.lock().unwrap().push(us);
                }
                Err(_) => {
                    errors.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let elapsed = start.elapsed();

    // ---- Report ----
    let mut lat = latencies.lock().unwrap();
    lat.sort_unstable();
    let n = lat.len();
    let err = errors.load(Ordering::Relaxed);
    let done = n as u64;
    let secs = elapsed.as_secs_f64().max(0.000001);
    let rps = done as f64 / secs;
    let avg_us = if n > 0 {
        lat.iter().sum::<u64>() as f64 / n as f64
    } else {
        0.0
    };
    let pct = |p: f64| -> u64 {
        if n == 0 {
            return 0;
        }
        let idx = ((n as f64 - 1.0) * p).round() as usize;
        lat[idx.min(n - 1)]
    };

    println!(
        "path={}  concurrency={}  requests={}",
        path, concurrency, total
    );
    println!("  successful : {}", done);
    println!("  errors     : {}", err);
    println!("  elapsed    : {:.3} s", secs);
    println!("  throughput : {:.0} req/s", rps);
    println!(
        "  latency(us): avg={:.0}  p50={}  p95={}  p99={}  max={}",
        avg_us,
        pct(0.50),
        pct(0.95),
        pct(0.99),
        lat.last().copied().unwrap_or(0)
    );
}

fn do_request(addr: &str, request: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(request.as_bytes())?;
    // Drain the response (read until EOF / Connection: close).
    let mut buf = [0u8; 1024];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
