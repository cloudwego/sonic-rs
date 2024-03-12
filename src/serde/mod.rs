//! Serde between JSON text and Rust data structure.

mod de;
pub(crate) mod number;
pub(crate) mod rawnumber;
mod ser;

pub(crate) use self::de::tri;
pub use self::{
    de::{from_slice, from_slice_unchecked, from_str, Deserializer},
    number::{JsonNumberTrait, Number},
    rawnumber::RawNumber,
    ser::{
        to_string, to_string_pretty, to_vec, to_vec_pretty, to_writer, to_writer_pretty, Serializer,
    },
};

#[cfg(test)]
#[allow(clippy::mutable_key_type)]
mod test {
    use std::{borrow::Cow, collections::HashMap, hash::Hash, marker::PhantomData};

    use bytes::Bytes;
    use faststr::FastStr;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::Result;

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
        nummap: HashMap<i64, FieldlessEnum>,
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
        phan_struct: Phan<&'a ()>,
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
            nummap: {
                let mut m = HashMap::new();
                m.insert(0, FieldlessEnum::Struct {});
                m.insert(1, FieldlessEnum::Unit);
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
        let json = r#"{"unknown":0,"unknown":null,"unknown":1234e123,"unknown":1.234,"unknown":[],"unknown":{},"unknown":{"a":[]},"unknown":[1,2,3],"fieldless":{"Struct":{}},"enummap":{"Zero":{"Struct":{}},"One":"Unit"},"nummap":{"0":{"Struct":{}},"1":"Unit"},"enum_":"One","boolean":true,"integer":-42,"float":3.33,"int128":-22000000000000000000000000,"uint128":11000000000000000000000000,"char_":"A","str_":"hello world","string":"hello world","faststr":"hello world","cow":"borrowed","vector":[42,24,7],"array":[99],"empty_array":[],"map":{"key2":2.2,"key1":1.1},"map_opkey":{"key1":1.1},"option":"I'm here","fieldenum":{"Tuple":["test",42]},"tuple":[42,"test"],"tuple_struct":[42,3.33],"unit_struct":null,"wrapper":"hello","phan_struct":{"phan":"test data","_data":null},"unknown":0,"unknown":null,"unknown":1234e123,"unknown":1.234,"unknown":[],"unknown":{},"unknown":{"a":[]},"unknown":[1,2,3]}"#;

        let expect: TestData = serde_json::from_str(json).unwrap();
        let val: TestData = from_str(json).unwrap();
        assert_eq!(val, expect);
    }

    #[test]
    fn test_serde_time() {
        use chrono::{DateTime, Utc};

        let time: DateTime<Utc> = Utc::now();
        let out = to_string_pretty(&time).unwrap();
        let got = from_str::<DateTime<Utc>>(&out).unwrap();
        assert_eq!(time, got);
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

    macro_rules! test_struct {
        ($ty:ty, $data:expr) => {
            match serde_json::from_slice::<$ty>($data) {
                Ok(jv) => {
                    let sv = crate::from_slice::<$ty>($data).expect(&format!(
                        "parse valid json {:?} failed for type {}",
                        $data,
                        stringify!($ty)
                    ));
                    assert_eq!(sv, jv);

                    // fuzz the struct to_string
                    let sout = crate::to_string(&sv).unwrap();
                    let jout = serde_json::to_string(&jv).unwrap();
                    let sv = crate::from_str::<$ty>(&sout).unwrap();
                    let jv = serde_json::from_str::<$ty>(&jout).unwrap();
                    assert_eq!(sv, jv);
                }
                Err(err) => {
                    println!(
                        "parse invalid json {:?} failed for type {}",
                        $data,
                        stringify!($ty)
                    );
                    let _ = crate::from_slice::<$ty>($data).expect_err(&format!(
                        "parse invalid json {:?} wrong for type {}, should error: {}",
                        $data,
                        stringify!($ty),
                        err
                    ));
                }
            }
        };
    }

    #[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
    pub struct Data {
        #[serde(with = "serde_bytes")]
        pub content: Vec<u8>,
    }

    use serde_bytes::ByteBuf;

    // the testcase is found by fuzzing tests
    #[test]
    fn test_more_structs() {
        // invalid json: has control chars
        test_struct!(String, &[34, 58, 55, 10, 0, 34, 32, 10]);
        test_struct!(String, &[34, b'\\', b't', 9, 34]);
        test_struct!(String, &[34, 92, 34, 34]);
        test_struct!(String, b"\"\\umap9map009\"");
        test_struct!(Foo, &b"[\"5XXXXXXZX:XXZX:[\",-0]"[..]);
        test_struct!(Bytes, &b"\"hello world\""[..]);
        test_struct!(
            Bytes,
            &b"[104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]"[..]
        );
        test_struct!(
            ByteBuf,
            &b"[104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]"[..]
        );
        test_struct!(ByteBuf, &b"\"hello world\""[..]);
        test_struct!(Bytes, &b"[]"[..]);
        test_struct!(Data, &br#"{"content":[1,2,3,4,5]}"#[..]);
    }
}
