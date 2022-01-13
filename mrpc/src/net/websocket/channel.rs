// use futures::{future, SinkExt, StreamExt, TryStreamExt};
// use serde::{Deserialize, Serialize};
// use tokio::net::TcpStream;
// use tokio_tungstenite::MaybeTlsStream;

// use crate::{spawn, sync::mpsc, Message as RpcMessage};

// use super::{message::Message, stream::Stream};

// pub async fn channel<Request, Response, R>(
//     r: R,
// ) -> anyhow::Result<mpsc::Sender<RpcMessage<Request, Response>>>
// where
//     R: ToString,
//     for<'de> Response: Deserialize<'de> + Send + 'static,
//     Request: Serialize + Send + 'static,
// {
    
// }
