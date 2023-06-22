use crate::get_tokio_rt;

use super::{
    callable,
    engine::{JsEngine, JsResult},
};
use anyhow::Result;
use grebuloff_macros::js_callable;
use log::info;
use std::sync::{Arc, Once, OnceLock};
use std::thread;
use std::{borrow::Cow, collections::HashMap};
use tokio::{
    sync::{mpsc, oneshot, RwLock},
    task,
};

static INIT: Once = Once::new();
static CONTEXTS: OnceLock<RwLock<HashMap<String, Arc<JsContext>>>> = OnceLock::new();

// store mpsc channel for sending messages to runtime thread
#[derive(Clone, Debug)]
pub struct JsContext {
    tx: mpsc::Sender<ContextMessage>,
}

impl JsContext {
    pub fn new(tx: mpsc::Sender<ContextMessage>) -> Self {
        Self { tx }
    }

    pub async fn execute_code(&self, code: &str) -> anyhow::Result<()> {
        self.tx
            .send(ContextMessage::ExecuteCode(code.to_owned()))
            .await?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ContextOptions {
    /// A unique identifier for this context.
    pub key: Cow<'static, str>,
}

#[derive(Clone, Debug)]
pub enum ContextMessage {
    /// Executes the specified string of JavaScript code.
    ExecuteCode(String),

    /// Signals the runtime thread to terminate.
    Terminate,
}

pub async fn spawn_context(options: ContextOptions) -> Result<Arc<JsContext>> {
    // initialize at first spawn (hlrt context) so we can guarantee non-blocking gets later
    INIT.call_once(|| {
        CONTEXTS.get_or_init(|| RwLock::new(HashMap::new()));
    });

    info!("spawning context with options: {:?}", options);

    let (tx, rx) = oneshot::channel::<Arc<JsContext>>();
    let ctx_opts = options.clone();
    thread::spawn(move || runtime_thread(ctx_opts, tx));

    let ctx = rx.await?;

    // add the context to the global contexts map
    let mut contexts = CONTEXTS.get().unwrap().write().await;
    contexts.insert(options.key.to_string(), ctx.clone());

    Ok(ctx.clone())
}

pub async fn get_context(key: &str) -> Result<Arc<JsContext>> {
    let contexts = CONTEXTS.get().unwrap().read().await;
    let ctx = contexts.get(key).unwrap();

    Ok(ctx.clone())
}

pub fn get_context_sync(key: &str) -> Result<Arc<JsContext>> {
    let contexts = CONTEXTS.get().unwrap().blocking_read();
    let ctx = contexts.get(key).unwrap();

    Ok(ctx.clone())
}

fn runtime_thread(options: ContextOptions, context_tx: oneshot::Sender<Arc<JsContext>>) {
    get_tokio_rt().block_on(async {
        info!("starting runtime thread for context: {:?}", options);

        let (tx, mut rx) = mpsc::channel(1);
        let ctx = JsContext::new(tx);

        let key = options.key.clone();
        let engine = JsEngine::new_with_key(key);
        register_globals(&engine).expect("failed to register globals");

        info!("runtime thread initialized for context: {:?}", options);
        context_tx.send(Arc::new(ctx)).unwrap();

        let local_set = task::LocalSet::new();
        local_set
            .run_until(async {
                while let Some(msg) = rx.recv().await {
                    info!("runtime thread received message: {:?}", msg);
                    match msg {
                        ContextMessage::ExecuteCode(code) => {
                            info!("runtime thread executing code: {:?}", code);
                            engine.eval::<String, ()>(code).unwrap();
                        }
                        ContextMessage::Terminate => {
                            info!("runtime thread terminating for context: {:?}", options);
                            rx.close();
                            break;
                        }
                    }
                }
            })
            .await;

        info!("runtime thread stopping for context: {:?}", options);
    });
}

fn register_globals(engine: &JsEngine) -> JsResult<()> {
    let global = engine.global();

    let grebuloff = engine.create_object();
    let llrt = engine.create_object();

    callable::register_all(engine, &llrt)?;

    grebuloff.set("LLRT", llrt)?;
    global.set("Grebuloff", grebuloff)?;

    Ok(())
}

#[js_callable]
fn print(msg: String) -> bool {
    info!("print: {:?}", msg);
    true
}

#[js_callable]
async fn create_context(name: String) -> Result<()> {
    // todo: remove, this is just for testing
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    info!("create_context: {:?}", name);
    spawn_context(ContextOptions {
        key: Cow::from(name),
    })
    .await?;
    Ok(())
}
