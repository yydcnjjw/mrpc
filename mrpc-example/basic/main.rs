use mrpc::tokio::sync::mpsc;
use std::sync::Arc;

#[mrpc::service(message(serde))]
trait Service {
    fn api1(a: i32, b: i32) -> i32;
    async fn api2(a: i32, b: String);
}

#[mrpc::server(message(serde))]
enum Server {
    Service(Service),
}

struct ServiceImpl {}

#[mrpc::async_trait]
impl Service for ServiceImpl {
    fn api1(self: Arc<Self>, a: i32, b: i32) -> i32 {
        println!("{}, {}", a, b);

        10
    }

    async fn api2(self: Arc<Self>, a: i32, b: String) {
        println!("{}, {}", a, b);
    }
}

struct ServerImpl {}

#[mrpc::async_trait]
impl Server for ServerImpl {
    async fn create_service(self: Arc<Self>) -> mrpc::anyhow::Result<Arc<dyn Service>> {
        Ok(Arc::new(ServiceImpl {}))
    }
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel(32);

    #[cfg(feature = "tcp")]
    tokio::spawn(mrpc::net::tcp::reader("127.0.0.1:8081", tx.clone()));

    #[cfg(feature = "websocket")]
    tokio::spawn(mrpc::net::websocket::reader("127.0.0.1:8080", tx.clone()));

    tokio::spawn(async move {
        Arc::new(ServerImpl {}).serve(rx).await.unwrap();
    });

    {
        let cli = ServerClient { sender: tx };

        println!("{:?}", cli.service().api1(1, 2).await);
        println!("{:?}", cli.service().api2(1, 2.to_string()).await);
    }

    #[cfg(feature = "tcp")]
    {
        let tx = mrpc::net::tcp::writer("127.0.0.1:8081").await.unwrap();
        let cli = ServerClient { sender: tx };

        println!("{:?}", cli.service().api1(1, 2).await);
        println!("{:?}", cli.service().api2(1, 2.to_string()).await);
    }

    #[cfg(feature = "websocket")]
    {
        let tx = mrpc::net::websocket::writer("ws://127.0.0.1:8080")
            .await
            .unwrap();
        let cli = ServerClient { sender: tx };

        println!("{:?}", cli.service().api1(1, 2).await);
        println!("{:?}", cli.service().api2(1, 2.to_string()).await);
    }
}
