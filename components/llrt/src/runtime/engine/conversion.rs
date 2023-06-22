use super::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::string::String as StdString;
use std::time::Duration;

impl<T: ToJsValue> ToJsValue for anyhow::Result<T, anyhow::Error> {
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue> {
        match self {
            Ok(val) => Ok(val.to_value(engine)?),
            Err(err) => Err(JsError::ExternalError(err)),
        }
    }
}

impl ToJsValue for JsValue {
    fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
        Ok(self)
    }
}

impl FromJsValue for JsValue {
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<Self> {
        Ok(value)
    }
}

impl ToJsValue for () {
    fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::Undefined)
    }
}

impl FromJsValue for () {
    fn from_value(_value: JsValue, _engine: &JsEngine) -> JsResult<Self> {
        Ok(())
    }
}

impl<T: ToJsValue> ToJsValue for Option<T> {
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue> {
        match self {
            Some(val) => val.to_value(engine),
            None => Ok(JsValue::Null),
        }
    }
}

impl<T: FromJsValue> FromJsValue for Option<T> {
    fn from_value(value: JsValue, engine: &JsEngine) -> JsResult<Self> {
        match value {
            JsValue::Null | JsValue::Undefined => Ok(None),
            value => Ok(Some(T::from_value(value, engine)?)),
        }
    }
}

impl ToJsValue for JsString {
    fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::String(self))
    }
}

impl FromJsValue for JsString {
    fn from_value(value: JsValue, engine: &JsEngine) -> JsResult<JsString> {
        value.coerce_string(engine)
    }
}

impl ToJsValue for JsArray {
    fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::Array(self))
    }
}

impl FromJsValue for JsArray {
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<JsArray> {
        match value {
            JsValue::Array(a) => Ok(a),
            value => Err(JsError::from_js_conversion(value.type_name(), "Array")),
        }
    }
}

impl ToJsValue for JsFunction {
    fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::Function(self))
    }
}

impl FromJsValue for JsFunction {
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<JsFunction> {
        match value {
            JsValue::Function(f) => Ok(f),
            value => Err(JsError::from_js_conversion(value.type_name(), "Function")),
        }
    }
}

impl ToJsValue for JsPromise {
    fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::Promise(self))
    }
}

impl FromJsValue for JsPromise {
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<JsPromise> {
        match value {
            JsValue::Promise(p) => Ok(p),
            value => Err(JsError::from_js_conversion(value.type_name(), "Promise")),
        }
    }
}

impl ToJsValue for JsObject {
    fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::Object(self))
    }
}

impl FromJsValue for JsObject {
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<JsObject> {
        match value {
            JsValue::Object(o) => Ok(o),
            value => Err(JsError::from_js_conversion(value.type_name(), "Object")),
        }
    }
}

impl<K, V, S> ToJsValue for HashMap<K, V, S>
where
    K: Eq + Hash + ToJsValue,
    V: ToJsValue,
    S: BuildHasher,
{
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue> {
        let object = engine.create_object();
        for (k, v) in self.into_iter() {
            object.set(k, v)?;
        }
        Ok(JsValue::Object(object))
    }
}

impl<K, V, S> FromJsValue for HashMap<K, V, S>
where
    K: Eq + Hash + FromJsValue,
    V: FromJsValue,
    S: BuildHasher + Default,
{
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<Self> {
        match value {
            JsValue::Object(o) => o.properties(false)?.collect(),
            value => Err(JsError::from_js_conversion(value.type_name(), "HashMap")),
        }
    }
}

impl<K, V> ToJsValue for BTreeMap<K, V>
where
    K: Ord + ToJsValue,
    V: ToJsValue,
{
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue> {
        let object = engine.create_object();
        for (k, v) in self.into_iter() {
            object.set(k, v)?;
        }
        Ok(JsValue::Object(object))
    }
}

impl<K, V> FromJsValue for BTreeMap<K, V>
where
    K: Ord + FromJsValue,
    V: FromJsValue,
{
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<Self> {
        match value {
            JsValue::Object(o) => o.properties(false)?.collect(),
            value => Err(JsError::from_js_conversion(value.type_name(), "BTreeMap")),
        }
    }
}

