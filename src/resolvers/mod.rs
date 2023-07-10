mod dalamud;
mod native;

use crate::{get_load_method, GrebuloffLoadMethod};
use anyhow::Result;
use ffxiv_client_structs::MemberFunctionSignature;
use log::info;

pub async unsafe fn init_resolvers(load_method: GrebuloffLoadMethod) -> Result<()> {
    info!("init resolvers: {:?}", load_method);
    native::prepare()?;

    ffxiv_client_structs::resolve_all_async(
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

/// Internal helper function used by the `resolve_signature` macro.
pub unsafe fn resolve_member_function(input: &MemberFunctionSignature) -> *const u8 {
    if get_load_method() == GrebuloffLoadMethod::Dalamud {
        dalamud::resolve_member_function(input)
    } else {
        native::resolve_member_function(input)
    }
}

/// Resolves a signature to a pointer.
/// Returns a null pointer if the signature could not be resolved.
macro_rules! resolve_signature {
    ($signature: tt) => {{
        let member_func = ::ffxiv_client_structs::MemberFunctionSignature::new(
            ::ffxiv_client_structs_macros::signature!($signature),
        );

        crate::resolvers::resolve_member_function(&member_func)
    }};
}
pub(crate) use resolve_signature;
