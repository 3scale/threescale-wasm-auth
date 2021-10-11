use crate::upstream::Upstream;

use super::root_context::RootAuthThreescale;

use straitjacket::api::v0::service::proxy;
use straitjacket::resources::http::endpoint::Endpoint;

pub enum FetcherState {
    Inactive,
    FetchingConfig(u32),
    ConfigFetched(proxy::configs::Config),
    FetchingRules(u32),
    RulesFetched(proxy::mapping_rules::MappingRules),
    Error(Error),
}

pub struct ConfigFetcher {
    upstream: Upstream,
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

    pub fn new(upstream: Upstream, service_id: String, environment: String) -> Self {
        Self {
            upstream,
            service_id,
            environment,
            state: FetcherState::Inactive,
        }
    }

    pub fn call(&self, ctx: &RootAuthThreescale, qs_params: &str) -> u32 {
        let new_state = match self.state {
            FetcherState::Inactive => {
                let config_ep = Self::CONFIG_EP;
                let method = config_ep.method().as_str();
                let path =
                    Self::CONFIG_EP.path(&[self.service_id.as_str(), self.environment.as_str()]);
                let state = if let Ok(path) = path {
                    match self.upstream.call(
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
                            FetcherState::FetchingConfig(call_id, self.service_id.clone())
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
                FetcherState::Inactive
            }
            FetcherState::FetchingConfig(token_id, svc_id) => {
                info!(
                    ctx,
                    "still fetching config!? - token_id: {}, svc_id: {}", token_id, svc_id
                );
                unimplemented!()
            }
            FetcherState::ConfigFetched => {
                info!(ctx, "fetched rules");
                FetcherState::Inactive
            }
            FetcherState::FetchingConfigs(token_id) => {
                info!(ctx, "still fetching configs!? - token_id: {}", token_id);
                unimplemented!()
            }
            FetcherState::Error(_) => todo!(),
        };

        42
    }

    pub fn response(ctx: &RootAuthThreescale) -> u32 {
        0
    }
}
