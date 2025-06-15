use swipl_rs::{PrologServer, ServerConfig, QueryResult, PrologError};

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
    
    println!("Demonstrating error handling:");
    
    // Syntax error
    println!("\n1. Testing syntax error:");
    match session.query("invalid syntax ][", None) {
        Err(PrologError::PrologException { kind, .. }) => {
            println!("   ✓ Syntax error caught: {}", kind);
        }
        _ => println!("   ✗ Unexpected result for syntax error"),
    }
    
    // Undefined predicate
    println!("\n2. Testing undefined predicate:");
    match session.query("undefined_predicate(X)", None) {
        Err(PrologError::PrologException { kind, .. }) => {
            println!("   ✓ Undefined predicate error: {}", kind);
        }
        Ok(QueryResult::Success(false)) => {
            println!("   ✓ Undefined predicate returned false (expected behavior)");
        }
        _ => println!("   ✗ Unexpected result for undefined predicate"),
    }
    
    // Type error
    println!("\n3. Testing type error:");
    match session.query("X is atom + 1", None) {
        Err(PrologError::PrologException { kind, .. }) => {
            println!("   ✓ Type error caught: {}", kind);
        }
        _ => println!("   ✗ Unexpected result for type error"),
    }
    
    // Timeout
    println!("\n4. Testing timeout (this may take a moment):");
    match session.query("sleep(2)", Some(0.1)) {
        Err(PrologError::Timeout) => {
            println!("   ✓ Timeout caught successfully");
        }
        Err(PrologError::PrologException { kind, .. }) => {
            println!("   ✓ Timeout caught as Prolog exception: {}", kind);
        }
        _ => println!("   ✗ Unexpected result for timeout"),
    }
    
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