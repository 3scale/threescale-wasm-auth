use std::borrow::Cow;

use proxy_wasm::traits::HttpContext;
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
    Test {
        #[serde(rename = "if")]
        r#if: Box<super::Operation>,
        then: Vec<super::Operation>,
        #[serde(rename = "else", default)]
        r#else: Vec<super::Operation>,
    },
    Or(Vec<super::Operation>),
    Xor(Vec<super::Operation>),
    And(Vec<super::Operation>),
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
    Top(Vec<super::Operation>),
    Log {
        #[serde(default)]
        level: LogLevel,
        msg: String,
    },
}

impl Control {
    pub fn process<'a>(
        &self,
        ctx: &dyn HttpContext,
        mut stack: Vec<Cow<'a, str>>,
    ) -> Result<Vec<Cow<'a, str>>, ControlError> {
        let res = match self {
            Self::Test { r#if, then, r#else } => {
                let ops = if super::process_operations(ctx, stack.clone(), &[r#if]).is_ok() {
                    then
                } else {
                    r#else
                };

                super::process_operations(ctx, stack, ops.as_slice())
                    .map_err(|e| ControlError::InnerOperationError(e.into()))?
            }
            Self::Or(ops) => ops
                .iter()
                .find_map(|op| super::process_operations(ctx, stack.clone(), &[op]).ok())
                .ok_or(ControlError::RequirementNotSatisfied)?,
            Self::Xor(ops) => ops
                .iter()
                .try_fold(None, |acc, op| {
                    if let Ok(result) = super::process_operations(ctx, stack.clone(), &[op]) {
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
            Self::And(ops) => super::process_operations(ctx, stack, ops.as_slice())
                .map_err(|e| ControlError::InnerOperationError(e.into()))?,
            Self::Cloned { result, ops } => {
                let new_stack = stack.clone();
                match super::process_operations(ctx, new_stack, ops.as_slice()) {
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

                match super::process_operations(ctx, partial, ops.as_slice()) {
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
            Self::Top(ops) => {
                let input = stack.pop().ok_or(ControlError::NoValuesError)?;
                let res = super::process_operations(ctx, vec![input], ops.as_slice())
                    .map_err(|e| ControlError::InnerOperationError(e.into()))?;
                stack.extend(res.into_iter());
                stack
            }
            Self::Log { level, msg } => {
                crate::log!(&"[3scale-auth/config]", *level, "{}", msg);
                stack
            }
        };

        Ok(res)
    }
}
