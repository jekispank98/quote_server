//! Error types used across the quote server crate.
//!
//! The primary error surface is `ParserError`, which wraps I/O issues, text/serialization
//! problems, channel failures, and several domain-specific conditions. Most public
//! functions in this crate return `result::Result<T>` (an alias whose default error
//! type is `ParserError`), allowing the `?` operator to be used ergonomically.
//!
//! Conversions:
//! - Common library errors (e.g., `std::io::Error`, UTF‑8 and bincode errors) are
//!   converted via `From` into `ParserError` automatically.
//! - A blanket `From<PoisonError<T>>` is provided to turn poisoned `Mutex`/`RwLock`
//!   errors into a structured `ParserError::MutexLock`.

use std::io;
use std::string::FromUtf8Error;
use thiserror::Error;
use bincode;
use std::sync::PoisonError;

/// Unified error type for parsing, networking, channel operations, and internal logic.
#[derive(Error, Debug)]
pub enum ParserError {
    /// Wrapper for underlying `std::io::Error` values (socket I/O, cloning sockets, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Free‑form formatting/validation error encountered while building or interpreting data.
    #[error("Format error: {0}")]
    Format(String),

    /// UTF‑8 decoding error when converting bytes into `String`.
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] FromUtf8Error),

    /// Failure while decoding with `bincode` (invalid or truncated payloads, etc.).
    #[error("Bincode serialization/deserialization error: {0}")]
    BincodeDecode(#[from] bincode::error::DecodeError),

    /// Failure while encoding with `bincode` (I/O or serialization issues).
    #[error("Bincode serialization/deserialization error: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),

    /// Crossbeam/channel send failed (e.g., receiver dropped); contains a short context string.
    #[error("Channel send failed: {0}")]
    ChannelSend(String),

    /// Crossbeam/channel receive failed (e.g., sender closed); contains a short context string.
    #[error("Channel receive failed: {0}")]
    ChannelRecv(String),

    /// Internal logic error where a requested ticker symbol could not be resolved.
    #[error("Internal Logic Error: Ticker not found: {0}")]
    TickerNotFound(String),

    /// Error indicating a poisoned mutex/lock was encountered.
    #[error("Mutex Lock Poisoned: {0}")]
    MutexLock(String),
}

/// Convert any `PoisonError<T>` (from `Mutex`/`RwLock`) into `ParserError::MutexLock`.
impl<T> From<PoisonError<T>> for ParserError {
    fn from(err: PoisonError<T>) -> Self {
        ParserError::MutexLock(err.to_string())
    }
}