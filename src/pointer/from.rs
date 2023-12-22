use faststr::FastStr;

use crate::PointerNode;

impl From<usize> for PointerNode {
    fn from(value: usize) -> Self {
        PointerNode::Index(value)
    }
}

impl From<&usize> for PointerNode {
    fn from(value: &usize) -> Self {
        PointerNode::Index(*value)
    }
}

impl From<&str> for PointerNode {
    fn from(value: &str) -> Self {
        PointerNode::Key(FastStr::new(value))
    }
}

impl From<FastStr> for PointerNode {
    fn from(value: FastStr) -> Self {
        PointerNode::Key(value)
    }
}

impl From<&FastStr> for PointerNode {
    fn from(value: &FastStr) -> Self {
        PointerNode::Key(value.clone())
    }
}
