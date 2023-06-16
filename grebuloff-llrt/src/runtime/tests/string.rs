use crate::runtime::JsEngine;

#[test]
fn to_string() {
    let engine = JsEngine::new();
    assert_eq!(
        engine.create_string("abc😊🈹").to_string(),
        "abc😊🈹".to_string()
    );
}
