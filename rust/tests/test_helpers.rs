use swipl_rs::{PrologServer, ServerConfig, PrologSession, PrologError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Condvar};
use std::time::{Duration, Instant};
use std::net::TcpListener;
use std::thread;
use log::debug;

#[cfg(feature = "unix-socket")]
use std::path::PathBuf;

/// Get a free port by binding to port 0
pub fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to port 0");
    let port = listener.local_addr()
        .expect("Failed to get local address")
        .port();
    drop(listener);
    
    // Give the OS a moment to fully release the port
    thread::sleep(Duration::from_millis(10));
    
    port
}

/// Get a unique Unix domain socket path
#[cfg(all(feature = "unix-socket", feature = "password-gen"))]
pub fn get_unique_socket_path() -> PathBuf {
    use uuid::Uuid;
    let uuid = Uuid::new_v4();
    std::env::temp_dir().join(format!("swipl_test_{}.sock", uuid))
}

/// Get a unique Unix domain socket path without UUID
#[cfg(all(feature = "unix-socket", not(feature = "password-gen")))]
pub fn get_unique_socket_path() -> PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("swipl_test_{}.sock", timestamp))
}

/// Test fixture that ensures proper cleanup of server resources
pub struct TestServer {
    server: Option<PrologServer>,
    #[cfg(feature = "unix-socket")]
    socket_path: Option<PathBuf>,
}

impl TestServer {
    /// Create a new test server with dynamic port allocation
    pub fn new() -> Result<Self, PrologError> {
        let mut config = ServerConfig::default();
        config.port = Some(get_free_port());
        
        let server = PrologServer::new(config)?;
        Ok(TestServer {
            server: Some(server),
            #[cfg(feature = "unix-socket")]
            socket_path: None,
        })
    }
    
    /// Create a new test server with custom config
    #[allow(dead_code)]
    pub fn with_config(mut config: ServerConfig) -> Result<Self, PrologError> {
        // Ensure we use a dynamic port if not specified
        if config.port.is_none() && config.unix_domain_socket.is_none() {
            config.port = Some(get_free_port());
        }
        
        #[cfg(feature = "unix-socket")]
        let socket_path = config.unix_domain_socket.clone();
        
        let server = PrologServer::new(config)?;
        Ok(TestServer {
            server: Some(server),
            #[cfg(feature = "unix-socket")]
            socket_path,
        })
    }
    
    /// Start the server with proper synchronization
    pub fn start(&mut self) -> Result<(), PrologError> {
        if let Some(ref mut server) = self.server {
            server.start()?;
            // Wait for server to be ready
            self.wait_for_ready(Duration::from_secs(5))?;
        }
        Ok(())
    }
    
    /// Connect to the server
    pub fn connect(&mut self) -> Result<PrologSession, PrologError> {
        if let Some(ref mut server) = self.server {
            server.connect()
        } else {
            Err(PrologError::InvalidState("Server already stopped".to_string()))
        }
    }
    
    /// Stop the server
    pub fn stop(&mut self, force: bool) -> Result<(), PrologError> {
        if let Some(mut server) = self.server.take() {
            server.stop(force)?;
        }
        Ok(())
    }
    
