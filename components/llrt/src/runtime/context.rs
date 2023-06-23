use crate::get_tokio_rt;

use super::{
    callable,
    engine::{ContextId, JsEngine, JsResult, ModuleMap},
};
use anyhow::Result;
use grebuloff_macros::js_callable;
use log::info;
use std::{
    borrow::{BorrowMut, Cow},
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::PathBuf,
};
use std::{cell::RefCell, thread};
use std::{
    rc::Rc,
    sync::{Arc, Once, OnceLock},
};
use tokio::{
    sync::{mpsc, oneshot, RwLock},
    task,
};

static INIT: Once = Once::new();
static CONTEXTS: OnceLock<RwLock<HashMap<String, Arc<JsContext>>>> = OnceLock::new();

thread_local! {
    static THREAD_CONTEXT: RefCell<Option<Rc<JsThreadContext>>> = RefCell::new(None);
}

/// A thread-safe context container for a runtime thread.
/// This struct stores some basic context information and allows for
/// communication with the runtime thread via a MPSC channel.
#[derive(Debug)]
pub struct JsContext {
    id: ContextId,
    tx: mpsc::Sender<ContextMessage>,
}

impl JsContext {
    fn new(id: ContextId, base_path: PathBuf, tx: mpsc::Sender<ContextMessage>) -> Self {
        let module_map = ModuleMap::new(base_path);
        let thread_ctx = Rc::new(JsThreadContext {
            id: id.clone(),
            module_map: module_map,
        });

        THREAD_CONTEXT.with(|c| c.borrow_mut().replace(thread_ctx));

        Self { id: id, tx }
    }

    /// Creates a new context with the given options.
    pub async fn create(options: ContextOptions) -> Result<Arc<JsContext>> {
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
        contexts.insert(options.id.to_string(), ctx.clone());

        Ok(ctx.clone())
    }

    /// Gets the context with the given ID.
    pub async fn get_context(id: &str) -> Option<Arc<JsContext>> {
        let contexts = CONTEXTS.get().unwrap().read().await;
        contexts.get(id).cloned()
    }

    /// Gets the thread-local `JsThreadContext` for this context.
    /// Panics if no `JsThreadContext` is found on the calling thread, or if the
    /// `JsThreadContext` found does not belong to this `JsContext`.
    pub fn get_thread_local_context(&self) -> Rc<JsThreadContext> {
        let ctx = THREAD_CONTEXT.with(|c| {
            c.borrow()
                .clone()
                .expect("No JsThreadContext found for the current thread")
        });

        assert!(ctx.id == self.id);

        ctx
    }

    pub async fn execute_code(&self, code: &str) -> anyhow::Result<()> {
        self.tx
            .send(ContextMessage::ExecuteCode(code.to_owned()))
            .await?;
        Ok(())
    }
}

/// A thread-local context container for a `JsContext`.
/// As opposed to the thread-safe nature of `JsContext`, this struct
/// is not thread-safe and should only be used from the runtime thread
/// that owns the `JsContext`.
#[derive(Debug)]
pub struct JsThreadContext {
    pub id: ContextId,
    pub module_map: ModuleMap,
}

impl JsThreadContext {
    /// Gets the thread-local `JsThreadContext` for the current thread.
    /// Panics if no `JsThreadContext` is found on the calling thread.
    ///
    /// Note that this does not perform sanity checks to ensure that the
    /// `JsThreadContext` found belongs to a specific `JsContext`.
    /// If you need to ensure that the `JsThreadContext` belongs to a specific
    /// `JsContext`, use `JsContext::get_thread_local_context` instead.
    pub fn for_current_thread() -> Rc<JsThreadContext> {
        THREAD_CONTEXT.with(|c| {
            c.borrow()
                .clone()
                .expect("No JsThreadContext found for the current thread")
        })
    }

    /// Executes the function with a mutable reference to the current thread's
    /// `JsThreadContext`.
    pub fn with_current_thread<F, R>(func: F) -> R
    where
        F: Fn(&mut Rc<JsThreadContext>) -> R,
    {
        THREAD_CONTEXT.with(|c| func(c.borrow_mut().as_mut().unwrap()))
    }

    /// Gets the `JsContext` that this `JsThreadContext` is owned by.
    /// This operation is guaranteed to be non-blocking.
    pub async fn get_owning_context(&self) -> Arc<JsContext> {
        let contexts = CONTEXTS.get().unwrap().read().await;
        contexts.get(&self.id.0).unwrap().clone()
    }

    /// Gets the `JsContext` that this `JsThreadContext` is owned by.
    /// This operation may block.
    pub fn get_owning_context_sync(&self) -> Arc<JsContext> {
        let contexts = CONTEXTS.get().unwrap().blocking_read();
        contexts.get(&self.id.0).unwrap().clone()
    }
}

#[derive(Clone, Debug)]
pub struct ContextOptions {
    /// A unique identifier for this context.
    pub id: Cow<'static, str>,

    /// The base path for module searches.
    /// If `None`, the current working directory is used.
    pub module_base_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub enum ContextMessage {
    /// Executes the specified string of JavaScript code.
    ExecuteCode(String),

    /// Signals the runtime thread to terminate.
    Terminate,
}

fn runtime_thread(options: ContextOptions, context_tx: oneshot::Sender<Arc<JsContext>>) {
    get_tokio_rt().block_on(async move {
        info!("starting runtime thread for context: {:?}", options);

        let (tx, mut rx) = mpsc::channel(1);
        let id = ContextId(options.id.clone().to_string());
        let engine = JsEngine::new_with_context(Some(id.clone()));

        let ctx = Arc::new(JsContext::new(
            id.clone(),
            options
                .module_base_path
                .unwrap_or_else(|| std::env::current_dir().unwrap()),
            tx,
        ));
        register_globals(&engine).expect("failed to register globals");

        info!("runtime thread initialized for context: {:?}", id.clone());
        context_tx.send(ctx).unwrap();

        let local_set = task::LocalSet::new();
        local_set
            .run_until(async move {
                while let Some(msg) = rx.recv().await {
                    info!("runtime thread received message: {:?}", msg);
                    match msg {
                        ContextMessage::ExecuteCode(code) => {
                            info!("runtime thread executing code: {:?}", code);
                            engine.eval::<String, ()>(code).unwrap();
                        }
                        ContextMessage::Terminate => {
                            rx.close();
                            break;
                        }
                    }
                }
            })
            .await;

        info!("runtime thread stopping for context: {:?}", id.clone());
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
    info!("create_context: {:?}", name);
    JsContext::create(ContextOptions {
        id: Cow::from(name),
        module_base_path: None,
    })
    .await?;
    Ok(())
}
