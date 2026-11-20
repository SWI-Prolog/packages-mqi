mod test_helpers;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use swipl_rs::{PrologError, QueryResult};
use test_helpers::*;

#[test]
#[ignore] // Run with: cargo test --test examples -- --ignored
fn example_basic_usage() {
    require_swipl();

    // Create server with dynamic port
    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");

    // Connect and run queries
    let mut session = server.connect().expect("Failed to connect");

    // Simple query
    match session
        .query("append([1,2], [3,4], X)", None)
        .expect("Query failed")
    {
        QueryResult::Solutions(solutions) => {
            println!("Found {} solution(s)", solutions.len());
            for (i, solution) in solutions.iter().enumerate() {
                println!("Solution {}: {:?}", i + 1, solution);
            }
        }
        _ => println!("No solutions found"),
    }

    // Clean up
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
#[ignore] // Run with: cargo test --test examples -- --ignored
fn example_async_queries() {
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");

    let mut session = server.connect().expect("Failed to connect");

    // Start async query
    println!("Starting async query...");
    session
        .query_async("between(1, 5, X)", false, None)
        .expect("Failed to start async query");

    // Retrieve results one by one with timeout check
    println!("Retrieving results:");
    let mut count = 0;
    let timeout = TestTimeout::new(Duration::from_secs(5));

    while let Some(result) = session
        .query_async_result(Some(1.0))
        .expect("Failed to get result")
    {
        timeout.check().expect("Test timed out");
        match result {
            QueryResult::Solutions(solutions) => {
                count += 1;
                println!("Result {}: {:?}", count, solutions[0]);
            }
            _ => break,
        }
    }
    println!("Total results: {}", count);

    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
#[ignore] // Run with: cargo test --test examples -- --ignored
fn example_knowledge_base() {
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");

    let mut session = server.connect().expect("Failed to connect");

    // Build a simple knowledge base
    println!("Building knowledge base...");
    session
        .query("assert(parent(tom, bob))", None)
        .expect("Failed to assert");
    session
        .query("assert(parent(tom, liz))", None)
        .expect("Failed to assert");
    session
        .query("assert(parent(bob, ann))", None)
        .expect("Failed to assert");
    session
        .query("assert(parent(bob, pat))", None)
        .expect("Failed to assert");
    session
        .query("assert(parent(pat, jim))", None)
        .expect("Failed to assert");

    // Define rules
    session
        .query(
            "assert((grandparent(X,Y) :- parent(X,Z), parent(Z,Y)))",
            None,
        )
        .expect("Failed to assert rule");

    // Query the knowledge base
    println!("\nQuerying grandparents:");
    match session
        .query("grandparent(X, Y)", None)
        .expect("Query failed")
    {
        QueryResult::Solutions(solutions) => {
            for solution in solutions {
                println!(
                    "  {} is grandparent of {}",
                    solution
                        .get("X")
                        .map(|t| format!("{:?}", t))
                        .unwrap_or_default(),
                    solution
                        .get("Y")
                        .map(|t| format!("{:?}", t))
                        .unwrap_or_default()
                );
            }
        }
        _ => println!("No grandparent relationships found"),
    }

    // Clean up
    session
        .query("retractall(parent(_, _))", None)
        .expect("Failed to clean up");
    session
        .query("retractall(grandparent(_, _))", None)
        .expect("Failed to clean up");
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
#[ignore] // Run with: cargo test --test examples -- --ignored
fn benchmark_query_performance() {
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");

    let mut session = server.connect().expect("Failed to connect");

    // Benchmark simple queries
    println!("Benchmarking query performance:");

    let queries = vec![
        ("true", "Simple success"),
        ("false", "Simple failure"),
        ("X = 42", "Single unification"),
        ("member(X, [1,2,3,4,5])", "List membership"),
        ("append([1,2,3], [4,5,6], X)", "List append"),
        ("between(1, 100, X)", "Generate 100 numbers"),
    ];

    for (query, description) in queries {
        let start = Instant::now();
        let result = session.query(query, None);
        let duration = start.elapsed();

        let result_desc = match result {
            Ok(QueryResult::Success(b)) => format!("Success({})", b),
            Ok(QueryResult::Solutions(ref s)) => format!("{} solutions", s.len()),
            Err(ref e) => format!("Error: {}", e),
        };

        println!(
            "  {:<30} {:>15} in {:?}",
            description, result_desc, duration
        );
    }

    // Benchmark async queries
    println!("\nBenchmarking async query performance:");
    let start = Instant::now();
    session
        .query_async("between(1, 1000, X)", false, None)
        .expect("Failed to start");

    let mut count = 0;
    let timeout = TestTimeout::new(Duration::from_secs(10));

    while session
        .query_async_result(Some(0.001))
        .expect("Failed to get result")
        .is_some()
    {
        count += 1;
        if timeout.check().is_err() {
            println!("  Timeout reached after {} results", count);
            break;
        }
    }
    let duration = start.elapsed();
    println!(
        "  Retrieved {} results in {:?} ({:.2} results/sec)",
        count,
        duration,
        count as f64 / duration.as_secs_f64()
    );

    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
#[ignore] // Run with: cargo test --test examples -- --ignored
fn example_error_handling() {
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");

    let mut session = server.connect().expect("Failed to connect");

    println!("Demonstrating error handling:");

    // Syntax error
    match session.query("invalid syntax ][", None) {
        Err(PrologError::PrologException { kind, .. }) => {
            println!("  Syntax error caught: {}", kind);
        }
        _ => println!("  Unexpected result for syntax error"),
    }

    // Undefined predicate
    match session.query("undefined_predicate(X)", None) {
        Err(PrologError::PrologException { kind, .. }) => {
            println!("  Undefined predicate error: {}", kind);
        }
        Ok(QueryResult::Success(false)) => {
            println!("  Undefined predicate returned false (expected behavior)");
        }
        _ => println!("  Unexpected result for undefined predicate"),
    }

    // Type error
    match session.query("X is atom + 1", None) {
        Err(PrologError::PrologException { kind, .. }) => {
            println!("  Type error caught: {}", kind);
        }
        _ => println!("  Unexpected result for type error"),
    }

    // Timeout
    match session.query("sleep(2)", Some(0.1)) {
        Err(PrologError::Timeout) => {
            println!("  Timeout caught successfully");
        }
        Err(PrologError::PrologException { kind, .. }) => {
            println!("  Timeout caught as Prolog exception: {}", kind);
        }
        _ => println!("  Unexpected result for timeout"),
    }

    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
#[ignore] // Run with: cargo test --test examples -- --ignored
fn example_concurrent_sessions() {
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");

    println!("Testing concurrent sessions:");

    // Use synchronization to coordinate threads
    let barrier = Arc::new(std::sync::Barrier::new(4));

    // Create multiple sessions in different threads
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let mut session = server.connect().expect("Failed to connect");
            let barrier_clone = barrier.clone();

            thread::spawn(move || {
                let thread_id = i;
                println!("  Thread {} started", thread_id);

                // Wait for all threads to be ready
                barrier_clone.wait();

                // Each thread runs different queries
                for j in 0..5 {
                    let query = format!("X is {} * {} + {}", thread_id, j, j);
                    match session.query(&query, None) {
                        Ok(QueryResult::Solutions(solutions)) => {
                            if let Some(x_val) = solutions[0].get("X") {
                                println!("    Thread {} query {}: X = {:?}", thread_id, j, x_val);
                            }
                        }
                        _ => println!("    Thread {} query {} failed", thread_id, j),
                    }
                }

                session.close().expect("Failed to close session");
                println!("  Thread {} finished", thread_id);
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    println!("All threads completed successfully");
    server.stop(false).expect("Failed to stop server");
}
