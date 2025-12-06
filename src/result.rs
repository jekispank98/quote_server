use crate::error::ParserError;

pub type Result<T, E = ParserError> = std::result::Result<T, E>;
