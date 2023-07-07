use super::{RpcClientboundMessage, RpcServer, RpcServerOptions, RpcServerboundMessage};
use crate::get_execution_id;

// 32MB buffer allows for 4K 32-bit RGBA images
// TODO: make this configurable, or automatically sized based on the game window size
const PIPE_BUFFER_SIZE: usize = 32 * 1024 * 1024;

#[derive(Debug, PartialEq, Deserialize)]
pub enum UiRpcServerboundMessage {
    /// A request to paint the UI.
    Paint(UiRpcServerboundPaint),
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct UiRpcServerboundPaint {
    #[serde(rename = "vw")]
    pub viewport_width: u32,

    #[serde(rename = "vh")]
    pub viewport_height: u32,

    #[serde(rename = "f")]
    pub format: ImageFormat,

    #[serde(rename = "dx")]
    pub dirty_x: u32,

    #[serde(rename = "dy")]
    pub dirty_y: u32,

    #[serde(rename = "dw")]
    pub dirty_width: u32,

    #[serde(rename = "dh")]
    pub dirty_height: u32,

    #[serde(rename = "d", with = "serde_bytes")]
    pub data: Vec<u8>,
}

impl UiRpcServerboundPaint {
    pub fn is_fully_dirty(&self) -> bool {
        self.dirty_x == 0
            && self.dirty_y == 0
            && self.dirty_width == self.viewport_width
            && self.dirty_height == self.viewport_height
    }
}

impl TryFrom<RpcServerboundMessage> for UiRpcServerboundMessage {
    type Error = ();

    fn try_from(msg: RpcServerboundMessage) -> Result<Self, Self::Error> {
        match msg {
            RpcServerboundMessage::Ui(msg) => Ok(msg),
            _ => Err(()),
        }
    }
}

impl From<UiRpcClientboundMessage> for RpcClientboundMessage {
    fn from(msg: UiRpcClientboundMessage) -> Self {
        RpcClientboundMessage::Ui(msg)
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub enum UiRpcClientboundMessage {
    /// Sent when the game window is resized.
    /// Triggers a resize of the UI.
    Resize { width: u32, height: u32 },
}

/// Represents supported image formats.
#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[repr(u8)]
pub enum ImageFormat {
    BGRA8,
}

impl ImageFormat {
    pub fn byte_size_of(&self, width: usize, height: usize) -> usize {
        width * height * self.bytes_per_pixel() as usize
    }

    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            ImageFormat::BGRA8 => 4,
        }
    }
}

pub struct UiRpcServer {
    options: RpcServerOptions,
}

impl RpcServer for UiRpcServer {
    const SERVER_NAME: &'static str = "ui";

    type Serverbound = UiRpcServerboundMessage;
    type Clientbound = UiRpcClientboundMessage;

    fn options(&self) -> &super::RpcServerOptions {
        &self.options
    }

    fn process_incoming_message(
        _send: tokio::sync::mpsc::UnboundedSender<<Self as RpcServer>::Clientbound>,
        message: Self::Serverbound,
    ) -> anyhow::Result<()> {
        match message {
            UiRpcServerboundMessage::Paint(paint) => crate::ui::update_buffer_on_paint(paint),
            _ => unimplemented!(),
        }

        Ok(())
    }
}

impl UiRpcServer {
    pub fn new() -> Self {
        Self {
            options: RpcServerOptions {
                pipe_name: format!("\\\\.\\pipe\\grebuloff-llrt-ui-{}", get_execution_id()).into(),
                buffer_size: PIPE_BUFFER_SIZE,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::rpc::RpcMessageDirection;

    #[test]
    fn test_from() {
        let valid = UiRpcServerboundMessage::Paint(UiRpcServerboundPaint {
            viewport_width: 0,
            viewport_height: 0,
            format: ImageFormat::BGRA8,
            dirty_x: 0,
            dirty_y: 0,
            dirty_width: 0,
            dirty_height: 0,
            data: vec![],
        });
        assert!(<<UiRpcServer as RpcServer>::Serverbound>::try_from(valid).is_ok());
    }

    // We deserialize in JSON for testing because it's easier to read.
    // Actual implementation code uses msgpack.
    #[test]
    fn test_deserialize() {
        let msg = RpcMessageDirection::Serverbound(RpcServerboundMessage::Ui(
            UiRpcServerboundMessage::Paint(UiRpcServerboundPaint {
                viewport_width: 123,
                viewport_height: 456,
                format: ImageFormat::BGRA8,
                dirty_x: 69,
                dirty_y: 42,
                dirty_width: 1337,
                dirty_height: 420,
                data: vec![12, 34, 56, 78],
            }),
        ));

        let serialized = r#"{"Ui":{"Paint":{"vw":123,"vh":456,"f":"BGRA8","dx":69,"dy":42,"dw":1337,"dh":420,"d":[12,34,56,78]}}}"#;

        let mut de = serde_json::Deserializer::from_str(serialized);
        let deserialized = RpcMessageDirection::deserialize(&mut de);
        assert!(deserialized.is_ok());
        assert_eq!(deserialized.unwrap(), msg);
    }
}
