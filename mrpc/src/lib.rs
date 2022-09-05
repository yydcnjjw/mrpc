#![feature(type_alias_impl_trait)]

use anyhow::Context;
pub use anyhow::{bail, Error as AnyError, Result as AnyResult};

use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt, lock::Mutex,
};

pub use mrpc_macro::*;
pub use serde;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::{oneshot, RwLock},
};

pub use tokio_tungstenite::*;

use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as WsMessage};

use core::task;
use std::{
    collections::HashMap, fmt::Debug, future::Future, marker::PhantomData, pin::Pin, sync::Arc,
    task::Poll,
};

pub trait Service {
    type Request;
    type Response;
}

pub trait Dispatcher<Service, Response> {
    type Result<'a>: Future<Output = AnyResult<Response>>
    where
        Service: 'a;

    fn dispatch<'a>(self, service: &'a Service) -> Self::Result<'_>
    where
        Service: 'a;
}

pub trait Sender {
    type Request;
    type Response;
    type FutureOutput<'a>: Future<Output = AnyResult<Self::Response>>
    where
        Self: 'a;
    fn send(&self, request: Self::Request) -> Self::FutureOutput<'_>;
}

pub struct NativeSender<'a, Serivce, Request, Response> {
    srv: &'a Serivce,
    phantom: PhantomData<(Request, Response)>,
}

impl<'a, Serivce, Request, Response> NativeSender<'a, Serivce, Request, Response> {
    pub fn new(srv: &'a Serivce) -> Self {
        Self {
            srv,
            phantom: PhantomData,
        }
    }
}

impl<'a, Service, Request, Response> Sender for NativeSender<'a, Service, Request, Response>
where
    Request: Dispatcher<Service, Response>,
    Request: Serialize + for<'c> Deserialize<'c>,
{
    type Request = Request;

    type Response = Response;

    type FutureOutput<'b> = impl Future<Output = AnyResult<Self::Response>>
        where Self: 'b;

    fn send(&self, request: Self::Request) -> Self::FutureOutput<'_> {
        async move {
            let req = serde_json::to_string(&request).context("Failed to serialize request")?;
            let req: Self::Request =
                serde_json::from_str(&req).context("Failed to deserialize request")?;
            req.dispatch(&self.srv).await
        }
    }
}

pub struct WsStream<NextLayer, Request, Response> {
    inner: WebSocketStream<NextLayer>,
    phantom: PhantomData<(Request, Response)>,
}

impl<Request, Response> WsStream<MaybeTlsStream<TcpStream>, Request, Response> {
    pub async fn connect<R>(r: R) -> AnyResult<Self>
    where
        R: IntoClientRequest + Unpin,
    {
        Ok(Self {
            inner: connect_async(r).await.context("")?.0,
            phantom: PhantomData,
        })
    }
}

impl<NextLayer, Request, Response> WsStream<NextLayer, Request, Response>
where
    NextLayer: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn accept(s: NextLayer) -> AnyResult<Self> {
        Ok(Self {
            inner: accept_async(s).await.context("")?,
            phantom: PhantomData,
        })
    }
}

