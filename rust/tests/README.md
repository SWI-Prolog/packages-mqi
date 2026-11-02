# MQI Rust Implementation Tests

This directory contains comprehensive tests for the Rust implementation of the SWI-Prolog Machine Query Interface (MQI).

## Test Organization

### Integration Tests (`integration_tests.rs`)
Full end-to-end tests that:
- Start actual SWI-Prolog MQI server processes
- Test real client-server communication
- Verify query execution and result handling
- Test error conditions and recovery
- Require SWI-Prolog to be installed

### Unit Tests (`unit_tests.rs`)
Tests for individual components:
- PrologTerm serialization/deserialization
- QueryResult parsing
- Error type conversions
- Helper functions
- Type safety

### Protocol Tests (`protocol_tests.rs`)
Tests specific to the MQI protocol:
- Message format parsing
- Command formatting
- Response handling
- Protocol version compatibility
- Special cases and edge conditions

## Running Tests

### Prerequisites
- Rust toolchain (rustc, cargo)
- SWI-Prolog installed and accessible in PATH
- Environment variable `RUST_LOG=debug` for detailed output (optional)

### Run all tests:
```bash
cargo test
```

### Run specific test file:
```bash
cargo test --test integration_tests
cargo test --test unit_tests
cargo test --test protocol_tests
```

### Run with logging:
```bash
RUST_LOG=debug cargo test -- --nocapture
```

### Run a specific test:
```bash
cargo test test_simple_query -- --exact
```

## Test Coverage

The tests cover:

1. **Server Management**
   - Starting/stopping servers
   - Configuration options
   - Multiple server instances
   - Error handling

2. **Connection Handling**
   - TCP/IP connections
   - Unix Domain Sockets (when feature enabled)
   - Authentication
   - Multiple concurrent sessions

3. **Query Execution**
   - Synchronous queries
   - Asynchronous queries
   - Query timeouts
   - Query cancellation
   - Multiple solutions

4. **Data Types**
   - Atoms, variables, numbers
   - Lists and compound terms
   - Special values (empty list, anonymous variables)
   - Constraint residuals

5. **Error Handling**
   - Syntax errors
   - Runtime exceptions
   - Connection failures
   - Timeout handling

6. **Protocol Compliance**
   - Message format validation
   - Version negotiation
   - Heartbeat handling
   - Proper cleanup

## Feature Flags

Tests may behave differently based on enabled features:
- `unix-socket`: Enables Unix Domain Socket tests
- `password-gen`: Tests automatic password generation

## Adding New Tests

When adding new tests:
1. Use descriptive test names
2. Add appropriate `#[cfg(...)]` guards for feature-specific tests
3. Use `require_swipl()` for tests that need SWI-Prolog
4. Clean up resources (sessions, servers) properly
5. Document any special requirements

## Troubleshooting

### Tests fail with "SWI-Prolog not found"
- Ensure `swipl` is in your PATH
- Install SWI-Prolog from https://www.swi-prolog.org/

### Connection failures
- Check if port 9999 is available (for custom port test)
- Ensure no other MQI servers are running
- Try with `RUST_LOG=debug` for more information

### Unix socket tests fail
- Ensure the `unix-socket` feature is enabled
- Check permissions on `/tmp` directory
- Verify platform support (Unix-like systems only)