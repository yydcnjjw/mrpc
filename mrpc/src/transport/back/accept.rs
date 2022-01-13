use futures::{future, SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, ToSocketAddrs};

use crate::{sync::Arc, transport};

use super::{message::Message, stream::Stream};

pub async fn accept<Addr, ServiceRequest, ServiceResponse>(
    addr: Addr,
    sender: Arc<dyn transport::Sender<ServiceRequest, ServiceResponse>>,
) -> anyhow::Result<()>
where
    Addr: ToSocketAddrs,
    for<'de> ServiceRequest: Deserialize<'de> + Unpin + Send + 'static,
    ServiceResponse: Serialize + Unpin + Send + 'static,
{
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (s, _) = listener.accept().await?;

        let sender = sender.clone();
        tokio::spawn(async move {
            let s = match Stream::accept(s).await {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("Failed to accept websocket: {:?}", e);
                    return;
                }
            };

            let (mut ws_sink, ws_source) = s.split();

            let mut ws_source = ws_source.try_filter(|message| {
                future::ready(!matches!(message, Message::Ping(_) | Message::Pong(_)))
            });

            while let Ok(request) = ws_source.try_next().await {
                if request.is_none() {
                    continue;
                }

                let response =
                    match websocket_handle_request(request.unwrap(), sender.clone()).await {
                        Ok(v) => v,
                        Err(e) => {
                            log::warn!("Failed to handle webscoket request: {:?}", e);
                            continue;
                        }
                    };
                if let Err(e) = ws_sink.send(response).await {
                    log::warn!("Failed to send webscoket response: {:?}", e);
                }
            }
        });
    }
}

async fn websocket_handle_request<ServiceRequest, ServiceResponse>(
    request: Message,
    sender: Arc<dyn transport::Sender<ServiceRequest, ServiceResponse>>,
) -> anyhow::Result<Message>
where
    for<'de> ServiceRequest: Deserialize<'de> + Unpin + Send + 'static,
    ServiceResponse: Serialize + Unpin + Send + 'static,
{
    if matches!(request, Message::Close(_)) {
        anyhow::bail!("websocket is closed!");
    }

    let request = serde_json::from_slice(&request.into_data())?;

    let response = sender.send_request(request).await?;

    Ok(Message::Binary(serde_json::to_vec(&response)?))
}
