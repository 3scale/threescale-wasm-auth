use serde::{Deserialize, Serialize};

mod decode;
mod format;

pub use decode::*;
pub use format::*;

#[derive(Debug, thiserror::Error)]
pub enum OperationError {
    #[error("decoding error")]
    EncodingError(#[from] DecodeError),
    #[error("format error")]
    FormatError(#[from] FormatError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Decode(Decode),
    Format(Format),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Matcher {
    OneOf(Vec<String>), // produces one result
    Many(Vec<String>),  // produces 1..vec.len() results
    All(Vec<String>),   // produces vec.len() results
}
