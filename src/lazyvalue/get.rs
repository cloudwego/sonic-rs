use bytes::Bytes;
use faststr::FastStr;

use super::LazyValue;
use crate::{
    error::Result,
    index::Index,
    input::JsonInput,
    parser::Parser,
    pointer::PointerTree,
    reader::{Read, Reader},
    util::utf8::from_utf8,
};

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// # Safety
///
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
///
/// # Examples
/// ```
/// # use sonic_rs::get_from_str_unchecked;
///
/// // get from the &[&str]
/// let lv = unsafe { get_from_str_unchecked(r#"{"a": 1}"#, &["a"]).unwrap() };
/// assert_eq!(lv.as_raw_str(), "1");
///
/// // get from the &[usize]
/// let lv = unsafe { get_from_str_unchecked(r#"[0, 1, "two"]"#, &[2]).unwrap() };
/// assert_eq!(lv.as_raw_str(), "\"two\"");
///
/// // get from pointer!
/// use sonic_rs::pointer;
/// let lv =
///     unsafe { get_from_str_unchecked(r#"{"a": [0, 1, "two"]}"#, &pointer!["a", 2]).unwrap() };
/// assert_eq!(lv.as_raw_str(), "\"two\"");
///
/// // not found the field "a"
/// let lv = unsafe { get_from_str_unchecked(r#"{"a": 1}"#, &["b"]) };
/// assert!(lv.unwrap_err().is_not_found());
///
/// // the type of JSON is unmatched, expect it is a object
/// let lv = unsafe { get_from_str_unchecked(r#"[1, 2, 3]"#, &["b"]) };
/// assert!(lv.unwrap_err().is_unmatched_type());
/// ```
pub unsafe fn get_from_str_unchecked<Path: IntoIterator>(
    json: &str,
    path: Path,
) -> Result<LazyValue<'_>>
where
    Path::Item: Index,
{
    get_unchecked(json, path)
}

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// # Safety
///
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_bytes_unchecked<Path: IntoIterator>(
    json: &Bytes,
    path: Path,
) -> Result<LazyValue<'_>>
where
    Path::Item: Index,
{
    get_unchecked(json, path)
}

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// # Safety
///
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
///
/// # Examples
///
/// ```
/// # use sonic_rs::get_from_faststr_unchecked;
///
/// // get from the &[&str]
/// let fs = faststr::FastStr::new(r#"{"a": 1}"#);
/// let lv = unsafe { get_from_faststr_unchecked(&fs, &["a"]).unwrap() };
/// assert_eq!(lv.as_raw_str(), "1");
///
/// // not found the field "a"
/// let lv = unsafe { get_from_faststr_unchecked(&fs, &["b"]) };
/// assert!(lv.unwrap_err().is_not_found());
///
/// // get from the &[usize]
/// let fs = faststr::FastStr::new(r#"[0, 1, "two"]"#);
/// let lv = unsafe { get_from_faststr_unchecked(&fs, &[2]).unwrap() };
/// assert_eq!(lv.as_raw_str(), "\"two\"");
///
/// // get from pointer!
/// use sonic_rs::pointer;
/// let fs = faststr::FastStr::new(r#"{"a": [0, 1, "two"]}"#);
/// let lv = unsafe { get_from_faststr_unchecked(&fs, &pointer!["a", 2]).unwrap() };
/// assert_eq!(lv.as_raw_str(), "\"two\"");
///
/// /// the type of JSON is unmatched, expect it is a object
/// let lv = unsafe { get_from_faststr_unchecked(&fs, &pointer!["a", "get key from array"]) };
/// assert!(lv.unwrap_err().is_unmatched_type());
/// ```
pub unsafe fn get_from_faststr_unchecked<Path: IntoIterator>(
    json: &FastStr,
    path: Path,
) -> Result<LazyValue<'_>>
where
    Path::Item: Index,
{
    get_unchecked(json, path)
}

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
pub unsafe fn get_from_slice_unchecked<Path: IntoIterator>(
    json: &[u8],
    path: Path,
) -> Result<LazyValue<'_>>
where
    Path::Item: Index,
{
    get_unchecked(json, path)
}

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// The input `json` is allowed to be `&FastStr`, `&[u8]`, `&str`, `&String` or `&bytes::Bytes`.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
///
/// # Examples
/// ```
/// use faststr::FastStr;
/// use sonic_rs::get_unchecked;
///
/// let lv = unsafe { get_unchecked(r#"{"a": 1}"#, &["a"]).unwrap() };
/// assert_eq!(lv.as_raw_str(), "1");
///
/// /// not found the field "a"
/// let fs = FastStr::new(r#"{"a": 1}"#);
/// let lv = unsafe { get_unchecked(&fs, &["b"]) };
/// assert!(lv.unwrap_err().is_not_found());
/// ```
pub unsafe fn get_unchecked<'de, Input, Path: IntoIterator>(
    json: Input,
    path: Path,
) -> Result<LazyValue<'de>>
where
    Input: JsonInput<'de>,
    Path::Item: Index,
{
    let slice = json.to_u8_slice();
    let reader = Read::new(slice, false);
    let mut parser = Parser::new(reader);
    let (sub, status) = parser.get_from_with_iter_unchecked(path)?;
    Ok(LazyValue::new(json.from_subset(sub), status.into()))
}

