mod dalamud;
mod hooking;
mod resolvers;
// mod runtime;
mod ui;

#[macro_use]
extern crate retour;

use crate::{dalamud::DalamudPipe, ui::UiHost};
use anyhow::Result;
use log::{error, info, trace};
use msgbox::IconType;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::thread;
use tokio::{task, time};

static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static DALAMUD_PIPE: OnceLock<DalamudPipe> = OnceLock::new();
static EXEC_ID: OnceLock<String> = OnceLock::new();

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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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

pub fn get_execution_id() -> String {
    // EXEC_ID.get().unwrap().clone()
    "dev".to_owned()
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
        let backtrace = std::backtrace::Backtrace::force_capture();

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
                    "thread '{}' panicked at '{}': {}:{}\nbacktrace:\n{:?}",
                    thread,
                    msg,
                    location.file(),
                    location.line(),
                    backtrace
                )
            }
            None => format!(
                "thread '{}' panicked at '{}'\nbacktrace:\n{:?}",
                thread, msg, backtrace
            ),
        };

        error!("{}", formatted);
        log::logger().flush();

        msgbox::create("Grebuloff", &formatted, IconType::Error).unwrap();
    }));
}

fn init_sync(runtime_dir: Vec<u8>, dalamud_pipe_name: Option<Vec<u8>>) {
    // generate an execution ID used for pipe communication
    EXEC_ID
        .set(uuid::Uuid::new_v4().to_string())
        .expect("failed to set execution ID");

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

async fn init_sync_on_tokio(_runtime_dir: PathBuf, dalamud_pipe_name: Option<Vec<u8>>) {
    if let Some(pipe_name) = dalamud_pipe_name {
        DALAMUD_PIPE
            .set(DalamudPipe::new(std::str::from_utf8(&pipe_name).unwrap()))
            .expect("failed to set Dalamud pipe");
    }

    info!("--------------------------------------------------");
    info!(
        "Grebuloff Low-Level Runtime starting (load method: {:?}, execution ID: {})",
        get_load_method(),
        get_execution_id()
    );
    info!("Build time: {}", env!("BUILD_TIMESTAMP"));
    info!("Git commit: {}", env!("GIT_DESCRIBE"));

    // start attempting connection to the Dalamud pipe, if applicable
    if let Some(pipe) = DALAMUD_PIPE.get() {
        task::spawn(pipe.connect());
    }

    // handle anything that needs to be loaded sync first
    // resolve clientstructs
    unsafe { resolvers::init_resolvers(get_load_method()) }
        .await
        .expect("failed to init resolvers");

    // core hooks
    unsafe { hooking::init_hooks() }.expect("failed to init hooks");

    // core js runtime
    // runtime::init_hlrt(&runtime_dir)
    //     .await
    //     .expect("failed to init core runtime");

    // call async init now
    task::spawn(init_async());
}

async fn init_async() -> Result<()> {
    info!("async init starting");

    // temporarily disabled; webview2 has no clickthrough support as-is and we kinda need that
    // probably going to have to render offscreen and graphics capture/bitblt to the screen
    // but hey even that's hard! wv2 has no offscreen rendering API! :D
    // some useful links:
    // https://github.com/robmikh/screenshot-rs/tree/main - example code for win10 capture api in rust
    // https://github.com/jnschulze/flutter-webview-windows - example where this technique is used
    //
    // task::spawn_blocking(|| webview::init_ui_host().expect("UI host unexpectedly exited")).await?;
    task::spawn(async { UiHost::new().run().await });

    // run the main loop
    // this is the last thing that should be called in init_async
    let mut interval = time::interval(time::Duration::from_millis(1000));

    loop {
        interval.tick().await;
        trace!("in main loop");
        hooking::HookManager::instance().dump_hooks();
    }

    Ok(())
}

pub fn get_tokio_rt() -> &'static tokio::runtime::Runtime {
    TOKIO_RT.get().unwrap()
}
