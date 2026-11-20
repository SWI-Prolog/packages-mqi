use std::io::{self, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex};

use log::{debug, error, info, trace, warn};
use serde_json::Value;

use crate::error::PrologError;
use crate::types::QueryResult;

// Use feature flags for Unix Domain Sockets
#[cfg(feature = "unix-socket")]
use std::os::unix::net::UnixStream;
#[cfg(feature = "unix-socket")]
use std::path::PathBuf;

/// Represents the type of connection address.
#[derive(Debug, Clone)]
pub enum ConnectionAddr {
    Tcp(String, u16), // Host and port number
    #[cfg(feature = "unix-socket")]
    Uds(PathBuf), // Path to socket file
}

/// Represents an active connection and query thread within the MQI server.
#[derive(Debug)]
pub struct PrologSession {
    // Use a trait object or enum to handle different stream types
    stream: Box<dyn ReadWriteShutdown>, // Custom trait for common socket ops
    connection_failed: Arc<Mutex<bool>>, // Shared flag with PrologServer
    _communication_thread_id: Option<String>, // Placeholder
    _goal_thread_id: Option<String>,    // Placeholder
    server_protocol_major: u32,
    server_protocol_minor: u32,
}

// Custom trait to unify socket operations needed
trait ReadWriteShutdown: Read + Write + Send + Sync + std::fmt::Debug {
    fn shutdown(&self, how: Shutdown) -> io::Result<()>;
    fn _set_read_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()>;
    fn _set_write_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()>;
}

impl ReadWriteShutdown for TcpStream {
    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        TcpStream::shutdown(self, how)
    }
    fn _set_read_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, dur)
    }
    fn _set_write_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        TcpStream::set_write_timeout(self, dur)
    }
}

#[cfg(feature = "unix-socket")]
impl ReadWriteShutdown for UnixStream {
    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        UnixStream::shutdown(self, how)
    }
    fn _set_read_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        UnixStream::set_read_timeout(self, dur)
    }
    fn _set_write_timeout(&self, dur: Option<std::time::Duration>) -> io::Result<()> {
        UnixStream::set_write_timeout(self, dur)
    }
}

impl PrologSession {
    /// Connects to a running SWI-Prolog MQI server.
    pub fn connect(
        addr: ConnectionAddr,
        password: &str,
        connection_failed_flag: Arc<Mutex<bool>>,
    ) -> Result<Self, PrologError> {
        // Create the stream based on the address type
        let mut stream: Box<dyn ReadWriteShutdown> = match addr {
            ConnectionAddr::Tcp(host, port) => {
                let addr_str = format!("{}:{}", host, port);
                let tcp_stream = TcpStream::connect(addr_str)?;
                // Set read/write timeouts?
                // tcp_stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                // tcp_stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                Box::new(tcp_stream)
            }
            #[cfg(feature = "unix-socket")]
            ConnectionAddr::Uds(path) => {
                let unix_stream = UnixStream::connect(path)?;
                Box::new(unix_stream)
            }
        };

        // Send password for authentication
        // Prolog expects the password string followed by ".\n"
        let password_with_suffix = format!("{}.\n", password);
        send_message(&mut *stream, &password_with_suffix)?;

        // Receive and parse the initial response
        let response_str = receive_message(&mut *stream)?;
        trace!("Connect response raw: {}", response_str);

        // Handle potential trailing newline from Prolog's term_to_json_string
        let response_json: Value = serde_json::from_str(response_str.trim_end())?;

        debug!("Initial response JSON: {}", response_json);

        // Parse initial response (true([[threads(CommId, GoalId), version(Major, Minor)]]))
        // Or just true([]) if older version or password failed.
        if response_json.get("functor").and_then(|f| f.as_str()) != Some("true") {
            // Check if it's an exception, specifically password mismatch
            if response_json.get("functor").and_then(|f| f.as_str()) == Some("exception") {
                if let Some(args) = response_json.get("args").and_then(|a| a.as_array()) {
                    if let Some(kind) = args.first().and_then(|k| k.as_str()) {
                        if kind == "password_mismatch" {
                            return Err(PrologError::AuthenticationFailed);
                        }
                    }
                }
            }
            return Err(PrologError::AuthenticationFailed); // Assume auth failure if not true(...)
        }

        let (comm_id, goal_id, major, minor) = Self::parse_initial_true_args(&response_json)?;

        let session = Self {
            stream,
            connection_failed: connection_failed_flag,
            _communication_thread_id: comm_id,
            _goal_thread_id: goal_id,
            server_protocol_major: major,
            server_protocol_minor: minor,
        };

        session.check_protocol_version()?;

        info!(
            "MQI session connected successfully. Server v{}.{}",
            major, minor
        );
        Ok(session)
    }

