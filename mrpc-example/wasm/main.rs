use mrpc::tokio::sync::mpsc;
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[cfg(target_arch = "wasm32")]
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
macro_rules! console_log {
    ($($t:tt)*) => (println!("{}", &format_args!($($t)*).to_string()))
}

#[mrpc::service]
trait Service {
    fn api1(a: i32, b: i32) -> i32;
    async fn api2(a: i32, b: String);
}

#[mrpc::server]
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

async fn run() {
    let (tx, rx) = mpsc::channel(32);

    mrpc::tokio::spawn(async move {
        Arc::new(ServerImpl {}).serve(rx).await.unwrap();
    });

    let cli = ServerClient { sender: tx };

    console_log!("{:?}", cli.service().api1(1, 2).await);
    console_log!("{:?}", cli.service().api2(1, 2.to_string()).await);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    main();
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    rt.block_on(async {
        run().await;
    });
}
