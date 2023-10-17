
# sonic-rs

A fast Rust JSON library based on SIMD. It has some references to other open-source libraries like [sonic_cpp](https://github.com/bytedance/sonic-cpp), [serde_json](https://github.com/serde-rs/json), [sonic](https://github.com/bytedance/sonic), [simdjson](https://github.com/simdjson/simdjson) and [rust-lang](https://github.com/rust-lang/rust).

## Requirements
1. Support x86_64 or aarch64. Note that the performance in aarch64 is low and it need to optimize.
2. Rust nightly version. Because we use the `packed_simd` crate.

## Features
1. Serde into Rust struct as `serde_json` and `serde`
2. Parse/Serialize JSON for untyped document, and document can be mutable
3. Get specific fields from a JSON with blazing performance
4. Use JSON as a lazied array or object iterator

## Benchmark

```
Architecture:        x86_64
Model name:          Intel(R) Xeon(R) Platinum 8260 CPU @ 2.40GHz
```

### Deserialize Struct 
`cargo bench --bench deserialize_struct  -- --quiet`

```
twitter/sonic_rs::from_str
                        time:   [824.02 µs 833.33 µs 843.23 µs]
twitter/simd_json::from_str
                        time:   [1.0762 ms 1.1134 ms 1.1535 ms]
twitter/serde_json::from_str
                        time:   [2.2768 ms 2.3201 ms 2.3700 ms]

citm_catalog/sonic_rs::from_str
                        time:   [1.3502 ms 1.3661 ms 1.3858 ms]
citm_catalog/simd_json::from_str
                        time:   [2.3854 ms 2.4642 ms 2.5473 ms]
citm_catalog/serde_json::from_str
                        time:   [3.1383 ms 3.1593 ms 3.1830 ms]

canada/sonic_rs::from_str
                        time:   [3.9409 ms 3.9917 ms 4.0542 ms]
canada/simd_json::from_str
                        time:   [7.9716 ms 8.0212 ms 8.0744 ms]
canada/serde_json::from_str
                        time:   [6.3506 ms 6.5278 ms 6.7434 ms]
```

### Deserialize Untyped
`cargo bench --bench deserialize_value  -- --quiet`

```
twitter/sonic_rs_dom::from_str
                        time:   [506.92 µs 513.16 µs 520.79 µs]
twitter/simd_json::to_borrowed_value
                        time:   [1.4096 ms 1.4386 ms 1.4683 ms]
twitter/serde_json::from_slice
                        time:   [3.8151 ms 3.8866 ms 3.9746 ms]
twitter/simd_json2::parse
                        time:   [401.29 µs 411.22 µs 422.51 µs]
twitter/simd_json::to_owned_value
                        time:   [1.7898 ms 1.8253 ms 1.8680 ms]

citm_catalog/sonic_rs_dom::from_str
                        time:   [1.4471 ms 1.4931 ms 1.5426 ms]
citm_catalog/simd_json::to_borrowed_value
                        time:   [3.6415 ms 3.7131 ms 3.7938 ms]
citm_catalog/serde_json::from_slice
                        time:   [8.6240 ms 8.7970 ms 8.9845 ms]
citm_catalog/simd_json2::parse
                        time:   [1.0627 ms 1.0751 ms 1.0903 ms]
citm_catalog/simd_json::to_owned_value
                        time:   [4.7230 ms 4.9033 ms 5.0954 ms]

canada/sonic_rs_dom::from_str
                        time:   [4.7793 ms 4.8831 ms 5.0072 ms]
canada/simd_json::to_borrowed_value
                        time:   [12.432 ms 12.585 ms 12.757 ms]
canada/serde_json::from_slice
                        time:   [14.214 ms 14.639 ms 15.115 ms]
canada/simd_json2::parse
                        time:   [4.6120 ms 4.6579 ms 4.7112 ms]
canada/simd_json::to_owned_value
                        time:   [12.214 ms 12.345 ms 12.504 ms]
```


### Serialize Untyped
`cargo bench --bench serialize_value  -- --quiet`

```
twitter/sonic_rs::to_string
                        time:   [408.33 µs 413.91 µs 420.21 µs]
twitter/serde_json::to_string
                        time:   [785.11 µs 804.31 µs 825.93 µs]
twitter/simd_json::to_string
                        time:   [971.26 µs 994.93 µs 1.0215 ms]

citm_catalog/sonic_rs::to_string
                        time:   [941.62 µs 956.98 µs 974.07 µs]
citm_catalog/serde_json::to_string
                        time:   [2.5998 ms 2.6813 ms 2.7672 ms]
citm_catalog/simd_json::to_string
                        time:   [1.9469 ms 1.9871 ms 2.0305 ms]

canada/sonic_rs::to_string
                        time:   [9.5913 ms 10.017 ms 10.440 ms]
canada/serde_json::to_string
                        time:   [7.4087 ms 7.5698 ms 7.7410 ms]
canada/simd_json::to_string
                        time:   [8.1523 ms 8.2870 ms 8.4354 ms]
```

### Serialize Struct
`cargo bench --bench serialize_struct  -- --quiet`
```
twitter/sonic_rs::to_string
                        time:   [450.14 µs 457.46 µs 466.17 µs]
twitter/simd_json::to_string
                        time:   [501.38 µs 508.94 µs 517.62 µs]
twitter/serde_json::to_string
                        time:   [728.69 µs 739.64 µs 752.84 µs]

canada/sonic_rs::to_string
                        time:   [4.4567 ms 4.4976 ms 4.5433 ms]
canada/simd_json::to_string
                        time:   [6.1011 ms 6.1888 ms 6.2919 ms]
canada/serde_json::to_string
                        time:   [4.6299 ms 4.6864 ms 4.7530 ms]

citm_catalog/sonic_rs::to_string
                        time:   [720.26 µs 729.01 µs 739.26 µs]
citm_catalog/simd_json::to_string
                        time:   [637.16 µs 652.30 µs 670.01 µs]
citm_catalog/serde_json::to_string
                        time:   [859.20 µs 870.30 µs 882.99 µs]
```

### Get from JSON
` cargo bench --bench get_from -- --quiet`

```
twitter/sonic-rs::get_from_str
                        time:   [67.407 µs 67.974 µs 68.653 µs]
twitter/gjson::get      time:   [340.67 µs 344.56 µs 349.10 µs]
```

## Usage


### Serde into Rust Type

```
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u8,
    phones: Vec<String>,
}

fn main() {
    let data = r#"{
  "name": "Xiaoming",
  "age": 18,
  "phones": [
    "+123456"
  ]
}"#;
    let p: Person = sonic_rs::from_str(data).unwrap();
    assert_eq!(p.age, 18);
    assert_eq!(p.name, "Xiaoming");
    let out = sonic_rs::to_string_pretty(&p).unwrap();
    assert_eq!(out, data);
}
```

### Get a field from JSON

```
use sonic_rs::{get_from_str, pointer, JsonValue, PointerNode};

fn main() {
    let path = pointer!["a", "b", "c", 1];
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}
    "#;
    let target = get_from_str(json, path.iter()).unwrap();
    assert_eq!(target.as_raw_str(), r#""found""#);
    assert_eq!(target.as_str().unwrap(), "found");

    let path = pointer!["a", "b", "c", "d"];
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}
    "#;
    // not found from json
    let target = get_from_str(json, path.iter());
    assert!(target.is_err());
}
```

### Parse and Serialize into untyped Value

```
use sonic_rs::value::{dom_from_slice, Value};
use sonic_rs::PointerNode;
use sonic_rs::{pointer, JsonValue};
fn main() {
    let json = r#"{
        "name": "Xiaoming",
        "obj": {},
        "arr": [],
        "age": 18,
        "address": {
            "city": "Beijing"
        },
        "phones": [
            "+123456",
        ]
    }"#;

    let mut dom = dom_from_slice(json.as_bytes()).unwrap();
    // get the value from dom
    let root = dom.as_value();

    // get key from value
    let age = root.get("age").as_i64();
    assert_eq!(age.unwrap_or_default(), 18);

    // get by index
    let first = root["phones"][0].as_str().unwrap();
    assert_eq!(first, "+123456");

    // get by pointer
    let phones = root.pointer(&pointer!["phones", 0]);
    assert_eq!(phones.as_str().unwrap(), "+123456");

    // convert to mutable object
    let mut obj = dom.as_object_mut().unwrap();
    let value = Value::new_bool(true);
    obj.insert("inserted", value);
    assert!(obj.contains_key("inserted"));
}

```

### JSON Iterator

```
use bytes::Bytes;
use sonic_rs::{to_array_iter, JsonValue};

fn main() {
    let json = Bytes::from(r#"[1, 2, 3, 4, 5, 6]"#);
    let iter = to_array_iter(&json);
    for (i, v) in iter.enumerate() {
        assert_eq!(i + 1, v.as_u64().unwrap() as usize);
    }

    let json = Bytes::from(r#"[1, 2, 3, 4, 5, 6"#);
    let mut iter = to_array_iter(&json);
    for _ in iter.iter() {}
    // deal with errors when invalid json
    let ret = iter.take_result();
    assert_eq!(
        ret.as_ref().err().unwrap().to_string(),
        "expected `,` or `]` at line 1 column 17"
    );
}
```

## Contributing
Please read `CONTRIBUTING.md` for information on contributing to sonic-cpp.