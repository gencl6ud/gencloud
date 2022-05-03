use crate::config::base::InboundConfig;
use crate::proxy::tcp::acceptor::Acceptor;
use crate::proxy::tcp::handler::Handler;
use crate::xflash::traffic::TrafficItem;
use crate::xflash::user::UserCenter;
use log::{info, warn};
use std::io::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;

const DEFAULT_TASK_TIMEOUT: u64 = 600;

pub struct TcpServer {
    local_addr: String,
    local_port: u16,
    acceptor: Arc<Acceptor>,
    handler: Arc<Handler>,
}

impl TcpServer {
    pub fn new(inbound_config: InboundConfig) -> Result<TcpServer> {
        let handler = Arc::from(Handler::new()?);
        let acceptor = Arc::from(Acceptor::new(&inbound_config));

        return Ok(TcpServer {
            local_addr: inbound_config.address,
            local_port: inbound_config.port,
            handler,
            acceptor,
        });
    }

    pub async fn start(
        self,
        user_center: &'static UserCenter,
        traffic_sender: Sender<TrafficItem>,
    ) -> Result<()> {
        let (local_addr, local_port) = (self.local_addr, self.local_port);

        let listener = TcpListener::bind((local_addr.as_ref(), local_port)).await?;

        info!(
            "TCP server started on {}:{}, ready to accept input stream",
            local_addr, local_port
        );

        loop {
            let (socket, addr) = match listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    warn!("listenter accept error: {}", e );
                    continue
                },
            };
            
            info!("Received new connection from {}", addr);

            let acceptor = Arc::clone(&self.acceptor);
            let handler = Arc::clone(&self.handler);
            let tx = traffic_sender.clone();

            tokio::spawn(async move {
                match acceptor.accept(socket, user_center).await {
                    Ok((request, inbound_stream, id)) => {
                        let res = tokio::time::timeout(
                            std::time::Duration::from_secs(DEFAULT_TASK_TIMEOUT),
                            async {
                                match handler.dispatch(inbound_stream, request).await {
                                    Ok((u, d)) => {
                                        info!("Connection from {} has finished", addr);
                                        let traffic_item = TrafficItem::new(id, u * 8, d * 8);

                                        if tx.send(traffic_item).await.is_err() {
                                            warn!("traffic item send error");
                                        }
                                        drop(tx);
                                    }
                                    Err(e) => {
                                        warn!("Failed to handle the inbound stream: {}", e);
                                        drop(tx);
                                    }
                                }
                            },
                        );
                        match res.await {
                            Err(_) => warn!("task timeout"),
                            Ok(_) => info!("task success"),
                        }
                    }
                    Err(e) => {
                        warn!("Failed to accept inbound connection from {}: {}", addr, e);
                    }
                };
            });
        }
    }
}
