use swipl_rs::{PrologServer, ServerConfig, QueryResult};

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
    
    // Build a simple knowledge base
    println!("Building knowledge base...");
    session.query("assert(parent(tom, bob))", None).expect("Failed to assert");
    session.query("assert(parent(tom, liz))", None).expect("Failed to assert");
    session.query("assert(parent(bob, ann))", None).expect("Failed to assert");
    session.query("assert(parent(bob, pat))", None).expect("Failed to assert");
    session.query("assert(parent(pat, jim))", None).expect("Failed to assert");
    
    // Define rules
    session.query("assert((grandparent(X,Y) :- parent(X,Z), parent(Z,Y)))", None)
        .expect("Failed to assert rule");
    
    // Query the knowledge base
    println!("\nQuerying grandparents:");
    match session.query("grandparent(X, Y)", None).expect("Query failed") {
        QueryResult::Solutions(solutions) => {
            for solution in solutions {
                println!("  {} is grandparent of {}", 
                    solution.get("X").map(|t| format!("{:?}", t)).unwrap_or_default(),
                    solution.get("Y").map(|t| format!("{:?}", t)).unwrap_or_default()
                );
            }
        }
        _ => println!("No grandparent relationships found"),
    }
    
    // Clean up
    session.query("retractall(parent(_, _))", None).expect("Failed to clean up");
    session.query("retractall(grandparent(_, _))", None).expect("Failed to clean up");
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