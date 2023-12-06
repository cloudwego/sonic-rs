use faststr::FastStr;

/// PointerTrait is a trait for the node in json pointer path.
pub trait PointerTrait {
    fn key(&self) -> Option<&str>;
    fn index(&self) -> Option<usize>;
}

impl From<usize> for PointerNode {
    fn from(value: usize) -> Self {
        PointerNode::Index(value)
    }
}

impl From<&'static str> for PointerNode {
    fn from(value: &'static str) -> Self {
        PointerNode::Key(FastStr::from_static_str(value))
    }
}

/// JsonPointer reprsents a json path.
/// You can use `jsonpointer!["a", "b", 1]` represent a json path.
/// It means that we will get the json field from `.a.b.1`.
/// Note: the key in jsonpointer should be unescaped.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PointerNode {
    Key(FastStr),
    Index(usize),
}

impl PointerTrait for &PointerNode {
    fn index(&self) -> Option<usize> {
        match self {
            PointerNode::Index(idx) => Some(*idx),
            PointerNode::Key(_) => None,
        }
    }

    fn key(&self) -> Option<&str> {
        match self {
            PointerNode::Key(key) => Some(key),
            PointerNode::Index(_) => None,
        }
    }
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

impl<'a> PointerTrait for &'a FastStr {
    fn index(&self) -> Option<usize> {
        None
    }

    fn key(&self) -> Option<&str> {
        Some(self.as_str())
    }
}

impl<'a> PointerTrait for &'a &str {
    fn index(&self) -> Option<usize> {
        None
    }

    fn key(&self) -> Option<&str> {
        Some(self)
    }
}

impl<'a> PointerTrait for &'a usize {
    fn index(&self) -> Option<usize> {
        Some(**self)
    }

    fn key(&self) -> Option<&str> {
        None
    }
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
