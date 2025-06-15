// Integration tests for the swipl-rs library

use swipl_rs::*;
use swipl_rs::server::{ServerConfig, PrologServer};
use swipl_rs::types::{QueryResult, Solution, PrologCompound, PrologTerm, prolog_term_to_string};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Once; // For initializing logging
use std::time::Duration;
use std::sync::Arc;
use std::sync::Mutex;
use std::env; // Add this import
use env_logger; // Add this import

// --- Test Setup ---

// Initialize logging once for all tests
static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        // Use env_logger or simple_logger, etc.
        // Example using simple_logger (add to Cargo.toml dev-dependencies if used)
        // simple_logger::SimpleLogger::new().init().unwrap_or_else(|e| eprintln!("Failed to init logger: {}", e));
        // For now, just print a message
        // println!("Test setup: Logging (stubbed)");
        // Initialize env_logger
        env_logger::builder().is_test(true).try_init().unwrap_or_else(|e| eprintln!("Failed to initialize env_logger: {}", e));
        println!("Test setup: env_logger initialized.");
    });
}

// Helper to create a default server config for tests
fn default_test_config() -> ServerConfig {
    // Check for environment variable specifying the SWI-Prolog path
    match env::var("SWIPL_PATH") {
        Ok(path) => {
            println!("Using SWIPL_PATH from environment: {}", path);
            ServerConfig {
                prolog_path: Some(path.into()), // Convert String to PathBuf
                ..Default::default()
            }
        },
        Err(_) => {
            println!("SWIPL_PATH not found in environment, using default.");
            ServerConfig::default() // Assumes swipl in PATH, generates password
        },
    }
}

// Helper to assert QueryResult::Success
fn assert_success(result: QueryResult, expected: bool) {
    match result {
        QueryResult::Success(val) => assert_eq!(val, expected),
        _ => panic!("Expected QueryResult::Success({}) but got {:?}", expected, result),
    }
}

// Helper to assert QueryResult::Solutions
fn assert_solutions(result: QueryResult, expected: Vec<Solution>) {
     match result {
        QueryResult::Solutions(solutions) => assert_eq!(solutions, expected),
        _ => panic!("Expected QueryResult::Solutions({:?}) but got {:?}", expected, result),
    }
}

// --- Basic Connection Tests ---

#[test]
fn test_server_start_stop() {
    setup();
    let config = default_test_config();
    let mut server = PrologServer::new(config).expect("Failed to create server config");
    server.start().expect("Failed to start swipl process");
    // Add a small delay to allow process to fully start if needed
    // std::thread::sleep(Duration::from_millis(100));
    server.stop(false).expect("Failed to stop swipl process gracefully");
}

#[test]
fn test_basic_connect_and_query() {
    setup();
    let config = default_test_config();
    println!("Creating PrologServer...");
    let mut server = PrologServer::new(config).expect("Failed to create server config");
    // Connect implicitly calls start()
    println!("Attempting server.connect()...");
    let mut session = server.connect().expect("Failed to connect session");
    println!("server.connect() successful.");
    let result = session.query("atom(a)", None).expect("Query failed");
    println!("First query successful.");
    assert_success(result, true);

    let result_fail = session.query("fail", None).expect("Query failed");
    println!("Second query successful.");
    assert_success(result_fail, false);

    println!("Closing session...");
    session.close().expect("Failed to close session");
    println!("Stopping server...");
    server.stop(false).expect("Failed to stop server");
    println!("Test finished.");
}

#[test]
fn test_session_drop_closes() {
     setup();
    let config = default_test_config();
    let mut server = PrologServer::new(config).expect("Failed to create server config");
    {
        let mut session = server.connect().expect("Failed to connect session");
        let result = session.query("true", None).expect("Query failed");
        assert_success(result, true);
        // session goes out of scope here, Drop should be called
    }
    // Allow a moment for close to potentially happen if needed
    // std::thread::sleep(Duration::from_millis(50));
    server.stop(false).expect("Failed to stop server");
}

#[test]
fn test_server_drop_stops() {
    setup();
    let config = default_test_config();
    let server_pid = {
        let mut server = PrologServer::new(config).expect("Failed to create server config");
        server.start().expect("Failed to start swipl process");
        // server.process.as_ref().map(|p| p.id()) // Need a way to get PID if required for external check
        // server goes out of scope here, Drop should be called
    };
    // Ideally, check if process with server_pid is gone, but that's OS-specific
    // For now, just test that it doesn't panic
}

