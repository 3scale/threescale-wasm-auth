use std::borrow::Cow;

use serde::{Deserialize, Serialize};

mod check;
mod control;
mod decode;
mod format;
mod stack;
mod string;

pub use check::*;
pub use control::*;
pub use decode::*;
pub use format::*;
pub use stack::*;
pub use string::*;

#[derive(Debug, thiserror::Error)]
pub enum OperationError {
    #[error("decoding error")]
    EncodingError(#[from] DecodeError),
    #[error("format error")]
    FormatError(#[from] FormatError),
    #[error("input error")]
    StackError(#[from] StackError),
    #[error("string op error")]
    StringOpError(#[from] StringOpError),
    #[error("control error")]
    ControlError(#[from] ControlError),
    #[error("check error")]
    CheckError(#[from] CheckError),
    #[error("operation should have produced at least one value")]
    NoOutputValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Operation {
    #[serde(rename = "string")]
    StringOp(StringOp),
    Check(Check),
    Control(Control),
    Decode(Decode),
    Format(Format),
    Stack(Stack),
}

impl AsRef<Operation> for Operation {
    fn as_ref(&self) -> &Operation {
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Matcher {
    OneOf(Vec<String>), // produces one result
    Many(Vec<String>),  // produces 1..vec.len() results
    All(Vec<String>),   // produces vec.len() results
}

pub fn process_operations<'a, O: AsRef<Operation>>(
    mut v: Vec<Cow<'a, str>>,
    ops: &[O],
) -> Result<Vec<Cow<'a, str>>, super::OperationError> {
    for op in ops {
        v = match op.as_ref() {
            Operation::Stack(stack) => stack.process(v)?,
            Operation::Check(check) => check.process(v)?,
            Operation::Control(control) => control.process(v)?,
            Operation::StringOp(string_op) => string_op.process(v)?,
            Operation::Decode(decoding) => decoding.process(v)?,
            Operation::Format(format) => format.process(v)?,
        };
        if v.is_empty() {
            return Err(super::OperationError::NoOutputValue);
        }
    }

    Ok(v)
}
