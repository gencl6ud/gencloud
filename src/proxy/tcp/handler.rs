use log::{info, warn};
use tokio::io::{AsyncRead, AsyncWrite};
use std::io::{Error, ErrorKind, Result};
use tokio::net::{TcpStream, UdpSocket};
use std::sync::{Arc};
use crate::protocol::trojan;
use crate::protocol::common::request::{InboundRequest, TransportProtocol};
use crate::protocol::common::stream::{StandardTcpStream, StandardStream};


pub struct Handler {
}

impl Handler {
    pub fn new() -> Result<Handler> {
        Ok(Handler {})
    }

    pub async fn dispatch<T: AsyncRead + AsyncWrite + Unpin>(
        &self,
        inbound_stream: StandardStream<StandardTcpStream<T>>,
        request: InboundRequest,
    ) -> Result<(u64, u64)> {
        match request.transport_protocol {
            TransportProtocol::TCP =>  return  self.handle_byte_stream(request, inbound_stream).await,
            //TransportProtocol::UDP => self.handle_packet_stream(request, inbound_stream).await,
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "unsupported protocol",
             )),
        }

    }

    async fn handle_byte_stream<T: AsyncRead + AsyncWrite + Unpin>(
        &self,
        request: InboundRequest,
        inbound_stream: StandardStream<StandardTcpStream<T>>,
    ) -> Result<(u64, u64)> {
        let outbound_stream = self.handle_tcp_direct(request).await?;

        let (mut source_read, mut source_write) = tokio::io::split(inbound_stream.into_inner());
        let (mut target_read, mut target_write) = tokio::io::split(outbound_stream.into_inner());

        return match tokio::join!(
            tokio::io::copy(&mut source_read, &mut target_write),
            tokio::io::copy(&mut target_read, &mut source_write),
        ){
            (Ok(u_size), Err(_))=> {
                Ok((u_size, 0))
            },
            (Err(_), Ok(d_size))  => {
                Ok((0, d_size))
            } ,
            (Ok(u_size), Ok(d_size)) => {
                Ok((u_size, d_size))
            }
            (Err(e1), Err(e2)) => {
                Err(Error::new(
                    ErrorKind::BrokenPipe,
                    format!("copy error{}: {}", e1, e2),
                ))
            }
        }
    }

    #[inline]
    async fn handle_tcp_direct(
        &self,
        request: InboundRequest,
    ) -> Result<StandardStream<StandardTcpStream<TcpStream>>> {
        let dest = request.into_destination_address();
        let connection = match TcpStream::connect(dest).await {
            Ok(connection) => {
                info!("Established connection to remote host at {}", dest);
                connection
            }
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::ConnectionReset,
                    format!("Failed to connect to {}: {}", dest, e),
                ));
            }
        };
        Ok(StandardStream::new(StandardTcpStream::Plain(connection)))
    }

    #[allow(dead_code)]
    async fn dial_udp_outbound(&self, request: &InboundRequest) -> Result<UdpSocket> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let dest = request.into_destination_address();
        socket.connect(dest).await?;
        Ok(socket)
    }

    #[allow(dead_code)]
    async fn handle_packet_stream<T: AsyncRead + AsyncWrite + Unpin>(
        &self,
        request: InboundRequest,
        inbound_stream: StandardStream<StandardTcpStream<T>>,
    ) -> Result<(u64, u64)> {
        let socket = match self.dial_udp_outbound(&request).await {
            Ok(s) => Arc::new(s),
            Err(e) => return Err(e),
        };

        let (mut client_reader, mut client_writer) = tokio::io::split(inbound_stream.into_inner());
        let (server_reader, server_writer) = (socket.clone(), socket.clone());

        return match tokio::try_join !(
            trojan::handler::handle_client_data(&mut client_reader, &server_writer),
            trojan::handler::handle_server_data(&mut client_writer, &server_reader, request)
        ) {
            Ok(_) => {
                info!("Connection finished");
                Ok((0,0))
            }
            Err(e) => {
                warn!("Encountered {} error while handing the transport", e);
                Err(Error::new(ErrorKind::ConnectionReset, "Connection reset"))
            }
        };
    }
}
