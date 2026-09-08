//! Error types shared across the whole crate.
//!
//! BasaltDB follows the principle: *never panic for recoverable situations*.
//! Every fallible operation returns `Result<T, Error>`.

use std::fmt;

/// The central error type of the database engine.
///
/// Variants are deliberately coarse: each subsystem can extend this enum
/// (or attach more context through descriptive messages) without forcing
/// callers to match on unstable internals.
#[derive(Debug)]
pub enum Error {
    /// An underlying I/O failure (file not found, permissions, disk errors...).
    Io(std::io::Error),
    /// A generic, storage-level failure described by `msg`.
    Storage(String),
    /// The requested page does not exist in the underlying file.
    InvalidPageId(u32),
    /// A record does not fit in the remaining free space of a page.
    PageFull {
        /// Bytes required by the record (data + slot + header overhead).
        needed: usize,
        /// Bytes currently available in the page.
        free: usize,
    },
    /// A slot id that does not exist (or was deleted) was accessed.
    InvalidSlot(u16),
    /// A page's on-disk layout failed validation (bad magic, truncated data...).
    Corrupt(String),
    /// A serialized row could not be decoded.
    Deserialize(String),
    /// A requested object (table, column...) does not exist.
    NotFound(String),
    /// An object was created twice (e.g. duplicate table name).
    Duplicate(String),
    /// Syntax error in the query specified
    Syntax(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Storage(msg) => write!(f, "storage error: {msg}"),
            Error::InvalidPageId(id) => write!(f, "invalid page id: {id}"),
            Error::PageFull { needed, free } => {
                write!(f, "page full: need {needed} bytes, only {free} available")
            }
            Error::InvalidSlot(slot) => write!(f, "invalid slot id: {slot}"),
            Error::Corrupt(msg) => write!(f, "corrupt page: {msg}"),
            Error::Deserialize(msg) => write!(f, "deserialization error: {msg}"),
            Error::NotFound(msg) => write!(f, "not found: {msg}"),
            Error::Duplicate(msg) => write!(f, "duplicate: {msg}"),
            Error::Syntax(msg) => write!(f, "syntax: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
