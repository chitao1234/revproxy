use std::error::Error;
use std::fmt::Display;

use anyhow::anyhow;
use futures_util::{Stream, StreamExt};
use http::{Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Full, StreamBody};
use hyper::body::{Body, Bytes, Frame};
use reqwest::Error as ReqwestError;
use reqwest::Response as ReqwestResponse;

use crate::{BoxError, ResponseResult};

// TODO: Error handling

pub(super) fn transform_reqwest_response(
    response: ReqwestResponse,
) -> ResponseResult<StreamBody<impl Stream<Item = Result<Frame<Bytes>, ReqwestError>>>> {
    let mut new_response = Response::builder()
        .version(response.version())
        .status(response.status());
    *new_response
        .headers_mut()
        .ok_or(anyhow!("Failed building response!"))? = response.headers().clone();
    let new_resp = new_response.body(StreamBody::new(
        response
            .bytes_stream()
            .map(|data| data.map(|bytes| Frame::data(bytes))),
    ))?;

    Ok(new_resp)
}

pub fn rust_error_to_page<D>(
    result: Result<Response<D>, BoxError>,
) -> Response<BoxBody<Bytes, BoxError>>
where
    D: Body<Data = Bytes> + Send + Sync + 'static,
    D::Error: Error + Send + Sync,
{
    result
        .map(|resp| resp.map(|body| body.map_err(|e| e.into()).boxed()))
        .unwrap_or_else(|e| unprocessable_entity(e))
}

// TODO: Better page
fn unprocessable_entity<E: Display>(err: E) -> Response<BoxBody<Bytes, BoxError>> {
    Response::builder()
        .status(StatusCode::UNPROCESSABLE_ENTITY)
        .body(
            Full::new(Bytes::from(err.to_string()))
                .map_err(|e| e.into())
                .boxed(),
        )
        .unwrap()
}
