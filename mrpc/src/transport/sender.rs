use async_trait::async_trait;

#[async_trait]
pub trait Sender<Request, Response> {
    async fn send_request(&self, request: Request) -> anyhow::Result<Response>;
}

// #[async_trait]
// impl<T, Request, Response> Sender<Request, Response> for T
// where
//     Request: Send + 'static,
//     T: Sink<Request> + Stream<Item = Response> + Send + Sync + Unpin,
// {
//     async fn send_request(&mut self, request: Request) -> anyhow::Result<Response> {
//         if let Err(_) = self.send(request).await {
//             anyhow::bail!("Failed to send request");
//         }

//         self.next()
//             .await
//             .ok_or(anyhow::anyhow!("Failed to recv response"))
//     }
// }
