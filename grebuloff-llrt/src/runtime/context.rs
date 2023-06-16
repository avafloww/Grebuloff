use crate::runtime::{Invocation, JsEngine};
use log::info;
use std::collections::HashMap;
use std::sync::{Arc, Once, OnceLock};
use std::thread;
use tokio::sync::{mpsc, oneshot, RwLock};

static INIT: Once = Once::new();
static CONTEXTS: OnceLock<RwLock<HashMap<String, Arc<JsEngineContext>>>> = OnceLock::new();

// store mpsc channel for sending messages to runtime thread
#[derive(Clone, Debug)]
pub struct JsEngineContext {
    tx: mpsc::Sender<ContextMessage>,
}

impl JsEngineContext {
    pub fn new(tx: mpsc::Sender<ContextMessage>) -> Self {
        Self { tx }
    }

    pub async fn execute(&self, code: &str) -> anyhow::Result<()> {
        self.tx
            .send(ContextMessage::Execute(code.to_owned()))
            .await?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ContextOptions {
    /// A unique identifier for this context.
    pub key: &'static str,

    /// Whether or not this context is the main context for the high-level runtime (HLRT).
    /// The HLRT context is considered privileged, and has access to more APIs than other contexts.
    pub(crate) is_main_context: bool,
}

#[derive(Clone, Debug)]
pub enum ContextMessage {
    /// Executes the specified string of JavaScript code.
    Execute(String),

    /// Signals the runtime thread to terminate.
    Terminate,
}

pub async fn spawn_context(options: ContextOptions) -> anyhow::Result<Arc<JsEngineContext>> {
    // initialize at first spawn (hlrt context) so we can guarantee non-blocking gets later
    INIT.call_once(|| {
        CONTEXTS.get_or_init(|| RwLock::new(HashMap::new()));
    });

    info!("spawning context with options: {:?}", options);

    let (tx, rx) = oneshot::channel::<Arc<JsEngineContext>>();
    let ctx_opts = options.clone();
    thread::spawn(move || runtime_thread(ctx_opts, tx));

    let ctx = rx.await?;

    // add the context to the global contexts map
    let mut contexts = CONTEXTS.get().unwrap().write().await;
    contexts.insert(options.key.to_string(), ctx.clone());

    Ok(ctx.clone())
}

pub async fn get_context(key: &str) -> anyhow::Result<Arc<JsEngineContext>> {
    let contexts = CONTEXTS.get().unwrap().read().await;
    let ctx = contexts.get(key).unwrap();

    Ok(ctx.clone())
}

pub fn get_context_sync(key: &str) -> anyhow::Result<Arc<JsEngineContext>> {
    let contexts = CONTEXTS.get().unwrap().blocking_read();
    let ctx = contexts.get(key).unwrap();

    Ok(ctx.clone())
}

fn runtime_thread(options: ContextOptions, context_tx: oneshot::Sender<Arc<JsEngineContext>>) {
    info!("starting runtime thread for context: {:?}", options);

    let (tx, mut rx) = mpsc::channel(1);
    let ctx = JsEngineContext::new(tx);

    let engine = JsEngine::new_with_key(options.key);
    add_base_api(&engine).expect("failed to setup base API");
    if options.is_main_context {
        add_privileged_api(&engine).expect("failed to setup privileged API");
    }

    info!("runtime thread initialized for context: {:?}", options);
    context_tx.send(Arc::new(ctx)).unwrap();

    while let Some(msg) = rx.blocking_recv() {
        info!("runtime thread received message: {:?}", msg);
        match msg {
            ContextMessage::Execute(code) => {
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

    info!("runtime thread stopping for context: {:?}", options);
}

fn add_base_api(engine: &JsEngine) -> crate::runtime::error::Result<()> {
    let global = engine.global();

    let console = engine.create_object();
    console.set("log", engine.create_function(handle_console))?;

    global.set("console", console)?;

    Ok(())
}

fn add_privileged_api(engine: &JsEngine) -> crate::runtime::error::Result<()> {
    let mut _global = engine.global();

    Ok(())
}

fn handle_console(inv: Invocation) -> crate::runtime::error::Result<()> {
    let msg = inv.args.from::<String>(&inv.engine, 0)?;

    info!("console.log: {:?}", msg);
    Ok(())
}
