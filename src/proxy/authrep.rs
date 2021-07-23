use std::collections::HashMap;

use super::request_headers::RequestHeaders;
use super::HttpAuthThreescale;
use crate::threescale::CredentialsError;
use threescalers::{
    api_call::{ApiCall, Kind},
    application::Application,
    extensions,
    http::{mapping_rule::Method, Request},
    service::Service,
    transaction::Transaction,
    usage::Usage,
};

#[derive(Debug, thiserror::Error)]
enum MatchError {
    #[error("no known service matched")]
    NoServiceMatched,
    #[error("credentials error")]
    CredentialsError(#[from] CredentialsError),
    #[error("no usage match")]
    NoUsageMatch,
}

#[derive(Debug, Clone)]
pub struct AuthRep<'a> {
    service: &'a crate::threescale::Service,
    apps: Vec<Application>,
    usages: HashMap<&'a str, i64>,
}

impl<'a> AuthRep<'a> {
    pub fn service(&self) -> &crate::threescale::Service {
        self.service
    }

    pub fn apps(&self) -> &Vec<Application> {
        &self.apps
    }

    pub fn usages(&self) -> &HashMap<&'a str, i64> {
        &self.usages
    }
}

#[allow(dead_code)]
pub fn request(ctx: &HttpAuthThreescale, rh: &RequestHeaders) -> Result<Request, anyhow::Error> {
    let ar = authrep(ctx, rh)?;
    build_call(&ar)
}

pub fn authrep<'a>(
    ctx: &'a HttpAuthThreescale,
    rh: &'a RequestHeaders,
) -> Result<AuthRep<'a>, anyhow::Error> {
    let config = ctx.configuration();
    let svclist = config.get_services()?;

    let metadata = rh.metadata();
    let method = Method::from(metadata.method());
    let url = rh.url()?;
    let authority = url.authority();
    let path = url.path();
    let mut pattern = path.to_string();
    let qs = url.query();
    if let Some(qs) = qs {
        pattern.push('?');
        pattern.push_str(qs);
    }

    let service = svclist
        .iter()
        .find(|&svc| svc.match_authority(authority))
        .ok_or(MatchError::NoServiceMatched)?;

    let credentials = service.credentials();

    let apps = credentials.resolve(ctx, rh, &url)?;

    debug!(ctx, "found credentials, values {:#?}", apps);
    if apps.len() > 1 {
        debug!(
            ctx,
            "found more than one source match for application - going to use {:?}", apps[0]
        );
    }

    let mut usages = std::collections::HashMap::new();
    for rule in service.mapping_rules() {
        debug!(
            ctx,
            "matching pat {} against rule {:#?}",
            pattern.as_str(),
            rule
        );
        if rule.is_match(&method, pattern.as_str()) {
            debug!(ctx, "matched pattern in {}", pattern);
            for usage in rule.usages() {
                let value = usages.entry(usage.name()).or_insert(0);
                *value += usage.delta();
            }
        }
    }

    if usages.is_empty() {
        anyhow::bail!(MatchError::NoUsageMatch);
    }

    Ok(AuthRep {
        service,
        apps,
        usages,
    })
}

pub fn build_call(ar: &AuthRep) -> Result<Request, anyhow::Error> {
    let apps = ar.apps();

    if apps.is_empty() {
        anyhow::bail!("could not extract application credentials");
    }

    //if apps.len() > 1 {
    //    debug!(
    //        "found more than one source match for application - going to use {:?}",
    //        apps[0]
    //    );
    //}

    let app = &apps[0];

    let usage = ar
        .usages()
        .iter()
        .map(|(k, v)| (k, format!("{}", v)))
        .collect::<Vec<_>>();
    let usage = Usage::new(usage.as_slice());
    let txn = Transaction::new(app, None, Some(&usage), None);
    let txns = vec![txn];
    let extensions = extensions::List::new().no_body();

    let service = ar.service();

    let service = Service::new(
        service.id(),
        threescalers::credentials::Credentials::ServiceToken(service.token().into()),
    );
    let mut apicall = ApiCall::builder(&service);
    // the builder here can only fail if we fail to set a kind
    let apicall = apicall
        .transactions(&txns)
        .extensions(&extensions)
        .kind(Kind::AuthRep)
        .build()?;

    Ok(Request::from(&apicall))
}
