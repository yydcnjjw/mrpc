use futures::{future, SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

use crate::{
    net::websocket::{message::Message, stream::Stream},
    sync::{mpsc, oneshot},
    Message as RpcMessage,
};

pub async fn accept<Addr, Request, Response>(
    addr: Addr,
    tx: mpsc::Sender<RpcMessage<Request, Response>>,
) -> anyhow::Result<()>
where
    Addr: ToSocketAddrs,
    for<'de> Request: Deserialize<'de> + Send + 'static,
    Response: Serialize + Send + 'static,
{
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (s, _) = listener.accept().await?;

        let tx = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = on_accept(s, tx).await {
                log::warn!("{:?}", e);
            }
        });
    }
}

async fn on_accept<Request, Response>(
    s: TcpStream,
    rpctx: mpsc::Sender<RpcMessage<Request, Response>>,
) -> anyhow::Result<()>
where
    for<'de> Request: Deserialize<'de> + Send + 'static,
    Response: Serialize + Send + 'static,
{
    let s = Stream::accept(s).await?;

    let (mut ws_sink, ws_source) = s.split();

    let mut ws_source = ws_source
        .try_filter(|msg| future::ready(matches!(msg, Message::Binary(_) | Message::Text(_))));

    while let Some(rpc_request) = ws_source.next().await {
        let rpc_request = match rpc_request {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Fail to recv from websocket: {:?}", e);
                continue;
            }
        };

        let (tx, rx) = oneshot::channel();

        let rpc_request = match serde_json::from_slice::<Request>(&rpc_request.into_data()) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("{:?}", e);
                continue;
            }
        };

        if let Err(e) = rpctx
            .send(RpcMessage {
                request: rpc_request,
                resp: tx,
            })
            .await
        {
            log::warn!("Failed to send request: {}", e);
            continue;
        }

        let rpc_response = match rx.await {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Failed to wait response: {:?}", e);
                continue;
            }
        };

        let rpc_response = match serde_json::to_vec(&rpc_response) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("{:?}", e);
                continue;
            }
        };

        if let Err(e) = ws_sink.send(Message::Binary(rpc_response)).await {
            anyhow::bail!("Failed to send from websocket: {:?}", e);
        }
    }

    anyhow::bail!("websocket stream exited")
}
