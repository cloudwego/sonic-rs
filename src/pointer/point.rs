use faststr::FastStr;

/// Represents a json pointer path. It can be created by [`pointer!`] macro.
pub type JsonPointer = [PointerNode];

/// Represents a node in a json pointer path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PointerNode {
    Key(FastStr),
    Index(usize),
}

/// Represents a json pointer path.
///
/// Used to indexing a [`Value`][`crate::Value`], [`LazyValue`][`crate::LazyValue`],
/// [`get`][`crate::get`] or [`get_unchecked`][`crate::get_unchecked`].
///
/// The path can includes both keys or indexes.
/// - keys: string-like, used to indexing an object.
/// - indexes: usize-like, used to indexing an array.
///
/// # Examples
///
/// ```
/// # use sonic_rs::pointer;
/// use sonic_rs::JsonValueTrait;
///
/// let value: sonic_rs::Value = sonic_rs::from_str(
///     r#"{
///     "foo": [
///        0,
///        1,
///        {
///          "bar": 123
///        }
///      ]
/// }"#,
/// )
/// .unwrap();
/// let path = pointer!["foo", 2, "bar"];
///
/// let got = value.pointer(&path).unwrap();
///
/// assert_eq!(got, 123);
/// ```
#[macro_export]
macro_rules! pointer {
    () => (
        ([] as [$crate::PointerNode; 0])
    );
    ($($x:expr),+ $(,)?) => (
        [$($crate::PointerNode::from($x)),+]
    );
}

#[cfg(test)]
mod test {
    #[test]
    fn test_json_pointer() {
        let pointers = pointer![];
        println!("{pointers:?}");
        let mut pointers = pointer![1, 2, 3, "foo", "bar"].to_vec();
        pointers.push(123.into());
        println!("{pointers:?}");
    }
}
