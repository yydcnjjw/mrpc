use std::{marker::PhantomData, pin::Pin, task};

use anyhow::Context;
use bytes::BytesMut;
use futures::{self, Sink, StreamExt};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_serde::{Deserializer, Serializer};
use tokio_tungstenite::MaybeTlsStream;

use self::message::Message;

mod message;
pub mod native;

pub struct Stream<NextLayer, Item, SinkItem, Codec> {
    s: NextLayer,
    codec: Codec,
    item: PhantomData<(Item, SinkItem)>,
}

impl<NextLayer, Item, SinkItem, Codec, E> Sink<SinkItem>
    for Stream<NextLayer, Item, SinkItem, Codec>
where
    E: std::error::Error + Send + Sync + 'static,
    NextLayer: Sink<Message, Error = E> + Unpin,
    Codec: Serializer<SinkItem> + Unpin,
    Codec::Error: std::error::Error + Send + Sync + 'static,
    Self: Unpin,
{
    type Error = anyhow::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        if let task::Poll::Ready(v) = Pin::new(&mut self.s).poll_ready(cx) {
            task::Poll::Ready(v.map_err(|e| anyhow::anyhow!(e)))
        } else {
            task::Poll::Pending
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: SinkItem) -> Result<(), Self::Error> {
        let data = Pin::new(&mut self.codec)
            .serialize(&item)
            .context("Failed to serialize message")?
            .to_vec();
        Pin::new(&mut self.s)
            .start_send(Message::Binary(data))
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        if let task::Poll::Ready(v) = Pin::new(&mut self.s).poll_flush(cx) {
            task::Poll::Ready(v.map_err(|e| anyhow::anyhow!(e)))
        } else {
            task::Poll::Pending
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        if let task::Poll::Ready(v) = Pin::new(&mut self.s).poll_close(cx) {
            task::Poll::Ready(v.map_err(|e| anyhow::anyhow!(e)))
        } else {
            task::Poll::Pending
        }
    }
}

impl<NextLayer, Item, SinkItem, Codec> futures::Stream for Stream<NextLayer, Item, SinkItem, Codec>
where
    NextLayer: futures::Stream<Item = anyhow::Result<Message>> + Unpin,
    Codec: Deserializer<Item> + Unpin,
    Codec::Error: std::error::Error + Send + Sync + 'static,
    Self: Unpin,
{
    type Item = anyhow::Result<Item>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let message = match self.s.poll_next_unpin(cx) {
            task::Poll::Ready(v) => match v {
                Some(v) => match v {
                    Ok(v) => v,
                    Err(e) => {
                        return task::Poll::Ready(Some(Err(e)));
                    }
                },
                None => {
                    return task::Poll::Ready(None);
                }
            },
            task::Poll::Pending => {
                return task::Poll::Pending;
            }
        };

        let message = match message {
            Message::Binary(v) => v,
            Message::Text(v) => v.into_bytes(),
            Message::Ping(_) | Message::Pong(_) => {
                return task::Poll::Pending;
            }
            Message::Close(_) => {
                return task::Poll::Ready(Some(Err(anyhow::anyhow!(
                    "Failed to recv response: websocket is closed"
                ))));
            }
        };

        task::Poll::Ready(Some(
            Pin::new(&mut self.codec)
                .deserialize(&BytesMut::from(&message[..]))
                .context("Failed to deserialize message"),
        ))
    }
}

pub async fn connect<R, Item, SinkItem, Codec>(
    r: R,
    codec: Codec,
) -> anyhow::Result<Stream<native::Transport<MaybeTlsStream<TcpStream>>, Item, SinkItem, Codec>>
where
    R: ToString,
{
    Ok(Stream {
        s: native::Transport::connect(r).await?,
        codec,
        item: Default::default(),
    })
}

pub async fn accept<NextLayer, Item, SinkItem, Codec>(
    s: NextLayer,
    codec: Codec,
) -> anyhow::Result<Stream<native::Transport<NextLayer>, Item, SinkItem, Codec>>
where
    NextLayer: AsyncRead + AsyncWrite + Unpin,
{
    Ok(Stream {
        s: native::Transport::accept(s).await?,
        codec,
        item: Default::default(),
    })
}
