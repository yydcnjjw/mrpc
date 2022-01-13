use anyhow::Context;
use async_trait::async_trait;
use futures::{SinkExt, Stream, StreamExt};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio_serde::{Deserializer, Serializer};

use crate::{message, sync::mpsc};

use super::{tcp, TranportMesssage};

#[async_trait]
pub trait Acceptable<NextLayer, Sink, Source, Codec> {
    async fn accept(s: NextLayer, codec: Codec) -> anyhow::Result<(Sink, Source)>;
}

async fn accept<Addr, Request, Response, Sink, Source, Codec, Accept>(
    addr: Addr,
    sender: mpsc::Sender<message::Request<Request, Response>>,
    codec: Codec,
) -> anyhow::Result<()>
where
    Addr: ToSocketAddrs,
    Request: Send + 'static,
    Response: Send + 'static,
    Codec: Default + Send,
    Accept: Acceptable<TcpStream, Sink, Source, Codec>,
    Sink: futures::Sink<TranportMesssage<Response>> + Unpin + Send,
    Sink::Error: std::error::Error + Send + Sync + 'static,
    Source: Stream<Item = anyhow::Result<TranportMesssage<Request>>> + Unpin + Send,
{
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (s, _) = listener.accept().await?;

        let mut sender = sender.clone();
        tokio::spawn(async move {
            let (mut sink, mut source): (Sink, Source) = Acceptable::accept(s, Codec::default())
                .await
                .context("Failed to accept tcp")?;

            while let Some(request) = source.next().await {
                let TranportMesssage { id, message } =
                    request.context("Failed to recv request from tcp")?;

                let (request, response) = message::Request::new(message);
                sender
                    .send(request)
                    .await
                    .context("Failed to send request to native")?;

                sink.send(TranportMesssage {
                    id,
                    message: response
                        .await
                        .context("Failed to recv response from native")?,
                })
                .await
                .context("Failed to send response to tcp")?;
            }

            Ok::<(), anyhow::Error>(())
        });
    }
}

pub async fn accept_with_tcp<Addr, Request, Response, Sink, Source, Codec>(
    addr: Addr,
    sender: mpsc::Sender<message::Request<Request, Response>>,
    codec: Codec,
) -> anyhow::Result<()>
where
    Addr: ToSocketAddrs,
    Request: Send + 'static,
    Response: Send + 'static,
    Codec: Serializer<TranportMesssage<Response>>
        + Deserializer<TranportMesssage<Request>>
        + Unpin
        + Send
        + Default
        + 'static,
    <Codec as Serializer<TranportMesssage<Response>>>::Error: Into<std::io::Error>,
    <Codec as Deserializer<TranportMesssage<Request>>>::Error: Into<std::io::Error>,
    std::io::Error: From<<Codec as Deserializer<TranportMesssage<Request>>>::Error>,
    Sink: futures::Sink<TranportMesssage<Response>> + Unpin + Send,
    Sink::Error: std::error::Error + Send + Sync + 'static,
    Source: Stream<Item = anyhow::Result<TranportMesssage<Request>>> + Unpin + Send,
{
    accept::<
        Addr,
        Request,
        Response,
        Sink,
        Source,
        Codec,
        tcp::Acceptor<TranportMesssage<Request>, TranportMesssage<Response>>,
    >(addr, sender, codec)
    .await
}
