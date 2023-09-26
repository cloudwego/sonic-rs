use super::PointerTarit;
use faststr::FastStr;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct PointerTree {
    count: usize, // the count of path
    pub(crate) root: PointerTreeNode,
}

impl PointerTree {
    pub fn new() -> Self {
        Self::default()
    }

    // we build tree and return value according by the order of path
    pub fn add_path<Path: Iterator>(&mut self, path: Path)
    where
        Path::Item: PointerTarit,
    {
        self.root.add_path(path, self.count);
        self.count += 1;
    }

    pub fn count(&self) -> usize {
        self.count
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

use PointerTreeInner::*;

impl PointerTreeNode {
    pub fn add_path<Path: IntoIterator>(&mut self, path: Path, order: usize)
    where
        Path::Item: PointerTarit,
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
            match mkey.entry(FastStr::new(key)) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => v.insert(Self::default()),
            }
        } else {
            unreachable!()
        }
    }

    fn insert_index(&mut self, idx: usize) -> &mut Self {
        if let Index(midx) = &mut self.children {
            match midx.entry(idx) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => v.insert(Self::default()),
            }
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
    use crate::PointerNode;

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
        assert_eq!(tree.count(), 7);
        println!("tree is {:#?}", tree);
    }
}
