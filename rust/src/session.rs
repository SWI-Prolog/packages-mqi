use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use log::{debug, error, trace, warn};
use serde_json::Value;

use crate::error::PrologError;
use crate::types::{PrologTerm, QueryResult};

// Use feature flags for Unix Domain Sockets
#[cfg(feature = "unix-socket")]
use std::os::unix::net::UnixStream;

/// Represents the type of connection address.
#[derive(Debug, Clone)]
pub enum ConnectionAddr {
    Tcp(u16), // Port number
    #[cfg(feature = "unix-socket")]
    Uds(PathBuf), // Path to socket file
}

/// Represents an active connection and query thread within the MQI server.
#[derive(Debug)]
pub struct PrologSession {
    // Use a trait object or enum to handle different stream types
    stream: Box<dyn ReadWriteShutdown>, // Custom trait for common socket ops
    connection_failed: Arc<Mutex<bool>>, // Shared flag with PrologServer
    communication_thread_id: Option<String>, // Placeholder
    goal_thread_id: Option<String>,          // Placeholder
    server_protocol_major: u32,
    server_protocol_minor: u32,
}

// Custom trait to unify socket operations needed
trait ReadWriteShutdown: Read + Write + Send + Sync + std::fmt::Debug {
    fn shutdown(&self, how: Shutdown) -> io::Result<()>;
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
}

impl ReadWriteShutdown for TcpStream {
    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        TcpStream::shutdown(self, how)
    }
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, dur)
    }
     fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        TcpStream::set_write_timeout(self, dur)
    }
}

#[cfg(feature = "unix-socket")]
impl ReadWriteShutdown for UnixStream {
    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        UnixStream::shutdown(self, how)
    }
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        UnixStream::set_read_timeout(self, dur)
    }
     fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        UnixStream::set_write_timeout(self, dur)
    }
}

impl PrologSession {
    /// Connects to the MQI server at the specified address.
    pub(crate) fn connect(
        addr: ConnectionAddr,
        password: &str,
        connection_failed_flag: Arc<Mutex<bool>>,
    ) -> Result<Self, PrologError> {
        debug!("Connecting to Prolog MQI at {:?}...", addr);

        let stream: Box<dyn ReadWriteShutdown> = match addr {
            ConnectionAddr::Tcp(port) => {
                // Add connection retry logic like in Python?
                let tcp_stream = TcpStream::connect(("127.0.0.1", port))?;
                // TODO: Consider setting TCP_NODELAY or keepalive options?
                Box::new(tcp_stream)
            }
            #[cfg(feature = "unix-socket")]
            ConnectionAddr::Uds(ref path) => { // Use ref path here
                let unix_stream = UnixStream::connect(path)?;
                Box::new(unix_stream)
            }
            #[cfg(not(feature = "unix-socket"))]
             _ => return Err(PrologError::FeatureNotEnabled("unix-socket".to_string())),
        };

        // Send password immediately
        // The password string from Prolog already includes the trailing .\n
        // The send_message helper expects a plain string without trailing .
        // Let's adjust send_message or handle it here.
        // For now, assuming send_message adds the .
        send_message(&mut *stream, password)?;

        // Receive initial response
        let response_str = receive_message(&mut *stream)?;
        // Handle potential trailing newline from Prolog's term_to_json_string
        let response_json: Value = serde_json::from_str(response_str.trim_end())?;

        debug!("Initial response JSON: {}", response_json);

        // Parse initial response (true([[threads(CommId, GoalId), version(Major, Minor)]]))
        // Or just true([]) if older version or password failed.
        if !response_json.get("functor").and_then(|f| f.as_str()).map_or(false, |f| f == "true") {
            // Check if it's an exception, specifically password mismatch
            if response_json.get("functor").and_then(|f| f.as_str()) == Some("exception") {
                 if let Some(args) = response_json.get("args").and_then(|a| a.as_array()) {
                     if let Some(kind) = args.get(0).and_then(|k| k.as_str()) {
                         if kind == "password_mismatch" {
                             return Err(PrologError::AuthenticationFailed);
                         }
                     }
                 }
            }
            return Err(PrologError::AuthenticationFailed); // Assume auth failure if not true(...)
        }

        let (comm_id, goal_id, major, minor) = Self::parse_initial_true_args(&response_json)?;

        let mut session = Self {
            stream,
            connection_failed: connection_failed_flag,
            communication_thread_id: comm_id,
            goal_thread_id: goal_id,
            server_protocol_major: major,
            server_protocol_minor: minor,
        };

        session.check_protocol_version()?;

        info!("MQI session connected successfully. Server v{}.{}", major, minor);
        Ok(session)
    }

