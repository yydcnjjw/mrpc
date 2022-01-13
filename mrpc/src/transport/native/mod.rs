use anyhow::Context;
use async_trait::async_trait;
use futures::SinkExt;

use crate::{
    message,
    sync::{mpsc, oneshot},
    Sender,
};

pub fn channel<Request, Response>(
    buffer: usize,
) -> (
    mpsc::Sender<message::Request<Request, Response>>,
    mpsc::Receiver<message::Request<Request, Response>>,
) {
    mpsc::channel(buffer)
}

#[async_trait]
impl<Request, Response> Sender<Request, Response>
    for mpsc::Sender<message::Request<Request, Response>>
where
    Request: Send,
    Response: Send,
{
    async fn send_request(&self, request: Request) -> anyhow::Result<Response> {
        let (tx, rx) = oneshot::channel();
        self.clone()
            .send(message::Request {
                request,
                response_sender: tx,
            })
            .await
            .context("Failed to send request")?;

        rx.await.context("Failed to recv response")
    }
}
