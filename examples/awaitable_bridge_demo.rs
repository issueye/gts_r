#[cfg(feature = "tokio")]
use gts::async_runtime::tokio_rt::tcp;
/// Example: Using GTS with Tokio via Awaitable Bridge
///
/// This example demonstrates the recommended patterns for integrating
/// GTS's single-threaded runtime with tokio's multi-threaded runtime.

#[cfg(feature = "tokio")]
use gts::async_runtime::{spawn_blocking_gts, AsyncCoordinator, SerializedResult};
#[cfg(feature = "tokio")]
use gts::runtime::Session;

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() {
    println!("=== GTS + Tokio Awaitable Bridge Demo ===\n");

    // Demo 1: Running GTS scripts on tokio's blocking pool
    demo_blocking_gts().await;

    // Demo 2: Using tokio I/O with GTS
    demo_tokio_io().await;

    // Demo 3: Coordinating async operations
    demo_async_coordination().await;

    println!("\n=== Demo Complete ===");
}

/// Demo 1: Run GTS scripts on tokio's blocking thread pool
#[cfg(feature = "tokio")]
async fn demo_blocking_gts() {
    println!("Demo 1: GTS on Tokio Blocking Pool");
    println!("-----------------------------------");

    // Use SerializedResult to transfer across threads
    let result = spawn_blocking_gts(|| {
        let session = Session::new();
        let result = session.run_source(
            "const x = 10; const y = 20; console.log('Sum:', x + y); x + y;",
            "demo1.gs",
        );

        // Serialize the result for thread transfer
        match result {
            Ok(obj) => format!("Success: {}", obj.inspect()),
            Err(err) => format!("Error: {}", err.inspect()),
        }
    })
    .await;

    match result {
        Ok(msg) => println!("Result: {}", msg),
        Err(e) => println!("Task error: {:?}", e),
    }

    println!();
}

/// Demo 2: Use tokio for I/O, GTS for processing
#[cfg(feature = "tokio")]
async fn demo_tokio_io() {
    println!("Demo 2: Tokio I/O with GTS Processing");
    println!("--------------------------------------");

    // Simulate async I/O operation
    let data = tokio::time::timeout(tokio::time::Duration::from_millis(100), async {
        // Simulate network delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        "Hello from tokio!".to_string()
    })
    .await;

    match data {
        Ok(content) => {
            println!("Received data: {}", content);

            // Process with GTS on blocking pool - serialize result
            let result = spawn_blocking_gts(move || {
                let session = Session::new();
                let script = format!(
                    r#"
                        const data = "{}";
                        const processed = data.toUpperCase();
                        console.log("Processed:", processed);
                        processed;
                    "#,
                    content
                );
                match session.run_source(&script, "demo2.gs") {
                    Ok(obj) => obj.inspect(),
                    Err(err) => format!("Error: {}", err.inspect()),
                }
            })
            .await;

            println!("Processing result: {:?}", result);
        }
        Err(_) => println!("Timeout!"),
    }

    println!();
}

/// Demo 3: Coordinate multiple async operations
#[cfg(feature = "tokio")]
async fn demo_async_coordination() {
    println!("Demo 3: Async Coordination");
    println!("--------------------------");

    let coordinator = AsyncCoordinator::new();

    // Spawn multiple async tasks that add work
    let coord1 = coordinator.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        coord1.add_pending("Task 1 completed".to_string()).await;
    });

    let coord2 = coordinator.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        coord2.add_pending("Task 2 completed".to_string()).await;
    });

    let coord3 = coordinator.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
        coord3.add_pending("Task 3 completed".to_string()).await;
    });

    // Wait for all tasks to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Get all pending results
    let results = coordinator.get_pending().await;
    println!("Collected {} results:", results.len());
    for (i, result) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, result);
    }

    println!();
}

#[cfg(not(feature = "tokio"))]
fn main() {
    println!("This example requires the 'tokio' feature.");
    println!("Run with: cargo run --example awaitable_bridge_demo --features tokio");
}
