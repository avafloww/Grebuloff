use anyhow::Result;
use log::{debug, error, info};
use std::sync::RwLock;
use tokio::{
    io::AsyncReadExt,
    net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions},
    sync::oneshot,
};

use crate::get_execution_id;

// 0xFFFFFFFF + "LLRT"
const MAGIC: [u8; 8] = [0xff, 0xff, 0xff, 0xff, 0x4c, 0x4c, 0x52, 0x54];

// 32MB buffer allows for 4K 32-bit RGBA images
// TODO: make this configurable, or automatically sized based on the game window size
const PIPE_BUFFER_SIZE: u32 = 32 * 1024 * 1024;

static LATEST_BUFFER: RwLock<Option<Vec<u8>>> = RwLock::new(None);

pub fn get_latest_buffer() -> Option<Box<[u8]>> {
    let lock = LATEST_BUFFER.read().unwrap();
    lock.as_ref().map(|v| v.clone().into_boxed_slice())
}

pub struct UiHost {
    pipe_name: String,
    shutdown_tx: oneshot::Sender<()>,
    shutdown_rx: oneshot::Receiver<()>,
}

impl UiHost {
    pub fn new() -> Self {
        let (tx, rx) = oneshot::channel::<()>();

        Self {
            pipe_name: format!("\\\\.\\pipe\\grebuloff-llrt-ui-{}", get_execution_id()).to_owned(),
            shutdown_tx: tx,
            shutdown_rx: rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("UI host starting on {}", self.pipe_name);
        let mut server = ServerOptions::new()
            .pipe_mode(PipeMode::Byte)
            .in_buffer_size(PIPE_BUFFER_SIZE)
            .create(self.pipe_name.clone())?;

        loop {
            tokio::select! {
                _ = &mut self.shutdown_rx => {
                    info!("UI host shutting down");
                    break;
                }
                res = server.connect() => match res {
                    Ok(_) => self.handle_client(&mut server).await?,
                    Err(e) => error!("UI host: failed to connect: {}", e),
                }
            }
        }

        Ok(())
    }

    async fn handle_client(&self, server: &mut NamedPipeServer) -> Result<()> {
        info!("UI host connected");

        loop {
            server.readable().await?;

            // try to read header and length
            let mut header = [0; 12];
            server.read_exact(&mut header).await?;

            // ensure the first 8 bytes match the magic
            if header[0..8] != MAGIC {
                error!("UI host: invalid magic");
                break;
            }

            // read the length from the last 4 bytes
            let length = u32_from_slice(&header[8..12]) as usize;

            let mut incoming = vec![0; length];
            server.read_exact(&mut incoming).await?;

            debug!("UI host: finished reading {} bytes", incoming.len());
            self.process_message(&incoming.as_slice());
        }

        Ok(())
    }

    fn process_message(&self, message: &[u8]) {
        // read null-terminated string
        let mut offset = 0;
        let mut string = Vec::new();
        while message[offset] != 0 {
            string.push(message[offset]);
            offset += 1;
        }

        let message_type = std::str::from_utf8(string.as_slice()).unwrap();
        match message_type {
            "UI:IMG" => self.process_image(&message[(offset + 1)..]),
            _ => error!("UI host: unknown message type {}", message_type),
        }
    }

    fn process_image(&self, message: &[u8]) {
        // read width (4 bytes), height (4 bytes), bytes per pixel (1 byte)
        let width = u32_from_slice(&message[0..4]);
        let height = u32_from_slice(&message[4..8]);
        let bpp = message[8];

        debug!(
            "UI host: image is {}x{} with {} bytes per pixel",
            width, height, bpp
        );

        let data = &message[9..];
        assert_eq!(data.len(), (width * height * bpp as u32) as usize);

        // Electron on Windows uses BGRA as its native format, so we need to convert to RGBA
        let mut data = data.to_vec();
        data.chunks_exact_mut(4).for_each(|c| c.swap(0, 2));

        let mut lock = LATEST_BUFFER.write().unwrap();
        *lock = Some(data);
    }
}

#[inline]
fn u32_from_slice(slice: &[u8]) -> u32 {
    let slice: &[u8; 4] = slice.try_into().unwrap();
    u32::from_le_bytes(*slice)
}
