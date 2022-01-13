pub mod client;
mod error;
mod message;
mod server;
mod service;
pub mod transport;

pub use anyhow;
pub use async_trait::async_trait;
pub use futures;
pub use log;
pub use mrpc_derive::*;
pub use serde;
pub use tokio_serde::formats;

pub mod sync {
    pub use futures::channel::{mpsc, oneshot};
    pub use std::sync::Arc;
    pub use tokio::sync::{Mutex, OnceCell};
}

#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_futures::{spawn_local as spawn, spawn_local};

#[cfg(not(target_arch = "wasm32"))]
pub use tokio::{spawn, task::spawn_local};

pub use server::Server;
pub use client::Client;
pub use service::{Service, SharedService};
pub use transport::Sender;
