use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, ToSocketAddrs},
};
use tokio_serde::Framed;
use tokio_util::codec::{self, LengthDelimitedCodec};

pub async fn connect<Addr, Item, SinkItem, Codec>(
    addr: Addr,
    codec: Codec,
) -> anyhow::Result<Framed<codec::Framed<TcpStream, LengthDelimitedCodec>, Item, SinkItem, Codec>>
where
    Addr: ToSocketAddrs,
{
    let s = TcpStream::connect(addr).await?;
    let s = codec::Framed::new(s, LengthDelimitedCodec::new());
    let s = Framed::new(s, codec);

    Ok(s)
}

pub async fn accept<NextLayer, Item, SinkItem, Codec>(
    next_layer: NextLayer,
    codec: Codec,
) -> Framed<codec::Framed<NextLayer, LengthDelimitedCodec>, Item, SinkItem, Codec>
where
    NextLayer: AsyncRead + AsyncWrite,
{
    let s = codec::Framed::new(next_layer, LengthDelimitedCodec::new());

    Framed::new(s, codec)
}