    // Renamed from parse_initial_response to be more specific
    fn parse_initial_true_args(json: &Value) -> Result<(Option<String>, Option<String>, u32, u32), PrologError> {
        // Expecting true([[threads(C, G), version(Ma, Mi)]]) or true([[]])
         if let Some(args) = json.get("args").and_then(|a| a.as_array()) {
            if args.len() == 1 {
                if let Some(outer_list) = args[0].as_array() {
                     if outer_list.is_empty() { // true([[]]) case
                         return Ok((None, None, 0, 0)); // Pre-version info MQI
                     }
                    if let Some(inner_list) = outer_list[0].as_array() {
                        if let Some(first_element) = inner_list.get(0) {
                            // Check for threads/2
                            if let Some(comm_args) = first_element.get("args").and_then(|a| a.as_array()) {
                                if first_element.get("functor").and_then(|f| f.as_str()) == Some("threads") && comm_args.len() == 2 {
                                    let comm_id = comm_args[0].as_str().map(String::from);
                                    let goal_id = comm_args[1].as_str().map(String::from);

                                    // Check for version/2 (optional)
                                    if let Some(second_element) = inner_list.get(1) {
                                        if let Some(version_args) = second_element.get("args").and_then(|a| a.as_array()) {
                                            if second_element.get("functor").and_then(|f| f.as_str()) == Some("version") && version_args.len() == 2 {
                                                let major = version_args[0].as_u64().ok_or_else(|| PrologError::InvalidState("Invalid version major number".into()))? as u32;
                                                let minor = version_args[1].as_u64().ok_or_else(|| PrologError::InvalidState("Invalid version minor number".into()))? as u32;
                                                return Ok((comm_id, goal_id, major, minor));
                                            }
                                        }
                                    }
                                    // No version info, assume 0.0
                                    return Ok((comm_id, goal_id, 0, 0));
                                }
                            }
                        }
                    }
                }
            }
         }
         // If structure doesn't match, assume something went wrong post-authentication
         Err(PrologError::InvalidState("Unexpected structure for initial 'true' response".into()))
    }

     fn check_protocol_version(&self) -> Result<(), PrologError> {
        // Client library requires MQI >= 1.0
        const REQUIRED_MAJOR: u32 = 1;
        const REQUIRED_MINOR: u32 = 0;

        // Version 0.0 had a protocol bug, but swiplserver works around it.
        // This Rust version *could* too, but let's mandate >= 1.0 for simplicity now.
        if self.server_protocol_major == 0 && self.server_protocol_minor == 0 {
            warn!("Server is MQI v0.0 which has known protocol issues. Compatibility not guaranteed.");
            // For now, allow 0.0 but warn. Could return error here instead.
            // return Err(PrologError::VersionMismatch { ... });
            return Ok(());
        }

        if self.server_protocol_major == REQUIRED_MAJOR && self.server_protocol_minor >= REQUIRED_MINOR {
            Ok(())
        } else {
             Err(PrologError::VersionMismatch {
                client: format!("{}.{}", REQUIRED_MAJOR, REQUIRED_MINOR),
                server: format!("{}.{}", self.server_protocol_major, self.server_protocol_minor),
            })
        }
    }


    /// Executes a query synchronously, waiting for all results (like findall/3).
    pub fn query(&mut self, goal: &str, timeout_seconds: Option<f64>) -> Result<QueryResult, PrologError> {
        let goal = goal.trim().trim_end_matches('.');
        let timeout_str = timeout_seconds.map_or_else(|| "_".to_string(), |t| t.to_string());
        let command = format!("run(({}), {}).", goal, timeout_str);
        send_message(&mut *self.stream, &command)?;
        self.handle_response()
    }

    /// Starts a query asynchronously.
    pub fn query_async(&mut self, goal: &str, find_all: bool, timeout_seconds: Option<f64>) -> Result<(), PrologError> {
         let goal = goal.trim().trim_end_matches('.');
         let timeout_str = timeout_seconds.map_or_else(|| "_".to_string(), |t| t.to_string());
         let find_all_str = if find_all { "true" } else { "false" };
         let command = format!("run_async(({}), {}, {}).", goal, timeout_str, find_all_str);
         send_message(&mut *self.stream, &command)?;
         match self.handle_response()? {
             // Expect simple true acknowledgment
             QueryResult::Success(true) => Ok(()),
             _ => Err(PrologError::InvalidState("Unexpected response from run_async".to_string())),
         }
    }

