#![feature(type_alias_impl_trait)]

use std::{future::Future, sync::Arc, time::Duration};

use mrpc::Dispatcher;
use tokio::net::TcpListener;

#[mrpc::service]
pub trait ServiceA {
    fn api1(a: i32, b: i32) -> i32;
    fn api2(a: i32, b: String);
}

struct ServiceAImpl {}

#[mrpc::service_impl]
impl ServiceA for ServiceAImpl {
    async fn api1(&self, a: i32, b: i32) -> mrpc::AnyResult<i32> {
        println!("{}, {}", a, b);

        Ok(10)
    }
    fn api2(&self, a: i32, b: String) -> mrpc::AnyResult<()> {
        println!("{}, {}", a, b);

        Ok(())
    }
}

#[mrpc::service]
pub trait ServiceB {
    fn api1(a: i32, b: i32) -> i32;
    fn api2(a: i32, b: String);
}

struct ServiceBImpl {}

#[mrpc::service_impl]
impl ServiceB for ServiceBImpl {
    async fn api1(&self, a: i32, b: i32) -> mrpc::AnyResult<i32> {
        println!("{}, {}", a, b);

        Ok(10)
    }
    fn api2(&self, a: i32, b: String) -> mrpc::AnyResult<()> {
        println!("{}, {}", a, b);

        Ok(())
    }
}

#[mrpc::router]
enum Service {
    ServiceA(ServiceA),
    ServiceB(ServiceB),
}

struct ServiceImpl {
    a: ServiceAImpl,
    b: ServiceBImpl,
}

impl ServiceRouter for ServiceImpl {
    type ServiceA = ServiceAImpl;

    type ServiceB = ServiceBImpl;

    fn service_a(&self) -> &Self::ServiceA {
        &self.a
    }

    fn service_b(&self) -> &Self::ServiceB {
        &self.b
    }
}

async fn test() {
    env_logger::init();

    tokio::spawn(async move {
        let service = Arc::new(ServiceImpl {
            a: ServiceAImpl {},
            b: ServiceBImpl {},
        });

        let try_socket = TcpListener::bind("127.0.0.1:8080").await;
        let listener = try_socket.expect("Failed to bind");

        while let Ok((stream, _)) = listener.accept().await {
            let service = service.clone();
            tokio::spawn(async move {
                let s = mrpc::WsStream::accept(stream).await.unwrap();

                mrpc::TransportReceiver::<ServiceRequest, ServiceResponse>::new()
                    .run_loop(s, service);
            });
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let s = mrpc::WsStream::connect("ws://127.0.0.1:8080")
        .await
        .unwrap();

    let cli = mrpc::TransportSender::new(s).unwrap();

    println!("{:?}", cli.service_a().api1(1, 2).await);

    println!("{:?}", cli.service_b().api2(1, "abc".into()).await);
}

#[tokio::main]
async fn main() {
    test().await;
}
