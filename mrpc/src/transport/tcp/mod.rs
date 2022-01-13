use std::marker::PhantomData;

use async_trait::async_trait;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, ToSocketAddrs},
};
use tokio_serde::Framed as SerdeFramed;
use tokio_util::codec::{self, LengthDelimitedCodec};

use super::accept::Acceptable;

pub type Framed<NextLayer, Item, SinkItem, Codec> =
    SerdeFramed<codec::Framed<NextLayer, LengthDelimitedCodec>, Item, SinkItem, Codec>;

pub async fn connect<Addr, Item, SinkItem, Codec>(
    addr: Addr,
    codec: Codec,
) -> anyhow::Result<Framed<TcpStream, Item, SinkItem, Codec>>
where
    Addr: ToSocketAddrs,
{
    let s = TcpStream::connect(addr).await?;
    let s = codec::Framed::new(s, LengthDelimitedCodec::new());
    let s = Framed::<TcpStream, Item, SinkItem, Codec>::new(s, codec);

    Ok(s)
}

pub struct Acceptor<Item, SinkItem> {
    item: PhantomData<fn(Item, SinkItem)>,
}

#[async_trait]
impl<NextLayer, Item, SinkItem, Sink, Source, Codec> Acceptable<NextLayer, Sink, Source, Codec>
    for Acceptor<Item, SinkItem>
where
    NextLayer: AsyncRead + AsyncWrite + Send,
    Codec: Send + 'static,
    Item: Send + 'static,
    SinkItem: Send + 'static,
    Sink: futures::Sink<SinkItem>,
    Source: futures::Stream,
{
    async fn accept(next_layer: NextLayer, codec: Codec) -> anyhow::Result<(Sink, Source)> {
        let s = codec::Framed::new(next_layer, LengthDelimitedCodec::new());
        let s = Framed::<NextLayer, Item, SinkItem, Codec>::new(s, codec);
        let (sink, source) = s.split();
        let sink = sink.sink_map_err(|e| anyhow::anyhow!(e));
        let source = source.map_err(|e| anyhow::anyhow!(e));
        Ok((sink, source))
    }
}
