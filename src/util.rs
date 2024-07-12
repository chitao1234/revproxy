use std::error::Error;
use std::fmt::Display;

use http::{Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::body::{Body, Bytes};
use tracing::error;

use crate::BoxError;

// TODO: Error handling

pub fn rust_error_to_page<D>(
    result: Result<Response<D>, BoxError>,
) -> Response<BoxBody<Bytes, BoxError>>
where
    D: Body<Data = Bytes> + Send + Sync + 'static,
    D::Error: Error + Send + Sync,
{
    result
        .map(|resp| {
            resp.map(|body| {
                body.map_err(|e| {
                    error!("{}", e);
                    e.into()
                })
                .boxed()
            })
        })
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
