use crate::{
    get_execution_id,
    rpc::ui::{ImageFormat, UiRpcServerboundPaint},
};
use anyhow::{bail, Result};
use bytes::Bytes;
use log::{error, info, warn};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
};

static LATEST_BUFFER: Mutex<Option<UiBuffer>> = Mutex::new(None);

pub async fn spawn_ui_host(runtime_dir: &PathBuf) -> Result<()> {
    loop {
        info!(
            "spawning HLRT process (runtime dir: {})",
            runtime_dir.to_str().unwrap()
        );

        let mut builder = Command::new(
            Path::new(runtime_dir)
                .join("grebuloff-hlrt-win32-x64")
                .join("grebuloff-hlrt.exe"),
        );

        builder.stdout(Stdio::piped());
        builder.stderr(Stdio::piped());
        builder.env("LLRT_PIPE_ID", get_execution_id());

        #[cfg(debug_assertions)]
        {
            // in debug builds, we want to use the local dev server
            // todo: this should probably be configurable
            builder.env("ELECTRON_RENDERER_URL", "http://localhost:5173/");
        }

        if let Ok(mut process) = builder.spawn() {
            info!("spawned HLRT process with pid {:?}", process.id());

            let mut stdout = BufReader::new(process.stdout.take().unwrap()).lines();
            let mut stderr = BufReader::new(process.stderr.take().unwrap()).lines();

            loop {
                tokio::select! {
                    out = stdout.next_line() => {
                        if let Ok(Some(line)) = out {
                            info!("[hlrt:out] {}", line);
                        }
                    },
                    err = stderr.next_line() => {
                        if let Ok(Some(line)) = err {
                            warn!("[hlrt:err] {}", line);
                        }
                    },
                    status = process.wait() => {
                        info!("[hlrt:exit] HLRT process exited with status {}", status.unwrap());
                        break;
                    }
                }
            }
        } else {
            error!("failed to spawn HLRT process");
            bail!("failed to spawn HLRT process");
        }
    }

    Ok(())
}

pub fn poll_dirty() -> Option<UiBufferSnapshot> {
    let mut lock = LATEST_BUFFER.lock().unwrap();
    lock.as_mut().map(|v| v.poll_dirty()).flatten()
}

pub fn update_buffer_on_paint(paint: UiRpcServerboundPaint) {
    assert_eq!(
        paint.format,
        ImageFormat::BGRA8,
        "only ImageFormat::BGRA8 is supported"
    );

    assert_eq!(
        paint.data.len(),
        paint
            .format
            .byte_size_of(paint.width as usize, paint.height as usize)
    );

    let mut lock = LATEST_BUFFER.lock().unwrap();
    let _ = lock.insert(UiBuffer::new_dirty(
        paint.width.into(),
        paint.height.into(),
        paint.data,
    ));
}

pub struct UiBufferSnapshot {
    pub width: u32,
    pub height: u32,
    pub data: Bytes,
}

struct UiBuffer {
    width: u32,
    height: u32,
    dirty: AtomicBool,
    data: Option<Bytes>,
}

impl UiBuffer {
    pub fn poll_dirty(&mut self) -> Option<UiBufferSnapshot> {
        if self.dirty.swap(false, Ordering::Relaxed) {
            if let Some(data) = self.data.take() {
                return Some(UiBufferSnapshot {
                    width: self.width,
                    height: self.height,
                    data,
                });
            }
        }

        None
    }

    fn new_dirty(width: u32, height: u32, data: Bytes) -> Self {
        Self {
            width,
            height,
            dirty: AtomicBool::new(true),
            data: Some(data),
        }
    }
}
