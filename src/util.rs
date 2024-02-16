use std::fmt::Display;

use anyhow::anyhow;
use http::{Response, StatusCode};
use hyper::Body;
use reqwest::Response as ReqwestResponse;

use crate::ResponseResult;

// TODO: Error handling

pub(super) fn transform_reqwest_response(response: ReqwestResponse) -> ResponseResult {
    let mut new_response = Response::builder()
        .version(response.version())
        .status(response.status());
    *new_response
        .headers_mut()
        .ok_or(anyhow!("Failed building response!"))? = response.headers().clone();
    let new_resp = new_response.body(Body::wrap_stream(response.bytes_stream()))?;

    Ok(new_resp)
}

pub fn rust_error_to_page<E: Display>(result: Result<Response<Body>, E>) -> Response<Body> {
    result.unwrap_or_else(move |err| unprocessable_entity(err))
}

// TODO: Better page
fn unprocessable_entity<E: Display>(err: E) -> Response<Body> {
    Response::builder()
        .status(StatusCode::UNPROCESSABLE_ENTITY)
        .body(err.to_string().into())
        .unwrap()
}
