use super::PointerTrait;
use faststr::FastStr;
use std::collections::HashMap;

/// PointerTree is designed for `get_many`.
/// It is recommended to use `get_many` when you need to get multiple values from json.
/// Instead of using `get` multiple times.
#[derive(Debug, Default)]
pub struct PointerTree {
    // the count of path
    size: usize,
    // the root of tree
    pub(crate) root: PointerTreeNode,
}

impl PointerTree {
    /// Creat a empty tree. If `get_many` from empty tree, it will return the whole json.
    pub fn new() -> Self {
        Self::default()
    }

    /// we build tree and return value according by the order of path.
    /// Allow the repeated path.
    pub fn add_path<Path: IntoIterator>(&mut self, path: Path)
    where
        Path::Item: PointerTrait,
    {
        self.root.add_path(path, self.size);
        self.size += 1;
    }

    /// the count of nodes
    pub fn size(&self) -> usize {
        self.size
    }
}

#[derive(Debug, Default)]
pub(crate) enum PointerTreeInner {
    #[default]
    Empty,
    Key(MultiKey),
    Index(MultiIndex),
}

// Note: support the repeat path
#[derive(Debug, Default)]
pub(crate) struct PointerTreeNode {
    pub(crate) order: Vec<usize>,
    pub(crate) children: PointerTreeInner,
}
use PointerTreeInner::{Empty, Index, Key};

impl PointerTreeNode {
    pub fn add_path<Path: IntoIterator>(&mut self, path: Path, order: usize)
    where
        Path::Item: PointerTrait,
    {
        let mut cur = self;
        let iter = path.into_iter();
        for p in iter {
            if let Some(key) = p.key() {
                if matches!(cur.children, Empty) {
                    cur.children = Key(HashMap::new());
                }
                cur = cur.insert_key(key)
            } else if let Some(index) = p.index() {
                if matches!(cur.children, Empty) {
                    cur.children = Index(HashMap::new());
                }
                cur = cur.insert_index(index)
            }
        }
        cur.order.push(order);
    }

    fn insert_key(&mut self, key: &str) -> &mut Self {
        if let Key(mkey) = &mut self.children {
            mkey.entry(FastStr::new(key)).or_insert(Self::default())
        } else {
            unreachable!()
        }
    }

    fn insert_index(&mut self, idx: usize) -> &mut Self {
        if let Index(midx) = &mut self.children {
            midx.entry(idx).or_insert(Self::default())
        } else {
            unreachable!()
        }
    }
}

#[allow(clippy::mutable_key_type)]
pub(crate) type MultiKey = HashMap<FastStr, PointerTreeNode>;

pub(crate) type MultiIndex = HashMap<usize, PointerTreeNode>;

#[cfg(test)]
mod test {
    use super::*;
    use crate::pointer;

    #[test]
    fn test_tree() {
        let mut tree = PointerTree::default();
        tree.add_path(["a", "a_b", "a_b_c"].iter());
        tree.add_path(["a", "a_b"].iter());
        tree.add_path(pointer!["a", "a_a", 1].iter());
        tree.add_path(pointer!["a"].iter());
        tree.add_path(pointer!["a"].iter());
        tree.add_path(pointer!["b", 2].iter());
        tree.add_path(pointer![].iter());
        assert_eq!(tree.size(), 7);
        println!("tree is {:#?}", tree);
    }
}
