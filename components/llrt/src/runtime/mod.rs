pub mod callable;
pub mod context;
pub mod engine;

use crate::runtime::context::ContextOptions;
use log::info;
use std::{borrow::Cow, path::PathBuf};

const CORE_RUNTIME_KEY: &str = "grebuloff-hlrt";
pub(crate) async fn init_hlrt_context(_runtime_dir: &PathBuf) -> anyhow::Result<()> {
    info!("initializing high-level runtime context");
    let hlrt_ctx = context::spawn_context(ContextOptions {
        key: Cow::from(CORE_RUNTIME_KEY),
    })
    .await?;

    info!("loading core runtime");
    hlrt_ctx
        .execute_code(
            "Grebuloff.LLRT.print('hello from Grebuloff.LLRT.print()!');
                        Grebuloff.LLRT.create_context('context_from_hlrt');
                        Grebuloff.LLRT.print('we did the thing!');",
        )
        .await?;

    Ok(())
}