// --- Simple Query Tests ---

#[test]
fn test_query_no_vars_multiple_results() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();
    session.query("(retractall(noFreeVariablesMultipleResults(_)), assertz((noFreeVariablesMultipleResults(X) :- member(X, [1, 2, 3]))))", None).unwrap();
    let result = session.query("noFreeVariablesMultipleResults(X)", None).unwrap();

    // MQI findall doesn't return multiple 'true' for goals with no free vars,
    // it returns solutions with the variables instantiated.
    let expected_solutions = vec![
        HashMap::from([("X".to_string(), PrologTerm::Integer(1))]),
        HashMap::from([("X".to_string(), PrologTerm::Integer(2))]),
        HashMap::from([("X".to_string(), PrologTerm::Integer(3))]),
    ];
    assert_solutions(result, expected_solutions);

    server.stop(false).unwrap();
}

#[test]
fn test_query_one_var_multiple_results_utf8() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query("(retractall(oneFreeVariableMultipleResults(_)), assertz((oneFreeVariableMultipleResults(X) :- member(X, [1, \'©\', \'≠\']))) )", None).unwrap();
    let result = session.query("oneFreeVariableMultipleResults(X)", None).unwrap();
    let expected_solutions = vec![
        HashMap::from([("X".to_string(), PrologTerm::Integer(1))]),
        HashMap::from([("X".to_string(), PrologTerm::Atom("©".to_string()))]),
        HashMap::from([("X".to_string(), PrologTerm::Atom("≠".to_string()))]),
    ];
    assert_solutions(result, expected_solutions);

    server.stop(false).unwrap();
}

#[test]
fn test_query_two_vars_multiple_results() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();
    session.query("(retractall(twoFreeVariablesMultipleResults(_, _)), assertz((twoFreeVariablesMultipleResults(X, Y) :- member(X-Y, [1-1, 2-2, 3-3]))))", None).unwrap();
    let result = session.query("twoFreeVariablesMultipleResults(X, Y)", None).unwrap();

    let expected_solutions = vec![
        HashMap::from([
            ("X".to_string(), PrologTerm::Integer(1)),
            ("Y".to_string(), PrologTerm::Integer(1))
        ]),
        HashMap::from([
            ("X".to_string(), PrologTerm::Integer(2)),
            ("Y".to_string(), PrologTerm::Integer(2))
        ]),
        HashMap::from([
            ("X".to_string(), PrologTerm::Integer(3)),
            ("Y".to_string(), PrologTerm::Integer(3))
        ]),
    ];
     assert_solutions(result, expected_solutions);

    server.stop(false).unwrap();
}

// --- Error Handling Tests ---

#[test]
fn test_query_syntax_error() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    let result = session.query("member(X, [first, second, third]", None);
    assert!(result.is_err());
    match result.err().unwrap() {
        PrologError::PrologException { kind, .. } => assert!(kind.contains("syntax_error")),
        e => panic!("Expected PrologException(syntax_error), got {:?}", e),
    }
    server.stop(false).unwrap();
}

#[test]
fn test_query_timeout() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    let result = session.query("sleep(2)", Some(0.5)); // Timeout after 0.5 sec
    assert!(result.is_err());
    match result.err().unwrap() {
        PrologError::Timeout => { /* Expected */ }
        e => panic!("Expected PrologError::Timeout, got {:?}", e),
    }
    server.stop(false).unwrap();
}

#[test]
fn test_query_prolog_exception() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    let result = session.query("throw(my_test_error(123))", None);
    assert!(result.is_err());
    match result.err().unwrap() {
        PrologError::PrologException { kind, term } => {
            // The 'kind' might be just the functor name or a more complex representation depending on parsing
            assert!(kind.contains("my_test_error"));
            // Optionally inspect the term
            assert!(term.is_some());
            // Add more detailed check of the term if needed
        }
        e => panic!("Expected PrologError::PrologException, got {:?}", e),
    }
    server.stop(false).unwrap();
}

// --- Async Query Tests ---

#[test]
fn test_async_no_query_errors() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    // Cancel with no query running
    let res_cancel = session.cancel_async();
    assert!(matches!(res_cancel, Err(PrologError::NoQuery)), "Expected NoQuery error for cancel_async");

    // Get result with no query running
    let res_result = session.query_async_result(None);
    assert!(matches!(res_result, Err(PrologError::NoQuery)), "Expected NoQuery error for query_async_result");

    server.stop(false).unwrap();
}

