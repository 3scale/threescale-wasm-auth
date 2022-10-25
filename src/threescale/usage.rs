use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub name: String,
    pub delta: i64,
}

impl Usage {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub const fn delta(&self) -> i64 {
        self.delta
    }
}
