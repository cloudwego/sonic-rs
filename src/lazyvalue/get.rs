use super::LazyValue;
use crate::error::Result;
use crate::input::JsonInput;
use crate::parser::Parser;
use crate::pointer::{PointerTrait, PointerTree};
use crate::reader::Reader;
use crate::reader::SliceRead;
use crate::util::utf8::from_utf8;
use bytes::Bytes;
use faststr::FastStr;

/// get_from_str_unchecked returns the raw value from path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_str_unchecked<Path: IntoIterator>(
    json: &str,
    path: Path,
) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get_unchecked(json, path)
}

/// get_from_slice_unchecked returns the raw value from path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_slice_unchecked<Path: IntoIterator>(
    json: &[u8],
    path: Path,
) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get_unchecked(json, path)
}

/// get_from_bytes_unchecked returns the raw value from path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_bytes_unchecked<Path: IntoIterator>(
    json: &Bytes,
    path: Path,
) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get_unchecked(json, path)
}

/// get_from_bytes_unchecked returns the raw value from path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_faststr_unchecked<Path: IntoIterator>(
    json: &FastStr,
    path: Path,
) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get_unchecked(json, path)
}

/// get_unchecked returns the raw value from path.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_unchecked<'de, Input, Path: IntoIterator>(
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
        .get_from_with_iter(path)
        .map(|sub| LazyValue::new(json.from_subset(sub)))
}

/// get_many_unchecked returns the raw value from the PointerTree. The result is a Vec<LazyValue>.
/// And the order of the result is the same as the order of the tree. If the tree is empty,
/// it will return the whole json.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_many_unchecked<'de, Input>(
    json: Input,
    tree: &PointerTree,
) -> Result<Vec<LazyValue<'de>>>
where
    Input: JsonInput<'de>,
{
    let slice = json.to_u8_slice();
    let reader = SliceRead::new(slice);
    let mut parser = Parser::new(reader);
    let out = parser.get_many(tree, false)?;
    Ok(out
        .into_iter()
        .map(|subset| LazyValue::new(json.from_subset(subset)))
        .collect())
}

/// Returns the raw value from path.
///
pub fn get_from_str<Path: IntoIterator>(json: &str, path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get(json, path)
}

/// Returns the raw value from path.
///
pub fn get_from_slice<Path: IntoIterator>(json: &[u8], path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get(json, path)
}

/// Returns the raw value from path.
///
pub fn get_from_bytes<Path: IntoIterator>(json: &Bytes, path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get(json, path)
}