    // Renamed from parse_initial_response to be more specific
    fn parse_initial_true_args(
        json: &Value,
    ) -> Result<(Option<String>, Option<String>, u32, u32), PrologError> {
        // Expecting true([[threads(C, G), version(Ma, Mi)]]) or true([[]])
        if let Some(args) = json.get("args").and_then(|a| a.as_array()) {
            if args.len() == 1 {
                if let Some(outer_list) = args[0].as_array() {
                    if outer_list.is_empty() {
                        // true([[]]) case
                        return Ok((None, None, 0, 0)); // Pre-version info MQI
                    }
                    if let Some(inner_list) = outer_list[0].as_array() {
                        if let Some(first_element) = inner_list.first() {
                            // Check for threads/2
                            if let Some(comm_args) =
                                first_element.get("args").and_then(|a| a.as_array())
                            {
                                if first_element.get("functor").and_then(|f| f.as_str())
                                    == Some("threads")
                                    && comm_args.len() == 2
                                {
                                    let comm_id = comm_args[0].as_str().map(String::from);
                                    let goal_id = comm_args[1].as_str().map(String::from);

                                    // Check for version/2 (optional)
                                    if let Some(second_element) = inner_list.get(1) {
                                        if let Some(version_args) =
                                            second_element.get("args").and_then(|a| a.as_array())
                                        {
                                            if second_element
                                                .get("functor")
                                                .and_then(|f| f.as_str())
                                                == Some("version")
                                                && version_args.len() == 2
                                            {
                                                let major =
                                                    version_args[0].as_u64().ok_or_else(|| {
                                                        PrologError::InvalidState(
                                                            "Invalid version major number".into(),
                                                        )
                                                    })?
                                                        as u32;
                                                let minor =
                                                    version_args[1].as_u64().ok_or_else(|| {
                                                        PrologError::InvalidState(
                                                            "Invalid version minor number".into(),
                                                        )
                                                    })?
                                                        as u32;
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
        Err(PrologError::InvalidState(
            "Unexpected structure for initial 'true' response".into(),
        ))
    }

    fn check_protocol_version(&self) -> Result<(), PrologError> {
        // Client library requires MQI >= 1.0
        const REQUIRED_MAJOR: u32 = 1;
        const REQUIRED_MINOR: u32 = 0;

        // Version 0.0 had a protocol bug, but swiplserver works around it.
        // This Rust version *could* too, but let's mandate >= 1.0 for simplicity now.
        if self.server_protocol_major == 0 && self.server_protocol_minor == 0 {
            warn!(
                "Server is MQI v0.0 which has known protocol issues. Compatibility not guaranteed."
            );
            // For now, allow 0.0 but warn. Could return error here instead.
            // return Err(PrologError::VersionMismatch { ... });
            return Ok(());
        }

        if self.server_protocol_major == REQUIRED_MAJOR {
            Ok(())
        } else {
            Err(PrologError::VersionMismatch {
                client: format!("{}.{}", REQUIRED_MAJOR, REQUIRED_MINOR),
                server: format!(
                    "{}.{}",
                    self.server_protocol_major, self.server_protocol_minor
                ),
            })
        }
    }

    /// Executes a query synchronously, waiting for all results (like findall/3).
    pub fn query(
        &mut self,
        goal: &str,
        timeout_seconds: Option<f64>,
    ) -> Result<QueryResult, PrologError> {
        let goal = goal.trim().trim_end_matches('.');
        let timeout_str = timeout_seconds.map_or_else(|| "_".to_string(), |t| t.to_string());
        let command = format!("run(({}), {}).", goal, timeout_str);
        send_message(&mut *self.stream, &command)?;
        self.handle_response()
    }

    /// Starts a query asynchronously.
    pub fn query_async(
        &mut self,
        goal: &str,
        find_all: bool,
        timeout_seconds: Option<f64>,
    ) -> Result<(), PrologError> {
        let goal = goal.trim().trim_end_matches('.');
        let timeout_str = timeout_seconds.map_or_else(|| "_".to_string(), |t| t.to_string());
        let find_all_str = if find_all { "true" } else { "false" };
        let command = format!("run_async(({}), {}, {}).", goal, timeout_str, find_all_str);
        send_message(&mut *self.stream, &command)?;
        match self.handle_response()? {
            // run_async returns true([[[]]]) when successful - one empty solution
            QueryResult::Solutions(ref sols) if sols.len() == 1 && sols[0].is_empty() => Ok(()),
            QueryResult::Success(true) => Ok(()), // For compatibility
            _ => Err(PrologError::InvalidState(
                "Unexpected response from run_async".to_string(),
            )),
        }
    }

    /// Retrieves the next result from an asynchronous query.
    pub fn query_async_result(
        &mut self,
        wait_timeout_seconds: Option<f64>,
    ) -> Result<Option<QueryResult>, PrologError> {
        let timeout_str = wait_timeout_seconds.map_or_else(|| "-1".to_string(), |t| t.to_string());
        let command = format!("async_result({}).", timeout_str);
        send_message(&mut *self.stream, &command)?;
        match self.handle_response() {
            Ok(result) => Ok(Some(result)),
            Err(PrologError::PrologException { kind, .. }) if kind == "no_more_results" => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Attempts to cancel the currently running asynchronous query.
    pub fn cancel_async(&mut self) -> Result<(), PrologError> {
        let command = "cancel_async.";
        send_message(&mut *self.stream, command)?;
        match self.handle_response()? {
            QueryResult::Success(true) => Ok(()),
            QueryResult::Solutions(ref sols) if sols.len() == 1 && sols[0].is_empty() => Ok(()),
            _ => Err(PrologError::InvalidState(
                "Unexpected response from cancel_async".to_string(),
            )),
        }
    }

    /// Sends the `close.` command to the server to cleanly end this session.
    pub fn close(&mut self) -> Result<(), PrologError> {
        debug!("Closing MQI session...");
        let command = "close.";
        if let Err(e) = send_message(&mut *self.stream, command) {
            warn!(
                "Error sending close command (connection might already be closed): {}",
                e
            );
            // Continue to shutdown socket anyway
        } else {
            // Try to read the acknowledgment, but don't error if it fails
            match self.handle_response() {
                Ok(_) => debug!("Received close acknowledgment."),
                Err(e) => warn!("Error receiving close acknowledgment: {}", e),
            }
        }

        // Shutdown write side first
        let _ = self
            .stream
            .shutdown(Shutdown::Write)
            .map_err(|e| warn!("Error shutting down socket write side: {}", e));
        // Maybe read remaining data?
        // let _ = self.stream.read_to_end(&mut Vec::new());
        let _ = self
            .stream
            .shutdown(Shutdown::Both)
            .map_err(|e| warn!("Error shutting down socket both sides: {}", e));
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
            _ => Err(PrologError::InvalidState(
                "Unexpected response from quit".to_string(),
            )),
        }
    }

    /// Handles receiving and parsing a response from the MQI server.
    fn handle_response(&mut self) -> Result<QueryResult, PrologError> {
        let response_str = receive_message(&mut *self.stream)?; // Can throw Io error

        // Check for simple "false" response for query failure
        let trimmed_response = response_str.trim();
        if trimmed_response == "\"false\"" {
            // Prolog sends "false" including quotes
            return Ok(QueryResult::Success(false));
        }

        let response_json: Value = serde_json::from_str(trimmed_response)?; // Parse the trimmed string
        trace!("Received JSON: {}", response_json);

        match response_json.get("functor").and_then(|f| f.as_str()) {
            Some("true") => {
                let args = response_json.get("args").and_then(|a| a.as_array());
                match args {
                    Some(outer_list) if outer_list.len() == 1 => {
                        let solutions = outer_list[0].as_array().ok_or_else(|| {
                            PrologError::InvalidState(
                                "Expected list of solutions in 'true' response".into(),
                            )
                        })?;
                        if solutions.is_empty() {
                            Ok(QueryResult::Success(true)) // true([]) -> Simple success
                        } else {
                            QueryResult::parse_solutions(solutions) // true([[...], [...]])
                        }
                    }
                    _ => Err(PrologError::InvalidState(
                        "Unexpected structure for 'true' response".into(),
                    )),
                }
            }
            Some("false") => Ok(QueryResult::Success(false)),
            Some("exception") => {
                let args = response_json.get("args").and_then(|a| a.as_array());
                match args {
                    Some(ex_arg) if ex_arg.len() == 1 => {
                        let ex_term = ex_arg[0].clone();
                        let kind = if let Some(simple_str) = ex_term.as_str() {
                            simple_str.to_string()
                        } else if let Some(functor) =
                            ex_term.get("functor").and_then(|f| f.as_str())
                        {
                            // For compound exceptions like syntax_error(operator_expected)
                            functor.to_string()
                        } else {
                            "complex_exception".to_string()
                        };
                        error!("Received Prolog exception: {}", kind);

                        // Map specific Prolog errors to specific Rust errors
                        let err = match kind.as_str() {
                            "connection_failed" => PrologError::ConnectionFailed(
                                "Server reported connection failure".into(),
                            ),
                            "time_limit_exceeded" => PrologError::Timeout,
                            "no_query" => PrologError::NoQuery,
                            "cancel_goal" => PrologError::QueryCancelled,
                            "result_not_available" => PrologError::ResultNotAvailable,
                            _ => PrologError::PrologException {
                                kind,
                                term: Some(ex_term),
                            },
                        };

                        if matches!(err, PrologError::ConnectionFailed(_)) {
                            *self.connection_failed.lock().unwrap() = true;
                        }
                        Err(err)
                    }
                    _ => Err(PrologError::InvalidState(
                        "Unexpected structure for 'exception' response".into(),
                    )),
                }
            }
            _ => Err(PrologError::InvalidState(format!(
                "Unknown response structure: {}",
                response_str
            ))),
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

/// Sends a properly formatted message (length prefix + message) to the MQI server.
fn send_message<W: Write + ?Sized>(stream: &mut W, message: &str) -> Result<(), PrologError> {
    debug!("[SEND] Sending message text: {}", message);
    let bytes = message.as_bytes();
    let len = bytes.len();
    let len_str = format!("{}.\n", len);
    let len_bytes = len_str.as_bytes();

    debug!(
        "[SEND] Length prefix bytes ({}) Hex: {:02X?}",
        len_str.trim_end(),
        len_bytes
    );
    // Write length prefix first
    stream.write_all(len_bytes)?;

    debug!(
        "[SEND] Message body bytes ({}) Hex: {:02X?}",
        message, bytes
    );
    // Then write the actual message
    stream.write_all(bytes)?;
    stream.flush()?; // Ensure the message is sent immediately
    debug!("[SEND] Message sent successfully.");
    Ok(())
}

/// Receives a properly formatted message (length prefix + message) from the MQI server.
fn receive_message<R: Read + ?Sized>(stream: &mut R) -> Result<String, PrologError> {
    debug!("[RECV] Attempting to receive message...");
    // Use BufReader for potentially better performance, but read byte-by-byte for delimiter handling
    let mut reader = BufReader::new(stream);
    let mut len_bytes = Vec::new();
    let mut raw_len_prefix_bytes = Vec::new(); // For logging raw bytes read
    let mut byte = [0; 1];

    // Read bytes until '.' is found
    debug!("[RECV] Reading length prefix...");
    loop {
        match reader.read_exact(&mut byte) {
            Ok(_) => raw_len_prefix_bytes.push(byte[0]),
            Err(e) => {
                error!(
                    "[RECV] Error reading length byte: {}. Raw prefix read so far: {:02X?}",
                    e, raw_len_prefix_bytes
                );
                return Err(e.into());
            }
        }

        let current_byte = byte[0];
        if current_byte == b'.' {
            // If we haven't read any digits yet, this might be a lone heartbeat.
            if len_bytes.is_empty() {
                trace!("[RECV] Read single '.' - likely heartbeat. Discarding and continuing.");
                raw_len_prefix_bytes.clear(); // Reset raw log for next attempt
                continue; // Read the next byte
            } else {
                // Found the end of the length prefix
                break;
            }
        } else if current_byte.is_ascii_digit() {
            len_bytes.push(current_byte);
        } else if current_byte == b'\r' || current_byte == b'\n' {
            // Ignore potential CR/LF in length part (unlikely but possible)
            trace!(
                "[RECV] Ignored CR/LF ({:02X?}) during length prefix read.",
                current_byte
            );
            continue;
        } else {
            // Received unexpected non-digit, non-delimiter byte.
            // Could be a heartbeat if len_bytes is empty, or an error.
            if len_bytes.is_empty() {
                trace!("[RECV] Read non-digit/non-delimiter byte ({:02X?}) before length - discarding as likely heartbeat/noise.", current_byte);
                raw_len_prefix_bytes.clear(); // Reset raw log
                continue; // Read the next byte
            } else {
                error!(
                    "[RECV] Invalid char in length prefix: {}. Raw prefix read: {:02X?}",
                    current_byte, raw_len_prefix_bytes
                );
                return Err(PrologError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Invalid character in message length prefix: {}",
                        current_byte
                    ),
                )));
            }
        }
    }
    debug!(
        "[RECV] Raw length prefix bytes read (including '.'): {:02X?}",
        raw_len_prefix_bytes
    );

    // Consume the newline character(s) after the '.'
    let mut nl_bytes_read = Vec::new();
    match reader.read_exact(&mut byte) {
        Ok(_) => nl_bytes_read.push(byte[0]),
        Err(e) => {
            error!(
                "[RECV] Error reading byte after '.': {}. Raw prefix read: {:02X?}",
                e, raw_len_prefix_bytes
            );
            return Err(e.into());
        }
    }

    if byte[0] == b'\r' {
        // Handle potential CRLF
        // If it was CR, try to read the LF
        match reader.read_exact(&mut byte) {
            Ok(_) => nl_bytes_read.push(byte[0]),
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                // EOF after CR is acceptable if previous read consumed LF implicitly
                debug!("[RECV] EOF encountered after CR, assuming implicit LF consumed.");
            }
            Err(e) => {
                error!(
                    "[RECV] Error reading potential LF after CR: {}. NL bytes read: {:02X?}",
                    e, nl_bytes_read
                );
                return Err(e.into()); // Other errors are fatal
            }
        }

        if nl_bytes_read.len() > 1 && nl_bytes_read[1] != b'\n' {
            // If we read something but it wasn't LF, that's unexpected
            error!(
                "[RECV] Expected LF after CR, got: {:02X?}. NL bytes read: {:02X?}",
                nl_bytes_read.get(1),
                nl_bytes_read
            );
            return Err(PrologError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Expected LF after CR in length delimiter",
            )));
        }
    } else if byte[0] != b'\n' {
        // If it wasn't CR, it must be LF
        error!(
            "[RECV] Expected LF after '.', got: {:02X?}. NL bytes read: {:02X?}",
            byte[0], nl_bytes_read
        );
        return Err(PrologError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Expected LF after length delimiter",
        )));
    }
    debug!(
        "[RECV] Newline bytes consumed after '.': {:02X?}",
        nl_bytes_read
    );

    // Parse the length string
    let len_str = String::from_utf8(len_bytes.clone()).map_err(|_| {
        error!(
            "[RECV] Length prefix bytes are not valid UTF-8: {:02X?}",
            len_bytes
        );
        PrologError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Length prefix is not valid UTF-8",
        ))
    })?;
    let len = len_str.parse::<usize>().map_err(|_| {
        error!(
            "[RECV] Failed to parse message length from string: '{}' (bytes: {:02X?})",
            len_str, len_bytes
        );
        PrologError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to parse message length: '{}'", len_str),
        ))
    })?;
    debug!("[RECV] Parsed message body length: {}", len);

    // Read the exact number of bytes for the message payload
    debug!("[RECV] Reading message body ({} bytes)...", len);
    let mut message_buf = vec![0; len];
    match reader.read_exact(&mut message_buf) {
        Ok(_) => debug!("[RECV] Successfully read {} bytes for message body.", len),
        Err(e) => {
            error!(
                "[RECV] Error reading message body (expected {} bytes): {}",
                len, e
            );
            return Err(e.into());
        }
    }
    debug!("[RECV] Message body bytes read: {:02X?}", message_buf);

    // Convert bytes to String (assuming UTF-8)
    let message_str = String::from_utf8(message_buf).map_err(|e| {
        error!("[RECV] Failed to decode message body as UTF-8: {}", e);
        PrologError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to decode UTF-8 message: {}", e),
        ))
    })?;
    debug!("[RECV] Decoded message string: {}", message_str);
    debug!("[RECV] Message received successfully.");
    Ok(message_str)
}
