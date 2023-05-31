use serde::{Deserialize, Serialize};

use crate::configuration::MissingError;
use crate::threescale::{Backend, Service, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache {
    pub ttl: Option<u64>,
    pub jitter: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "3scale")]
pub struct Configuration {
    pub system: Option<System>,
    pub backend: Option<Backend>,
    pub services: Option<Vec<Service>>,
    pub cache: Option<Cache>,
    // pass request to the next filter in the chain
    pub passthrough_metadata: Option<bool>,
}

impl Configuration {
    #[allow(dead_code)]
    pub const fn system(&self) -> Option<&System> {
        self.system.as_ref()
    }

    pub const fn backend(&self) -> Option<&Backend> {
        self.backend.as_ref()
    }

    pub const fn services(&self) -> Option<&Vec<Service>> {
        self.services.as_ref()
    }

    pub fn get_backend(&self) -> Result<&Backend, MissingError> {
        self.backend().ok_or(MissingError::Backend)
    }

    pub fn get_services(&self) -> Result<&Vec<Service>, MissingError> {
        self.services().ok_or(MissingError::Services)
    }
}
