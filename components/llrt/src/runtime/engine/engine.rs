// largely copied from MiniV8: mini_v8.rs
use super::types::*;
use super::*;
use std::rc::Rc;
use std::string::String as StdString;
use std::sync::{Arc, Condvar, Mutex, Once};
use std::thread;
use std::time::Duration;
use std::{cell::RefCell, fmt};

const CONTEXT_ID_STR: &str = "id";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextId(pub String);

impl fmt::Display for ContextId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone)]
pub struct JsEngine {
    interface: Interface,
}

impl JsEngine {
    pub fn new() -> Self {
        Self::new_with_context(None)
    }

    pub fn new_with_context(id: Option<ContextId>) -> Self {
        init_v8();
        let mut isolate = v8::Isolate::new(Default::default());
        init_isolate(&mut isolate, id);

        Self {
            interface: Interface::new(isolate),
        }
    }

    /// Returns the global JavaScript object.
    pub fn global(&self) -> JsObject {
        self.scope(|scope| {
            let global = scope.get_current_context().global(scope);
            JsObject {
                engine: self.clone(),
                handle: v8::Global::new(scope, global),
            }
        })
    }

    /// Executes a JavaScript script and returns its result.
    pub fn eval<S, R>(&self, script: S) -> JsResult<R>
    where
        S: Into<Script>,
        R: FromJsValue,
    {
        let script = script.into();
        let isolate_handle = self.interface.isolate_handle();
        match (self.interface.len() == 1, script.timeout) {
            (true, Some(timeout)) => execute_with_timeout(
                timeout,
                || self.eval_inner(script),
                move || {
                    isolate_handle.terminate_execution();
                },
            )?
            .into(self),
            (false, Some(_)) => Err(JsError::InvalidTimeout),
            (_, None) => self.eval_inner(script)?.into(self),
        }
    }

    fn eval_inner(&self, script: Script) -> JsResult<JsValue> {
        self.try_catch(|scope| {
            let source = create_string(scope, &script.source);
            let origin = script.origin.map(|o| {
                let name = create_string(scope, &o.name).into();
                let source_map_url = create_string(scope, "").into();
                v8::ScriptOrigin::new(
                    scope,
                    name,
                    o.line_offset,
                    o.column_offset,
                    false,
                    0,
                    source_map_url,
                    true,
                    false,
                    false,
                )
            });
            let script = v8::Script::compile(scope, source, origin.as_ref());
            self.exception(scope)?;
            let result = script.unwrap().run(scope);
            self.exception(scope)?;
            Ok(JsValue::from_v8_value(self, scope, result.unwrap()))
        })
    }

    /// Gets the context ID that was set when the `JsEngine` was created.
    /// Returns `None` if no ID was set.
    pub fn get_context_id(&self) -> Option<ContextId> {
        self.interface
            .top(|entry| entry.get_slot::<ContextId>().cloned())
    }

    /// Creates and returns a string managed by V8.
    ///
    /// # Panics
    ///
    /// Panics if source value is longer than `(1 << 28) - 16` bytes.
    pub fn create_string(&self, value: &str) -> JsString {
        self.scope(|scope| {
            let string = create_string(scope, value);
            JsString {
                engine: self.clone(),
                handle: v8::Global::new(scope, string),
            }
        })
    }

    /// Creates and returns an empty `Array` managed by V8.
    pub fn create_array(&self) -> JsArray {
        self.scope(|scope| {
            let array = v8::Array::new(scope, 0);
            JsArray {
                engine: self.clone(),
                handle: v8::Global::new(scope, array),
            }
        })
    }

    /// Creates and returns an empty `Object` managed by V8.
    pub fn create_object(&self) -> JsObject {
        self.scope(|scope| {
            let object = v8::Object::new(scope);
            JsObject {
                engine: self.clone(),
                handle: v8::Global::new(scope, object),
            }
        })
    }

    /// Creates and returns a pending `Promise` managed by V8.
    pub fn create_promise(&self) -> JsPromise {
        self.scope(|scope| {
            let resolver = v8::PromiseResolver::new(scope).unwrap();
            let promise = resolver.get_promise(scope);
            JsPromise {
                engine: self.clone(),
                handle: v8::Global::new(scope, promise),
                resolver: Some(v8::Global::new(scope, resolver)),
            }
        })
    }

    /// Creates and returns an `Object` managed by V8 filled with the keys and values from an
    /// iterator. Keys are coerced to object properties.
    ///
    /// This is a thin wrapper around `JsEngine::create_object` and `Object::set`. See `Object::set`
    /// for how this method might return an error.
    pub fn create_object_from<K, V, I>(&self, iter: I) -> JsResult<JsObject>
    where
        K: ToJsValue,
        V: ToJsValue,
        I: IntoIterator<Item = (K, V)>,
    {
        let object = self.create_object();
        for (k, v) in iter {
            object.set(k, v)?;
        }
        Ok(object)
    }

