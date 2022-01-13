use futures::{Stream, StreamExt};
use send_wrapper::SendWrapper;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::{cell::RefCell, rc::Rc};


use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, Event, MessageEvent, WebSocket};

use crate::{Message, sync::{mpsc, oneshot, Mutex}, spawn_local};

use super::message::WsRpcMessage;

#[derive(Debug)]
pub enum WsEvent {
    Open,
    Message(Vec<u8>),
    Error(SendWrapper<ErrorEvent>),
}

struct State {
    evq: Vec<WsEvent>,
    waker: Option<Waker>,
}

impl State {
    fn new() -> Self {
        Self {
            evq: Vec::new(),
            waker: None,
        }
    }
}

pub struct WsStream {
    state: SendWrapper<Rc<RefCell<State>>>,
}

impl WsStream {
    pub fn new(ws: WebSocket) -> Self {
        let self_ = Self {
            state: SendWrapper::new(Rc::new(RefCell::new(State::new()))),
        };

        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let on_open = Self::on_open(&self_);
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        let on_error = Self::on_error(&self_);
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        let on_message = Self::on_message(&self_);
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        self_
    }

    fn on_open(&self) -> Closure<dyn FnMut(Event)> {
        let state = self.state.clone();
        Closure::wrap(Box::new(move |_| {
            let mut s = state.borrow_mut();
            s.evq.push(WsEvent::Open);
            if let Some(w) = s.waker.take() {
                w.wake();
            }
        }) as Box<dyn FnMut(Event)>)
    }

    fn on_error(&self) -> Closure<dyn FnMut(ErrorEvent)> {
        let state = self.state.clone();
        Closure::wrap(Box::new(move |e: ErrorEvent| {
            let mut s = state.borrow_mut();
            s.evq.push(WsEvent::Error(SendWrapper::new(e)));
            if let Some(w) = s.waker.take() {
                w.wake();
            }
        }) as Box<dyn FnMut(ErrorEvent)>)
    }

    fn on_message(&self) -> Closure<dyn FnMut(MessageEvent)> {
        let state = self.state.clone();
        Closure::wrap(Box::new(move |e: MessageEvent| {
            let message;
            if let Ok(bin) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                message = js_sys::Uint8Array::new(&bin).to_vec();
            } else if let Ok(s) = e.data().dyn_into::<js_sys::JsString>() {
                message = js_sys::Uint8Array::new(&s).to_vec();
            } else {
                log::warn!("Unsupported message data format: {:?}", e.data());
                return;
            }

            let mut s = state.borrow_mut();
            s.evq.push(WsEvent::Message(message));
            if let Some(w) = s.waker.take() {
                w.wake();
            }
        }) as Box<dyn FnMut(MessageEvent)>)
    }
}

impl Stream for WsStream {
    type Item = WsEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut s = self.state.borrow_mut();

        if s.evq.is_empty() {
            s.waker = Some(cx.waker().clone());
            return Poll::Pending;
        } else {
            let e = s.evq.pop();
            return Poll::Ready(e);
        }
    }
}

// pub async fn connect<Request, Response, Addr>(
//     addr: Addr,
// ) -> anyhow::Result<mpsc::Sender<Message<Request, Response>>>
// where
//     Addr: ToString,
//     for<'de> Response: Deserialize<'de> + Send + 'static,
//     Request: Serialize + Send + 'static,
// {
//     let (tx, rpc_request_source) = mpsc::channel(32);

//     let ws = match WebSocket::new(&addr.to_string()) {
//         Ok(ws) => SendWrapper::new(ws),
//         Err(e) => {
//             anyhow::bail!("{:?}", e);
//         }
//     };

//     let mut wss = WsStream::new((*ws).clone());

//     if let Some(ev) = wss.next().await {
//         match ev {
//             WsEvent::Open => {  }
//             WsEvent::Message(_) => {
//                 anyhow::bail!(
//                     "Failed to connect websocket: message event should not have happened"
//                 );
//             }
//             WsEvent::Error(e) => {
//                 anyhow::bail!("Failed to connect websocket: {:?}", e);
//             }
//         };
//     } else {
//         anyhow::bail!("Failed to recv websocket event");
//     }

//     let id_map = Arc::new(Mutex::new(HashMap::new()));
//     spawn_local(accept_ws_event_loop(wss, id_map.clone()));
//     spawn_local(accept_rpc_request_loop(
//         rpc_request_source,
//         ws,
//         id_map.clone(),
//     ));
//     Ok(tx)
// }

// async fn accept_ws_event_loop<Response>(
//     mut s: WsStream,
//     id_map: Arc<Mutex<HashMap<i64, oneshot::Sender<Response>>>>,
// ) where
//     for<'de> Response: Deserialize<'de> + Send + 'static,
// {
//     while let Some(ev) = s.next().await {
//         match ev {
//             WsEvent::Open => {
//                 log::warn!("open event should not have happened");
//             }
//             WsEvent::Message(data) => {
//                 let WsRpcMessage { id, value } =
//                     match serde_json::from_slice::<WsRpcMessage<Response>>(&data) {
//                         Ok(data) => data,
//                         Err(e) => {
//                             log::warn!("{:?}", e);
//                             continue;
//                         }
//                     };

//                 match id_map.lock().await.remove(&id) {
//                     Some(rpc_response_tx) => {
//                         if let Err(_) = rpc_response_tx.send(value) {
//                             log::warn!("Failed to send rpc response");
//                         }
//                     }
//                     None => {
//                         log::warn!("message {} is removed", id);
//                     }
//                 }
//             }
//             WsEvent::Error(e) => {
//                 log::warn!("{:?}", e);
//                 break;
//             }
//         }
//     }
// }

// async fn accept_rpc_request_loop<Request, Response>(
//     mut rpc_request_source: mpsc::Receiver<Message<Request, Response>>,
//     ws: SendWrapper<WebSocket>,
//     id_map: Arc<Mutex<HashMap<i64, oneshot::Sender<Response>>>>,
// ) where
//     for<'de> Response: Deserialize<'de> + Send + 'static,
//     Request: Serialize + Send + 'static,
// {
//     let mut id_generator: i64 = 0;
//     while let Some(message) = rpc_request_source.recv().await {
//         let Message::<Request, Response> { req, resp } = message;

//         let data = match serde_json::to_string(&WsRpcMessage {
//             id: id_generator,
//             value: req,
//         }) {
//             Ok(data) => data,
//             Err(e) => {
//                 log::warn!("{:?}", e);
//                 continue;
//             }
//         };

//         if let Err(e) = ws.send_with_str(&data) {
//             log::warn!("{:?}", e);
//             continue;
//         }

//         id_map.lock().await.insert(id_generator, resp);

//         id_generator += 1;
//     }
// }
