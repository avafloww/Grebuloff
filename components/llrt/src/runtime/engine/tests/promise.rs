use crate::runtime::engine::*;

#[test]
fn test_type() {
    let engine = JsEngine::new();
    let resolved_val: JsValue = engine.eval("Promise.resolve(123)").unwrap();
    assert!(resolved_val.is_promise());

    let resolved_promise = if let JsValue::Promise(p) = resolved_val {
        p
    } else {
        unreachable!();
    };

    let state = resolved_promise.state().unwrap();
    if let PromiseState::Resolved(value) = state {
        assert_eq!(123, value.into::<i32>(&engine).unwrap());
    } else {
        panic!();
    }
}

#[test]
fn test_resolve() {
    let engine = JsEngine::new();

    let promise = engine.create_promise();
    match promise.state().unwrap() {
        PromiseState::Pending => {}
        _ => panic!(),
    }

    promise.resolve(123).unwrap();
    let state = promise.state().unwrap();
    if let PromiseState::Resolved(value) = state {
        assert_eq!(123, value.into::<i32>(&engine).unwrap());
    } else {
        panic!();
    }
}

#[test]
fn test_reject() {
    let engine = JsEngine::new();

    let promise = engine.create_promise();
    match promise.state().unwrap() {
        PromiseState::Pending => {}
        _ => panic!(),
    }

    promise.reject(123).unwrap();
    let state = promise.state().unwrap();
    if let PromiseState::Rejected(value) = state {
        assert_eq!(123, value.into::<i32>(&engine).unwrap());
    } else {
        panic!();
    }
}

#[tokio::test]
async fn test_async_resolve() {
    let engine = JsEngine::new();

    let promise = engine.create_promise();
    match promise.state().unwrap() {
        PromiseState::Pending => {}
        _ => panic!(),
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    match promise.state().unwrap() {
        PromiseState::Pending => {}
        _ => panic!(),
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    promise.resolve(456).unwrap();

    let state = promise.state().unwrap();
    if let PromiseState::Resolved(value) = state {
        assert_eq!(456, value.into::<i32>(&engine).unwrap());
    } else {
        panic!();
    }
}

#[tokio::test]
async fn test_async_reject() {
    let engine = JsEngine::new();

    let promise = engine.create_promise();
    match promise.state().unwrap() {
        PromiseState::Pending => {}
        _ => panic!(),
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    match promise.state().unwrap() {
        PromiseState::Pending => {}
        _ => panic!(),
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    promise.reject(456).unwrap();

    let state = promise.state().unwrap();
    if let PromiseState::Rejected(value) = state {
        assert_eq!(456, value.into::<i32>(&engine).unwrap());
    } else {
        panic!();
    }
}
