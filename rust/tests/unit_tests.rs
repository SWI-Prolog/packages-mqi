use serde_json::json;
use std::collections::HashMap;
use swipl_rs::types::{
    is_prolog_atom, is_prolog_functor, is_prolog_variable, prolog_args, prolog_name,
};
use swipl_rs::{PrologCompound, PrologError, PrologTerm, QueryResult};

#[test]
fn test_prolog_term_serialization() {
    // Test atom
    let atom = PrologTerm::Atom("hello".to_string());
    let json = serde_json::to_value(&atom).unwrap();
    assert_eq!(json, json!("hello"));

    // Test variable
    let var = PrologTerm::Variable("X".to_string());
    let json = serde_json::to_value(&var).unwrap();
    assert_eq!(json, json!("X"));

    // Test integer
    let int = PrologTerm::Integer(42);
    let json = serde_json::to_value(&int).unwrap();
    assert_eq!(json, json!(42));

    // Test float
    let float = PrologTerm::Float(3.14);
    let json = serde_json::to_value(&float).unwrap();
    assert_eq!(json, json!(3.14));

    // Test list
    let list = PrologTerm::List(vec![
        PrologTerm::Integer(1),
        PrologTerm::Integer(2),
        PrologTerm::Integer(3),
    ]);
    let json = serde_json::to_value(&list).unwrap();
    assert_eq!(json, json!([1, 2, 3]));

    // Test compound
    let compound = PrologTerm::Compound(PrologCompound {
        functor: "foo".to_string(),
        args: vec![PrologTerm::Atom("bar".to_string()), PrologTerm::Integer(42)],
    });
    let json = serde_json::to_value(&compound).unwrap();
    assert_eq!(
        json,
        json!({
            "functor": "foo",
            "args": ["bar", 42]
        })
    );
}

#[test]
fn test_prolog_term_deserialization() {
    // Test atom
    let json = json!("hello");
    let term: PrologTerm = serde_json::from_value(json).unwrap();
    assert_eq!(term, PrologTerm::Atom("hello".to_string()));

    // Test that strings are deserialized as atoms (untagged enum behavior)
    let json = json!("X");
    let term: PrologTerm = serde_json::from_value(json).unwrap();
    assert_eq!(term, PrologTerm::Atom("X".to_string()));

    // Variables must be explicitly constructed or use the helper functions
    assert!(is_prolog_variable(&json!("X")));
    assert!(!is_prolog_atom(&json!("X")));

    // Test integer
    let json = json!(42);
    let term: PrologTerm = serde_json::from_value(json).unwrap();
    assert_eq!(term, PrologTerm::Integer(42));

    // Test compound
    let json = json!({
        "functor": "foo",
        "args": ["bar", 42]
    });
    let term: PrologTerm = serde_json::from_value(json).unwrap();
    match term {
        PrologTerm::Compound(c) => {
            assert_eq!(c.functor, "foo");
            assert_eq!(c.args.len(), 2);
        }
        _ => panic!("Expected Compound term"),
    }
}

