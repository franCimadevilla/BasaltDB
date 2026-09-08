//! # BasaltDB
//!
//! An educational relational database engine implemented in Rust.
//!
//! Development follows the roadmap in `docs/phases/`; the storage engine
//! (Phase 1) lives in [`storage`], with error handling in [`error`].
//!
//! ## Example
//!
//! ```
//! use basalt_db::{Error, Result};
//!
//! fn do_work() -> Result<()> {
//!     // Your logic here
//!     Ok(())
//! }
//! ```

// Declaration of the modules. 
pub mod error;
pub mod parser;
pub mod storage;

// Re-export key items for a cleaner public API.
// This allows users to write `use basalt_db::Error` instead of `use basalt_db::error::Error`.
pub use error::Error;

/// A specialized `Result` type for this crate.
///
/// This alias defaults to our custom `Error` type, saving users from 
/// specifying the error type in every function signature.
pub type Result<T> = std::result::Result<T, Error>;   