use crate::runtime::engine::*;

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
