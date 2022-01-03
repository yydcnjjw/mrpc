pub use mrpc_derive::*;

pub use anyhow;
pub use async_trait::async_trait;
pub use serde;
// use serde::{Deserialize, Serialize};
pub use tokio;
use tokio::sync;

pub struct Message<Request, Response> {
    pub req: Request,
    pub resp: sync::oneshot::Sender<Response>,
}

#[async_trait]
pub trait Poster<Request, Response> {
    async fn post(&self, req: Request, resp: sync::oneshot::Sender<Response>)
        -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::{SinkExt, TryStreamExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio_serde::formats::SymmetricalJson;
    use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

    use super::*;

    #[tokio::test]
    async fn test_demo() {
        let server = tokio::spawn(async move {
            let listener = TcpListener::bind("127.0.0.1:17653").await.unwrap();

            loop {
                let (socket, _) = listener.accept().await.unwrap();

                // Delimit frames using a length header
                let length_delimited = FramedRead::new(socket, LengthDelimitedCodec::new());

                // Deserialize frames
                let mut deserialized = tokio_serde::SymmetricallyFramed::new(
                    length_delimited,
                    SymmetricalJson::<Request<String>>::default(),
                );

                // Spawn a task that prints all received messages to STDOUT
                tokio::spawn(async move {
                    while let Some(msg) = deserialized.try_next().await.unwrap() {
                        println!("GOT: {:?}", msg);
                    }
                });
            }
        });

        tokio::time::sleep(Duration::from_secs(1)).await;

        // Bind a server socket
        let socket = TcpStream::connect("127.0.0.1:17653").await.unwrap();

        // Delimit frames using a length header
        let length_delimited = FramedWrite::new(socket, LengthDelimitedCodec::new());

        // Serialize frames with JSON
        let mut serialized = tokio_serde::SymmetricallyFramed::new(
            length_delimited,
            SymmetricalJson::<Request<String>>::default(),
        );

        // Send the value
        serialized
            .send(Request::<String> {
                id: 0,
                message: "test".into(),
            })
            .await
            .unwrap();

        server.await.unwrap();
    }
}
