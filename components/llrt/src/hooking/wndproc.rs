use super::create_function_hook;
use crate::resolvers::resolve_signature;
use anyhow::Result;
use grebuloff_macros::function_hook;
use log::debug;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};

pub unsafe fn hook_wndproc() -> Result<()> {
    let wndproc_ptr = resolve_signature!("E8 ?? ?? ?? ?? 80 7C 24 ?? ?? 74 ?? B8");
    debug!("WndProc: {:p}", wndproc_ptr);
    create_function_hook!(wndproc, wndproc_ptr).enable()?;

    Ok(())
}

#[function_hook]
unsafe fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    debug!(
        "WndProc invoked with: hwnd = {:?}, msg = {}, wparam = {:?}, lparam = {:?}",
        hwnd, msg, wparam, lparam
    );
    original.call(hwnd, msg, wparam, lparam)
}
