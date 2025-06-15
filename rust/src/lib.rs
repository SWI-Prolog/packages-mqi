// Placeholder for the main library module

pub mod error;
pub mod server;
pub mod session;
pub mod types;

// Re-export key types for easier use
pub use error::PrologError;
pub use server::PrologServer;
pub use session::PrologSession;
pub use types::PrologTerm;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
} 