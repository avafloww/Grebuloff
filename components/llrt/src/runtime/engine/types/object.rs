use super::*;
use crate::runtime::engine::*;
use std::fmt;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct JsObject {
    pub engine: JsEngine,
    pub handle: v8::Global<v8::Object>,
}

impl JsObject {
    /// Get an object property value using the given key. Returns `Value::Undefined` if no property
    /// with the key exists.
    ///
    /// Returns an error if `ToValue::to_value` fails for the key or if the key value could not be
    /// cast to a property key string.
    pub fn get<K: ToJsValue, V: FromJsValue>(&self, key: K) -> JsResult<V> {
        let key = key.to_value(&self.engine)?;
        self.engine
            .try_catch(|scope| {
                let object = v8::Local::new(scope, self.handle.clone());
                let key = key.to_v8_value(scope);
                let result = object.get(scope, key);
                self.engine.exception(scope)?;
                Ok(JsValue::from_v8_value(&self.engine, scope, result.unwrap()))
            })
            .and_then(|v| v.into(&self.engine))
    }

    /// Sets an object property using the given key and value.
    ///
    /// Returns an error if `ToValue::to_value` fails for either the key or the value or if the key
    /// value could not be cast to a property key string.
    pub fn set<K: ToJsValue, V: ToJsValue>(&self, key: K, value: V) -> JsResult<()> {
        let key = key.to_value(&self.engine)?;
        let value = value.to_value(&self.engine)?;
        self.engine.try_catch(|scope| {
            let object = v8::Local::new(scope, self.handle.clone());
            let key = key.to_v8_value(scope);
            let value = value.to_v8_value(scope);
            object.set(scope, key, value);
            self.engine.exception(scope)
        })
    }

    /// Removes the property associated with the given key from the object. This function does
    /// nothing if the property does not exist.
    ///
    /// Returns an error if `ToValue::to_value` fails for the key or if the key value could not be
    /// cast to a property key string.
    pub fn remove<K: ToJsValue>(&self, key: K) -> JsResult<()> {
        let key = key.to_value(&self.engine)?;
        self.engine.try_catch(|scope| {
            let object = v8::Local::new(scope, self.handle.clone());
            let key = key.to_v8_value(scope);
            object.delete(scope, key);
            self.engine.exception(scope)
        })
    }

    /// Returns `true` if the given key is a property of the object, `false` otherwise.
    ///
    /// Returns an error if `ToValue::to_value` fails for the key or if the key value could not be
    /// cast to a property key string.
    pub fn has<K: ToJsValue>(&self, key: K) -> JsResult<bool> {
        let key = key.to_value(&self.engine)?;
        self.engine.try_catch(|scope| {
            let object = v8::Local::new(scope, self.handle.clone());
            let key = key.to_v8_value(scope);
            let has = object.has(scope, key);
            self.engine.exception(scope)?;
            Ok(has.unwrap())
        })
    }

    /// Calls the function at the key with the given arguments, with `this` set to the object.
    /// Returns an error if the value at the key is not a function.
    pub fn call_prop<K, A, R>(&self, key: K, args: A) -> JsResult<R>
    where
        K: ToJsValue,
        A: ToJsValues,
        R: FromJsValue,
    {
        let func: JsFunction = self.get(key)?;
        func.call_method(self.clone(), args)
    }

    /// Returns an array containing all of this object's enumerable property keys. If
    /// `include_inherited` is `false`, then only the object's own enumerable properties will be
    /// collected (similar to `Object.getOwnPropertyNames` in Javascript). If `include_inherited` is
    /// `true`, then the object's own properties and the enumerable properties from its prototype
    /// chain will be collected.
    pub fn keys(&self, include_inherited: bool) -> JsResult<JsArray> {
        self.engine.try_catch(|scope| {
            let object = v8::Local::new(scope, self.handle.clone());
            let keys = if include_inherited {
                object.get_property_names(scope, Default::default())
            } else {
                object.get_own_property_names(scope, Default::default())
            };
            self.engine.exception(scope)?;
            Ok(JsArray {
                engine: self.engine.clone(),
                handle: v8::Global::new(scope, keys.unwrap()),
            })
        })
    }

