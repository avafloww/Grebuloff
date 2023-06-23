use super::*;
use crate::runtime::engine::*;
use std::iter::FromIterator;
use std::ops::{Deref, DerefMut};
use std::{fmt, slice, vec};

/// A JavaScript value.
///
/// `Value`s can either hold direct values (undefined, null, booleans, numbers, dates) or references
/// (strings, arrays, functions, other objects). Cloning values (via Rust's `Clone`) of the direct
/// types defers to Rust's `Copy`, while cloning values of the referential types results in a simple
/// reference clone similar to JavaScript's own "by-reference" semantics.
#[derive(Clone)]
pub enum JsValue {
    /// The JavaScript value `undefined`.
    Undefined,
    /// The JavaScript value `null`.
    Null,
    /// The JavaScript value `true` or `false`.
    Boolean(bool),
    /// A JavaScript floating point number.
    Number(f64),
    /// Elapsed milliseconds since Unix epoch.
    Date(f64),
    /// An immutable JavaScript string, managed by V8.
    String(JsString),
    /// Reference to a JavaScript array.
    Array(JsArray),
    /// Reference to a JavaScript function.
    Function(JsFunction),
    /// Reference to a JavaScript object. If a value is a function or an array in JavaScript, it
    /// will be converted to `Value::Array` or `Value::Function` instead of `Value::Object`.
    Object(JsObject),
    /// Reference to a JavaScript promise.
    Promise(JsPromise),
}

