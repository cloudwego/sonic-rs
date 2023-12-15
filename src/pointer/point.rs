use faststr::FastStr;

/// JsonPointer reprsents a json path.
/// You can use `jsonpointer!["a", "b", 1]` represent a json path.
/// It means that we will get the json field from `.a.b.1`.
/// Note: the key in jsonpointer should be unescaped.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PointerNode {
    Key(FastStr),
    Index(usize),
}

pub type JsonPointer<'a> = Vec<PointerNode>;

#[macro_export]
macro_rules! pointer {
    () => (
        std::vec::Vec::<$crate::PointerNode>::new()
    );
    ($($x:expr),+ $(,)?) => (
        <[_]>::into_vec(
            std::boxed::Box::new([$($crate::PointerNode::from($x)),+])
        )
    );
}

#[cfg(test)]
mod test {
    #[test]
    fn test_json_pointer() {
        let pointers = pointer![];
        println!("{:?}", pointers);
        let mut pointers = pointer![1, 2, 3, "foo", "bar"];
        pointers.push(123.into());
        println!("{:?}", pointers);
    }
}
