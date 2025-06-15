// Placeholder for the main library module

pub mod error;
pub mod server;
pub mod session;
pub mod types;

// Re-export key types for easier use
pub use error::PrologError;
pub use server::{PrologServer, ServerConfig};
pub use session::PrologSession;
pub use types::{PrologTerm, PrologCompound, QueryResult}; 