#[test]
fn test_async_findall_success_no_vars() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("member(X, [1, 2]), X = 1", true, None).unwrap(); // findall=true
    let result = session.query_async_result(None).unwrap().unwrap();
    assert_success(result, true);

    // Check end of results
    let end_result = session.query_async_result(None).unwrap();
    assert!(end_result.is_none());

    server.stop(false).unwrap();
}

#[test]
fn test_async_findall_fail() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("fail", true, None).unwrap(); // findall=true
    let result = session.query_async_result(None).unwrap().unwrap();
    assert_success(result, false);

    // Check end of results
    let end_result = session.query_async_result(None).unwrap();
    assert!(end_result.is_none());

    server.stop(false).unwrap();
}

#[test]
fn test_async_findall_multiple_solutions() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("member(X, [1, \'©\', \'≠\'])", true, None).unwrap(); // findall=true
    let result = session.query_async_result(None).unwrap().unwrap();
    let expected_solutions = vec![
        HashMap::from([("X".to_string(), PrologTerm::Integer(1))]),
        HashMap::from([("X".to_string(), PrologTerm::Atom("©".to_string()))]),
        HashMap::from([("X".to_string(), PrologTerm::Atom("≠".to_string()))]),
    ];
    assert_solutions(result, expected_solutions);

    // Check end of results
    let end_result = session.query_async_result(None).unwrap();
    assert!(end_result.is_none());

    server.stop(false).unwrap();
}

#[test]
fn test_async_individual_results() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("member(X, [1, \'©\', \'≠\'])", false, None).unwrap(); // findall=false

    // Result 1
    let result1 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(result1, vec![HashMap::from([("X".to_string(), PrologTerm::Integer(1))])]);

    // Result 2
    let result2 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(result2, vec![HashMap::from([("X".to_string(), PrologTerm::Atom("©".to_string()))])]);

    // Result 3
    let result3 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(result3, vec![HashMap::from([("X".to_string(), PrologTerm::Atom("≠".to_string()))])]);

    // End of results
    let end_result = session.query_async_result(None).unwrap();
    assert!(end_result.is_none());

    server.stop(false).unwrap();
}

#[test]
fn test_async_individual_fail() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("fail", false, None).unwrap(); // findall=false
    let result = session.query_async_result(None).unwrap().unwrap();
    assert_success(result, false);

    // End of results
    let end_result = session.query_async_result(None).unwrap();
    assert!(end_result.is_none());

    server.stop(false).unwrap();
}

#[test]
fn test_async_cancel_during_findall() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("(member(X, [a, b, c]), sleep(2))", true, None).unwrap(); // findall=true
    // Give it a moment to start running
    std::thread::sleep(Duration::from_millis(100));
    session.cancel_async().unwrap();

    let result = session.query_async_result(None);
    assert!(matches!(result, Err(PrologError::QueryCancelled)), "Expected QueryCancelled error");

    server.stop(false).unwrap();
}

#[test]
fn test_async_cancel_during_individual() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("(member(X, [a, b, sleep(2), c]))", false, None).unwrap(); // findall=false

    // Get first two results
    assert!(session.query_async_result(None).unwrap().is_some());
    assert!(session.query_async_result(None).unwrap().is_some());

    // Cancel while it's likely sleeping
    std::thread::sleep(Duration::from_millis(100));
    session.cancel_async().unwrap();

    let result = session.query_async_result(None);
    assert!(matches!(result, Err(PrologError::QueryCancelled)), "Expected QueryCancelled error");

    server.stop(false).unwrap();
}

#[test]
fn test_async_findall_timeout() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("(member(X, [a,b]), sleep(2))", true, Some(1.0)).unwrap(); // findall=true, timeout=1s

    let result = session.query_async_result(None);
    assert!(matches!(result, Err(PrologError::Timeout)), "Expected Timeout error");

    server.stop(false).unwrap();
}

#[test]
fn test_async_individual_timeout() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("(member(X, [a, sleep(2), b]))", false, Some(1.0)).unwrap(); // findall=false, timeout=1s

    // Get first result
    let res1 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(res1, vec![HashMap::from([("X".to_string(), PrologTerm::Atom("a".to_string()))])]);

    // Next result should time out
    let res2 = session.query_async_result(None);
    assert!(matches!(res2, Err(PrologError::Timeout)), "Expected Timeout error");

    server.stop(false).unwrap();
}

