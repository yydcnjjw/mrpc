use mrpc::Server;

use std::sync::Arc;

#[mrpc::service(message(serde))]
pub trait SubService {
    fn api1(a: i32, b: i32) -> i32;
    async fn api2(a: i32, b: String);
}

struct SubServiceImpl {}

#[mrpc::service]
impl SubService for SubServiceImpl {
    fn api1(self: Arc<Self>, a: i32, b: i32) -> i32 {
        println!("{}, {}", a, b);

        10
    }

    async fn api2(self: Arc<Self>, a: i32, b: String) {
        println!("{}, {}", a, b);
    }
}

#[mrpc::service(message(serde))]
enum MainService {
    SubService(SubService),
}

struct MainServiceImpl {}

#[mrpc::service]
impl MainService for MainServiceImpl {
    async fn create_sub_service(
        self: Arc<Self>,
    ) -> mrpc::anyhow::Result<mrpc::SharedService<SubServiceRequest, SubServiceResponse>> {
        Ok(Arc::new(SubServiceImpl {}))
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let (tx, rx) = mrpc::transport::native::channel(32);

    // // #[cfg(feature = "tcp")]
    // tokio::spawn(transport::accept_with_tcp(
    //     "127.0.0.1:8080",
    //     tx.clone(),
    //     Json::default(),
    // ));

    // #[cfg(feature = "websocket")]
    // tokio::spawn(transport::websocket::accept("127.0.0.1:8081", tx.clone()));

    // tokio::time::sleep(Duration::from_secs(2)).await;

    mrpc::spawn(Arc::new(MainServiceImpl {}).run_loop(rx));

    {
        let cli = mrpc::Client::<MainServiceApi>::new(Arc::new(tx));

        println!("{:?}", cli.sub_service().api1(1, 2).await);
        println!("{:?}", cli.sub_service().api2(1, 2u8.to_string()).await);
    }

    // #[cfg(feature = "tcp")]
    // {
    //     let cli =
    //         mrpc::Client::<MainServiceApi>::connect_with_tcp("127.0.0.1:8080", Json::default())
    //             .await
    //             .unwrap();

    //     println!("{:?}", cli.sub_service().api1(1, 2).await);
    //     println!("{:?}", cli.sub_service().api2(1, 2u8.to_string()).await);
    // }

    // #[cfg(feature = "websocket")]
    // {
    //     let cli = mrpc::Client::<MainServiceApi>::connect_with_ws("ws://127.0.0.1:8081")
    //         .await
    //         .unwrap();

    //     println!("{:?}", cli.sub_service().api1(1, 2).await);
    //     println!("{:?}", cli.sub_service().api2(1, 2.to_string()).await);
    // }
}
