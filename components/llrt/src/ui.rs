use anyhow::Result;
use bytes::{Bytes, BytesMut};
use log::{error, info};
use std::{
    mem,
    sync::{
        atomic::{AtomicBool, Ordering},
        RwLock,
    },
};
use tokio::{
    io::AsyncReadExt,
    net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions},
    sync::oneshot,
};

use crate::{get_execution_id, get_tokio_rt};

// 0xFFFFFFFF + "LLRT"
const MAGIC: [u8; 8] = [0xff, 0xff, 0xff, 0xff, 0x4c, 0x4c, 0x52, 0x54];

// 32MB buffer allows for 4K 32-bit RGBA images
// TODO: make this configurable, or automatically sized based on the game window size
const PIPE_BUFFER_SIZE: u32 = 32 * 1024 * 1024;

static LATEST_BUFFER: RwLock<Option<UiBuffer>> = RwLock::new(None);

pub fn poll_buffer_for_new_data() -> Option<UiBufferSnapshot> {
    let lock = LATEST_BUFFER.read().unwrap();
    lock.as_ref().map(|v| v.poll_dirty()).flatten()
}

pub struct UiBufferSnapshot {
    pub width: u32,
    pub height: u32,
    pub data: Box<[u8]>,
}

struct UiBuffer {
    width: u32,
    height: u32,
    dirty: AtomicBool,
    data: Vec<u8>,
}

impl UiBuffer {
    pub fn poll_dirty(&self) -> Option<UiBufferSnapshot> {
        if self.dirty.swap(false, Ordering::Relaxed) {
            Some(UiBufferSnapshot {
                width: self.width,
                height: self.height,
                data: self.data.clone().into_boxed_slice(),
            })
        } else {
            None
        }
    }

    fn new_dirty(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            dirty: AtomicBool::new(true),
            data,
        }
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
    }
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
            .out_buffer_size(PIPE_BUFFER_SIZE)
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
    } // 0x24517CC0040

    async fn handle_client(&self, server: &mut NamedPipeServer) -> Result<()> {
        info!("UI host connected");

        // start the buffer at 4MB capacity (enough for at least 1280x720 RGBA)
        let mut buf = BytesMut::with_capacity(4 * 1024 * 1024);
        loop {
            server.readable().await?;

            match server.read_buf(&mut buf).await {
                Ok(0) => {
                    info!("UI host disconnected");
                    break;
                }
                Ok(_) => {
                    // read until we have a full message
                    // a full message is: the 8 byte magic, 4 byte message length, and the message
                    while buf.len() >= 12 {
                        if buf[0..8] == MAGIC {
                            let message_length = u32_from_slice(&buf[8..12]) as usize;
                            if buf.len() >= (12 + message_length) {
                                let mut message = buf.split_to(12 + message_length).split_off(12);
                                get_tokio_rt()
                                    .spawn_blocking(move || Self::process_message(&mut message));
                            } else {
                                break;
                            }
                        } else {
                            error!("UI host: invalid magic");
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("UI host: failed to read: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    fn process_message(message: &mut BytesMut) {
        // read null-terminated string
        let mut offset = 0;
        let mut string = Vec::new();
        while message[offset] != 0 {
            string.push(message[offset]);
            offset += 1;
        }

        // SAFETY: it doesn't matter if the string is not valid UTF-8, we just need to compare it
        let message_type = unsafe { std::str::from_utf8_unchecked(string.as_slice()) };
        let mut message = message.split_off(offset + 1);
        match message_type {
            "UI:IMG" => Self::process_image(&mut message),
            _ => error!("UI host: unknown message type {}", message_type),
        }
    }

    /// Processes an image message to prepare it for rendering to the screen.
    /// We take a ManuallyDrop<BytesMut> because we will avoid a clone by feeding
    /// the buffer directly to D3D as a texture.
    fn process_image(message: &mut BytesMut) {
        // read width (4 bytes), height (4 bytes), bytes per pixel (1 byte)
        let width = u32_from_slice(&message[0..4]);
        let height = u32_from_slice(&message[4..8]);
        let bpp = message[8];
        assert_eq!(bpp, 4, "bpp != 4 not supported");

        // dirty region x, y (4 bytes each), width, height (4 bytes each)
        let dirty_x = u32_from_slice(&message[9..13]);
        let dirty_y = u32_from_slice(&message[13..17]);
        let dirty_width = u32_from_slice(&message[17..21]);
        let dirty_height = u32_from_slice(&message[21..25]);

        // Electron on Windows uses BGRA as its native format, so we need to convert to RGBA
        let mut data = message.split_off(25);

        #[cfg(debug_assertions)]
        if data.len() != (dirty_width * dirty_height * bpp as u32) as usize {
            unsafe { std::intrinsics::breakpoint() };
        }
        // assert_eq!(
        //     data.len(),
        //     (dirty_width * dirty_height * bpp as u32) as usize
        // );

        data.chunks_exact_mut(4).for_each(|c| c.swap(0, 2));

        Self::merge_dirty_region(
            width,
            height,
            dirty_x,
            dirty_y,
            dirty_width,
            dirty_height,
            data.freeze(),
        )
    }

    fn merge_dirty_region(
        width: u32,
        height: u32,
        dirty_x: u32,
        dirty_y: u32,
        dirty_width: u32,
        dirty_height: u32,
        new_data: Bytes,
    ) {
        let mut lock = LATEST_BUFFER.write().unwrap();

        // if there's no existing buffer, or the dirty region is the entire buffer, just replace it
        if lock.is_none() {
            *lock = Some(UiBuffer::new_dirty(width, height, new_data.to_vec()));
            return;
        }

        let buffer = lock.as_mut().unwrap();

        // if the dirty region is the entire buffer, and the existing buffer is the same size, copy in place
        // instead of allocating new memory
        if buffer.data.len() == new_data.len() && buffer.width == width && buffer.height == height {
            buffer.data.copy_from_slice(&new_data);
            buffer.mark_dirty();
            return;
        }

        let bpp = 4;
        let stride = width * bpp;
        let dirty_stride = dirty_width * bpp;

        let mut offset = (dirty_y * stride + dirty_x * bpp) as usize;
        let mut dirty_offset = 0;
        for _ in 0..dirty_height {
            buffer.data[offset..(offset + dirty_stride as usize)]
                .copy_from_slice(&new_data[dirty_offset..(dirty_offset + dirty_stride as usize)]);
            offset += stride as usize;
            dirty_offset += dirty_stride as usize;
        }

        buffer.mark_dirty();
    }
}

#[inline]
fn u32_from_slice(slice: &[u8]) -> u32 {
    let slice: &[u8; 4] = slice.try_into().unwrap();
    u32::from_le_bytes(*slice)
}
