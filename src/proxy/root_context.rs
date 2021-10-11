use proxy_wasm::traits::{Context, RootContext};
use proxy_wasm::types::{BufferType, ChildContext};

use core::time::Duration;
use std::time::SystemTime;

use crate::configuration::Configuration;
use crate::log::IdentLogger;
use crate::util::rand::thread_rng::{thread_rng_init_fallible, ThreadRng};
use crate::util::serde::ErrorLocation;

use super::config_fetcher::FetcherState;
use super::http_context::HttpAuthThreescale;

const MIN_SYNC: u64 = 20;

pub(super) struct RootAuthThreescale {
    vm_configuration: Option<Vec<u8>>,
    configuration: Option<Configuration>,
    rng: ThreadRng,
    context_id: u32,
    id: u32,
    log_id: String,
    fetcher: FetcherState,
    config_deadline: SystemTime,
}

impl RootAuthThreescale {
    pub const fn new() -> Self {
        Self {
            vm_configuration: None,
            configuration: None,
            rng: ThreadRng,
            context_id: 0,
            id: 0,
            log_id: String::new(),
            fetcher: FetcherState::Inactive,
            config_deadline: std::time::UNIX_EPOCH,
        }
    }
}

impl IdentLogger for RootAuthThreescale {
    fn ident(&self) -> &str {
        self.log_id.as_str()
    }
}

impl Context for RootAuthThreescale {
    fn on_registered(&mut self, context_id: u32) {
        use crate::log::LogLevel;
        use core::fmt::Write as _;

        self.context_id = context_id;
        // Initialize the PRNG for this thread in the root context
        // This only needs to happen once per thread. Since we are
        // single-threaded, this means it just needs to happen once.
        self.rng = match thread_rng_init_fallible(self, context_id) {
            Ok(r) => r,
            Err(e) => {
                // No access yet to an initialized identity for logging, use raw API.
                let _ = proxy_wasm::hostcalls::log(
                    LogLevel::Critical.into(),
                    &format!(
                        "{}: FATAL: failed to initialize thread pseudo RNG: {}",
                        context_id, e
                    ),
                );
                panic!("failed to initialize thread pseudo RNG: {}", e);
            }
        };

        self.id = self.rng.next_u32();
        write!(
            &mut self.log_id,
            "(root/{}) {:>10}",
            self.context_id, self.id
        )
        .unwrap();

        info!(self, "Testing random values:");

        // Could as well use `self.rng.next_u32()` or `self.rng.u32()`,
        // but `with` is more efficient for multiple sequential calls
        // by amortizing a single access to TLS and initialization check.
        self.rng.with(|r| {
            for _ in 0..10 {
                use rand::RngCore as _;
                let n = r.next_u32();
                info!(self, "{} ({:#b})", n, n);
            }
        });

        info!(self, "registered");
    }

    fn on_http_call_response(
        &mut self,
        token_id: u32,
        _num_headers: usize,
        _body_size: usize,
        _num_trailers: usize,
    ) {
        match &self.fetcher {
            FetcherState::Inactive => {
                // This could be due to receiving a new configuration mid-flight of a system request
                // We'll just ignore the response and start over again, as system and/or config
                // could have changed.
                debug!(
                    self,
                    "ignoring call response due to configuration fetcher being inactive"
                );
            }
            FetcherState::ConfigFetched => {
                warn!(self, "config already fetched but got a response !?");
            }
            FetcherState::FetchingConfig(call_id, svc_id) => {
                if *call_id != token_id {
                    warn!(self, "seen a call response without the right token id");
                }
                debug!(self, "received response for config for service {}", svc_id);
                match self.get_http_call_response_body(0, usize::MAX) {
                    Some(body) => {
                        info!(self, "got config!");
                        //let svclist = straitjacket::api::v0::service::LIST;
                        let configep = straitjacket::api::v0::service::proxy::configs::LATEST;
                        let body_s = String::from_utf8_lossy(body.as_slice());
                        //let res = svclist.parse_str(body_s.as_ref());
                        let res = configep.parse_str(body_s.as_ref());
                        match res {
                            Ok(config) => {
                                info!(self, "config: {:#?}", config);
                                self.fetcher = FetcherState::ConfigFetched;
                            }
                            Err(e) => {
                                error!(self, "failed to parse config: {}", e);
                                match serde_json::from_str::<serde_json::Value>(body_s.as_ref())
                                    .and_then(|json_val| {
                                        serde_json::to_string_pretty(&json_val)
                                            .or_else(|_| serde_json::to_string(&json_val))
                                    }) {
                                    Ok(json) => error!(self, "JSON error response:\n{}", json),
                                    Err(_) => {
                                        error!(self, "RAW error response:\n{}", body_s.as_ref())
                                    }
                                }
                                // TODO FIXME Try to retrieve mapping rules at the very least.
                            }
                        }
                    }
                    None => {
                        info!(self, "FAILED TO GET list of mapping rules!");
                    }
                }
            }
            FetcherState::FetchingConfigs(call_id) => {
                if *call_id != token_id {
                    warn!(self, "seen a call response without the right token id");
                }
            }
            FetcherState::Error(_) => todo!(),
        }
    }
}

