pub mod callable;
pub mod context;
pub mod engine;

use crate::runtime::context::ContextOptions;
use log::info;
use std::{borrow::Cow, path::PathBuf};

const CORE_RUNTIME_KEY: &str = "HLRT";
pub async fn init_hlrt_context(runtime_dir: &PathBuf) -> anyhow::Result<()> {
    info!("initializing high-level runtime context");
    let hlrt_ctx = context::JsContext::create(ContextOptions {
        id: Cow::from(CORE_RUNTIME_KEY),
        module_base_path: Some(runtime_dir.clone()),
    })
    .await?;

    info!("loading core runtime");
    hlrt_ctx
        .execute_code(
            "Grebuloff.LLRT.print('hello from Grebuloff.LLRT.print()!');
                        Grebuloff.LLRT.create_context('context_from_hlrt');
                        Grebuloff.LLRT.print('we did the thing! testing import...');
                        import('test import');
                        Grebuloff.LLRT.print('past the import');",
        )
        .await?;

    Ok(())
}
