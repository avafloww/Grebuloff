use crate::hooking::create_function_hook;
use anyhow::Result;
use ffxiv_client_structs::generated::ffxiv::client::graphics::kernel::{
    Device, Device_Fn_Instance,
};
use grebuloff_macros::{function_hook, vtable_functions, VTable};
use log::{debug, trace};
use std::ffi::c_void;

#[derive(VTable)]
struct ResolvedSwapChain {
    #[vtable_base]
    base: *mut *mut c_void,
}

vtable_functions!(impl ResolvedSwapChain {
    #[vtable_fn(8)]
    unsafe fn present(&self, sync_interval: u32, present_flags: u32);

    #[vtable_fn(13)]
    unsafe fn resize_buffers(
        &self,
        buffer_count: u32,
        width: u32,
        height: u32,
        new_format: u32,
        swap_chain_flags: u32,
    );
});

unsafe fn resolve_swap_chain() -> ResolvedSwapChain {
    debug!("resolving swap chain");
    let device = loop {
        let device = ffxiv_client_structs::address::get::<Device_Fn_Instance>() as *mut Device;

        if device.is_null() {
            trace!("device is null, waiting");
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        break device;
    };

    debug!("device: {:p}", device);
    let swap_chain = (*device).swap_chain;
    debug!("swap chain: {:p}", swap_chain);
    let dxgi_swap_chain = (*swap_chain).dxgiswap_chain as *mut *mut *mut c_void;
    debug!("dxgi swap chain: {:p}", *dxgi_swap_chain);

    ResolvedSwapChain {
        base: *dxgi_swap_chain,
    }
}

pub unsafe fn hook_swap_chain() -> Result<()> {
    let resolved = resolve_swap_chain();

    create_function_hook!(present, *resolved.address_table().present()).enable()?;

    Ok(())
}

#[function_hook]
unsafe fn present(this: *mut c_void, sync_interval: u32, present_flags: u32) -> i32 {
    original.call(this, sync_interval, present_flags)
}
