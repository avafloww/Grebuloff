use std::sync::{
    atomic::{AtomicBool, Ordering},
    RwLock,
};

use crate::rpc::ui::{ImageFormat, UiRpcServerboundPaint};

static LATEST_BUFFER: RwLock<Option<UiBuffer>> = RwLock::new(None);

pub fn poll_dirty() -> Option<UiBufferSnapshot> {
    let lock = LATEST_BUFFER.read().unwrap();
    lock.as_ref().map(|v| v.poll_dirty()).flatten()
}

pub fn update_buffer_on_paint(mut paint: UiRpcServerboundPaint) {
    assert_eq!(
        paint.format,
        ImageFormat::RGBA8,
        "only ImageFormat::RGBA8 is supported"
    );

    assert_eq!(
        paint.data.len(),
        paint
            .format
            .byte_size_of(paint.dirty_width as usize, paint.dirty_height as usize)
    );

    merge_dirty_region(&paint)
}

fn merge_dirty_region(paint: &UiRpcServerboundPaint) {
    let mut lock = LATEST_BUFFER.write().unwrap();

    // if there's no existing buffer, create one
    if lock.is_none() {
        *lock = Some(UiBuffer::new_dirty(
            paint.dirty_width,
            paint.dirty_height,
            paint.data.to_vec(),
        ));
        return;
    }

    let buffer = lock.as_mut().unwrap();

    // if the dirty region is the entire buffer, and the existing buffer is the same size, copy in place
    // instead of allocating new memory
    if buffer.data.len() == paint.data.len() && paint.is_fully_dirty() {
        buffer.data.copy_from_slice(&paint.data);
        buffer.mark_dirty();
        return;
    }

    let stride = paint.viewport_width * paint.format.bytes_per_pixel();
    let dirty_stride = paint.dirty_width * paint.format.bytes_per_pixel();

    let mut offset =
        (paint.dirty_y * stride + paint.dirty_x * paint.format.bytes_per_pixel()) as usize;
    let mut dirty_offset = 0;
    for _ in 0..paint.dirty_height {
        buffer.data[offset..(offset + dirty_stride as usize)]
            .copy_from_slice(&paint.data[dirty_offset..(dirty_offset + dirty_stride as usize)]);
        offset += stride as usize;
        dirty_offset += dirty_stride as usize;
    }

    buffer.mark_dirty();
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
