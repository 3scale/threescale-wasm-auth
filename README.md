# Proxy-WASM Authorization extension using 3scale

![Clippy check](https://github.com/3scale-rs/threescale-wasm-auth/workflows/Clippy%20check/badge.svg)
![Security audit](https://github.com/3scale-rs/threescale-wasm-auth/workflows/Security%20audit/badge.svg)
![Continuous integration](https://github.com/3scale-rs/threescale-wasm-auth/workflows/Continuous%20integration/badge.svg)

This is a proxy-wasm filter integration for 3scale.

To run the demo:

1. Edit lds.conf in compose/envoy to fill in service data (ids, tokens, rules, ...).
1.1 Optionally edit compose/envoy/envoy.yaml to point the 3scale SaaS cluster to your 3scale (backend) instance.
2. Run `make build` to build the WebAssembly extension.
3. Run `make up` to run the docker-compose environment.
4. Create a `secrets` file with the following contents:
```shell
export WEB_KEY=<a user_key for the service handling the web.app backend>
```
5. Run `source secrets`.
6. Run `make curl-compose`.
6.1 Optionally specify a path to hit a specific pattern rule: `make SVC_PATH=productpage curl-compose` (N.B. no initial slash!)
    This specific path is used as well for Istio/SM configurations, and is set up in 3scale to have a 5 hits/minute rate limiting,
    so it is useful to test the integration with 3scale.

If you set up other limits, those should be respected by this plug-in, and reporting should be happening and visible in your 3scale dashboard.

### Examples

You can run `make shell` after the docker-compose environment is up and run examples from the `/examples` directory.

### Istio/Service Mesh

Run `make help` to learn about a few targets useful for these environments.

You will also find useful contents under the `servicemesh` directory.

If you want to test this module with the Bookinfo sample application there are targets to ease debugging by automatically deploying CRDs or streaming logs.
