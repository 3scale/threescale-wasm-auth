use core::convert::TryFrom;

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod operation;
pub use operation::*;

mod source;
pub use source::*;

pub mod api;

#[derive(Debug, Error)]
pub enum MissingError {
    #[error("no backend configured")]
    Backend,
    #[error("no services configured")]
    Services,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationKind {
    UserKey,
    AppId,
    AppKey,
    Oidc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "api")]
pub enum Configuration {
    #[serde(rename = "v1", alias = "v1.0", alias = "v1.0.0")]
    V1(api::v1::Configuration),
}

impl Configuration {
    pub fn get(&self) -> &api::v1::Configuration {
        match self {
            Self::V1(c) => c,
        }
    }

    pub fn get_mut(&mut self) -> &mut api::v1::Configuration {
        match self {
            Self::V1(c) => c,
        }
    }
}

// Default to JSON configuration deserialization
#[cfg(any(feature = "json_config", not(feature = "yaml_config")))]
impl TryFrom<&[u8]> for Configuration {
    type Error = serde_json::Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        serde_json::from_slice(buf)
    }
}

#[cfg(all(
    feature = "yaml_config",
    feature = "danger",
    not(feature = "json_config")
))]
impl TryFrom<&[u8]> for Configuration {
    type Error = serde_yaml::Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        serde_yaml::from_slice(buf)
    }
}

#[cfg(test)]
mod test {
    use core::time::Duration;
    use std::convert::TryInto;

    use threescalers::http::mapping_rule::{Method, RestRule};

    use crate::threescale::{
        Backend, Credentials, Environment, MappingRule, Service, System, Usage,
    };
    use crate::upstream::Upstream;
    use crate::util::glob::GlobPatternSet;
    use crate::util::serde::ErrorLocation;

    use super::*;

