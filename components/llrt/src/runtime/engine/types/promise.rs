use super::*;
use crate::runtime::engine::*;
use std::fmt;

#[derive(Clone)]
pub struct JsPromise {
    pub engine: JsEngine,
    pub handle: v8::Global<v8::Promise>,
    pub resolver: Option<v8::Global<v8::PromiseResolver>>,
}

impl JsPromise {
    /// Resolves the promise with the given value.
    pub fn resolve<T: ToJsValue>(&self, value: T) -> JsResult<()> {
        if self.resolver.is_none() {
            return Err(JsError::PromiseWithoutResolver);
        }

        let value = value.to_value(&self.engine)?;
        self.engine.try_catch(|scope| {
            let resolver = v8::Local::new(scope, self.resolver.as_ref().unwrap().clone());
            let value = value.to_v8_value(scope);
            resolver.resolve(scope, value);
            self.engine.exception(scope)?;
            Ok(())
        })
    }

    /// Rejects the promise with the given value.
    pub fn reject<T: ToJsValue>(&self, value: T) -> JsResult<()> {
        if self.resolver.is_none() {
            return Err(JsError::PromiseWithoutResolver);
        }

        let value = value.to_value(&self.engine)?;
        self.engine.try_catch(|scope| {
            let resolver = v8::Local::new(scope, self.resolver.as_ref().unwrap().clone());
            let value = value.to_v8_value(scope);
            resolver.reject(scope, value);
            self.engine.exception(scope)?;
            Ok(())
        })
    }

    /// Gets the state of the promise.
    /// If the promise has been resolved or rejected, the value is included.
    pub fn state(&self) -> JsResult<PromiseState> {
        self.engine.try_catch(|scope| {
            let promise = v8::Local::new(scope, self.handle.clone());
            let state = promise.state();
            if let v8::PromiseState::Pending = state {
                self.engine.exception(scope)?;
                Ok(PromiseState::Pending)
            } else {
                let value = promise.result(scope);
                self.engine.exception(scope)?;
                Ok(match state {
                    v8::PromiseState::Fulfilled => PromiseState::Resolved(JsValue::from_v8_value(
                        &self.engine,
                        scope,
                        value.into(),
                    )),
                    v8::PromiseState::Rejected => PromiseState::Rejected(JsValue::from_v8_value(
                        &self.engine,
                        scope,
                        value.into(),
                    )),
                    _ => unreachable!(),
                })
            }
        })
    }
}

#[derive(Clone, Debug)]
pub enum PromiseState {
    Pending,
    Resolved(JsValue),
    Rejected(JsValue),
}

impl From<v8::PromiseState> for PromiseState {
    fn from(state: v8::PromiseState) -> Self {
        match state {
            v8::PromiseState::Pending => Self::Pending,
            v8::PromiseState::Fulfilled => Self::Resolved(JsValue::Undefined),
            v8::PromiseState::Rejected => Self::Rejected(JsValue::Undefined),
        }
    }
}

impl fmt::Debug for JsPromise {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<promise>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type() {
        let engine = JsEngine::new();
        let resolved_val: JsValue = engine.eval("Promise.resolve(123)").unwrap();
        assert!(resolved_val.is_promise());

        let resolved_promise = if let JsValue::Promise(p) = resolved_val {
            p
        } else {
            unreachable!();
        };

        let state = resolved_promise.state().unwrap();
        if let PromiseState::Resolved(value) = state {
            assert_eq!(123, value.into::<i32>(&engine).unwrap());
        } else {
            panic!();
        }
    }

    #[test]
    fn test_resolve() {
        let engine = JsEngine::new();

        let promise = engine.create_promise();
        match promise.state().unwrap() {
            PromiseState::Pending => {}
            _ => panic!(),
        }

        promise.resolve(123).unwrap();
        let state = promise.state().unwrap();
        if let PromiseState::Resolved(value) = state {
            assert_eq!(123, value.into::<i32>(&engine).unwrap());
        } else {
            panic!();
        }
    }

    #[test]
    fn test_reject() {
        let engine = JsEngine::new();

        let promise = engine.create_promise();
        match promise.state().unwrap() {
            PromiseState::Pending => {}
            _ => panic!(),
        }

        promise.reject(123).unwrap();
        let state = promise.state().unwrap();
        if let PromiseState::Rejected(value) = state {
            assert_eq!(123, value.into::<i32>(&engine).unwrap());
        } else {
            panic!();
        }
    }

    #[tokio::test]
    async fn test_async_resolve() {
        let engine = JsEngine::new();

        let promise = engine.create_promise();
        match promise.state().unwrap() {
            PromiseState::Pending => {}
            _ => panic!(),
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        match promise.state().unwrap() {
            PromiseState::Pending => {}
            _ => panic!(),
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        promise.resolve(456).unwrap();

        let state = promise.state().unwrap();
        if let PromiseState::Resolved(value) = state {
            assert_eq!(456, value.into::<i32>(&engine).unwrap());
        } else {
            panic!();
        }
    }

    #[tokio::test]
    async fn test_async_reject() {
        let engine = JsEngine::new();

        let promise = engine.create_promise();
        match promise.state().unwrap() {
            PromiseState::Pending => {}
            _ => panic!(),
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        match promise.state().unwrap() {
            PromiseState::Pending => {}
            _ => panic!(),
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        promise.reject(456).unwrap();

        let state = promise.state().unwrap();
        if let PromiseState::Rejected(value) = state {
            assert_eq!(456, value.into::<i32>(&engine).unwrap());
        } else {
            panic!();
        }
    }
}
