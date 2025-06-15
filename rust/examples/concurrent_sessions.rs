use swipl_rs::{PrologServer, ServerConfig, QueryResult};
use std::thread;
use std::time::Duration;

fn main() {
    // Check if SWI-Prolog is available
    if !swipl_available() {
        eprintln!("SWI-Prolog not found. Please install SWI-Prolog and ensure it's in your PATH.");
        std::process::exit(1);
    }
    
    let config = ServerConfig::default();
    let mut server = PrologServer::new(config).expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    println!("Testing concurrent sessions:");
    
    // Create multiple sessions in different threads
    let handles: Vec<_> = (0..4).map(|i| {
        let mut session = server.connect().expect("Failed to connect");
        
        thread::spawn(move || {
            let thread_id = i;
            println!("  Thread {} started", thread_id);
            
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
                thread::sleep(Duration::from_millis(10));
            }
            
            session.close().expect("Failed to close session");
            println!("  Thread {} finished", thread_id);
        })
    }).collect();
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    
    println!("All threads completed successfully");
    server.stop(false).expect("Failed to stop server");
}

// Helper function to check if SWI-Prolog is available
fn swipl_available() -> bool {
    std::process::Command::new("swipl")
        .arg("--version")
        .output()
        .is_ok()
}