use crate::runtime::*;

#[test]
fn coerce_boolean() {
    let engine = JsEngine::new();
    assert!(!Value::Undefined.coerce_boolean(&engine));
    assert!(!Value::Null.coerce_boolean(&engine));
    assert!(!Value::Number(0.0).coerce_boolean(&engine));
    assert!(Value::Number(1.0).coerce_boolean(&engine));
    assert!(!Value::String(engine.create_string("")).coerce_boolean(&engine));
    assert!(Value::String(engine.create_string("a")).coerce_boolean(&engine));
    assert!(Value::Object(engine.create_object()).coerce_boolean(&engine));
}

#[test]
fn coerce_number() {
    let engine = JsEngine::new();
    assert!(Value::Undefined.coerce_number(&engine).unwrap().is_nan());
    assert_eq!(0.0, Value::Null.coerce_number(&engine).unwrap());
    assert_eq!(0.0, Value::Number(0.0).coerce_number(&engine).unwrap());
    assert_eq!(1.0, Value::Number(1.0).coerce_number(&engine).unwrap());
    assert_eq!(
        0.0,
        Value::String(engine.create_string(""))
            .coerce_number(&engine)
            .unwrap()
    );
    assert!(Value::String(engine.create_string("a"))
        .coerce_number(&engine)
        .unwrap()
        .is_nan());
    assert!(Value::Object(engine.create_object())
        .coerce_number(&engine)
        .unwrap()
        .is_nan());
}

#[test]
fn coerce_string() {
    fn assert_string_eq(engine: &JsEngine, value: Value, expected: &str) {
        assert_eq!(expected, value.coerce_string(engine).unwrap().to_string());
    }

    let engine = JsEngine::new();
    assert_string_eq(&engine, Value::Undefined, "undefined");
    assert_string_eq(&engine, Value::Null, "null");
    assert_string_eq(&engine, Value::Number(123.0), "123");
    assert_string_eq(&engine, Value::String(engine.create_string("abc")), "abc");
    assert_string_eq(
        &engine,
        Value::Object(engine.create_object()),
        "[object Object]",
    );
}
