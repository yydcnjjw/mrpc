use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct WsRpcMessage<Value> {
    pub id: i64,
    pub value: Value,
}