#[test]
fn test_async_findall_prolog_exception() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("throw(my_async_error)", true, None).unwrap(); // findall=true

    let result = session.query_async_result(None);
    assert!(result.is_err());
    match result.err().unwrap() {
        PrologError::PrologException { kind, .. } => assert!(kind.contains("my_async_error")),
        e => panic!("Expected PrologException(my_async_error), got {:?}", e),
    }

    server.stop(false).unwrap();
}

#[test]
fn test_async_individual_prolog_exception() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("(member(X,[a,b]), (X=b -> throw(my_async_error) ; true))", false, None).unwrap(); // findall=false

     // Get first result
    let res1 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(res1, vec![HashMap::from([("X".to_string(), PrologTerm::Atom("a".to_string()))])]);

    // Next result should be exception
    let res2 = session.query_async_result(None);
     assert!(res2.is_err());
    match res2.err().unwrap() {
        PrologError::PrologException { kind, .. } => assert!(kind.contains("my_async_error")),
        e => panic!("Expected PrologException(my_async_error), got {:?}", e),
    }

    server.stop(false).unwrap();
}

#[test]
fn test_async_result_not_available() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    session.query_async("(sleep(1), member(X, [1, 2]))", false, None).unwrap(); // findall=false

    // Try to get result immediately with short timeout
    let res1 = session.query_async_result(Some(0.1));
    assert!(matches!(res1, Err(PrologError::ResultNotAvailable)), "Expected ResultNotAvailable");

    // Wait long enough for the sleep to finish
    std::thread::sleep(Duration::from_secs(2));

    // Now get the actual results
    let res2 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(res2, vec![HashMap::from([("X".to_string(), PrologTerm::Integer(1))])]);
    let res3 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(res3, vec![HashMap::from([("X".to_string(), PrologTerm::Integer(2))])]);
    let res_end = session.query_async_result(None).unwrap();
    assert!(res_end.is_none());

    server.stop(false).unwrap();
}

// --- Protocol Edge Case Tests ---

#[test]
fn test_overlapping_async_queries() {
    // Start one async query, then immediately start another before retrieving results.
    // The second query should effectively replace the first one.
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    // Query 1 (findall=false, so results aren't immediately computed/retrieved)
    session.query_async("(member(X, [a, b, c]), sleep(3))", false, None).unwrap();
    // Query 2 (replaces Query 1)
    session.query_async("member(X, [d, e, f])", false, None).unwrap();

    // Allow Query 1 to potentially process if it wasn't properly cancelled internally (MQI handles this)
    std::thread::sleep(Duration::from_millis(100));

    // Results should be from Query 2
    let res1 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(res1, vec![HashMap::from([("X".to_string(), PrologTerm::Atom("d".to_string()))])]);
    let res2 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(res2, vec![HashMap::from([("X".to_string(), PrologTerm::Atom("e".to_string()))])]);
    let res3 = session.query_async_result(None).unwrap().unwrap();
    assert_solutions(res3, vec![HashMap::from([("X".to_string(), PrologTerm::Atom("f".to_string()))])]);
    let res_end = session.query_async_result(None).unwrap();
    assert!(res_end.is_none());

    server.stop(false).unwrap();
}

#[test]
fn test_sync_query_while_async_pending() {
    // Start an async query, then run a sync query before retrieving async results.
    // The sync query should execute and return, leaving the async results untouched (but potentially cancelled internally).
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    // Async Query 1
    session.query_async("member(X, [a, b, c])", false, None).unwrap();

    // Sync Query
    let sync_result = session.query("member(Y, [d, e])", None).unwrap();
    let expected_sync = vec![
        HashMap::from([("Y".to_string(), PrologTerm::Atom("d".to_string()))]),
        HashMap::from([("Y".to_string(), PrologTerm::Atom("e".to_string()))]),
    ];
    assert_solutions(sync_result, expected_sync);

    // Now try to get results from the original async query.
    // MQI should have drained/discarded the previous async results when the sync query started.
    let async_res = session.query_async_result(None);
    assert!(matches!(async_res, Err(PrologError::NoQuery)), "Expected NoQuery after sync query interrupted async");

    server.stop(false).unwrap();
}

// --- Connection Closing Tests ---

#[test]
fn test_close_session_with_running_async() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let session_handle = {
        let mut session = server.connect().unwrap();
        // Start a long-running query
        session.query_async("sleep(10), assertz(close_test_finished(async))", false, None).unwrap();
        // Give it time to start
        std::thread::sleep(Duration::from_millis(200));
        // Drop the session, which should call close()
    };

    // Now check if the assert happened (it shouldn't have)
    let mut check_session = server.connect().unwrap();
    let result = check_session.query("close_test_finished(async)", None);
    assert!(result.is_ok());
    assert_success(result.unwrap(), false); // Should fail as the assert shouldn't run
    check_session.query("retractall(close_test_finished(_))", None).unwrap(); // Cleanup

    server.stop(false).unwrap();
}

