use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::proxy::metadata::{LookupError as LookupErr, ValueExt};

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("input has no values")]
    NoValuesError,
    #[error("failed to decode protobuf: {0}")]
    ProtoBufDecodingError(#[from] prost::DecodeError),
    #[error("failed to decode JSON: {0}")]
    JsonDecodingError(#[from] serde_json::Error),
    #[error("value lookup failed: {0}")]
    LookupError(#[from] LookupErr),
    #[error("could not find a string")]
    NoStringFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
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
}

impl Format {
    pub fn process<'a>(
        &self,
        mut stack: Vec<Cow<'a, str>>,
    ) -> Result<Vec<Cow<'a, str>>, FormatError> {
        let input = stack.pop().ok_or(FormatError::NoValuesError)?;

        let s = match self {
            Self::ProtoBuf { path, keys } => {
                let st = <prost_types::Struct as prost::Message>::decode(input.as_bytes())?;
                let format_value = prost_types::Value {
                    kind: Some(prost_types::value::Kind::StructValue(st)),
                };
                resolve_value(format_value, path, keys)?
            }
            Self::Json { path, keys } => {
                let format_value = serde_json::from_str::<serde_json::Value>(input.as_ref())?;
                resolve_value(format_value, path, keys)?
            }
        };

        stack.push(s.into());
        Ok(stack)
    }
}

fn resolve_value<V: ValueExt>(
    format_value: V,
    path: &[String],
    keys: &[String],
) -> Result<String, FormatError> {
    let (v, _) = format_value.lookup(
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
    Ok(s.into())
}
