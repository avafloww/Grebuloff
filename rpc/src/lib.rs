use serde::{Deserialize, Serialize};

pub mod ui;

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RpcMessageDirection {
    /// Serverbound (client-to-server) communication.
    #[serde(skip_serializing)]
    Serverbound(RpcServerboundMessage),

    /// Clientbound (server-to-client) communication.
    #[serde(skip_deserializing)]
    Clientbound(RpcClientboundMessage),
}

#[derive(Debug, PartialEq, Deserialize)]
pub enum RpcServerboundMessage {
    Ui(ui::UiRpcServerboundMessage),
}

#[derive(Debug, PartialEq, Serialize)]
pub enum RpcClientboundMessage {
    Ui(ui::UiRpcClientboundMessage),
}
