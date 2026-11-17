//! Hands a value that sonic-rs has already built (a `Value` or an `OwnedLazyValue`) to the
//! matching visitor.
//!
//! serde has no way to pass an arbitrary Rust value to a visitor, so the value is moved into a
//! thread-local slot and the visitor is called with an empty byte string. Only the matching
//! visitor takes it back out of the slot ([`take`]). The bytes themselves are never read as a
//! value, so a byte string from any other deserializer (serde's
//! `BytesDeserializer`/`BorrowedBytesDeserializer`, or any deserializer that forwards
//! `deserialize_newtype_struct` to `deserialize_any` while holding bytes, e.g. via
//! `forward_to_deserialize_any!`) can only be refused, never turned into a value.
//!
//! The slot is restored when the visitor returns or unwinds, so a panicking visitor cannot leave
//! a value behind for a later, unrelated `visit_bytes` to pick up.

use std::{cell::Cell, thread::LocalKey};

use serde::de::{self, Visitor};

use crate::{value::node::Value, OwnedLazyValue};

/// A type that can be handed to its visitor through a thread-local slot.
pub(crate) trait HandOff: Sized + 'static {
    fn slot() -> &'static LocalKey<Cell<Option<Self>>>;
}

thread_local! {
    static VALUE: Cell<Option<Value>> = const { Cell::new(None) };
    static OWNED_LAZY_VALUE: Cell<Option<OwnedLazyValue>> = const { Cell::new(None) };
}

impl HandOff for Value {
    fn slot() -> &'static LocalKey<Cell<Option<Self>>> {
        &VALUE
    }
}

impl HandOff for OwnedLazyValue {
    fn slot() -> &'static LocalKey<Cell<Option<Self>>> {
        &OWNED_LAZY_VALUE
    }
}

/// Puts back what the slot held before a hand-off, and drops whatever the visitor did not take.
struct Restore<T: HandOff>(Option<T>);

impl<T: HandOff> Drop for Restore<T> {
    fn drop(&mut self) {
        let prev = self.0.take();
        // `try_with`: never panic here, this may run while unwinding. The leftover is dropped
        // after the slot is released.
        let _ = T::slot().try_with(|slot| slot.replace(prev));
    }
}

/// Passes `value` to `visitor`, as an empty byte string; the matching visitor gets it with
/// [`take`]. If the visitor does not take it, it is dropped here.
pub(crate) fn hand_off<'de, T, V, E>(value: T, visitor: V) -> Result<V::Value, E>
where
    T: HandOff,
    V: Visitor<'de>,
    E: de::Error,
{
    // Keep what the slot held (a hand-off in progress further up the stack) and put it back
    // afterwards, so nested hand-offs do not lose it.
    let prev = T::slot()
        .try_with(|slot| slot.replace(Some(value)))
        .map_err(|_| E::custom("thread-local storage is being destroyed"))?;
    let _restore = Restore::<T>(prev);
    visitor.visit_bytes(&[])
}

/// The value handed off by [`hand_off`], if `bytes` is its empty byte string and a hand-off is in
/// progress. Anything else is a byte string from some other deserializer.
pub(crate) fn take<T: HandOff>(bytes: &[u8]) -> Option<T> {
    if !bytes.is_empty() {
        return None;
    }
    T::slot().try_with(|slot| slot.take()).ok().flatten()
}
