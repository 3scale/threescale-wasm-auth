#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::upstream::Upstream;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct System {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub upstream: Upstream,
    pub token: String,
}

impl System {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub const fn upstream(&self) -> &Upstream {
        &self.upstream
    }

    pub fn token(&self) -> &str {
        self.token.as_str()
    }
}
