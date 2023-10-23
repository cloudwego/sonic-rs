# sonic-rs

[![Crates.io](https://img.shields.io/crates/v/sonic-rs)](https://crates.io/crates/sonic-rs)
[![Documentation](https://docs.rs/sonic-rs/badge.svg)](https://docs.rs/sonic-rs)
[![Website](https://img.shields.io/website?up_message=cloudwego&url=https%3A%2F%2Fwww.cloudwego.io%2F)](https://www.cloudwego.io/)
[![License](https://img.shields.io/crates/l/sonic-rs)](#license)
[![Build Status][actions-badge]][actions-url]

[actions-badge]: https://github.com/cloudwego/sonic-rs/actions/workflows/ci.yml/badge.svg
[actions-url]: https://github.com/cloudwego/sonic-rs/actions

English | [中文](README_ZH.md)

A fast Rust JSON library based on SIMD. It has some references to other open-source libraries like [sonic_cpp](https://github.com/bytedance/sonic-cpp), [serde_json](https://github.com/serde-rs/json), [sonic](https://github.com/bytedance/sonic), [simdjson](https://github.com/simdjson/simdjson), [rust-std](https://github.com/rust-lang/rust/tree/master/library/core/src/num) and more.

The main optimization in sonic-rs is the use of SIMD. However, we do not use the two-stage SIMD algorithms from `simd-json`. We primarily use SIMD in the following scenarios:
1. parsing/serialize long JSON strings
2. parsing the fraction of float number
3. Getting a specific elem or field from JSON
4. Skipping white spaces when parsing JSON

More details about optimization can be found in [performance.md](docs/performance.md).

## Requirements/Notes

1. Support x86_64 or aarch64. Note that the performance in aarch64 is lower and needs optimization.
2. Requires Rust nightly version, as we use the `packed_simd` crate.
3. When using `get_from`, `get_many`, `JsonIter` or `RawValue`, ***Warn:*** the JSON should be well-formed and valid.

## Features
1. Serde into Rust struct as `serde_json` and `serde`.

2. Parse/Serialize JSON for untyped document, which can be mutable.

3. Get specific fields from a JSON with the blazing performance.

4. Use JSON as a lazy array or object iterator with the blazing performance.

5. Supprt `RawValue`, `Number` and `RawNumber`(just like Golang's `JsonNumber`) in default.

6. The floating parsing percision is as Rust std in default.

## Quick to use sonic-rs

To ensure that SIMD instruction is used in sonic-rs, you need to add rustflags `-C target-cpu=native` and compile on the host machine. For example, Rust flags can be configured in Cargo [config](.cargo/config).

Add sonic-rs in `Cargo.toml`
```
[dependencies]
sonic-rs = 0.2.0
```

## Benchmark

Benchmarks environemnt:

```
Architecture:        x86_64
Model name:          Intel(R) Xeon(R) Platinum 8260 CPU @ 2.40GHz
```
Benchmarks:

- Deserialize Struct: Deserialize the JSON into Rust struct. The defined struct and testdata is from [json-benchmark][https://github.com/serde-rs/json-benchmark]

- Deseirlize Untyped: Deseialize the JSON into a document

The serialize benchmarks work in the opposite way.

All deserialized benchmark enabled utf-8, and enabled `float_roundtrip` in `serde-json` to get sufficient precision as Rust std. 

### Deserialize Struct

The benchmark will parse JSON into a Rust struct, and there are no unknown fields in JSON text. All fields are parsed into struct fields in the JSON. 

Sonic-rs is faster than simd-json because simd-json (Rust) first parses the JSON into a `tape`, then parses the `tape` into a Rust struct. Sonic-rs directly parses the JSON into a Rust struct, and there are no temporary data structures. The [flamegraph](assets/pngs/) is profiled in the citm_catalog case.

`cargo bench --bench deserialize_struct -- --quiet`

```
twitter/sonic_rs::from_slice
                        time:   [721.80 µs 747.81 µs 776.19 µs]
twitter/simd_json::from_slice
                        time:   [1.0909 ms 1.1225 ms 1.1561 ms]
twitter/serde_json::from_slice
                        time:   [2.3218 ms 2.3491 ms 2.3787 ms]
twitter/serde_json::from_str
                        time:   [1.4123 ms 1.4460 ms 1.4842 ms]

citm_catalog/sonic_rs::from_slice
                        time:   [1.2133 ms 1.2447 ms 1.2827 ms]
citm_catalog/simd_json::from_slice
                        time:   [2.0556 ms 2.0822 ms 2.1126 ms]
citm_catalog/serde_json::from_slice
                        time:   [2.9939 ms 3.0271 ms 3.0674 ms]
citm_catalog/serde_json::from_str
                        time:   [2.4043 ms 2.4604 ms 2.5283 ms]

canada/sonic_rs::from_slice
                        time:   [3.8612 ms 3.9070 ms 3.9574 ms]
canada/simd_json::from_slice
                        time:   [8.8144 ms 8.9206 ms 9.0317 ms]
canada/serde_json::from_slice
                        time:   [8.8703 ms 8.9586 ms 9.0555 ms]
canada/serde_json::from_str
                        time:   [9.2865 ms 9.4272 ms 9.6032 ms]
```


### Deserialize Untyped

The benchmark will parse JSON into a document. Sonic-rs seems faster for several reasons:
- There are also no temporary data structures in sonic-rs, as detailed above.
- Sonic-rs uses a memory arena for the whole document, resulting in fewer memory allocations, better cache-friendliness, and mutability.
- The JSON object in sonic-rs's document is actually a vector. Sonic-rs does not build a hashmap.

`cargo bench --bench deserialize_value -- --quiet`

```
twitter/sonic_rs_dom::from_slice
                        time:   [589.34 µs 593.81 µs 599.02 µs]
twitter/simd_json::slice_to_borrowed_value
                        time:   [1.2174 ms 1.2281 ms 1.2406 ms]
twitter/serde_json::from_slice
                        time:   [3.9370 ms 3.9658 ms 3.9960 ms]
twitter/serde_json::from_str
                        time:   [2.8013 ms 2.8278 ms 2.8584 ms]
twitter/simd_json::slice_to_owned_value
                        time:   [1.7537 ms 1.7857 ms 1.8220 ms]

citm_catalog/sonic_rs_dom::from_slice
                        time:   [1.7779 ms 1.8326 ms 1.8942 ms]
citm_catalog/simd_json::slice_to_borrowed_value
                        time:   [4.0278 ms 4.1167 ms 4.2103 ms]
citm_catalog/serde_json::from_slice
                        time:   [9.4022 ms 9.5598 ms 9.7242 ms]
citm_catalog/serde_json::from_str
                        time:   [7.7487 ms 7.9720 ms 8.2212 ms]
citm_catalog/simd_json::slice_to_owned_value
                        time:   [4.1156 ms 4.1760 ms 4.2489 ms]

canada/sonic_rs_dom::from_slice
                        time:   [4.9905 ms 5.0650 ms 5.1539 ms]
canada/simd_json::slice_to_borrowed_value
                        time:   [11.931 ms 12.142 ms 12.384 ms]
canada/serde_json::from_slice
                        time:   [17.262 ms 17.433 ms 17.634 ms]
canada/serde_json::from_str
                        time:   [16.579 ms 16.773 ms 17.025 ms]
canada/simd_json::slice_to_owned_value
                        time:   [12.024 ms 12.209 ms 12.423 ms]
```

### Serialize Untyped

`cargo bench --bench serialize_value  -- --quiet`

We serialize the document into a string. In the following benchmarks, sonic-rs appears faster for the `twitter` JSON. The `twitter` JSON contains many long JSON strings, which fit well with sonic-rs's SIMD optimization.

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

The explanation is as mentioned above.

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

The benchmark is getting a specific field from the twitter JSON. In both sonic-rs and gjson, the JSON should be well-formed and valid when using get or get_from. Sonic-rs utilize SIMD to quickly skip unnecessary fields, thus enhancing the performance.

```
twitter/sonic-rs::get_from_str
                        time:   [79.432 µs 80.008 µs 80.738 µs]
twitter/gjson::get      time:   [344.41 µs 351.36 µs 362.03 µs]
```

## Usage

### Serde into Rust Type

Directly use the `Deserialize` or `Serialize` trait.

```rs
use sonic_rs::{Deserialize, Serialize}; 
// sonic-rs re-exported them from serde
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

Get a specific field from a JSON with the `pointer` path. The return is a `LazyValue`, which is a wrapper of a raw JSON slice. Note that the JSON must be valid and well-formed, otherwise it may return unexpected result.

```rs
use sonic_rs::{get_from_str, pointer, JsonValue, PointerNode};

fn main() {
    let path = pointer!["a", "b", "c", 1];
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}
    "#;
    let target = unsafe { get_from_str(json, path.iter()).unwrap() };
    assert_eq!(target.as_raw_str(), r#""found""#);
    assert_eq!(target.as_str().unwrap(), "found");

    let path = pointer!["a", "b", "c", "d"];
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}
    "#;
    // not found from json
    let target = unsafe { get_from_str(json, path.iter()) };
    assert!(target.is_err());
}
```

### Parse and Serialize into untyped Value

Parse a JSON into a document, which is mutable. Be aware that the document is managed by a `bump` allocator. It is recommended to convert documents into `Object/ObjectMut` or `Array/ArrayMut` to make them typed and easier to use.

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

### JSON RawValue & Number & RawNumber

If we need parse a JSON value as a raw string, we can use `RawValue`.
If we need parse a JSON number into a untyped type, we can use `Number`.
If we need parse a JSON number ***without loss of percision***, we can use `RawNumber`. It likes `JsonNumber` in Golang, and can also be parsed from a JSON string.

Detailed examples can be found in [raw_value.rs](examples/raw_value.rs) and [json_number.rs](examples/json_number.rs).

## FAQs

### About UTF-8

By default, sonic-rs does not enable UTF-8 validation. This is a trade-off to achieve the fastest performance.

- For the `from_slice` and `dom_from_slice` interfaces, validate UTF-8 in default. If users make sure that the json is utf-8 valid, recommended use the `from_slice_unchecked` and `dom_from_slice_unchecked`.

- For the `get` and `lazyvalue` related interfaces, due to the algorithm design, these interfaces are ***only suitable for use in valid-json scenarios***, and we will not provide UTF-8 validation in the future.

### About floating point precision

By default, sonic-rs uses floating point precision consistent with the Rust standard library, and there is no need to add an extra `float_roundtrip` feature like `serde-json` to ensure floating point precision.

If you want to achieve lossless precision when parsing floating-point numbers, such as Golang `JsonNumber` and `serde-json arbitrary_precision`, you can use `RawNumber`.

## Acknowledgement

Thanks the following open-source libraries. sonic-rs has some references to other open-source libraries like [sonic_cpp](https://github.com/bytedance/sonic-cpp), [serde_json](https://github.com/serde-rs/json), [sonic](https://github.com/bytedance/sonic), [simdjson](https://github.com/simdjson/simdjson), [yyjson](https://github.com/ibireme/yyjson), [rust-std](https://github.com/rust-lang/rust/tree/master/library/core/src/num) and so on.

We rewrote many SIMD algorithms from sonic-cpp/sonic/simdjson/yyjson for performance. We reused the de/ser codes and modified necessary parts from serde_json to make high compatibility with `serde`. We resued part codes about floating parsing from rust-std to make it more accurate.

## Contributing
Please read `CONTRIBUTING.md` for information on contributing to sonic-rs.
