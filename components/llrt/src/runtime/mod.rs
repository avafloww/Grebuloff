pub mod context;
pub mod engine;

use crate::runtime::context::ContextOptions;
use log::info;
use std::path::PathBuf;

const CORE_RUNTIME_KEY: &str = "grebuloff-hlrt";
pub(crate) async fn init_hlrt_context(runtime_dir: &PathBuf) -> anyhow::Result<()> {
    info!("initializing high-level runtime context");
    let hlrt_ctx = context::spawn_context(ContextOptions {
        key: CORE_RUNTIME_KEY,
        is_main_context: true,
    })
    .await?;

    info!("loading core runtime");
    hlrt_ctx
        .execute("console.log('hello from hlrt in v8!');")
        .await?;

    Ok(())
}
