use std::net::{Ipv6Addr, SocketAddr};
use std::{convert::Infallible, net::IpAddr};

use clap::{Args, Parser};
use hyper::service::service_fn;
use tokio::net::TcpListener;
use tracing::{debug, error, info};

use revproxy::revproxy;
use revproxy::util::rust_error_to_page;

/// Run reverse proxy from normal proxy
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    proxy: Option<Proxy>,

    /// The IP address to bind
    #[arg(short, long, default_value_t = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)))]
    address: IpAddr,

    /// The port to listen on
    #[arg(short, long, default_value_t = 8001, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
    
}

#[derive(Debug, Args)]
struct Proxy {
    /// Proxy host, must be specified together with proxy port
    #[arg(short = 'd', long, requires = "proxy_port")]
    proxy_host: Option<IpAddr>,
    /// Proxy port, must be specified together with proxy host
    #[arg(short = 's', long, requires = "proxy_host", value_parser = clap::value_parser!(u16).range(1..))]
    proxy_port: Option<u16>,
}

// TODO: Proper use of span in tracing
#[tokio::main]
async fn main() -> Result<Infallible, anyhow::Error> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();
    debug!("args: {:?}", args);

    println!(
        "Listening for connections on {}, port {}, with proxy {:?}",
        args.address, args.port, args.proxy
    );
    let listener = TcpListener::bind(SocketAddr::new(args.address, args.port)).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        info!("Received connection.");

        tokio::spawn(async move {
            if let Err(err) = hyper::server::conn::Http::new()
                .http1_keep_alive(true)
                .serve_connection(stream, service_fn(move |conn| {
                    rust_error_to_page(revproxy(conn))
                }))
                .await
            {
                error!("Error serving: {:#}", anyhow::anyhow!(err));
            }
        });
    }
}
