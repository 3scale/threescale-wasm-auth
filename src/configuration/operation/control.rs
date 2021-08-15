use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::log::LogLevel;

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("requirement not satisfied")]
    RequirementNotSatisfied,
    #[error("inner operation error")]
    InnerOperationError(#[from] Box<super::OperationError>),
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
    Assert(Box<super::Operation>),
    Refute(Box<super::Operation>),
    Log {
        #[serde(default)]
        level: LogLevel,
        msg: String,
    },
}

impl Control {
    pub fn process<'a>(&self, input: Cow<'a, str>) -> Result<Vec<Cow<'a, str>>, ControlError> {
        let res = match self {
            Self::True => vec![input],
            Self::False => return Err(ControlError::RequirementNotSatisfied),
            Self::Any(ops) => ops
                .iter()
                .find_map(|op| super::process_operations(vec![input.clone()], &[op]).ok())
                .ok_or(ControlError::RequirementNotSatisfied)?,
            Self::OneOf(ops) => ops
                .iter()
                .try_fold(None, |acc, op| {
                    if let Ok(result) = super::process_operations(vec![input.clone()], &[op]) {
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
            Self::All(ops) => ops
                .iter()
                .all(|op| super::process_operations(vec![input.clone()], &[op]).is_ok())
                .then(|| vec![input])
                .ok_or(ControlError::RequirementNotSatisfied)?,
            Self::None(ops) => ops
                .iter()
                .all(|op| super::process_operations(vec![input.clone()], &[op]).is_err())
                .then(|| vec![input])
                .ok_or(ControlError::RequirementNotSatisfied)?,
            Self::Assert(op) => super::process_operations(vec![input.clone()], &[op])
                .is_ok()
                .then(|| vec![input])
                .ok_or(ControlError::RequirementNotSatisfied)?,
            Self::Refute(op) => super::process_operations(vec![input.clone()], &[op])
                .is_err()
                .then(|| vec![input])
                .ok_or(ControlError::RequirementNotSatisfied)?,
            Self::Log { level, msg } => {
                crate::log!(&"[3scale-auth/config]", *level, "{}", msg);
                vec![input]
            }
        };

        Ok(res)
    }
}