#[test]
fn test_query_result_parse_solutions() {
    // Test empty solution
    let json = json!([[]]);
    let solutions = json.as_array().unwrap();
    let result = QueryResult::parse_solutions(solutions).unwrap();
    match result {
        QueryResult::Solutions(sols) => {
            assert_eq!(sols.len(), 1);
            assert!(sols[0].is_empty());
        }
        _ => panic!("Expected Solutions"),
    }

    // Test single variable binding
    let json = json!([[{"functor": "=", "args": ["X", 42]}]]);
    let solutions = json.as_array().unwrap();
    let result = QueryResult::parse_solutions(solutions).unwrap();
    match result {
        QueryResult::Solutions(sols) => {
            assert_eq!(sols.len(), 1);
            assert_eq!(sols[0].len(), 1);
            assert!(sols[0].contains_key("X"));
        }
        _ => panic!("Expected Solutions"),
    }

    // Test multiple variable bindings
    let json = json!([[
        {"functor": "=", "args": ["X", 1]},
        {"functor": "=", "args": ["Y", "hello"]}
    ]]);
    let solutions = json.as_array().unwrap();
    let result = QueryResult::parse_solutions(solutions).unwrap();
    match result {
        QueryResult::Solutions(sols) => {
            assert_eq!(sols.len(), 1);
            assert_eq!(sols[0].len(), 2);
            assert!(sols[0].contains_key("X"));
            assert!(sols[0].contains_key("Y"));
        }
        _ => panic!("Expected Solutions"),
    }

    // Test multiple solutions
    let json = json!([
        [{"functor": "=", "args": ["X", 1]}],
        [{"functor": "=", "args": ["X", 2]}],
        [{"functor": "=", "args": ["X", 3]}]
    ]);
    let solutions = json.as_array().unwrap();
    let result = QueryResult::parse_solutions(solutions).unwrap();
    match result {
        QueryResult::Solutions(sols) => {
            assert_eq!(sols.len(), 3);
            for sol in &sols {
                assert_eq!(sol.len(), 1);
                assert!(sol.contains_key("X"));
            }
        }
        _ => panic!("Expected Solutions"),
    }
}

#[test]
fn test_prolog_json_helpers() {
    // Test is_prolog_atom
    assert!(is_prolog_atom(&json!("hello")));
    assert!(is_prolog_atom(&json!("")));
    assert!(!is_prolog_atom(&json!("X"))); // Variable
    assert!(!is_prolog_atom(&json!("_var"))); // Variable
    assert!(!is_prolog_atom(&json!(42)));

    // Test is_prolog_variable
    assert!(is_prolog_variable(&json!("X")));
    assert!(is_prolog_variable(&json!("Variable")));
    assert!(is_prolog_variable(&json!("_")));
    assert!(is_prolog_variable(&json!("_var")));
    assert!(!is_prolog_variable(&json!("hello")));
    assert!(!is_prolog_variable(&json!("")));

    // Test is_prolog_functor
    let functor = json!({"functor": "foo", "args": [1, 2]});
    assert!(is_prolog_functor(&functor));
    assert!(!is_prolog_functor(&json!("atom")));
    assert!(!is_prolog_functor(&json!([1, 2, 3])));

    // Test prolog_name
    assert_eq!(prolog_name(&json!("hello")), Some("hello"));
    assert_eq!(prolog_name(&json!("X")), Some("X"));
    let functor = json!({"functor": "foo", "args": []});
    assert_eq!(prolog_name(&functor), Some("foo"));
    assert_eq!(prolog_name(&json!(42)), None);

    // Test prolog_args
    let functor = json!({"functor": "foo", "args": [1, 2, 3]});
    let args = prolog_args(&functor).unwrap();
    assert_eq!(args.len(), 3);
    assert_eq!(prolog_args(&json!("atom")), None);
}

#[test]
fn test_error_display() {
    // Test various error types display correctly
    let io_err = PrologError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
    assert!(format!("{}", io_err).contains("I/O error"));

    let config_err = PrologError::ConfigError("missing port".to_string());
    assert_eq!(
        format!("{}", config_err),
        "Configuration error: missing port"
    );

    let auth_err = PrologError::AuthenticationFailed;
    assert_eq!(format!("{}", auth_err), "Authentication failed");

    let timeout_err = PrologError::Timeout;
    assert_eq!(format!("{}", timeout_err), "Query timed out");

    let prolog_ex = PrologError::PrologException {
        kind: "syntax_error".to_string(),
        term: Some(json!("unexpected")),
    };
    assert!(format!("{}", prolog_ex).contains("Prolog exception: syntax_error"));
}

#[test]
fn test_query_result_variants() {
    // Test Success variant
    let success_true = QueryResult::Success(true);
    assert!(matches!(success_true, QueryResult::Success(true)));

    let success_false = QueryResult::Success(false);
    assert!(matches!(success_false, QueryResult::Success(false)));

    // Test Solutions variant
    let mut solution = HashMap::new();
    solution.insert("X".to_string(), PrologTerm::Integer(42));
    let solutions = QueryResult::Solutions(vec![solution]);
    match solutions {
        QueryResult::Solutions(sols) => {
            assert_eq!(sols.len(), 1);
            assert!(sols[0].contains_key("X"));
        }
        _ => panic!("Expected Solutions"),
    }
}

