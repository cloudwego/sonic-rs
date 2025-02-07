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

***For Golang users to use `sonic_rs`, please see [for_Golang_user.md](https://github.com/cloudwego/sonic-rs/blob/main/docs/for_Golang_user.md)***

***For users to migrate from `serde_json` to `sonic_rs`, can see [serdejson_compatibility](https://github.com/cloudwego/sonic-rs/blob/main/docs/serdejson_compatibility.md)***

## Requirements/Notes

1. Faster in x86_64 or aarch64, other architecture is fallback and maybe very slower.

2. ~~Requires Rust nightly version~~ Support Stable Rust now.

3. Please add the compile options `-C target-cpu=native`

4. Should enable `sanitize` feature to avoid false-positive if you are using LLVM-sanitizer in your program. Don't enable this feature in production, since it will cause 30% performance loss in serialize.

## Quick to use sonic-rs

To ensure that SIMD instruction is used in sonic-rs, you need to add rustflags `-C target-cpu=native` and compile on the host machine. For example, Rust flags can be configured in Cargo [config](.cargo/config.toml).

Add sonic-rs in `Cargo.toml`

```
[dependencies]
sonic-rs = "0.3"
```

## Features
1. Serde into Rust struct as `serde_json` and `serde`.

2. Parse/Serialize JSON for untyped `sonic_rs::Value`, which can be mutable.

3. Get specific fields from a JSON with the blazing performance.

4. Use JSON as a lazy array or object iterator with the blazing performance.

5. Support `LazyValue`, `Number` and `RawNumber`(just like Golang's `JsonNumber`) in default.

6. The floating parsing precision is as Rust std in default.


## Benchmark

The main optimization in sonic-rs is the use of SIMD. However, we do not use the two-stage SIMD algorithms from `simd-json`. We primarily use SIMD in the following scenarios:
1. parsing/serialize long JSON strings
2. parsing the fraction of float number
3. Getting a specific elem or field from JSON
4. Skipping white spaces when parsing JSON

More details about optimization can be found in [performance.md](docs/performance.md).

Benchmarks environment:

```
Architecture:        x86_64
Model name:          Intel(R) Xeon(R) Platinum 8260 CPU @ 2.40GHz
```
AArch64 benchmark data can be found in [benchmark_aarch64.md](docs/benchmark_aarch64.md).

Benchmarks:

- Deserialize Struct: Deserialize the JSON into Rust struct. The defined struct and testdata is from [json-benchmark](https://github.com/serde-rs/json-benchmark)

- Deseirlize Untyped: Deseialize the JSON into an untyped document

The serialize benchmarks work oppositely.

All deserialized benchmarks enabled UTF-8 validation and enabled `float_roundtrip` in `serde-json` to get sufficient precision as Rust std. 

### Deserialize Struct

The benchmark will parse JSON into a Rust struct, and there are no unknown fields in JSON text. All fields are parsed into struct fields in the JSON. 

Sonic-rs is faster than simd-json because simd-json (Rust) first parses the JSON into a `tape`, then parses the `tape` into a Rust struct. Sonic-rs directly parses the JSON into a Rust struct, and there are no temporary data structures. The [flamegraph](assets/pngs/) is profiled in the citm_catalog case.

`cargo bench --bench deserialize_struct -- --quiet`

```
twitter/sonic_rs::from_slice_unchecked
                        time:   [694.74 µs 707.83 µs 723.19 µs]
twitter/sonic_rs::from_slice
                        time:   [796.44 µs 827.74 µs 861.30 µs]
twitter/simd_json::from_slice
                        time:   [1.0615 ms 1.0872 ms 1.1153 ms]
twitter/serde_json::from_slice
                        time:   [2.2659 ms 2.2895 ms 2.3167 ms]
twitter/serde_json::from_str
                        time:   [1.3504 ms 1.3842 ms 1.4246 ms]

citm_catalog/sonic_rs::from_slice_unchecked
                        time:   [1.2271 ms 1.2467 ms 1.2711 ms]
citm_catalog/sonic_rs::from_slice
                        time:   [1.3344 ms 1.3671 ms 1.4050 ms]
citm_catalog/simd_json::from_slice
                        time:   [2.0648 ms 2.0970 ms 2.1352 ms]
citm_catalog/serde_json::from_slice
                        time:   [2.9391 ms 2.9870 ms 3.0481 ms]
citm_catalog/serde_json::from_str
                        time:   [2.5736 ms 2.6079 ms 2.6518 ms]

canada/sonic_rs::from_slice_unchecked
                        time:   [3.7779 ms 3.8059 ms 3.8368 ms]
canada/sonic_rs::from_slice
                        time:   [3.9676 ms 4.0212 ms 4.0906 ms]
canada/simd_json::from_slice
                        time:   [7.9582 ms 8.0932 ms 8.2541 ms]
canada/serde_json::from_slice
                        time:   [9.2184 ms 9.3560 ms 9.5299 ms]
canada/serde_json::from_str
                        time:   [9.0383 ms 9.2563 ms 9.5048 ms]
```


### Deserialize Untyped

The benchmark will parse JSON into a document. Sonic-rs seems faster for several reasons:
- There are also no temporary data structures in sonic-rs, as detailed above.
- Sonic-rs uses a memory arena for the whole document, resulting in fewer memory allocations, better cache-friendliness, and mutability.
- The JSON object in `sonic_rs::Value` is an array. Sonic-rs does not build a hashmap.

`cargo bench --bench deserialize_value -- --quiet`

```
twitter/sonic_rs_dom::from_slice
                        time:   [550.95 µs 556.10 µs 562.89 µs]
twitter/sonic_rs_dom::from_slice_unchecked
                        time:   [525.97 µs 530.26 µs 536.06 µs]
twitter/serde_json::from_slice
                        time:   [3.7599 ms 3.8009 ms 3.8513 ms]
twitter/serde_json::from_str
                        time:   [2.8618 ms 2.8960 ms 2.9396 ms]
twitter/simd_json::slice_to_owned_value
                        time:   [1.7302 ms 1.7557 ms 1.7881 ms]
twitter/simd_json::slice_to_borrowed_value
                        time:   [1.1870 ms 1.1951 ms 1.2039 ms]

canada/sonic_rs_dom::from_slice
                        time:   [4.9060 ms 4.9568 ms 5.0213 ms]
canada/sonic_rs_dom::from_slice_unchecked
                        time:   [4.7858 ms 4.8728 ms 4.9803 ms]
canada/serde_json::from_slice
                        time:   [16.689 ms 16.980 ms 17.335 ms]
canada/serde_json::from_str
                        time:   [16.398 ms 16.640 ms 16.932 ms]
canada/simd_json::slice_to_owned_value
                        time:   [12.627 ms 12.846 ms 13.070 ms]
canada/simd_json::slice_to_borrowed_value
                        time:   [12.030 ms 12.164 ms 12.323 ms]

citm_catalog/sonic_rs_dom::from_slice
                        time:   [1.6657 ms 1.6981 ms 1.7341 ms]
citm_catalog/sonic_rs_dom::from_slice_unchecked
                        time:   [1.5109 ms 1.5253 ms 1.5424 ms]
citm_catalog/serde_json::from_slice
                        time:   [8.1618 ms 8.2566 ms 8.3653 ms]
citm_catalog/serde_json::from_str
                        time:   [7.8652 ms 8.0706 ms 8.3074 ms]
citm_catalog/simd_json::slice_to_owned_value
                        time:   [3.9834 ms 4.0325 ms 4.0956 ms]
citm_catalog/simd_json::slice_to_borrowed_value
                        time:   [3.3196 ms 3.3433 ms 3.3689 ms]
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

The benchmark is getting a specific field from the `twitter.json`. 

- sonic-rs::get_unchecked_from_str: without validate
- sonic-rs::get_from_str: with validate
- gjson::get_from_str: without validate

Sonic-rs utilize SIMD to quickly skip unnecessary fields in the unchecked case, thus enhancing the performance.

```
twitter/sonic-rs::get_unchecked_from_str
                        time:   [75.671 µs 76.766 µs 77.894 µs]
twitter/sonic-rs::get_from_str
                        time:   [430.45 µs 434.62 µs 439.43 µs]
twitter/gjson::get_from_str
                        time:   [359.61 µs 363.14 µs 367.19 µs]
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

Get a specific field from a JSON with the `pointer` path. The return is a `LazyValue`, which is a wrapper of a raw valid JSON slice. 

We provide the `get` and `get_unchecked` apis. `get_unchecked` apis should be used in valid JSON, otherwise it may return unexpected result.


```rs
use sonic_rs::JsonValueTrait;
use sonic_rs::{get, get_unchecked, pointer};

fn main() {
    let path = pointer!["a", "b", "c", 1];
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}
    "#;
    let target = unsafe { get_unchecked(json, &path).unwrap() };
    assert_eq!(target.as_raw_str(), r#""found""#);
    assert_eq!(target.as_str().unwrap(), "found");

    let target = get(json, &path);
    assert_eq!(target.as_str().unwrap(), "found");
    assert_eq!(target.unwrap().as_raw_str(), r#""found""#);

    let path = pointer!["a", "b", "c", "d"];
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}
    "#;
    // not found from json
    let target = get(json, &path);
    assert!(target.is_err());
}

```

### Parse and Serialize into untyped Value

Parse a JSON into a `sonic_rs::Value`.

```rs
use sonic_rs::{from_str, json};
use sonic_rs::JsonValueMutTrait;
use sonic_rs::{pointer, JsonValueTrait, Value};

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
            "+123456"
        ]
    }"#;

    let mut root: Value = from_str(json).unwrap();

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
    let obj = root.as_object_mut().unwrap();
    obj.insert(&"inserted", true);
    assert!(obj.contains_key(&"inserted"));

    let mut object = json!({ "A": 65, "B": 66, "C": 67 });
    *object.get_mut("A").unwrap() = json!({
        "code": 123,
        "success": false,
        "payload": {}
    });

    let mut val = json!(["A", "B", "C"]);
    *val.get_mut(2).unwrap() = json!("D");

    // serialize
    assert_eq!(serde_json::to_string(&val).unwrap(), r#"["A","B","D"]"#);
}
```

### JSON Iterator

Parse an object or array JSON into a lazy iterator.

```rs
use bytes::Bytes;
use faststr::FastStr;
use sonic_rs::JsonValueTrait;
use sonic_rs::{to_array_iter, to_object_iter_unchecked};
fn main() {
    let json = Bytes::from(r#"[1, 2, 3, 4, 5, 6]"#);
    let iter = to_array_iter(&json);
    for (i, v) in iter.enumerate() {
        assert_eq!(i + 1, v.as_u64().unwrap() as usize);
    }

    let json = Bytes::from(r#"[1, 2, 3, 4, 5, 6"#);
    let iter = to_array_iter(&json);
    for elem in iter {
        // do something for each elem

        // deal with errors when invalid json
        if elem.is_err() {
            assert_eq!(
                elem.err().unwrap().to_string(),
                "Expected this character to be either a ',' or a ']' while parsing at line 1 column 17"
            );
        }
    }

    let json = FastStr::from(r#"{"a": null, "b":[1, 2, 3]}"#);
    let iter = unsafe { to_object_iter_unchecked(&json) };
    for ret in iter {
        // deal with errors
        if ret.is_err() {
            println!("{}", ret.unwrap_err());
            return;
        }

        let (k, v) = ret.unwrap();
        if k == "a" {
            assert!(v.is_null());
        } else if k == "b" {
            let iter = to_array_iter(v.as_raw_str());
            for (i, v) in iter.enumerate() {
                assert_eq!(i + 1, v.as_u64().unwrap() as usize);
            }
        }
    }
}
```

### JSON LazyValue & Number & RawNumber

If we need to parse a JSON value as a raw string, we can use `LazyValue`.

If we need to parse a JSON number into an untyped type, we can use `Number`.

If we need to parse a JSON number ***without loss of precision***, we can use `RawNumber`. It likes `encoding/json.Number` in Golang, and can also be parsed from a JSON string.

Detailed examples can be found in [raw_value.rs](examples/raw_value.rs) and [json_number.rs](examples/json_number.rs).

### Error handle

Sonic's errors are followed as `serde-json` and have a display around the error position, examples in [handle_error.rs](examples/handle_error.rs).


## FAQs

### About UTF-8

By default, sonic-rs enable the UTF-8 validation, except for `xx_unchecked` APIs.


### About floating point precision

By default, sonic-rs uses floating point precision consistent with the Rust standard library, and there is no need to add an extra `float_roundtrip` feature like `serde-json` to ensure floating point precision.

If you want to achieve lossless precision when parsing floating-point numbers, such as Golang `encoding/json.Number` and `serde-json arbitrary_precision`, you can use `sonic_rs::RawNumber`.


## Acknowledgement

Thanks the following open-source libraries. sonic-rs has some references to other open-source libraries like [sonic_cpp](https://github.com/bytedance/sonic-cpp), [serde_json](https://github.com/serde-rs/json), [sonic](https://github.com/bytedance/sonic), [simdjson](https://github.com/simdjson/simdjson), [yyjson](https://github.com/ibireme/yyjson), [rust-std](https://github.com/rust-lang/rust/tree/master/library/core/src/num) and so on.

We rewrote many SIMD algorithms from sonic-cpp/sonic/simdjson/yyjson for performance. We reused the de/ser codes and modified necessary parts from serde_json to make high compatibility with `serde`. We reused part codes about floating parsing from rust-std to make it more accurate.

Referenced papers:
1. [Parsing Gigabytes of JSON per Second](https://arxiv.org/abs/1902.08318)
2. [JSONSki: streaming semi-structured data with bit-parallel fast-forwarding](https://dl.acm.org/doi/10.1145/3503222.3507719)


## Contributing
Please read [CONTRIBUTING.md](CONTRIBUTING.md) for information on contributing to sonic-rs.
