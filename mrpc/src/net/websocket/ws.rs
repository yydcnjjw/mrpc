use futures::{future, SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::{mpsc, oneshot},
};
use tokio_tungstenite::{
    connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

use crate::Message;

use super::message::WsRpcMessage;

pub async fn writer<Request, Response, R>(
    r: R,
) -> anyhow::Result<mpsc::Sender<Message<Request, Response>>>
where
    R: ToString,
    for<'de> Response: Deserialize<'de> + Send + Unpin + 'static,
    Request: Serialize + Send + Unpin + 'static,
{
    let (tx, rx) = mpsc::channel(32);

    let (s, _) = connect_async(r.to_string()).await?;

    tokio::spawn(run_loop(s, rx));

    Ok(tx)
}

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn run_loop<Request, Response>(
    s: WsStream,
    mut rpc_rx: mpsc::Receiver<Message<Request, Response>>,
) where
    for<'de> Response: Deserialize<'de> + Send + Unpin + 'static,
    Request: Serialize + Send + Unpin + 'static,
{
    let (mut w, r) = s.split();

    let mut id_generator: i64 = 0;
    let mut ws_rx = r.try_filter(|msg| future::ready(msg.is_binary() || msg.is_text()));
    while let Some(msg) = rpc_rx.recv().await {
        let Message::<Request, Response> { req, resp } = msg;

        let data = match serde_json::to_vec(&WsRpcMessage {
            id: id_generator,
            value: req,
        }) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("{:?}", e);
                continue;
            }
        };

        if let Err(e) = w.send(WsMessage::Binary(data)).await {
            log::warn!("Fail to send from websocket: {:?}", e);
            continue;
        }

        let response = match ws_rx.next().await {
            Some(v) => v,
            None => {
                log::warn!("Failed to recv from webscoket");
                continue;
            }
        };

        let response = match response {
            Ok(v) => v,
            Err(e) => {
                log::warn!("{:?}", e);
                continue;
            }
        };

        let WsRpcMessage { id: _, value } =
            match serde_json::from_slice::<WsRpcMessage<Response>>(&response.into_data()) {
                Ok(data) => data,
                Err(e) => {
                    log::warn!("{:?}", e);
                    continue;
                }
            };

        if let Err(_) = resp.send(value) {
            log::warn!("Failed to send response");
        }

        id_generator += 1;
    }
}

async fn on_accept<Request, Response>(
    s: TcpStream,
    rpctx: mpsc::Sender<Message<Request, Response>>,
) -> anyhow::Result<()>
where
    for<'de> Request: Deserialize<'de> + Send + 'static,
    Response: Serialize + Send + 'static,
{
    let ws = tokio_tungstenite::accept_async(s).await?;

    let (mut w, r) = ws.split();

    let mut ws_rx = r.try_filter(|msg| future::ready(msg.is_binary() || msg.is_text()));

    while let Some(message) = ws_rx.next().await {
        let message = match message {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Fail to recv from websocket: {:?}", e);
                continue;
            }
        };

        let (tx, rx) = oneshot::channel();

        let WsRpcMessage { id, value } =
            match serde_json::from_slice::<WsRpcMessage<Request>>(&message.into_data()) {
                Ok(data) => data,
                Err(e) => {
                    log::warn!("{:?}", e);
                    continue;
                }
            };

        if let Err(e) = rpctx
            .send(Message {
                req: value,
                resp: tx,
            })
            .await
        {
            log::warn!("Failed to send request: {}", e);
            continue;
        }

        let response = match rx.await {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Failed to wait response: {:?}", e);
                continue;
            }
        };

        let data = match serde_json::to_vec(&WsRpcMessage {
            id,
            value: response,
        }) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("{:?}", e);
                continue;
            }
        };

        if let Err(e) = w.send(WsMessage::Binary(data)).await {
            anyhow::bail!("Failed to send from websocket: {:?}", e);
        }
    }

    anyhow::bail!("websocket stream exited")
}

pub async fn reader<Addr, Request, Response>(
    addr: Addr,
    tx: mpsc::Sender<Message<Request, Response>>,
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
