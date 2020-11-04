use std::convert::TryFrom;

use log::debug;
use prost::Message;
use prost_types::value::Kind as ProtoKind;
use prost_types::{ListValue as ProtoList, Struct as ProtoStruct, Value as ProtoValue};
use serde_json::{Map as JMap, Value as JValue};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("failed to decode protobuffer")]
    ProtoBufDecodeError(#[from] prost::DecodeError),
    #[error("failed to decode JSON")]
    JsonDecodeError(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
#[error("cannot lookup path `{segment}` within `{prev_segment}` of type {kind}: {message}")]
pub struct LookupError {
    message: String,
    segment: String,
    prev_segment: String,
    kind: String,
}

#[derive(Debug, Error)]
#[allow(clippy::module_name_repetitions)]
pub enum MetadataError {
    #[error("failed to decode: {0}")]
    DecodeError(#[from] DecodeError),
    #[error("lookup failed: {0}")]
    LookupError(#[from] LookupError),
    #[error("passed in an empty path to look up")]
    LookupEmptyPath,
}

#[derive(Clone, PartialEq, Message)]
pub struct Metadata {
    /// Key is the reverse DNS filter name, e.g. com.acme.widget. The envoy.*
    /// namespace is reserved for Envoy's built-in filters.
    #[prost(map = "string, message", tag = "1")]
    filter_metadata: ::std::collections::HashMap<std::string::String, ProtoStruct>,
}

impl TryFrom<&[u8]> for Metadata {
    type Error = MetadataError;

    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self::decode(buffer).map_err(DecodeError::from)?)
    }
}

impl Metadata {
    pub fn new(buffer: &[u8]) -> Result<Self, MetadataError> {
        Self::try_from(buffer)
    }

    pub fn get_filter(&self, key: &str) -> Option<&ProtoStruct> {
        self.filter_metadata.get(key)
    }

    pub fn lookup<'a>(
        &self,
        filter: &str,
        path: &[&'a str],
    ) -> Result<(&ProtoValue, &'a str), MetadataError> {
        log::debug!("metadata lookup: filter is {}, path is {:?}", filter, path);
        if path.is_empty() {
            return Err(MetadataError::LookupEmptyPath);
        }

        let entry = path[0];

        self.get_filter(filter)
            .and_then(|st| {
                log::debug!(
                    "looking for entry {} in filter struct ({} entries): {:?}",
                    entry,
                    st.fields.len(),
                    st
                );
                st.fields.get(entry).or_else(|| {
                    // look for a key to mean "if normal lookup failed, take the sole, unique entry"
                    if st.fields.len() == 1 && entry == "0" {
                        st.fields.values().next()
                    } else {
                        None
                    }
                })
            })
            .map(|v| (v, entry))
            .ok_or_else(|| {
                LookupError {
                    message: "not found".into(),
                    prev_segment: filter.into(),
                    segment: entry.into(),
                    kind: "Struct".into(),
                }
                .into()
            })
            .and_then(|(v, _entry)| v.lookup(&path[1..]).map_err(MetadataError::from))
    }
}

pub enum ValueKind {
    Struct,
    List,
    String,
    Number,
    Bool,
    Null,
    Unknown,
}

impl ValueKind {
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Struct => "struct",
            Self::List => "list",
            Self::String => "string",
            Self::Number => "number",
            Self::Bool => "bool",
            Self::Null => "null",
            Self::Unknown => "unknown",
        }
    }
}

//pub(crate) enum

pub trait ValueExt {
    type Struct;
    type List;

    fn kind(&self) -> ValueKind;

    fn as_str(&self) -> Option<&str>;
    fn as_list(&self) -> Option<&Self::List>;
    fn as_struct(&self) -> Option<&Self::Struct>;
    fn as_number(&self) -> Option<f64>;
    fn as_bool(&self) -> Option<bool>;
    fn match_one<'a>(&'a self, keys: &[&str]) -> Option<&'a Self>;
    fn lookup<'a>(&self, path: &[&'a str]) -> Result<(&Self, &'a str), LookupError> {
        debug!("looking up path {:?}", path);
        path.iter()
            .try_fold((self, "(root)"), |(acc, prev_segment), &segment| {
                if segment.is_empty() {
                    Ok((acc, segment))
                } else {
                    acc.match_one(&[segment])
                        .ok_or_else(|| LookupError {
                            segment: segment.into(),
                            prev_segment: prev_segment.into(),
                            kind: acc.kind().as_str().into(),
                            message: "not found".into(),
                        })
                        .map(|v| (v, segment))
                }
            })
    }
}

impl ValueExt for ProtoValue {
    type Struct = ProtoStruct;
    type List = ProtoList;

