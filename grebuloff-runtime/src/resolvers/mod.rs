mod dalamud;
mod native;

use crate::GrebuloffLoadMethod;
use anyhow::Result;
use log::info;

pub unsafe fn init_resolvers(load_method: GrebuloffLoadMethod) -> Result<()> {
    info!("init resolvers: {:?}", load_method);
    match load_method {
        GrebuloffLoadMethod::Native => {
            native::prepare()?;
            ffxivclientstructs::resolve_all(
                native::resolve_vtable,
                native::resolve_static_address,
                native::resolve_member_function,
            )
        }
        GrebuloffLoadMethod::Dalamud => ffxivclientstructs::resolve_all(
            dalamud::resolve_vtable,
            dalamud::resolve_static_address,
            dalamud::resolve_member_function,
        ),
    }

    Ok(())
}
