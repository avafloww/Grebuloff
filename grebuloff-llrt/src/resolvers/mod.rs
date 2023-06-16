mod dalamud;
mod native;

use crate::GrebuloffLoadMethod;
use anyhow::Result;
use log::info;

pub async unsafe fn init_resolvers(load_method: GrebuloffLoadMethod) -> Result<()> {
    info!("init resolvers: {:?}", load_method);
    native::prepare()?;

    ffxivclientstructs::resolve_all_async(
        native::resolve_vtable,
        native::resolve_static_address,
        if load_method == GrebuloffLoadMethod::Dalamud {
            dalamud::resolve_member_function
        } else {
            native::resolve_member_function
        },
    )
    .await;

    Ok(())
}
