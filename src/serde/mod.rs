mod de;
mod number;
mod raw;
mod ser;

pub use self::de::{from_slice, from_slice_unchecked, from_str, Deserializer};
pub use self::number::{JsonNumberTrait, Number, RawNumber};
pub use self::raw::{to_raw_value, RawValue};
pub use self::ser::{
    to_string, to_string_pretty, to_vec, to_vec_pretty, to_writer, to_writer_pretty, Serializer,
};

pub(crate) use self::de::tri;

#[cfg(test)]
#[allow(clippy::mutable_key_type)]
mod test {
    use super::*;
    use crate::Result;
    use faststr::FastStr;
    use serde::{Deserialize, Serialize};
    use std::borrow::Cow;
    use std::{collections::HashMap, hash::Hash, marker::PhantomData};

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Foo {
        name: FastStr,
        id: u64,
    }

    #[derive(Debug, Deserialize, Serialize, Hash, Eq, PartialEq)]
    enum Enum {
        Zero = 0,
        One = 1,
        Two = 2,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    enum FieldEnum {
        Integer(i8),
        Tuple((FastStr, i32)),
        Struct(Foo),
        Unit,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    enum FieldlessEnum {
        Tuple(),
        Struct {},
        Unit,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Wrapper<'a>(&'a str);

    // A unit struct
    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Unit;

    // A uint struct
    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Phan<T> {
        phan: String,
        _data: PhantomData<T>,
    }

    // A tuple struct
    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Pair(i32, f32);

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct TestData<'a> {
        fieldless: FieldlessEnum,
        enummap: HashMap<Enum, FieldlessEnum>,
        enum_: Enum,

        // basic types
        boolean: bool,
        integer: i32,
        float: f64,
        int128: i128,
        uint128: u128,
        char_: char,

        // string or bytes
        str_: &'a str,
        // bytes_: &'a [u8],
        string: String,
        faststr: FastStr,
        #[serde(borrow)]
        cow: Cow<'a, str>,

        // containers
        vector: Vec<u32>,
        array: [u32; 1],
        empty_array: [u8; 0],
        map: HashMap<FastStr, f64>,
        map_opkey: HashMap<Option<FastStr>, f64>,

        // enum types
        option: Option<String>,
        fieldenum: FieldEnum,

        // tuple or struct
        tuple: (u64, String),
        tuple_struct: Pair,
        unit_struct: Unit,

        #[serde(borrow)]
        wrapper: Wrapper<'a>,
        phan_struct: Phan<()>,
    }

    #[test]
    fn test_serde_struct() {
        let data = TestData {
            boolean: true,
            integer: -42,
            float: 3.33,
            int128: -22_000_000_000_000_000_000_000_000,
            uint128: 11_000_000_000_000_000_000_000_000,
            char_: 'A',

            str_: "hello world",
            // bytes_: &[0x52, 0x75, 0x73, 0x74],
            string: String::from("hello world"),
            faststr: FastStr::from("hello world"),
            cow: Cow::Borrowed("borrowed"),

            vector: vec![42, 24, 7],
            array: [99],
            empty_array: [],
            map: {
                let mut m = HashMap::new();
                m.insert(FastStr::from("key1"), 1.1);
                m.insert(FastStr::from("key2"), 2.2);
                m
            },
            map_opkey: {
                #[allow(clippy::mutable_key_type)]
                let mut m = HashMap::new();
                m.insert(Some(FastStr::from("key1")), 1.1);
                m
            },

            option: Some(String::from("I'm here")),
            enummap: {
                let mut m = HashMap::new();
                m.insert(Enum::Zero, FieldlessEnum::Struct {});
                m.insert(Enum::One, FieldlessEnum::Unit);
                m
            },
            fieldenum: FieldEnum::Tuple((FastStr::from("test"), 42)),
            fieldless: FieldlessEnum::Struct {},
            enum_: Enum::One,

            tuple: (42, String::from("test")),
            tuple_struct: Pair(42, 3.33),
            unit_struct: Unit,
            wrapper: Wrapper("hello"),
            phan_struct: Phan {
                phan: String::from("test data"),
                _data: PhantomData,
            },
        };

        let expect = serde_json::to_string(&data).expect("Failed to serialize the data");
        let got = to_string(&data).expect("Failed to serialize the data");
        assert_eq!(expect, got);
        println!("serialized json is {}", got);

        let expect_value: TestData =
            serde_json::from_str(&expect).expect("Failed to deserialize the data");
        let got_value: TestData = from_str(&expect).expect("Failed to deserialize the data");
        assert_eq!(expect_value, got_value);
    }