impl<V: ToJsValue> ToJsValue for BTreeSet<V> {
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue> {
        let array = engine.create_array();
        for v in self.into_iter() {
            array.push(v)?;
        }
        Ok(JsValue::Array(array))
    }
}

impl<V: FromJsValue + Ord> FromJsValue for BTreeSet<V> {
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<Self> {
        match value {
            JsValue::Array(a) => a.elements().collect(),
            value => Err(JsError::from_js_conversion(value.type_name(), "BTreeSet")),
        }
    }
}

impl<V: ToJsValue> ToJsValue for HashSet<V> {
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue> {
        let array = engine.create_array();
        for v in self.into_iter() {
            array.push(v)?;
        }
        Ok(JsValue::Array(array))
    }
}

impl<V: FromJsValue + Hash + Eq> FromJsValue for HashSet<V> {
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<Self> {
        match value {
            JsValue::Array(a) => a.elements().collect(),
            value => Err(JsError::from_js_conversion(value.type_name(), "HashSet")),
        }
    }
}

impl<V: ToJsValue> ToJsValue for Vec<V> {
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue> {
        let array = engine.create_array();
        for v in self.into_iter() {
            array.push(v)?;
        }
        Ok(JsValue::Array(array))
    }
}

impl<V: FromJsValue> FromJsValue for Vec<V> {
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<Self> {
        match value {
            JsValue::Array(a) => a.elements().collect(),
            value => Err(JsError::from_js_conversion(value.type_name(), "Vec")),
        }
    }
}

impl ToJsValue for bool {
    fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::Boolean(self))
    }
}

impl FromJsValue for bool {
    fn from_value(value: JsValue, engine: &JsEngine) -> JsResult<Self> {
        Ok(value.coerce_boolean(engine))
    }
}

impl ToJsValue for StdString {
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::String(engine.create_string(&self)))
    }
}

impl FromJsValue for StdString {
    fn from_value(value: JsValue, engine: &JsEngine) -> JsResult<Self> {
        Ok(value.coerce_string(engine)?.to_string())
    }
}

impl<'a> ToJsValue for &'a str {
    fn to_value(self, engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::String(engine.create_string(self)))
    }
}

macro_rules! convert_number {
    ($prim_ty: ty) => {
        impl ToJsValue for $prim_ty {
            fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
                Ok(JsValue::Number(self as f64))
            }
        }

        impl FromJsValue for $prim_ty {
            fn from_value(value: JsValue, engine: &JsEngine) -> JsResult<Self> {
                Ok(value.coerce_number(engine)? as $prim_ty)
            }
        }
    };
}

convert_number!(i8);
convert_number!(u8);
convert_number!(i16);
convert_number!(u16);
convert_number!(i32);
convert_number!(u32);
convert_number!(i64);
convert_number!(u64);
convert_number!(isize);
convert_number!(usize);
convert_number!(f32);
convert_number!(f64);

impl ToJsValue for Duration {
    fn to_value(self, _engine: &JsEngine) -> JsResult<JsValue> {
        Ok(JsValue::Date(
            (self.as_secs() as f64) + (self.as_nanos() as f64) / 1_000_000_000.0,
        ))
    }
}

impl FromJsValue for Duration {
    fn from_value(value: JsValue, _engine: &JsEngine) -> JsResult<Duration> {
        match value {
            JsValue::Date(timestamp) => {
                let secs = timestamp / 1000.0;
                let nanos = ((secs - secs.floor()) * 1_000_000.0).round() as u32;
                Ok(Duration::new(secs as u64, nanos))
            }
            value => Err(JsError::from_js_conversion(value.type_name(), "Duration")),
        }
    }
}

impl ToJsValues for JsValues {
    fn to_values(self, _engine: &JsEngine) -> JsResult<JsValues> {
        Ok(self)
    }
}

impl FromJsValues for JsValues {
    fn from_values(values: JsValues, _engine: &JsEngine) -> JsResult<Self> {
        Ok(values)
    }
}

