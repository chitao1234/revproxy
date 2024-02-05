use std::convert::Infallible;
use std::net::SocketAddr;

use hyper::service::service_fn;
use tokio::net::TcpListener;
use tracing::{error, info};

use revproxy::revproxy;

#[tokio::main]
async fn main() -> Result<Infallible, anyhow::Error> {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = "[::]:8001".parse()?;

    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        info!("Received connection.");

        tokio::spawn(async move {
            if let Err(err) = hyper::server::conn::Http::new()
                .http1_keep_alive(true)
                .serve_connection(stream, service_fn(revproxy))
                .await
            {
                error!("Error serving: {:#}", anyhow::anyhow!(err));
            }
        });
    }
}