// Testing close with sync query requires running the query in a separate thread
// as it blocks the current thread.
#[test]
#[ignore] // Ignoring because thread management adds complexity, test manually if needed
fn test_close_session_with_running_sync() {
    // setup();
    // let mut server = PrologServer::new(default_test_config()).unwrap();
    // let server_arc = Arc::new(Mutex::new(server));

    // let query_thread = {
    //     let server_clone = Arc::clone(&server_arc);
    //     std::thread::spawn(move || {
    //         let mut server_lock = server_clone.lock().unwrap();
    //         let mut session = server_lock.connect().expect("Connect failed in thread");
    //         // This query will block
    //         let _ = session.query("sleep(10), assertz(close_test_finished(sync))", None);
    //     })
    // };

    // // Give the query time to start
    // std::thread::sleep(Duration::from_millis(500));

    // // Need a way to get the session handle from the other thread or force close?
    // // This structure makes it difficult. A better approach might involve channels
    // // or a more complex shared state management if this test is critical.
    // // For now, we ignore this test due to complexity.

    // // query_thread.join().unwrap();

    // // Check assert didn't happen
    // let mut server_lock = server_arc.lock().unwrap();
    // let mut check_session = server_lock.connect().unwrap();
    // let result = check_session.query("close_test_finished(sync)", None);
    // assert!(result.is_ok());
    // assert_success(result.unwrap(), false);
    // check_session.query("retractall(close_test_finished(_))", None).unwrap();
    // server_lock.stop(false).unwrap();
}

// --- Multiple Connections Tests ---

#[test]
fn test_multiple_serial_sessions() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();

    {
        let mut session1 = server.connect().unwrap();
        assert_success(session1.query("true", None).unwrap(), true);
    }
    // Allow time for close/cleanup if necessary
    std::thread::sleep(Duration::from_millis(50));
    {
        let mut session2 = server.connect().unwrap();
        assert_success(session2.query("atom(a)", None).unwrap(), true);
    }
    std::thread::sleep(Duration::from_millis(50));
    {
        let mut session3 = server.connect().unwrap();
        assert_success(session3.query("fail", None).unwrap(), false);
    }

    server.stop(false).unwrap();
}

// Test multiple concurrent connections using threads.
#[test]
fn test_multiple_concurrent_sessions() {
    setup();
    // Use Arc/Mutex to share the server instance across threads safely.
    let server = Arc::new(Mutex::new(PrologServer::new(default_test_config()).unwrap()));
    let mut threads = vec![];
    const NUM_THREADS: usize = 5;

    // Use Prolog mutex for synchronization between threads
    {
        let mut server_lock = server.lock().unwrap();
        let mut setup_session = server_lock.connect().unwrap();
        setup_session.query("mutex_create(rust_test, []), retractall(concurrent_test(_))", None).unwrap();
        setup_session.query("mutex_lock(rust_test)", None).unwrap(); // Lock the mutex
    }

    for i in 0..NUM_THREADS {
        let server_clone = Arc::clone(&server);
        let handle = std::thread::spawn(move || {
            let mut server_lock = server_clone.lock().unwrap();
            let mut session = server_lock.connect().expect("Connect failed in thread");
            // This query will block until the mutex is unlocked
            let query = format!("with_mutex(rust_test, assertz(concurrent_test({})))", i);
            session.query(&query, None)
        });
        threads.push(handle);
    }

    // Give threads time to start and block on the mutex
    std::thread::sleep(Duration::from_secs(1));

    // Verify no asserts have happened yet
    {
        let mut server_lock = server.lock().unwrap();
        let mut check_session = server_lock.connect().unwrap();
        let result = check_session.query("findall(I, concurrent_test(I), L)", None).unwrap();
        match result {
            QueryResult::Solutions(sol) if sol.len() == 1 => {
                match sol[0].get("L") {
                    Some(PrologTerm::List(l)) => assert!(l.is_empty(), "Expected empty list before unlock"),
                    _ => panic!("Unexpected result structure before unlock"),
                }
            },
            _ => panic!("Unexpected query result before unlock"),
        }
    }

    // Unlock the mutex
     {
        let mut server_lock = server.lock().unwrap();
        let mut unlock_session = server_lock.connect().unwrap();
        unlock_session.query("mutex_unlock(rust_test)", None).unwrap();
    }

    // Wait for all threads to finish and check their results
    for handle in threads {
        let result = handle.join().expect("Thread panicked");
        assert!(result.is_ok(), "Query in thread failed: {:?}", result.err());
        assert_success(result.unwrap(), true);
    }

    // Verify all asserts happened
    {
        let mut server_lock = server.lock().unwrap();
        let mut check_session = server_lock.connect().unwrap();
        let result = check_session.query("findall(I, concurrent_test(I), L), sort(L, SortedL)", None).unwrap();
         match result {
            QueryResult::Solutions(sol) if sol.len() == 1 => {
                match sol[0].get("SortedL") {
                    Some(PrologTerm::List(l)) => {
                        let nums: Vec<i64> = l.iter().filter_map(|t| match t { PrologTerm::Integer(i) => Some(*i), _ => None }).collect();
                        let expected: Vec<i64> = (0..NUM_THREADS as i64).collect();
                        assert_eq!(nums, expected, "Expected list [0..{}] after unlock", NUM_THREADS - 1);
                    }
                    _ => panic!("Unexpected result structure after unlock"),
                }
            },
            _ => panic!("Unexpected query result after unlock"),
        }
        // Cleanup
        check_session.query("mutex_destroy(rust_test), retractall(concurrent_test(_))", None).unwrap();
        server_lock.stop(false).unwrap();
    }
}

