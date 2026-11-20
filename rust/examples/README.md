# SWI-Prolog Rust MQI Examples

This directory contains example programs demonstrating how to use the `swipl-rs` library to interact with SWI-Prolog through the Machine Query Interface (MQI).

## Prerequisites

- SWI-Prolog must be installed and available in your PATH
- Rust toolchain

## Running Examples

To run any example:

```bash
cargo run --example <example_name>
```

## Available Examples

### basic_usage
Simple demonstration of connecting to Prolog and running basic queries.
```bash
cargo run --example basic_usage
```

### async_queries
Shows how to use asynchronous queries to retrieve results one at a time.
```bash
cargo run --example async_queries
```

### knowledge_base
Demonstrates building a knowledge base with facts and rules, then querying it.
```bash
cargo run --example knowledge_base
```

### error_handling
Shows how to handle various types of errors that can occur when interacting with Prolog.
```bash
cargo run --example error_handling
```

### benchmark
Performance benchmarking of different types of queries.
```bash
cargo run --example benchmark
```

### concurrent_sessions
Demonstrates using multiple Prolog sessions concurrently from different threads.
```bash
cargo run --example concurrent_sessions
```

## Notes

- All examples check for SWI-Prolog availability before running
- If SWI-Prolog is not found, the examples will exit with an error message
- The examples use the default server configuration, which uses TCP/IP communication