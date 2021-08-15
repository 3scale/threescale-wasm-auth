use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, thiserror::Error)]
pub enum DecodeError {
    #[error("failed to decode base64")]
    Base64Error(#[from] base64::DecodeError),
    #[error("invalid utf8 string")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decode {
    #[serde(rename = "base64_standard")]
    Base64,
    #[serde(rename = "base64_urlsafe")]
    Base64UrlSafe,
}

impl Decode {
    pub fn decode<'a>(&self, input: Cow<'a, str>) -> Result<Cow<'a, str>, DecodeError> {
        let res = match self {
            Self::Base64 => Cow::from(String::from_utf8(base64::decode_config(
                input.as_ref(),
                base64::STANDARD,
            )?)?),
            Self::Base64UrlSafe => Cow::from(String::from_utf8(base64::decode_config(
                input.as_ref(),
                base64::URL_SAFE,
            )?)?),
        };

        Ok(res)
    }
}
