use super::LazyValue;
use crate::error::Result;
use crate::input::JsonInput;
use crate::parser::Parser;
use crate::pointer::{PointerTrait, PointerTree};
use crate::reader::SliceRead;
use bytes::Bytes;
use faststr::FastStr;

/// get_from_str returns the raw value from json path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_str<Path: Iterator>(json: &str, path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get_from(json, path)
}

/// get_from_slice returns the raw value from json path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_slice<Path: Iterator>(json: &[u8], path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get_from(json, path)
}

/// get_from_bytes returns the raw value from json path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_bytes<Path: Iterator>(json: &Bytes, path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get_from(json, path)
}

/// get_from_bytes returns the raw value from json path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_faststr<Path: Iterator>(json: &FastStr, path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get_from(json, path)
}

/// get_from returns the raw value from json path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from<'de, Input, Path: Iterator>(
    json: Input,
    path: Path,
) -> Result<LazyValue<'de>>
where
    Input: JsonInput<'de>,
    Path::Item: PointerTrait,
{
    let slice = json.to_u8_slice();
    let reader = SliceRead::new(slice);
    let mut parser = Parser::new(reader);
    parser
        .get_from_with_iter(path.into_iter())
        .map(|sub| LazyValue::new(json.from_subset(sub)))
}

/// get_many returns the raw value from the PointerTree.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_many<'de, Input>(json: Input, tree: &PointerTree) -> Result<Vec<LazyValue<'de>>>
where
    Input: JsonInput<'de>,
{
    let slice = json.to_u8_slice();
    let reader = SliceRead::new(slice);
    let mut parser = Parser::new(reader);
    let out = parser.get_many(tree)?;
    Ok(out
        .into_iter()
        .map(|subset| LazyValue::new(json.from_subset(subset)))
        .collect())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{pointer, JsonPointer, PointerNode};
    use std::str::FromStr;

    #[test]
    fn test_get_from_json() {
        fn test_get(json: &str, path: &JsonPointer, expect: &str) {
            unsafe {
                let out = unsafe { get_from_str(json, path.iter()).unwrap() };
                assert_eq!(out.as_raw_str(), expect);

                let bytes = Bytes::copy_from_slice(json.as_bytes());
                let out = unsafe { get_from_bytes(&bytes, path.iter()).unwrap() };
                assert_eq!(out.as_raw_str(), expect);

                let fstr = faststr::FastStr::from_str(json).unwrap();
                let out = unsafe { get_from_faststr(&fstr, path.iter()).unwrap() };
                assert_eq!(out.as_raw_str(), expect);
                let out = unsafe { get_from(&fstr, path.iter()).unwrap() };
                assert_eq!(out.as_raw_str(), expect);

                let bytes = Bytes::copy_from_slice(json.as_bytes());
                let out = unsafe { get_from(&bytes, path.iter()).unwrap() };
                assert_eq!(out.as_raw_str(), expect);

                let out = unsafe { get_from(json, path.iter()).unwrap() };
                assert_eq!(out.as_raw_str(), expect);
            }
        }

        test_get(
            r#"{"a":"\n\tHello,\nworld!\n"}"#,
            &pointer!["a"],
            r#""\n\tHello,\nworld!\n""#,
        );
        test_get(
            r#"{"a":"\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\"}"#,
            &pointer!["a"],
            r#""\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\""#,
        );
        test_get(
            r#"{"":"\"\\\\\\\\\\\\\\\\\\\\\\\\\\\\\""}"#,
            &pointer![""],
            r#""\"\\\\\\\\\\\\\\\\\\\\\\\\\\\\\"""#,
        );
        test_get("[1, null, 2, 3]", &pointer![], "[1, null, 2, 3]");
        test_get("[[1], [2, 3], [4, 5, 6]]", &pointer![2, 2], "6");
        test_get(
            r#"{"a":{"b":{"c":"hello, world!"}}}"#,
            &pointer!["a", "b", "c"],
            r#""hello, world!""#,
        );
    }

    #[test]
    fn test_get_from_json_with_iter() {
        fn test_str_path(json: &str, path: &[&str], expect: &str) {
            let out = unsafe { get_from(json, path.iter()).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
        }

        fn test_faststr_path(json: FastStr, path: &[FastStr], expect: FastStr) {
            let out = unsafe { get_from(&json, path.iter()).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
        }

        fn test_index_path(json: &str, path: &[usize], expect: &str) {
            let out = unsafe { get_from(json, path.iter()).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
        }
        test_str_path(
            r#"{"a":{"b":{"c":"hello, world!"}}}"#,
            &["a", "b", "c"],
            r#""hello, world!""#,
        );

        test_faststr_path(
            r#"{"a":{"b":{"c":"hello, world!"}}}"#.into(),
            &["a".into(), "b".into(), "c".into()],
            r#""hello, world!""#.into(),
        );

        test_index_path(
            r#"["a", ["b" , ["c", "hello, world!"]]]"#,
            &[1, 1, 1],
            r#""hello, world!""#,
        );
    }

    fn build_tree() -> PointerTree {
        let mut tree = PointerTree::default();
        tree.add_path(["a", "a_b", "a_b_c"].iter()); // 0
        tree.add_path(["a", "a_b"].iter()); // 1
        tree.add_path(pointer!["a", "a_a", 1].iter()); // 2
        tree.add_path(pointer!["a"].iter()); // 3
        tree.add_path(pointer!["a"].iter()); // 4
        tree.add_path(pointer!["b", 2].iter()); // 5
        tree.add_path(pointer![].iter()); // 6
        assert_eq!(tree.count(), 7);
        tree
    }

    #[test]
    fn test_get_many() {
        let json = Bytes::from(
            r#"{
                "b": [0, 1, true],
                "a": {
                    "a_b":{
                        "a_b_c":"hello, world!"
                    },
                    "a_a": [0, 1, 2]
                }
            }
            "#,
        );

        let tree = build_tree();
        let many = unsafe { get_many(&json, &tree).unwrap() };
        assert_eq!(many[0].as_raw_slice(), b"\"hello, world!\"");
        assert_eq!(
            many[1].as_raw_slice(),
            b"{\n                        \"a_b_c\":\"hello, world!\"\n                    }"
        );
        assert_eq!(many[2].as_raw_slice(), b"1");
        assert_eq!(many[3].as_raw_slice(), b"{\n                    \"a_b\":{\n                        \"a_b_c\":\"hello, world!\"\n                    },\n                    \"a_a\": [0, 1, 2]\n                }");
        assert_eq!(many[4].as_raw_slice(), many[3].as_raw_slice());
        assert_eq!(many[5].as_raw_slice(), b"true");
        // we have strip the leading or trailing spaces
        assert_eq!(many[6].as_raw_slice(), b"{\n                \"b\": [0, 1, true],\n                \"a\": {\n                    \"a_b\":{\n                        \"a_b_c\":\"hello, world!\"\n                    },\n                    \"a_a\": [0, 1, 2]\n                }\n            }");
    }
}
