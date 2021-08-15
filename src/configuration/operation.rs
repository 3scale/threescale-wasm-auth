use std::borrow::Cow;

use serde::{Deserialize, Serialize};

mod control;
mod decode;
mod format;
mod stack;
mod string;

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
    #[error("operation should have produced at least one value")]
    NoOutputValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged, rename_all = "snake_case")]
pub enum Operation {
    Stack(Stack),
    Decode(Decode),
    Format(Format),
    #[serde(rename = "string")]
    StringOp(StringOp),
    Control(Control),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Matcher {
    OneOf(Vec<String>), // produces one result
    Many(Vec<String>),  // produces 1..vec.len() results
    All(Vec<String>),   // produces vec.len() results
}

pub fn process_operations<'a>(
    mut v: Vec<Cow<'a, str>>,
    ops: &[&Operation],
) -> Result<Vec<Cow<'a, str>>, super::OperationError> {
    for op in ops {
        v = match op {
            Operation::Stack(stack) => stack.process(v)?,
            Operation::Control(control) => {
                let value = v.pop().unwrap();
                let values = control.process(value)?;
                v.extend(values.into_iter());
                v
            }
            Operation::StringOp(string_op) => {
                let value = v.pop().unwrap();
                let values = string_op.process(value)?;
                v.extend(values.into_iter());
                v
            }
            Operation::Decode(decoding) => {
                let value = v.pop().unwrap();
                let result = decoding.decode(value)?;
                v.push(result);
                v
            }
            Operation::Format(format) => {
                let value = v.pop().unwrap();
                let values = format.process(value)?;
                v.extend(values.into_iter());
                v
            }
        };
        if v.is_empty() {
            return Err(super::OperationError::NoOutputValue);
        }
    }

    Ok(v)
}
