mod dalamud;
mod hooking;
mod resolvers;
mod rpc;
mod ui;

#[macro_use]
extern crate retour;
#[macro_use]
extern crate serde;

use crate::{
    dalamud::DalamudPipe,
    rpc::{ui::UiRpcServer, RpcServer},
};
use anyhow::Result;
use log::{debug, error, info};
use msgbox::IconType;
use std::sync::OnceLock;
use std::thread;
use std::{ffi::CString, path::PathBuf};
use tokio::task;

static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static DALAMUD_PIPE: OnceLock<DalamudPipe> = OnceLock::new();
static EXEC_ID: OnceLock<String> = OnceLock::new();
static RUNTIME_DIR: OnceLock<PathBuf> = OnceLock::new();
static LOAD_METHOD: OnceLock<GrebuloffLoadMethod> = OnceLock::new();

dll_syringe::payload_procedure! {
    fn init_injected(runtime_dir: CString) {
        init_sync_rt(&runtime_dir, None)
    }
}

dll_syringe::payload_procedure! {
    fn init_dalamud(runtime_dir: CString, dalamud_pipe_name: CString) {
        init_sync_rt(&runtime_dir, Some(&dalamud_pipe_name))
    }
}

#[no_mangle]
pub unsafe extern "system" fn init_loader(runtime_dir: &CString) {
    LOAD_METHOD.set(GrebuloffLoadMethod::Loader).unwrap();
    init_sync_rt(runtime_dir, None)
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GrebuloffLoadMethod {
    Loader,
    Injected,
    Dalamud,
}

impl GrebuloffLoadMethod {
    pub fn controls_its_own_destiny(self) -> bool {
        match self {
            GrebuloffLoadMethod::Dalamud => false,
            _ => true,
        }
    }
}

pub fn get_load_method() -> GrebuloffLoadMethod {
    *LOAD_METHOD.get().unwrap()
}

pub fn get_execution_id() -> String {
    EXEC_ID.get().unwrap().clone()
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

fn init_sync_rt(runtime_dir: &CString, dalamud_pipe_name: Option<&CString>) {
    if LOAD_METHOD.get().is_none() {
        LOAD_METHOD
            .set(if dalamud_pipe_name.is_some() {
                GrebuloffLoadMethod::Dalamud
            } else {
                GrebuloffLoadMethod::Injected
            })
            .unwrap();
    }

    // generate an execution ID used for pipe communication
    EXEC_ID
        .set(uuid::Uuid::new_v4().to_string())
        .expect("failed to set execution ID");

    let runtime_dir = PathBuf::from(std::str::from_utf8(runtime_dir.as_bytes()).unwrap());

    // set up logging early
    setup_logging(&runtime_dir);

    RUNTIME_DIR.set(runtime_dir).unwrap();

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
    tokio_rt.block_on(init_sync_early(
        dalamud_pipe_name
            .map(CString::as_bytes)
            .map(std::str::from_utf8)
            .transpose()
            .unwrap(),
    ));
}

async fn init_sync_early(dalamud_pipe_name: Option<&str>) {
    if let Some(pipe_name) = dalamud_pipe_name {
        DALAMUD_PIPE
            .set(DalamudPipe::new(&pipe_name))
            .expect("failed to set Dalamud pipe");
    }

    info!("--------------------------------------------------");
    info!(
        "Grebuloff Low-Level Runtime starting (load method: {:?})",
        get_load_method(),
    );
    info!("Build time: {}", env!("BUILD_TIMESTAMP"));
    info!("Git commit: {}", env!("GIT_DESCRIBE"));
    info!("Execution ID: {}", get_execution_id());

    // resolve clientstructs
    unsafe { resolvers::init_resolvers(get_load_method()) }
        .await
        .expect("failed to init resolvers");

    // initialize early hooks (framework)
    unsafe { hooking::init_early_hooks() }.expect("failed to init early hooks");

    match get_load_method() {
        GrebuloffLoadMethod::Loader => {
            // if we're loaded by the loader, we're loading very early
            // into the game's boot process. we need to wait for
            // Framework::Tick to be called by the game, so we just
            // return here and wait for the game to call us back
            debug!("waiting for framework tick before continuing init");
        }
        _ => {
            // if we're loaded by anything else, we're loading later
            // into the boot process, so we shouldn't wait - call
            // init_sync_late now
            init_sync_late().await;
        }
    }
}

pub async fn init_sync_late() {
    info!("late sync init starting");

    // start attempting connection to the Dalamud pipe, if applicable
    if let Some(pipe) = DALAMUD_PIPE.get() {
        task::spawn(pipe.connect());
    }

    // handle anything that needs to be loaded sync first
    // core hooks
    unsafe { hooking::init_hooks() }.expect("failed to init hooks");

    // call async init now
    task::spawn(init_async());
}

async fn init_async() -> Result<()> {
    info!("async init starting");

    // start RPC for the UI server
    task::spawn(async { UiRpcServer::instance().listen_forever().await });

    // start the UI server itself
    task::spawn(async move { ui::spawn_ui_host(RUNTIME_DIR.get().clone().unwrap()).await });

    // run the main loop
    // this is the last thing that should be called in init_async
    // let mut interval = time::interval(time::Duration::from_millis(1000));

    // loop {
    //     interval.tick().await;
    //     trace!("in main loop");
    //     hooking::HookManager::instance().dump_hooks();
    // }

    Ok(())
}

pub fn get_tokio_rt() -> &'static tokio::runtime::Runtime {
    TOKIO_RT.get().unwrap()
}
