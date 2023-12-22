use super::owned::OwnedLazyValue;
use super::value::LazyValue;
use crate::Result;
use ::serde::ser::SerializeStruct;
use ::serde::Serialize;

impl<'a> serde::ser::Serialize for LazyValue<'a> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let raw = self.as_raw_str();
        let mut s = serializer.serialize_struct(super::TOKEN, 1)?;
        // will directly write raw in `LazyValueStrEmitter::seriazlie_str`
        s.serialize_field(super::TOKEN, raw)?;
        s.end()
    }
}

impl serde::ser::Serialize for OwnedLazyValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let raw = self.as_raw_str();
        let mut s = serializer.serialize_struct(super::TOKEN, 1)?;
        // will directly write raw in `LazyValueStrEmitter::seriazlie_str`
        s.serialize_field(super::TOKEN, raw)?;
        s.end()
    }
}

/// Convert a `T: Serialize` into a `LazyValue`.
pub fn to_lazyvalue<'a, T>(value: &T) -> Result<LazyValue<'a>>
where
    T: ?Sized + Serialize,
{
    let json_string = crate::to_string(value)?;
    Ok(LazyValue::new(json_string.into()))
}

/// Convert a `T: Serialize` into a `OwnedLazyValue`.
pub fn to_ownedlazyvalue<T>(value: &T) -> Result<OwnedLazyValue>
where
    T: ?Sized + Serialize,
{
    let json_string = crate::to_string(value)?;
    Ok(OwnedLazyValue::new(json_string.into()))
}

#[cfg(test)]
mod test {
    use crate::OwnedLazyValue;
    use crate::{from_str, to_string, LazyValue, Result};
    use ::serde::Deserialize;
    use ::serde::Serialize;

    #[test]
    fn test_lazyvalue_serde() {
        let json = r#"{
            "a": 1,
            "b": "2",
            "c": [3, 4, 5],
            "d": {
                "e": 6,
                "f": "7",
                "g": [8, 9, 10]
            }
        }"#;
        let value = crate::from_str::<crate::LazyValue>(json).unwrap();
        let json2 = crate::to_string(&value).unwrap();
        assert_eq!(json, json2);
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct TestLazyValue<'a> {
        #[serde(borrow)]
        borrowed_lv: LazyValue<'a>,
        owned_lv: OwnedLazyValue,
    }

    #[test]
    fn test_raw_value_ok() {
        fn test_json_ok(json: &str) {
            let data = TestLazyValue {
                borrowed_lv: from_str(json).expect(json),
                owned_lv: from_str(json).expect(json),
            };

            // test long json for SIMD
            let json2 = json.to_string() + &" ".repeat(1000);
            let data2 = TestLazyValue {
                borrowed_lv: from_str(json).expect(&json2),
                owned_lv: from_str(json).expect(&json2),
            };
            assert_eq!(data, data2);
            let json = json.trim();
            let expect: String = format!("{{\"borrowed_lv\":{},\"owned_lv\":{}}}", json, json);
            let serialized = to_string(&data).expect(json);
            assert_eq!(expect, serialized);
            assert_eq!(from_str::<TestLazyValue>(&serialized).expect(json), data);
        }
        test_json_ok(r#""""#);
        test_json_ok(r#""raw value""#);
        test_json_ok(r#""哈哈哈☺""#);
        test_json_ok(r#"true"#);
        test_json_ok(r#"false"#);
        test_json_ok(r#"0"#);
        test_json_ok(r#"-1"#);
        test_json_ok(r#"-1e+1111111111111"#);
        test_json_ok(r#"-1e-1111111111111"#);
        test_json_ok(r#"{}"#);
        test_json_ok(r#"[]"#);
        test_json_ok(r#"{"":[], "": ["", "", []]}"#);
        test_json_ok(r#"{"":[], "": ["", "", []]}"#);
    }

    #[test]
    fn test_raw_value_failed() {
        fn test_json_failed(json: &str) {
            let ret: Result<LazyValue<'_>> = from_str(json);
            assert!(ret.is_err(), "invalid json is {}", json);
        }
        test_json_failed(r#"""#);
        test_json_failed(r#""raw " value""#);
        test_json_failed(r#"哈哈哈""#);
        test_json_failed(r#""\x""#);
        test_json_failed("\"\x00\"");
        test_json_failed(r#"tru"#);
        test_json_failed(r#"fals"#);
        test_json_failed(r#"0."#);
        test_json_failed(r#"-"#);
        test_json_failed(r#"-1e"#);
        test_json_failed(r#"-1e-"#);
        test_json_failed(r#"-1e-1.111"#);
        test_json_failed(r#"-1e-1,"#);
        test_json_failed(r#"{"#);
        test_json_failed(r#" ]"#);
        test_json_failed(r#"{"":[], ["", "", []]}"#);
    }
}
