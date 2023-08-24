use std::sync::Once;

use anyhow::Result;
use ffxiv_client_structs::generated::ffxiv::client::system::framework::{
    Framework, Framework_Fn_Instance,
};
use grebuloff_macros::{function_hook, vtable_functions, VTable};
use log::debug;

use crate::{get_tokio_rt, hooking::create_function_hook};

#[derive(VTable)]
struct FrameworkVTable {
    #[vtable_base]
    base: *mut *mut Framework,
}

vtable_functions!(impl FrameworkVTable {
    #[vtable_fn(1)]
    unsafe fn setup(this: *const Framework);

    #[vtable_fn(2)]
    unsafe fn destroy(this: *const Framework);

    #[vtable_fn(3)]
    unsafe fn free(this: *const Framework);

    #[vtable_fn(4)]
    unsafe fn tick(this: *const Framework) -> bool;
});

pub unsafe fn hook_framework() -> Result<()> {
    let framework =
        ffxiv_client_structs::address::get::<Framework_Fn_Instance>() as *mut *mut *mut Framework;
    assert!(!framework.is_null(), "failed to resolve Framework instance");

    debug!("framework: {:p}", framework);
    let vtable = FrameworkVTable { base: *framework };
    debug!("framework vtable: {:p}", vtable.base);

    create_function_hook!(tick, *vtable.address_table().tick()).enable()?;

    Ok(())
}

#[function_hook]
unsafe extern "C" fn tick(this: *const Framework) -> bool {
    static LATE_INIT: Once = Once::new();
    LATE_INIT.call_once(|| {
        get_tokio_rt().block_on(crate::init_sync_late());
    });

    original.call(this)
}
