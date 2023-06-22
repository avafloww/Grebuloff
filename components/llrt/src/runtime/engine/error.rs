use super::*;
use std::error::Error as StdError;
use std::fmt;
use std::result::Result as StdResult;

pub type JsResult<T> = StdResult<T, JsError>;

/// An error originating from `JsEngine` usage.
#[derive(Debug)]
pub enum JsError {
    /// A Rust value could not be converted to a JavaScript value.
    ToJsConversionError {
        /// Name of the Rust type that could not be converted.
        from: &'static str,
        /// Name of the JavaScript type that could not be created.
        to: &'static str,
    },
    /// A JavaScript value could not be converted to the expected Rust type.
    FromJsConversionError {
        /// Name of the JavaScript type that could not be converted.
        from: &'static str,
        /// Name of the Rust type that could not be created.
        to: &'static str,
    },
    /// An evaluation timeout occurred.
    Timeout,
    /// A mutable callback has triggered JavaScript code that has called the same mutable callback
    /// again.
    ///
    /// This is an error because a mutable callback can only be borrowed mutably once.
    RecursiveMutCallback,
    /// An evaluation timeout was specified from within a Rust function embedded in V8.
    InvalidTimeout,
    /// An attempt was made to resolve or reject a promise without a resolver.
    PromiseWithoutResolver,
    /// A custom error that occurs during runtime.
    ///
    /// This can be used for returning user-defined errors from callbacks.
    ExternalError(anyhow::Error),
    /// An exception that occurred within the JavaScript environment.
    Value(JsValue),
}

impl JsError {
    /// Normalizes an error into a JavaScript value.
    pub fn to_value(self, engine: &JsEngine) -> JsValue {
        match self {
            JsError::Value(value) => value,
            JsError::ToJsConversionError { .. } | JsError::FromJsConversionError { .. } => {
                let object = engine.create_object();
                let _ = object.set("name", "TypeError");
                let _ = object.set("message", self.to_string());
                JsValue::Object(object)
            }
            _ => {
                let object = engine.create_object();
                let _ = object.set("name", "Error");
                let _ = object.set("message", self.to_string());
                JsValue::Object(object)
            }
        }
    }

    pub(crate) fn from_js_conversion(from: &'static str, to: &'static str) -> JsError {
        JsError::FromJsConversionError { from, to }
    }
}

impl StdError for JsError {
    fn description(&self) -> &'static str {
        "JavaScript execution error"
    }
}

impl fmt::Display for JsError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JsError::ToJsConversionError { from, to } => {
                write!(fmt, "error converting {} to JavaScript {}", from, to)
            }
            JsError::FromJsConversionError { from, to } => {
                write!(fmt, "error converting JavaScript {} to {}", from, to)
            }
            JsError::Timeout => write!(fmt, "evaluation timed out"),
            JsError::RecursiveMutCallback => write!(fmt, "mutable callback called recursively"),
            JsError::InvalidTimeout => write!(fmt, "invalid request for evaluation timeout"),
            JsError::PromiseWithoutResolver => write!(fmt, "promise without resolver"),
            JsError::ExternalError(ref err) => err.fmt(fmt),
            JsError::Value(v) => write!(fmt, "JavaScript runtime error ({})", v.type_name()),
        }
    }
}