    fn kind(&self) -> ValueKind {
        match self.kind {
            Some(ProtoKind::StructValue(_)) => ValueKind::Struct,
            Some(ProtoKind::ListValue(_)) => ValueKind::List,
            Some(ProtoKind::StringValue(_)) => ValueKind::String,
            Some(ProtoKind::NumberValue(_)) => ValueKind::Number,
            Some(ProtoKind::BoolValue(_)) => ValueKind::Bool,
            Some(ProtoKind::NullValue(_)) => ValueKind::Null,
            None => ValueKind::Unknown,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            ProtoValue {
                kind: Some(ProtoKind::StringValue(s)),
            } => Some(&*s),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<&Self::List> {
        match self {
            ProtoValue {
                kind: Some(ProtoKind::ListValue(list)),
            } => Some(list),
            _ => None,
        }
    }

    fn as_struct(&self) -> Option<&Self::Struct> {
        match self {
            ProtoValue {
                kind: Some(ProtoKind::StructValue(st)),
            } => Some(st),
            _ => None,
        }
    }

    fn as_number(&self) -> Option<f64> {
        match self {
            ProtoValue {
                kind: Some(ProtoKind::NumberValue(n)),
            } => Some(*n),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            ProtoValue {
                kind: Some(ProtoKind::BoolValue(b)),
            } => Some(*b),
            _ => None,
        }
    }

    fn match_one<'a>(&'a self, keys: &[&str]) -> Option<&'a Self> {
        let mut keys_it = keys.iter();

        self.kind.as_ref().and_then(|kind| {
            match kind {
                ProtoKind::StructValue(st) => {
                    // try to match a direct entry first
                    keys_it.find_map(|&k| st.fields.get(k)).or_else(|| {
                        // look for a key to mean "if everything else failed, take the sole, unique entry"
                        if st.fields.len() == 1 && keys.contains(&"0") {
                            st.fields.values().next()
                        } else {
                            None
                        }
                    })
                }
                ProtoKind::ListValue(list) => keys_it
                    .find_map(|&k| k.parse::<usize>().ok().and_then(|idx| list.values.get(idx))),
                ProtoKind::StringValue(s) => {
                    keys_it.find_map(|&k| if k == *s { Some(self) } else { None })
                }
                ProtoKind::NumberValue(f) => keys_it.find_map(|&k| {
                    <f64 as core::str::FromStr>::from_str(k).ok().and_then(|n| {
                        if (*f - n).abs() < f64::EPSILON {
                            Some(self)
                        } else {
                            None
                        }
                    })
                }),
                ProtoKind::BoolValue(b) => keys_it.find_map(|&k| {
                    <bool as core::str::FromStr>::from_str(k)
                        .ok()
                        .and_then(|n| if *b == n { Some(self) } else { None })
                }),
                // prost associates NullValue to an i32,
                // but I don't think it represents any meaningful value for us.
                ProtoKind::NullValue(_) => None,
            }
        })
    }
}

// Might want to consider JSON Pointer + exceptions for looking up values
impl ValueExt for JValue {
    type List = Vec<Self>;
    type Struct = JMap<String, Self>;

    fn kind(&self) -> ValueKind {
        if self.is_object() {
            ValueKind::Struct
        } else if self.is_array() {
            ValueKind::List
        } else if self.is_string() {
            ValueKind::String
        } else if self.is_number() {
            ValueKind::Number
        } else if self.is_boolean() {
            ValueKind::Bool
        } else if self.is_null() {
            ValueKind::Null
        } else {
            ValueKind::Unknown
        }
    }

    fn as_str(&self) -> Option<&str> {
        Self::as_str(self)
    }

    fn as_list(&self) -> Option<&Self::List> {
        Self::as_array(self)
    }

    fn as_struct(&self) -> Option<&Self::Struct> {
        Self::as_object(self)
    }

    fn as_number(&self) -> Option<f64> {
        Self::as_f64(self)
    }

    fn as_bool(&self) -> Option<bool> {
        Self::as_bool(self)
    }

    fn match_one<'a>(&'a self, keys: &[&str]) -> Option<&'a Self> {
        let mut keys_it = keys.iter();

        self.as_object()
            .map(|st| {
                keys_it.find_map(|&k| st.get(k)).or_else(|| {
                    // look for a key to mean "if everything else failed, take the sole, unique entry"
                    if st.len() == 1 && keys.contains(&"0") {
                        st.values().next()
                    } else {
                        None
                    }
                })
            })
            .or_else(|| {
                self.as_array().map(|list| {
                    keys_it.find_map(|&k| k.parse::<usize>().ok().and_then(|idx| list.get(idx)))
                })
            })
            .or_else(|| {
                Self::as_str(self)
                    .map(|s| keys_it.find_map(|&k| if k == s { Some(self) } else { None }))
            })
            .or_else(|| {
                Self::as_f64(self).map(|f| {
                    keys_it.find_map(|&k| {
                        <f64 as core::str::FromStr>::from_str(k).ok().and_then(|n| {
                            if (f - n).abs() < f64::EPSILON {
                                Some(self)
                            } else {
                                None
                            }
                        })
                    })
                })
            })
            .or_else(|| {
                Self::as_bool(self).map(|b| {
                    keys_it.find_map(|&k| {
                        <bool as core::str::FromStr>::from_str(k)
                            .ok()
                            .and_then(|n| if b == n { Some(self) } else { None })
                    })
                })
            })
            .flatten()
    }
}
