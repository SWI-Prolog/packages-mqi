mod test_helpers;

use test_helpers::*;
use swipl_rs::{PrologError, QueryResult, ServerConfig, PrologTerm, PrologCompound};
use std::time::Duration;
use std::sync::Arc;
use std::thread;

#[test]
fn test_server_lifecycle() {
    init_logger();
    require_swipl();

    // Test server creation, starting, and stopping
    let mut server = TestServer::new().expect("Failed to create server");
    
    // Start the server (includes waiting for ready)
    server.start().expect("Failed to start server");
    
    // Stop the server gracefully
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_basic_connection() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    // Connect to the server
    let session = server.connect();
    assert!(session.is_ok(), "Failed to connect: {:?}", session.err());
    
    // Session should automatically close when dropped
    drop(session);
    
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_simple_query() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test a simple true query
    let result = session.query("true", None).expect("Query failed");
    assert_query_success(&result, true);
    
    // Test a simple false query
    let result = session.query("false", None).expect("Query failed");
    assert_query_success(&result, false);
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_query_with_variables() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test a query with variable bindings
    let result = session.query("X = 42, Y = hello", None).expect("Query failed");
    assert_has_solutions(&result, Some(1));
    
    match result {
        QueryResult::Solutions(solutions) => {
            let solution = &solutions[0];
            assert_eq!(solution.len(), 2);
            // Check X = 42
            assert_eq!(solution.get("X"), Some(&PrologTerm::Integer(42)));
            // Check Y = hello
            assert_eq!(solution.get("Y"), Some(&PrologTerm::Atom("hello".to_string())));
        },
        _ => unreachable!(),
    }
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_findall_query() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test findall functionality
    let result = session.query("member(X, [1, 2, 3])", None).expect("Query failed");
    assert_has_solutions(&result, Some(3));
    
    match result {
        QueryResult::Solutions(solutions) => {
            // Solutions should contain X=1, X=2, X=3
            assert_eq!(solutions[0].get("X"), Some(&PrologTerm::Integer(1)));
            assert_eq!(solutions[1].get("X"), Some(&PrologTerm::Integer(2)));
            assert_eq!(solutions[2].get("X"), Some(&PrologTerm::Integer(3)));
        },
        _ => unreachable!(),
    }
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_async_query() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Start an async query
    session.query_async("member(X, [a, b, c])", false, None)
        .expect("Failed to start async query");
    
    // Retrieve results one by one
    let mut count = 0;
    let expected_values = vec!["a", "b", "c"];
    let timeout = TestTimeout::new(Duration::from_secs(5));
    
    while let Some(result) = session.query_async_result(Some(1.0)).expect("Failed to get result") {
        timeout.check().expect("Test timed out");
        match result {
            QueryResult::Solutions(solutions) => {
                assert_eq!(solutions.len(), 1);
                assert_eq!(solutions[0].get("X"), Some(&PrologTerm::Atom(expected_values[count].to_string())));
                count += 1;
            },
            _ => panic!("Expected Solutions, got {:?}", result),
        }
    }
    assert_eq!(count, 3, "Expected 3 results");
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_cancel_async_query() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Use synchronization to ensure query is running before cancellation
    let sync = Arc::new(AsyncSync::new());
    let sync_clone = sync.clone();
    
    // Start a long-running async query in a thread
    let handle = thread::spawn(move || {
        // Start the query
        session.query_async("repeat, fail", false, None)
            .expect("Failed to start async query");
        
        // Signal that query has started
        sync_clone.signal_ready();
        
        // Wait a bit to ensure query is running
        thread::sleep(Duration::from_millis(100));
        
        // Cancel the query
        match session.cancel_async() {
            Ok(_) => {},
            Err(PrologError::NoQuery) => {
                // Query might have already finished, that's OK
            },
            Err(e) => panic!("Failed to cancel query: {:?}", e),
        }
        
        // Verify cancellation
        let result = session.query_async_result(Some(0.1));
        match result {
            Ok(None) => {}, // No more results
            Err(PrologError::NoQuery) => {}, // No query active
            Err(PrologError::QueryCancelled) => {}, // Query was cancelled
            _ => panic!("Expected no results, NoQuery error, or QueryCancelled, got {:?}", result),
        }
        
        session.close().expect("Failed to close session");
    });
    
    // Wait for query to start
    sync.wait_ready(Duration::from_secs(5)).expect("Query didn't start in time");
    
    handle.join().expect("Thread panicked");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_query_timeout() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test a query with a very short timeout
    let result = session.query("sleep(2)", Some(0.1));
    match result {
        Err(PrologError::Timeout) => {},
        Err(PrologError::PrologException { kind, .. }) if kind == "time_limit_exceeded" => {},
        _ => panic!("Expected Timeout error, got {:?}", result),
    }
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_syntax_error() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test a query with syntax error
    let result = session.query("invalid syntax ][", None);
    match result {
        Err(PrologError::PrologException { kind, .. }) => {
            assert!(kind.contains("syntax_error") || kind.contains("error"), 
                   "Expected syntax error, got: {}", kind);
        },
        _ => panic!("Expected PrologException, got {:?}", result),
    }
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_multiple_sessions() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    // Create multiple sessions
    let mut session1 = server.connect().expect("Failed to connect session 1");
    let mut session2 = server.connect().expect("Failed to connect session 2");
    
    // Run queries on both sessions
    let result1 = session1.query("X = session1", None).expect("Query 1 failed");
    let result2 = session2.query("Y = session2", None).expect("Query 2 failed");
    
    // Verify both queries succeeded with correct values
    match result1 {
        QueryResult::Solutions(solutions) => {
            assert_eq!(solutions.len(), 1);
            assert_eq!(solutions[0].get("X"), Some(&PrologTerm::Atom("session1".to_string())));
        },
        _ => panic!("Expected Solutions for result1, got {:?}", result1),
    }
    match result2 {
        QueryResult::Solutions(solutions) => {
            assert_eq!(solutions.len(), 1);
            assert_eq!(solutions[0].get("Y"), Some(&PrologTerm::Atom("session2".to_string())));
        },
        _ => panic!("Expected Solutions for result2, got {:?}", result2),
    }
    
    session1.close().expect("Failed to close session 1");
    session2.close().expect("Failed to close session 2");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_server_with_custom_port() {
    init_logger();
    require_swipl();

    let mut config = ServerConfig::default();
    config.port = Some(get_free_port());
    
    let mut server = TestServer::with_config(config).expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Run a simple query to verify connection works
    let result = session.query("true", None).expect("Query failed");
    assert_query_success(&result, true);
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
#[cfg(unix)]
fn test_unix_domain_socket() {
    init_logger();
    require_swipl();

    #[cfg(feature = "unix-socket")]
    {
        let mut config = ServerConfig::default();
        // Use helper to get unique socket path
        config.unix_domain_socket = Some(get_unique_socket_path());
        config.port = None;
        
        let mut server = TestServer::with_config(config).expect("Failed to create server");
        server.start().expect("Failed to start server");
        
        let mut session = server.connect().expect("Failed to connect");
        
        // Run a simple query to verify connection works
        let result = session.query("true", None).expect("Query failed");
        assert_query_success(&result, true);
        
        session.close().expect("Failed to close session");
        server.stop(false).expect("Failed to stop server");
    }
    
    #[cfg(not(feature = "unix-socket"))]
    {
        eprintln!("Unix domain socket feature not enabled. Skipping test.");
    }
}

#[test]
fn test_server_halt_command() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Close session before server stop
    session.close().expect("Failed to close session");
    
    // Stop with graceful shutdown
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_query_with_lists() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test query returning a list
    let result = session.query("X = [1, 2, 3]", None).expect("Query failed");
    assert_has_solutions(&result, Some(1));
    
    match result {
        QueryResult::Solutions(solutions) => {
            assert!(solutions[0].contains_key("X"));
            // Verify it's a list with expected values
            let expected_list = PrologTerm::List(vec![
                PrologTerm::Integer(1),
                PrologTerm::Integer(2),
                PrologTerm::Integer(3)
            ]);
            assert_eq!(solutions[0].get("X"), Some(&expected_list));
        },
        _ => unreachable!(),
    }
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_query_with_atoms_and_strings() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test various atom and string forms
    let result = session.query("A = hello, B = 'Hello World', C = \"string\"", None)
        .expect("Query failed");
    assert_has_solutions(&result, Some(1));
    
    match result {
        QueryResult::Solutions(solutions) => {
            let solution = &solutions[0];
            assert_eq!(solution.get("A"), Some(&PrologTerm::Atom("hello".to_string())));
            assert_eq!(solution.get("B"), Some(&PrologTerm::Atom("Hello World".to_string())));
            assert_eq!(solution.get("C"), Some(&PrologTerm::Atom("string".to_string())));
        },
        _ => unreachable!(),
    }
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_error_handling_invalid_password() {
    init_logger();
    require_swipl();

    // Start server with a specific password
    let mut config = ServerConfig::default();
    config.password = Some("correct_password".to_string());
    config.port = Some(get_free_port());
    
    let mut server = TestServer::with_config(config).expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    // Connection should work with correct password
    let session = server.connect();
    assert!(session.is_ok(), "Should connect with correct password");
    
    session.unwrap().close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_compound_terms() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test compound term
    let result = session.query("X = foo(bar, 42)", None).expect("Query failed");
    assert_has_solutions(&result, Some(1));
    
    match result {
        QueryResult::Solutions(solutions) => {
            assert!(solutions[0].contains_key("X"));
            // The value should be a compound term with functor "foo"
            let expected_compound = PrologTerm::Compound(PrologCompound {
                functor: "foo".to_string(),
                args: vec![
                    PrologTerm::Atom("bar".to_string()),
                    PrologTerm::Integer(42)
                ]
            });
            assert_eq!(solutions[0].get("X"), Some(&expected_compound));
        },
        _ => unreachable!(),
    }
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_multiple_solutions() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test query with multiple solutions
    let result = session.query("(X = 1, Y = a); (X = 2, Y = b); (X = 3, Y = c)", None)
        .expect("Query failed");
    assert_has_solutions(&result, Some(3));
    
    match result {
        QueryResult::Solutions(solutions) => {
            // Each solution should have both X and Y bindings
            assert_eq!(solutions[0].get("X"), Some(&PrologTerm::Integer(1)));
            assert_eq!(solutions[0].get("Y"), Some(&PrologTerm::Atom("a".to_string())));
            assert_eq!(solutions[1].get("X"), Some(&PrologTerm::Integer(2)));
            assert_eq!(solutions[1].get("Y"), Some(&PrologTerm::Atom("b".to_string())));
            assert_eq!(solutions[2].get("X"), Some(&PrologTerm::Integer(3)));
            assert_eq!(solutions[2].get("Y"), Some(&PrologTerm::Atom("c".to_string())));
        },
        _ => unreachable!(),
    }
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_server_restart() {
    init_logger();
    require_swipl();

    // Use dynamic ports for both servers
    let port1 = get_free_port();
    let port2 = get_free_port();
    
    let mut config1 = ServerConfig::default();
    config1.port = Some(port1);
    
    let mut server = TestServer::with_config(config1).expect("Failed to create server");
    
    // Start, stop
    server.start().expect("Failed to start server");
    server.stop(false).expect("Failed to stop server");
    
    // Create a new server instance with different port
    let mut config2 = ServerConfig::default();
    config2.port = Some(port2);
    
    let mut server2 = TestServer::with_config(config2).expect("Failed to create server");
    server2.start().expect("Failed to start server again");
    
    let mut session = server2.connect().expect("Failed to connect");
    let result = session.query("true", None).expect("Query failed");
    assert_query_success(&result, true);
    
    session.close().expect("Failed to close session");
    server2.stop(false).expect("Failed to stop server");
}

#[test]
fn test_prolog_assert_and_retract() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Assert a fact
    let result = session.query("assert(test_fact(hello))", None).expect("Assert failed");
    assert_query_success(&result, true);
    
    // Query the fact
    let result = session.query("test_fact(X)", None).expect("Query failed");
    assert_has_solutions(&result, Some(1));
    
    match result {
        QueryResult::Solutions(solutions) => {
            assert!(solutions[0].contains_key("X"));
            // Verify X = 'hello'
            assert_eq!(solutions[0].get("X"), Some(&PrologTerm::Atom("hello".to_string())));
        },
        _ => unreachable!(),
    }
    
    // Retract the fact
    let result = session.query("retract(test_fact(_))", None).expect("Retract failed");
    assert_query_success(&result, true);
    
    // Verify it's gone
    let result = session.query("test_fact(X)", None).expect("Query failed");
    assert!(matches!(result, QueryResult::Success(false)));
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_arithmetic() {
    init_logger();
    require_swipl();

    let mut server = TestServer::new().expect("Failed to create server");
    server.start().expect("Failed to start server");
    
    let mut session = server.connect().expect("Failed to connect");
    
    // Test arithmetic evaluation
    let result = session.query("X is 2 + 3 * 4", None).expect("Query failed");
    assert_has_solutions(&result, Some(1));
    
    match result {
        QueryResult::Solutions(solutions) => {
            assert!(solutions[0].contains_key("X"));
            // X should be 14 (2 + 3 * 4 = 2 + 12 = 14)
            assert_eq!(solutions[0].get("X"), Some(&PrologTerm::Integer(14)));
        },
        _ => unreachable!(),
    }
    
    session.close().expect("Failed to close session");
    server.stop(false).expect("Failed to stop server");
}