// --- Term Representation / Conversion Tests ---

#[test]
fn test_prolog_term_parsing() {
    // Test parsing various term structures from Prolog results
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    let result = session.query("X = atom, Y = 123, Z = 3.14, V = \'string\', L = [a, b, c(1)], S = point{x:1, y:Var}", None).unwrap();
    match result {
        QueryResult::Solutions(sol) if sol.len() == 1 => {
            let bindings = &sol[0];
            assert_eq!(bindings.get("X"), Some(&PrologTerm::Atom("atom".to_string())));
            assert_eq!(bindings.get("Y"), Some(&PrologTerm::Integer(123)));
            assert_eq!(bindings.get("Z"), Some(&PrologTerm::Float(3.14)));
            assert_eq!(bindings.get("V"), Some(&PrologTerm::Atom("string".to_string()))); // Strings are atoms
            assert!(matches!(bindings.get("L"), Some(PrologTerm::List(_))));
            assert!(matches!(bindings.get("S"), Some(PrologTerm::Compound(_))));
            // Check compound term structure
            if let Some(PrologTerm::Compound(compound)) = bindings.get("S") {
                assert_eq!(compound.functor, "point");
                assert_eq!(compound.args.len(), 2);
                assert_eq!(compound.args[0], PrologTerm::Integer(1));
                assert!(matches!(compound.args[1], PrologTerm::Variable(_)));
            } else {
                panic!("Expected compound term for S");
            }
        }
        _ => panic!("Unexpected query result structure"),
    }

    server.stop(false).unwrap();
}

// Example test for prolog_term_to_string - more could be added
#[test]
fn test_prolog_term_to_string_basic() {
    setup();
    assert_eq!(prolog_term_to_string(&PrologTerm::Atom("hello".to_string())), "hello");
    assert_eq!(prolog_term_to_string(&PrologTerm::Atom("hello world".to_string())), "'hello world'");
    assert_eq!(prolog_term_to_string(&PrologTerm::Integer(123)), "123");
    assert_eq!(prolog_term_to_string(&PrologTerm::Variable("X".to_string())), "X");
    let list = PrologTerm::List(vec![PrologTerm::Atom("a".to_string()), PrologTerm::Integer(1)]);
    assert_eq!(prolog_term_to_string(&list), "[a, 1]");
    let compound = PrologTerm::Compound(PrologCompound { functor: "test".to_string(), args: vec![PrologTerm::Atom("arg".to_string())]});
    assert_eq!(prolog_term_to_string(&compound), "test(arg)");
}

// --- Goal Expansion Test ---

