use crate::runtime::*;
use std::fmt;

#[derive(Clone)]
pub struct Function {
    pub(crate) engine: JsEngine,
    pub(crate) handle: v8::Global<v8::Function>,
}

impl Function {
    /// Consumes the function and downgrades it to a JavaScript object.
    pub fn into_object(self) -> Object {
        self.engine.clone().scope(|scope| {
            let object: v8::Local<v8::Object> = v8::Local::new(scope, self.handle.clone()).into();
            Object {
                engine: self.engine,
                handle: v8::Global::new(scope, object),
            }
        })
    }

    /// Calls the function with the given arguments, with `this` set to `undefined`.
    pub fn call<A, R>(&self, args: A) -> Result<R>
    where
        A: ToValues,
        R: FromValue,
    {
        self.call_method(Value::Undefined, args)
    }

    /// Calls the function with the given `this` and arguments.
    pub fn call_method<T, A, R>(&self, this: T, args: A) -> Result<R>
    where
        T: ToValue,
        A: ToValues,
        R: FromValue,
    {
        let this = this.to_value(&self.engine)?;
        let args = args.to_values(&self.engine)?;
        self.engine
            .try_catch(|scope| {
                let function = v8::Local::new(scope, self.handle.clone());
                let this = this.to_v8_value(scope);
                let args = args.into_vec();
                let args_v8: Vec<_> = args.into_iter().map(|v| v.to_v8_value(scope)).collect();
                let result = function.call(scope, this, &args_v8);
                self.engine.exception(scope)?;
                Ok(Value::from_v8_value(&self.engine, scope, result.unwrap()))
            })
            .and_then(|v| v.into(&self.engine))
    }

    /// Calls the function as a constructor function with the given arguments.
    pub fn call_new<A, R>(&self, args: A) -> Result<R>
    where
        A: ToValues,
        R: FromValue,
    {
        let args = args.to_values(&self.engine)?;
        self.engine
            .try_catch(|scope| {
                let function = v8::Local::new(scope, self.handle.clone());
                let args = args.into_vec();
                let args_v8: Vec<_> = args.into_iter().map(|v| v.to_v8_value(scope)).collect();
                let result = function.new_instance(scope, &args_v8);
                self.engine.exception(scope)?;
                Ok(Value::from_v8_value(
                    &self.engine,
                    scope,
                    result.unwrap().into(),
                ))
            })
            .and_then(|v| v.into(&self.engine))
    }
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<function>")
    }
}

/// A bundle of information about an invocation of a function that has been embedded from Rust into
/// JavaScript.
pub struct Invocation {
    /// The `JsEngine` within which the function was called.
    pub engine: JsEngine,
    /// The value of the function invocation's `this` binding.
    pub this: Value,
    /// The list of arguments with which the function was called.
    pub args: Values,
}
