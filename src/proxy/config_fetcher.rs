use crate::upstream::Upstream;

use super::root_context::RootAuthThreescale;

mod thread_local;
pub use thread_local::fetcher_with;
pub use thread_local::Fetcher;

use proxy_wasm::traits::RootContext;
use straitjacket::api::v0::service::proxy;
use straitjacket::resources::http::endpoint::Endpoint;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config fetching failed")]
    Failed,
    #[error("client error")]
    Threescalers(#[from] threescalers::Error),
    #[error("error: {0}")]
    Boxed(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug)]
pub enum FetcherState {
    Inactive,
    FetchingConfig(u32),
    ConfigFetched(proxy::configs::Config),
    FetchingRules(u32),
    RulesFetched(proxy::mapping_rules::MappingRules),
    Error(Error),
}

impl FetcherState {
    pub fn token_id(&self) -> Option<u32> {
        match self {
            Self::FetchingConfig(st) | Self::FetchingRules(st) => Some(*st),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ConfigFetcher {
    service_id: String,
    environment: String,
    state: FetcherState,
}

impl PartialEq for ConfigFetcher {
    fn eq(&self, other: &Self) -> bool {
        self.service_id == other.service_id
    }
}

impl Eq for ConfigFetcher {}

impl PartialOrd for ConfigFetcher {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.service_id().partial_cmp(other.service_id())
    }
}

impl Ord for ConfigFetcher {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.service_id().cmp(other.service_id())
    }
}

impl ConfigFetcher {
    const CONFIG_EP: Endpoint<'static, 'static, proxy::configs::Config> = proxy::configs::LATEST;
    const RULES_EP: Endpoint<'static, 'static, proxy::mapping_rules::MappingRules> =
        proxy::mapping_rules::LIST;

    pub fn new(service_id: String, environment: String) -> Self {
        Self {
            service_id,
            environment,
            state: FetcherState::Inactive,
        }
    }

    pub fn token_id(&self) -> Option<u32> {
        self.state.token_id()
    }

    pub fn service_id(&self) -> &str {
        self.service_id.as_str()
    }

    pub fn call(&mut self, ctx: &RootAuthThreescale, upstream: &Upstream, qs_params: &str) -> u32 {
        let new_state = match &self.state {
            FetcherState::Inactive | FetcherState::Error(_) => {
                let config_ep = Self::CONFIG_EP;
                let method = config_ep.method().as_str();
                let path =
                    Self::CONFIG_EP.path(&[self.service_id.as_str(), self.environment.as_str()]);
                let state = if let Ok(path) = path {
                    match upstream.call(
                        ctx,
                        path.as_str(),
                        method,
                        vec![],
                        Some(qs_params),
                        None,
                        None,
                        None,
                    ) {
                        Ok(call_id) => {
                            debug!(
                                ctx,
                                "fetching config for service {}",
                                self.service_id.as_str()
                            );
                            FetcherState::FetchingConfig(call_id)
                        }
                        Err(e) => {
                            error!(
                                ctx,
                                "failed to initiate fetch of config for service {}: {}",
                                self.service_id.as_str(),
                                e
                            );
                            FetcherState::Error(e.into())
                        }
                    }
                } else {
                    critical!(
                        ctx,
                        "failed to obtain path for latest config endpoint: {}",
                        path.unwrap_err()
                    );
                    FetcherState::Inactive
                };
                state.into()
            }
            FetcherState::FetchingConfig(token_id) => {
                info!(
                    ctx,
                    "still fetching config!? - token_id: {}, svc_id: {}",
                    token_id,
                    self.service_id.as_str()
                );
                FetcherState::FetchingConfig(*token_id).into()
            }
            FetcherState::FetchingRules(token_id) => {
                info!(
                    ctx,
                    "still fetching rules!? - token_id: {}, svc_id: {}",
                    token_id,
                    self.service_id.as_str()
                );
                FetcherState::FetchingRules(*token_id).into()
            }
            //FetcherState::ConfigFetched(_) => todo!(),
            //FetcherState::RulesFetched(_) => todo!(),
            _ => {
                info!(ctx, "data has been retrieved");
                None
            }
        };

        if let Some(new_state) = new_state {
            self.state = new_state;
        }
        42
    }