    fn show_parsed_deserialization<'de, T, E, F>(input: &'de str, deser_fn: F) -> T
    where
        F: FnOnce(&'de str) -> Result<T, E>,
        T: std::fmt::Debug + serde::Deserialize<'de>,
        E: std::error::Error + std::fmt::Display,
        for<'e> &'e E: TryInto<ErrorLocation<'e, E>>,
    {
        let res = deser_fn(input);
        if let Err(ref e) = res {
            if let Ok(el) = e.try_into() {
                eprintln!("{}", el.error_to_string(input));
            } else {
                eprintln!("{}", e);
            }
        }
        assert!(res.is_ok());
        let parsed = res.unwrap();
        eprintln!("PARSED:\n{:#?}", parsed);
        parsed
    }

    fn get_config() -> Configuration {
        Configuration::V1(api::v1::Configuration {
            system: Some(System {
                name: Some("system-name".into()),
                upstream: Upstream {
                    name: "outbound|443||multitenant.3scale.net".into(),
                    url: "https://istiodevel-admin.3scale.net".parse().unwrap(),
                    timeout: Duration::from_millis(5000),
                },
                token: "atoken".into(),
                ttl: Some(300),
            }),
            backend: Some(Backend {
                name: Some("backend-name".into()),
                upstream: Upstream {
                    name: "outbound|443||su1.3scale.net".into(),
                    url: "https://su1.3scale.net".parse().unwrap(),
                    timeout: Duration::from_millis(5000),
                },
                extensions: Some(vec!["no_body".to_string()]),
            }),
            services: Some(vec![Service {
                id: "2555417834780".into(),
                token: Some("service_token".into()),
                environment: Environment::Production,
                authorities: GlobPatternSet::new(
                    [
                        "ingress",
                        "web",
                        "web.app",
                        "0.0.0.0",
                        "0.0.0.0:8080",
                        "0.0.0.0:8443",
                    ]
                    .iter(),
                )
                .unwrap(),
                credentials: Credentials::new(
                    Some(vec![Source::QueryString {
                        keys: vec!["api_key".into()],
                        ops: Some(vec![
                            Operation::Check(Check::Assert(vec![Operation::Check(Check::Ok)])),
                            Operation::StringOp(StringOp::Split {
                                separator: ":".into(),
                                max: Some(2),
                            }),
                            Operation::Stack(Stack::Reverse),
                            Operation::Stack(Stack::Values {
                                level: Default::default(),
                                id: Some("stackid".into()),
                            }),
                            Operation::Stack(Stack::Take {
                                head: None,
                                tail: Some(1),
                            }),
                        ]),
                    }]),
                    Some(vec![
                        Source::Header {
                            keys: vec!["authorization".into()],
                            ops: vec![
                                Operation::Stack(Stack::Values {
                                    level: Default::default(),
                                    id: Some("init".into()),
                                }),
                                Operation::StringOp(StringOp::Split {
                                    separator: " ".into(),
                                    max: Some(2),
                                }),
                                Operation::Stack(Stack::Length {
                                    min: Some(2),
                                    max: None,
                                }),
                                Operation::Stack(Stack::Reverse),
                                Operation::StringOp(StringOp::Glob(
                                    GlobPatternSet::new(["Basic"].iter()).unwrap(),
                                )),
                                Operation::Stack(Stack::Drop {
                                    tail: Some(1),
                                    head: None,
                                }),
                                Operation::Decode(Decode::Base64UrlSafe),
                                Operation::StringOp(StringOp::Split {
                                    max: Some(2),
                                    separator: ":".into(),
                                }),
                                Operation::Control(Control::Test {
                                    r#if: Operation::Stack(Stack::Length {
                                        min: Some(2),
                                        max: None,
                                    })
                                    .into(),
                                    then: vec![Operation::StringOp(StringOp::Length {
                                        min: Some(1),
                                        max: Some(63),
                                        mode: Default::default(),
                                    })],
                                    r#else: vec![],
                                }),
                                Operation::Check(Check::Assert(vec![Operation::Control(
                                    Control::And(vec![
                                        Operation::Stack(Stack::Reverse),
                                        Operation::Control(Control::Or(
                                            [
                                                Operation::StringOp(StringOp::Length {
                                                    min: Some(8),
                                                    max: None,
                                                    mode: Default::default(),
                                                }),
                                                Operation::StringOp(StringOp::Glob(
                                                    GlobPatternSet::new(
                                                        ["aladdin", "admin"].iter(),
                                                    )
                                                    .unwrap(),
                                                )),
                                            ]
                                            .into(),
                                        )),
                                    ]),
                                )])),
                            ]
                            .into(),
                        },
                        Source::Filter {
                            path: vec!["envoy.filters.http.jwt_authn".into(), "0".into()],
                            keys: vec!["azp".into(), "aud".into()],
                            ops: Default::default(),
                        },
                        Source::Header {
                            keys: vec!["x-jwt-payload".into()],
                            ops: Some(vec![
                                Operation::Decode(Decode::Base64UrlSafe),
                                Operation::Format(Format::Json {
                                    path: vec![],
                                    keys: vec!["azp".into(), "aud".into()],
                                }),
                            ]),
                        },
                        Source::Header {
                            keys: vec!["x-app-id".into()],
                            ops: Default::default(),
                        },
                    ]),
                    None,
                ),
                mapping_rules: vec![MappingRule {
                    rule: RestRule::new(Method::from("any"), "/").unwrap(),
                    usages: vec![Usage {
                        name: "Hits".into(),
                        delta: 1,
                    }],
                    last: Default::default(),
                }],
            }]),
            passthrough_metadata: Some(true),
        })
    }

    #[cfg(any(feature = "json_config", not(feature = "yaml_config")))]
    mod json {
        use super::*;

        mod fixtures {
            pub const CONFIG: &str = r#"{
              "api": "v1",
              "system": {
                "name": "system-name",
                "upstream": {
                  "name": "outbound|443||multitenant.3scale.net",
                  "url": "https://istiodevel-admin.3scale.net/",
                  "timeout": 5000
                },
                "token": "atoken"
              },
              "backend": {
                "name": "backend-name",
                "upstream": {
                  "name": "outbound|443||su1.3scale.net",
                  "url": "https://su1.3scale.net/",
                  "timeout": 5000
                },
                "extensions": [
                  "no_body"
                ]
              },
              "services": [
                {
                  "id": "2555417834780",
                  "token": "service_token",
                  "authorities": [
                    "ingress",
                    "web",
                    "web.app",
                    "0.0.0.0",
                    "0.0.0.0:8080",
                    "0.0.0.0:8443"
                  ],
                  "credentials": {
                    "user_key": [
                      {
                        "query_string": {
                          "keys": [
                            "api_key"
                          ],
                          "ops": [
                            {
                              "split": {
                                "separator": ":",
                                "max": 2,
                                "indexes": [
                                  0
                                ]
                              }
                            }
                          ]
                        }
                      }
                    ],
                    "app_id": [
                      {
                        "filter": {
                          "path": [
                            "envoy.filters.http.jwt_authn",
                            "0"
                          ],
                          "keys": [
                            "azp",
                            "aud"
                          ],
                          "ops": null
                        }
                      },
                      {
                        "header": {
                          "keys": [
                            "x-jwt-payload"
                          ],
                          "ops": [
                            "base64_urlsafe",
                            {
                              "json": {
                                "path": [],
                                "keys": [
                                  "azp",
                                  "aud"
                                ]
                              }
                            }
                          ]
                        }
                      },
                      {
                        "header": {
                          "keys": [
                            "x-app-id"
                          ],
                          "ops": null
                        }
                      }
                    ]
                  },
                  "mapping_rules": [
                    {
                      "method": "ANY",
                      "pattern": "/",
                      "usages": [
                        {
                          "name": "Hits",
                          "delta": 1
                        }
                      ]
                    }
                  ]
                }
              ],
              "passthrough_metadata": true
            }"#;
        }

        #[test]
        fn it_parses_a_configuration_string() {
            let _: Configuration =
                show_parsed_deserialization(fixtures::CONFIG, serde_json::from_str);
        }

        #[test]
        fn print_config() {
            let config = get_config();
            let str = serde_json::to_string_pretty(&config);
            match &str {
                Err(e) => eprintln!("Failed to serialize configuration: {:#?}", e),
                Ok(s) => println!("{}", s),
            }
            assert!(str.is_ok());
            let s = str.unwrap();

            let _: Configuration = show_parsed_deserialization(&s, serde_json::from_str);
        }
    }

    #[cfg(all(
        feature = "yaml_config",
        feature = "danger",
        not(feature = "json_config")
    ))]
    mod yaml {
        use super::*;

        mod fixtures {
            pub const CONFIG_YAML: &str = r#"
              api: v1
              system:
                name: system-name
                upstream:
                  name: outbound|443||multitenant.3scale.net
                  url: "https://istiodevel-admin.3scale.net/"
                  timeout: 5000
                token: atoken
              backend:
                name: backend-name
                upstream:
                  name: outbound|443||su1.3scale.net
                  url: "https://su1.3scale.net/"
                  timeout: 5000
                extensions:
                  - no_body
              services:
                - id: "2555417834780"
                  token: service_token
                  authorities:
                    - ingress
                    - web
                    - web.app
                    - 0.0.0.0
                    - "0.0.0.0:8080"
                    - "0.0.0.0:8443"
                  credentials:
                    user_key:
                      - query_string:
                          keys:
                            - api_key
                          ops:
                            - format:
                                joined:
                                  separator: ":"
                                  max: 2
                                  indexes:
                                    - 0
                    app_id:
                      - filter:
                          path:
                            - envoy.filters.http.jwt_authn
                            - "0"
                          keys:
                            - azp
                            - aud
                          ops: ~
                      - header:
                          keys:
                            - x-jwt-payload
                          ops:
                            - decode: base64_urlsafe
                            - format:
                                json:
                                  path: []
                                  keys:
                                    - azp
                                    - aud
                      - header:
                          keys:
                            - x-app-id
                          ops: ~
                  mapping_rules:
                    - method: ANY
                      pattern: /
                      usages:
                        - name: Hits
                          delta: 1
            "#;
        }

        #[test]
        fn it_parses_a_configuration_string() {
            let _: Configuration =
                super::show_parsed_deserialization(fixtures::CONFIG_YAML, serde_yaml::from_str);
        }

        #[test]
        fn print_config() {
            let config = get_config();
            let str = serde_yaml::to_string(&config); // to_string_pretty(&config);
            match &str {
                Err(e) => eprintln!("Failed to serialize configuration: {:#?}", e),
                Ok(s) => println!("{}", s),
            }
            assert!(str.is_ok());
            let s = str.unwrap();

            let _: Configuration = show_parsed_deserialization(&s, serde_yaml::from_str);
        }
    }
}
