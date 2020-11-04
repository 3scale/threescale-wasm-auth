use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("failed to decode protobuf: {0}")]
    ProtoBufDecodingError(#[from] prost::DecodeError),
    #[error("failed to decode JSON: {0}")]
    JsonDecodingError(#[from] serde_json::Error),
    #[error("value lookup failed: {0}")]
    LookupError(#[from] crate::proxy::metadata::LookupError),
    #[error("could not find a string")]
    NoStringFound,
    #[error("index out of bounds")]
    IndexOutOfBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Plain,
    #[serde(rename = "json")]
    Json {
        #[serde(default)]
        path: Vec<String>,
        keys: Vec<String>,
    },
    #[serde(rename = "protobuf")]
    ProtoBuf {
        #[serde(default)]
        path: Vec<String>,
        keys: Vec<String>,
    },
    Joined {
        #[serde(default = "defaults::separator")]
        separator: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
        #[serde(default)]
        indexes: Vec<usize>,
    },
}

mod defaults {
    pub(super) fn separator() -> String {
        ":".into()
    }
}

impl Default for Format {
    fn default() -> Self {
        Self::Plain
    }
}

impl Format {
    pub fn parse<'a>(&self, input: Cow<'a, str>) -> Result<Vec<Cow<'a, str>>, FormatError> {
        use crate::proxy::metadata::ValueExt;
        let res = match self {
            Self::Plain => vec![input],
            Self::Joined {
                separator,
                max,
                indexes,
            } => {
                let max = max.unwrap_or(0);
                let parts: Vec<_> = if max > 0 {
                    input.splitn(max, separator).collect()
                } else {
                    input.split(separator).collect()
                };
                indexes.iter().try_fold(vec![], |mut acc, &idx| {
                    parts
                        .get(idx)
                        .ok_or(FormatError::IndexOutOfBounds)
                        .map(|&s| {
                            acc.push(Cow::from(s.to_string()));
                            acc
                        })
                })?
            }
            Self::ProtoBuf { path, keys } => {
                let st = <prost_types::Struct as prost::Message>::decode(input.as_bytes())?;
                let v = prost_types::Value {
                    kind: Some(prost_types::value::Kind::StructValue(st)),
                };
                let (v, _) = v.lookup(
                    path.iter()
                        .map(std::ops::Deref::deref)
                        .collect::<Vec<_>>()
                        .as_slice(),
                )?;
                let s = v
                    .match_one(
                        keys.iter()
                            .map(std::ops::Deref::deref)
                            .collect::<Vec<_>>()
                            .as_slice(),
                    )
                    // XXX TODO FIXME accept also numbers/bools?
                    .and_then(|v| v.as_str())
                    .ok_or(FormatError::NoStringFound)?;
                // the allocation for s comes from the value we decoded, so must return an allocated string
                vec![Cow::from(s.to_string())]
            }
            Self::Json { path, keys } => {
                let json = serde_json::from_str::<serde_json::Value>(input.as_ref())?;
                log::debug!("parsed JSON value: {}", json);
                let (v, _segment) = <serde_json::Value as ValueExt>::lookup(
                    &json,
                    path.iter()
                        .map(std::ops::Deref::deref)
                        .collect::<Vec<_>>()
                        .as_slice(),
                )?;
                log::debug!("looked up JSON value: {}", v);
                let s = v
                    .match_one(
                        keys.iter()
                            .map(std::ops::Deref::deref)
                            .collect::<Vec<_>>()
                            .as_slice(),
                    )
                    // XXX TODO FIXME accept also numbers/bools?
                    .and_then(serde_json::Value::as_str)
                    .ok_or(FormatError::NoStringFound)?;
                // the allocation for s comes from the value we decoded, so must return an allocated string
                log::debug!("matched JSON string: {}", s);
                vec![Cow::from(s.to_string())]
            }
        };

        Ok(res)
    }
}
