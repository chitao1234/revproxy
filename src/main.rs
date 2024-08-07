use std::net::{Ipv6Addr, SocketAddr};
use std::{convert::Infallible, net::IpAddr};

use clap::Parser;
use revproxy::server::RevProxyServer;
use tokio::net::TcpListener;
use tracing::{debug, error, info};

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

    let socket_addr = SocketAddr::new(args.address, args.port);

    println!(
        "Listening for connections on http://{}, with proxy {}",
        socket_addr,
        args.proxy.as_deref().unwrap_or("from environment")
    );

    let mut reqwest_client = reqwest::Client::builder();
    if let Some(proxy) = args.proxy {
        reqwest_client = reqwest_client.proxy(reqwest::Proxy::all(proxy)?);
    }
    let reqwest_client = reqwest_client.build()?;

    let revserver = RevProxyServer::new(reqwest_client.into(), false, true);

    let listener = TcpListener::bind(socket_addr).await?;

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("Received connection from {}", addr);
        let revserver = revserver.clone();

        tokio::spawn(async move {
            if let Err(err) = hyper::server::conn::http1::Builder::new()
                .keep_alive(true)
                .serve_connection(hyper_util::rt::TokioIo::new(stream), revserver)
                .await
            {
                error!("Error serving: {:#}", anyhow::anyhow!(err));
            }
        });
    }
}