    /// Wait for the server to be ready to accept connections
    fn wait_for_ready(&mut self, timeout: Duration) -> Result<(), PrologError> {
        let start = Instant::now();
        
        while start.elapsed() < timeout {
            match self.connect() {
                Ok(session) => {
                    // Successfully connected, server is ready
                    drop(session);
                    return Ok(());
                }
                Err(_) => {
                    // Not ready yet, wait a bit
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
        
        Err(PrologError::Timeout)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Ensure server is stopped on drop
        let _ = self.stop(true);
        
        // Clean up Unix socket file if it exists
        #[cfg(feature = "unix-socket")]
        if let Some(ref path) = self.socket_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Helper struct for running tests with timeout
pub struct TestTimeout {
    start: Instant,
    duration: Duration,
}

impl TestTimeout {
    pub fn new(duration: Duration) -> Self {
        TestTimeout {
            start: Instant::now(),
            duration,
        }
    }
    
    pub fn check(&self) -> Result<(), PrologError> {
        if self.start.elapsed() > self.duration {
            Err(PrologError::Timeout)
        } else {
            Ok(())
        }
    }
    
    #[allow(dead_code)]
    pub fn remaining(&self) -> Duration {
        self.duration.saturating_sub(self.start.elapsed())
    }
}

/// Synchronization helper for async operations
#[allow(dead_code)]
pub struct AsyncSync {
    ready: Arc<(Mutex<bool>, Condvar)>,
    #[allow(dead_code)]
    done: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl AsyncSync {
    pub fn new() -> Self {
        AsyncSync {
            ready: Arc::new((Mutex::new(false), Condvar::new())),
            done: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Signal that an async operation is ready
    pub fn signal_ready(&self) {
        let (lock, cvar) = &*self.ready;
        let mut ready = lock.lock().unwrap();
        *ready = true;
        cvar.notify_all();
    }
    
    /// Wait for the async operation to be ready
    pub fn wait_ready(&self, timeout: Duration) -> Result<(), PrologError> {
        let (lock, cvar) = &*self.ready;
        let mut ready = lock.lock().unwrap();
        
        let start = Instant::now();
        while !*ready {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(PrologError::Timeout);
            }
            
            let result = cvar.wait_timeout(ready, remaining).unwrap();
            ready = result.0;
            if result.1.timed_out() && !*ready {
                return Err(PrologError::Timeout);
            }
        }
        
        Ok(())
    }
    
    /// Mark the operation as done
    pub fn mark_done(&self) {
        self.done.store(true, Ordering::SeqCst);
    }
    
    /// Check if the operation is done
    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }
}

/// Retry helper for flaky operations
#[allow(dead_code)]
pub fn retry_with_backoff<T, E, F>(
    mut f: F,
    max_attempts: usize,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let mut delay = initial_delay;
    
    for attempt in 1..=max_attempts {
        match f() {
            Ok(result) => return Ok(result),
            Err(_e) if attempt < max_attempts => {
                debug!("Attempt {} failed, retrying after {:?}", attempt, delay);
                thread::sleep(delay);
                delay *= 2; // Exponential backoff
            }
            Err(e) => return Err(e),
        }
    }
    
    unreachable!()
}

/// Helper to run a test with a timeout
#[allow(dead_code)]
pub fn run_with_timeout<F, T>(f: F, timeout: Duration) -> Result<T, PrologError>
where
    F: FnOnce() -> Result<T, PrologError> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    
    thread::spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });
    
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => Err(PrologError::Timeout),
    }
}

/// Check if SWI-Prolog is available
pub fn swipl_available() -> bool {
    std::process::Command::new("swipl")
        .arg("--version")
        .output()
        .is_ok()
}

/// Skip test if SWI-Prolog is not available
pub fn require_swipl() {
    if !swipl_available() {
        eprintln!("SWI-Prolog not found in PATH. Skipping test.");
        std::process::exit(0);
    }
}

/// Initialize logger for tests
#[allow(dead_code)]
pub fn init_logger() {
    let _ = env_logger::builder()
        .is_test(true)
        .try_init();
}

/// Assert that a query result matches expected success
#[allow(dead_code)]
pub fn assert_query_success(result: &swipl_rs::QueryResult, expected: bool) {
    match result {
        swipl_rs::QueryResult::Success(success) => {
            assert_eq!(*success, expected, "Expected Success({})", expected);
        }
        swipl_rs::QueryResult::Solutions(sols) if expected && sols.len() == 1 && sols[0].is_empty() => {
            // Empty solution is equivalent to Success(true)
        }
        _ => panic!("Expected Success({}), got {:?}", expected, result),
    }
}

/// Assert that a query result has solutions
#[allow(dead_code)]
pub fn assert_has_solutions(result: &swipl_rs::QueryResult, expected_count: Option<usize>) {
    match result {
        swipl_rs::QueryResult::Solutions(sols) => {
            if let Some(count) = expected_count {
                assert_eq!(sols.len(), count, "Expected {} solutions, got {}", count, sols.len());
            } else {
                assert!(!sols.is_empty(), "Expected at least one solution");
            }
        }
        _ => panic!("Expected Solutions, got {:?}", result),
    }
}