use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::util::glob::{GlobPattern, GlobPatternSet};

#[derive(Debug, thiserror::Error)]
pub enum StringOpError {
    #[error("input has no values")]
    NoValuesError,
    #[error("requirement not satisfied")]
    RequirementNotSatisfied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StringOp {
    Reverse,
    Split {
        #[serde(default = "defaults::separator")]
        separator: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
    },
    Replace {
        pattern: String,
        with: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
    },
    Prefix(String),
    Suffix(String),
    Contains(String),
    GlobSet(GlobPatternSet),
    Glob(GlobPattern),
}

mod defaults {
    pub(super) fn separator() -> String {
        ":".into()
    }
}

impl StringOp {
    pub fn process<'a>(
        &self,
        mut stack: Vec<Cow<'a, str>>,
    ) -> Result<Vec<Cow<'a, str>>, StringOpError> {
        let input = stack.pop().ok_or(StringOpError::NoValuesError)?;

        match self {
            Self::Reverse => {
                let value = input.chars().into_iter().rev().collect::<Cow<str>>();
                stack.push(value);
            }
            Self::Split { separator, max } => {
                let max = max.unwrap_or(0);
                if max > 0 {
                    stack.extend(
                        input
                            .splitn(max, separator)
                            .map(|s| Cow::from(s.to_string())),
                    )
                } else {
                    stack.extend(input.split(separator).map(|s| Cow::from(s.to_string())))
                }
            }
            Self::Replace { pattern, with, max } => {
                let max = max.unwrap_or(0);
                let replaced = if max > 0 {
                    input.replacen(pattern, with, max)
                } else {
                    input.replace(pattern, with)
                };

                stack.push(replaced.into());
            }
            Self::Prefix(prefix) => {
                if !input.starts_with(prefix) {
                    return Err(StringOpError::RequirementNotSatisfied);
                }

                stack.push(input);
            }
            Self::Suffix(suffix) => {
                if !input.ends_with(suffix) {
                    return Err(StringOpError::RequirementNotSatisfied);
                }

                stack.push(input);
            }
            Self::Contains(contains) => {
                if !input.contains(contains) {
                    return Err(StringOpError::RequirementNotSatisfied);
                }

                stack.push(input);
            }
            Self::Glob(pattern) => {
                if !pattern.is_match(input.as_ref()) {
                    return Err(StringOpError::RequirementNotSatisfied);
                }

                stack.push(input);
            }
            Self::GlobSet(pattern_set) => {
                if !pattern_set.is_match(input.as_ref()) {
                    return Err(StringOpError::RequirementNotSatisfied);
                }

                stack.push(input);
            }
        };

        Ok(stack)
    }
}
