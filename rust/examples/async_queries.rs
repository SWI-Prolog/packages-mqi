use swipl_rs::{PrologServer, QueryResult, ServerConfig};

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

    // Start async query
    println!("Starting async query: between(1, 5, X)");
    session
        .query_async("between(1, 5, X)", false, None)
        .expect("Failed to start async query");

    // Retrieve results one by one
    println!("Retrieving results:");
    let mut count = 0;
    while let Some(result) = session
        .query_async_result(Some(1.0))
        .expect("Failed to get result")
    {
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

// Helper function to check if SWI-Prolog is available
fn swipl_available() -> bool {
    std::process::Command::new("swipl")
        .arg("--version")
        .output()
        .is_ok()
}
