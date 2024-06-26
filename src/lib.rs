use std::error::Error;

use anyhow::anyhow;
use futures_util::{Stream, StreamExt};
use http::header::HOST;
use http_body_util::{BodyStream, StreamBody};
use hyper::{
    body::{Body, Bytes, Frame},
    Request, Response,
};
use reqwest::Error as ReqwestError;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

pub mod util;

type StdError = dyn Error + Send + Sync + 'static;
type BoxError = Box<StdError>;
type ResponseResult<B, E2 = BoxError> = Result<Response<B>, E2>;

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
struct RevProxyRequest {
    dest: String,
    #[serde(rename = "default")]
    #[serde(default)]
    without_default_header_strategy: bool,
    #[serde(default)]
    keep: Vec<String>,
    #[serde(default)]
    drop: Vec<String>,
}

// #[cfg(test)]
// mod tests {
//     use crate::RevProxyRequest;

//     #[test]
//     fn serde() {
//         let req_default = RevProxyRequest {
//             dest: "http://localhost:8001/?dest=https%3A%2F%2Fexample.com".to_owned(),
//             ..Default::default()
//         };
//         let req_default_str1 =
//             "dest=http%3A%2F%2Flocalhost%3A8001%2F%3Fdest%3Dhttps%253A%252F%252Fexample.com&default=false";
//         assert_eq!(serde_qs::to_string(&req_default).unwrap(), req_default_str1);
//         let req_default_str2 =
//             "dest=http%3A%2F%2Flocalhost%3A8001%2F%3Fdest%3Dhttps%253A%252F%252Fexample.com";
//         assert_eq!(
//             serde_qs::from_str::<RevProxyRequest>(req_default_str2).unwrap(),
//             req_default
//         );

//         let req = RevProxyRequest {
//             dest: "https://example.com".to_owned(),
//             without_default_header_strategy: false,
//             keep: vec!["cookie".to_owned(), "USER-AGENT".to_owned()],
//             drop: vec!["accept".to_owned(), "Accept-Language".to_owned()],
//         };
//         let req_str1 = "dest=https%3A%2F%2Fexample.com&default=false&keep[0]=cookie&keep[1]=USER-AGENT&drop[0]=accept&drop[1]=Accept-Language";
//         assert_eq!(serde_qs::to_string(&req).unwrap(), req_str1);
//         let req_str2 = "dest=https%3A%2F%2Fexample.com&keep[]=cookie&default=false&keep[]=USER-AGENT&drop[999]=Accept-Language&drop[1]=accept";
//         assert_eq!(
//             serde_qs::from_str::<RevProxyRequest>(&req_str2).unwrap(),
//             req
//         );
//     }
// }

pub struct RevProxyClient {
    client: reqwest::Client,
}

impl RevProxyClient {
    // pub fn builder<U: IntoUrl>() -> RevProxyBuilder<U> {
    //     RevProxyBuilder::default()
    // }

    pub async fn revproxy<B>(
        &self,
        request: Request<B>,
    ) -> ResponseResult<StreamBody<impl Stream<Item = Result<Frame<Bytes>, ReqwestError>>>>
    where
        B: Body + Send + Sync + 'static,
        <B as Body>::Error: std::error::Error + Send + Sync,
        Bytes: From<<B as Body>::Data>,
        <B as Body>::Data: std::fmt::Debug + Send + Sync,
    {
        let rev_req = self.parse_query(request.uri().query())?;
        info!("Parsed request {:?}", rev_req);

        let (parts, body) = request.into_parts();

        let mut headers = parts.headers;
        headers.remove(HOST);

        let response = self
            .client
            .request(parts.method, rev_req.dest)
            .headers(headers)
            .body(reqwest::Body::wrap_stream(
                BodyStream::new(body).map(|result| result.map(|frame| frame.into_data().unwrap())),
            ))
            .send()
            .await?;

        debug!("Response: {:?}", response);

        util::transform_reqwest_response(response)
    }

    // TODO: Custom error type
    fn parse_query(&self, query: Option<&str>) -> Result<RevProxyRequest, BoxError> {
        let req = match query {
            Some(query) => {
                info!("Found query: {}", query);
                serde_qs::from_str(query)?
            }
            None => return Err(anyhow!("Query not set.").into()),
        };
        Ok(req)
    }
}

impl From<reqwest::Client> for RevProxyClient {
    fn from(value: reqwest::Client) -> Self {
        Self { client: value }
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
