use std::sync::Mutex;
use std::time::Duration;
use log::info;
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use tokio::time;
use windows::Win32::Foundation::ERROR_PIPE_BUSY;

#[derive(Debug)]
pub struct DalamudPipe {
    pipe_name: String,
    pipe_client: Mutex<Option<NamedPipeClient>>,
}

impl DalamudPipe {
    pub fn new(pipe_name: &str) -> Self {
        Self {
            pipe_name: pipe_name.to_owned(),
            pipe_client: Mutex::new(None),
        }
    }

    pub(crate) async fn connect(&self) {
        let pipe_name = self.pipe_name.to_owned();

        let client = loop {
            match ClientOptions::new().open(&pipe_name) {
                Ok(client) => break client,
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => (),
                Err(e) => panic!("failed to connect to Dalamud pipe: {}", e),
            }

            time::sleep(Duration::from_millis(50)).await;
        };

        self.pipe_client.lock().unwrap().replace(client);

        info!("connected to Dalamud pipe at {}", pipe_name);
    }
}