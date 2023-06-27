mod module;
mod ops;

use crate::get_tokio_rt;
use crate::runtime::module::RuntimeModuleLoader;
use anyhow::Result;
use deno_core::{include_js_files, Extension, JsRuntime, RuntimeOptions};
use grebuloff_macros::{libhlrt_esm_files, libhlrt_esm_main};
use log::{debug, info};
use std::{path::Path, rc::Rc};
use std::{path::PathBuf, thread};
use tokio::sync::{watch, OnceCell};

pub type ShutdownSignalSender = watch::Sender<bool>;
pub type ShutdownSignalReceiver = watch::Receiver<bool>;

#[derive(Debug)]
struct ShutdownSignal {
    tx: ShutdownSignalSender,
    rx: ShutdownSignalReceiver,
}

impl ShutdownSignal {
    fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self { tx, rx }
    }

    fn get_sender(&self) -> &ShutdownSignalSender {
        &self.tx
    }

    fn receiver(&self) -> ShutdownSignalReceiver {
        self.rx.clone()
    }
}

static GLOBAL_SHUTDOWN_SIGNAL: OnceCell<ShutdownSignal> = OnceCell::const_new();

pub fn shutdown_all() {
    if let Some(signal) = GLOBAL_SHUTDOWN_SIGNAL.get() {
        info!("shutting down all runtimes");
        signal.get_sender().send(true).unwrap();
    }
}

pub async fn init_hlrt(runtime_dir: &PathBuf) -> Result<()> {
    info!("initializing high-level runtime");

    // create the global shutdown signal
    GLOBAL_SHUTDOWN_SIGNAL.set(ShutdownSignal::new()).unwrap();

    // create the high-level runtime
    spawn_runtime(runtime_dir)?;

    Ok(())
}

pub fn spawn_runtime(runtime_dir: &PathBuf) -> Result<()> {
    info!("spawning runtime");

    let mut runtime = Runtime::new(runtime_dir.to_owned());
    thread::spawn(move || runtime.runtime_thread_exec());

    Ok(())
}

pub struct Runtime {
    runtime_dir: PathBuf,
    shutdown_signal: ShutdownSignal,
}

impl Runtime {
    pub fn new(runtime_dir: PathBuf) -> Self {
        Self {
            runtime_dir,
            shutdown_signal: ShutdownSignal::new(),
        }
    }

    fn runtime_thread_exec(&mut self) {
        get_tokio_rt()
            .block_on(async move { self.runtime_thread_async().await })
            .unwrap();
    }

    async fn runtime_thread_async(&mut self) -> Result<()> {
        let mut js_runtime = JsRuntime::new(RuntimeOptions {
            extensions: vec![extension_decl()],
            is_main: true,
            module_loader: Some(Rc::new(RuntimeModuleLoader)),
            ..Default::default()
        });

        let main_module = deno_core::resolve_path("hlrt/index.js", &self.runtime_dir)?;
        let code = std::fs::read_to_string(main_module.to_file_path().unwrap())?;

        let mod_id = js_runtime
            .load_main_module(&main_module, Some(code.into()))
            .await?;
        let mut receiver = js_runtime.mod_evaluate(mod_id);

        info!("runtime thread starting event loop");

        loop {
            tokio::select! {
                maybe_result = &mut receiver => {
                    debug!("received module evaluate {:#?}", maybe_result);
                }

                _ = js_runtime.run_event_loop(false) => {
                    debug!("run_event_loop returned");
                }

                _ = self.await_shutdown() => {
                    debug!("received shutdown signal");
                    return Ok(());
                }
            }
        }
    }

    async fn await_shutdown(&self) {
        let mut global_shutdown_signal = GLOBAL_SHUTDOWN_SIGNAL.get().unwrap().receiver();
        let mut rt_shutdown_signal = self.shutdown_signal.receiver();

        tokio::select! {
            _ = global_shutdown_signal.changed() => {
                info!("received global shutdown signal");
            }

            _ = rt_shutdown_signal.changed() => {
                info!("received runtime shutdown signal");
            }
        }
    }
}

fn extension_decl() -> Extension {
    Extension::builder("grebuloff")
        .js(libhlrt_js_files!())
        .ops(ops::collect())
        .build()
}
