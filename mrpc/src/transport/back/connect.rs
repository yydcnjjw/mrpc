use async_trait::async_trait;
use futures::{SinkExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;

use crate::{
    sync::{Arc, Mutex},
    transport,
};

use super::{message::Message, stream::Stream};

pub struct Sender {
    s: Arc<Mutex<Stream<MaybeTlsStream<TcpStream>>>>,
}

impl Sender {
    async fn new<R>(r: R) -> anyhow::Result<Arc<Self>>
    where
        R: ToString,
    {
        Ok(Arc::new(Self {
            s: Arc::new(Mutex::new(Stream::connect(r).await?)),
        }))
    }
}

#[async_trait]
impl<ServiceRequest, ServiceResponse> transport::Sender<ServiceRequest, ServiceResponse> for Sender
where
    for<'de> ServiceResponse: Deserialize<'de> + Send + Unpin + 'static,
    ServiceRequest: Serialize + Send + Sync + Unpin + 'static,
{
    async fn send_request(&self, request: ServiceRequest) -> anyhow::Result<ServiceResponse> {
        let mut s = self.s.lock().await;

        if let Err(e) = s.send(Message::Binary(serde_json::to_vec(&request)?)).await {
            anyhow::bail!("Failed to send tcp request: {:?}", e);
        }

        while let Ok(message) = s.try_next().await {
            let message = match message {
                Some(v) => v,
                None => {
                    continue;
                }
            };

            let message = match message {
                Message::Binary(v) => v,
                Message::Text(v) => v.into_bytes(),
                Message::Ping(_) | Message::Pong(_) => {
                    continue;
                }
                Message::Close(v) => {
                    anyhow::bail!("websocket is closed: {:?}", v);
                }
            };

            return Ok(serde_json::from_slice(&message)?);
        }

        unreachable!()
    }
}

pub async fn connect<ServiceRequest, ServiceResponse, R>(r: R) -> anyhow::Result<Arc<Sender>>
where
    R: ToString,
{
    Sender::new(r).await
}
