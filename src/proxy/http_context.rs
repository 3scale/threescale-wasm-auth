use proxy_wasm::traits::{Context, HttpContext};
use proxy_wasm::types::FilterHeadersStatus;
use threescalers::application::Application;

use crate::configuration::Configuration;
use crate::log::IdentLogger;
use crate::threescale::CredentialsError;

use super::authrep;
use super::authrep::MatchError;
use super::request_headers::RequestHeaders;

pub struct HttpAuthThreescale {
    pub configuration: Configuration,
    pub context_id: u32,
    pub id: u32,
    pub log_id: String,
}

impl IdentLogger for HttpAuthThreescale {
    fn ident(&self) -> &str {
        self.log_id.as_str()
    }
}

impl HttpAuthThreescale {
    //pub const fn configuration(&self) -> &Configuration {
    pub fn configuration(&self) -> &crate::configuration::api::v1::Configuration {
        self.configuration.get()
    }
}

impl HttpContext for HttpAuthThreescale {
    fn on_http_request_headers(&mut self, _: usize) -> FilterHeadersStatus {
        info!(
            self,
            "on_http_request_headers: context_id {}", self.context_id
        );

        //let backend = match self.configuration.get_backend() {
        //    Err(e) => {
        //        error!("error obtaining configuration for 3scale backend: {:?}", e);
        //        return FilterHeadersStatus::Continue;
        //    }
        //    Ok(backend) => backend,
        //};

        let backend = self.configuration().get_backend().ok();

        let rh = RequestHeaders::new(self);

        let ar = match authrep::authrep(self, &rh) {
            Err(e) => {
                error!(self, "error computing authrep {:?}", e);
                match e.downcast_ref::<MatchError>() {
                    Some(MatchError::NoServiceMatched) => {
                        self.send_http_response(403, vec![], Some(b"Unknown service\n"))
                    }
                    Some(MatchError::NoUsageMatch) => {
                        self.send_http_response(404, vec![], Some(b"No Mapping Rule matched\n"))
                    }
                    Some(MatchError::CredentialsError(_)) => self.send_http_response(
                        403,
                        vec![],
                        Some(b"Authentication parameters missing\n"),
                    ),
                    _ => self.send_http_response(403, vec![], Some(b"Authentication failed\n")),
                };
                debug!(self, "403 sent");
                return FilterHeadersStatus::StopIteration;
            }
            Ok(params) => params,
        };

        let passthrough_metadata: bool = self.configuration().passthrough_metadata.unwrap_or(false);

        if passthrough_metadata {
            match self.threescale_info_to_metadata(&ar) {
                Ok(()) => return FilterHeadersStatus::Continue,
                Err(e) => {
                    error!(self, "failed to pass app info to next filter: {:?}", e);
                    self.send_http_response(403, vec![], Some(b"Authentication failed\n"));
                    debug!(self, "403 sent");
                    return FilterHeadersStatus::StopIteration;
                }
            }
        }

        if let Some(backend) = backend {
            let request = match authrep::build_call(&ar) {
                Err(e) => {
                    error!(self, "error computing authrep request {:?}", e);
                    let message = match e.downcast_ref::<CredentialsError>() {
                        Some(CredentialsError::NotFound) => "Authentication parameters missing\n",
                        _ => "Authentication failed\n",
                    };
                    self.send_http_response(403, vec![], Some(message.as_bytes()));
                    debug!(self, "403 sent");
                    return FilterHeadersStatus::StopIteration;
                }
                Ok(request) => request,
            };

            // uri will actually just get the whole path + parameters
            let (uri, body) = request.uri_and_body();

            let headers = request
                .headers
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect::<Vec<_>>();

            let upstream = backend.upstream();
            let call_token = match upstream.call(
                self,
                uri.as_ref(),
                request.method.as_str(),
                headers,
                None,
                body.map(str::as_bytes),
                None,
                None,
            ) {
                Ok(call_token) => call_token,
                Err(e) => {
                    error!(self, "on_http_request_headers: could not dispatch HTTP call to {}: did you create the cluster to do so? - {:#?}", upstream.name(), e);
                    self.send_http_response(403, vec![], Some(b"Authentication failed\n"));
                    debug!(self, "403 sent");
                    return FilterHeadersStatus::StopIteration;
                }
            };

            info!(
                self,
                "on_http_request_headers: call token is {}", call_token
            );

            FilterHeadersStatus::StopIteration
        } else {
            // no backend configured
            debug!(self, "on_http_request_headers: no backend configured");
            self.send_http_response(403, vec![], Some(b"Authentication failed\n"));
            debug!(self, "403 sent");
            FilterHeadersStatus::StopIteration
        }
    }

