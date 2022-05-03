use std::io::{Error, ErrorKind, Result};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsAcceptor;

use crate::protocol::trojan;
use crate::config::base::InboundConfig;
use crate::config::tls::make_server_config;
use crate::protocol::common::request::InboundRequest;
use crate::protocol::common::stream::{StandardStream, StandardTcpStream};
use crate::proxy::base::SupportedProtocols;
use crate::xflash::user::UserCenter;

pub struct Acceptor {
    tls_acceptor: Option<TlsAcceptor>,
    protocol: SupportedProtocols,
}

impl Acceptor {

    pub fn new(inbound: &InboundConfig) -> Acceptor {
        let tls_acceptor = match &inbound.tls {
            Some(tls) => match make_server_config(&tls) {
                Some(cfg) => Some(TlsAcceptor::from(cfg)),
                None => None,
            },
            None => None,
        };

        Acceptor {
            tls_acceptor,
            protocol: inbound.protocol,
        }
    }

    pub async fn accept<T: AsyncRead + AsyncWrite + Unpin>(
        &self,
        inbound_stream: T,
        user_center: &'static UserCenter
    ) -> Result<(InboundRequest, StandardStream<StandardTcpStream<T>>, i32)>
    {
        match self.protocol {
            SupportedProtocols::TROJAN if self.tls_acceptor.is_some() => {
                let tls_stream = self
                    .tls_acceptor
                    .as_ref()
                    .unwrap()
                    .accept(inbound_stream)
                    .await?;
                Ok(trojan::accept(StandardTcpStream::RustlsServer(tls_stream), user_center).await?)
            }
            SupportedProtocols::TROJAN => {
                Ok(trojan::accept(StandardTcpStream::Plain(inbound_stream), user_center).await?)
            }
            _ => Err(Error::new(
                ErrorKind::ConnectionReset,
                "Failed to accept inbound stream, unsupported protocol",
            )),
        }
    }
}
