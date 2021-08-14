use std::borrow::Cow;

use log::debug;
use serde::{Deserialize, Serialize};

use super::Operation;
use crate::proxy::{
    metadata::{Metadata, ValueExt},
    request_headers::RequestHeaders,
    HttpAuthThreescale,
};

const METADATA: &[&str] = &["metadata"];
//TODO static METADATA_VEC: Vec<&str> = METADATA.into(); // via lazy_static or some similar mechanism

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Header {
        keys: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ops: Option<Vec<Operation>>,
    },
    QueryString {
        keys: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ops: Option<Vec<Operation>>,
    },
    Filter {
        #[serde(default)]
        path: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        keys: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ops: Option<Vec<Operation>>,
    },
}

impl Source {
    pub fn resolve<'url, 'a: 'url>(
        &'a self,
        ctx: &'a HttpAuthThreescale,
        rh: &'a RequestHeaders,
        url: &'url url::Url,
    ) -> Option<Vec<Cow<'a, str>>> {
        use proxy_wasm::traits::Context;

        let res = match self {
            Source::QueryString { keys, ops } => {
                keys.iter().map(std::ops::Deref::deref).find_map(|key| {
                    url.query_pairs().find_map(|(k, v)| {
                        if key == k.as_ref() {
                            // must extract from v and create a new Cow with owned data to stop relying on 'url url
                            Some((vec![v.into_owned().into()], ops))
                        } else {
                            None
                        }
                    })
                })
            }
            Source::Header { keys, ops } => {
                debug!("looking for headers");
                keys.iter().map(std::ops::Deref::deref).find_map(|key| {
                    debug!("looking for header {}", key);
                    rh.get(key).map(|v| {
                        debug!("found header {} with value {} - ops {:?}", key, v, ops,);
                        (vec![Cow::from(v)], ops)
                    })
                })
            }
            Source::Filter { path, keys, ops } => {
                let keys = keys.iter().map(String::as_str).collect::<Vec<_>>();
                let path = if path.is_empty() {
                    return None;
                } else {
                    path.iter().map(String::as_str).collect::<Vec<_>>()
                };
                let path_s = path.join("/");
                debug!("Looking up metadata property path {}", path_s);
                // Note: we are paying the prices of:
                //
                // 1. Creating a new vec on each call - fixable with some sort of lazy static or const stuff.
                // 2. Getting the full metadata set - not fixable until upstream fixes their stuff, as the alternative pairs typing is... horrendous.
                if let Some(property) = ctx.get_property(METADATA.into()) {
                    debug!("asked for {:?} property", METADATA);

                    let proto = Metadata::new(property.as_slice());

                    if let Ok(metadata) = proto {
                        debug!("parsed global metadata as ok");
                        let r = metadata
                        .lookup(path[0], &path[1..])
                        .map(|(v, _segment)| {
                            keys.iter().find_map(|&k| v.lookup(&[k]).ok())
                                .map(|(v, segment)| {
                                    v.as_str().map(|s| vec![s])
                                        .or_else(|| v.as_list().and_then(|l|
                                            l.values.iter().try_fold(vec![], |mut acc, v| {
                                                v.as_str()
                                                    .map(|s| {
                                                        acc.push(s);
                                                        acc
                                                    })
                                            })
                                        ))
                                        .or_else(|| v.as_struct().and_then(|st| {
                                            if st.fields.len() == 1 {
                                                st.fields.values().next().and_then(|v| {
                                                    v.as_str().map(|s| vec![s])
                                                        .or_else(|| v.as_list().and_then(|l|
                                                            l.values.iter().try_fold(vec![], |mut acc, v| {
                                                            v.as_str()
                                                                .map(|s| {
                                                                    acc.push(s);
                                                                    acc
                                                                })
                                                            })
                                                        ))
                                                })
                                            } else {
                                                None
                                            }
                                        }))
                                    .ok_or_else(|| {
                                        format!("a string, non empty list of strings, or one-element struct mapping to a string is needed to obtain a value - got a {} at {}", v.kind().as_str(), segment)
                                    })
                                })
                            });

                        // no flatten in stable yet :/
                        match r {
                            // must own the string, as it references the property vec
                            Ok(Some(Ok(v))) => Some((
                                v.into_iter().map(|s| Cow::from(s.to_string())).collect(),
                                ops,
                            )),
                            Err(e) => {
                                debug!("failed to fetch metadata: {}", e);
                                None
                            }
                            Ok(Some(Err(e))) => {
                                debug!("failed to fetch metadata: {}", e);
                                None
                            }
                            Ok(None) => {
                                debug!("failed to fetch metadata");
                                None
                            }
                        }
                    } else {
                        debug!("parsing global metadata failed");
                        None
                    }
                } else {
                    debug!("Property path not found {}", path_s);
                    None
                }
            }
        };

        res.and_then(|(values, ops)| {
            if let Some(ops) = ops {
                let ops = ops.iter().collect::<Vec<_>>();
                super::process_operations(values, ops.as_slice()).ok()
            } else {
                Some(values)
            }
        })
    }
}