    fn on_http_response_headers(&mut self, _: usize) -> FilterHeadersStatus {
        self.set_http_response_header("Powered-By", Some("3scale"));
        FilterHeadersStatus::Continue
    }
}

impl Context for HttpAuthThreescale {
    fn on_http_call_response(&mut self, token_id: u32, _: usize, _: usize, _: usize) {
        info!(
            self,
            "http_ctx: on_http_call_response: token id is {}", token_id
        );
        let status_code = match self
            .get_http_call_response_headers()
            .into_iter()
            .find(|(key, _)| key.as_str() == ":status")
        {
            None => {
                debug!(
                    self,
                    "on_http_call_response: empty status header {}", token_id
                );
                self.send_http_response(502, vec![], Some(b"Bad Gateway\n"));
                return;
            }
            Some((_, code)) => code.parse::<u32>().unwrap_or(500),
        };

        info!(
            self,
            "on_http_call_response: received {} response {}", status_code, token_id
        );
        if status_code == 200 {
            info!(self, "on_http_call_response: authorized {}", token_id);
            self.resume_http_request();
        } else {
            let rejection_reason = match self
                .get_http_call_response_headers()
                .into_iter()
                .find(|(key, _)| key.as_str() == "3scale-rejection-reason")
            {
                None => "None".to_string(),
                Some((_, reason)) => reason,
            };

            if rejection_reason == "limits_exceeded" {
                info!(self, "on_http_call_response: limits_exceeded {}", token_id);
                self.send_http_response(429, vec![], Some(b"Usage limit exceeded\n"));
            } else {
                info!(self, "on_http_call_response: not authorized {}", token_id);
                self.send_http_response(403, vec![], Some(b"Authentication failed\n"));
            }
        }
    }
}

impl HttpAuthThreescale {
    fn threescale_info_to_metadata(&self, ar: &authrep::AuthRep) -> Result<(), anyhow::Error> {
        let apps = ar.apps();
        let service = ar.service();
        let usages = ar.usages();

        if apps.is_empty() {
            anyhow::bail!(CredentialsError::NotFound);
        }

        let backend = self.configuration().backend();
        if backend.is_none() {
            anyhow::bail!("backend not configured");
        }
        let upstream = backend.unwrap().upstream();

        let mut app_id_key = String::new();
        let (header, value) = match &apps[0] {
            Application::AppId(app_id, app_key) => {
                app_id_key.push_str(app_id.as_ref());
                if let Some(key) = app_key {
                    app_id_key.push(':');
                    app_id_key.push_str(key.as_ref());
                }
                ("x-3scale-app-id", app_id_key.as_str())
            }
            Application::UserKey(user_key) => ("x-3scale-user-key", user_key.as_ref()),
            Application::OAuthToken(_token) => anyhow::bail!("Oauth token not supported"),
        };

        // Adding threescale info as request headers
        self.add_http_request_header(header, value);
        self.add_http_request_header("x-3scale-cluster-name", upstream.name());
        self.add_http_request_header("x-3scale-upstream-url", upstream.url.as_str());
        self.add_http_request_header("x-3scale-timeout", &upstream.default_timeout().to_string());
        self.add_http_request_header("x-3scale-service-id", service.id());
        let service_token = if let Some(token) = service.token() {
            token
        } else {
            // without service token, other info is useless.
            anyhow::bail!("service token unavailable");
        };
        self.add_http_request_header("x-3scale-service-token", service_token);
        self.add_http_request_header("x-3scale-usages", &serde_json::to_string(&usages)?);
        Ok(())
    }
}
