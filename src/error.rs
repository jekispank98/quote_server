use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("I/o error: {0}")]
    Io(#[from] io::Error),
    #[error("Format error: {0}")]
    Format(String),
}
