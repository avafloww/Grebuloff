use super::*;
use crate::runtime::engine::*;
use std::fmt;

#[derive(Clone)]
pub struct JsFunction {
    pub engine: JsEngine,
    pub handle: v8::Global<v8::Function>,
}

impl JsFunction {
    /// Consumes the function and downgrades it to a JavaScript object.
    pub fn into_object(self) -> JsObject {
        self.engine.clone().scope(|scope| {
            let object: v8::Local<v8::Object> = v8::Local::new(scope, self.handle.clone()).into();
            JsObject {
                engine: self.engine,
                handle: v8::Global::new(scope, object),
            }
        })
    }

    /// Calls the function with the given arguments, with `this` set to `undefined`.
    pub fn call<A, R>(&self, args: A) -> JsResult<R>
    where
        A: ToJsValues,
        R: FromJsValue,
    {
        self.call_method(JsValue::Undefined, args)
    }

    /// Calls the function with the given `this` and arguments.
    pub fn call_method<T, A, R>(&self, this: T, args: A) -> JsResult<R>
    where
        T: ToJsValue,
        A: ToJsValues,
        R: FromJsValue,
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
                Ok(JsValue::from_v8_value(&self.engine, scope, result.unwrap()))
            })
            .and_then(|v| v.into(&self.engine))
    }

    /// Calls the function as a constructor function with the given arguments.
    pub fn call_new<A, R>(&self, args: A) -> JsResult<R>
    where
        A: ToJsValues,
        R: FromJsValue,
    {
        let args = args.to_values(&self.engine)?;
        self.engine
            .try_catch(|scope| {
                let function = v8::Local::new(scope, self.handle.clone());
                let args = args.into_vec();
                let args_v8: Vec<_> = args.into_iter().map(|v| v.to_v8_value(scope)).collect();
                let result = function.new_instance(scope, &args_v8);
                self.engine.exception(scope)?;
                Ok(JsValue::from_v8_value(
                    &self.engine,
                    scope,
                    result.unwrap().into(),
                ))
            })
            .and_then(|v| v.into(&self.engine))
    }
}

impl fmt::Debug for JsFunction {
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
    pub this: JsValue,
    /// The list of arguments with which the function was called.
    pub args: JsValues,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::String as StdString;

    #[test]
    fn js_function() {
        let engine = JsEngine::new();
        let func: JsValue = engine.eval("(function(y) { return this + y; })").unwrap();
        assert!(func.is_function());
        let func = if let JsValue::Function(f) = func {
            f
        } else {
            unreachable!();
        };
        let value: f64 = func.call_method(1, (2,)).unwrap();
        assert_eq!(3.0f64, value);
        let value: f64 = func.call((2,)).unwrap();
        assert!(value.is_nan());
    }

    #[test]
    fn js_constructor() {
        let engine = JsEngine::new();
        let func: JsFunction = engine.eval("(function(x) { this.x = x; })").unwrap();
        let value: JsObject = func.call_new((10,)).unwrap();
        assert_eq!(10, value.get("x").unwrap());
    }

    #[test]
    fn rust_function() {
        fn add(inv: Invocation) -> JsResult<usize> {
            let (a, b): (usize, usize) = inv.args.into(&inv.engine)?;
            return Ok(a + b);
        }

        let engine = JsEngine::new();
        let func = engine.create_function(add);
        let value: f64 = func.call((1, 2)).unwrap();
        assert_eq!(3.0f64, value);

        engine.global().set("add", func).unwrap();
        let value: f64 = engine.eval("add(4, 5)").unwrap();
        assert_eq!(9.0f64, value);
    }

    #[test]
    fn rust_function_error() {
        fn err(inv: Invocation) -> JsResult<()> {
            let _: (JsFunction,) = inv.args.into(&inv.engine)?;
            Ok(())
        }

        let engine = JsEngine::new();
        let func = engine.create_function(err);
        engine.global().set("err", func).unwrap();
        let _: () = engine
            .eval(
                r#"
        try {
            err(123);
        } catch (e) {
            if (e.name !== 'TypeError') {
                throw new Error('unexpected error');
            }
        }
    "#,
            )
            .unwrap();
    }

    #[test]
    fn rust_closure() {
        let engine = JsEngine::new();
        let func = engine.create_function(|inv| {
            let (a, b): (usize, usize) = inv.args.into(&inv.engine)?;
            Ok(a + b)
        });
        let value: f64 = func.call((1, 2)).unwrap();
        assert_eq!(3.0f64, value);
    }

    #[test]
    fn double_drop_rust_function() {
        let engine = JsEngine::new();
        let func = engine.create_function(|_| Ok(()));
        let _func_dup = func.clone();
        // The underlying boxed closure is only dropped once. (Otherwise a segfault or something might
        // occur. This admittedly isn't a very great test.)
    }

    #[test]
    fn return_unit() {
        let engine = JsEngine::new();
        let func = engine.create_function(|_| Ok(()));
        let _: () = func.call(()).unwrap();
        let _: () = func.call((123,)).unwrap();
        let number_cast: usize = func.call(()).unwrap();
        assert_eq!(number_cast, 0);
    }

    #[test]
    fn rust_closure_mut_callback_error() {
        let engine = JsEngine::new();

        let mut v = Some(Box::new(123));
        let f = engine.create_function_mut(move |inv| {
            let engine = inv.engine;
            let (mutate,) = inv.args.into(&engine)?;
            if mutate {
                v = None;
            } else {
                // Produce a mutable reference:
                let r = v.as_mut().unwrap();
                // Whoops, this will recurse into the function and produce another mutable reference!
                engine.global().get::<_, JsFunction>("f")?.call((true,))?;
                println!("Should not get here, mutable aliasing has occurred!");
                println!("value at {:p}", r as *mut _);
                println!("value is {}", r);
            }

            Ok(())
        });

        engine.global().set("f", f).unwrap();
        match engine
            .global()
            .get::<_, JsFunction>("f")
            .unwrap()
            .call::<_, ()>((false,))
        {
            Err(JsError::Value(v)) => {
                let message: StdString = v.as_object().unwrap().get("message").unwrap();
                assert_eq!(message, "mutable callback called recursively".to_string());
            }
            other => panic!("incorrect result: {:?}", other),
        };
    }

    #[test]
    fn number_this() {
        fn add(inv: Invocation) -> JsResult<f64> {
            let this: f64 = inv.this.into(&inv.engine)?;
            let (acc,): (f64,) = inv.args.into(&inv.engine)?;
            return Ok(this + acc);
        }

        let engine = JsEngine::new();
        let func = engine.create_function(add);

        let value: f64 = func.call_method(10, (20,)).unwrap();
        assert_eq!(30.0f64, value);
        let value: f64 = func.call((1,)).unwrap();
        assert!(value.is_nan());

        engine.global().set("add", func).unwrap();
        let value: f64 = engine.eval("add.call(12, 13)").unwrap();
        assert_eq!(25.0f64, value);
        let value: f64 = engine.eval("add(5)").unwrap();
        assert!(value.is_nan());
    }
}
