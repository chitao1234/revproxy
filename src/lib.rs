use core::fmt;
use std::ops::DerefMut;
use std::{error::Error, ops::Deref};

use crate::rewrite::RewriteRules;
use anyhow::anyhow;
use encoding_rs::Encoding;
use fancy_regex::Regex;
use futures_util::StreamExt;
use http::{header::HOST, Uri};
use http_body_util::{BodyStream, Full};
use hyper::{
    body::{Body, Bytes},
    Request, Response,
};
use mime::Mime;
use reqwest::Response as ReqwestResponse;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};

pub mod rewrite;
pub mod server;
pub mod util;

type StdError = dyn Error + Send + Sync + 'static;
type BoxError = Box<StdError>;
type ResponseResult<B, E2 = BoxError> = Result<Response<B>, E2>;

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct RevProxyRequest {
    #[serde(rename = "default", default)]
    no_remove_host: bool,
    #[serde(default)]
    append: Vec<(String, String)>,
    #[serde(default)]
    drop: Vec<String>,
    #[serde(default = "_default_true")]
    host_rewrite: bool,
    custom_rewrite: Option<Vec<RewriteRules>>,
    #[serde(skip)]
    local_addr: String,
    dest: UriWrapper,
}

const fn _default_true() -> bool {
    true
}

// reqwest::Client is clone so no need to wrap it in Arc
#[derive(Debug, Clone)]
pub struct RevProxyClient {
    client: reqwest::Client,
}

impl RevProxyClient {
    pub async fn revproxy<B>(
        &self,
        rev_req: RevProxyRequest,
        request: Request<B>,
    ) -> ResponseResult<Full<Bytes>>
    where
        B: Body + Send + Sync + 'static,
        <B as Body>::Error: std::error::Error + Send + Sync,
        Bytes: From<<B as Body>::Data>,
        <B as Body>::Data: std::fmt::Debug + Send + Sync,
    {
        let (parts, body) = request.into_parts();

        let mut headers = parts.headers;
        if !rev_req.no_remove_host {
            headers.remove(HOST);
        }
        for key in &rev_req.drop {
            headers.remove(key);
        }
        for (key, value) in &rev_req.append {
            headers.append(http::HeaderName::try_from(key)?, value.try_into()?);
        }

        // TODO: http version?
        let response = self
            .client
            .request(parts.method, rev_req.dest.to_string())
            .headers(headers)
            .body(reqwest::Body::wrap_stream(
                BodyStream::new(body).map(|result| result.map(|frame| frame.into_data().unwrap())),
            ))
            .send()
            .await?;

        trace!("Response: {:?}", response);

        Self::transform_reqwest_response(&rev_req, response).await
    }

    // remove :80 and :443 from authority
    fn sanitize_authority(uri: &Uri) -> Result<String, BoxError> {
        debug!("{:?}", uri);
        Ok(uri
            .authority()
            .map(|authority| {
                let mut auth = authority.as_str().to_owned();
                if auth.ends_with(":443") || auth.ends_with(":80") {
                    auth = auth.replace(":443", "").replace(":80", "");
                }
                auth
            })
            .ok_or(anyhow!("Uri does not contain authority."))?)
    }

    // TODO: Error handling
    async fn transform_reqwest_response(
        rev_req: &RevProxyRequest,
        response: ReqwestResponse,
    ) -> ResponseResult<Full<Bytes>> {
        let mut new_response = Response::builder()
            .version(response.version())
            .status(response.status());

        *new_response
            .headers_mut()
            .ok_or(anyhow!("Failed building response!"))? = response.headers().clone();

        let host = Self::sanitize_authority(&response.url().to_string().parse()?)?;
        info!(host);

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok());
        let encoding_name = content_type
            .as_ref()
            .and_then(|mime| {
                mime.get_param(mime::CHARSET)
                    .map(|charset| charset.as_str())
            })
            .unwrap_or("utf-8");
        let encoding = Encoding::for_label(encoding_name.as_bytes())
            .ok_or(anyhow!("Unable to get encoding: {}", encoding_name))?;

        let body = response.bytes().await?;
        let (text, _, decode_error) = encoding.decode(&body);

        let mut text = text.into_owned();
        trace!("Response after decode: {}", text);

        let s = if decode_error {
            // XXX: if error, assumes binary, do nothing
            info!("Decode error, not replacing.");
            body
        } else {
            if rev_req.host_rewrite {
                let new_req = RevProxyRequest {
                    // dest: host,
                    ..rev_req.clone()
                };
                let query = serde_qs::to_string(&new_req)?;
                let replace = Uri::builder()
                    .authority(rev_req.local_addr.as_str())
                    .path_and_query("/?".to_owned() + &query)
                    .scheme("PLACEHOLDER")
                    .build()?
                    .to_string();
                let replace = replace.replace("PLACEHOLDER://", ""); // FIXME: dirty hack
                debug!("Replaceing {} with {}", new_req.dest, replace);
                text = text.replace(&new_req.dest.to_string(), &replace);
            }

            if let Some(rules) = rev_req.custom_rewrite.as_ref() {
                for rule in rules {
                    // FIXME: Why clone?????
                    let regex: Regex = rule.find.clone().into();
                    text = regex.replace_all(&text, &rule.replace).into_owned();
                }
            }
            text.into()
        };

        let new_resp = new_response.body(Full::new(s))?;
        Ok(new_resp)
    }
}

impl From<reqwest::Client> for RevProxyClient {
    fn from(value: reqwest::Client) -> Self {
        Self { client: value }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct UriWrapper {
    uri: Uri,
}

impl<'de> Deserialize<'de> for UriWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let uri = String::deserialize(deserializer)?;
        Ok(Self {
            uri: uri.parse().map_err(serde::de::Error::custom)?,
        })
    }
}

impl Serialize for UriWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.uri.to_string().serialize(serializer)
    }
}

impl TryFrom<String> for UriWrapper {
    type Error = BoxError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Self {
            uri: value.parse()?,
        })
    }
}

impl From<Uri> for UriWrapper {
    fn from(value: Uri) -> Self {
        Self { uri: value }
    }
}

impl From<UriWrapper> for Uri {
    fn from(value: UriWrapper) -> Self {
        value.uri
    }
}

impl Deref for UriWrapper {
    type Target = Uri;

    fn deref(&self) -> &Self::Target {
        &self.uri
    }
}

impl DerefMut for UriWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.uri
    }
}

impl fmt::Display for UriWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.uri)
    }
}

// pub struct RevProxyBuilder<U>
// where
//     U: IntoUrl,
// {
//     proxy: Option<U>,
// }

// impl<U> RevProxyBuilder<U>
// where
//     U: IntoUrl,
// {
//     pub fn proxy(mut self, proxy: U) -> Self {
//         self.proxy = Some(proxy);
//         self
//     }

//     pub fn build(self) -> Result<RevProxyClient, BoxError> {
//         let mut client_builder = reqwest::Client::builder();
//         if let Some(proxy) = self.proxy {
//             client_builder = client_builder.proxy(Proxy::all(proxy)?);
//         }
//         Ok(RevProxyClient {
//             client: client_builder.build()?,
//         })
//     }
// }

// impl<U> Default for RevProxyBuilder<U>
// where
//     U: IntoUrl,
// {
//     fn default() -> Self {
//         Self { proxy: None }
//     }
// }
