use std::{pin::Pin, task};

use anyhow::Context;
use futures::{self, Sink, SinkExt};
use serde::Serialize;

use super::message::Message;

mod native;

// pub use native::Stream;

struct Stream<NextLayer> {
    s: NextLayer,
}

impl<NextLayer, E, Item> Sink<Item> for Stream<NextLayer>
where
    E: std::error::Error + Send + Sync + 'static,
    NextLayer: Sink<Message, Error = E> + Unpin,
    Item: Serialize,
{
    type Error = anyhow::Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        if let task::Poll::Ready(v) = self.s.poll_ready_unpin(cx) {
            task::Poll::Ready(v.map_err(|e| anyhow::anyhow!(e)))
        } else {
            task::Poll::Pending
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        self.s
            .start_send_unpin(Message::Binary(serde_json::to_vec(&item)?))
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        todo!()
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        todo!()
    }
}

// impl<NextLayer, T> futures::Stream for Stream<NextLayer>
// where T: {
//     type Item = T;

//     fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Option<Self::Item>> {
//         todo!()
//     }
// }
