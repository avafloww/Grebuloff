mod dalamud;

use std::path::PathBuf;
use std::sync::OnceLock;
use log::info;
use msgbox::IconType;
use crate::dalamud::DalamudPipe;

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
    if let Some(pipe) = DALAMUD_PIPE.get() {
        return GrebuloffLoadMethod::Dalamud;
    }

    GrebuloffLoadMethod::Native
}

fn alert(message: &str) {
    msgbox::create("Grebuloff", message, IconType::Info).unwrap();
}

fn setup_logging(dir: PathBuf) {
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
}

fn init_sync(runtime_dir: Vec<u8>, dalamud_pipe_name: Option<Vec<u8>>) {
    let runtime_dir = PathBuf::from(std::str::from_utf8(&runtime_dir).unwrap());

    // set up the tokio runtime and Dalamud pipe instance
    TOKIO_RT.set(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()).expect("failed to set tokio runtime");

    if let Some(pipe_name) = dalamud_pipe_name {
        DALAMUD_PIPE.set(
            DalamudPipe::new(&std::str::from_utf8(&pipe_name).unwrap())
        ).expect("failed to set Dalamud pipe");
    }

    setup_logging(runtime_dir);

    info!("--------------------------------------------------");
    info!("Grebuloff Framework starting (load method: {:?})", get_load_method());
    info!("Build time: {}", env!("VERGEN_BUILD_TIMESTAMP"));
    info!("Git commit: {}", env!("VERGEN_GIT_DESCRIBE"));

    let tokio_rt = TOKIO_RT.get().unwrap();

    // start attempting connection to the Dalamud pipe, if applicable
    if let Some(pipe) = DALAMUD_PIPE.get() {
        tokio_rt.spawn(pipe.connect());
    }

    // handle anything that needs to be loaded sync first

    // call async init now
    tokio_rt.spawn(init_async());
}

async fn init_async() {
    info!("async init starting");
}