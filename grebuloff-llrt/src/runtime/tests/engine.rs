use crate::runtime::*;
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
    let result = engine.eval::<_, Value>(Script {
        source: "a = 0; while (true) { a++; }".to_owned(),
        timeout: Some(Duration::from_millis(50)),
        ..Default::default()
    });

    match result {
        Err(Error::Timeout) => {}
        _ => panic!("unexpected result: {:?}", result),
    }

    // Make sure we can still evaluate again:
    let a: f64 = engine.eval("a").unwrap();
    assert!(a > 0.0);
}

#[test]
fn eval_wasm() {
    let engine = JsEngine::new();
    let result = engine.eval::<_, Value>(
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
        Ok(Value::Number(n)) if n == 7.0 => {}
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
    let _ = Value::String(str_1).coerce_number(&engine_2);
}

#[test]
fn user_data_drop() {
    let engine = JsEngine::new();
    let (count, data) = make_test_user_data();
    engine.set_user_data("data", data);
    drop(engine);
    assert_eq!(*count.borrow(), 1000);
}

#[test]
fn user_data_get() {
    let engine = JsEngine::new();
    let (_, data) = make_test_user_data();
    engine.set_user_data("data", data);
    assert!(engine.use_user_data::<_, TestUserData, _>("no-exist", |u| u.is_none()));
    assert!(engine.use_user_data::<_, usize, _>("data", |u| u.is_none()));

    engine.use_user_data::<_, TestUserData, _>("data", |data| {
        let data = data.unwrap();
        assert_eq!(data.get(), 0);
        data.increase();
        assert_eq!(data.get(), 1);
    });
}

#[test]
fn user_data_remove() {
    let engine = JsEngine::new();
    let (count, data) = make_test_user_data();
    engine.set_user_data("data", data);
    assert_eq!(*count.borrow(), 0);
    let data = engine.remove_user_data("data").unwrap();
    assert_eq!(*count.borrow(), 0);
    data.downcast_ref::<TestUserData>().unwrap().increase();
    assert_eq!(*count.borrow(), 1);
    drop(data);
    assert_eq!(*count.borrow(), 1000);
}

struct TestUserData {
    count: Rc<RefCell<usize>>,
}

impl TestUserData {
    fn increase(&self) {
        *self.count.borrow_mut() += 1;
    }

    fn get(&self) -> usize {
        *self.count.borrow()
    }
}

impl Drop for TestUserData {
    fn drop(&mut self) {
        *self.count.borrow_mut() = 1000;
    }
}

fn make_test_user_data() -> (Rc<RefCell<usize>>, TestUserData) {
    let count = Rc::new(RefCell::new(0));
    (count.clone(), TestUserData { count })
}
