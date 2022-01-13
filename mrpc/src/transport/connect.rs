use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Context;
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use tokio::sync::{Mutex, RwLock};

use crate::{
    spawn,
    sync::{oneshot, Arc},
    transport::{self, TranportMesssage},
};

pub struct Connector<Sink, Source, Request, Response> {
    sink: Arc<Mutex<Sink>>,
    buf: Arc<RwLock<HashMap<usize, Message<Response>>>>,
    item: PhantomData<fn(Request, Source)>,
}

impl<Sink, Source, Request, Response> Connector<Sink, Source, Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    pub async fn new(sink: Sink, source: Source) -> Arc<Self>
    where
        Sink: 'static,
        Source: futures::Stream + Send + Unpin + 'static,
        Source::Item: anyhow::Context<TranportMesssage<Response>, anyhow::Error> + Send + 'static,
    {
        let self_ = Arc::new(Self {
            sink: Arc::new(Mutex::new(sink)),
            buf: Arc::new(RwLock::new(HashMap::new())),
            item: Default::default(),
        });

        spawn(Self::recv_loop(source, self_.buf.clone()));

        self_
    }

    async fn recv_loop(
        mut source: Source,
        buf: Arc<RwLock<HashMap<usize, Message<Response>>>>,
    ) -> anyhow::Result<()>
    where
        Source: futures::Stream + Send + Unpin + 'static,
        Source::Item: anyhow::Context<TranportMesssage<Response>, anyhow::Error> + Send + 'static,
    {
        while let Some(response) = source.next().await {
            let response = response.context("Failed to recv response")?;

            let TranportMesssage { id, message } = response;

            let Message { tx } = buf
                .write()
                .await
                .remove(&id)
                .context(format!("message not found: {}", id))?;

            if let Err(_) = tx.send(message) {
                anyhow::bail!("Failed to send response: {}", id)
            }
        }
        unreachable!()
    }
}

#[async_trait]
impl<Sink, Source, Request, Response> transport::Sender<Request, Response>
    for Connector<Sink, Source, Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
    Sink: futures::Sink<TranportMesssage<Request>, Error = anyhow::Error> + Send + Unpin + 'static,
{
    async fn send_request(&self, request: Request) -> anyhow::Result<Response> {
        let id = gen_id();

        let (tx, rx) = oneshot::channel();

        self.buf.write().await.insert(id, Message { tx });

        self.sink
            .lock()
            .await
            .send(TranportMesssage {
                id,
                message: request,
            })
            .await?;

        rx.await.context("Failed to waiting response")
    }
}

struct Message<Response> {
    tx: oneshot::Sender<Response>,
}

fn gen_id() -> usize {
    static ID: AtomicUsize = AtomicUsize::new(0);
    return ID.fetch_add(1, Ordering::AcqRel);
}
