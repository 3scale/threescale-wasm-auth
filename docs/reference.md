- [threescale-wasm-auth](#threescale-wasm-auth)
  - [Introduction](#introduction)
  - [Compatibility](#compatibility)
    - [Usage as a standalone module](#usage-as-a-standalone-module)
    - [3scale](#3scale)
- [Configuration](#configuration)
  - [Service Mesh Extension](#service-mesh-extension)
    - [Service Mesh Extension Custom Resource](#service-mesh-extension-custom-resource)
  - [External services](#external-services)
  - [Module configuration](#module-configuration)
    - [Anatomy of the configuration](#anatomy-of-the-configuration)
    - [The `api` object](#the-api-object)
    - [The `system` object](#the-system-object)
    - [The `upstream` object](#the-upstream-object)
    - [The `backend` object](#the-backend-object)
    - [The `services` object](#the-services-object)
    - [The `credentials` object](#the-credentials-object)
    - [Lookup queries](#lookup-queries)
    - [The `source` object](#the-source-object)
      - [The `operation` object](#the-operation-object)
    - [The `mapping_rules` object](#the-mapping_rules-object)
    - [The `mapping_rule` object](#the-mapping_rule-object)
  - [Examples](#examples)
    - [API key (user_key) in query string parameters](#api-key-user_key-in-query-string-parameters)
    - [Application ID and Key in Authorization header](#application-id-and-key-in-authorization-header)
    - [Open ID Connect](#open-id-connect)
      - [Picking up the JWT token from a header](#picking-up-the-jwt-token-from-a-header)

# threescale-wasm-auth

This document describes how to configure and run the `threescale-wasm-auth` module for integrating
the [`3scale API Manager`](https://3scale.net/) with the `OpenShift Service Mesh` product version
*2.1.0* or later.

## Introduction

The `threescale-wasm-auth` module is a [`WebAssembly`](https://webassembly.org/) module that uses
a set of interfaces, commonly referred to as an [`ABI`](https://en.wikipedia.org/wiki/Application_binary_interface),
as defined by the [`Proxy-WASM`](https://github.com/proxy-wasm/spec) specification, to drive any
piece of software that implements the ABI so it can authorize HTTP requests against `3scale`.

As an `ABI` specification, `Proxy-WASM` defines the interaction between a piece of software named
`host` and another named `module`, `program` or `extension`. The host exposes a set of services
used by the module to perform a task, and in this case, to process proxy requests.

The `host` environment is composed of a `WebAssembly` [`virtual machine`](https://en.wikipedia.org/wiki/WebAssembly#Virtual_machine) interacting with a piece of software, in this case, an HTTP proxy.

The `module` itself runs completely isolated to the outside world except for the instructions it
can run on the `virtual machine` and the `ABI` specified by `Proxy-WASM`. This is a safe way to
provide extension points to software: the extension can only interact in well-defined ways with the
`virtual machine`, which provides a computing model, and the `host`, which provides the interaction
with the outside world (ie. side effects) the program is meant to have.

## Compatibility

The `threescale-wasm-auth` module has been carefully designed to be fully compatible will all
implementations of the `Proxy-WASM` `ABI` specification. At this point, however, it has only been
thoroughly tested to work with the [`Envoy`](https://www.envoyproxy.io/) reverse proxy.

### Usage as a standalone module

Because of its self-contained design, this module can be set up to work with `Proxy-WASM`-adhering
proxies independently of `OpenShift Service Mesh` (as well as [`Istio`](https://istio.io/))
deployments. Running the module in this way, however, falls outside the scope of this document.

### 3scale

The module can work with all supported `3scale` releases except when configuring a service to use
[`Open ID Connect`](https://openid.net/connect/). In this case you'll need `3scale 2.11` or later.

# Configuration

**Note**: references are hereby made to `OpenShift Service Mesh` custom resources. Please refer to
          the relevant sections in the product documentation for your product version.

## Service Mesh Extension

`OpenShift Service Mesh` provides a [`custom resource definition`](https://kubernetes.io/docs/concepts/extend-kubernetes/api-extension/custom-resources/)
to specify and apply `Proxy-WASM` extensions to sidecar proxies. This `custom resource` is referred
to as [`ServiceMeshExtension](https://docs.openshift.com/container-platform/4.8/service_mesh/v2x/ossm-extensions.html)
(**note**: this is a link to the 2.0 docs, you should select a later version when available).

This `custom resource` will be applied to a set of [`workloads`](https://kubernetes.io/docs/concepts/workloads/`)
that require HTTP API management with `3scale`.

### Service Mesh Extension Custom Resource

Before proceeding, make sure you identify a [`Kubernetes`](https://kubernetes.io/) `workload`
and [`namespace`](https://kubernetes.io/docs/concepts/overview/working-with-objects/namespaces/) on
your `OpenShift Service Mesh` deployment that you'd like to apply this module to, and that you have
a 3scale account with a matching service and relevant applications and metrics defined.

Assuming you want to apply the module to the `productpage` microservice in the `bookinfo` namespace
(see [`Bookinfo sample application`](https://istio.io/latest/docs/examples/bookinfo/)), this is the
format expected for this `custom resource` when using `threescale-wasm-auth`:

```yaml
apiVersion: maistra.io/v1alpha1
kind: ServiceMeshExtension
metadata:
  name: threescale-wasm-auth
  namespace: bookinfo
spec:
  workloadSelector:
    labels:
      app: productpage
  config: <yaml configuration>
  image: quay.io/3scale/threescale-wasm-auth:1.0
  phase: PostAuthZ
  priority: 100
```

Do note that the `spec.config` field depends on the module configuration, so we have chosen not to
populate it here, but instead fill in a placeholder `<yaml configuration>` value so we can focus
on the format of this `custom resource`.

As you can see, the [`YAML`](https://yaml.org) document above refers to the [`Maistra`](https://maistra.io)
(upstream version of `OpenShift Service Mesh`) `ServiceMeshExtension` API. We declare the `namespace`
where our module will be deployed, alongside a [`WorkloadSelector`](https://istio.io/v1.9/docs/reference/config/type/workload-selector/)
to identify the set of applications the module will apply to.

Besides the `spec.config` field, which will vary depending on the application, the other fields
will stay the same across multiple instances of this `custom resource`. Namely, the `image` will
only change when newer versions of the module are deployed, and the `phase` will remain the same,
since this module needs to be invoked right after the proxy has done any local authorization,
such as validating [`OIDC`](https://openid.net/connect/) tokens. Please refer to the product
documentation for information on any of the other fields present in the above `YAML` object.

Once you are happy with the module configuration in `spec.config` and the rest of the
`custom resource`, you can apply it via the usual `oc apply` command:

> $ oc apply -f threescale-wasm-auth-bookinfo.yaml

## External services

In order for the module to authorize requests against `3scale` it must have access to `3scale`
services. This can be accomplished within `OpenShift Service Mesh` and `Istio` by applying
an external [`ServiceEntry`](https://istio.io/v1.9/docs/reference/config/networking/service-entry/)
object.

The `custom resources` below just set up the `service entries` for access from within the mesh
to the well-known [`SaaS`](https://en.wikipedia.org/wiki/Software_as_a_service) services from
`3scale` for the [`Apisonator`](https://github.com/3scale/apisonator) (aka `backend`) and [`Porta`](https://github.com/3scale/porta)
(aka `system`) components offering respectively the `Service Management API` and the
`Account Management API`, with the former meant to query for the authorization status of each
request, and the latter meant to provide API management configuration settings for your services.

```yaml
apiVersion: networking.istio.io/v1beta1
kind: ServiceEntry
metadata:
  name: threescale-saas-backend
spec:
  hosts:
  - su1.3scale.net
  ports:
  - number: 443
    name: https
    protocol: HTTPS
  location: MESH_EXTERNAL
  resolution: DNS
---
apiVersion: networking.istio.io/v1beta1
kind: ServiceEntry
metadata:
  name: threescale-saas-system
spec:
  hosts:
  - multitenant.3scale.net
  ports:
  - number: 443
    name: https
    protocol: HTTPS
  location: MESH_EXTERNAL
  resolution: DNS
---
```

Just like any `YAML` `custom resource` file, these objects can be applied in your cluster via
the `oc apply` command.

**Note**: Technically there is nothing preventing you from deploying an in-mesh `3scale` service.
          If so, then you'll want to change the `location` of these services in the resources
          above. Check the `ServiceEntry` [`documentation`](https://istio.io/v1.9/docs/reference/config/networking/service-entry/)
          for more details.

## Module configuration

The `spec.config` of the `ServiceMeshExtension` `custom resource` is used to provide the
configuration that the `Proxy-WASM` module will read.

This configuration is embedded in the `host` configuration and read by the module. Typically you
will find these configurations in [`JSON`](https://json.org/) format for the modules to parse,
but the `ServiceMeshExtension` resource will interpret the `spec.config` field value as `YAML`
and then convert the value to `JSON` for consumption by the module. `YAML` representations can
be significantly more compact than their `JSON` counterparts, and given you will be writing the
configuration within a `YAML` document, accepting `YAML` to be seamlessly integrated with the
extension object will avoid issues with escaping and quoting, as well as make for significantly
less verbose configurations.

**Note**: when using the module in standalone mode, you'll have to write the configuration using
          `JSON` formatting, escaping and quoting as needed, within the `host` (ie. `Envoy`)
          configuration files. When used in combination with the `ServiceMeshExtension` resource,
          you should take into account that even though you'll be writing the configuration in
          `YAML` format, an invalid configuration will force the module to emit diagnostics based
          on its `JSON` representation to a sidecar's logging stream.

**Note**: you can technically make use of the [`EnvoyFilter`](https://istio.io/v1.9/docs/reference/config/networking/envoy-filter/)
          `custom resource` in some `Istio` or `OpenShift Service Mesh` releases, but that resource
          is not a supported API at all. However, if you still wanted to use it, note that you'll
          also have to rely on specifying this configuration in `JSON` format. This is **not
          recommended** - instead you should use the `ServiceMeshExtension` API.

**Note**: Red Hat is working with upstream `Istio` to provide the `ServiceMeshExtension` (or an
          equivalent API) to all upstream users so this API becomes the widely adopted method to
          work with `Proxy-WASM` modules.

### Anatomy of the configuration

The configuration of the module can be divided in two big sections.

1. 3scale account and authorization service.
2. List of services to handle.

There are a set of minimum mandatory fields in all cases:

- For the `3scale` account and authorization service: the `Apisonator` (backend-listener) URL.
- For the list of services to handle: the service ids and at least one credential look up
  method and where to find it.

We'll discuss these two main sections and their subsections below, and we'll provide examples
for dealing with `user key`, `app id`, `app id` with `app key`, and `OIDC` patterns.

**Note**: whatever your settings are in the static configuration, they'll always be taken into
          account. For example, if you add a mapping rule configuration here, it will always
          apply even when the 3scale administation portal has no such mapping rule.

**Note**: imagine the rest of the `ServiceMeshExtension` resource exists around the
          `spec.config` YAML entry.

### The `api` object

The `api` top level string of the configuration tells the module which version of the
configuration to expect. A non existing or unsupported version will render the module
inoperant.

```yaml
spec:
  config:
    api: v1
```

The `api` entry defines the rest of the values for the configuration. At this point
the only accepted value is `v1`. Newer configuration settings that break compatibility
with the current configuration definition or otherwise require additional logic that
modules dealing with `v1` cannot handle, will require different values.

### The `system` object

The `system` top level object specifies how to access the 3scale Account Management API
for a specific account. The most important part is the `upstream` field, which you'll see
again soon.

This field is **optional but recommended** unless you are going to provide a fully static
configuration for the module, which is an interesting option if you don't want to provide
connectiviy to the Porta/System component of 3scale.

Note that whenever you provide static configuration objects in addition to this object,
_the static ones always take precedence_.

```yaml
system:
  name: saas_porta
  upstream: <object, see definition below>
  token: myaccount_token
  ttl: 300
```

The following fields are part of this object:

* `name`: Optional. An identifier for this 3scale service, currently not referenced elsewhere.
* `upstream`: Required. The details about a network host to be contacted. In this case, this has
              to refer to the 3scale Account Management API host, known as Porta or system. See
              below for the contents' description.
* `token`: Required. A 3scale personal access token with read permissions.
* `ttl`: Optional. The *minimum* amount of seconds to consider a configuration retrieved from
         this host as valid before trying to fetch new changes. Default is 600 (10 minutes).
         *Note*: there is no *maximum* amount, but the module will generally fetch any such
                 configuration within some reasonable amount of time after this TTL elapses.

### The `upstream` object

The `upstream` object describes an external host to which the proxy can perform calls.

```yaml
upstream:
  name: outbound|443||multitenant.3scale.net
  url: "https://myaccount-admin.3scale.net/"
  timeout: 5000
```

The fields are defined as follows:

* `name`: Required. This is _not_ a free-form identifier. Instead, it is the idenfitier for
          the external host as defined by the proxy configuration. In the case of standalone
          `Envoy` configurations, it maps to the name of a [`Cluster`](https://www.envoyproxy.io/docs/envoy/v1.19.0/api-v3/config/cluster/v3/cluster.proto#config-cluster-v3-cluster)
          (also known as `upstream` in other proxies). Pay special attention to the value of
          this field, because the `OpenShift Service Mesh` and `Istio` [`control plane`](https://istio.io/v1.9/docs/ops/deployment/architecture/)
          will configure the name according to a format using `|` as separator of multiple fields.
          For the purposes of this integration, always use the format: `outbound|<port>||<hostname>`.
* `url`: Required. The complete [`URL`](https://en.wikipedia.org/wiki/URL) to access the described
         service. Unless implied by the scheme, make sure to include the TCP port.
* `timeout`: Optional. Timeout in milliseconds so that connections to this service that take more than
             that amount of time to respond will be considered errors. Default is 1000.

### The `backend` object

The `backend` top level object specifies how to access the 3scale Service Management API
for authorizing and reporting HTTP requests. This service is provided by the `Apisonator` component,
also known as `backend`.

The most important part is the `upstream` field, which you just read about in the previous section.

This field is **required**.

```yaml
backend:
  name: saas_apisonator
  upstream: <object, see definition above>
```

The following fields are part of this object:

* `name`: Optional. An identifier for this 3scale service, currently not referenced elsewhere.
* `upstream`: Required. The details about a network host to be contacted. In this case, this has
              to refer to the 3scale Service Management API host, known as Apisonator or backend.
              See above for the contents' description.

### The `services` object

The `backend` top level object specifies which service identifiers will be handled by this
particular instance of the `module`.

Since accounts can have multiple services, you must specify here which ones are handled. The
rest of the configuration revolved around how to configure `services`.

This field is **required**, and is an `array` that should contain at least one service in order
to do useful work.

```yaml
services:
  - id: "2555417834789"
    token: service_token
    environment: production
    authorities:
      - "*.app"
      - 0.0.0.0
      - "0.0.0.0:8443"
    credentials: <object, see definition below>
    mapping_rules: <object, see definition below>
```

Each element in the `services` array represents a `3scale` service. The fields are defined below:

* `id`: Required. The `3scale` service identifier for this service.
* `token`: Required. The `3scale` service token to be used to authenticate this service against
           Apisonator.
* `environment`: Optional, defaults to `production`. The `3scale` environment of this service.
* `authorities`: Required. An array of strings, each one representing the [`Authority`](https://en.wikipedia.org/wiki/Uniform_Resource_Identifier#Syntax)
                 of a `URL` to match. These strings do accept [`glob patterns`](https://en.wikipedia.org/wiki/Glob_%28programming%29)
                 supporting the `*`, `+` and `?` matchers.
* `credentials`: Required. An object defining which kind of credentials to look for and where.
                 See definition below.
* `mapping_rules`: Required. An array of objects representing mapping rules and `3scale` methods to hit.
                   See definition below.

### The `credentials` object

The `credentials` object is part of the `service` object. It specifies which kind of credentials
should be looked up. All fields are optional, but at least one of `user_key` or `app_id` should
be specified.

```yaml
credentials:
  user_key: <array of lookup queries>
  app_id: <array of lookup queries>
  app_key: <array of lookup queries>
```

The fields specify which credentials are going to be looked up by specifying _how_ to do so. The order
in which you specify each kind of credential is irrelevant because it is pre-established by the module,
and you can only specify one instance of each of them.

The next section will deal with `lookup queries`. Fields are as follows:

* `user_key`: Optional. This is an array of `lookup queries` that will define a `3scale` user key.
              A user key is commonly known as an [`API key`](https://en.wikipedia.org/wiki/Application_programming_interface_key).
* `app_id`: Optional. This is an array of `lookup queries` that will define a `3scale` application
            identifier. Application identifiers are provided by `3scale`, or via an identity provider
            like [`Red Hat SSO`](https://access.redhat.com/products/red-hat-single-sign-on) (known
            as [`Keycloak`](https://www.keycloak.org/) upstream) via `OIDC`. The resolution of the
            `lookup queries` specified here, whenever it is successful _and resolves to two values_,
            will not only set up the application identifier, but also the application key, `app_key`.
* `app_key`: Optional. This is an array of `lookup queries` that will define a `3scale` application
             key. Application keys _without_ a resolved application identifier (`app_id`) are
             useless, so you should only specify this field whenever `app_id` has also been specified.

These `credentials` fields will be resolved in the following order:

1. If `user_key` is defined and resolved, its value will be used to authorize against `3scale`.
   No other fields are taken into account.
2. Otherwise, if `app_id` is defined and resolved to a _single value_, that value will be used to
   authorize against `3scale`, but a potential `app_key` field will first be evaluated if defined.
   If the resolution turns out not one but at least _two values_, `app_id` and `app_key` will be
   assigned those values respectively, and they will be used to authorize against `3scale` ignoring
   any potential `app_key` definition.
3. If `app_key` is defined and not yet assigned as a result of the previous step, and `app_id` was
   successfully resolved, then this field is evaluated and its value, if resolved successfully, used
   to authorize against `3scale`. If `app_id` was not successfully assigned, this section is ignored.

This gives us essentially two types of credentials used for authorizing against `3scale`: an `user key`
(also known as `API key`), or an `application identifier` (`app id`), which optionally might have an
`application key` (`app_key`), as configured in the `3scale` admin portal.

Note that this leaves out `Open ID Connect` (`OIDC`). This is because `OIDC` is just a mechanism to
obtain and validate an `app_id`. We'll see specific use cases later on.

### Lookup queries

The `lookup query` object is part of any one of the fields in the `credentials` object. It specifies
how a given credential field should be found and processed. When evaluated, a successful resolution
means that one or more values were found. A failed resolution means that no value was found.

Arrays of `lookup queries` describe a short-circuit `OR` relationship: a successful resolution of one
of the queries stops the evaluation of any remaining queries and assigns the value (or values) to the
specified credential type. Each query in the array is independent of each other.

A `lookup query` is made up of a single field, a `source` object, which can be one of a number of
`source types`.

```yaml
credentials:
  user_key:
    - <source_type>: <object, see definition below>
    - <source_type>: <object, see definition below>
    ...
  app_id:
    - <source_type>: <object, see definition below>
    ...
  app_key:
    - <source_type>: <object, see definition below>
    ...
```

### The `source` object

A `source` object is found as part of an array of `source`s within any one of the `credentials`
object fields. The object field name, referred to as a `source type` is any one, and only one, of
the following:

* `header`: The `lookup query` will receive HTTP request headers as input.
* `query_string`: The `lookup query` will receive the `URL` [`query string`](https://en.wikipedia.org/wiki/Query_string)
                  parameters as input.
* `filter`: The `lookup query` will receive filter metadata as input.

All `source type` objects have at least the following two fields:

* `keys`: Required. An array of strings, each one a `key`, referring to entries found in the input
          data.
* `ops`: Optional. An array of `operations` to perform on a `key` entry match, as a pipeline where
         operations will receive inputs and generate outputs to be consumed by the next operation.
         An `operation` failing to provide an output resolves the `lookup query` as failed. Because
         these work like a pipeline, the order of the `operations` is significant and determines
         the evaluation order.

`filter` has, in addition, a required `path` entry to indicate the path within the metadata that
we should be looking up at for the data we are looking for. We will see an example later on.

Whenever a `key` matches the input data, the rest of the `keys` _are not evaluated_ and the source
resolution algorithm jumps to executing the `operations` (`ops`) specified, if any. If no `ops` are
specified then the result value of the matching `key`, if any, is returned.

`Operations` provide a way to specify certain conditions and transformations for inputs you have
after the first phase successfully looks up a key. They should be used when you need to transform,
decode, and assert properties, but they do not provide a fully fledged language to deal with all
needs, and they lack [`Turing-completeness`](https://en.wikipedia.org/wiki/Turing_completeness).

A stack is used to keep outputs of `operations` around, and once they are successfully evaluated the
`lookup query` finishes by assigning the value or values in the bottom of the stack, depending on
how many values will be consumed by the credential kind specified.

#### The `operation` object

Each element in the `ops` array belonging to a specific `source type` is an `operation` object that
either applies transformations to values or performs tests. The field name to use for such an object
is the name of the `operation` itself, and any values are the parameters to the `operation`, which
could themselves be structure objects (ie. maps with fields and values), lists or strings.

Most `operations` consume one or more inputs, and produce one or more outputs. Whenever they consume
inputs or produce outputs, they work with a stack of values: each value consumed by the operations
is popped from the stack of values (initially populated with any `source` matches), and any values
output by them will be pushed to the stack. Some other `operations` don't consume or produce outputs
other than asserting certain properties, but they still inspect a stack of values.

*Note*: whenever resolution finishes, the values picked up by the next step (such as assigning the
values to be an `app_id`, an `app_key` or a `user_key`) are always taken from the bottom values of
the stack.

There are a few different `operations` categories:

* `decode`: these transform an input value by decoding it to obtain a different format.
* `string`: these take a (string) value as input and perform transformations and checks on it.
* `stack`: these take a set of values in the input and perform multiple stack transformations and
           selection of specific positions in the stack.
* `check`: these assert properties about sets of operations in a side-effect free way.
* `control`: these perform operations that allow for modifying the evaluation flow.
* `format`: these parse the format-specific structure of input values and look up values in it.

All operations are specified by the name identifiers as strings.

You can read about the available operations in the [reference](./operations.md), and we'll make use
of a few of them in the examples describing the most common use cases.

### The `mapping_rules` object

The `mapping_rules` object is part of the `service` object. It specifies a set of [`REST`](https://en.wikipedia.org/wiki/Representational_state_transfer)
path patterns and associated `3scale` metrics and count increments to use when the patterns match.

This value is only required if no dynamic configuration is provided in the `system` top level object.
If this object is provided _in addition to_ the `system` top level entry, then this one is evaluated
first.

This object's value is an array of `mapping rule` objects. All mapping rules that are evaluated as
matching on an incoming request provide the set of `3scale` `method`s to report for authorization and
reporting to the `API Manager`. Whenever multiple matching rules refer to the same `method`s, there
will be a summation of `delta`s when calling into `3scale`, that is, if two rules increase some
`Hits` method twice with `delta`s of `1` and `3`, a single method entry for `Hits` reported to
`3scale` will be associated with a `delta` of `4`.

### The `mapping_rule` object

This object is specified as part of an `array` in the `mapping_rules` object. The fields specify
which [`HTTP request method`](https://en.wikipedia.org/wiki/Hypertext_Transfer_Protocol#Request_methods)
to match, a pattern to match the path against, and which `3scale` methods to report along with
the amount to report. The order in which you specify the fields determines the evaluation order.

Fields are as follows:

* `method`: Required. Specifies a string representing an `HTTP request method`, also known as `verb`.
            Values accepted match the any one of the accepted HTTP method names, case-insensitive. A
            special value of `any` matches any method.
* `pattern`: Required. The pattern to match the HTTP request's `URI` `path` component. This pattern
             follows the same syntax as documented by `3scale`. Notably, it allows wildcards (same
             effect as a globbing pattern with a `*` character) using any sequence of characters in
             between braces like `{this}`.
* `usages`: Required. A list of `usage` objects. When the rule is matched, all `method`s with their
            `delta`s here will be added to the list of `method`s that will be sent to `3scale` for
            authorization and reporting. This object is so simple we'll embed its fields here:
            `name` refers to the `method` system name to report, and `delta` refers to how much to increase
            that `method` by. Both fields are required.
* `last`: Optional boolean, defaulting to `false`. Whether the successful matching of this rule
          should stop the evaluation of additional mapping rules.

```yaml
mapping_rules:
  - method: GET
    pattern: /
    usages:
      - name: hits
        delta: 1
  - method: GET
    pattern: /products/
    usages:
      - name: products
        delta: 1
  - method: ANY
    pattern: /products/{id}/sold
    usages:
      - name: sales
        delta: 1
      - name: products
        delta: 1
```

In the above example a `GET` request to a path `/products/1/sold` will match all the rules, so all
the usages will be added to the request the module will perform to `3scale` with usage data as
follows:

- `Hits`: 1
- `products`: 2
- `sales`: 1

**Note**: this is independent of existing hierarchies between methods at `3scale` - those will be
          computed by `3scale` on its end, ie. `Hits` might be a parent of them all, thus storing
          `4` hits overall due to the sum of all reported `method`s, provided the request is
          successfully authorized (this module calls the `3scale` `Authrep` API endpoint).

## Examples

Despite having quite a bit of flexibility around using operations to obtain the data you are looking
for, most of the time you'll be applying simple configuration steps to obtain credentials in the
requests to your services.

Here you'll find examples about each credential kind which will need little modification to adapt to
specific use cases. Note that you can combine them all - the only caveat is that whenever you specify
multiple `source` objects with their own `lookup queries`, they are evaluated in order until one of
them is successfully resolved.

### API key (user_key)

This looks up a `user_key` in a query string parameter or header of the same name.

```yaml
credentials:
  user_key:
    - query_string:
        keys:
          - user_key
    - header:
        keys:
          - user_key
```
### Application ID and Key
This looks up a `app_key` and `app_id` in a query or headers.

```yaml
credentials:
  app_id:
    - header:
        keys:
          - app_id
    - query_string:
        keys:
          - app_id
  app_key:
    - header:
        keys:
          - app_key
    - query_string:
        keys:
          - app_key
```
#### Authorization header

A request might also include these in an `Authorization` header. The resolution here will assign the
`application key` if there is one or two output at the end.

The `Authorization` header specifies a value with the type of authorization and then its value
encoded as [`Base64`](https://en.wikipedia.org/wiki/Base64) in a URL-safe way. This means we
can split the value by a space character, take the second output and then split it again using
`:` as the separator, assuming the format is `app_id:app_key`.

The header might look like this for credential `aladdin:opensesame`:

> Authorization: Basic YWxhZGRpbjpvcGVuc2VzYW1l

_Note_ the use of lower case header field names.

```yaml
credentials:
  app_id:
    - header:
        keys:
          - authorization
        ops:
          - split:
              separator: " "
              max: 2
          - length:
              min: 2
          - drop:
              head: 1
          - base64_urlsafe
          - split:
              max: 2
  app_key:
    - header:
        keys:
          - app_key
```

This will look into the headers for an `Authorization` one, take its string value and split it by
space, checking at least two values were generated for credential type and credential itself, and
dropping the credential type. Then it will decode the second value containing the data we are
interested in, and we'll split it by the `:` character to have an operations' stack including first
the `app_id`, then the `app_key`, if it exists. If `app_key` does not exist in the authorization header then its specific sources are checked, i.e., header with key `app_key` in this case.

You might want to augment this example with extra conditions: let's now ensure you only allow `Basic`
authorizations, and `app_id` being either `aladdin` or `admin`, or any `app_id` with at least 8
characters in length. Additionally the `app_key` should be non-empty but smaller than 64 characters.

Here's one way to go about it:

```yaml
credentials:
  app_id:
    - header:
        keys:
          - authorization
        ops:
          - split:
              separator: " "
              max: 2
          - length:
              min: 2
          - reverse
          - glob:
            - Basic
          - drop:
              tail: 1
          - base64_urlsafe
          - split:
              max: 2
          - test:
              if:
                length:
                  min: 2
              then:
                - strlen:
                    max: 63
                - or:
                    - strlen:
                        min: 1
                    - drop:
                        tail: 1
          - assert:
            - and:
              - reverse
              - or:
                - strlen:
                    min: 8
                - glob:
                  - aladdin
                  - admin
```

Here we check, after picking up the `Authorization` header value, that we got a `Basic` credential
type by reversing the stack so that the type is placed on the top of the stack, and then running a
glob match on it.

Once we have validated this, and decoded and split the credential, we have the `app_id` at the
bottom of the stack, and potentially an `app_key` at the top. At this point we run a `test`: if we
got two values in the stack, meaning we got an `app_key`, then we ensure its string length is
between 1 and 63, both included. If the key's length was zero we drop it and continue as if no key
was specified. If there was only an `app_id` and no `app_key`, the missing `else` branch indicates
that the `test` should be successful so evaluation continues.

The last operation, `assert`, indicates that no side effects will make it to the stack, so we can
modify it at will to perform our checks. We will first `reverse` the stack to have the `app_id` at
the top - we don't know whether an `app_key` is present, but with `reverse` we ensure `app_id` will
be at the top whether we have just an `app_id`, or an `app_id` and an `app_key` (we could just as
well have used `indexes: [0]` or some other combination).

Since we want to preserve the contents of the stack across these tests, we use `and`. Then we require
any one of two possibilities: either `app_id` has a string length of at least `8`, or it matches
either `"aladdin"` or `"admin"`. At this point we have used `or`, because even though that operation
keeps changes to the stack, the operations specified perform no changes (and the top level `assert`
would drop them anyway), so while we could have used `any`, it would also be slightly less efficient,
since the latter needs to hand temporary stacks to its operations as opposed to the former.

### Open ID Connect

In the case of `OpenShift Service Mesh` and `Istio`, you will need to deploy a [`RequestAuthentication`](https://istio.io/v1.9/docs/reference/config/security/request_authentication/)
like the one below, filling in your own workload data and `jwtRules`:

```yaml
apiVersion: security.istio.io/v1beta1
  kind: RequestAuthentication
  metadata:
    name: jwt-example
    namespace: bookinfo
  spec:
    selector:
      matchLabels:
        app: productpage
    jwtRules:
    - issuer: >-
        http://keycloak-keycloak.34.242.107.254.nip.io/auth/realms/3scale-keycloak
      jwksUri: >-
        http://keycloak-keycloak.34.242.107.254.nip.io/auth/realms/3scale-keycloak/protocol/openid-connect/certs
```

The above snippet, when applied, will configure `Envoy` with [a native plug-in](https://www.envoyproxy.io/docs/envoy/v1.19.0/api-v3/extensions/filters/http/jwt_authn/v3/config.proto.html)
to validate `JWT` tokens. The proxy will take care of validation before running the `module`, so any
requests that fail this step will never be seen by this `WASM` `module` if the phase of execution has
been well configured per the instructions above.

Once a `JWT` token is validated, the proxy will store the its contents in an internal metadata
object, with an entry whose key depends on the specific configuration of the plug-in (see the
`payload_in_metadata` setting). This is a great use case for the ability to look up structure objects
with a single entry with an unknown key name.

The `3scale` `app_id` for `OIDC` matches the OAuth's `client_id`, typically found in the `azp` or `aud`
fields of `JWT` tokens.

Here's how you'd get this field from Envoy's native JWT authentication filter:

```yaml
credentials:
  app_id:
    - filter:
        path:
          - envoy.filters.http.jwt_authn
          - "0"
        keys:
          - azp
          - aud
        ops:
          - take:
              head: 1
```

In this case, we instruct the module to use the `filter` `source type` to look up filter metadata for
an object from the `Envoy`-specific JWT Authn native plug-in. This plug-in will include the JWT token
as part of a structure object with a single entry and a pre-configured name. Since we don't want to
have to deal with somehow guessing the name of that entry, we use `"0"` which is how we specify that
we only care about accessing the single entry. At that point the resulting value is a structure for
which we'll try to resolve two fields: `azp`, typically the value where `app_id` is found, and `aud`,
where sometimes this information can be found as well.

The specified operation ensures that only one value is held for assignment, since in the case where
we'd end up with two, such as if `aud` contained more than one value, assigning to `app_id` would also
assign `app_key`, which is something that we don't want for `OIDC`.

#### Picking up the JWT token from a header

Some setups might have validation processes for JWT tokens where the validated token would
reach this module via a header in JSON format. Here's how you can obtain the `app_id`:

```yaml
credentials:
  app_id:
    - header:
        keys:
          - x-jwt-payload
        ops:
          - base64_urlsafe
          - json:
            - keys:
              - azp
              - aud
          - take:
              head: 1
```