    pub fn response(&mut self, ctx: &RootAuthThreescale, token_id: u32) -> u32 {
        match self.state {
            FetcherState::Inactive => {
                // This could be due to receiving a new configuration mid-flight of a system request
                // We'll just ignore the response and start over again, as system and/or config
                // could have changed.
                debug!(
                    ctx,
                    "ignoring call response due to configuration fetcher being inactive"
                );
            }
            FetcherState::FetchingConfig(call_id) => {
                if call_id != token_id {
                    warn!(ctx, "seen a call response without the right token id");
                }
                debug!(
                    ctx,
                    "received response for config for service {}",
                    self.service_id()
                );
                match (ctx as &dyn RootContext).get_http_call_response_body(0, usize::MAX) {
                    Some(body) => {
                        info!(ctx, "got config!");
                        let configep = straitjacket::api::v0::service::proxy::configs::LATEST;
                        let body_s = String::from_utf8_lossy(body.as_slice());
                        let res = configep.parse_str(body_s.as_ref());
                        match res {
                            Ok(config) => {
                                info!(ctx, "config: {:#?}", config);
                                self.state = FetcherState::ConfigFetched(config);
                            }
                            Err(e) => {
                                error!(ctx, "failed to parse config: {}", e);
                                match serde_json::from_str::<serde_json::Value>(body_s.as_ref())
                                    .and_then(|json_val| {
                                        serde_json::to_string_pretty(&json_val)
                                            .or_else(|_| serde_json::to_string(&json_val))
                                    }) {
                                    Ok(json) => error!(ctx, "JSON error response:\n{}", json),
                                    Err(_) => {
                                        error!(ctx, "RAW error response:\n{}", body_s.as_ref())
                                    }
                                }
                                // TODO FIXME Try to retrieve mapping rules at the very least.
                            }
                        }
                    }
                    None => {
                        info!(ctx, "FAILED TO GET list of mapping rules!");
                    }
                }
            }
            FetcherState::ConfigFetched(ref _cfg) => {
                warn!(ctx, "config already fetched but got a response !?");
            }
            FetcherState::RulesFetched(ref _rules) => {
                warn!(ctx, "rules already fetched but got a response !?");
            }
            FetcherState::FetchingRules(call_id) => {
                if call_id != token_id {
                    warn!(ctx, "seen a call response without the right token id");
                }
                debug!(
                    ctx,
                    "received response for config for service {}",
                    self.service_id()
                );
                match (ctx as &dyn RootContext).get_http_call_response_body(0, usize::MAX) {
                    Some(body) => {
                        info!(ctx, "got config!");
                        let rulesep = straitjacket::api::v0::service::proxy::mapping_rules::LIST;
                        let body_s = String::from_utf8_lossy(body.as_slice());
                        let res = rulesep.parse_str(body_s.as_ref());
                        match res {
                            Ok(rules) => {
                                info!(ctx, "rules: {:#?}", rules);
                                self.state = FetcherState::RulesFetched(rules);
                            }
                            Err(e) => {
                                error!(ctx, "failed to parse rules: {}", e);
                                match serde_json::from_str::<serde_json::Value>(body_s.as_ref())
                                    .and_then(|json_val| {
                                        serde_json::to_string_pretty(&json_val)
                                            .or_else(|_| serde_json::to_string(&json_val))
                                    }) {
                                    Ok(json) => error!(ctx, "JSON error response:\n{}", json),
                                    Err(_) => {
                                        error!(ctx, "RAW error response:\n{}", body_s.as_ref())
                                    }
                                }
                                self.state = FetcherState::Error(Error::Failed);
                            }
                        }
                    }
                    None => {
                        info!(ctx, "FAILED TO GET list of mapping rules!");
                    }
                }
            }
            FetcherState::Error(_) => todo!(),
        }
        0
    }
}

mod imp {
    use std::cell::RefCell;
    use std::sync::Once;

    use super::*;

    thread_local! {
        static FETCHER: RefCell<Option<Vec<ConfigFetcher>>> = RefCell::new(None);
        static FETCHER_INIT: Once = Once::new();
    }

    pub(super) fn initialize() -> Result<(), Error> {
        FETCHER_INIT.with(|once| {
            let mut res = Ok(());
            once.call_once(|| {
                res = FETCHER.with(|fetcher| {
                    let new_fetcher: Vec<ConfigFetcher> = vec![];
                    let _ = fetcher.borrow_mut().replace(new_fetcher);
                    Ok(())
                });
            });
            res
        })
    }
}
