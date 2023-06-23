use super::*;
use crate::runtime::engine::*;
use std::fmt;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct JsArray {
    pub engine: JsEngine,
    pub handle: v8::Global<v8::Array>,
}

impl JsArray {
    /// Consumes the array and downgrades it to a JavaScript object.
    pub fn into_object(self) -> JsObject {
        self.engine.clone().scope(|scope| {
            let object: v8::Local<v8::Object> = v8::Local::new(scope, self.handle.clone()).into();
            JsObject {
                engine: self.engine,
                handle: v8::Global::new(scope, object),
            }
        })
    }

    /// Get the value using the given array index. Returns `Value::Undefined` if no element at the
    /// index exists.
    ///
    /// Returns an error if `FromValue::from_value` fails for the element.
    pub fn get<V: FromJsValue>(&self, index: u32) -> JsResult<V> {
        self.engine
            .try_catch(|scope| {
                let array = v8::Local::new(scope, self.handle.clone());
                let result = array.get_index(scope, index);
                self.engine.exception(scope)?;
                Ok(JsValue::from_v8_value(&self.engine, scope, result.unwrap()))
            })
            .and_then(|v| v.into(&self.engine))
    }

    /// Sets an array element using the given index and value.
    ///
    /// Returns an error if `ToValue::to_value` fails for the value.
    pub fn set<V: ToJsValue>(&self, index: u32, value: V) -> JsResult<()> {
        let value = value.to_value(&self.engine)?;
        self.engine.try_catch(|scope| {
            let array = v8::Local::new(scope, self.handle.clone());
            let value = value.to_v8_value(scope);
            array.set_index(scope, index, value);
            self.engine.exception(scope)
        })
    }

    /// Returns the number of elements in the array.
    pub fn len(&self) -> u32 {
        self.engine
            .scope(|scope| v8::Local::new(scope, self.handle.clone()).length())
    }

    /// Pushes an element to the end of the array. This is a shortcut for `set` using `len` as the
    /// index.
    pub fn push<V: ToJsValue>(&self, value: V) -> JsResult<()> {
        self.set(self.len(), value)
    }

    /// Returns an iterator over the array's indexable values.
    pub fn elements<V: FromJsValue>(self) -> Elements<V> {
        Elements {
            array: self,
            index: 0,
            len: None,
            _phantom: PhantomData,
        }
    }
}

impl fmt::Debug for JsArray {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let len = self.len();
        write!(f, "[")?;
        for i in 0..len {
            match self.get::<JsValue>(i) {
                Ok(v) => write!(f, "{:?}", v)?,
                Err(_) => write!(f, "?")?,
            };
            if i + 1 < len {
                write!(f, ", ")?;
            }
        }
        write!(f, "]")
    }
}

pub struct Elements<V> {
    array: JsArray,
    index: u32,
    len: Option<u32>,
    _phantom: PhantomData<V>,
}

impl<V: FromJsValue> Iterator for Elements<V> {
    type Item = JsResult<V>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len.is_none() {
            self.len = Some(self.array.len());
        }

        if self.index >= self.len.unwrap() {
            return None;
        }

        let result = self.array.get(self.index);
        self.index += 1;
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::String as StdString;

    #[test]
    fn set_get() {
        let engine = JsEngine::new();

        let array = engine.create_array();
        array.set(0, 123).unwrap();
        array.set(2, 456).unwrap();
        assert_eq!(array.get::<StdString>(0).unwrap(), "123");
        assert!(array.get::<JsValue>(1).unwrap().is_undefined());
        assert_eq!(array.get::<StdString>(2).unwrap(), "456");
        assert!(array.get::<JsValue>(3).unwrap().is_undefined());
    }

    #[test]
    fn len() {
        let engine = JsEngine::new();

        let array = engine.create_array();
        assert_eq!(array.len(), 0);
        array.set(0, 123).unwrap();
        assert_eq!(array.len(), 1);
        array.set(2, 456).unwrap();
        assert_eq!(array.len(), 3);
    }

    #[test]
    fn push() {
        let engine = JsEngine::new();

        let array = engine.create_array();
        array.push(0).unwrap();
        array.push(1).unwrap();
        array.set(3, 3).unwrap();
        array.push(4).unwrap();
        assert_eq!(array.get::<usize>(0).unwrap(), 0);
        assert_eq!(array.get::<usize>(1).unwrap(), 1);
        assert!(array.get::<JsValue>(2).unwrap().is_undefined());
        assert_eq!(array.get::<usize>(3).unwrap(), 3);
        assert_eq!(array.get::<usize>(4).unwrap(), 4);
        assert_eq!(array.len(), 5);
    }

    #[test]
    fn elements() {
        let engine = JsEngine::new();

        let array = engine.create_array();
        array.push(0).unwrap();
        array.push(1).unwrap();
        array.set(3, 3).unwrap();
        array.push(4).unwrap();

        let list: JsResult<Vec<usize>> = array.elements().collect();
        assert_eq!(list.unwrap(), vec![0, 1, 0, 3, 4]);
    }
}