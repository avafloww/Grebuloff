use super::engine::{Invocation, JsEngine, JsObject, JsResult, JsValue};

/// A JsCallable is the data structure used to represent a function in LLRT
/// that is callable from JavaScript. It contains the Rust function pointer,
/// and the function's name. Note that privileges are handled by the HLRT.
pub struct JsCallable {
    /// The name of the function, as it will appear in JavaScript.
    pub name: &'static str,

    /// The function pointer.
    pub func: fn(Invocation) -> JsResult<JsValue>,
}

impl JsCallable {
    pub const fn new(name: &'static str, func: fn(Invocation) -> JsResult<JsValue>) -> Self {
        Self { name, func }
    }
}

pub fn register_all(engine: &JsEngine, dest: &JsObject) -> JsResult<()> {
    for callable in inventory::iter::<JsCallable> {
        dest.set(callable.name, engine.create_function(callable.func))?;
    }

    Ok(())
}

inventory::collect!(JsCallable);

#[macro_export]
macro_rules! register_js_callable {
    ($name:expr, $func:ident) => {
        inventory::submit! {
            crate::runtime::callable::JsCallable::new(
                $name,
                $func
            )
        }
    };
}
