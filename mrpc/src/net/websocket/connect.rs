use futures::{future, SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;

use crate::{spawn, sync::mpsc, Message as RpcMessage};

use super::{message::Message, stream::Stream};

pub async fn connect<Request, Response, R>(
    r: R,
) -> anyhow::Result<mpsc::Sender<RpcMessage<Request, Response>>>
where
    R: ToString,
    for<'de> Response: Deserialize<'de> + Send + 'static,
    Request: Serialize + Send + 'static,
{
    let (tx, rpc_request_source) = mpsc::channel(32);

    let s = Stream::connect(r.to_string()).await?;

    spawn(run_loop(s, rpc_request_source));

    Ok(tx)
}

async fn run_loop<Request, Response>(
    s: Stream<MaybeTlsStream<TcpStream>>,
    mut rpc_request_source: mpsc::Receiver<RpcMessage<Request, Response>>,
) where
    for<'de> Response: Deserialize<'de> + Send + 'static,
    Request: Serialize + Send + 'static,
{
    let (mut ws_sink, ws_source) = s.split();

    let mut ws_source = ws_source
        .try_filter(|msg| future::ready(matches!(msg, Message::Binary(_) | Message::Text(_))));

    while let Some(rpc_request) = rpc_request_source.recv().await {
        let RpcMessage::<Request, Response> { request: req, resp } = rpc_request;

        let data = match serde_json::to_vec(&req) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("{:?}", e);
                continue;
            }
        };

        if let Err(e) = ws_sink.send(Message::Binary(data)).await {
            log::warn!("Fail to send from websocket: {:?}", e);
            continue;
        }

        let ws_message = match ws_source.next().await {
            Some(v) => match v {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("{:?}", e);
                    continue;
                }
            },
            None => {
                log::warn!("Failed to recv from webscoket");
                continue;
            }
        };

        let rpc_response = match serde_json::from_slice::<Response>(&ws_message.into_data()) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("{:?}", e);
                continue;
            }
        };

        if let Err(_) = resp.send(rpc_response) {
            log::warn!("Failed to send response");
        }
    }
}