    /// Converts the object into an iterator over the object's keys and values, acting like a
    /// `for-in` loop.
    ///
    /// For information on the `include_inherited` argument, see `Object::keys`.
    pub fn properties<K, V>(self, include_inherited: bool) -> JsResult<Properties<K, V>>
    where
        K: FromJsValue,
        V: FromJsValue,
    {
        let keys = self.keys(include_inherited)?;
        Ok(Properties {
            object: self,
            keys,
            index: 0,
            _phantom: PhantomData,
        })
    }
}

impl fmt::Debug for JsObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let keys = match self.keys(false) {
            Ok(keys) => keys,
            Err(_) => return write!(f, "<object with keys exception>"),
        };

        let len = keys.len();
        if len == 0 {
            return write!(f, "{{}}");
        }

        write!(f, "{{ ")?;
        for i in 0..len {
            if let Ok(k) = keys
                .get::<JsValue>(i)
                .and_then(|k| k.coerce_string(&self.engine))
            {
                write!(f, "{:?}: ", k)?;
                match self.get::<_, JsValue>(k) {
                    Ok(v) => write!(f, "{:?}", v)?,
                    Err(_) => write!(f, "?")?,
                };
            } else {
                write!(f, "?")?;
            }
            if i + 1 < len {
                write!(f, ", ")?;
            }
        }
        write!(f, " }}")
    }
}

/// An iterator over an object's keys and values, acting like a `for-in` loop.
pub struct Properties<K, V> {
    object: JsObject,
    keys: JsArray,
    index: u32,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> Iterator for Properties<K, V>
where
    K: FromJsValue,
    V: FromJsValue,
{
    type Item = JsResult<(K, V)>;

    /// This will return `Some(Err(...))` if the next property's key or value failed to be converted
    /// into `K` or `V` respectively (through `ToValue`).
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.keys.len() {
            return None;
        }

        let key = self.keys.get::<JsValue>(self.index);
        self.index += 1;

        let key = match key {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };

        let value = match self.object.get::<_, V>(key.clone()) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };

        let key = match key.into(&self.object.engine) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };

        Some(Ok((key, value)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::String as StdString;

    #[test]
    fn set_get() {
        let engine = JsEngine::new();

        let object = engine.create_object();
        object.set("a", 123).unwrap();
        object.set(123, "a").unwrap();
        let parent = engine.create_object();
        parent.set("obj", object).unwrap();
        let object: JsObject = parent.get("obj").unwrap();
        assert_eq!(object.get::<_, i8>("a").unwrap(), 123);
        assert_eq!(object.get::<_, StdString>("a").unwrap(), "123");
        assert_eq!(object.get::<_, StdString>("123").unwrap(), "a");
        assert_eq!(object.get::<_, StdString>(123).unwrap(), "a");
    }

    #[test]
    fn remove() {
        let engine = JsEngine::new();
        let globals = engine.global();
        assert!(globals.has("Object").unwrap());
        globals.remove("Object").unwrap();
        assert!(!globals.has("Object").unwrap());
        // Removing keys that don't exist does nothing:
        globals.remove("Object").unwrap();
        assert!(!globals.has("Object").unwrap());
    }

    #[test]
    fn has() {
        let engine = JsEngine::new();
        let globals = engine.global();
        assert!(globals.has("Array").unwrap());
        assert!(!globals.has("~NOT-EXIST~").unwrap());
    }

    #[test]
    fn keys() {
        let engine = JsEngine::new();
        let object = engine.create_object();
        object.set("c", 3).unwrap();
        object.set("b", 2).unwrap();
        object.set("a", 1).unwrap();
        let keys: JsResult<Vec<StdString>> = object.keys(true).unwrap().elements().collect();
        assert_eq!(
            keys.unwrap(),
            vec!["c".to_string(), "b".to_string(), "a".to_string()]
        )
    }

    #[test]
    fn properties() {
        let engine = JsEngine::new();

        let object = engine.create_object();
        object.set("a", 123).unwrap();
        object.set(4, JsValue::Undefined).unwrap();
        object.set(123, "456").unwrap();

        let list = object
            .properties(false)
            .unwrap()
            .map(|property| {
                let result: (StdString, usize) = property.unwrap();
                result
            })
            .collect::<Vec<_>>();

        assert_eq!(
            list,
            vec![
                ("4".to_string(), 0),
                ("123".to_string(), 456),
                ("a".to_string(), 123)
            ]
        );
    }
}
