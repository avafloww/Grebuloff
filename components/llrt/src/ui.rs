use bytes::Bytes;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use crate::rpc::ui::{ImageFormat, UiRpcServerboundPaint};

static LATEST_BUFFER: Mutex<Option<UiBuffer>> = Mutex::new(None);

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