impl<NextLayer, Request, Response> futures::Stream for WsStream<NextLayer, Request, Response>
where
    NextLayer: AsyncRead + AsyncWrite + Unpin,
    Request: Unpin,
    Response: for<'c> Deserialize<'c> + Unpin,
{
    type Item = AnyResult<Response>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.poll_next_unpin(cx) {
            Poll::Ready(v) => Poll::Ready({
                // TODO:
                Some(Ok(match v.unwrap().unwrap() {
                    WsMessage::Binary(v) => serde_json::from_slice(&v).unwrap(),
                    _ => unreachable!(),
                }))
            }),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<NextLayer, Request, Response> futures::Sink<Request> for WsStream<NextLayer, Request, Response>
where
    NextLayer: AsyncRead + AsyncWrite + Unpin,
    Request: Serialize + Unpin,
    Response: Unpin,
{
    type Error = AnyError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready_unpin(cx)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Request) -> Result<(), Self::Error> {
        self.inner
            .start_send_unpin(WsMessage::Binary(serde_json::to_vec(&item).context("")?))
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner
            .poll_flush_unpin(cx)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.inner
            .poll_close_unpin(cx)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message<Payload> {
    pub id: usize,
    pub payload: Payload,
}

impl<Payload> Message<Payload> {
    fn new_with_id(id: usize, payload: Payload) -> Self {
        Self { id, payload }
    }

    fn new(payload: Payload) -> Self {
        Self {
            id: {
                static mut ID: usize = 0;
                unsafe {
                    ID += 1;
                    ID
                }
            },
            payload,
        }
    }
}

#[derive(Debug)]
pub struct Inner<Request, Response, RpcStream> {
    sink: SplitSink<RpcStream, Message<Request>>,
    waitq: HashMap<usize, oneshot::Sender<Response>>,
}

#[derive(Debug)]
pub struct TransportSender<Request, Response, RpcStream> {
    inner: Arc<RwLock<Inner<Request, Response, RpcStream>>>,
}

impl<Request, Response, RpcStream> Clone for TransportSender<Request, Response, RpcStream> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Request, Response, RpcStream> TransportSender<Request, Response, RpcStream>
where
    RpcStream: futures::Sink<Message<Request>>
        + futures::Stream<Item = AnyResult<Message<Response>>>
        + Send
        + 'static,
    Request: Debug + Send + Sync + 'static,
    Response: Debug + Send + 'static,
{
    pub fn new(stream: RpcStream) -> AnyResult<Self> {
        let (sink, stream) = stream.split();
        let inner = Arc::new(RwLock::new(Inner {
            sink,
            waitq: HashMap::new(),
        }));

        let self_ = Self {
            inner: inner.clone(),
        };

        Self::run_loop(self_.clone(), stream);

        Ok(self_)
    }

    fn run_loop(self, mut stream: SplitStream<RpcStream>) {
        tokio::spawn(async move {
            let inner = self.inner.clone();
            while let Some(message) = stream.next().await {
                let Message { id, payload } = message.unwrap();

                let inner = inner.write();
                inner
                    .await
                    .waitq
                    .remove(&id)
                    .unwrap()
                    .send(payload)
                    .unwrap();
            }
        });
    }
}

impl<Request, Response, RpcStream> Sender for TransportSender<Request, Response, RpcStream>
where
    RpcStream: futures::Sink<Message<Request>>,
    <RpcStream as futures::Sink<Message<Request>>>::Error: Debug,
{
    type Request = Request;

    type Response = Response;

    type FutureOutput<'a> = impl Future<Output = AnyResult<Self::Response>>
    where Self: 'a;

    fn send(&self, request: Self::Request) -> Self::FutureOutput<'_> {
        async move {
            let message = Message::new(request);
            let id = message.id;

            // TODO:
            self.inner.write().await.sink.send(message).await.unwrap();

            let (tx, rx) = oneshot::channel();

            self.inner.write().await.waitq.insert(id, tx);

            rx.await.context("")
        }
    }
}

pub struct TransportReceiver<Request, Response> {
    phantom: PhantomData<(Request, Response)>,
}

impl<Request, Response> TransportReceiver<Request, Response>
where
    Request: Debug + Send + 'static,
    Response: Debug + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }

    pub fn run_loop<RpcStream, Service>(&self, stream: RpcStream, service: Arc<Service>)
    where
        RpcStream: futures::Sink<Message<Response>>
            + futures::Stream<Item = AnyResult<Message<Request>>>
            + Send
            + 'static,
        <RpcStream as futures::Sink<Message<Response>>>::Error: Debug,
        Request: Dispatcher<Service, Response>,
        for<'a> <Request as Dispatcher<Service, Response>>::Result<'a>: Send,
        Service: Send + Sync + 'static,
    {
        tokio::spawn(async move {
            let (sink, mut stream) = stream.split();
            let sink = Arc::new(Mutex::new(sink));

            while let Some(message) = stream.next().await {
                let sink = sink.clone();
                let service = service.clone();
                tokio::spawn(async move {
                    let Message { id, payload } = message.unwrap();
                    let response = payload.dispatch(&service).await.unwrap();
                    sink.lock().await.send(Message::new_with_id(id, response)).await.unwrap();
                });
            }
        });
    }
}

pub struct RouterSender<'a, Sender, RouterRequest, RouterResponse, ItemRequest, ItemResponse> {
    inner: &'a Sender,
    phantom: PhantomData<(RouterRequest, RouterResponse, ItemRequest, ItemResponse)>,
}

impl<'a, Sender, RouterRequest, RouterResponse, ItemRequest, ItemResponse>
    RouterSender<'a, Sender, RouterRequest, RouterResponse, ItemRequest, ItemResponse>
{
    pub fn new(inner: &'a Sender) -> Self {
        Self {
            inner,
            phantom: PhantomData,
        }
    }
}

impl<'a, Sender_, RouterRequest, RouterResponse, ItemRequest, ItemResponse> Sender
    for RouterSender<'a, Sender_, RouterRequest, RouterResponse, ItemRequest, ItemResponse>
where
    Sender_: Sender<Request = RouterRequest, Response = RouterResponse>,
    RouterRequest: From<ItemRequest>,
    ItemResponse: TryFrom<RouterResponse, Error = AnyError>,
{
    type Request = ItemRequest;

    type Response = ItemResponse;

    type FutureOutput<'b> = impl Future<Output = AnyResult<Self::Response>>
        where Self: 'b;

    fn send(&self, request: Self::Request) -> Self::FutureOutput<'_> {
        async move {
            ItemResponse::try_from(self.inner.send(RouterRequest::from(request)).await?).context("")
        }
    }
}
