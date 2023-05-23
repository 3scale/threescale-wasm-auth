use std::borrow::Cow;

use base64::{engine::general_purpose, Engine as _};
use proxy_wasm::traits::HttpContext;
use serde::{Deserialize, Serialize};

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, thiserror::Error)]
pub enum DecodeError {
    #[error("input has no values")]
    NoValuesError,
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
    pub fn process<'a>(
        &self,
        _ctx: &dyn HttpContext,
        mut stack: Vec<Cow<'a, str>>,
    ) -> Result<Vec<Cow<'a, str>>, DecodeError> {
        let input = stack.pop().ok_or(DecodeError::NoValuesError)?;

        let s = match self {
            Self::Base64 => String::from_utf8(general_purpose::STANDARD.decode(input.as_ref())?)?,
            Self::Base64UrlSafe => {
                String::from_utf8(general_purpose::URL_SAFE.decode(input.as_ref())?)?
            }
        };

        stack.push(s.into());
        Ok(stack)
    }
}
