use anyhow::Context;
use futures::{future, Future, Sink, StreamExt};
use serde::Deserialize;

use self::{message::Message};

// mod accept;
// mod connect;
mod message;
mod stream;

// pub use accept::accept;
// pub use connect::connect;

// async fn connect<R, ServiceRequest, ServiceResponse>(r: R) -> anyhow::Result<()>
// where
//     R: ToString,
//     for<'de> ServiceRequest: Deserialize<'de>,
// {
//     Stream::connect(r)
//         .await?
//         .filter_map::<_, anyhow::Result<ServiceRequest>, _>(|message| {
//             let message = match message {
//                 Ok(v) => v,
//                 Err(e) => {
//                     return future::ready(Some(Err(e)));
//                 }
//             };

//             let message = match message {
//                 Message::Binary(v) => v,
//                 Message::Text(v) => v.into_bytes(),
//                 Message::Ping(_) | Message::Pong(_) => {
//                     return future::ready(None);
//                 }
//                 Message::Close(v) => {
//                     return future::ready(Some(Err(anyhow::anyhow!(
//                         "websocket is closed: {:?}",
//                         v
//                     ))));
//                 }
//             };

//             return future::ready(Some(
//                 serde_json::from_slice::<ServiceRequest>(&message)
//                     .context("Failed to deserialize message"),
//             ));
//         });
//     Ok(())
// }