impl RootContext for RootAuthThreescale {
    fn on_vm_start(&mut self, vm_configuration_size: usize) -> bool {
        info!(
            self,
            "{}",
            concat!(
                env!("CARGO_PKG_NAME"),
                " version ",
                env!("CARGO_PKG_VERSION"),
                " booting up."
            )
        );
        info!(
            self,
            "on_vm_start: vm_configuration_size is {}", vm_configuration_size
        );
        let vm_config = proxy_wasm::hostcalls::get_buffer(
            BufferType::VmConfiguration,
            0,
            vm_configuration_size,
        );

        if let Err(e) = vm_config {
            error!(
                self,
                "on_vm_start: error retrieving VM configuration: {:#?}", e
            );
            return false;
        }

        self.vm_configuration = vm_config.unwrap();

        if let Some(conf) = self.vm_configuration.as_ref() {
            info!(
                self,
                "on_vm_start: VM configuration is {}",
                core::str::from_utf8(conf).unwrap()
            );
        } else {
            // We currently don't need a VM config, so don't
            // fail if there's none.
            warn!(self, "on_vm_start: empty VM config");
        }

        true
    }

    fn on_configure(&mut self, plugin_configuration_size: usize) -> bool {
        use core::convert::TryFrom;

        info!(
            self,
            "on_configure: plugin_configuration_size is {}", plugin_configuration_size
        );

        let conf = match proxy_wasm::hostcalls::get_buffer(
            BufferType::PluginConfiguration,
            0,
            plugin_configuration_size,
        ) {
            Ok(Some(conf)) => conf,
            Ok(None) => {
                warn!(self, "empty module configuration - module has no effect");
                return true;
            }
            Err(e) => {
                error!(self, "error retrieving module configuration: {:#?}", e);
                return false;
            }
        };

        debug!(self, "loaded raw config");

        let conf = match Configuration::try_from(conf.as_slice()) {
            Ok(conf) => conf,
            Err(e) => {
                if let Ok(el) = ErrorLocation::try_from(&e) {
                    let conf_str = String::from_utf8_lossy(conf.as_slice());
                    for line in el.error_lines(conf_str.as_ref(), 4, 4) {
                        error!(self, "{}", line);
                    }
                } else {
                    // not a configuration syntax/data error (ie. programmatic)
                    error!(self, "fatal configuration error: {:#?}", e);
                }
                return false;
            }
        };

        self.configuration = conf.into();
        info!(
            self,
            "on_configure: plugin configuration {:#?}", self.configuration
        );

        // cancel any previous work updating configurations
        self.fetcher = FetcherState::Inactive;

        let _ = self.set_next_tick();

        true
    }

    fn on_create_child_context(&mut self, context_id: u32) -> Option<ChildContext> {
        info!(self, "creating new context {}", context_id);
        let ctx = HttpAuthThreescale {
            context_id,
            configuration: self.configuration.as_ref().unwrap().clone(),
            id: self.rng.next_u32(),
            log_id: format!("{} ({}/http)", self.id, self.context_id),
        };

        Some(ChildContext::HttpContext(Box::new(ctx)))
    }

