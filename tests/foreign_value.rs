//! `Value`, `Object` and `Array` read through deserializers that forward
//! `deserialize_newtype_struct` to `deserialize_any`, such as serde's own `de::value` ones.

use std::collections::BTreeMap;

use serde::{
    de::value::{
        BoolDeserializer, Error, F64Deserializer, I64Deserializer, MapDeserializer,
        SeqDeserializer, StrDeserializer, U64Deserializer, UnitDeserializer,
    },
    Deserialize,
};
use sonic_rs::{json, Array, JsonValueTrait, Object, Value};

#[test]
fn value_from_scalars() {
    let v = Value::deserialize(BoolDeserializer::<Error>::new(true)).unwrap();
    assert_eq!(v.as_bool(), Some(true));
    let v = Value::deserialize(I64Deserializer::<Error>::new(-7)).unwrap();
    assert_eq!(v.as_i64(), Some(-7));
    let v = Value::deserialize(U64Deserializer::<Error>::new(u64::MAX)).unwrap();
    assert_eq!(v.as_u64(), Some(u64::MAX));
    let v = Value::deserialize(F64Deserializer::<Error>::new(1.5)).unwrap();
    assert_eq!(v.as_f64(), Some(1.5));
    let v = Value::deserialize(StrDeserializer::<Error>::new("hi")).unwrap();
    assert_eq!(v.as_str(), Some("hi"));
    let v = Value::deserialize(UnitDeserializer::<Error>::new()).unwrap();
    assert!(v.is_null());
}

#[test]
fn value_from_containers() {
    let seq = SeqDeserializer::<_, Error>::new(vec![1u64, 2, 3].into_iter());
    assert_eq!(Value::deserialize(seq).unwrap(), json!([1, 2, 3]));

    let entries = BTreeMap::from([("a".to_string(), 1u64), ("b".to_string(), 2)]);
    let map = MapDeserializer::<_, Error>::new(entries.clone().into_iter());
    assert_eq!(Value::deserialize(map).unwrap(), json!({"a": 1, "b": 2}));

    let map = MapDeserializer::<_, Error>::new(entries.into_iter());
    let object = Object::deserialize(map).unwrap();
    assert_eq!(object.get(&"b").and_then(|v| v.as_u64()), Some(2));

    let seq = SeqDeserializer::<_, Error>::new(vec!["x", "y"].into_iter());
    let array = Array::deserialize(seq).unwrap();
    assert_eq!(array.len(), 2);
    assert_eq!(array[1].as_str(), Some("y"));
}
