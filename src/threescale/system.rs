#![allow(dead_code)]

use core::time::Duration;

use serde::{Deserialize, Serialize};

use crate::upstream::Upstream;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct System {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub upstream: Upstream,
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
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

    pub fn ttl(&self) -> Duration {
        let ttl = self.ttl.unwrap_or(300);
        Duration::from_secs(ttl)
    }
}
