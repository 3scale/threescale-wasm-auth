# Proxy-WASM Authorization extension using 3scale

[![Build Status](https://github.com/3scale/threescale-wasm-auth/actions/workflows/ci.yaml/badge.svg)](https://github.com/3scale/threescale-wasm-auth/actions/workflows/CI/)
[![Security audit](https://github.com/3scale/threescale-wasm-auth/actions/workflows/audit.yaml/badge.svg)](https://github.com/3scale/threescale-wasm-auth/actions/workflows/Dependencies/)
[![Licensing](https://github.com/3scale/threescale-wasm-auth/actions/workflows/license.yaml/badge.svg)](https://github.com/3scale/threescale-wasm-auth/actions/workflows/Licensing/)
[![Clippy check](https://github.com/3scale/threescale-wasm-auth/actions/workflows/clippy.yaml/badge.svg)](https://github.com/3scale/threescale-wasm-auth/actions/workflows/Clippy/)
[![Rustfmt](https://github.com/3scale/threescale-wasm-auth/actions/workflows/format.yaml/badge.svg)](https://github.com/3scale/threescale-wasm-auth/actions/workflows/Rustfmt/)

This is a proxy-wasm filter integration for 3scale.

## Documentation

Please check out the [reference](./docs/reference.md) for information on how to set up and configure this module.
For information on the supported operations, read the [operations reference](./docs/operations.md).

## Demo

To run the demo:

1. Edit lds.conf in compose/envoy to fill in service data (ids, tokens, rules, ...).
2. Optionally edit compose/envoy/envoy.yaml to point the 3scale SaaS cluster to your 3scale (backend) instance.
3. Run `make build` to build the WebAssembly extension.
4. Run `make up` to run the docker-compose environment.
5. Run `make shell` in a different terminal and explore the scripts in `/examples`.
   There is an OIDC script that obtains a token from Keycloak and uses it to authenticate against the proxy.

If you set up other limits, those should be respected by this plug-in, and reporting should be happening and visible in your 3scale dashboard.

### Istio/Service Mesh

Run `make help` to learn about a few targets useful for these environments.

You will also find useful contents under the `servicemesh` directory.

If you want to test this module with the Bookinfo sample application there are targets to ease debugging by automatically deploying CRDs or streaming logs.
