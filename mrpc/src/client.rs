use std::{ops::Deref, sync::Arc};

use async_trait::async_trait;
use futures::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::ToSocketAddrs;
use tokio_serde::{Deserializer, Serializer};

use crate::transport::{self, Connector, TranportMesssage};

pub type Sender<Request, Response> = Arc<dyn transport::Sender<Request, Response> + Send + Sync>;

#[async_trait]
pub trait Api {
    type Request;
    type Response;

    fn create(sender: Sender<Self::Request, Self::Response>) -> Self;

    fn sender(&self) -> Sender<Self::Request, Self::Response>;

    async fn request(&self, request: Self::Request) -> anyhow::Result<Self::Response>
    where
        Self::Request: Send,
    {
        self.sender().send_request(request).await
    }
}

pub struct Client<ClientApi> {
    api: ClientApi,
}

impl<ClientApi> Deref for Client<ClientApi> {
    type Target = ClientApi;

    fn deref(&self) -> &Self::Target {
        &self.api
    }
}

impl<ClientApi> Client<ClientApi>
where
    ClientApi: Api,
    for<'de> ClientApi::Response: Deserialize<'de> + Send + Unpin + 'static,
    ClientApi::Request: Serialize + Send + Unpin + 'static,
{
    pub fn new(sender: Sender<ClientApi::Request, ClientApi::Response>) -> Self {
        Self {
            api: ClientApi::create(sender),
        }
    }

    pub async fn connect_with_tcp<Addr, Codec>(addr: Addr, codec: Codec) -> anyhow::Result<Self>
    where
        Addr: ToSocketAddrs,
        Codec: Serializer<TranportMesssage<ClientApi::Request>>
            + Deserializer<TranportMesssage<ClientApi::Response>>
            + Unpin
            + Send
            + 'static,
        <Codec as Serializer<TranportMesssage<ClientApi::Request>>>::Error: Into<std::io::Error>,
        <Codec as Deserializer<TranportMesssage<ClientApi::Response>>>::Error: Into<std::io::Error>,
        std::io::Error:
            From<
                <Codec as tokio_serde::Deserializer<
                    TranportMesssage<<ClientApi as Api>::Response>,
                >>::Error,
            >,
    {
        let s = transport::tcp::connect::<
            _,
            TranportMesssage<ClientApi::Response>,
            TranportMesssage<ClientApi::Request>,
            _,
        >(addr, codec)
        .await?;

        let (sink, source) = s.split();

        let sink = sink.sink_map_err(|e| anyhow::anyhow!(e));

        let source = source.map_err(|e| anyhow::anyhow!(e));

        Ok(Self {
            api: ClientApi::create(Connector::new(sink, source).await),
        })
    }

    // pub async fn connect_with_ws<R, Codec>(r: R, codec: Codec) -> anyhow::Result<Self>
    // where
    //     R: ToString,
    //     Codec: Serializer<TranportMesssage<ClientApi::Request>>
    //         + Deserializer<TranportMesssage<ClientApi::Response>>
    //         + Unpin
    //         + Send
    //         + 'static,
    //     <Codec as Serializer<TranportMesssage<ClientApi::Request>>>::Error:
    //         std::error::Error + Send + Sync + 'static,
    //     <Codec as Deserializer<TranportMesssage<ClientApi::Response>>>::Error:
    //         std::error::Error + Send + Sync + 'static,
    // {
    //     let s = transport::websocket::connect::<_, _, _, _>(r, codec).await?;
    //     Ok(Self {
    //         api: ClientApi::create(Connector::new(s).await),
    //     })
    // }

    // pub fn channel(sender: Sender<ClientApi::Request, ClientApi::Response>) -> Self {
    //     Self {
    //         api: ClientApi::create(sender),
    //     }
    // }
}
