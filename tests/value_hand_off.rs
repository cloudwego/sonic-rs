//! `Value` and `OwnedLazyValue` never take a byte string from another deserializer as a value.

use serde::{
    de::value::{BytesDeserializer, Error},
    Deserialize,
};
use sonic_rs::{JsonValueTrait, OwnedLazyValue, Value};

/// Byte strings of every interesting length: empty, short, and exactly the size of the type.
fn byte_strings<T>() -> Vec<Vec<u8>> {
    let size = std::mem::size_of::<T>();
    vec![vec![], vec![1, 2, 3], vec![0; size], vec![0xff; size]]
}

#[test]
fn value_refuses_foreign_bytes() {
    for bytes in byte_strings::<Value>() {
        let err = Value::deserialize(BytesDeserializer::<Error>::new(&bytes)).unwrap_err();
        assert!(err.to_string().contains("byte array"), "{err}");
    }
}

#[test]
fn owned_lazy_value_refuses_foreign_bytes() {
    for bytes in byte_strings::<OwnedLazyValue>() {
        let err = OwnedLazyValue::deserialize(BytesDeserializer::<Error>::new(&bytes)).unwrap_err();
        assert!(err.to_string().contains("byte array"), "{err}");
    }
}

#[test]
fn own_deserializers_still_hand_over_values() {
    let value: Value = sonic_rs::from_str(r#"{"a":[1,"x",null]}"#).unwrap();
    assert_eq!(value["a"][1].as_str(), Some("x"));

    let copy: Value = sonic_rs::from_value(&value).unwrap();
    assert_eq!(copy, value);

    let lazy: OwnedLazyValue = sonic_rs::from_str(r#"{"a":[1,"x",null]}"#).unwrap();
    assert_eq!(lazy.get("a").unwrap().get(1).unwrap().as_str(), Some("x"));

    // several values in one document, one after the other on the same thread
    let three: (Value, OwnedLazyValue, Value) = sonic_rs::from_str(r#"[1,{"b":2},"c"]"#).unwrap();
    assert_eq!(three.0.as_u64(), Some(1));
    assert_eq!(sonic_rs::to_string(&three.1).unwrap(), r#"{"b":2}"#);
    assert_eq!(three.2.as_str(), Some("c"));
}

// wasm targets abort on panic
#[cfg(not(target_family = "wasm"))]
mod unwind {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use serde::de::{self, Deserializer as _, Visitor};

    use super::*;

    /// A visitor that panics where `Value`'s own visitor would take the value.
    struct Panics;

    impl<'de> Visitor<'de> for Panics {
        type Value = ();

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("nothing")
        }

        fn visit_bytes<E: de::Error>(self, _: &[u8]) -> Result<(), E> {
            panic!("visitor panics");
        }
    }

    /// A visitor that unwinds while a value is being handed to it leaves nothing behind that a
    /// later byte string could pick up.
    #[test]
    fn panicking_visitor_leaves_nothing_behind() {
        // the private names under which `Value` and `OwnedLazyValue` ask sonic-rs's deserializer
        // for a ready value
        for name in ["$sonic_rs::private::Value", "$sonic::OwnedLv"] {
            let unwound = catch_unwind(AssertUnwindSafe(|| {
                let mut de = sonic_rs::Deserializer::from_str(r#"{"a":1}"#);
                let _ = (&mut de).deserialize_newtype_struct(name, Panics);
            }));
            assert!(unwound.is_err());
        }
        assert!(Value::deserialize(BytesDeserializer::<Error>::new(&[])).is_err());
        assert!(OwnedLazyValue::deserialize(BytesDeserializer::<Error>::new(&[])).is_err());
    }
}