    /// Retrieves the next result from an asynchronous query.
    pub fn query_async_result(&mut self, wait_timeout_seconds: Option<f64>) -> Result<Option<QueryResult>, PrologError> {
        let timeout_str = wait_timeout_seconds.map_or_else(|| "-1".to_string(), |t| t.to_string());
        let command = format!("async_result({}).", timeout_str);
        send_message(&mut *self.stream, &command)?;
        match self.handle_response() {
            Ok(result) => Ok(Some(result)),
            Err(PrologError::PrologException{ kind, .. }) if kind == "no_more_results" => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Attempts to cancel the currently running asynchronous query.
    pub fn cancel_async(&mut self) -> Result<(), PrologError> {
        let command = "cancel_async.";
        send_message(&mut *self.stream, command)?;
         match self.handle_response()? {
             QueryResult::Success(true) => Ok(()),
             _ => Err(PrologError::InvalidState("Unexpected response from cancel_async".to_string())),
         }
    }

    /// Sends the `close.` command to the server to cleanly end this session.
    pub fn close(&mut self) -> Result<(), PrologError> {
        debug!("Closing MQI session...");
        let command = "close.";
        if let Err(e) = send_message(&mut *self.stream, command) {
            warn!("Error sending close command (connection might already be closed): {}", e);
            // Continue to shutdown socket anyway
        } else {
            // Try to read the acknowledgment, but don't error if it fails
            match self.handle_response() {
                Ok(_) => debug!("Received close acknowledgment."),
                Err(e) => warn!("Error receiving close acknowledgment: {}", e),
            }
        }

        // Shutdown write side first
        let _ = self.stream.shutdown(Shutdown::Write).map_err(|e| warn!("Error shutting down socket write side: {}", e));
        // Maybe read remaining data?
        // let _ = self.stream.read_to_end(&mut Vec::new());
        let _ = self.stream.shutdown(Shutdown::Both).map_err(|e| warn!("Error shutting down socket both sides: {}", e));
        info!("MQI session closed.");
        Ok(())
    }

    /// Internal function called by Server Drop to send quit.
    pub(crate) fn halt_server_internal(&mut self) -> Result<(), PrologError> {
        let command = "quit.";
        send_message(&mut *self.stream, command)?;
        match self.handle_response()? {
             QueryResult::Success(true) => {
                *self.connection_failed.lock().unwrap() = true; // Mark connection as intentionally down
                 Ok(())
             }
             _ => Err(PrologError::InvalidState("Unexpected response from quit".to_string())),
         }
    }

    /// Handles receiving and parsing a response from the MQI server.
    fn handle_response(&mut self) -> Result<QueryResult, PrologError> {
        let response_str = receive_message(&mut *self.stream)?; // Can throw Io error
        let response_json: Value = serde_json::from_str(&response_str)?; // Can throw Json error
        trace!("Received JSON: {}", response_json);

        match response_json.get("functor").and_then(|f| f.as_str()) {
            Some("true") => {
                let args = response_json.get("args").and_then(|a| a.as_array());
                match args {
                    Some(outer_list) if outer_list.len() == 1 => {
                        let solutions = outer_list[0].as_array().ok_or_else(|| PrologError::InvalidState("Expected list of solutions in 'true' response".into()))?;
                        if solutions.is_empty() {
                            Ok(QueryResult::Success(true)) // true([]) -> Simple success
                        } else {
                            QueryResult::parse_solutions(solutions) // true([[...], [...]])
                        }
                    }
                    _ => Err(PrologError::InvalidState("Unexpected structure for 'true' response".into()))
                }
            }
            Some("false") => Ok(QueryResult::Success(false)),
            Some("exception") => {
                 let args = response_json.get("args").and_then(|a| a.as_array());
                 match args {
                     Some(ex_arg) if ex_arg.len() == 1 => {
                         let ex_term = ex_arg[0].clone();
                         let kind = ex_term.as_str().unwrap_or("complex_exception").to_string();
                         error!("Received Prolog exception: {}", kind);

                         // Map specific Prolog errors to specific Rust errors
                         let err = match kind.as_str() {
                             "connection_failed" => PrologError::ConnectionFailed("Server reported connection failure".into()),
                             "time_limit_exceeded" => PrologError::Timeout,
                             "no_query" => PrologError::NoQuery,
                             "cancel_goal" => PrologError::QueryCancelled,
                             "result_not_available" => PrologError::ResultNotAvailable,
                             _ => PrologError::PrologException { kind, term: Some(ex_term) }
                         };

                         if matches!(err, PrologError::ConnectionFailed(_)) {
                            *self.connection_failed.lock().unwrap() = true;
                         }
                         Err(err)
                     }
                     _ => Err(PrologError::InvalidState("Unexpected structure for 'exception' response".into()))
                 }
            }
            _ => Err(PrologError::InvalidState(format!("Unknown response structure: {}", response_str))),
        }
    }
}

impl Drop for PrologSession {
    fn drop(&mut self) {
        // Avoid double-closing if connection is already marked as failed (e.g., by halt_server)
        if !*self.connection_failed.lock().unwrap() {
            debug!("PrologSession dropped, ensuring connection is closed.");
            if let Err(e) = self.close() {
                warn!("Error closing session during drop: {}", e);
            }
        }
    }
}

// --- Communication Helpers ---

/// Sends a message according to the MQI protocol.
fn send_message<W: Write>(stream: &mut W, message: &str) -> Result<(), PrologError> {
    // Ensure message ends with exactly one '.\n'
    let msg = message.trim().trim_end_matches('.').to_string() + ".\n";
    trace!("Sending: {}", msg.trim_end());
    let msg_bytes = msg.as_bytes(); // MQI v1.0 uses UTF-8 byte length
    let header = format!("{}.\n", msg_bytes.len());

    stream.write_all(header.as_bytes())?;
    stream.write_all(msg_bytes)?;
    stream.flush()?;
    Ok(())
}

/// Receives a message according to the MQI protocol.
fn receive_message<R: Read>(stream: &mut R) -> Result<String, PrologError> {
    let mut size_buf = Vec::new();
    let mut byte_buf = [0u8; 1];
    let mut heartbeat_count = 0;
    let mut saw_dot = false;

    // Read the size header: <digits>.<newline>
    loop {
        // TODO: Add read timeout logic here?
        stream.read_exact(&mut byte_buf)?;
        match byte_buf[0] {
            b'\n' => {
                if saw_dot { break; } // Correct termination
                else { return Err(PrologError::InvalidState("Unexpected newline in size header before dot".into())); }
            },
            b'.' => {
                if size_buf.is_empty() && !saw_dot { // Only treat leading dots as heartbeats
                     heartbeat_count += 1;
                     trace!("Received heartbeat");
                } else if !saw_dot { // First dot after digits
                    saw_dot = true;
                } else { // Multiple dots - error
                    return Err(PrologError::InvalidState("Multiple dots found in size header".into()));
                }
            },
            digit if digit.is_ascii_digit() => {
                if saw_dot {
                    return Err(PrologError::InvalidState("Digit received after dot in size header".into()));
                }
                size_buf.push(digit)
            },
            other => return Err(PrologError::InvalidState(format!("Unexpected character in size header: {}", other as char))),
        }
    }

    if !saw_dot {
        return Err(PrologError::InvalidState("Size header did not contain a dot separator".into()));
    }

    let size_str = String::from_utf8(size_buf)
        .map_err(|_| PrologError::InvalidState("Invalid UTF-8 in size header".into()))?;
    let size: usize = size_str
        .parse()
        .map_err(|_| PrologError::InvalidState(format!("Invalid number in size header: \"{}\"", size_str)))?;

    trace!("Expecting {} bytes (after {} heartbeats)", size, heartbeat_count);

    // Read the message body
    let mut msg_buf = vec![0u8; size];
    stream.read_exact(&mut msg_buf)?;

    let msg_str = String::from_utf8(msg_buf)
        .map_err(|_| PrologError::InvalidState("Invalid UTF-8 in message body".into()))?;

    // The message from Prolog includes the trailing ".\n"
    trace!("Received raw: {:?}", msg_str);

    // We should return the string *without* the trailing .
    if !msg_str.ends_with(".\n") {
        // This indicates a framing error or protocol violation from the server
        warn!("Received message did not end with expected .\n");
        // Depending on strictness, could return error or try to recover
        // return Err(PrologError::InvalidState("Received message frame error".into()));
    }

    // Return the string content, trimming the trailing .
    Ok(msg_str.trim_end_matches(".\n").to_string())
} 