/// Returns the raw value from path.
pub fn get_from_faststr<Path: IntoIterator>(json: &FastStr, path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: PointerTrait,
{
    get(json, path)
}

/// Returns the raw value from path.
///
pub fn get<'de, Input, Path: IntoIterator>(json: Input, path: Path) -> Result<LazyValue<'de>>
where
    Input: JsonInput<'de>,
    Path::Item: PointerTrait,
{
    let slice = json.to_u8_slice();
    let reader = SliceRead::new(slice);
    let mut parser = Parser::new(reader);
    let node = parser
        .get_from_with_iter_checked(path)
        .map(|sub| LazyValue::new(json.from_subset(sub)))?;

    // validate the utf-8 if slice
    let index = parser.read.index();
    if json.need_utf8_valid() {
        from_utf8(&slice[..index])?;
    }
    Ok(node)
}

/// get_many returns the raw value from the PointerTree. The result is a Vec<LazyValue>.
/// And the order of the result is the same as the order of the tree. If the tree is empty,
/// it will return the whole json.
pub fn get_many<'de, Input>(json: Input, tree: &PointerTree) -> Result<Vec<LazyValue<'de>>>
where
    Input: JsonInput<'de>,
{
    let slice = json.to_u8_slice();
    let reader = SliceRead::new(slice);
    let mut parser = Parser::new(reader);
    let out = parser.get_many(tree, true)?;
    let nodes = out
        .into_iter()
        .map(|subset| LazyValue::new(json.from_subset(subset)))
        .collect();

    // validate the utf-8 if slice
    let index = parser.read.index();
    if json.need_utf8_valid() {
        from_utf8(&slice[..index])?;
    }
    Ok(nodes)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{pointer, JsonPointer, PointerNode};
    use std::str::{from_utf8_unchecked, FromStr};

    #[test]
    fn test_get_from_json() {
        fn test_get_ok(json: &str, path: &JsonPointer, expect: &str) {
            let out = unsafe { get_from_str_unchecked(json, path).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
            let out = get_from_str(json, path).unwrap();
            assert_eq!(out.as_raw_str(), expect);

            let bytes = Bytes::copy_from_slice(json.as_bytes());
            let out = unsafe { get_from_bytes_unchecked(&bytes, path).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
            let out = get_from_bytes(&bytes, path).unwrap();
            assert_eq!(out.as_raw_str(), expect);

            let fstr = faststr::FastStr::from_str(json).unwrap();
            let out = unsafe { get_from_faststr_unchecked(&fstr, path).unwrap() };
            assert_eq!(out.as_raw_str(), expect);

            // test get from traits
            let out = unsafe { get_unchecked(&fstr, path).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
            let out = get(&fstr, path).unwrap();
            assert_eq!(out.as_raw_str(), expect);

            let bytes = Bytes::copy_from_slice(json.as_bytes());
            let out = unsafe { get_unchecked(&bytes, path).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
            let out = get(&bytes, path).unwrap();
            assert_eq!(out.as_raw_str(), expect);

            let out = unsafe { get_unchecked(json.as_bytes(), path).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
            let out = get(json.as_bytes(), path).unwrap();
            assert_eq!(out.as_raw_str(), expect);

            // test for SIMD codes
            let json = json.to_string() + &" ".repeat(1000);
            let out = unsafe { get_unchecked(&json, path).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
            let out = get(&json, path).unwrap();
            assert_eq!(out.as_raw_str(), expect);
        }

        test_get_ok(
            r#"{"a":"\n\tHello,\nworld!\n"}"#,
            &pointer!["a"],
            r#""\n\tHello,\nworld!\n""#,
        );
        test_get_ok(
            r#"{"a":"\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\"}"#,
            &pointer!["a"],
            r#""\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\""#,
        );
        test_get_ok(
            r#"{"":"\"\\\\\\\\\\\\\\\\\\\\\\\\\\\\\""}"#,
            &pointer![""],
            r#""\"\\\\\\\\\\\\\\\\\\\\\\\\\\\\\"""#,
        );
        test_get_ok("[1, null, 2, 3]", &pointer![], "[1, null, 2, 3]");
        test_get_ok("[[1], [2, 3], [4, 5, 6]]", &pointer![2, 2], "6");
        test_get_ok(
            r#"{"a":{"b":{"c":"hello, world!"}}}"#,
            &pointer!["a", "b", "c"],
            r#""hello, world!""#,
        );
    }

    #[test]
    fn test_get_from_json_failed() {
        fn test_get_failed(json: &[u8], path: &JsonPointer) {
            let out = get_from_slice(json, path);
            assert!(out.is_err(), "json is {:?}", json);

            // test for SIMD codes
            let json = unsafe { from_utf8_unchecked(json) }.to_string() + &" ".repeat(1000);
            let out = get_from_slice(json.as_bytes(), path);
            assert!(out.is_err());
        }

        test_get_failed(br#"{"a":"\n\tHello,\nworld!\n"}"#, &pointer!["b"]);
        test_get_failed(br#"{"a":"\n\tHello,\nworld!\n"}"#, &pointer!["a", "b"]);
        test_get_failed(br#"{"a":"\n\tHello,\nworld!\n"}"#, &pointer!["a", 1]);
        test_get_failed(br#"{"a": ""invalid", "b":null}"#, &pointer!["a", "b"]);
        test_get_failed(br#"{"a": "", "b":["123]"}"#, &pointer!["a", "b"]);
        let data = [b'"', 0x32, 0x32, 0x32, 0x80, 0x90, b'"'];
        test_get_failed(&data, &pointer![]);
    }

    #[test]
    fn test_get_from_json_with_iter() {
        fn test_str_path(json: &str, path: &[&str], expect: &str) {
            let out = unsafe { get_unchecked(json, path).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
        }

        fn test_faststr_path(json: FastStr, path: &[FastStr], expect: FastStr) {
            let out = unsafe { get_unchecked(&json, path).unwrap() };
            assert_eq!(out.as_raw_str(), expect);
        }

        fn test_index_path(json: &str, path: &[usize], expect: &str) {
            let out = unsafe { get_unchecked(json, path).unwrap() };
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
        assert_eq!(tree.size(), 7);
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
        test_many_ok(unsafe { get_many_unchecked(&json, &tree).unwrap() });
        test_many_ok(get_many(&json, &tree).unwrap());

        fn test_many_ok(many: Vec<LazyValue<'_>>) {
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
}
