use std::error::Error;

use anyhow::anyhow;
use hyper::{body::Body, Request, Response};
use serde::{Deserialize, Serialize};
use tracing::info;

pub mod util;

type StdError = dyn Error + Send + Sync + 'static;
type BoxError = Box<StdError>;
type ResponseResult<E2 = BoxError> = Result<Response<Body>, E2>;

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

#[cfg(test)]
mod tests {
    use crate::RevProxyRequest;

    #[test]
    fn serde() {
        let req_default = RevProxyRequest {
            dest: "http://localhost:8001/?dest=https%3A%2F%2Fexample.com".to_owned(),
            ..Default::default()
        };
        let req_default_str1 =
            "dest=http%3A%2F%2Flocalhost%3A8001%2F%3Fdest%3Dhttps%253A%252F%252Fexample.com&default=false";
        assert_eq!(serde_qs::to_string(&req_default).unwrap(), req_default_str1);
        let req_default_str2 =
            "dest=http%3A%2F%2Flocalhost%3A8001%2F%3Fdest%3Dhttps%253A%252F%252Fexample.com";
        assert_eq!(
            serde_qs::from_str::<RevProxyRequest>(req_default_str2).unwrap(),
            req_default
        );

        let req = RevProxyRequest {
            dest: "https://example.com".to_owned(),
            without_default_header_strategy: false,
            keep: vec!["cookie".to_owned(), "USER-AGENT".to_owned()],
            drop: vec!["accept".to_owned(), "Accept-Language".to_owned()],
        };
        let req_str1 = "dest=https%3A%2F%2Fexample.com&default=false&keep[0]=cookie&keep[1]=USER-AGENT&drop[0]=accept&drop[1]=Accept-Language";
        assert_eq!(serde_qs::to_string(&req).unwrap(), req_str1);
        let req_str2 = "dest=https%3A%2F%2Fexample.com&keep[]=cookie&default=false&keep[]=USER-AGENT&drop[999]=Accept-Language&drop[1]=accept";
        assert_eq!(
            serde_qs::from_str::<RevProxyRequest>(&req_str2).unwrap(),
            req
        );
    }
}

pub async fn revproxy(request: Request<hyper::body::Body>) -> ResponseResult<BoxError> {
    let rev_req = parse_query(request.uri().query())?;
    info!("Parsed request {:?}", rev_req);
    proxy_request(rev_req, request).await
}

// TODO: Custom error type
fn parse_query(query: Option<&str>) -> Result<RevProxyRequest, BoxError> {
    let req = match query {
        Some(query) => {
            info!("Found query: {}", query);
            serde_qs::from_str(query)?
        }
        None => return Err(anyhow!("Query not set.").into()),
    };
    Ok(req)
}

async fn proxy_request(
    rev_req: RevProxyRequest,
    request: Request<hyper::body::Body>,
) -> ResponseResult {
    let response = util::send_hyper_request(rev_req, request).await?;

    // let resp = client.get(dest).send().await?;

    util::transform_reqwest_response(response).await
}

