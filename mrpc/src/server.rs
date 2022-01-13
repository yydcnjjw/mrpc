use async_trait::async_trait;
use futures::StreamExt;

use crate::{
    message::Request,
    spawn,
    sync::{mpsc, Arc},
    Service,
};

#[async_trait]
pub trait Server<ServiceRequest, ServiceResponse> {
    async fn run_loop(
        self: Arc<Self>,
        source: mpsc::Receiver<Request<ServiceRequest, ServiceResponse>>,
    );
}

#[async_trait]
impl<T, ServiceRequest, ServiceResponse> Server<ServiceRequest, ServiceResponse> for T
where
    ServiceRequest: Send + 'static,
    ServiceResponse: Send + 'static,
    T: Service<Request = ServiceRequest, Response = ServiceResponse> + Send + Sync + 'static,
{
    async fn run_loop(
        self: Arc<Self>,
        mut source: mpsc::Receiver<Request<ServiceRequest, ServiceResponse>>,
    ) {
        while let Some(request) = source.next().await {
            let self_ = self.clone();
            spawn(async move {
                let Request {
                    request,
                    response_sender,
                } = request;

                let response = match Self::serve(self_, request).await {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!("Failed to serve: {:?}", e);
                        return;
                    }
                };

                if let Err(_) = response_sender.send(response) {
                    log::warn!("Failed to send response");
                }
            });
        }
    }
}
