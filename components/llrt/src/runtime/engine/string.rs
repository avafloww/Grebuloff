use super::JsEngine;
use std::fmt;
use std::string::String as StdString;

#[derive(Clone)]
pub struct JsString {
    pub engine: JsEngine,
    pub handle: v8::Global<v8::String>,
}

impl JsString {
    /// Returns a Rust string converted from the V8 string.
    pub fn to_string(&self) -> StdString {
        self.engine
            .scope(|scope| v8::Local::new(scope, self.handle.clone()).to_rust_string_lossy(scope))
    }
}

impl fmt::Debug for JsString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.to_string())
    }
}
