use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("requirement not satisfied")]
    RequirementNotSatisfied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    Any(Vec<super::Operation>),
    OneOf(Vec<super::Operation>),
    All(Vec<super::Operation>),
    None(Vec<super::Operation>),
    Assert(Vec<super::Operation>),
    Refute(Vec<super::Operation>),
    Ok,
    Fail,
}

impl Check {
    pub fn process<'a>(&self, stack: Vec<Cow<'a, str>>) -> Result<Vec<Cow<'a, str>>, CheckError> {
        match self {
            Self::Any(ops) => {
                let _ = ops
                    .iter()
                    .find(|op| super::process_operations(stack.clone(), &[op]).is_ok())
                    .ok_or(CheckError::RequirementNotSatisfied)?;
            }
            Self::OneOf(ops) => {
                let _ = ops
                    .iter()
                    .try_fold(None, |acc, op| {
                        if let Ok(result) = super::process_operations(stack.clone(), &[op]) {
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
                    .ok_or(CheckError::RequirementNotSatisfied)?;
            }
            Self::All(ops) => {
                if !ops
                    .iter()
                    .all(|op| super::process_operations(stack.clone(), &[op]).is_ok())
                {
                    return Err(CheckError::RequirementNotSatisfied);
                }
            }
            Self::None(ops) => {
                if !ops
                    .iter()
                    .all(|op| super::process_operations(stack.clone(), &[op]).is_err())
                {
                    return Err(CheckError::RequirementNotSatisfied);
                }
            }
            Self::Assert(ops) => {
                let _ = super::process_operations(stack.clone(), ops)
                    .map_err(|_| CheckError::RequirementNotSatisfied)?;
            }
            Self::Refute(ops) => {
                if super::process_operations(stack.clone(), ops).is_ok() {
                    return Err(CheckError::RequirementNotSatisfied);
                }
            }
            Self::Ok => (),
            Self::Fail => return Err(CheckError::RequirementNotSatisfied),
        };

        Ok(stack)
    }
}
