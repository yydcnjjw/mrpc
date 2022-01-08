use std::sync::Arc;

use mod2::*;
use mrpc::tokio::{self, sync::mpsc};

#[mrpc::service()]
pub trait Service1 {
    fn api1(a: i32, b: i32) -> i32;
    async fn api2(a: i32, b: String);
}

struct Service1Impl {}

#[mrpc::async_trait]
impl Service1 for Service1Impl {
    fn api1(self: Arc<Self>, a: i32, b: i32) -> i32 {
        println!("{}, {}", a, b);

        10
    }

    async fn api2(self: Arc<Self>, a: i32, b: String) {
        println!("{}, {}", a, b);
    }
}

mod mod2 {
    use std::sync::Arc;

    #[mrpc::service()]
    pub trait Service2 {
        fn api1(a: i32, b: i32);
        async fn api2(a: i32, b: String);
    }

    pub struct Service2Impl {}

    #[mrpc::async_trait]
    impl Service2 for Service2Impl {
        fn api1(self: Arc<Self>, a: i32, b: i32) {
            println!("{}, {}", a, b);
        }

        async fn api2(self: Arc<Self>, a: i32, b: String) {
            println!("{}, {}", a, b);
        }
    }
}

#[mrpc::server()]
pub enum Server {
    Service1(Service1),
    Service2(Service2),
}

struct ServerImpl {}

#[mrpc::async_trait]
impl Server for ServerImpl {
    async fn create_service_1(self: Arc<Self>) -> mrpc::anyhow::Result<Arc<dyn Service1>> {
        Ok(Arc::new(Service1Impl {}))
    }

    async fn create_service_2(self: Arc<Self>) -> mrpc::anyhow::Result<Arc<dyn mod2::Service2>> {
        Ok(Arc::new(mod2::Service2Impl {}))
    }
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel(32);

    tokio::spawn(async move {
        Arc::new(ServerImpl {}).serve(rx).await.unwrap();
    });

    let cli = ServerClient { sender: tx };

    println!("{:?}", cli.service_1().api1(1, 2).await);
    println!("{:?}", cli.service_1().api2(1, 2.to_string()).await);
    println!("{:?}", cli.service_2().api1(1, 2).await);
    println!("{:?}", cli.service_2().api2(1, 2.to_string()).await);
}
