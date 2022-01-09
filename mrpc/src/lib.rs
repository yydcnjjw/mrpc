pub mod net;

pub use mrpc_derive::*;

pub use anyhow;
pub use async_trait::async_trait;
pub use log;
pub use serde;

pub mod sync {
    pub use tokio::sync::{mpsc, oneshot, Mutex};
}

#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_futures::{spawn_local as spawn, spawn_local};

#[cfg(not(target_arch = "wasm32"))]
pub use tokio::{spawn, task::spawn_local};

use sync::*;

pub struct Message<Request, Response> {
    pub req: Request,
    pub resp: oneshot::Sender<Response>,
}

#[async_trait]
pub trait Poster<Request, Response> {
    async fn post(&self, req: Request, resp: oneshot::Sender<Response>) -> anyhow::Result<()>;
}