    #[test]
    fn test_struct_with_skipped() {
        let json = r#"{"unknown":0,"unknown":null,"unknown":1234e123,"unknown":1.234,"unknown":[],"unknown":{},"unknown":{"a":[]},"unknown":[1,2,3],"fieldless":{"Struct":{}},"enummap":{"Zero":{"Struct":{}},"One":"Unit"},"enum_":"One","boolean":true,"integer":-42,"float":3.33,"int128":-22000000000000000000000000,"uint128":11000000000000000000000000,"char_":"A","str_":"hello world","string":"hello world","faststr":"hello world","cow":"borrowed","vector":[42,24,7],"array":[99],"empty_array":[],"map":{"key2":2.2,"key1":1.1},"map_opkey":{"key1":1.1},"option":"I'm here","fieldenum":{"Tuple":["test",42]},"tuple":[42,"test"],"tuple_struct":[42,3.33],"unit_struct":null,"wrapper":"hello","phan_struct":{"phan":"test data","_data":null},"unknown":0,"unknown":null,"unknown":1234e123,"unknown":1.234,"unknown":[],"unknown":{},"unknown":{"a":[]},"unknown":[1,2,3]}"#;

        let expect: TestData = serde_json::from_str(json).unwrap();
        let val: TestData = from_str(json).unwrap();
        assert_eq!(val, expect);
    }

    fn read_file(path: &str, vec: &mut Vec<u8>) {
        use std::io::Read;
        let root = env!("CARGO_MANIFEST_DIR").to_owned();
        std::fs::File::open(root + "/benches/testdata/" + path)
            .unwrap()
            .read_to_end(vec)
            .unwrap();
    }

    #[test]
    fn test_struct() {
        use json_benchmark::{citm_catalog::CitmCatalog, twitter::Twitter};
        let mut vec = Vec::new();
        read_file("twitter.json", &mut vec);
        let _value: Twitter = from_slice(&vec).unwrap();

        let mut vec = Vec::new();
        read_file("citm_catalog.json", &mut vec);
        let _value: CitmCatalog = from_slice(&vec).unwrap();
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct TestRawValue<'a> {
        #[serde(borrow)]
        rawvalue: &'a RawValue,
        rawvalue2: Box<RawValue>,
    }

    #[test]
    fn test_raw_value_ok() {
        fn test_json_ok(json: &str) {
            let data = TestRawValue {
                rawvalue: from_str(json).expect(json),
                rawvalue2: from_str(json).expect(json),
            };

            // test long json for SIMD
            let json2 = json.to_string() + &" ".repeat(1000);
            let data2 = TestRawValue {
                rawvalue: from_str(json).expect(&json2),
                rawvalue2: from_str(json).expect(&json2),
            };
            assert_eq!(data, data2);
            let json = json.trim();
            let expect: String = format!("{{\"rawvalue\":{},\"rawvalue2\":{}}}", json, json);
            let serialized = to_string(&data).expect(json);
            assert_eq!(expect, serialized);
            assert_eq!(from_str::<TestRawValue>(&serialized).expect(json), data);
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
            let ret: Result<Box<RawValue>> = from_str(json);
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

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct TestJsonNumber {
        num: Number,
        raw_num: RawNumber,
    }

    #[test]
    fn test_json_number() {
        let number: RawNumber = from_str("  123").unwrap();
        assert_eq!(number, RawNumber::new("123"));
        assert_eq!(to_string(&number).unwrap(), "123");

        let number: RawNumber = from_str(r#""0.123""#).unwrap();
        assert_eq!(number, RawNumber::new("0.123"));
        assert_eq!(to_string(&number).unwrap(), "0.123");
        assert!(number.is_f64());
        assert_eq!(number.as_f64().unwrap(), 0.123);
        assert_eq!(number.as_u64(), None);

        let num: Number = number.try_into().unwrap();
        assert_eq!(num.as_f64().unwrap(), 0.123);
        assert_eq!(num.as_u64(), None);

        let data = TestJsonNumber {
            num: Number::from_f64(1.23).unwrap(),
            raw_num: RawNumber::new("1.23e123"),
        };
        let expect = r#"{"num":1.23,"raw_num":1.23e123}"#;
        let got = to_string(&data).expect("Failed to serialize the data");
        assert_eq!(expect, got);
        println!("serialized json is {}", got);

        let got_value: TestJsonNumber = from_str(expect).expect("Failed to deserialize the data");
        assert_eq!(data, got_value);
    }

    #[test]
    fn test_json_number_invalid() {
        fn test_json_failed(json: &str) {
            let ret: Result<RawNumber> = from_str(json);
            assert!(ret.is_err(), "invalid json is {}", json);
        }
        test_json_failed(r#"0."#);
        test_json_failed(r#"-"#);
        test_json_failed(r#"-1e"#);
        test_json_failed(r#"-1e-"#);
        test_json_failed(r#"-1e-1.111"#);
        test_json_failed(r#"-1e-1,"#);
        test_json_failed(
            r#""0.123#);
        test_json_failed(r#""-""#,
        );
        test_json_failed(r#""-1e""#);
    }

    #[test]
    fn test_invalid_utf8() {
        let data = [b'"', 0, 0, 0, 0x80, 0x90, b'"'];
        let value: crate::Result<String> = from_slice(&data);
        assert_eq!(
            value.err().unwrap().to_string(),
            "Invalid UTF-8 characters in json at line 1 column 4\n\n\t\"\0\0\0��\"\n\t....^..\n"
        );
    }
}
