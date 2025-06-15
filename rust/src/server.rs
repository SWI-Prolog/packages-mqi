use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::io::{BufRead, BufReader};
use std::thread;
use log::{
    debug, error, info, trace, warn
};

use crate::error::PrologError;
use crate::session::PrologSession;

// Placeholder for PrologServer configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub launch_mqi: bool,
    pub port: Option<u16>,
    pub password: Option<String>,
    pub unix_domain_socket: Option<PathBuf>,
    pub query_timeout_seconds: Option<f64>,
    pub pending_connection_count: Option<u32>,
    pub output_file_name: Option<PathBuf>,
    pub mqi_traces: Option<String>,
    pub prolog_path: Option<PathBuf>,
    pub prolog_path_args: Option<Vec<String>>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            launch_mqi: true,
            port: None,
            password: None, // Will be generated if None and launch_mqi is true
            unix_domain_socket: None,
            query_timeout_seconds: None,
            pending_connection_count: None, // Use Prolog's default (5)
            output_file_name: None,
            mqi_traces: None,
            prolog_path: None, // Assumes 'swipl' is in PATH
            prolog_path_args: None,
        }
    }
}

/// Represents and manages a connection to a SWI-Prolog MQI server process.
#[derive(Debug)]
pub struct PrologServer {
    config: ServerConfig,
    process: Option<Child>,
    // Need Arc<Mutex> for thread safety if accessed by session
    connection_failed: Arc<Mutex<bool>>, 
    // Details needed by session to connect
    effective_port: Option<u16>,
    effective_uds_path: Option<PathBuf>,
    effective_password: Option<String>,
}

impl PrologServer {
    /// Creates a new PrologServer instance with the given configuration.
    /// This does not start the server process yet; call `start()` for that.
    pub fn new(config: ServerConfig) -> Result<Self, PrologError> {
        // Initial validation
        if config.unix_domain_socket.is_some() {
            #[cfg(not(unix))]
            return Err(PrologError::FeatureNotEnabled(
                "Unix Domain Sockets are only supported on Unix-like systems".to_string(),
            ));
            #[cfg(unix)]
            if config.port.is_some() {
                return Err(PrologError::InvalidState(
                    "Cannot specify both port and unix_domain_socket".to_string(),
                ));
            }
        }

        if !config.launch_mqi && config.output_file_name.is_some() {
            return Err(PrologError::InvalidState(
                "output_file_name only works when launch_mqi is true".to_string(),
            ));
        }

        // Standalone mode validation
        if !config.launch_mqi && (config.port.is_none() && config.unix_domain_socket.is_none()) {
             return Err(PrologError::InvalidState(
                "Must specify port or unix_domain_socket when launch_mqi is false".to_string(),
            ));
        }
         if !config.launch_mqi && config.password.is_none() {
             return Err(PrologError::InvalidState(
                "Must specify password when launch_mqi is false".to_string(),
            ));
        }

        Ok(Self {
            effective_port: config.port,
            effective_uds_path: config.unix_domain_socket.clone(), // Clone path if provided
            effective_password: config.password.clone(),
            config,
            process: None,
            connection_failed: Arc::new(Mutex::new(false)),
        })
    }

    /// Starts the SWI-Prolog MQI server process if `launch_mqi` is true.
    /// If `launch_mqi` is false, this method does nothing but basic validation.
    pub fn start(&mut self) -> Result<(), PrologError> {
        if !self.config.launch_mqi {
            info!("Running in standalone mode, not launching swipl.");
            // Already validated connection details in new()
            return Ok(());
        }

        if self.process.is_some() {
            info!("SWI-Prolog process already started.");
            return Ok(());
        }

        info!("Starting SWI-Prolog MQI process...");

        // TODO: Implement process launching logic here
        // - Build Command based on config
        // - Generate password if needed
        // - Handle path conversion for output_file/uds_path
        // - Spawn process with stdout/stderr piped
        // - Read connection details (port/uds, password) from stdout
        // - Store process handle and effective connection details
        // - Spawn threads to read/log stdout/stderr
        // - Handle launch errors

        unimplemented!("Process launching not yet implemented");

        // Example: Placeholder for spawning stdout/stderr readers
        // if let Some(mut child) = self.process.take() {
        //     if let Some(stdout) = child.stdout.take() {
        //          thread::spawn(move || {
        //             let reader = BufReader::new(stdout);
        //             for line in reader.lines() {
        //                  match line {
        //                     Ok(l) => trace!("Prolog stdout: {}", l),
        //                     Err(e) => error!("Error reading Prolog stdout: {}", e),
        //                 }
        //             }
        //         });
        //     }
        //     // Similar for stderr...
        //     self.process = Some(child); // Put it back
        // }

        Ok(())
    }

