use serde::{Deserialize, Serialize};
use threescalers::http::mapping_rule::{Method, RestRule};

use super::Usage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingRule {
    #[serde(flatten)]
    pub rule: RestRule,
    pub usages: Vec<Usage>,
    #[serde(default)]
    pub last: bool,
}

impl MappingRule {
    #[allow(dead_code)]
    pub fn method(&self) -> &Method {
        self.rule.method()
    }

    #[allow(dead_code)]
    pub fn pattern(&self) -> String {
        self.rule.pattern()
    }

    pub fn usages(&self) -> &Vec<Usage> {
        self.usages.as_ref()
    }

    #[allow(dead_code)]
    pub fn match_pattern(&self, pattern: &str) -> bool {
        self.rule.matches_path_with_qs(pattern)
    }

    #[allow(dead_code)]
    pub fn match_method(&self, method: &Method) -> bool {
        self.method() == method
    }

    pub fn is_match(&self, method: &Method, pattern: &str) -> bool {
        self.rule.matches(method, pattern)
    }

    pub fn is_last(&self) -> bool {
        self.last
    }
}