    /// Wraps a Rust function or closure, creating a callable JavaScript function handle to it.
    ///
    /// The function's return value is always a `Result`: If the function returns `Err`, the error
    /// is raised as a JavaScript exception, which can be caught within JavaScript or bubbled up
    /// back into Rust by not catching it. This allows using the `?` operator to propagate errors
    /// through intermediate JavaScript code.
    ///
    /// If the function returns `Ok`, the contained value will be converted to a JavaScript value.
    /// For details on Rust-to-JavaScript conversions, refer to the `ToValue` and `ToValues` traits.
    ///
    /// If the provided function panics, the executable will be aborted.
    pub fn create_function<F, R>(&self, func: F) -> JsFunction
    where
        F: Fn(Invocation) -> JsResult<R> + 'static,
        R: ToJsValue,
    {
        let func = move |engine: &JsEngine, this: JsValue, args: JsValues| {
            func(Invocation {
                engine: engine.clone(),
                this,
                args,
            })?
            .to_value(engine)
        };

        self.scope(|scope| {
            let callback = Box::new(func);
            let callback_info = CallbackInfo {
                engine: self.clone(),
                callback,
            };
            let ptr = Box::into_raw(Box::new(callback_info));
            let ext = v8::External::new(scope, ptr as _);

            let v8_func = |scope: &mut v8::HandleScope,
                           fca: v8::FunctionCallbackArguments,
                           mut rv: v8::ReturnValue| {
                let data = fca.data();
                let ext = v8::Local::<v8::External>::try_from(data).unwrap();
                let callback_info_ptr = ext.value() as *mut CallbackInfo;
                let callback_info = unsafe { &mut *callback_info_ptr };
                let CallbackInfo { engine, callback } = callback_info;
                let ptr = scope as *mut v8::HandleScope;
                // We can erase the lifetime of the `v8::HandleScope` safely because it only lives
                // on the interface stack during the current block:
                let ptr: *mut v8::HandleScope<'static> = unsafe { std::mem::transmute(ptr) };
                engine.interface.push(ptr);
                let this = JsValue::from_v8_value(&engine, scope, fca.this().into());
                let len = fca.length();
                let mut args = Vec::with_capacity(len as usize);
                for i in 0..len {
                    args.push(JsValue::from_v8_value(&engine, scope, fca.get(i)));
                }
                match callback(&engine, this, JsValues::from_vec(args)) {
                    Ok(v) => {
                        rv.set(v.to_v8_value(scope));
                    }
                    Err(e) => {
                        let exception = e.to_value(&engine).to_v8_value(scope);
                        scope.throw_exception(exception);
                    }
                };
                engine.interface.pop();
            };

            let value = v8::Function::builder(v8_func)
                .data(ext.into())
                .build(scope)
                .unwrap();
            // TODO: `v8::Isolate::adjust_amount_of_external_allocated_memory` should be called
            // appropriately with the following external resource size calculation. This cannot be
            // done as of now, since `v8::Weak::with_guaranteed_finalizer` does not provide a
            // `v8::Isolate` to the finalizer callback, and so the downward adjustment cannot be
            // made.
            //
            // let func_size = mem::size_of_val(&func); let ext_size = func_size +
            // mem::size_of::<CallbackInfo>;
            let drop_ext = Box::new(move || drop(unsafe { Box::from_raw(ptr) }));
            add_finalizer(scope, value, drop_ext);
            JsFunction {
                engine: self.clone(),
                handle: v8::Global::new(scope, value),
            }
        })
    }

    /// Wraps a mutable Rust closure, creating a callable JavaScript function handle to it.
    ///
    /// This is a version of `create_function` that accepts a FnMut argument. Refer to
    /// `create_function` for more information about the implementation.
    pub fn create_function_mut<F, R>(&self, func: F) -> JsFunction
    where
        F: FnMut(Invocation) -> JsResult<R> + 'static,
        R: ToJsValue,
    {
        let func = RefCell::new(func);
        self.create_function(move |invocation| {
            (&mut *func
                .try_borrow_mut()
                .map_err(|_| JsError::RecursiveMutCallback)?)(invocation)
        })
    }

    // Opens a new handle scope in the global context. Nesting calls to this or `JsEngine::try_catch`
    // will cause a panic (unless a callback is entered, see `JsEngine::create_function`).
    pub fn scope<F, T>(&self, func: F) -> T
    where
        F: FnOnce(&mut v8::ContextScope<v8::HandleScope>) -> T,
    {
        self.interface.scope(func)
    }

    // Opens a new try-catch scope in the global context. Nesting calls to this or `JsEngine::scope`
    // will cause a panic (unless a callback is entered, see `JsEngine::create_function`).
    pub fn try_catch<F, T>(&self, func: F) -> T
    where
        F: FnOnce(&mut v8::TryCatch<v8::HandleScope>) -> T,
    {
        self.interface.try_catch(func)
    }