/// get_many returns multiple fields from the `PointerTree`.
///
/// The result is a `Result<Vec<Option<LazyValue>>>`. The order of the `Vec` is same as the order of
/// the tree.
///
/// If a key is unknown, the value at the corresponding position in `Vec` will be None.
/// If json is invalid, or the field not be found, it will return a err.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
///
/// # Examples
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
///     }
///     Err(e) => {
///         panic!("err: {:?}", e)
///     }
/// }
/// ```
pub unsafe fn get_many_unchecked<'de, Input>(
    json: Input,
    tree: &PointerTree,
) -> Result<Vec<Option<LazyValue<'de>>>>
where
    Input: JsonInput<'de>,
{
    let slice = json.to_u8_slice();
    let reader = Read::new(slice, false);
    let mut parser = Parser::new(reader);
    parser.get_many(tree, false)
}

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// # Examples
/// ```
/// # use sonic_rs::get_from_str;
///
/// // get from the &[&str]
/// let lv = get_from_str(r#"{"a": 1}"#, &["a"]).unwrap();
/// assert_eq!(lv.as_raw_str(), "1");
///
/// // get from the &[usize]
/// let lv = get_from_str(r#"[0, 1, "two"]"#, &[2]).unwrap();
/// assert_eq!(lv.as_raw_str(), "\"two\"");
///
/// // get from pointer!
/// use sonic_rs::pointer;
/// let lv = get_from_str(r#"{"a": [0, 1, "two"]}"#, &pointer!["a", 2]).unwrap();
/// assert_eq!(lv.as_raw_str(), "\"two\"");
///
/// // not found the field "a"
/// let lv = get_from_str(r#"{"a": 1}"#, &["b"]);
/// assert!(lv.unwrap_err().is_not_found());
///
/// // the type of JSON is unmatched, expect it is a object
/// let lv = get_from_str(r#"[1, 2, 3]"#, &["b"]);
/// assert!(lv.unwrap_err().is_unmatched_type());
/// ```
pub fn get_from_str<Path: IntoIterator>(json: &str, path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: Index,
{
    get(json, path)
}

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// # Examples
/// ```
/// use sonic_rs::get_from_slice;
/// let lv = get_from_slice(br#"{"a": 1}"#, &["a"]).unwrap();
/// assert_eq!(lv.as_raw_str(), "1");
///
/// /// not found the field "a"
/// let lv = get_from_slice(br#"{"a": 1}"#, &["b"]);
/// assert!(lv.is_err());
/// ```
pub fn get_from_slice<Path: IntoIterator>(json: &[u8], path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: Index,
{
    get(json, path)
}

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// # Examples
/// ```
/// # use sonic_rs::get_from_bytes;
/// use bytes::Bytes;
///
/// let bs = Bytes::from(r#"{"a": 1}"#);
/// let lv = get_from_bytes(&bs, &["a"]).unwrap();
///
/// assert_eq!(lv.as_raw_str(), "1");
///
/// /// not found the field "a"
/// let lv = get_from_bytes(&bs, &["b"]);
/// assert!(lv.is_err());
/// ```
pub fn get_from_bytes<Path: IntoIterator>(json: &Bytes, path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: Index,
{
    get(json, path)
}

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// # Examples
///
/// ```
/// # use sonic_rs::get_from_faststr;
///
/// // get from the &[&str]
/// let fs = faststr::FastStr::new(r#"{"a": 1}"#);
/// let lv = get_from_faststr(&fs, &["a"]).unwrap();
/// assert_eq!(lv.as_raw_str(), "1");
///
/// // not found the field "a"
/// let lv = get_from_faststr(&fs, &["b"]);
/// assert!(lv.unwrap_err().is_not_found());
///
/// // get from the &[usize]
/// let fs = faststr::FastStr::new(r#"[0, 1, "two"]"#);
/// let lv = get_from_faststr(&fs, &[2]).unwrap();
/// assert_eq!(lv.as_raw_str(), "\"two\"");
///
/// // get from pointer!
/// use sonic_rs::pointer;
/// let fs = faststr::FastStr::new(r#"{"a": [0, 1, "two"]}"#);
/// let lv = get_from_faststr(&fs, &pointer!["a", 2]).unwrap();
/// assert_eq!(lv.as_raw_str(), "\"two\"");
///
/// /// the type of JSON is unmatched, expect it is a object
/// let lv = get_from_faststr(&fs, &pointer!["a", "get key from array"]);
/// assert!(lv.unwrap_err().is_unmatched_type());
/// ```
pub fn get_from_faststr<Path: IntoIterator>(json: &FastStr, path: Path) -> Result<LazyValue<'_>>
where
    Path::Item: Index,
{
    get(json, path)
}

/// Gets a field from a `path`. And return it as a [`Result<LazyValue>`][crate::LazyValue].
///
/// If not found, return an error. If the `path` is empty, return the whole JSON as a `LazyValue`.
///
/// The `Item` of the `path` should implement the [`Index`][crate::index::Index] trait.
///
/// The input `json` is allowed to be `&FastStr`, `&[u8]`, `&str`, `&String` or `&bytes::Bytes`.
///
/// # Safety
/// The JSON must be valid and well-formed, otherwise it may return unexpected result.
///
/// # Examples
/// ```
/// use bytes::Bytes;
/// use faststr::FastStr;
/// use sonic_rs::get;
///
/// let lv = get(r#"{"a": 1}"#, &["a"]).unwrap();
/// assert_eq!(lv.as_raw_str(), "1");
///
/// /// not found the field "a"
/// let fs = FastStr::new(r#"{"a": 1}"#);
/// let lv = get(&fs, &["b"]);
/// assert!(lv.is_err());
///
/// /// the JSON is invalid
/// let b = Bytes::from(r#"{"a": tru }"#);
/// let lv = get(&b, &["a"]);
/// assert!(lv.is_err());
/// ```
pub fn get<'de, Input, Path: IntoIterator>(json: Input, path: Path) -> Result<LazyValue<'de>>
where
    Input: JsonInput<'de>,
    Path::Item: Index,
{
    let slice = json.to_u8_slice();
    let reader = Read::new(slice, false);
    let mut parser = Parser::new(reader);
    let (sub, status) = parser.get_from_with_iter(path)?;
    let lv = LazyValue::new(json.from_subset(sub), status.into());

    // validate the utf-8 if slice
    let index = parser.read.index();
    if json.need_utf8_valid() {
        from_utf8(&slice[..index])?;
    }
    Ok(lv)
}

/// get_many returns multiple fields from the [`PointerTree`].
///
/// The result is a `Result<Vec<Option<LazyValue>>>`. The order of the `Vec` is same as the order of
/// the tree.
///
/// If a key is unknown, the value at the corresponding position in `Vec` will be None.  
/// If json is invalid, or the field not be found, it will return a err.
///
/// # Examples
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
/// let nodes = unsafe { sonic_rs::get_many(json, &tree) };
///
/// match nodes {
///     Ok(vals) => {
///         assert_eq!(vals[0].as_ref().unwrap().as_raw_str(), "123");
///         assert!(vals[1].is_none());
///         assert_eq!(vals[2].as_ref().unwrap().as_raw_str(), "\"found\"");
///     }
///     Err(e) => {
///         panic!("err: {:?}", e)
///     }
/// }
/// ```
pub fn get_many<'de, Input>(json: Input, tree: &PointerTree) -> Result<Vec<Option<LazyValue<'de>>>>
where
    Input: JsonInput<'de>,
{
    let slice = json.to_u8_slice();
    let reader = Read::new(slice, false);
    let mut parser = Parser::new(reader);
    let nodes = parser.get_many(tree, true)?;

    // validate the utf-8 if slice
    let index = parser.read.index();
    if json.need_utf8_valid() {
        from_utf8(&slice[..index])?;
    }
    Ok(nodes)
}

#[cfg(test)]
mod test {
    use std::str::{from_utf8_unchecked, FromStr};

    use super::*;
    use crate::{pointer, JsonPointer};

    fn test_get_ok(json: &str, path: &JsonPointer, expect: &str) {
        // get from str
        let out = unsafe { get_from_str_unchecked(json, path).unwrap() };
        assert_eq!(out.as_raw_str(), expect);
        let out = get_from_str(json, path).unwrap();
        assert_eq!(out.as_raw_str(), expect);

        // get from slice
        let out = unsafe { get_from_slice_unchecked(json.as_bytes(), path).unwrap() };
        assert_eq!(out.as_raw_str(), expect);
        let out = get_from_slice(json.as_bytes(), path).unwrap();
        assert_eq!(out.as_raw_str(), expect);

        // get from bytes
        let bytes = Bytes::copy_from_slice(json.as_bytes());
        let out = unsafe { get_from_bytes_unchecked(&bytes, path).unwrap() };
        assert_eq!(out.as_raw_str(), expect);
        let out = get_from_bytes(&bytes, path).unwrap();
        assert_eq!(out.as_raw_str(), expect);

        // get from FastStr
        let fstr = faststr::FastStr::from_str(json).unwrap();
        let out = unsafe { get_from_faststr_unchecked(&fstr, path).unwrap() };
        assert_eq!(out.as_raw_str(), expect);
        let out = get_from_faststr(&fstr, path).unwrap();
        assert_eq!(out.as_raw_str(), expect);

        // get from traits
        let out = unsafe { get_unchecked(&fstr, path).unwrap() };
        assert_eq!(out.as_raw_str(), expect);
        let out = get(&fstr, path).unwrap();
        assert_eq!(out.as_raw_str(), expect);

        // test for SIMD codes
        let json = json.to_string() + &" ".repeat(1000);
        let out = unsafe { get_unchecked(&json, path).unwrap() };
        assert_eq!(out.as_raw_str(), expect);
        let out = get(&json, path).unwrap();
        assert_eq!(out.as_raw_str(), expect);
    }

    #[test]
    fn test_get_from_empty_path() {
        test_get_ok(r#""""#, &pointer![], r#""""#);
        test_get_ok(r#"{}"#, &pointer![], r#"{}"#);
        test_get_ok(r#"[]"#, &pointer![], r#"[]"#);
        test_get_ok(r#"true"#, &pointer![], r#"true"#);
        test_get_ok(r#"false"#, &pointer![], r#"false"#);
        test_get_ok(r#"null"#, &pointer![], r#"null"#);
    }

    #[test]
    fn test_get_from_json() {
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
        test_get_ok(
            r#"{"a\"":{"b\"":{"c\"":"hello, world!"}}}"#,
            &pointer!["a\"", "b\"", "c\""],
            r#""hello, world!""#,
        );
    }

    #[test]
    fn test_get_from_json_with_trailings() {
        test_get_ok(
            r#"1230/(xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"#,
            &pointer![],
            r#"1230"#,
        );
    }

    #[test]
    fn test_get_from_json_failed() {
        fn test_get_failed(json: &[u8], path: &JsonPointer) {
            let out = get_from_slice(json, path);
            assert!(out.is_err(), "json is {json:?}");

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
        tree.add_path(pointer!["unknown_key"].iter()); // 7
        assert_eq!(tree.size(), 8);
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

        fn test_many_ok(many: Vec<Option<LazyValue<'_>>>) {
            assert_eq!(many[0].as_ref().unwrap().as_raw_str(), "\"hello, world!\"");
            assert_eq!(
                many[1].as_ref().unwrap().as_raw_str(),
                "{\n                        \"a_b_c\":\"hello, world!\"\n                    }"
            );
            assert_eq!(many[2].as_ref().unwrap().as_raw_str(), "1");
            assert_eq!(many[3].as_ref().unwrap().as_raw_str(), "{\n                    \"a_b\":{\n                        \"a_b_c\":\"hello, world!\"\n                    },\n                    \"a_a\": [0, 1, 2]\n                }");
            assert_eq!(
                many[4].as_ref().unwrap().as_raw_str(),
                many[3].as_ref().unwrap().as_raw_str()
            );
            assert_eq!(many[5].as_ref().unwrap().as_raw_str(), "true");
            // we have strip the leading or trailing spaces
            assert_eq!(many[6].as_ref().unwrap().as_raw_str(), "{\n                \"b\": [0, 1, true],\n                \"a\": {\n                    \"a_b\":{\n                        \"a_b_c\":\"hello, world!\"\n                    },\n                    \"a_a\": [0, 1, 2]\n                }\n            }");
            assert!(many[7].is_none())
        }
    }
}
