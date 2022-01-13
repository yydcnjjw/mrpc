use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TranportMesssage<T> {
    pub id: usize,
    pub message: T,
}
