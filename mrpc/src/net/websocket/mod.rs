mod message;

#[cfg(all(feature = "websocket_web", target_arch = "wasm32"))]
mod ws_web;

#[cfg(all(feature = "websocket_web", target_arch = "wasm32"))]
pub use ws_web::*;

#[cfg(not(target_arch = "wasm32"))]
mod ws;

#[cfg(not(target_arch = "wasm32"))]
pub use ws::*;



