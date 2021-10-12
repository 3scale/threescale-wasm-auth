use proxy_wasm::traits::{Context, RootContext};
use proxy_wasm::types::{BufferType, ChildContext};

use core::time::Duration;
use std::time::SystemTime;

use crate::configuration::Configuration;
use crate::log::IdentLogger;
use crate::proxy::config_fetcher::{self, ConfigFetcher, Fetcher, FetcherState};
use crate::threescale::{MappingRule, Usage};
use crate::util::rand::thread_rng::{thread_rng_init_fallible, ThreadRng};
use crate::util::serde::ErrorLocation;

use threescalers::http::mapping_rule::{Method, RestRule};

use super::http_context::HttpAuthThreescale;

const MIN_SYNC: u64 = 20;

pub(super) struct RootAuthThreescale {
    vm_configuration: Option<Vec<u8>>,
    configuration: Option<Configuration>,
    rng: ThreadRng,
    context_id: u32,
    id: u32,
    log_id: String,
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

        // Initialize the config fetcher
        let _ = config_fetcher::fetcher_init();

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
        if let Some(sys) = self.get_system_config() {
            let upstream = sys.upstream();
            let qs_params = format!("access_token={}", sys.token());

            let idx = Fetcher::with(|vcf| {
                vcf.iter_mut()
                    .position(|cf| cf.token_id().map(|t| t == token_id).unwrap_or(false))
            });

            if idx.is_none() {
                error!(self, "config fetcher for token id {} not found!", token_id);
                return;
            }

            let idx = idx.unwrap();
            Fetcher::with(|vcf| {
                let cf = vcf.get_mut(idx).unwrap();
                let _a = cf.response(self, token_id, upstream, qs_params.as_str());
            });

            // update mapping rules
            info!(self, "updating mapping rules using fetched config");
            Fetcher::with(|vcf| {
                let cf = vcf.get_mut(idx).unwrap();
                let mut rules_updated: bool = false;

                let config = self.configuration.as_mut().map(|config| config.get_mut());
                let services_op = config.map(|config| config.services.as_mut()).flatten();
                let services = services_op.unwrap(); // cannot make a callout without services

                if let Some(service) = services.iter_mut().find(|sv| sv.id() == cf.service_id()) {
                    let mut latest_service = cf.service().clone();
                    match cf.state() {
                        FetcherState::ConfigFetched(proxy_config) => {
                            let proxy_config = proxy_config.get_inner().item();
                            let proxy_rules = proxy_config.content().proxy().mapping_rules();

                            for proxy_rule in proxy_rules {
                                let metric_name = proxy_rule.metric_system_name.clone();

                                latest_service.mapping_rules.push(MappingRule {
                                    rule: RestRule::new(
                                        Method::from(proxy_rule.http_method.as_ref()),
                                        proxy_rule.pattern.clone(),
                                    )
                                    .unwrap(),
                                    usages: vec![Usage {
                                        name: metric_name.unwrap_or_else(|| "Hits".into()),
                                        delta: proxy_rule.delta as i64,
                                    }],
                                    last: proxy_rule.last,
                                })
                            }
                            rules_updated = true;
                        }
                        FetcherState::RulesFetched(ref rules) => {
                            let mapping_rules = rules.get_inner();

                            for mapping_rule_tag in mapping_rules {
                                let mapping_rule_inner = mapping_rule_tag.get_inner();
                                let mapping_rule = mapping_rule_inner.item();
                                let metric_name = mapping_rule.metric_system_name.clone();

                                latest_service.mapping_rules.push(MappingRule {
                                    rule: RestRule::new(
                                        Method::from(mapping_rule.http_method.as_ref()),
                                        mapping_rule.pattern.clone(),
                                    )
                                    .unwrap(),
                                    usages: vec![Usage {
                                        name: metric_name.unwrap_or_else(|| "Hits".into()),
                                        delta: mapping_rule.delta as i64,
                                    }],
                                    last: mapping_rule.last,
                                })
                            }
                            rules_updated = true;
                        }
                        _ => (),
                    }
                    if rules_updated {
                        *service = latest_service;
                        cf.set_state(FetcherState::Inactive);
                    }
                }
            });
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
        Fetcher::clear();

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
                    return;
                }

                if let Some(services) = config.services() {
                    let upstream = sys.upstream();
                    let qs = format!("access_token={}", sys.token());

                    Fetcher::with(|vcf| {
                        vcf.sort_unstable();
                        for service in services {
                            let idx = match vcf
                                .binary_search_by_key(&service.id(), |cf| cf.service_id())
                            {
                                Ok(idx) => idx,
                                Err(idx) => {
                                    let cf = ConfigFetcher::new(service.clone());
                                    vcf.insert(idx, cf);
                                    idx
                                }
                            };
                            let cf = vcf.get_mut(idx).unwrap();
                            cf.call(self, upstream, qs.as_str());
                        }
                    });
                }

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
