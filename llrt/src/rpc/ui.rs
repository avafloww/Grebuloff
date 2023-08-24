use super::{RpcServer, RpcServerOptions};
use crate::{get_execution_id, get_tokio_rt};
use anyhow::Result;
use bytes::BytesMut;
use grebuloff_rpc::ui::*;
use log::debug;
use std::sync::OnceLock;
use tokio::sync::mpsc;

// 32MB buffer allows for 4K 32-bit RGBA images
// TODO: make this configurable, or automatically sized based on the game window size
const PIPE_BUFFER_SIZE: usize = 32 * 1024 * 1024;

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
