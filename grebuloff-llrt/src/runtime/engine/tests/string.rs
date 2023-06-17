use crate::runtime::engine::*;

#[test]
fn to_string() {
    let engine = JsEngine::new();
    assert_eq!(
        engine.create_string("abc😊🈹").to_string(),
        "abc😊🈹".to_string()
    );
}
