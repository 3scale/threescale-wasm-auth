use core::time::Duration;

use proxy_wasm::traits::{Context, RootContext};
use proxy_wasm::types::{BufferType, ChildContext};

use crate::configuration::Configuration;
use crate::log::IdentLogger;
use crate::util::rand::thread_rng::{thread_rng_init_fallible, ThreadRng};
use crate::util::serde::ErrorLocation;

use super::http_context::HttpAuthThreescale;

pub(super) struct RootAuthThreescale {
    vm_configuration: Option<Vec<u8>>,
    configuration: Option<Configuration>,
    dynconf_cas: Option<u32>,
    rng: ThreadRng,
    context_id: u32,
    id: u32,
    log_id: String,
}

impl RootAuthThreescale {
    pub const fn new() -> Self {
        Self {
            vm_configuration: None,
            configuration: None,
            dynconf_cas: None,
            rng: ThreadRng,
            context_id: 0,
            id: 0,
            log_id: String::new(),
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
        let status = self
            .get_http_call_response_headers()
            .into_iter()
            .find_map(|(key, val)| {
                if key.as_str() == ":status" {
                    Some(val)
                } else {
                    None
                }
            });
        info!(
            self,
            "threescale_wasm_auth: root_ctx: on_http_call_response: token id is {}, status {:?}",
            token_id,
            status
        );
    }
}

impl RootContext for RootAuthThreescale {
    fn on_vm_start(&mut self, vm_configuration_size: usize) -> bool {
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
            true
        } else {
            warn!(self, "on_vm_start: empty VM config");
            false
        }
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

        self.set_tick_period(Duration::from_secs(10));

        true
    }

    fn on_create_child_context(&mut self, context_id: u32) -> Option<ChildContext> {
        info!(
            self,
            "threescale_wasm_auth: creating new context {}", context_id
        );
        let ctx = HttpAuthThreescale {
            context_id,
            configuration: self.configuration.as_ref().unwrap().clone(),
            id: self.rng.next_u32(),
            log_id: format!("{} ({}/http)", self.id, self.context_id),
        };
        let boxctx = Box::new(ctx);
        //self.children.push(boxctx.as_ref());
        let childctx = ChildContext::HttpContext(boxctx);

        Some(childctx)
    }

    fn on_tick(&mut self) {
        use core::convert::TryFrom;

        warn!(self, "{}: executing on_tick for root ctx", self.context_id);

        let id = self.rng.next_u32();

        if let Some(cfg) = self.configuration.as_ref() {
            let config = cfg.get();
            if let Some(system) = config.system() {
                let upstream = system.upstream();
                let url = &upstream.url;
                let key_shared_mem = format!("lock_{}", url.as_str());
                let (mut value, mut cas) = self.get_shared_data(key_shared_mem.as_str());
                if let Some(oid) = cas {
                    // wait 10-20s
                    let next_period = self.rng.next_u32() % 10_000 + 10_000;
                    info!(self,
                        "{}: {} owned by ctx with rand id {}, bailing temporarily for {} ms: value => {:?}",
                        self.context_id,
                        key_shared_mem,
                        oid,
                        next_period,
                        value
                    );
                    self.set_tick_period(Duration::from_millis(next_period as u64));
                    return;
                }

                //let ttl = ttl + system.ttl().as_secs();

                info!(self, "RNG id: {}, trying to acquire {}", id, key_shared_mem);

                let v = value.as_ref().map(|v| v.as_slice());
                let lock = self.set_shared_data(key_shared_mem.as_str(), v, Some(id));
                match lock {
                    Ok(()) => {
                        info!(self, "GOT THE LOCK WITH ID: {}", id);
                    }
                    _ => {
                        info!(self, "COULD NOT TAKE THE LOCK WITH ID: {}, uish", id);
                    }
                }

                let _ = value.replace(format!("some_val_{}", id).as_bytes().into());

                let _ = cas.replace(id);
                let v = value.as_ref().map(|v| v.as_slice());
                let set = self.set_shared_data(key_shared_mem.as_str(), v, cas);
                if set.is_ok() {
                    // got ownership
                    info!(
                        self,
                        "{}: got ownership of {} with id {}", self.context_id, key_shared_mem, id
                    );
                } else {
                    let status = set.unwrap_err();
                    // wait 10-20s
                    let next_period = self.rng.next_u32() % 10_000 + 10_000;
                    warn!(self,
                        "{}: could not get ownership of {}, status {:?} ({}), bailing temporarily for {} ms",
                        self.context_id,
                        key_shared_mem,
                        status,
                        status as u32,
                        next_period,
                    );
                    self.set_tick_period(Duration::from_millis(next_period as u64));
                    return;
                }

                let mut url = url.clone();
                url.set_path("testing.call");
                let res = upstream.call_url(
                    self,
                    &url,
                    "GET",
                    vec![],
                    None,
                    None,
                    Some(u64::try_from(upstream.timeout.as_millis()).unwrap()),
                );

                if let Err(e) = &res {
                    error!(
                        self,
                        "failed to call system configuration cluster {} (with URL {}): {:#?}",
                        upstream.name(),
                        url,
                        e
                    );
                }

                info!(
                    self,
                    "{}: releasing cas for {}", self.context_id, key_shared_mem
                );
                self.set_shared_data(key_shared_mem.as_str(), v, None)
                    .expect("failed to set_shared_data with None cas");

                //self.config_call = res.ok();
                warn!(
                    self,
                    "setting up on_tick for root ctx for {} seconds",
                    system.ttl().as_secs()
                );
                self.set_tick_period(system.ttl());
            }
        }
    }
}
