use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Chunk {
    Data(Vec<u8>),
    Error,
    End,
}

#[derive(Serialize, Deserialize)]
pub enum NetMessage {
    Hi(String),
    Bye(String),
    BBB(String),
    UserMessage(String, String),
    WhisperMessage(String, String, String),
    UserData(String, String, String, Chunk),
}