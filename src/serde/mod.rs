//! Serde between JSON text and Rust data structure.

pub(crate) mod de;
pub(crate) mod number;
pub(crate) mod rawnumber;
pub(crate) mod ser;

pub(crate) use self::de::tri;
pub use self::{
    de::{from_slice, from_slice_unchecked, from_str, Deserializer, StreamDeserializer},
    number::{JsonNumberTrait, Number},
    rawnumber::RawNumber,
    ser::{
        to_string, to_string_pretty, to_vec, to_vec_pretty, to_writer, to_writer_pretty, Serializer,
    },
};

#[cfg(test)]
#[allow(clippy::mutable_key_type)]
mod test {
    use std::{
        borrow::Cow,
        collections::{BTreeMap, HashMap},
        hash::Hash,
        marker::PhantomData,
    };

    use bytes::Bytes;
    use faststr::FastStr;
    use serde::{de::IgnoredAny, Deserialize, Serialize};

    use super::*;
    use crate::Result;

    macro_rules! hashmap {
        () => {
            HashMap::new()
        };
        ($($k:expr => $v:expr),+ $(,)?) => {
            {
                let mut m = HashMap::new();
                $(
                    m.insert($k, $v);
                )+
                m
            }
        };
    }

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

    // newtype struct
    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct NewtypeStruct<'a>(&'a str);

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
        wrapper: NewtypeStruct<'a>,
        phan_struct: Phan<&'a ()>,

        // non-str keys for map
        map_u8: HashMap<u8, u8>,
        map_u16: HashMap<u16, u16>,
        map_u32: HashMap<u32, u32>,
        map_u64: HashMap<u64, u64>,
        map_u128: HashMap<u128, u128>,

        map_i8: HashMap<i8, i8>,
        map_i16: HashMap<i16, i16>,
        map_i32: HashMap<i32, i32>,
        map_i64: HashMap<i64, i64>,
        map_i128: HashMap<i128, i128>,

        map_bool: HashMap<bool, bool>,

        #[serde(skip_serializing)]
        ignored: IgnoredAny,
    }

    fn gen_data() -> TestData<'static> {
        TestData {
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
            map: hashmap!(
                FastStr::from("key1") => 1.1,
                FastStr::from("key2") => 2.2,
            ),
            map_opkey: hashmap!(
                Some(FastStr::from("key1")) => 1.1,
            ),
            option: Some(String::from("I'm here")),
            enummap: hashmap!(
                Enum::Zero => FieldlessEnum::Struct {},
                Enum::One => FieldlessEnum::Unit,
            ),
            nummap: hashmap!(
                0 => FieldlessEnum::Struct {},
                1 => FieldlessEnum::Unit,
            ),
            fieldenum: FieldEnum::Tuple((FastStr::from("test"), 42)),
            fieldless: FieldlessEnum::Struct {},
            enum_: Enum::One,

            tuple: (42, String::from("test")),
            tuple_struct: Pair(42, 3.33),
            unit_struct: Unit,
            wrapper: NewtypeStruct("hello"),
            phan_struct: Phan {
                phan: String::from("test data"),
                _data: PhantomData,
            },

            map_u8: hashmap!(u8::MAX => u8::MAX),
            map_u16: hashmap!(u16::MAX => u16::MAX),
            map_u32: hashmap!(u32::MAX => u32::MAX),
            map_u64: hashmap!(u64::MAX => u64::MAX),
            map_u128: hashmap!(u128::MAX => u128::MAX),

            map_i8: hashmap!(i8::MAX => i8::MAX),
            map_i16: hashmap!(i16::MAX => i16::MAX),
            map_i32: hashmap!(i32::MAX => i32::MAX),
            map_i64: hashmap!(i64::MAX => i64::MAX),
            map_i128: hashmap!(i128::MAX => i128::MAX),

            map_bool: hashmap!(true => true, false => false),
            ignored: IgnoredAny,
        }
    }

    #[allow(clippy::mutable_key_type)]
    #[test]
    fn test_serde_struct() {
        let data = gen_data();
        let expect = serde_json::to_string(&data).expect("Failed to serialize the data");
        let got = to_string(&data).expect("Failed to serialize the data");
        assert_eq!(expect, got);
        println!("serialized json is {}", got);

        let got = r#"{"ignored":0,"#.to_string() + &got[1..];
        let expect_value: TestData =
            serde_json::from_str(&got).expect("Failed to deserialize the data");
        let got_value: TestData = from_str(&got).expect("Failed to deserialize the data");
        assert_eq!(expect_value, got_value);
    }

    #[test]
    fn test_struct_with_skipped() {
        let data = gen_data();
        let json = r#"{"ignored":0, "unknown":0,"unknown":null,"unknown":1234e123,"unknown":1.234,"unknown":[],"unknown":{},"unknown":{"a":[]},"unknown":[1,2,3],"#.to_string()
            + &serde_json::to_string(&data).expect("Failed to serialize the data")[1..];

        let expect: TestData = serde_json::from_str(&json).unwrap();
        let val: TestData = from_str(&json).unwrap();
        assert_eq!(val, expect);
    }

    #[test]
    fn test_struct_with_ignored() {
        let data = gen_data();
        let json = r#"{"ignored":[1,2,3],"#.to_string()
            + &serde_json::to_string(&data).expect("Failed to serialize the data")[1..];

        let expect: TestData = serde_json::from_str(&json).unwrap();
        let val: TestData = from_str(&json).unwrap();
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
        std::fs::File::open(root + "/benchmarks/benches/testdata/" + path)
            .unwrap()
            .read_to_end(vec)
            .unwrap();
    }

    #[test]
    fn test_struct() {
        use schema::{citm_catalog::CitmCatalog, twitter::Twitter};
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

    macro_rules! test_from_slice {
        ($ty:ty, $data:expr) => {
            test_from! {$ty, from_slice, $data};
        };
    }

    macro_rules! test_from {
        ($ty:ty, $f:ty, $data:expr) => {
            ::paste::paste! {
            match serde_json::$f::<$ty>($data) {
                Ok(jv) => {
                    let sv = crate::$f::<$ty>($data).expect(&format!(
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
                    let _ = crate::$f::<$ty>($data).expect_err(&format!(
                        "parse invalid json {:?} wrong for type {}, should error: {}",
                        $data,
                        stringify!($ty),
                        err
                    ));
                }
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
        test_from_slice!(String, &[34, 58, 55, 10, 0, 34, 32, 10]);
        test_from_slice!(String, &[34, b'\\', b't', 9, 34]);
        test_from_slice!(String, &[34, 92, 34, 34]);
        test_from_slice!(String, b"\"\\umap9map009\"");
        test_from_slice!(Foo, &b"[\"5XXXXXXZX:XXZX:[\",-0]"[..]);
        test_from_slice!(Bytes, &b"\"hello world\""[..]);
        test_from_slice!(
            Bytes,
            &b"[104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]"[..]
        );
        test_from_slice!(
            ByteBuf,
            &b"[104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]"[..]
        );
        test_from_slice!(ByteBuf, &b"\"hello world\""[..]);
        test_from_slice!(Bytes, &b"[]"[..]);
        test_from_slice!(Data, &br#"{"content":[1,2,3,4,5]}"#[..]);
    }

    use std::{
        fmt::{Formatter, Result as FmtResult},
        result::Result as StdResult,
    };

    fn my_deseirlize_seq<'de, D>(deserializer: D) -> StdResult<(i64, i64), D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct TupleVisitor;

        impl<'de> serde::de::Visitor<'de> for TupleVisitor {
            type Value = (i64, i64);

            fn expecting(&self, formatter: &mut Formatter) -> FmtResult {
                formatter.write_str("expect an array")
            }

            fn visit_seq<S>(self, mut seq: S) -> StdResult<Self::Value, S::Error>
            where
                S: serde::de::SeqAccess<'de>,
            {
                let x = seq
                    .next_element::<i64>()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let y = seq
                    .next_element::<i64>()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                Ok((x, y))
            }
        }

        deserializer.deserialize_seq(TupleVisitor)
    }

    fn my_deseirlize_map<'de, D>(deserializer: D) -> StdResult<(String, i64), D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct MapVisitor;

        impl<'de> serde::de::Visitor<'de> for MapVisitor {
            type Value = (String, i64);

            fn expecting(&self, formatter: &mut Formatter) -> FmtResult {
                formatter.write_str("expect an array")
            }

            fn visit_map<S>(self, mut map: S) -> StdResult<Self::Value, S::Error>
            where
                S: serde::de::MapAccess<'de>,
            {
                let x = map
                    .next_key()?
                    .ok_or_else(|| serde::de::Error::custom("miss a key"))?;
                let y = map.next_value::<i64>()?;
                Ok((x, y))
            }
        }

        deserializer.deserialize_map(MapVisitor)
    }

    #[derive(serde::Deserialize, Debug, Eq, PartialEq)]
    struct MyTuple {
        #[serde(deserialize_with = "my_deseirlize_seq")]
        seq: (i64, i64),
        #[serde(deserialize_with = "my_deseirlize_map")]
        map: (String, i64),
    }

    #[test]
    fn test_serde_invalid_utf8() {
        let json = r#""王先生""#;

        let (encoded, _, error) = encoding_rs::GB18030.encode(json);
        assert!(!error, "Encoded error");

        let obj: &[u8] = from_slice(encoded.as_ref()).expect("Failed deserialize");
        println!("Deserialized {:?}", obj);

        let sout = crate::to_string(&obj).unwrap();
        let jout = serde_json::to_string(&obj).unwrap();
        assert_eq!(jout, sout);
        println!("json is {}", jout);
        // this will failed
        // let jv = serde_json::from_str::<&[u8]>(&jout).unwrap();
    }

    #[test]
    fn test_ser_errors() {
        #[derive(Debug, serde::Serialize, Hash, Default, Eq, PartialEq)]
        struct User {
            string: String,
            number: i32,
            array: Vec<String>,
        }

        let mut map = HashMap::<User, i64>::new();
        map.insert(User::default(), 123);

        let got = to_string(&map);
        println!("{:?}", got);
        assert!(got.is_err());
    }

    #[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone)]
    struct Float;
    impl Serialize for Float {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_f32(1.23)
        }
    }
    impl<'de> Deserialize<'de> for Float {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            f32::deserialize(deserializer).map(|_| Float)
        }
    }

    #[test]
    fn test_float_key() {
        // map with float key
        let mut map = BTreeMap::new();
        map.insert(&Float, "x");

        test_from!(BTreeMap<Float, String>, from_str, "{\"1.23\":\"x\"}" );
        test_from!(BTreeMap<Float, String>, from_str, "{\"1.23\":null}" );
    }

    // test deserialize into different mapkeys
    #[derive(PartialEq, Debug)]
    struct MapKeys<'a> {
        invalutf: Vec<u8>,
        bytes: Vec<u8>,
        string: String,
        bool: bool,

        i8: i8,
        i16: i16,
        i32: i32,
        i64: i64,
        i128: i128,

        u8: u8,
        u16: u16,
        u32: u32,
        u64: u64,
        u128: u128,

        isize: isize,
        usize: usize,

        f32: f32,
        f64: f64,
        option: Option<i64>,
        wrapper: NewtypeStruct<'a>,
        enum_key: Enum,
        char_key: char,
        ignored: IgnoredAny,
    }

    fn gen_keys_json() -> String {
        r#"
        {   "invalid utf8 here": "invalid utf8 here",
            "bytes": [1,2,3],
            "string": "hello",
        
            "true": true,
    
            "1": 1,
            "123": 123,
            "-123": -123,
            "12345": 12345,
            "1": 1,
    
            "1": 1,
            "123": 123,
            "123": -123,
            "12345": 12345,
            "1": 1,
    
            "12345": 12345,
            "1": 1,
    
            "1.23e+2": 1.23,
            "-1.23": -1.23,
    
            "123": "option",
    
            "wrapper": {},
    
            "Zero": "enum",
    
            "A": "char",
    
            "ignored": "ignored"
        }
        "#
        .to_string()
        .replace("invalid utf8", unsafe {
            std::str::from_utf8_unchecked(&[0xff, 0xff, 0xff][..])
        })
    }

    impl<'de> serde::de::Deserialize<'de> for MapKeys<'de> {
        fn deserialize<D>(d: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct Visitor {}

            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = MapKeys<'de>;

                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("a map")
                }

                fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
                where
                    A: serde::de::MapAccess<'de>,
                {
                    fn parse_key<'de, V, A>(map: &mut A) -> std::result::Result<V, A::Error>
                    where
                        A: serde::de::MapAccess<'de>,
                        V: serde::Deserialize<'de>,
                    {
                        let key = map.next_key::<V>()?.unwrap();
                        let _ = map.next_value::<IgnoredAny>()?;
                        Ok(key)
                    }

                    Ok(MapKeys {
                        invalutf: parse_key::<&'de [u8], A>(&mut map)?.to_owned(),
                        bytes: parse_key::<&'de [u8], A>(&mut map)?.to_owned(),
                        string: parse_key::<String, A>(&mut map)?,
                        bool: parse_key::<bool, A>(&mut map)?,
                        i8: parse_key::<i8, A>(&mut map)?,
                        i16: parse_key::<i16, A>(&mut map)?,
                        i32: parse_key::<i32, A>(&mut map)?,
                        i64: parse_key::<i64, A>(&mut map)?,
                        i128: parse_key::<i128, A>(&mut map)?,
                        u8: parse_key::<u8, A>(&mut map)?,
                        u16: parse_key::<u16, A>(&mut map)?,
                        u32: parse_key::<u32, A>(&mut map)?,
                        u64: parse_key::<u64, A>(&mut map)?,
                        u128: parse_key::<u128, A>(&mut map)?,
                        isize: parse_key::<isize, A>(&mut map)?,
                        usize: parse_key::<usize, A>(&mut map)?,
                        f32: parse_key::<f32, A>(&mut map)?,
                        f64: parse_key::<f64, A>(&mut map)?,
                        option: parse_key::<Option<i64>, A>(&mut map)?,
                        wrapper: parse_key::<NewtypeStruct<'de>, A>(&mut map)?,
                        enum_key: parse_key::<Enum, A>(&mut map)?,
                        char_key: parse_key::<char, A>(&mut map)?,
                        ignored: parse_key::<IgnoredAny, A>(&mut map)?,
                    })
                }
            }
            d.deserialize_map(Visitor {})
        }
    }

    #[test]
    fn test_parse_map_keys() {
        let s = gen_keys_json();
        let expect: MapKeys = serde_json::from_str(&s).unwrap();
        let got: MapKeys = crate::from_str(&s).unwrap();
        assert_eq!(expect, got);
    }
}
