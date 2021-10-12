use crate::threescale::{Environment, Service};
use crate::upstream::Upstream;

use super::root_context::RootAuthThreescale;

mod thread_local;
pub use thread_local::{fetcher_init, fetcher_init_fallible, Fetcher};

use proxy_wasm::traits::RootContext;
pub use straitjacket::api::v0::service::proxy;
use straitjacket::resources::http::endpoint::Endpoint;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config fetching failed")]
    Failed,
    #[error("client error: {0}")]
    Client(#[from] threescalers::Error),
    //#[error("endpoint error: {0}")]
    //Endpoint(Box<dyn std::error::Error + Send + Sync>),
    #[error("error: {0}")]
    Boxed(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug)]
pub enum FetcherState {
    Inactive,
    FetchingConfig(u32),
    ConfigFetched(Box<proxy::configs::ProxyConfig>),
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
    service: Service, // hold service from static config
    state: FetcherState,
}

impl PartialEq for ConfigFetcher {
    fn eq(&self, other: &Self) -> bool {
        self.service.id == other.service.id
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
    const CONFIG_EP: Endpoint<'static, 'static, proxy::configs::ProxyConfig> =
        proxy::configs::LATEST;
    const RULES_EP: Endpoint<'static, 'static, proxy::mapping_rules::MappingRules> =
        proxy::mapping_rules::LIST;

    pub fn new(service: Service) -> Self {
        Self {
            service,
            state: FetcherState::Inactive,
        }
    }

    pub fn token_id(&self) -> Option<u32> {
        self.state.token_id()
    }

    pub fn service_id(&self) -> &str {
        self.service.id.as_str()
    }

    pub fn service(&self) -> &Service {
        &self.service
    }

    pub fn environment(&self) -> &Environment {
        &self.service.environment
    }

    pub fn state(&self) -> &FetcherState {
        &self.state
    }

    pub fn set_state(&mut self, new_state: FetcherState) {
        self.state = new_state;
    }

    pub(super) fn fetch_endpoint<E>(
        &self,
        ctx: &RootAuthThreescale,
        upstream: &Upstream,
        qs_params: &str,
        endpoint: Endpoint<'_, '_, E>,
        args: &[&str],
    ) -> Result<u32, Error> {
        let method = endpoint.method().as_str();
        let path = endpoint.path(args);
        if let Ok(path) = path {
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
                Ok(call_id) => Ok(call_id),
                Err(e) => {
                    error!(
                        ctx,
                        "failed to initiate fetch of configuration data for {}: {}",
                        self.service_id(),
                        e
                    );
                    Err(Error::Failed)
                }
            }
        } else {
            critical!(
                ctx,
                "failed to obtain path for endpoint: {}",
                path.unwrap_err()
            );
            Err(Error::Failed)
        }
    }

    pub(super) fn call(
        &mut self,
        ctx: &RootAuthThreescale,
        upstream: &Upstream,
        qs_params: &str,
    ) -> u32 {
        let new_state = match &self.state {
            FetcherState::Inactive | FetcherState::Error(_) => {
                let state = match self.fetch_endpoint(
                    ctx,
                    upstream,
                    qs_params,
                    Self::CONFIG_EP,
                    &[self.service_id(), self.environment().as_str()],
                ) {
                    Ok(call_id) => FetcherState::FetchingConfig(call_id),
                    Err(e) => FetcherState::Error(e),
                };
                state.into()
            }
            FetcherState::FetchingConfig(token_id) => {
                info!(
                    ctx,
                    "still fetching config!? - token_id: {}, svc_id: {}",
                    token_id,
                    self.service_id()
                );
                FetcherState::FetchingConfig(*token_id).into()
            }
            FetcherState::FetchingRules(token_id) => {
                info!(
                    ctx,
                    "still fetching rules!? - token_id: {}, svc_id: {}",
                    token_id,
                    self.service_id()
                );
                FetcherState::FetchingRules(*token_id).into()
            }
            _ => {
                info!(ctx, "data has been retrieved");
                None
            }
        };

        if let Some(new_state) = new_state {
            self.set_state(new_state);
        }
        42
    }

    fn parsing_error(ctx: &RootAuthThreescale, body: &str, e: Box<dyn std::error::Error>) {
        error!(ctx, "failed to parse config: {}", e);
        match serde_json::from_str::<serde_json::Value>(body).and_then(|json_val| {
            serde_json::to_string_pretty(&json_val).or_else(|_| serde_json::to_string(&json_val))
        }) {
            Ok(json) => error!(ctx, "JSON error response:\n{}", json),
            Err(_) => {
                error!(ctx, "RAW error response:\n{}", body)
            }
        }
    }

    pub(super) fn response(
        &mut self,
        ctx: &RootAuthThreescale,
        token_id: u32,
        upstream: &Upstream,
        qs_params: &str,
    ) -> u32 {
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
                let config = match (ctx as &dyn RootContext)
                    .get_http_call_response_body(0, usize::MAX)
                {
                    Some(body) => {
                        info!(
                            ctx,
                            "got config response for service {}!",
                            self.service_id()
                        );
                        let configep = straitjacket::api::v0::service::proxy::configs::LATEST;
                        let body_s = String::from_utf8_lossy(body.as_slice());
                        configep.parse_str(body_s.as_ref()).map_err(|e| {
                            Self::parsing_error(ctx, body_s.as_ref(), e);
                            Error::Failed
                        })
                    }
                    None => {
                        info!(ctx, "response contained no body - failed to get configuration for service {}", self.service_id());
                        Err(Error::Failed)
                    }
                };
                let state = match config {
                    Ok(config) => FetcherState::ConfigFetched(Box::new(config)),
                    Err(_e) => {
                        // Try to fetch rules
                        match self.fetch_endpoint(
                            ctx,
                            upstream,
                            qs_params,
                            Self::RULES_EP,
                            &[self.service_id()],
                        ) {
                            Ok(call_id) => FetcherState::FetchingRules(call_id),
                            Err(e) => FetcherState::Error(e),
                        }
                    }
                };
                self.state = state;
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
