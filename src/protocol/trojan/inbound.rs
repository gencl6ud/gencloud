use std::io::{Error, ErrorKind, Result};
use std::pin::Pin;
use std::task::{Context, Poll};

use log::info;
use tokio::io::{AsyncRead, AsyncWrite, BufStream, ReadBuf};

use crate::xflash::user::UserCenter;
use crate::protocol::common::request::InboundRequest;
use crate::protocol::common::stream::InboundStream;
use crate::protocol::trojan::parser::parse;

pub struct TrojanInboundStream<IO> {
    stream: BufStream<IO>,
}

impl<IO> InboundStream for TrojanInboundStream<IO> where
    IO: AsyncRead + AsyncWrite + Unpin + Send + Sync
{

}

impl<IO> AsyncRead for TrojanInboundStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        return Pin::new(&mut self.stream).poll_read(cx, buf);
    }
}

impl<IO> AsyncWrite for TrojanInboundStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        return Pin::new(&mut self.stream).poll_write(cx, buf);
    }

    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        return Pin::new(&mut self.stream).poll_flush(cx);
    }

    #[inline]
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        return Pin::new(&mut self.stream).poll_shutdown(cx);
    }
}

impl<IO> TrojanInboundStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    pub async fn new(
        stream: IO,
        user_center: &'static UserCenter
    ) -> Result<(InboundRequest, Box<dyn InboundStream>, i32)> {
        let mut imbound_stream = TrojanInboundStream {
            stream: BufStream::with_capacity(256, 256, stream),
        };

        let request = parse(&mut imbound_stream).await?;

        info!("Received request: trojan {}", request.to_string());

        let (validate_result, id) = user_center.validate(request.get_hex()).await;
        if !validate_result {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Received invalid hex value",
            ));
        }
        Ok((request.inbound_request(), Box::new(imbound_stream), id))
    }
}
