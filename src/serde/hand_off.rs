//! Move a parsed value to its serde visitor without exposing the value's representation.
//! Sonic's private receivers take the value directly after a type-equality check.
//! Wrapped and custom receivers use a one-byte stack marker to claim the current
//! hand-off once while the call is active.

use std::{cell::Cell, mem::ManuallyDrop, thread::LocalKey};

use serde::de::{self, Visitor};

use crate::{
    lazyvalue::de::OwnedVisitor,
    value::{de::ValueVisitor, node::Value},
    OwnedLazyValue,
};

// The guard owns cleanup, even on unwind. Keeping the slot free of thread-local destructors
// also lets a hand-off run inside another thread-local's destructor.
type Entry<T> = ManuallyDrop<Option<(*const u8, T)>>;
type Slot<T> = Cell<Entry<T>>;

struct Output<'de>(std::marker::PhantomData<&'de ()>);
impl<'de, V: Visitor<'de>> try_specialize::TypeFn<V> for Output<'de> {
    type Output = V::Value;
}
// SAFETY: Both receivers are private unit structs with no lifetime parameters.
unsafe impl try_specialize::LifetimeFree for ValueVisitor {}
unsafe impl try_specialize::LifetimeFree for OwnedVisitor {}

pub(crate) trait HandOff: Sized + 'static {
    type Receiver: try_specialize::LifetimeFree + for<'a> Visitor<'a, Value = Self>;
    fn slot() -> &'static LocalKey<Slot<Self>>;
}

thread_local! {
    static VALUE: Slot<Value> = const { Cell::new(ManuallyDrop::new(None)) };
    static OWNED_LAZY_VALUE: Slot<OwnedLazyValue> = const { Cell::new(ManuallyDrop::new(None)) };
}

impl HandOff for Value {
    type Receiver = ValueVisitor;
    fn slot() -> &'static LocalKey<Slot<Self>> {
        &VALUE
    }
}

impl HandOff for OwnedLazyValue {
    type Receiver = OwnedVisitor;
    fn slot() -> &'static LocalKey<Slot<Self>> {
        &OWNED_LAZY_VALUE
    }
}

struct Restore<'a, T> {
    slot: &'a Slot<T>,
    previous: Option<(*const u8, T)>,
}

impl<T> Drop for Restore<'_, T> {
    fn drop(&mut self) {
        let leftover = self.slot.replace(ManuallyDrop::new(self.previous.take()));
        drop(ManuallyDrop::into_inner(leftover));
    }
}

pub(crate) fn hand_off<'de, T, V, E>(value: T, visitor: V) -> Result<V::Value, E>
where
    T: HandOff,
    V: Visitor<'de>,
    E: de::Error,
{
    if let Some(eq) = try_specialize::Specialization::<V, T::Receiver>::try_new() {
        // Equality of the receiver types also proves equality of their associated outputs.
        // Dropping a private unit visitor has no side effects.
        return Ok(eq.rev().map::<Output<'de>>().specialize(value));
    }
    let marker = [0u8];
    T::slot().with(|slot| {
        let previous = slot.replace(ManuallyDrop::new(Some((marker.as_ptr(), value))));
        let _restore = Restore {
            slot,
            previous: ManuallyDrop::into_inner(previous),
        };
        visitor.visit_bytes(&marker)
    })
}

pub(crate) fn take<T: HandOff>(bytes: &[u8]) -> Option<T> {
    if bytes.len() != 1 {
        return None;
    }
    T::slot().with(
        |slot| match ManuallyDrop::into_inner(slot.replace(ManuallyDrop::new(None))) {
            Some((marker, value)) if marker == bytes.as_ptr() => Some(value),
            previous => {
                slot.set(ManuallyDrop::new(previous));
                None
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use serde::de::value::Error;

    use super::*;

    #[test]
    fn custom_receiver_with_the_same_output_keeps_its_callback() {
        struct Reject<'a>(&'a Cell<bool>);
        impl<'de> Visitor<'de> for Reject<'_> {
            type Value = Value;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a value rejected by the wrapper")
            }

            fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Value, E> {
                self.0.set(true);
                assert!(take::<Value>(bytes).is_some());
                Err(E::custom("wrapper rejected value"))
            }
        }

        let called = Cell::new(false);
        let result: Result<Value, Error> = hand_off(Value::new_null(), Reject(&called));
        assert!(called.get());
        assert_eq!(result.unwrap_err().to_string(), "wrapper rejected value");
    }

    struct Nested<T>(T);
    impl<'de, T: HandOff> Visitor<'de> for Nested<T> {
        type Value = T;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an outer hand-off")
        }
        fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<T, E> {
            assert!(take::<T>(&[0]).is_none());
            let panic = catch_unwind(AssertUnwindSafe(|| {
                struct Panics;
                impl<'de> Visitor<'de> for Panics {
                    type Value = ();
                    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        f.write_str("a nested hand-off")
                    }
                    fn visit_bytes<E: de::Error>(self, _: &[u8]) -> Result<(), E> {
                        panic!("nested visitor");
                    }
                }
                let _: Result<(), Error> = hand_off(self.0, Panics);
            }));
            assert!(panic.is_err());
            let outer = take::<T>(bytes).expect("outer hand-off restored after nested panic");
            assert!(take::<T>(bytes).is_none());
            Ok(outer)
        }
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn nested_panic_restores_outer_value() {
        let result: Result<Value, Error> =
            hand_off(Value::new_bool(true), Nested(Value::new_null()));
        assert!(crate::JsonValueTrait::as_bool(&result.unwrap()).unwrap());
        let result: Result<OwnedLazyValue, Error> = hand_off(
            OwnedLazyValue::from(true),
            Nested(OwnedLazyValue::default()),
        );
        assert!(crate::JsonValueTrait::as_bool(&result.unwrap()).unwrap());
        VALUE.with(|slot| {
            assert!(ManuallyDrop::into_inner(slot.replace(ManuallyDrop::new(None))).is_none())
        });
    }
}
