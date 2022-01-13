use std::{
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Context as AnyhowContext;
use futures::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_tungstenite::{
    accept_async, connect_async, tungstenite, MaybeTlsStream, WebSocketStream,
};

use crate::net::websocket::message::{CloseFrame, Message};

pub struct Stream<NextLayer> {
    inner: WebSocketStream<NextLayer>,
}

impl Stream<MaybeTlsStream<TcpStream>> {
    pub async fn connect<R>(r: R) -> anyhow::Result<Self>
    where
        R: ToString,
    {
        let (s, _) = connect_async(r.to_string())
            .await
            .context("Failed to connect websocket")?;
        Ok(Self { inner: s })
    }
}

impl<NextLayer> Stream<NextLayer>
where
    NextLayer: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn accept(s: NextLayer) -> anyhow::Result<Self> {
        Ok(Self {
            inner: accept_async(s)
                .await
                .context("Failed to accept websocket")?,
        })
    }
}

impl<NextLayer> futures::Stream for Stream<NextLayer>
where
    NextLayer: AsyncRead + AsyncWrite + Unpin,
{
    type Item = anyhow::Result<Message>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.poll_next_unpin(cx) {
            Poll::Ready(v) => Poll::Ready(v.map(|v| {
                v.map_err(|e| anyhow::anyhow!("{}", e))
                    .map(|v| Message::from(v))
            })),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<NextLayer> futures::Sink<Message> for Stream<NextLayer>
where
    NextLayer: AsyncRead + AsyncWrite + Unpin,
{
    type Error = <WebSocketStream<NextLayer> as futures::Sink<tungstenite::Message>>::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready_unpin(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        self.inner.start_send_unpin(item.into())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_flush_unpin(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_close_unpin(cx)
    }
}

impl From<tungstenite::Message> for Message {
    fn from(v: tungstenite::Message) -> Self {
        use tungstenite::Message::*;
        match v {
            Text(v) => Message::Text(v),
            Binary(v) => Message::Binary(v),
            Ping(v) => Message::Ping(v),
            Pong(v) => Message::Pong(v),
            Close(v) => {
                Message::Close(v.map(|tungstenite::protocol::CloseFrame { code, reason }| {
                    CloseFrame {
                        code: code.into(),
                        reason,
                    }
                }))
            }
        }
    }
}

impl Into<tungstenite::Message> for Message {
    fn into(self) -> tungstenite::Message {
        use tungstenite::Message::*;
        match self {
            Message::Text(v) => Text(v),
            Message::Binary(v) => Binary(v),
            Message::Ping(v) => Ping(v),
            Message::Pong(v) => Pong(v),
            Message::Close(v) => {
                Close(v.map(
                    |CloseFrame { code, reason }| tungstenite::protocol::CloseFrame {
                        code: code.into(),
                        reason,
                    },
                ))
            }
        }
    }
}
