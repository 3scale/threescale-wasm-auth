use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::log::LogLevel;

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("input has no values")]
    NoValuesError,
    #[error("requirement not satisfied")]
    RequirementNotSatisfied,
    #[error("inner operation error")]
    InnerOperationError(#[from] Box<super::OperationError>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackExtendMode {
    Prepend,
    Append,
}

impl Default for StackExtendMode {
    fn default() -> Self {
        Self::Append
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    True,
    False,
    Any(Vec<super::Operation>),
    OneOf(Vec<super::Operation>),
    All(Vec<super::Operation>),
    None(Vec<super::Operation>),
    Assert(Vec<super::Operation>),
    Refute(Vec<super::Operation>),
    Group(Vec<super::Operation>),
    Cloned {
        #[serde(default)]
        result: StackExtendMode,
        ops: Vec<super::Operation>,
    },
    Partial {
        #[serde(default)]
        result: StackExtendMode,
        ops: Vec<super::Operation>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
    },
    Pipe(Vec<super::Operation>),
    Log {
        #[serde(default)]
        level: LogLevel,
        msg: String,
    },
}

impl Control {
    pub fn process<'a>(
        &self,
        mut stack: Vec<Cow<'a, str>>,
    ) -> Result<Vec<Cow<'a, str>>, ControlError> {
        let input = stack.pop().ok_or(ControlError::NoValuesError)?;

        let res = match self {
            Self::True => {
                stack.push(input);
                stack
            }
            Self::False => return Err(ControlError::RequirementNotSatisfied),
            Self::Any(ops) => {
                stack.extend(
                    ops.iter()
                        .find_map(|op| super::process_operations(vec![input.clone()], &[op]).ok())
                        .ok_or(ControlError::RequirementNotSatisfied)?
                        .into_iter(),
                );
                stack
            }
            Self::OneOf(ops) => {
                stack.extend(
                    ops.iter()
                        .try_fold(None, |acc, op| {
                            if let Ok(result) =
                                super::process_operations(vec![input.clone()], &[op])
                            {
                                if acc.is_some() {
                                    None
                                } else {
                                    Some(Some(result))
                                }
                            } else {
                                Some(acc)
                            }
                        })
                        .flatten()
                        .ok_or(ControlError::RequirementNotSatisfied)?,
                );
                stack
            }
            Self::All(ops) => ops
                .iter()
                .all(|op| super::process_operations(vec![input.clone()], &[op]).is_ok())
                .then(|| {
                    stack.push(input);
                    stack
                })
                .ok_or(ControlError::RequirementNotSatisfied)?,
            Self::None(ops) => ops
                .iter()
                .all(|op| super::process_operations(vec![input.clone()], &[op]).is_err())
                .then(|| {
                    stack.push(input);
                    stack
                })
                .ok_or(ControlError::RequirementNotSatisfied)?,
            Self::Assert(ops) => {
                stack.push(input);

                let _ = super::process_operations(stack.clone(), ops)
                    .map_err(|_| ControlError::RequirementNotSatisfied)?;

                stack
            }
            Self::Refute(ops) => {
                stack.push(input);

                if super::process_operations(stack.clone(), ops).is_ok() {
                    return Err(ControlError::RequirementNotSatisfied);
                }

                stack
            }
            Self::Group(ops) => {
                stack.push(input);
                super::process_operations(stack, ops.as_slice())
                    .map_err(|e| ControlError::InnerOperationError(e.into()))?
            }
            Self::Cloned { result, ops } => {
                let new_stack = stack.clone();
                match super::process_operations(new_stack, ops.as_slice()) {
                    Ok(mut v) => match result {
                        StackExtendMode::Append => {
                            stack.extend(v.into_iter());
                            stack
                        }
                        StackExtendMode::Prepend => {
                            v.extend(stack.into_iter());
                            v
                        }
                    },
                    Err(e) => return Err(ControlError::InnerOperationError(e.into())),
                }
            }
            Self::Partial { result, ops, max } => {
                let max = core::cmp::min(max.unwrap_or(1), 1);
                let partial = stack.split_off(stack.len().saturating_sub(max));
                if partial.is_empty() {
                    return Err(ControlError::NoValuesError);
                }

                match super::process_operations(partial, ops.as_slice()) {
                    Ok(mut v) => match result {
                        StackExtendMode::Append => {
                            stack.extend(v.into_iter());
                            stack
                        }
                        StackExtendMode::Prepend => {
                            v.extend(stack.into_iter());
                            v
                        }
                    },
                    Err(e) => return Err(ControlError::InnerOperationError(e.into())),
                }
            }
            Self::Pipe(ops) => {
                stack.extend(
                    super::process_operations(vec![input], ops.as_slice())
                        .map_err(|_| ControlError::RequirementNotSatisfied)?
                        .into_iter(),
                );
                stack
            }
            Self::Log { level, msg } => {
                crate::log!(&"[3scale-auth/config]", *level, "{}", msg);
                stack.push(input);
                stack
            }
        };

        Ok(res)
    }
}
