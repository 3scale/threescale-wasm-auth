use serde::{Deserialize, Serialize};

use super::{Credentials, MappingRule};
use crate::util::glob::GlobPatternSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    pub token: String,
    #[serde(default)]
    pub authorities: GlobPatternSet,
    pub credentials: Credentials,
    pub mapping_rules: Vec<MappingRule>,
}

impl Service {
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    pub fn token(&self) -> &str {
        self.token.as_str()
    }

    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    pub fn mapping_rules(&self) -> &Vec<MappingRule> {
        self.mapping_rules.as_ref()
    }

    pub fn match_authority(&self, authority: &str) -> bool {
        self.authorities.is_match(authority)
    }
}
