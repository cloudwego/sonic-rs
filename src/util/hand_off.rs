//! Move a parsed value to its serde visitor without exposing the value's representation.
//! The visitor receives a borrow of a one-byte stack marker. Only that slice can claim the
//! current hand-off, once, while the call is active.

use std::{cell::Cell, mem::ManuallyDrop, thread::LocalKey};

use serde::de::{self, Visitor};

use crate::{value::node::Value, OwnedLazyValue};

// The guard owns cleanup, even on unwind. Keeping the slot free of thread-local destructors
// also lets a hand-off run inside another thread-local's destructor.
type Entry<T> = ManuallyDrop<Option<(*const u8, T)>>;
type Slot<T> = Cell<Entry<T>>;

pub(crate) trait HandOff: Sized + 'static {
    fn slot() -> &'static LocalKey<Slot<Self>>;
}

thread_local! {
    static VALUE: Slot<Value> = const { Cell::new(ManuallyDrop::new(None)) };
    static OWNED_LAZY_VALUE: Slot<OwnedLazyValue> = const { Cell::new(ManuallyDrop::new(None)) };
}

impl HandOff for Value {
    fn slot() -> &'static LocalKey<Slot<Self>> {
        &VALUE
    }
}

impl HandOff for OwnedLazyValue {
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