    pub fn exception(&self, scope: &mut v8::TryCatch<v8::HandleScope>) -> JsResult<()> {
        if scope.has_terminated() {
            Err(JsError::Timeout)
        } else if let Some(exception) = scope.exception() {
            Err(JsError::Value(JsValue::from_v8_value(
                self, scope, exception,
            )))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
struct Interface(Rc<RefCell<Vec<Rc<RefCell<InterfaceEntry>>>>>);

impl Interface {
    fn len(&self) -> usize {
        self.0.borrow().len()
    }

    fn isolate_handle(&self) -> v8::IsolateHandle {
        self.top(|entry| entry.isolate_handle())
    }

    // Opens a new handle scope in the global context.
    fn scope<F, T>(&self, func: F) -> T
    where
        F: FnOnce(&mut v8::ContextScope<v8::HandleScope>) -> T,
    {
        self.top(|entry| entry.scope(func))
    }

    // Opens a new try-catch scope in the global context.
    fn try_catch<F, T>(&self, func: F) -> T
    where
        F: FnOnce(&mut v8::TryCatch<v8::HandleScope>) -> T,
    {
        self.scope(|scope| func(&mut v8::TryCatch::new(scope)))
    }

    fn new(isolate: v8::OwnedIsolate) -> Interface {
        Interface(Rc::new(RefCell::new(vec![Rc::new(RefCell::new(
            InterfaceEntry::Isolate(isolate),
        ))])))
    }

    fn push(&self, handle_scope: *mut v8::HandleScope<'static>) {
        self.0
            .borrow_mut()
            .push(Rc::new(RefCell::new(InterfaceEntry::HandleScope(
                handle_scope,
            ))));
    }

    fn pop(&self) {
        self.0.borrow_mut().pop();
    }

    fn top<F, T>(&self, func: F) -> T
    where
        F: FnOnce(&mut InterfaceEntry) -> T,
    {
        let top = self.0.borrow().last().unwrap().clone();
        let mut top_mut = top.borrow_mut();
        func(&mut top_mut)
    }
}

enum InterfaceEntry {
    Isolate(v8::OwnedIsolate),
    HandleScope(*mut v8::HandleScope<'static>),
}

impl InterfaceEntry {
    fn scope<F, T>(&mut self, func: F) -> T
    where
        F: FnOnce(&mut v8::ContextScope<v8::HandleScope>) -> T,
    {
        match self {
            InterfaceEntry::Isolate(isolate) => {
                let global_context = isolate.get_slot::<Global>().unwrap().context.clone();
                let scope = &mut v8::HandleScope::new(isolate);
                let context = v8::Local::new(scope, global_context);
                let scope = &mut v8::ContextScope::new(scope, context);
                func(scope)
            }
            InterfaceEntry::HandleScope(ref ptr) => {
                let scope: &mut v8::HandleScope = unsafe { &mut **ptr };
                let scope = &mut v8::ContextScope::new(scope, scope.get_current_context());
                func(scope)
            }
        }
    }

    fn get_slot<T: 'static>(&self) -> Option<&T> {
        match self {
            InterfaceEntry::Isolate(isolate) => isolate.get_slot::<T>(),
            InterfaceEntry::HandleScope(ref ptr) => {
                let scope: &mut v8::HandleScope = unsafe { &mut **ptr };
                scope.get_slot::<T>()
            }
        }
    }

    fn isolate_handle(&self) -> v8::IsolateHandle {
        match self {
            InterfaceEntry::Isolate(isolate) => isolate.thread_safe_handle(),
            InterfaceEntry::HandleScope(ref ptr) => {
                let scope: &mut v8::HandleScope = unsafe { &mut **ptr };
                scope.thread_safe_handle()
            }
        }
    }
}

struct Global {
    context: v8::Global<v8::Context>,
}

static V8_INIT: Once = Once::new();

fn init_v8() {
    V8_INIT.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    })
}

fn init_isolate(isolate: &mut v8::Isolate, context_id: Option<ContextId>) {
    bindings::setup_bindings(isolate);

    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope);
    let scope = &mut v8::ContextScope::new(scope, context);
    let global_context = v8::Global::new(scope, context);
    scope.set_slot::<Global>(Global {
        context: global_context,
    });

    if let Some(id) = context_id {
        scope.set_slot::<ContextId>(id);
    }
}

fn create_string<'s>(scope: &mut v8::HandleScope<'s>, value: &str) -> v8::Local<'s, v8::String> {
    v8::String::new(scope, value).expect("string exceeds maximum length")
}

