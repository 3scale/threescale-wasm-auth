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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthMode {
    #[serde(rename = "utf8")]
    UTF8,
    Bytes,
}

impl Default for LengthMode {
    fn default() -> Self {
        Self::UTF8
    }
}

impl LengthMode {
    pub fn for_str<S: AsRef<str>>(&self, s: S) -> usize {
        let s = s.as_ref();

        match self {
            Self::UTF8 => s.len(),
            Self::Bytes => s.as_bytes().len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StringOp {
    #[serde(rename = "strlen")]
    Length {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
        #[serde(default)]
        mode: LengthMode,
    },
    #[serde(rename = "strrev")]
    Reverse,
    Split {
        #[serde(default = "defaults::separator")]
        separator: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
    },
    #[serde(rename = "rsplit")]
    RSplit {
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
            Self::Length { min, max, mode } => {
                if let Some(min) = min {
                    if mode.for_str(&input) < *min {
                        return Err(StringOpError::RequirementNotSatisfied);
                    }
                }
                if let Some(max) = max {
                    if mode.for_str(&input) > *max {
                        return Err(StringOpError::RequirementNotSatisfied);
                    }
                }
            }
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
            Self::RSplit { separator, max } => {
                let max = max.unwrap_or(0);
                if max > 0 {
                    stack.extend(
                        input
                            .rsplitn(max, separator)
                            .map(|s| Cow::from(s.to_string())),
                    )
                } else {
                    stack.extend(input.rsplit(separator).map(|s| Cow::from(s.to_string())))
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
