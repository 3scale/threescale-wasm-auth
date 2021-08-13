use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum StringOpError {
    #[error("requirement not satisfied")]
    RequirementNotSatisfied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StringOp {
    #[serde(rename = "nop")]
    NoOp,
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
    Contents {
        prefix: Option<String>,
        suffix: Option<String>,
        contains: Option<String>,
    },
}

mod defaults {
    pub(super) fn separator() -> String {
        ":".into()
    }
}

impl StringOp {
    pub fn process<'a>(&self, input: Cow<'a, str>) -> Result<Vec<Cow<'a, str>>, StringOpError> {
        let res = match self {
            Self::NoOp => vec![input],
            Self::Reverse => vec![input.chars().into_iter().rev().collect::<String>().into()],
            Self::Split { separator, max } => {
                let max = max.unwrap_or(0);
                if max > 0 {
                    input
                        .splitn(max, separator)
                        .map(|s| Cow::from(s.to_string()))
                        .collect()
                } else {
                    input
                        .split(separator)
                        .map(|s| Cow::from(s.to_string()))
                        .collect()
                }
            }
            Self::Replace { pattern, with, max } => {
                let max = max.unwrap_or(0);
                let out = if max > 0 {
                    input.replacen(pattern, with, max)
                } else {
                    input.replace(pattern, with)
                };

                vec![out.into()]
            }
            Self::Contents {
                prefix,
                suffix,
                contains,
            } => {
                if let Some(prefix) = prefix {
                    if !input.starts_with(prefix) {
                        return Err(StringOpError::RequirementNotSatisfied);
                    }
                }
                if let Some(suffix) = suffix {
                    if !input.ends_with(suffix) {
                        return Err(StringOpError::RequirementNotSatisfied);
                    }
                }
                if let Some(substr) = contains {
                    if !input.contains(substr) {
                        return Err(StringOpError::RequirementNotSatisfied);
                    }
                }

                vec![input]
            }
        };

        Ok(res)
    }
}
