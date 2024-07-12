use std::{convert::Infallible, fmt, pin::Pin};

use futures_util::Future;
use http::{header::{ToStrError, HOST}, HeaderValue, Request, Response};
use http_body_util::Full;
use hyper::{
    body::{Body, Bytes},
    service::Service,
};
use tracing::{debug, error, info};

use crate::{BoxError, RevProxyClient, RevProxyRequest};

type ServerResult<T> = Result<T, RevProxyServerError>;

// Client is a wrapper around reqwest::Client, just clone it
#[derive(Debug, Clone)]
pub struct RevProxyServer {
    client: RevProxyClient,
    use_host_field: bool,
    use_query: bool,
}

impl RevProxyServer {
    pub fn new(client: RevProxyClient, use_host_field: bool, use_query: bool) -> Self {
        Self {
            client: client,
            use_host_field,
            use_query,
        }
    }

    pub fn mk_response(s: String) -> Result<Response<Full<Bytes>>, Infallible> {
        Ok(Response::builder().body(Full::new(Bytes::from(s))).unwrap())
    }

    pub async fn do_request_wrap_error<ReqBody>(
        &self,
        // addr: &SocketAddr,
        req: Request<ReqBody>,
    ) -> Result<Response<Full<Bytes>>, Infallible>
    where
        ReqBody: Body + Send + Sync + 'static,
        <ReqBody as Body>::Error: std::error::Error + Send + Sync,
        Bytes: From<<ReqBody as Body>::Data>,
        <ReqBody as Body>::Data: std::fmt::Debug + Send + Sync,
    {
        Ok(self.do_request(req).await.unwrap_or_else(|e| {
            error!("Error processing request: {:?}", e);
            Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from_static(
                    "Internal server error".as_bytes(),
                )))
                .unwrap()
        }))
    }

    pub async fn do_request<ReqBody>(
        &self,
        // addr: &SocketAddr,
        req: Request<ReqBody>,
    ) -> ServerResult<Response<Full<Bytes>>>
    where
        ReqBody: Body + Send + Sync + 'static,
        <ReqBody as Body>::Error: std::error::Error + Send + Sync,
        Bytes: From<<ReqBody as Body>::Data>,
        <ReqBody as Body>::Data: std::fmt::Debug + Send + Sync,
    {
        let rev_req = self.parse_request(&req)?;
        info!("Parsed request {:?}", rev_req);

        self.client
            .revproxy(rev_req, req)
            .await
            .map_err(|e| e.into())
    }

    pub fn parse_request<ReqBody>(&self, req: &Request<ReqBody>) -> ServerResult<RevProxyRequest> {
        let uri = req.uri();
        let mut request;
        if self.use_query {
            let res = Self::process_query(uri.query());
            if res.is_err() {
                let err = res.unwrap_err();
                error!("Error processing query: {}", err);
                return Err(err);
            }
            request = res.unwrap();
        } else {
            request = RevProxyRequest::default();
            if self.use_host_field {
                request.dest = uri.clone().into();
            }
        }
        request.local_addr = Self::get_local_addr(req.headers().get(HOST))?;
        Ok(request)
    }

    fn get_local_addr(host: Option<&HeaderValue>) -> ServerResult<String> {
        debug!("HOST: {:?}", host);
        if host.is_none() {
            Err(RevProxyServerError::NoHostField)
        } else {
            let mut auth = host.unwrap().to_str()?.to_owned();
            // XXX: http on :443?
            if auth.ends_with(":443") || auth.ends_with(":80") {
                auth = auth.replace(":443", "").replace(":80", "");
            }
            Ok(auth)
        }
    }

    fn process_query(query: Option<&str>) -> ServerResult<RevProxyRequest> {
        if query.is_none() {
            Err(RevProxyServerError::NoQuery)
        } else {
            let query = query.unwrap();
            debug!("Found query: {}", query);
            Ok(serde_qs::from_str(query)?)
        }
    }
}

impl<ReqBody> Service<Request<ReqBody>> for RevProxyServer
where
    ReqBody: Body + Send + Sync + 'static,
    <ReqBody as Body>::Error: std::error::Error + Send + Sync,
    Bytes: From<<ReqBody as Body>::Data>,
    <ReqBody as Body>::Data: std::fmt::Debug + Send + Sync,
{
    type Response = Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response<Full<Bytes>>, Infallible>> + Send>>;

    fn call(&self, req: Request<ReqBody>) -> Self::Future {
        let what = self.clone();
        Box::pin(async move {
            let res = what.do_request_wrap_error(req);
            res.await
        })
    }
}

#[derive(Debug)]
pub enum RevProxyServerError {
    // TODO: Remove BoxError
    RevProxyClientError(BoxError),
    NoHostField,
    NoQuery,
    SerdeError(serde_qs::Error),
    // InvalidHeaderName(String),
    InvalidHeaderValue(ToStrError),
}

impl fmt::Display for RevProxyServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RevProxyServerError::NoHostField => write!(f, "No Host field in request"),
            RevProxyServerError::NoQuery => write!(f, "No query"),
            RevProxyServerError::SerdeError(err) => write!(f, "Serde error: {:?}", err),
            RevProxyServerError::RevProxyClientError(err) => {
                write!(f, "RevProxyClient error: {:?}", err)
            }
            // RevProxyServerError::InvalidHeaderName(name) => {
            //     write!(f, "Invalid header name: {}", name)
            // }
            RevProxyServerError::InvalidHeaderValue(value) => {
                write!(f, "Invalid header value: {}", value)
            }
        }
    }
}

impl From<serde_qs::Error> for RevProxyServerError {
    fn from(err: serde_qs::Error) -> Self {
        RevProxyServerError::SerdeError(err)
    }
}

impl From<BoxError> for RevProxyServerError {
    fn from(err: BoxError) -> Self {
        RevProxyServerError::RevProxyClientError(err)
    }
}

impl From<ToStrError> for RevProxyServerError {
    fn from(err: ToStrError) -> Self {
        RevProxyServerError::InvalidHeaderValue(err)
    }
}
