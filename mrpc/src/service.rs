use async_trait::async_trait;

use crate::sync::Arc;

pub type SharedService<Request, Response> =
    Arc<dyn Service<Request = Request, Response = Response> + Send + Sync>;

#[async_trait]
pub trait Service {
    type Request;
    type Response;
    async fn serve(self: Arc<Self>, request: Self::Request) -> anyhow::Result<Self::Response>;
}
