pub mod base;
pub mod outbound;
pub mod inbound;
pub mod parser;
pub mod handler;

use log::info;
use self::parser::parse;
use std::io::{Error, ErrorKind, Result};
use tokio::io::{AsyncRead, AsyncWrite};
use crate::protocol::common::{request::InboundRequest, stream::StandardTcpStream};
use crate::protocol::common::stream::StandardStream;
use crate::xflash::user::UserCenter;


/// Helper function to accept an abstract TCP stream to Trojan connection
pub async fn accept<T: AsyncRead + AsyncWrite + Unpin>(
    mut stream: StandardTcpStream<T>,
    user_center: &'static UserCenter,
) -> Result<(InboundRequest, StandardStream<StandardTcpStream<T>>, i32)> {
    // Read trojan request header and generate request header
    let request = parse(&mut stream).await?;

    // Validate the request secret and decide if the connection should be accepted

    info!("Received request: trojan {}", request.to_string());

    let (validate_result, id) = user_center.validate(request.get_hex()).await;
    if !validate_result {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Received invalid hex value",
        ));
    }

    Ok((request.inbound_request(), StandardStream::new(stream), id))
}