    /// Creates a new session (connection) to the MQI server.
    /// This will implicitly call `start()` if the server hasn't been started yet.
    pub fn connect(&mut self) -> Result<PrologSession, PrologError> {
        if self.process.is_none() && self.config.launch_mqi {
            self.start()?;
        }

        // Ensure we have connection details
        let password = self.effective_password.clone().ok_or_else(|| PrologError::InvalidState("Password not available for connection".to_string()))?;

        // TODO: Implement session creation logic
        // - Choose between TCP and UDS based on effective_uds_path
        // - Call PrologSession::connect with address/path and password

        unimplemented!("Session connection not yet implemented");

        // Example placeholder:
        // let address = self.effective_uds_path.as_ref()
        //     .map(|p| ConnectionAddr::Uds(p.clone()))
        //     .or_else(|| self.effective_port.map(ConnectionAddr::Tcp))
        //     .ok_or_else(|| PrologError::InvalidState("No valid connection address (port/UDS)".to_string()))?;
        //
        // PrologSession::connect(address, password, self.connection_failed.clone())
    }

    /// Stops the SWI-Prolog process if it was launched by this instance.
    ///
    /// # Arguments
    /// * `kill` - If true, forcefully kills the process immediately. If false,
    ///   attempts a graceful shutdown by sending the `quit.` command first.
    pub fn stop(&mut self, kill: bool) -> Result<(), PrologError> {
        if let Some(mut child) = self.process.take() {
            info!("Stopping SWI-Prolog process (PID: {})...", child.id());
            let conn_failed = *self.connection_failed.lock().unwrap();

            if !kill && !conn_failed {
                // Try graceful shutdown
                match self.connect() { // Need a temporary session
                    Ok(mut session) => {
                        match session.halt_server_internal() {
                            Ok(_) => info!("Sent quit command successfully."),
                            Err(e) => warn!("Failed to send quit command gracefully: {}. Will kill.", e),
                        }
                    }
                    Err(e) => {
                        warn!("Failed to connect for graceful shutdown: {}. Will kill.", e);
                    }
                }
                // Give it a moment to exit? Small sleep perhaps?
                // std::thread::sleep(std::time::Duration::from_millis(100));
            }

            // Kill if forced, failed graceful, or connection was already known to be bad
            match child.kill() {
                Ok(_) => info!("SWI-Prolog process killed."),
                Err(e) if e.kind() == io::ErrorKind::InvalidInput => {
                    info!("SWI-Prolog process likely already exited.")
                }
                Err(e) => {
                    error!("Failed to kill SWI-Prolog process: {}", e);
                    // Put it back if killing failed, maybe retrying later works?
                    self.process = Some(child);
                    return Err(PrologError::Io(e));
                }
            }

            match child.wait() {
                Ok(status) => info!("SWI-Prolog process exited with status: {}", status),
                Err(e) => error!("Failed to wait for SWI-Prolog process exit: {}", e),
            }

            // TODO: Clean up Unix Domain Socket file if created by us
            // Need to store if *we* created it
            // if let Some(path) = &self.config.unix_domain_socket {
            //     // Check if path is one we generated...
            //     if self.we_generated_uds {
            //          std::fs::remove_file(path).unwrap_or_else(|e| warn!("Failed to remove UDS file {:?}: {}", path, e));
            //     }
            // }

        } else {
            debug!("stop() called but no process was running (or not launched by us).");
        }
        Ok(())
    }
}

// Implement Drop to ensure the process is stopped
impl Drop for PrologServer {
    fn drop(&mut self) {
        if self.process.is_some() {
            warn!("PrologServer dropped without explicit stop(), killing process.");
            if let Err(e) = self.stop(true) {
                error!("Error stopping Prolog process during drop: {}", e);
            }
        }
    }
}

// Helper function for OS path to Prolog POSIX path (placeholder)
fn create_prolog_path(path: &PathBuf) -> Result<String, PrologError> {
    // TODO: Implement proper path conversion logic similar to Python's create_posix_path
    // For now, just return the path as a string, assuming Unix-like paths
    path.to_str().map(|s| s.to_string()).ok_or_else(|| PrologError::InvalidState(format!("Invalid path: {:?}", path)))
} 