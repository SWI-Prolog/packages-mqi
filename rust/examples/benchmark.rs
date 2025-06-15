use swipl_rs::{PrologServer, ServerConfig, QueryResult};
use std::time::Instant;

fn main() {
    // Check if SWI-Prolog is available
    if !swipl_available() {
        eprintln!("SWI-Prolog not found. Please install SWI-Prolog and ensure it's in your PATH.");
        std::process::exit(1);
    }

    let config = ServerConfig::default();
    let mut server = PrologServer::new(config).expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Benchmark simple queries
    println!("Benchmarking query performance:");
    println!("{:<35} {:>20} {:>15}", "Query", "Result", "Time");
    println!("{}", "-".repeat(70));
    
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
        
        println!("{:<35} {:>20} {:>15?}", description, result_desc, duration);
    }
    
    // Benchmark async queries
    println!("\nBenchmarking async query performance:");
    let start = Instant::now();
    session.query_async("between(1, 1000, X)", false, None).expect("Failed to start");
    
    let mut count = 0;
    while session.query_async_result(Some(0.001)).expect("Failed to get result").is_some() {
        count += 1;
    }
    let duration = start.elapsed();
    println!("Retrieved {} results in {:?} ({:.2} results/sec)", 
        count, duration, count as f64 / duration.as_secs_f64());
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

// Helper function to check if SWI-Prolog is available
fn swipl_available() -> bool {
    std::process::Command::new("swipl")
        .arg("--version")
        .output()
        .is_ok()
}