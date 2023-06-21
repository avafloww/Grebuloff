use crate::runtime::engine::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[test]
fn option() {
    let engine = JsEngine::new();

    let none_val = None::<()>.to_value(&engine).unwrap();
    assert!(none_val.is_null());
    let num_val = Some(123).to_value(&engine).unwrap();
    assert!(num_val.is_number());

    let none: Option<()> = FromJsValue::from_value(none_val.clone(), &engine).unwrap();
    assert_eq!(none, None::<()>);
    let none: Option<()> = FromJsValue::from_value(JsValue::Null, &engine).unwrap();
    assert_eq!(none, None::<()>);
    let none: Option<()> = FromJsValue::from_value(JsValue::Undefined, &engine).unwrap();
    assert_eq!(none, None::<()>);
    let some_num: Option<usize> = FromJsValue::from_value(num_val.clone(), &engine).unwrap();
    assert_eq!(some_num, Some(123));
    let num: usize = FromJsValue::from_value(num_val.clone(), &engine).unwrap();
    assert_eq!(num, 123);
    let num_zero: usize = FromJsValue::from_value(none_val.clone(), &engine).unwrap();
    assert_eq!(num_zero, 0);
}

#[test]
fn variadic() {
    let engine = JsEngine::new();
    let values = (true, false, true).to_values(&engine).unwrap();

    let var: Variadic<bool> = FromJsValues::from_values(values.clone(), &engine).unwrap();
    assert_eq!(*var, vec![true, false, true]);

    let values = (true, Variadic::from_vec(vec![false, true]))
        .to_values(&engine)
        .unwrap();
    let var: Variadic<bool> = FromJsValues::from_values(values.clone(), &engine).unwrap();
    assert_eq!(*var, vec![true, false, true]);
}

#[test]
fn tuple() {
    let engine = JsEngine::new();
    let values = (true, false, true).to_values(&engine).unwrap();

    let out: (bool, bool, bool) = FromJsValues::from_values(values.clone(), &engine).unwrap();
    assert_eq!((true, false, true), out);

    let out: (bool, bool) = FromJsValues::from_values(values.clone(), &engine).unwrap();
    assert_eq!((true, false), out);

    type Overflow = (bool, bool, bool, JsValue, JsValue);
    let (a, b, c, d, e): Overflow = FromJsValues::from_values(values.clone(), &engine).unwrap();
    assert_eq!((true, false, true), (a, b, c));
    assert!(d.is_undefined());
    assert!(e.is_undefined());

    type VariadicTuple = (bool, Variadic<bool>);
    let (a, var): VariadicTuple = FromJsValues::from_values(values.clone(), &engine).unwrap();
    assert_eq!(true, a);
    assert_eq!(*var, vec![false, true]);

    type VariadicOver = (bool, bool, bool, bool, Variadic<bool>);
    let (a, b, c, d, var): VariadicOver = FromJsValues::from_values(values.clone(), &engine).unwrap();
    assert_eq!((true, false, true, false), (a, b, c, d));
    // assert_eq!(*var, vec![]); // todo: this test fails on nightly 2023-06-02
}

#[test]
fn hash_map() {
    let mut map = HashMap::new();
    map.insert(1, 2);
    map.insert(3, 4);
    map.insert(5, 6);

    let engine = JsEngine::new();
    let list = map
        .to_value(&engine)
        .unwrap()
        .into::<JsObject>(&engine)
        .unwrap()
        .properties(false)
        .unwrap()
        .map(|p| {
            let result: (usize, usize) = p.unwrap();
            result
        })
        .collect::<Vec<_>>();
    assert_eq!(list, vec![(1, 2), (3, 4), (5, 6)]);
}

#[test]
fn btree_map() {
    let mut map = BTreeMap::new();
    map.insert(1, 2);
    map.insert(3, 4);
    map.insert(5, 6);

    let engine = JsEngine::new();
    let list = map
        .to_value(&engine)
        .unwrap()
        .into::<JsObject>(&engine)
        .unwrap()
        .properties(false)
        .unwrap()
        .map(|p| {
            let result: (usize, usize) = p.unwrap();
            result
        })
        .collect::<Vec<_>>();
    assert_eq!(list, vec![(1, 2), (3, 4), (5, 6)]);
}

#[test]
fn vec() {
    let vec = vec![1, 2, 3];
    let engine = JsEngine::new();
    let list: JsResult<Vec<usize>> = vec
        .to_value(&engine)
        .unwrap()
        .into::<JsArray>(&engine)
        .unwrap()
        .elements()
        .collect();
    assert_eq!(list.unwrap(), vec![1, 2, 3]);
}

#[test]
fn btree_set() {
    let btree_set: BTreeSet<_> = vec![1, 2, 3].into_iter().collect();
    let engine = JsEngine::new();
    let list: JsResult<BTreeSet<usize>> = btree_set
        .to_value(&engine)
        .unwrap()
        .into::<JsArray>(&engine)
        .unwrap()
        .elements()
        .collect();
    assert_eq!(list.unwrap(), vec![1, 2, 3].into_iter().collect());
}

#[test]
fn hash_set() {
    let hash_set: HashSet<_> = vec![1, 2, 3].into_iter().collect();
    let engine = JsEngine::new();
    let list: JsResult<HashSet<usize>> = hash_set
        .to_value(&engine)
        .unwrap()
        .into::<JsArray>(&engine)
        .unwrap()
        .elements()
        .collect();
    assert_eq!(list.unwrap(), vec![1, 2, 3].into_iter().collect());
}
