mod authrep;
pub mod metadata;
pub mod request_headers;

mod http_context;
pub use http_context::HttpAuthThreescale;

mod root_context;

use proxy_wasm::traits::RootContext;
use proxy_wasm::types::LogLevel;

#[cfg_attr(
    all(
        target_arch = "wasm32",
        target_vendor = "unknown",
        target_os = "unknown"
    ),
    export_name = "_start"
)]
#[cfg_attr(
    not(all(
        target_arch = "wasm32",
        target_vendor = "unknown",
        target_os = "unknown"
    )),
    allow(dead_code)
)]
// This is a C interface, so make it explicit in the fn signature (and avoid mangling)
extern "C" fn start() {
    proxy_wasm::set_log_level(LogLevel::Trace);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> {
        Box::new(root_context::RootAuthThreescale::new())
    });
}
