use super::{RpcClientboundMessage, RpcServer, RpcServerOptions, RpcServerboundMessage};
use crate::{get_execution_id, get_tokio_rt};
use anyhow::{bail, Result};
use bytes::{Buf, Bytes, BytesMut};
use log::debug;
use std::sync::OnceLock;
use tokio::sync::mpsc;

// 32MB buffer allows for 4K 32-bit RGBA images
// TODO: make this configurable, or automatically sized based on the game window size
const PIPE_BUFFER_SIZE: usize = 32 * 1024 * 1024;

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
    fn from_raw(mut buf: BytesMut) -> Result<Self> {
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
            _ => unimplemented!(),
        }

        Ok(())
    }

    fn process_incoming_message_raw(
        _send: mpsc::UnboundedSender<<Self as RpcServer>::Clientbound>,
        message: BytesMut,
    ) -> Result<()> {
        // UI only uses raw messages for paint, so process it directly
        let paint = UiRpcServerboundPaint::from_raw(message)?;
        crate::ui::update_buffer_on_paint(paint);

        Ok(())
    }
}

static mut UI_RPC_SERVER: OnceLock<UiRpcServer> = OnceLock::new();

impl UiRpcServer {
    fn new() -> Self {
        Self {
            options: RpcServerOptions {
                pipe_name: format!("\\\\.\\pipe\\grebuloff-llrt-ui-{}", get_execution_id()).into(),
                buffer_size: PIPE_BUFFER_SIZE,
            },
        }
    }

    pub fn instance() -> &'static Self {
        unsafe { UI_RPC_SERVER.get_or_init(Self::new) }
    }

    pub fn resize(width: u32, height: u32) {
        get_tokio_rt().spawn(async move {
            debug!("informing UI of resize to {}x{}", width, height);
            Self::queue_send(UiRpcClientboundMessage::Resize(UiRpcClientboundResize {
                width,
                height,
            }))
            .await
        });
    }
}
