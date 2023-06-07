mod dalamud;
mod runtime;
mod webview;

use crate::dalamud::DalamudPipe;
use crate::runtime::init_core_runtime;
use crate::webview::WebView;
use anyhow::Result;
use log::{error, info};
use msgbox::IconType;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::thread;
use tokio::runtime::Handle;
use tokio::task;

static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static DALAMUD_PIPE: OnceLock<DalamudPipe> = OnceLock::new();

dll_syringe::payload_procedure! {
    fn init_native(runtime_dir: Vec<u8>) {
        init_sync(runtime_dir, None)
    }
}

dll_syringe::payload_procedure! {
    fn init_dalamud(runtime_dir: Vec<u8>, dalamud_pipe_name: Vec<u8>) {
        init_sync(runtime_dir, Some(dalamud_pipe_name))
    }
}

#[derive(Copy, Clone, Debug)]
pub enum GrebuloffLoadMethod {
    Native,
    Dalamud,
}

pub fn get_load_method() -> GrebuloffLoadMethod {
    if let Some(_) = DALAMUD_PIPE.get() {
        return GrebuloffLoadMethod::Dalamud;
    }

    GrebuloffLoadMethod::Native
}

fn alert(message: &str) {
    msgbox::create("Grebuloff", message, IconType::Info).unwrap();
}

fn setup_logging(dir: &PathBuf) {
    // log to grebuloff.log in the specified directory
    // log format should have timestamps, level, module, and message
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .chain(fern::log_file(dir.join("grebuloff.log")).unwrap())
        .apply()
        .unwrap();

    // log on panic
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::capture();

        let thread = thread::current();
        let thread = thread.name().unwrap_or("<unnamed>");
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };

        let formatted = match info.location() {
            Some(location) => {
                format!(
                    "thread '{}' panicked at '{}': {}:{}{:?}",
                    thread,
                    msg,
                    location.file(),
                    location.line(),
                    backtrace
                )
            }
            None => format!("thread '{}' panicked at '{}'{:?}", thread, msg, backtrace),
        };

        error!("{}", formatted);
        log::logger().flush();

        msgbox::create("Grebuloff", &formatted, IconType::Error).unwrap();
    }));
}

fn init_sync(runtime_dir: Vec<u8>, dalamud_pipe_name: Option<Vec<u8>>) {
    let runtime_dir = PathBuf::from(std::str::from_utf8(&runtime_dir).unwrap());

    // set up logging early
    setup_logging(&runtime_dir);

    // set up the tokio runtime
    TOKIO_RT
        .set(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap(),
        )
        .expect("failed to set tokio runtime");

    let tokio_rt = TOKIO_RT.get().unwrap();

    // run the sync init on the tokio runtime
    tokio_rt.block_on(init_sync_on_tokio(runtime_dir, dalamud_pipe_name));
}

async fn init_sync_on_tokio(runtime_dir: PathBuf, dalamud_pipe_name: Option<Vec<u8>>) {
    if let Some(pipe_name) = dalamud_pipe_name {
        DALAMUD_PIPE
            .set(DalamudPipe::new(std::str::from_utf8(&pipe_name).unwrap()))
            .expect("failed to set Dalamud pipe");
    }

    info!("--------------------------------------------------");
    info!(
        "Grebuloff Runtime starting (load method: {:?})",
        get_load_method()
    );
    info!("Build time: {}", env!("VERGEN_BUILD_TIMESTAMP"));
    info!("Git commit: {}", env!("VERGEN_GIT_DESCRIBE"));

    // start attempting connection to the Dalamud pipe, if applicable
    if let Some(pipe) = DALAMUD_PIPE.get() {
        task::spawn(pipe.connect());
    }

    // handle anything that needs to be loaded sync first
    init_core_runtime()
        .await
        .expect("failed to init core runtime");

    // call async init now
    task::spawn(init_async());
}

async fn init_async() -> Result<()> {
    info!("async init starting");

    task::spawn_blocking(|| {
        info!("webview2 init");
        let webview = WebView::new();
        webview.run().unwrap();
    })
    .await?;

    Ok(())
}
