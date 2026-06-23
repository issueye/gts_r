/// Practical example demonstrating tokio integration
///
/// This example can be run with:
/// ```bash
/// cargo run --example tokio_demo --features tokio
/// ```

#[cfg(feature = "tokio")]
use gts::async_runtime::tokio_rt::TokioRuntime;
#[cfg(feature = "tokio")]
use gts::runtime::Session;

#[cfg(feature = "tokio")]
fn main() {
    println!("=== GTS Tokio Integration Demo ===\n");

    // Demo 1: Basic tokio runtime
    demo_basic_runtime();

    // Demo 2: Concurrent tasks
    demo_concurrent_tasks();

    // Demo 3: default Session with tokio
    demo_session();

    println!("\n=== Demo Complete ===");
}

#[cfg(feature = "tokio")]
fn demo_basic_runtime() {
    println!("Demo 1: Basic Tokio Runtime");
    println!("----------------------------");

    let runtime = TokioRuntime::new();

    let result = runtime.block_on(async {
        println!("Running async code...");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        println!("Async code completed!");
        42
    });

    println!("Result: {}\n", result);
}

#[cfg(feature = "tokio")]
fn demo_concurrent_tasks() {
    println!("Demo 2: Concurrent Task Execution");
    println!("----------------------------------");

    let runtime = TokioRuntime::with_worker_threads(4);

    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            runtime.spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                println!(
                    "Task {} completed on thread {:?}",
                    i,
                    std::thread::current().id()
                );
                i * i
            })
        })
        .collect();

    let results = runtime.block_on(async {
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }
        results
    });

    let elapsed = start.elapsed();

    println!("All tasks completed in {:?}", elapsed);
    println!("Results: {:?}", results);
    println!("Expected time: ~100ms (parallel) vs 1000ms (sequential)\n");
}

#[cfg(feature = "tokio")]
fn demo_session() {
    println!("Demo 3: GTS Session with Tokio");
    println!("-------------------------------");

    let session = Session::new();

    println!("Tokio enabled: {}", cfg!(feature = "tokio"));

    let script = "const x = 10; const y = 20; console.log('Result:', x + y); x + y;";

    match session.run_source(script, "demo.gs") {
        Ok(result) => println!("Script executed successfully: {:?}", result),
        Err(err) => println!("Script error: {:?}", err),
    }
}

#[cfg(not(feature = "tokio"))]
fn main() {
    println!("This example requires the 'tokio' feature.");
    println!("Run with: cargo run --example tokio_demo --features tokio");
}
