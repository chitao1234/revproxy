use std::{convert::Infallible, fmt::Display};

use anyhow::anyhow;
use futures_util::{Future, FutureExt};
use http::{header::HOST, Request, Response, StatusCode};
use hyper::Body;
use reqwest::Response as ReqwestResponse;
use tracing::info;

use crate::{BoxError, ResponseResult, RevProxyRequest};

// TODO: Error handling

pub(super) async fn send_hyper_request(
    rev_req: RevProxyRequest,
    request: Request<hyper::body::Body>,
) -> Result<reqwest::Response, BoxError> {
    let (parts, body) = request.into_parts();
    let client = reqwest::Client::builder().build()?;

    let mut headers = parts.headers;
    headers.remove(HOST);

    let response = client
        .request(parts.method, rev_req.dest)
        .version(parts.version)
        .headers(headers)
        .body(body)
        .send()
        .await?;

    info!("Response: {:?}", response);

    Ok(response)
}

pub(super) async fn transform_reqwest_response(response: ReqwestResponse) -> ResponseResult {
    let mut new_response = Response::builder()
        .version(response.version())
        .status(response.status());
    *new_response
        .headers_mut()
        .ok_or(anyhow!("Failed building response!"))? = response.headers().clone();
    let new_resp = new_response.body(Body::wrap_stream(response.bytes_stream()))?;

    Ok(new_resp)
}

pub fn rust_error_to_page<E: Display>(
    result: impl Future<Output = Result<Response<Body>, E>>,
) -> impl Future<Output = Result<Response<Body>, Infallible>> {
    result.map(|result| result.or_else(move |err| Ok(unprocessable_entity(err))))
}

// TODO: Better page
fn unprocessable_entity<E: Display>(err: E) -> Response<Body> {
    Response::builder()
        .status(StatusCode::UNPROCESSABLE_ENTITY)
        .body(err.to_string().into())
        .unwrap()
}
