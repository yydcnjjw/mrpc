use futures::{SinkExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::{mpsc, oneshot},
};
use tokio_serde::{formats::Json, Framed};
use tokio_util::codec::LengthDelimitedCodec;

use crate::Message;

pub async fn writer<Request, Response, Addr>(
    addr: Addr,
) -> anyhow::Result<mpsc::Sender<Message<Request, Response>>>
where
    Addr: ToSocketAddrs,
    for<'de> Response: Deserialize<'de> + Send + Unpin + 'static,
    Request: Serialize + Send + Unpin + 'static,
{
    let (tx, rx) = mpsc::channel(32);

    let s = TcpStream::connect(addr).await?;
    let s = tokio_util::codec::Framed::new(s, LengthDelimitedCodec::new());
    let s = Framed::new(s, Json::<Response, Request>::default());

    tokio::spawn(run_loop(s, rx));

    Ok(tx)
}

async fn run_loop<Request, Response>(
    mut s: Framed<
        tokio_util::codec::Framed<TcpStream, LengthDelimitedCodec>,
        Response,
        Request,
        Json<Response, Request>,
    >,
    mut rx: mpsc::Receiver<Message<Request, Response>>,
) where
    for<'de> Response: Deserialize<'de> + Send + Unpin + 'static,
    Request: Serialize + Send + Unpin + 'static,
{
    while let Some(msg) = rx.recv().await {
        let Message::<Request, Response> { req, resp } = msg;

        if let Err(e) = s.send(req).await {
            log::warn!("{:?}", e);
            continue;
        }

        if let Ok(response) = s.try_next().await {
            if let Some(response) = response {
                if let Err(_) = resp.send(response) {
                    log::warn!("Failed to send response");
                }
            }
        }
    }
}

pub async fn reader<Addr, Request, Response>(
    addr: Addr,
    tx: mpsc::Sender<Message<Request, Response>>,
) -> anyhow::Result<()>
where
    Addr: ToSocketAddrs,
    for<'de> Request: Deserialize<'de> + Unpin + Send + 'static,
    Response: Serialize + Unpin + Send + 'static,
{
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (s, _) = listener.accept().await?;

        let s = tokio_util::codec::Framed::new(s, LengthDelimitedCodec::new());

        let mut s = Framed::new(s, Json::<Request, Response>::default());

        let msgtx = tx.clone();
        tokio::spawn(async move {
            while let Some(req) = s.try_next().await.unwrap() {
                let (tx, rx) = oneshot::channel();

                if let Err(e) = msgtx
                    .send(Message::<Request, Response> { req, resp: tx })
                    .await
                {
                    log::warn!("{}", e);
                }

                match rx.await {
                    Ok(resp) => {
                        if let Err(e) = s.send(resp).await {
                            log::warn!("{:?}", e);
                        }
                    }
                    Err(e) => {
                        log::warn!("{:?}", e);
                    }
                }
            }
        });
    }
}
