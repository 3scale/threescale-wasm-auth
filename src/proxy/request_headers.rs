use log::debug;
use proxy_wasm::traits::HttpContext;
use std::convert::TryFrom;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
enum MetadataError {
    #[error("missing authority")]
    Authority,
    #[error("missing path")]
    Path,
}

mod helpers {
    pub fn parse_path_n_qs(path_n_qs: &str) -> (&str, Option<&str>) {
        let mut v = path_n_qs.splitn(2, '?');
        let path = v.next().unwrap();
        (path, v.next())
    }

    fn extract_cookies(cookie_value: &str) -> impl Iterator<Item = (&str, Option<&str>)> {
        cookie_value.split(';').map(|kv| {
            let mut kviter = kv.splitn(2, '=');
            (kviter.next().unwrap(), kviter.next())
        })
    }
    pub fn get_cookie<'a>(cookie_value: &'a str, name: &str) -> Option<Option<&'a str>> {
        extract_cookies(cookie_value)
            .find_map(|(cookie, v)| if cookie == name { Some(v) } else { None })
    }
}

pub struct RequestMetadata<'a> {
    scheme: &'a str,
    authority: &'a str,
    method: &'a str,
    path: &'a str,
    qs: Option<&'a str>,
}

impl RequestMetadata<'_> {
    pub const fn scheme(&self) -> &str {
        self.scheme
    }

    pub const fn authority(&self) -> &str {
        self.authority
    }

    pub const fn method(&self) -> &str {
        self.method
    }

    pub const fn path(&self) -> &str {
        self.path
    }

    pub const fn qs(&self) -> Option<&str> {
        self.qs
    }
}

pub struct RequestHeaders(Vec<(String, String)>);

impl RequestHeaders {
    pub fn new(ctx: &dyn HttpContext) -> Self {
        Self(ctx.get_http_request_headers())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find_map(|(h, v)| if h == name { Some(v.as_str()) } else { None })
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (String, String)> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, (String, String)> {
        self.0.iter_mut()
    }

    pub fn get_cookie_from_header(&self, header: &str, name: &str) -> Option<Option<&str>> {
        match self.get(header) {
            Some(cookie_value) => helpers::get_cookie(cookie_value, name),
            None => None,
        }
    }

    pub fn path_n_qs(&self) -> (&str, Option<&str>) {
        helpers::parse_path_n_qs(self.get(":path").unwrap())
    }

    pub fn metadata(&self) -> RequestMetadata {
        let (path, qs) = self.path_n_qs();

        RequestMetadata {
            scheme: self.get_scheme(),
            authority: self.get(":authority").unwrap(),
            method: self.get(":method").unwrap(),
            path,
            qs,
        }
    }

    pub fn url(&self) -> Result<Url, anyhow::Error> {
        debug!("headers: {:?}", self.0);

        let scheme = self.get_scheme();
        let authority = self.get(":authority").ok_or(MetadataError::Authority)?;
        let path = self.get(":path").ok_or(MetadataError::Path)?;
        let url = Url::parse(format!("{}://{}{}", scheme, authority, path).as_str())?;
        Ok(url)
    }

    fn get_scheme(&self) -> &str {
        //let scheme = self.get(":scheme").ok_or(MetadataError::Scheme)?;
        self.get(":scheme")
            .or_else(|| self.get("x-forwarded-proto"))
            .unwrap_or("https")
    }
}

impl core::iter::IntoIterator for RequestHeaders {
    type Item = <Vec<(String, String)> as core::iter::IntoIterator>::Item;

    type IntoIter = <Vec<(String, String)> as core::iter::IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl TryFrom<&RequestHeaders> for Url {
    type Error = anyhow::Error;

    fn try_from(rh: &RequestHeaders) -> Result<Self, Self::Error> {
        rh.url()
    }
}