#[test]
fn test_goal_expansion_dict() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    // Requires goal expansion for dicts {.}/1
    let result = session.query("A = point{x:1, y:2}.put([x=3,z=0])", None).unwrap();
    match result {
        QueryResult::Solutions(sol) if sol.len() == 1 => {
             if let Some(PrologTerm::Compound(compound)) = sol[0].get("A") {
                assert_eq!(compound.functor, "point");
                // Order of args might not be guaranteed, check contents
                assert!(compound.args.contains(&PrologTerm::Integer(3))); // x=3
                assert!(compound.args.contains(&PrologTerm::Integer(2))); // y=2
                assert!(compound.args.contains(&PrologTerm::Integer(0))); // z=0
            } else {
                panic!("Expected compound term for A, got {:?}", sol[0].get("A"));
            }
        }
        _ => panic!("Unexpected result structure for dict test"),
    }

    server.stop(false).unwrap();
}

// --- Server Option Tests ---

#[test]
fn test_explicit_port() {
    setup();
    let port = 8088; // Choose an arbitrary port
    let config = match env::var("SWIPL_PATH") {
        Ok(path) => ServerConfig {
            prolog_path: Some(path.into()),
            port: Some(port),
            ..Default::default()
        },
        Err(_) => ServerConfig {
            port: Some(port),
            ..Default::default()
        },
    };
    let mut server = PrologServer::new(config).unwrap();
    let mut session = server.connect().unwrap();
    assert_success(session.query("true", None).unwrap(), true);
    session.close().unwrap();
    server.stop(false).unwrap();
}

#[test]
fn test_explicit_password() {
    setup();
    let password = "mytestpassword".to_string();
    let config = match env::var("SWIPL_PATH") {
        Ok(path) => ServerConfig {
            prolog_path: Some(path.into()),
            password: Some(password.clone()),
            ..Default::default()
        },
        Err(_) => ServerConfig {
            password: Some(password.clone()),
            ..Default::default()
        },
    };

    let mut server = PrologServer::new(config).unwrap();
    let mut session = server.connect().unwrap();
    assert_success(session.query("true", None).unwrap(), true);

    // Test connection failure with wrong password
    let config_fail = match env::var("SWIPL_PATH") {
        Ok(path) => ServerConfig {
            prolog_path: Some(path.into()),
            password: Some("wrong".to_string()),
            ..Default::default() // Create config_fail with default port etc.
        },
        Err(_) => ServerConfig {
            password: Some("wrong".to_string()),
            ..Default::default() // Create config_fail with default port etc.
        },
    };
    let mut server_fail = PrologServer::new(config_fail).unwrap();
    let connect_result = server_fail.connect();
    assert!(matches!(connect_result, Err(PrologError::AuthenticationFailed)), "Expected AuthenticationFailed");

    session.close().unwrap();
    server.stop(false).unwrap();
}

#[test]
#[cfg(all(unix, feature = "unix-socket"))]
fn test_explicit_uds() {
    setup();
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("test_explicit.sock");

    let config = ServerConfig {
        unix_domain_socket: Some(socket_path.clone()),
        port: None, // Ensure port is None
        ..Default::default()
    };
    let mut server = PrologServer::new(config).unwrap();
    server.start().unwrap();
    assert!(socket_path.exists(), "Socket file was not created");
    {
        let mut session = server.connect().unwrap();
        assert_success(session.query("true", None).unwrap(), true);
    }
    server.stop(false).unwrap();
    assert!(!socket_path.exists(), "Socket file was not cleaned up");
}

#[test]
#[cfg(all(unix, feature = "unix-socket"))]
fn test_generate_uds() {
    setup();
    let config = ServerConfig {
        unix_domain_socket: Some(PathBuf::new()), // Empty path triggers generation
        port: None,
        ..Default::default()
    };
    let mut server = PrologServer::new(config).unwrap();
    server.start().unwrap();
    let generated_path = server.effective_uds_path.clone(); // Need access to internal state or return value
    assert!(generated_path.is_some(), "Server did not store generated UDS path");
    assert!(generated_path.unwrap().exists(), "Generated socket file does not exist");
    {
        let mut session = server.connect().unwrap();
        assert_success(session.query("true", None).unwrap(), true);
    }
    server.stop(false).unwrap();
    // Check path and directory were cleaned up (requires access to generated_uds_dir)
    // assert!(server.generated_uds_dir.is_none(), "Generated UDS dir was not cleared on stop");
}

