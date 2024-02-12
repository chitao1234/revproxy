use std::net::{Ipv6Addr, SocketAddr};
use std::{convert::Infallible, net::IpAddr};

use clap::Parser;
use hyper::service::service_fn;
use tokio::net::TcpListener;
use tracing::{debug, error, info};

use revproxy::revproxy;

/// Run reverse proxy from normal proxy
#[derive(Parser, Debug)]
struct Cli {
    /// The IP address to bind
    #[arg(short, long, default_value_t = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)))]
    address: IpAddr,
    /// The port to listen on
    #[arg(short, long, default_value_t = 8001, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
    /// Proxy host
    #[arg(short = 'd', long, group = "proxy")]
    proxy_host: Option<IpAddr>,
    /// Proxy port
    #[arg(short = 's', long, group = "proxy", value_parser = clap::value_parser!(u16).range(1..))]
    proxy_port: Option<u16>,
}

// TODO: Proper use of span in tracing
#[tokio::main]
async fn main() -> Result<Infallible, anyhow::Error> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();
    debug!("args: {:?}", args);

    println!(
        "Listening for connections on {}, port {}.",
        args.address, args.port
    );
    let listener = TcpListener::bind(SocketAddr::new(args.address, args.port)).await?;

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
