# sonic-rs

[![Crates.io](https://img.shields.io/crates/v/sonic-rs)](https://crates.io/crates/sonic-rs)
[![Documentation](https://docs.rs/sonic-rs/badge.svg)](https://docs.rs/sonic-rs)
[![Website](https://img.shields.io/website?up_message=cloudwego&url=https%3A%2F%2Fwww.cloudwego.io%2F)](https://www.cloudwego.io/)
[![License](https://img.shields.io/crates/l/sonic-rs)](#license)
[![Build Status][actions-badge]][actions-url]

[actions-badge]: https://github.com/cloudwego/sonic-rs/actions/workflows/ci.yaml/badge.svg
[actions-url]: https://github.com/cloudwego/sonic-rs/actions

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
twitter/sonic_rs::from_slice
                        time:   [668.73 µs 675.09 µs 681.94 µs]
twitter/simd_json::from_slice
                        time:   [963.34 µs 972.64 µs 982.57 µs]
twitter/serde_json::from_slice
                        time:   [2.2711 ms 2.2930 ms 2.3176 ms]
twitter/serde_json::from_str
                        time:   [1.3854 ms 1.4056 ms 1.4286 ms]

citm_catalog/sonic_rs::from_slice
                        time:   [1.2461 ms 1.2740 ms 1.3062 ms]
citm_catalog/simd_json::from_slice
                        time:   [2.2162 ms 2.2696 ms 2.3288 ms]
citm_catalog/serde_json::from_slice
                        time:   [2.8963 ms 2.9163 ms 2.9374 ms]
citm_catalog/serde_json::from_str
                        time:   [2.4114 ms 2.4376 ms 2.4667 ms]

canada/sonic_rs::from_slice
                        time:   [3.8334 ms 3.8643 ms 3.9019 ms]
canada/simd_json::from_slice
                        time:   [8.0765 ms 8.1702 ms 8.2898 ms]
canada/serde_json::from_slice
                        time:   [6.4104 ms 6.4529 ms 6.4990 ms]
canada/serde_json::from_str
                        time:   [6.5044 ms 6.5687 ms 6.6476 ms]
```

### Deserialize Untyped
`cargo bench --bench deserialize_value  -- --quiet`

```
twitter/sonic_rs_dom::from_slice
                        time:   [608.82 µs 620.44 µs 633.70 µs]
twitter/simd_json::slice_to_borrowed_value
                        time:   [1.2590 ms 1.2937 ms 1.3317 ms]
twitter/serde_json::from_slice
                        time:   [4.1013 ms 4.1890 ms 4.2840 ms]
twitter/serde_json::from_str
                        time:   [3.0287 ms 3.0771 ms 3.1319 ms]
twitter/simd_json::slice_to_owned_value
                        time:   [1.7877 ms 1.8202 ms 1.8573 ms]

citm_catalog/sonic_rs_dom::from_slice
                        time:   [1.8739 ms 1.9256 ms 1.9822 ms]
citm_catalog/simd_json::slice_to_borrowed_value
                        time:   [3.5107 ms 3.5864 ms 3.6763 ms]
citm_catalog/serde_json::from_slice
                        time:   [8.3670 ms 8.4982 ms 8.6585 ms]
citm_catalog/serde_json::from_str
                        time:   [8.1648 ms 8.3301 ms 8.5224 ms]
citm_catalog/simd_json::slice_to_owned_value
                        time:   [4.1098 ms 4.1530 ms 4.2017 ms]

canada/sonic_rs_dom::from_slice
                        time:   [5.2630 ms 5.4299 ms 5.6119 ms]
canada/simd_json::slice_to_borrowed_value
                        time:   [12.002 ms 12.135 ms 12.289 ms]
canada/serde_json::from_slice
                        time:   [14.909 ms 15.148 ms 15.431 ms]
canada/serde_json::from_str
                        time:   [15.277 ms 15.656 ms 16.089 ms]
canada/simd_json::slice_to_owned_value
                        time:   [12.025 ms 12.137 ms 12.261 ms]
```


### Serialize Untyped
`cargo bench --bench serialize_value  -- --quiet`

```
twitter/sonic_rs::to_string
                        time:   [380.90 µs 390.00 µs 400.38 µs]
twitter/serde_json::to_string
                        time:   [788.98 µs 797.34 µs 807.69 µs]
twitter/simd_json::to_string
                        time:   [965.66 µs 981.14 µs 998.08 µs]

citm_catalog/sonic_rs::to_string
                        time:   [805.85 µs 821.99 µs 841.06 µs]
citm_catalog/serde_json::to_string
                        time:   [1.8299 ms 1.8880 ms 1.9498 ms]
citm_catalog/simd_json::to_string
                        time:   [1.7356 ms 1.7636 ms 1.7972 ms]

canada/sonic_rs::to_string
                        time:   [6.5808 ms 6.7082 ms 6.8570 ms]
canada/serde_json::to_string
                        time:   [6.4800 ms 6.5747 ms 6.6893 ms]
canada/simd_json::to_string
                        time:   [7.3751 ms 7.5690 ms 7.7944 ms]
```

### Serialize Struct
`cargo bench --bench serialize_struct  -- --quiet`
```
twitter/sonic_rs::to_string
                        time:   [434.03 µs 448.25 µs 463.97 µs]
twitter/simd_json::to_string
                        time:   [506.21 µs 515.54 µs 526.35 µs]
twitter/serde_json::to_string
                        time:   [719.70 µs 739.97 µs 762.69 µs]

canada/sonic_rs::to_string
                        time:   [4.6701 ms 4.7481 ms 4.8404 ms]
canada/simd_json::to_string
                        time:   [5.8072 ms 5.8793 ms 5.9625 ms]
canada/serde_json::to_string
                        time:   [4.5708 ms 4.6281 ms 4.6967 ms]

citm_catalog/sonic_rs::to_string
                        time:   [624.86 µs 629.54 µs 634.57 µs]
citm_catalog/simd_json::to_string
                        time:   [624.10 µs 633.55 µs 644.78 µs]
citm_catalog/serde_json::to_string
                        time:   [802.10 µs 814.15 µs 828.10 µs]
```

### Get from JSON
`cargo bench --bench get_from -- --quiet`

```
twitter/sonic-rs::get_from_str
                        time:   [79.432 µs 80.008 µs 80.738 µs]
twitter/gjson::get      time:   [344.41 µs 351.36 µs 362.03 µs]
```

## Usage


### Serde into Rust Type

Directly use the `Deserialize` or `Serialize` trait, recommended use `sonic_rs::{Deserialize, Serialize}`.

```rs
use sonic_rs::{Deserialize, Serialize};
// or use serde::{Deserialize, Serialize};

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

Get a specific field from a JSON with the `pointer` path. The return is a `LazyValue`, which is a wrapper of a raw JSON slice. Note that the JSON must be valid and well-formed,  otherwise it may return unexpected result.

```rs
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

Parse a JSON as a document, and the document is mutable. 

```rs
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

Parse a object or array JSON into a iterator. The `item` of iterator is the `LazyValue`, which is wrapper of a raw JSON slice.

```rs
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
        "Expected this character to be either a ',' or a ']' while parsing at line 1 column 17"
    );
}
```

## Contributing
Please read `CONTRIBUTING.md` for information on contributing to sonic-rs.
