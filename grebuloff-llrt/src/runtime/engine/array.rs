use super::*;
use std::fmt;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct Array {
    pub(crate) engine: JsEngine,
    pub(crate) handle: v8::Global<v8::Array>,
}

impl Array {
    /// Consumes the array and downgrades it to a JavaScript object.
    pub fn into_object(self) -> Object {
        self.engine.clone().scope(|scope| {
            let object: v8::Local<v8::Object> = v8::Local::new(scope, self.handle.clone()).into();
            Object {
                engine: self.engine,
                handle: v8::Global::new(scope, object),
            }
        })
    }

    /// Get the value using the given array index. Returns `Value::Undefined` if no element at the
    /// index exists.
    ///
    /// Returns an error if `FromValue::from_value` fails for the element.
    pub fn get<V: FromValue>(&self, index: u32) -> Result<V> {
        self.engine
            .try_catch(|scope| {
                let array = v8::Local::new(scope, self.handle.clone());
                let result = array.get_index(scope, index);
                self.engine.exception(scope)?;
                Ok(Value::from_v8_value(&self.engine, scope, result.unwrap()))
            })
            .and_then(|v| v.into(&self.engine))
    }

    /// Sets an array element using the given index and value.
    ///
    /// Returns an error if `ToValue::to_value` fails for the value.
    pub fn set<V: ToValue>(&self, index: u32, value: V) -> Result<()> {
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
    pub fn push<V: ToValue>(&self, value: V) -> Result<()> {
        self.set(self.len(), value)
    }

    /// Returns an iterator over the array's indexable values.
    pub fn elements<V: FromValue>(self) -> Elements<V> {
        Elements {
            array: self,
            index: 0,
            len: None,
            _phantom: PhantomData,
        }
    }
}

impl fmt::Debug for Array {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let len = self.len();
        write!(f, "[")?;
        for i in 0..len {
            match self.get::<Value>(i) {
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
    array: Array,
    index: u32,
    len: Option<u32>,
    _phantom: PhantomData<V>,
}

impl<V: FromValue> Iterator for Elements<V> {
    type Item = Result<V>;

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
