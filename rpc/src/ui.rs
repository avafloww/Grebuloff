use super::{RpcClientboundMessage, RpcServerboundMessage};
use anyhow::{bail, Result};
use bytes::{Buf, Bytes, BytesMut};
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, PartialEq, Deserialize)]
pub enum UiRpcServerboundMessage {}

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

// note to future self: use actual structs instead of enum variant values
// since rmp-serde doesn't properly (how we want it to, anyways) support
// variant values
#[derive(Debug, PartialEq, Serialize)]
pub enum UiRpcClientboundMessage {
    /// Sent when the game window is resized.
    /// Triggers a resize of the UI.
    Resize(UiRpcClientboundResize),
}

#[derive(Debug, PartialEq)]
pub struct UiRpcServerboundPaint {
    pub width: u16,
    pub height: u16,
    pub format: ImageFormat,
    pub data: Bytes,
}

impl UiRpcServerboundPaint {
    pub fn from_raw(mut buf: BytesMut) -> Result<Self> {
        let data = buf.split_off(5).freeze();

        // image format is first, so we don't overlap 0x80..=0x8F | 0xDE..=0xDF (msgpack map)
        let format = match buf.get_u8() {
            0 => ImageFormat::BGRA8,
            _ => bail!("invalid image format"),
        };
        let width = buf.get_u16_le();
        let height = buf.get_u16_le();

        Ok(Self {
            width,
            height,
            format,
            data,
        })
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub struct UiRpcClientboundResize {
    pub width: u32,
    pub height: u32,
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