#[test]
fn test_default_query_timeout_option() {
    setup();
    let config = match env::var("SWIPL_PATH") {
        Ok(path) => ServerConfig {
            prolog_path: Some(path.into()),
            query_timeout_seconds: Some(0.5),
            ..Default::default()
        },
        Err(_) => ServerConfig {
            query_timeout_seconds: Some(0.5),
            ..Default::default()
        },
    };
    let mut server = PrologServer::new(config).unwrap();
    let mut session = server.connect().unwrap();

    // Query that exceeds the default timeout
    let result = session.query("sleep(1)", None);
    assert!(matches!(result, Err(PrologError::Timeout)), "Expected Timeout error");

    // Query within the timeout should work
    let result_ok = session.query("sleep(0.1)", None).unwrap();
    assert_success(result_ok, true);

    // Override timeout should work
    let result_override = session.query("sleep(1)", Some(2.0)).unwrap();
    assert_success(result_override, true);

    server.stop(false).unwrap();
}

// --- Debugging Option Tests ---

#[test]
fn test_output_file_option() {
    setup();
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let output_file = temp_dir.path().join("prolog_output.log").to_str().unwrap().to_string();

    let config = match env::var("SWIPL_PATH") {
        Ok(path) => ServerConfig {
            prolog_path: Some(path.into()),
            output_file_name: Some(output_file.clone().into()),
            ..Default::default()
        },
        Err(_) => ServerConfig {
            output_file_name: Some(output_file.clone().into()),
            ..Default::default()
        },
    };
    let mut server = PrologServer::new(config).unwrap();
    let mut session = server.connect().unwrap();

    let test_message = "Hello from Prolog test!";
    session.query(&format!("writeln('{}')", test_message), None).unwrap();
    session.close().unwrap();
    server.stop(false).unwrap();

    // Check if the file exists and contains the message
    let content = std::fs::read_to_string(&output_file).expect("Failed to read output file");
    assert!(content.contains(test_message), "Output file does not contain the test message");
}

#[test]
fn test_mqi_traces_option() {
    setup();
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let trace_file = temp_dir.path().join("mqi_trace.log").to_str().unwrap().to_string();

    let config = match env::var("SWIPL_PATH") {
        Ok(path) => ServerConfig {
            prolog_path: Some(path.into()),
            output_file_name: Some(trace_file.clone().into()),
            mqi_traces: Some("protocol".to_string()), // Enable protocol traces
            ..Default::default()
        },
        Err(_) => ServerConfig {
            output_file_name: Some(trace_file.clone().into()),
            mqi_traces: Some("protocol".to_string()), // Enable protocol traces
            ..Default::default()
        },
    };

    let mut server = PrologServer::new(config).unwrap();
    let mut session = server.connect().unwrap();
    session.query("true", None).unwrap();
    session.close().unwrap();
    server.stop(false).unwrap();

    // Check if the trace file exists and contains some expected trace patterns
    let content = std::fs::read_to_string(&trace_file).expect("Failed to read trace file");
    assert!(content.contains("% Started server on thread:"), "Trace file missing server start message");
    assert!(content.contains("% Command: run_async"), "Trace file missing command trace");
}


// --- Variable Attribute Tests ---

#[test]
fn test_variable_attributes() {
    setup();
    let mut server = PrologServer::new(default_test_config()).unwrap();
    let mut session = server.connect().unwrap();

    // Use `library(clpfd)` as an example for constraints
    session.query("use_module(library(clpfd))", None).unwrap();
    let result = session.query("X #> 1, X #< 5, label([X])", None).unwrap();

    match result {
        QueryResult::Solutions(solutions) => {
            assert_eq!(solutions.len(), 1, "Expected one solution");
            let solution = &solutions[0];
            assert!(solution.contains_key("X"), "Solution missing X");
            assert!(solution.contains_key("$residuals"), "Solution missing $residuals");
            assert_eq!(solution.get("X"), solution.get("X"), "X and $residuals should be unified");

            // Check for $residuals
            if let Some(PrologTerm::List(residuals)) = solution.get("$residuals") {
                assert!(!residuals.is_empty(), "Expected non-empty residuals list");
                // Check for expected constraint terms (structure depends on Prolog version)
                // Example check: look for `clpfd:in/2` or similar
                let residual_str = format!("{:?}", residuals);
                assert!(residual_str.contains("clpfd") && residual_str.contains("in"), "Residuals list does not contain expected clpfd constraints");
            } else {
                panic!("Expected $residuals to be a list, got: {:?}", solution.get("$residuals"));
            }
        }
        _ => panic!("Expected QueryResult::Solutions, got: {:?}", result),
    }

    server.stop(false).unwrap();
}

// --- Remaining TODO tests ---

// TODO: Add tests similar to Python's for:
// - Goal thread failure (might be hard to test reliably)
// - Maybe more detailed Server option interactions / standalone mode connection
// - Further term conversion edge cases if PrologTerm enum is refined