#[test]
fn test_prolog_term_to_string() {
    use swipl_rs::types::prolog_term_to_string;

    // Test atoms
    assert_eq!(
        prolog_term_to_string(&PrologTerm::Atom("hello".to_string())),
        "hello"
    );
    assert_eq!(
        prolog_term_to_string(&PrologTerm::Atom("Hello".to_string())),
        "'Hello'"
    );
    assert_eq!(
        prolog_term_to_string(&PrologTerm::Atom("hello world".to_string())),
        "'hello world'"
    );

    // Test variables
    assert_eq!(
        prolog_term_to_string(&PrologTerm::Variable("X".to_string())),
        "X"
    );
    assert_eq!(
        prolog_term_to_string(&PrologTerm::Variable("_Var".to_string())),
        "_Var"
    );

    // Test numbers
    assert_eq!(prolog_term_to_string(&PrologTerm::Integer(42)), "42");
    assert_eq!(prolog_term_to_string(&PrologTerm::Float(3.14)), "3.14");

    // Test boolean
    assert_eq!(prolog_term_to_string(&PrologTerm::Bool(true)), "true");
    assert_eq!(prolog_term_to_string(&PrologTerm::Bool(false)), "false");

    // Test list
    let list = PrologTerm::List(vec![
        PrologTerm::Integer(1),
        PrologTerm::Integer(2),
        PrologTerm::Integer(3),
    ]);
    assert_eq!(prolog_term_to_string(&list), "[1, 2, 3]");

    // Test compound
    let compound = PrologTerm::Compound(PrologCompound {
        functor: "foo".to_string(),
        args: vec![PrologTerm::Atom("bar".to_string()), PrologTerm::Integer(42)],
    });
    assert_eq!(prolog_term_to_string(&compound), "foo(bar, 42)");
}

#[test]
fn test_connection_addr() {
    use swipl_rs::session::ConnectionAddr;

    // Test TCP address
    let tcp_addr = ConnectionAddr::Tcp("127.0.0.1".to_string(), 8080);
    match tcp_addr {
        ConnectionAddr::Tcp(host, port) => {
            assert_eq!(host, "127.0.0.1");
            assert_eq!(port, 8080);
        }
        #[cfg(feature = "unix-socket")]
        _ => panic!("Expected TCP address"),
    }

    // Test Unix domain socket (only when feature is enabled)
    #[cfg(feature = "unix-socket")]
    {
        use std::path::PathBuf;
        let uds_addr = ConnectionAddr::Uds(PathBuf::from("/tmp/test.sock"));
        match uds_addr {
            ConnectionAddr::Uds(path) => {
                assert_eq!(path, PathBuf::from("/tmp/test.sock"));
            }
            _ => panic!("Expected UDS address"),
        }
    }
}

#[test]
fn test_server_config_defaults() {
    use swipl_rs::ServerConfig;

    let config = ServerConfig::default();
    assert!(config.launch_mqi);
    assert_eq!(config.host, None);
    assert_eq!(config.port, None);
    assert_eq!(config.password, None);
    assert_eq!(config.unix_domain_socket, None);
    assert_eq!(config.query_timeout_seconds, None);
    assert_eq!(config.pending_connection_count, None);
    assert_eq!(config.output_file_name, None);
    assert_eq!(config.mqi_traces, None);
    assert_eq!(config.prolog_path, Some(std::path::PathBuf::from("swipl")));
    assert_eq!(config.prolog_path_args, None);
}

#[test]
fn test_error_conversion() {
    // Test that std::io::Error converts to PrologError
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let prolog_error: PrologError = io_error.into();
    assert!(matches!(prolog_error, PrologError::Io(_)));

    // Test that serde_json::Error converts to PrologError
    let json_str = "invalid json";
    let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
    let prolog_error: PrologError = json_error.into();
    assert!(matches!(prolog_error, PrologError::Json(_)));
}
