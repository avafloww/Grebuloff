use crate::runtime::engine::*;
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
