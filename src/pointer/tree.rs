use std::collections::HashMap;

use faststr::FastStr;

use crate::index::Index;

/// PointerTree is designed for [`get_many`][`crate::get_many`] and
/// [`get_many_unchecked`][`crate::get_many_unchecked`].
///
/// It is recommended to use `get_many` when you need to get multiple values from json. Instead of
/// using `get` multiple times.
///
/// # Examples
///
/// ```
/// # use sonic_rs::pointer;
/// # use sonic_rs::PointerTree;
///
/// let json = r#"
/// {"u": 123, "a": {"b" : {"c": [null, "found"]}}}"#;
///
/// // build a pointer tree, representing multiple json path
/// let mut tree = PointerTree::new();
///
/// tree.add_path(&["u"]);
/// tree.add_path(&["unknown_key"]);
/// tree.add_path(&pointer!["a", "b", "c", 1]);
///
/// let nodes = unsafe { sonic_rs::get_many_unchecked(json, &tree) };
///
/// match nodes {
///     Ok(vals) => {
///         assert_eq!(vals[0].as_ref().unwrap().as_raw_str(), "123");
///         assert!(vals[1].is_none());
///         assert_eq!(vals[2].as_ref().unwrap().as_raw_str(), "\"found\"");
///         for val in vals {
///             match val {
///                 Some(_) => println!("{}", val.as_ref().unwrap().as_raw_str()),
///                 None => println!("None"),
///             };
///         }
///     }
///     Err(e) => {
///         println!("err: {:?}", e)
///     }
/// }
/// ```

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
        Path::Item: Index,
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

impl PointerTreeNode {
    pub fn add_path<Path: IntoIterator>(&mut self, path: Path, order: usize)
    where
        Path::Item: Index,
    {
        let mut cur = self;
        let iter = path.into_iter();
        for p in iter {
            if let Some(key) = p.as_key() {
                if matches!(cur.children, PointerTreeInner::Empty) {
                    cur.children = PointerTreeInner::Key(HashMap::new());
                }
                cur = cur.insert_key(key)
            } else if let Some(index) = p.as_index() {
                if matches!(cur.children, PointerTreeInner::Empty) {
                    cur.children = PointerTreeInner::Index(HashMap::new());
                }
                cur = cur.insert_index(index)
            }
        }
        cur.order.push(order);
    }

    fn insert_key(&mut self, key: &str) -> &mut Self {
        if let PointerTreeInner::Key(mkey) = &mut self.children {
            mkey.entry(FastStr::new(key)).or_insert(Self::default())
        } else {
            unreachable!()
        }
    }

    fn insert_index(&mut self, idx: usize) -> &mut Self {
        if let PointerTreeInner::Index(midx) = &mut self.children {
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
        println!("tree is {tree:#?}");
    }
}
