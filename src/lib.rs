#![deny(clippy::all, clippy::cargo)]
#![cfg_attr(feature_test, feature(test))]
#![cfg_attr(feature_unsafe_op_in_unsafe_fn, feature(unsafe_op_in_unsafe_fn))]

mod configuration;
mod proxy;
mod threescale;
mod upstream;
mod util;
