use std::net::{Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::{convert::Infallible, net::IpAddr};

use anyhow::anyhow;
use clap::Parser;
use hyper::service::service_fn;
use tokio::net::TcpListener;
use tracing::{debug, error, info};

use revproxy::util;
use revproxy::RevProxy;

/// Run reverse proxy from normal proxy
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Proxy to use, leave it blank for using proxy from environment
    #[arg(short = 's', long)]
    proxy: Option<String>,

    /// The IP address to bind
    #[arg(short, long, default_value_t = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)))]
    address: IpAddr,

    /// The port to listen on
    #[arg(short, long, default_value_t = 8001, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
}

// TODO: Proper use of span in tracing
#[tokio::main]
async fn main() -> Result<Infallible, anyhow::Error> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();
    debug!("args: {:?}", args);

    println!(
        "Listening for connections on {}, port {}, with proxy {}",
        args.address, args.port, args.proxy.as_deref().unwrap_or("from environment")
    );

    let mut revbuilder = RevProxy::builder();
    if let Some(proxy) = args.proxy {
        revbuilder = revbuilder.proxy(proxy);
    }
    let revclient = revbuilder.build().map_err(|e| anyhow!(e))?;
    let revclient = Arc::new(revclient);

    let listener = TcpListener::bind(SocketAddr::new(args.address, args.port)).await?;

    // let revclient = revclient.clone();
    let service = move |req| {
        let revclient = revclient.clone();

        async move {
            let resp = revclient.revproxy(req).await;
            Ok::<_, Infallible>(util::rust_error_to_page(resp))
        }
    };
    let service = service_fn(service);

    loop {
        let (stream, _) = listener.accept().await?;
        info!("Received connection.");


        let service = service.clone();
        tokio::spawn(async move {
            if let Err(err) = hyper::server::conn::Http::new()
                .http1_keep_alive(true)
                .serve_connection(
                    stream,
                    service,
                )
                .await
            {
                error!("Error serving: {:#}", anyhow::anyhow!(err));
            }
        });
    }
}