fn add_finalizer<T: 'static>(
    isolate: &mut v8::Isolate,
    handle: impl v8::Handle<Data = T>,
    finalizer: impl FnOnce() + 'static,
) {
    let rc = Rc::new(RefCell::new(None));
    let weak = v8::Weak::with_guaranteed_finalizer(
        isolate,
        handle,
        Box::new({
            let rc = rc.clone();
            move || {
                let weak = rc.replace(None).unwrap();
                finalizer();
                drop(weak);
            }
        }),
    );
    rc.replace(Some(weak));
}

type Callback = Box<dyn Fn(&JsEngine, JsValue, JsValues) -> JsResult<JsValue>>;

struct CallbackInfo {
    engine: JsEngine,
    callback: Callback,
}

// A JavaScript script.
#[derive(Clone, Debug, Default)]
pub struct Script {
    /// The source of the script.
    pub source: StdString,
    /// The maximum runtime duration of the script's execution. This cannot be set within a nested
    /// evaluation, i.e. it cannot be set when calling `JsEngine::eval` from within a `Function`
    /// created with `JsEngine::create_function` or `JsEngine::create_function_mut`.
    ///
    /// V8 can only cancel script evaluation while running actual JavaScript code. If Rust code is
    /// being executed when the timeout is triggered, the execution will continue until the
    /// evaluation has returned to running JavaScript code.
    pub timeout: Option<Duration>,
    /// The script's origin.
    pub origin: Option<ScriptOrigin>,
}

/// The origin, within a file, of a JavaScript script.
#[derive(Clone, Debug, Default)]
pub struct ScriptOrigin {
    /// The name of the file this script belongs to.
    pub name: StdString,
    /// The line at which this script starts.
    pub line_offset: i32,
    /// The column at which this script starts.
    pub column_offset: i32,
}

impl From<StdString> for Script {
    fn from(source: StdString) -> Script {
        Script {
            source,
            ..Default::default()
        }
    }
}

impl<'a> From<&'a str> for Script {
    fn from(source: &'a str) -> Script {
        source.to_string().into()
    }
}

fn execute_with_timeout<T>(
    timeout: Duration,
    execute_fn: impl FnOnce() -> T,
    timed_out_fn: impl FnOnce() + Send + 'static,
) -> T {
    let wait = Arc::new((Mutex::new(true), Condvar::new()));
    let timer_wait = wait.clone();
    thread::spawn(move || {
        let (mutex, condvar) = &*timer_wait;
        let timer = condvar
            .wait_timeout_while(mutex.lock().unwrap(), timeout, |&mut is_executing| {
                is_executing
            })
            .unwrap();
        if timer.1.timed_out() {
            timed_out_fn();
        }
    });

    let result = execute_fn();
    let (mutex, condvar) = &*wait;
    *mutex.lock().unwrap() = false;
    condvar.notify_one();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::String as StdString;
    use std::time::Duration;

    #[test]
    fn eval_origin() {
        let engine = JsEngine::new();
        let result: StdString = engine
            .eval(Script {
                source: "try { MISSING_VAR } catch (e) { e.stack }".to_owned(),
                origin: Some(ScriptOrigin {
                    name: "eval_origin".to_owned(),
                    line_offset: 123,
                    column_offset: 456,
                }),
                ..Default::default()
            })
            .unwrap();
        let result = result.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(
            "ReferenceError: MISSING_VAR is not defined at eval_origin:124:463",
            result
        );
    }

    #[test]
    fn eval_timeout() {
        let engine = JsEngine::new();
        let result = engine.eval::<_, JsValue>(Script {
            source: "a = 0; while (true) { a++; }".to_owned(),
            timeout: Some(Duration::from_millis(50)),
            ..Default::default()
        });

        match result {
            Err(JsError::Timeout) => {}
            _ => panic!("unexpected result: {:?}", result),
        }

        // Make sure we can still evaluate again:
        let a: f64 = engine.eval("a").unwrap();
        assert!(a > 0.0);
    }

    #[test]
    fn eval_wasm() {
        let engine = JsEngine::new();
        let result = engine.eval::<_, JsValue>(
            r#"
        let bytes = new Uint8Array([
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f,
            0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64,
            0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b
        ]);

        let module = new WebAssembly.Module(bytes);
        let instance = new WebAssembly.Instance(module);
        instance.exports.add(3, 4)
    "#,
        );

        match result {
            Ok(JsValue::Number(n)) if n == 7.0 => {}
            _ => panic!("unexpected result: {:?}", result),
        }
    }

    #[test]
    #[should_panic(expected = "attempt to use Handle in an Isolate that is not its host")]
    fn value_cross_contamination() {
        let engine_1 = JsEngine::new();
        let str_1 = engine_1.create_string("123");
        let engine_2 = JsEngine::new();
        let _str_2 = engine_2.create_string("456");
        let _ = JsValue::String(str_1).coerce_number(&engine_2);
    }
}
