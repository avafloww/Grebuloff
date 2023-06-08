mod ext;
mod ops;

use crate::runtime::ext::{get_ext_privileged, get_ext_unprivileged};
use anyhow::{Context, Result};
use deno_core::{Extension, JsRuntime, RuntimeOptions};
use include_dir::{include_dir, Dir};
use log::{error, info, Log};
use std::rc::Rc;

static RUNTIME_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/runtime/js");

pub async fn init_core_runtime() -> Result<()> {
    info!("initializing core runtime");

    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![get_ext_unprivileged(), get_ext_privileged()],
        is_main: true,
        module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
        ..Default::default()
    });

    let main_module = deno_core::resolve_path(
        "./core/main.js",
        &std::env::current_dir()
            .context("failed to get current directory")
            .unwrap(),
    )?;

    info!("main module: {:?}", main_module);

    let mod_id = runtime.load_main_module(&main_module, None).await?;

    info!("loaded main module: {:?}", mod_id);

    let result = runtime.mod_evaluate(mod_id);

    info!("running event loop");

    runtime.run_event_loop(false).await?;
    result.await??;

    Ok(())
}
