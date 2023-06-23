use crate::runtime::engine::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::string::String as StdString;
use std::time::Duration;

#[test]
fn eval_origin() {
    let engine = JsEngine::new();
    let result: StdString = engine
        .eval(Script {
            source: "try { MISSING_VAR } catch (e) { e.stack }".to_owned(),
            origin: Some(ScriptOrigin {
                name: "eval_origin".to_owned(),
                line_offset: 123,
                column_offset: 456,
            }),
            ..Default::default()
        })
        .unwrap();
    let result = result.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        "ReferenceError: MISSING_VAR is not defined at eval_origin:124:463",
        result
    );
}

#[test]
fn eval_timeout() {
    let engine = JsEngine::new();
    let result = engine.eval::<_, JsValue>(Script {
        source: "a = 0; while (true) { a++; }".to_owned(),
        timeout: Some(Duration::from_millis(50)),
        ..Default::default()
    });

    match result {
        Err(JsError::Timeout) => {}
        _ => panic!("unexpected result: {:?}", result),
    }

    // Make sure we can still evaluate again:
    let a: f64 = engine.eval("a").unwrap();
    assert!(a > 0.0);
}

#[test]
fn eval_wasm() {
    let engine = JsEngine::new();
    let result = engine.eval::<_, JsValue>(
        r#"
        let bytes = new Uint8Array([
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f,
            0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64,
            0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b
        ]);

        let module = new WebAssembly.Module(bytes);
        let instance = new WebAssembly.Instance(module);
        instance.exports.add(3, 4)
    "#,
    );

    match result {
        Ok(JsValue::Number(n)) if n == 7.0 => {}
        _ => panic!("unexpected result: {:?}", result),
    }
}

#[test]
#[should_panic(expected = "attempt to use Handle in an Isolate that is not its host")]
fn value_cross_contamination() {
    let engine_1 = JsEngine::new();
    let str_1 = engine_1.create_string("123");
    let engine_2 = JsEngine::new();
    let _str_2 = engine_2.create_string("456");
    let _ = JsValue::String(str_1).coerce_number(&engine_2);
}
