//! Crate-wide result alias tying operations to the crate's unified error type.
//! Most public functions return `result::Result<T>`, which defaults the error parameter to
//! `crate::error::ParserError`. This keeps signatures concise and enables ergonomic `?`
//! propagation while still allowing callers to override the error type with
//! `result::Result<T, OtherError>` when needed.

use crate::error::ParserError;

/// Convenient alias for `std::result::Result<T, ParserError>` used throughout the crate.
pub type Result<T, E = ParserError> = std::result::Result<T, E>;