impl JsValue {
    /// Returns `true` if this is a `Value::Undefined`, `false` otherwise.
    pub fn is_undefined(&self) -> bool {
        if let JsValue::Undefined = *self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if this is a `Value::Null`, `false` otherwise.
    pub fn is_null(&self) -> bool {
        if let JsValue::Null = *self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if this is a `Value::Boolean`, `false` otherwise.
    pub fn is_boolean(&self) -> bool {
        if let JsValue::Boolean(_) = *self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if this is a `Value::Number`, `false` otherwise.
    pub fn is_number(&self) -> bool {
        if let JsValue::Number(_) = *self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if this is a `Value::Date`, `false` otherwise.
    pub fn is_date(&self) -> bool {
        if let JsValue::Date(_) = *self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if this is a `Value::String`, `false` otherwise.
    pub fn is_string(&self) -> bool {
        if let JsValue::String(_) = *self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if this is a `Value::Array`, `false` otherwise.
    pub fn is_array(&self) -> bool {
        if let JsValue::Array(_) = *self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if this is a `Value::Function`, `false` otherwise.
    pub fn is_function(&self) -> bool {
        if let JsValue::Function(_) = *self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if this is a `Value::Object`, `false` otherwise.
    pub fn is_object(&self) -> bool {
        if let JsValue::Object(_) = *self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if this is a `Value::Promise`, `false` otherwise.
    pub fn is_promise(&self) -> bool {
        if let JsValue::Promise(_) = *self {
            true
        } else {
            false
        }
    }

    /// Returns `Some(())` if this is a `Value::Undefined`, `None` otherwise.
    pub fn as_undefined(&self) -> Option<()> {
        if let JsValue::Undefined = *self {
            Some(())
        } else {
            None
        }
    }

    /// Returns `Some(())` if this is a `Value::Null`, `None` otherwise.
    pub fn as_null(&self) -> Option<()> {
        if let JsValue::Undefined = *self {
            Some(())
        } else {
            None
        }
    }

    /// Returns `Some` if this is a `Value::Boolean`, `None` otherwise.
    pub fn as_boolean(&self) -> Option<bool> {
        if let JsValue::Boolean(value) = *self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns `Some` if this is a `Value::Number`, `None` otherwise.
    pub fn as_number(&self) -> Option<f64> {
        if let JsValue::Number(value) = *self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns `Some` if this is a `Value::Date`, `None` otherwise.
    pub fn as_date(&self) -> Option<f64> {
        if let JsValue::Date(value) = *self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns `Some` if this is a `Value::String`, `None` otherwise.
    pub fn as_string(&self) -> Option<&JsString> {
        if let JsValue::String(ref value) = *self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns `Some` if this is a `Value::Array`, `None` otherwise.
    pub fn as_array(&self) -> Option<&JsArray> {
        if let JsValue::Array(ref value) = *self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns `Some` if this is a `Value::Function`, `None` otherwise.
    pub fn as_function(&self) -> Option<&JsFunction> {
        if let JsValue::Function(ref value) = *self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns `Some` if this is a `Value::Object`, `None` otherwise.
    pub fn as_object(&self) -> Option<&JsObject> {
        if let JsValue::Object(ref value) = *self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns `Some` if this is a `Value::Promise`, `None` otherwise.
    pub fn as_promise(&self) -> Option<&JsPromise> {
        if let JsValue::Promise(ref value) = *self {
            Some(value)
        } else {
            None
        }
    }

    /// A wrapper around `FromValue::from_value`.
    pub fn into<T: FromJsValue>(self, engine: &JsEngine) -> JsResult<T> {
        T::from_value(self, engine)
    }

    /// Coerces a value to a boolean. Returns `true` if the value is "truthy", `false` otherwise.
    pub fn coerce_boolean(&self, engine: &JsEngine) -> bool {
        match self {
            &JsValue::Boolean(b) => b,
            value => engine.scope(|scope| value.to_v8_value(scope).boolean_value(scope)),
        }
    }

    /// Coerces a value to a number. Nearly all JavaScript values are coercible to numbers, but this
    /// may fail with a runtime error under extraordinary circumstances (e.g. if the ECMAScript
    /// `ToNumber` implementation throws an error).
    ///
    /// This will return `std::f64::NAN` if the value has no numerical equivalent.
    pub fn coerce_number(&self, engine: &JsEngine) -> JsResult<f64> {
        match self {
            &JsValue::Number(n) => Ok(n),
            value => engine.try_catch(|scope| {
                let maybe = value.to_v8_value(scope).to_number(scope);
                engine.exception(scope).map(|_| maybe.unwrap().value())
            }),
        }
    }

    /// Coerces a value to a string. Nearly all JavaScript values are coercible to strings, but this
    /// may fail with a runtime error if `toString()` fails or under otherwise extraordinary
    /// circumstances (e.g. if the ECMAScript `ToString` implementation throws an error).
    pub fn coerce_string(&self, engine: &JsEngine) -> JsResult<JsString> {
        match self {
            &JsValue::String(ref s) => Ok(s.clone()),
            value => engine.try_catch(|scope| {
                let maybe = value.to_v8_value(scope).to_string(scope);
                engine.exception(scope).map(|_| JsString {
                    engine: engine.clone(),
                    handle: v8::Global::new(scope, maybe.unwrap()),
                })
            }),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match *self {
            JsValue::Undefined => "undefined",
            JsValue::Null => "null",
            JsValue::Boolean(_) => "boolean",
            JsValue::Number(_) => "number",
            JsValue::Date(_) => "date",
            JsValue::Function(_) => "function",
            JsValue::Array(_) => "array",
            JsValue::Object(_) => "object",
            JsValue::String(_) => "string",
            JsValue::Promise(_) => "promise",
        }
    }

    pub fn from_v8_value(
        engine: &JsEngine,
        scope: &mut v8::HandleScope,
        value: v8::Local<v8::Value>,
    ) -> JsValue {
        if value.is_undefined() {
            JsValue::Undefined
        } else if value.is_null() {
            JsValue::Null
        } else if value.is_boolean() {
            JsValue::Boolean(value.boolean_value(scope))
        } else if value.is_int32() {
            JsValue::Number(value.int32_value(scope).unwrap() as f64)
        } else if value.is_number() {
            JsValue::Number(value.number_value(scope).unwrap())
        } else if value.is_date() {
            let value: v8::Local<v8::Date> = value.try_into().unwrap();
            JsValue::Date(value.value_of())
        } else if value.is_string() {
            let value: v8::Local<v8::String> = value.try_into().unwrap();
            let handle = v8::Global::new(scope, value);
            JsValue::String(JsString {
                engine: engine.clone(),
                handle,
            })
        } else if value.is_array() {
            let value: v8::Local<v8::Array> = value.try_into().unwrap();
            let handle = v8::Global::new(scope, value);
            JsValue::Array(JsArray {
                engine: engine.clone(),
                handle,
            })
        } else if value.is_function() {
            let value: v8::Local<v8::Function> = value.try_into().unwrap();
            let handle = v8::Global::new(scope, value);
            JsValue::Function(JsFunction {
                engine: engine.clone(),
                handle,
            })
        } else if value.is_promise() {
            let value: v8::Local<v8::Promise> = value.try_into().unwrap();
            let handle = v8::Global::new(scope, value);
            JsValue::Promise(JsPromise {
                engine: engine.clone(),
                handle,
                resolver: None,
            })
        } else if value.is_object() {
            let value: v8::Local<v8::Object> = value.try_into().unwrap();
            let handle = v8::Global::new(scope, value);
            JsValue::Object(JsObject {
                engine: engine.clone(),
                handle,
            })
        } else {
            JsValue::Undefined
        }
    }

    pub fn to_v8_value<'s>(&self, scope: &mut v8::HandleScope<'s>) -> v8::Local<'s, v8::Value> {
        match self {
            JsValue::Undefined => v8::undefined(scope).into(),
            JsValue::Null => v8::null(scope).into(),
            JsValue::Boolean(v) => v8::Boolean::new(scope, *v).into(),
            JsValue::Number(v) => v8::Number::new(scope, *v).into(),
            JsValue::Date(v) => v8::Date::new(scope, *v).unwrap().into(),
            JsValue::Function(v) => v8::Local::new(scope, v.handle.clone()).into(),
            JsValue::Promise(v) => v8::Local::new(scope, v.handle.clone()).into(),
            JsValue::Array(v) => v8::Local::new(scope, v.handle.clone()).into(),
            JsValue::Object(v) => v8::Local::new(scope, v.handle.clone()).into(),
            JsValue::String(v) => v8::Local::new(scope, v.handle.clone()).into(),
        }
    }
}

impl fmt::Debug for JsValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JsValue::Undefined => write!(f, "undefined"),
            JsValue::Null => write!(f, "null"),
            JsValue::Boolean(b) => write!(f, "{:?}", b),
            JsValue::Number(n) => write!(f, "{}", n),
            JsValue::Date(d) => write!(f, "date:{}", d),
            JsValue::String(s) => write!(f, "{:?}", s),
            JsValue::Promise(p) => write!(f, "{:?}", p),
            JsValue::Array(a) => write!(f, "{:?}", a),
            JsValue::Function(u) => write!(f, "{:?}", u),
            JsValue::Object(o) => write!(f, "{:?}", o),
        }
    }
}

/// Trait for types convertible to `JsValue`.
pub trait ToJsValue {
    /// Performs the conversion.
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue>;
}

/// Trait for types convertible from `JsValue`.
pub trait FromJsValue: Sized {
    /// Performs the conversion.
    fn from_value(value: JsValue, engine: &JsEngine) -> JsResult<Self>;
}

/// A collection of multiple JavaScript values used for interacting with function arguments.
#[derive(Clone)]
pub struct JsValues(Vec<JsValue>);

impl JsValues {
    /// Creates an empty `Values`.
    pub fn new() -> JsValues {
        JsValues(Vec::new())
    }

    pub fn from_vec(vec: Vec<JsValue>) -> JsValues {
        JsValues(vec)
    }

    pub fn into_vec(self) -> Vec<JsValue> {
        self.0
    }

    pub fn get(&self, index: usize) -> JsValue {
        self.0
            .get(index)
            .map(Clone::clone)
            .unwrap_or(JsValue::Undefined)
    }

    pub fn from<T: FromJsValue>(&self, engine: &JsEngine, index: usize) -> JsResult<T> {
        T::from_value(
            self.0
                .get(index)
                .map(Clone::clone)
                .unwrap_or(JsValue::Undefined),
            engine,
        )
    }

    pub fn into<T: FromJsValues>(self, engine: &JsEngine) -> JsResult<T> {
        T::from_values(self, engine)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &JsValue> {
        self.0.iter()
    }
}

impl FromIterator<JsValue> for JsValues {
    fn from_iter<I: IntoIterator<Item = JsValue>>(iter: I) -> Self {
        JsValues::from_vec(Vec::from_iter(iter))
    }
}

impl IntoIterator for JsValues {
    type Item = JsValue;
    type IntoIter = vec::IntoIter<JsValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a JsValues {
    type Item = &'a JsValue;
    type IntoIter = slice::Iter<'a, JsValue>;

    fn into_iter(self) -> Self::IntoIter {
        (&self.0).into_iter()
    }
}

/// Trait for types convertible to any number of JavaScript values.
///
/// This is a generalization of `ToValue`, allowing any number of resulting JavaScript values
/// instead of just one. Any type that implements `ToValue` will automatically implement this trait.
pub trait ToJsValues {
    /// Performs the conversion.
    fn to_values(self, engine: &JsEngine) -> JsResult<JsValues>;
}

/// Trait for types that can be created from an arbitrary number of JavaScript values.
///
/// This is a generalization of `FromValue`, allowing an arbitrary number of JavaScript values to
/// participate in the conversion. Any type that implements `FromValue` will automatically implement
/// this trait.
pub trait FromJsValues: Sized {
    /// Performs the conversion.
    ///
    /// In case `values` contains more values than needed to perform the conversion, the excess
    /// values should be ignored. Similarly, if not enough values are given, conversions should
    /// assume that any missing values are undefined.
    fn from_values(values: JsValues, engine: &JsEngine) -> JsResult<Self>;
}

/// Wraps a variable number of `T`s.
///
/// Can be used to work with variadic functions more easily. Using this type as the last argument of
/// a Rust callback will accept any number of arguments from JavaScript and convert them to the type
/// `T` using [`FromValue`]. `Variadic<T>` can also be returned from a callback, returning a
/// variable number of values to JavaScript.
#[derive(Clone)]
pub struct Variadic<T>(pub Vec<T>);

impl<T> Variadic<T> {
    /// Creates an empty `Variadic` wrapper containing no values.
    pub fn new() -> Variadic<T> {
        Variadic(Vec::new())
    }

    pub fn from_vec(vec: Vec<T>) -> Variadic<T> {
        Variadic(vec)
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<T> FromIterator<T> for Variadic<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Variadic(Vec::from_iter(iter))
    }
}

impl<T> IntoIterator for Variadic<T> {
    type Item = T;
    type IntoIter = <Vec<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T> Deref for Variadic<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Variadic<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coerce_boolean() {
        let engine = JsEngine::new();
        assert!(!JsValue::Undefined.coerce_boolean(&engine));
        assert!(!JsValue::Null.coerce_boolean(&engine));
        assert!(!JsValue::Number(0.0).coerce_boolean(&engine));
        assert!(JsValue::Number(1.0).coerce_boolean(&engine));
        assert!(!JsValue::String(engine.create_string("")).coerce_boolean(&engine));
        assert!(JsValue::String(engine.create_string("a")).coerce_boolean(&engine));
        assert!(JsValue::Object(engine.create_object()).coerce_boolean(&engine));
    }

    #[test]
    fn coerce_number() {
        let engine = JsEngine::new();
        assert!(JsValue::Undefined.coerce_number(&engine).unwrap().is_nan());
        assert_eq!(0.0, JsValue::Null.coerce_number(&engine).unwrap());
        assert_eq!(0.0, JsValue::Number(0.0).coerce_number(&engine).unwrap());
        assert_eq!(1.0, JsValue::Number(1.0).coerce_number(&engine).unwrap());
        assert_eq!(
            0.0,
            JsValue::String(engine.create_string(""))
                .coerce_number(&engine)
                .unwrap()
        );
        assert!(JsValue::String(engine.create_string("a"))
            .coerce_number(&engine)
            .unwrap()
            .is_nan());
        assert!(JsValue::Object(engine.create_object())
            .coerce_number(&engine)
            .unwrap()
            .is_nan());
    }

    #[test]
    fn coerce_string() {
        fn assert_string_eq(engine: &JsEngine, value: JsValue, expected: &str) {
            assert_eq!(expected, value.coerce_string(engine).unwrap().to_string());
        }

        let engine = JsEngine::new();
        assert_string_eq(&engine, JsValue::Undefined, "undefined");
        assert_string_eq(&engine, JsValue::Null, "null");
        assert_string_eq(&engine, JsValue::Number(123.0), "123");
        assert_string_eq(&engine, JsValue::String(engine.create_string("abc")), "abc");
        assert_string_eq(
            &engine,
            JsValue::Object(engine.create_object()),
            "[object Object]",
        );
    }
}