impl<T: ToJsValue> ToJsValues for Variadic<T> {
    fn to_values(self, engine: &JsEngine) -> JsResult<JsValues> {
        self.0
            .into_iter()
            .map(|value| value.to_value(engine))
            .collect()
    }
}

impl<T: FromJsValue> FromJsValues for Variadic<T> {
    fn from_values(values: JsValues, engine: &JsEngine) -> JsResult<Self> {
        values
            .into_iter()
            .map(|value| T::from_value(value, engine))
            .collect::<JsResult<Vec<T>>>()
            .map(Variadic)
    }
}

impl ToJsValues for () {
    fn to_values(self, _engine: &JsEngine) -> JsResult<JsValues> {
        Ok(JsValues::new())
    }
}

impl FromJsValues for () {
    fn from_values(_values: JsValues, _engine: &JsEngine) -> JsResult<Self> {
        Ok(())
    }
}

macro_rules! impl_tuple {
    ($($name:ident),*) => (
        impl<$($name),*> ToJsValues for ($($name,)*)
        where
            $($name: ToJsValue,)*
        {
            #[allow(non_snake_case)]
            fn to_values(self, engine: &JsEngine) -> JsResult<JsValues> {
                let ($($name,)*) = self;
                let reservation = $({ let _ = &$name; 1 } +)* 0;
                let mut results = Vec::with_capacity(reservation);
                $(results.push($name.to_value(engine)?);)*
                Ok(JsValues::from_vec(results))
            }
        }

        impl<$($name),*> FromJsValues for ($($name,)*)
        where
            $($name: FromJsValue,)*
        {
            #[allow(non_snake_case, unused_mut, unused_variables)]
            fn from_values(values: JsValues, engine: &JsEngine) -> JsResult<Self> {
                let mut iter = values.into_vec().into_iter();
                Ok(($({
                    let $name = ();
                    FromJsValue::from_value(iter.next().unwrap_or(JsValue::Undefined), engine)?
                },)*))
            }
        }

        impl<$($name,)* VAR> ToJsValues for ($($name,)* Variadic<VAR>)
        where
            $($name: ToJsValue,)*
            VAR: ToJsValue,
        {
            #[allow(non_snake_case)]
            fn to_values(self, engine: &JsEngine) -> JsResult<JsValues> {
                let ($($name,)* variadic) = self;
                let reservation = $({ let _ = &$name; 1 } +)* 1;
                let mut results = Vec::with_capacity(reservation);
                $(results.push($name.to_value(engine)?);)*
                if results.is_empty() {
                    Ok(variadic.to_values(engine)?)
                } else {
                    results.append(&mut variadic.to_values(engine)?.into_vec());
                    Ok(JsValues::from_vec(results))
                }
            }
        }

        impl<$($name,)* VAR> FromJsValues for ($($name,)* Variadic<VAR>)
        where
            $($name: FromJsValue,)*
            VAR: FromJsValue,
        {
            #[allow(non_snake_case, unused_mut, unused_variables)]
            fn from_values(values: JsValues, engine: &JsEngine) -> JsResult<Self> {
                let mut values = values.into_vec();
                let len = values.len();
                let split = $({ let $name = (); 1 } +)* 0;

                if len < split {
                    values.reserve(split - len);
                    for _ in len..split {
                        values.push(JsValue::Undefined);
                    }
                }

                let last_values = JsValues::from_vec(values.split_off(split));
                let variadic = FromJsValues::from_values(last_values, engine)?;

                let mut iter = values.into_iter();
                let ($($name,)*) = ($({ let $name = (); iter.next().unwrap() },)*);

                Ok(($(FromJsValue::from_value($name, engine)?,)* variadic))
            }
        }
    )
}

impl_tuple!(A);
impl_tuple!(A, B);
impl_tuple!(A, B, C);
impl_tuple!(A, B, C, D);
impl_tuple!(A, B, C, D, E);
impl_tuple!(A, B, C, D, E, F);
impl_tuple!(A, B, C, D, E, F, G);
impl_tuple!(A, B, C, D, E, F, G, H);
impl_tuple!(A, B, C, D, E, F, G, H, I);
impl_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
