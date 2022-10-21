use serde::{Deserialize, Serialize};

use crate::upstream::Upstream;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Backend {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub upstream: Upstream,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
}

impl Backend {
    #[allow(dead_code)]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub const fn upstream(&self) -> &Upstream {
        &self.upstream
    }

    #[allow(dead_code)]
    pub const fn extensions(&self) -> Option<&Vec<String>> {
        self.extensions.as_ref()
    }
}
