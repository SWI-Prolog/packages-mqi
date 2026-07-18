use swipl_rs::{PrologServer, QueryResult, ServerConfig};

fn main() {
    // Check if SWI-Prolog is available
    if !swipl_available() {
        eprintln!("SWI-Prolog not found. Please install SWI-Prolog and ensure it's in your PATH.");
        std::process::exit(1);
    }

    // Create server with default configuration
    let config = ServerConfig::default();
    let mut server = PrologServer::new(config).expect("Failed to create server");
    server.start().expect("Failed to start server");

    // Connect and run queries
    let mut session = server.connect().expect("Failed to connect");

    // Simple query
    println!("Running query: append([1,2], [3,4], X)");
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

// Helper function to check if SWI-Prolog is available
fn swipl_available() -> bool {
    std::process::Command::new("swipl")
        .arg("--version")
        .output()
        .is_ok()
}