    fn on_tick(&mut self) {
        debug!(self, "executing on_tick");
        if let Some(config) = self.get_configuration() {
            if let Some(sys) = self.get_system_config() {
                let current_time = self.get_current_time();
                if current_time < self.config_deadline {
                    warn!(
                        self,
                        "on_tick running while the configuration is still valid"
                    );
                }

                let upstream = sys.upstream();
                let new_state = match &self.fetcher {
                    FetcherState::Inactive => {
                        if let Some(services) = config.services() {
                            info!(self, "fetching rules for {} services", services.len());
                            let ruleslist =
                                straitjacket::api::v0::service::proxy::mapping_rules::LIST;
                            let method = ruleslist.method().as_str();
                            let qs = format!("access_token={}", sys.token());
                            let results = services.iter().map(|svc| {
                                let svc_id = svc.id();
                                let path = ruleslist.path(&[svc_id]);
                                if let Ok(path) = path {
                                    match upstream.call(
                                        self,
                                        path.as_str(),
                                        method,
                                        vec![],
                                        Some(qs.as_str()),
                                        None,
                                        None,
                                        None,
                                    ) {
                                        Ok(call_id) => {
                                            debug!(self, "fetching rules list for service {}", svc_id);
                                            FetcherState::FetchingConfig(call_id, svc_id.to_string())
                                        }
                                        Err(e) => {
                                            error!(
                                                self,
                                                "failed to initiate fetch of mapping rules list for service {}: {}", svc_id, e
                                            );
                                            FetcherState::Error(e.into())
                                        }
                                    }
                                } else {
                                    critical!(
                                        self,
                                        "failed to obtain path for mapping rules list API: {}",
                                        path.unwrap_err()
                                    );
                                    FetcherState::Inactive
                                }
                            });
                            FetcherState::Inactive
                        } else {
                            FetcherState::Inactive
                        }
                    }
                    FetcherState::FetchingConfig(token_id, svc_id) => {
                        info!(
                            self,
                            "still fetching rules!? - token_id: {}, svc_id: {}", token_id, svc_id
                        );
                        unimplemented!()
                    }
                    FetcherState::ConfigFetched => {
                        info!(self, "fetched rules");
                        FetcherState::Inactive
                    }
                    FetcherState::FetchingConfigs(token_id) => {
                        info!(self, "still fetching configs!? - token_id: {}", token_id);
                        unimplemented!()
                    }
                    FetcherState::Error(_) => todo!(),
                };

                self.fetcher = new_state;
                self.set_next_tick();
            }
        }
    }
}

impl RootAuthThreescale {
    pub fn get_configuration(&self) -> Option<&crate::configuration::api::v1::Configuration> {
        self.configuration.as_ref().map(|conf| conf.get())
    }

    pub fn get_system_config(&self) -> Option<&crate::threescale::System> {
        self.get_configuration().map(|conf| conf.system()).flatten()
    }

    fn get_next_tick(&self) -> Option<(Duration, Duration)> {
        self.get_system_config().map(|sys| {
            let jitter = self.rng.next_u32() as u64 & 0x0F; // add 0-15 seconds on top

            // ensure we only do this at most once per minute, and at least not within the timeout
            let original_ttl = core::cmp::min(
                sys.ttl(),
                core::cmp::max(Duration::from_secs(MIN_SYNC), sys.upstream().timeout),
            );
            let ttl = original_ttl.saturating_add(Duration::from_secs(jitter));
            info!(
                self,
                "system configuration TTL set to {} seconds",
                ttl.as_secs()
            );

            (ttl, original_ttl)
        })
    }

    pub fn set_next_tick(&mut self) -> Option<Duration> {
        self.get_next_tick().map(|(tick, original_ttl)| {
            self.config_deadline = self
                .get_current_time()
                .checked_add(original_ttl)
                .unwrap_or(std::time::UNIX_EPOCH);
            self.set_tick_period(tick);
            tick
        })
    }
}
