# sonic-rs

[![Crates.io](https://img.shields.io/crates/v/sonic-rs)](https://crates.io/crates/sonic-rs)
[![Documentation](https://docs.rs/sonic-rs/badge.svg)](https://docs.rs/sonic-rs)
[![Website](https://img.shields.io/website?up_message=cloudwego&url=https%3A%2F%2Fwww.cloudwego.io%2F)](https://www.cloudwego.io/)
[![License](https://img.shields.io/crates/l/sonic-rs)](#license)
[![Build Status][actions-badge]][actions-url]

[actions-badge]: https://github.com/cloudwego/sonic-rs/actions/workflows/ci.yml/badge.svg
[actions-url]: https://github.com/cloudwego/sonic-rs/actions

中文 | [English](README.md)

sonic-rs 是一个基于 SIMD 的高性能 JSON 库。它参考了其他开源库如 [sonic_cpp](https://github.com/bytedance/sonic-cpp)，[serde_json](https://github.com/serde-rs/json)，[sonic](https://github.com/bytedance/sonic)，[simdjson](https://github.com/simdjson/simdjson)，[rust-std](https://github.com/rust-lang/rust/tree/master/library/core/src/num) 等。

sonic-rs 的主要优化是使用 SIMD。然而，sonic-rs 没有使用来自`simd-json`的两阶段SIMD算法。sonic-rs 主要在以下场景中使用 SIMD：
1. 解析/序列化长 JSON 字符串
2. 解析浮点数的小数部分
3. 从 JSON 中获取特定元素或字段
4. 在解析JSON时跳过空格

有关优化的更多细节，请参见 [performance_zh.md](docs/performance_zh.md)。

## ***要求/注意事项***

1. 支持 x86_64 或 aarch64，aarch64 的性能较低，需要优化。
2. 需要 Rust nightly 版本，因为 sonic-rs 使用了 `packed_simd` 包。
3. 使用 `get_from`、`get_many`、`JsonIter` 或 `RawValue` 时，JSON 应该是格式正确且有效的。

## 功能

1. JSON 与 Rust 结构体之间的序列化，基于兼容 `serde_json` 和 `serde`。
2. JSON 与 document 之间的序列化，document是可变数据结构
3. 从 JSON 中获取特定字段
4. 将 JSON 解析为惰性迭代器
5. 在默认情况下支持 `RawValue`，`Number` 和 `RawNumber`（就像 Golang 的 `JsonNumber`）。
6. 浮点数精度默认和 Rust 标准库对齐

## 如何使用 sonic-rs

要确保在 sonic-rs 中使用 SIMD 指令，您需要添加 rustflags `-C target-cpu=native` 并在主机上进行编译。例如，Rust 标志可以在 Cargo [config](.cargo/config) 中配置。

在 Cargo 依赖中添加 sonic-rs:
```
[dependencies]
sonic-rs = 0.2.0
```


## 基准测试

基准测试环境:

```
Architecture:        x86_64
Model name:          Intel(R) Xeon(R) Platinum 8260 CPU @ 2.40GHz
```

基准测试主要有两个方面：

- 解析到结构体：定义的结构体和测试数据来自 [json-benchmark][https://github.com/serde-rs/json-benchmark]

- 解析到 document

序列化基准测试也是如此。

解析相关 benchmark 都开启了 UTF-8 校验，同时 `serde-json` 开启了 `float_roundtrip` feature, 以便解析浮点数具有足够精度，和 Rust 标准库对齐。

### 解析到结构体

基准测试将把 JSON 解析成 Rust 结构体，JSON 文本中没有未知字段。JSON 中的所有字段都被解析为结构体字段。

Sonic-rs 比 simd-json 更快，因为 simd-json (Rust) 首先将 JSON 解析成 `tape`，然后将 `tape` 解析成 Rust 结构体。Sonic-rs 直接将 JSON 解析成 Rust 结构体，没有临时数据结构。在 citm_catalog 案例中对 [flamegraph](assets/pngs/) 进行了分析。

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


### 解析到 document

该测试将把 JSON 解析成 document。由于以下几个原因，Sonic-rs 会看起来更快一些：
- 如上所述，在 sonic-rs 中没有临时数据结构，例如 `tape`。
- Sonic-rs 为整个 document 使用内存区，从而减少内存分配、提高缓存友好性和可变性。
- sonic-rs document中的 JSON 对象实际上是一个向量。Sonic-rs 不会构建 hashmap。

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



### 序列化 document

`cargo bench --bench serialize_value  -- --quiet`

在以下基准测试中，对于 `twitter` JSON，sonic-rs 看似更快。 因为 `twitter` JSON 包含许多长 JSON 字符串，这非常适合 sonic-rs 的 SIMD 优化。

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

### 序列化 Rust 结构体
`cargo bench --bench serialize_struct  -- --quiet`

解释如上所述。

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

### 从 JSON 中获取

`cargo bench --bench get_from -- --quiet`

基准测试是从 twitter JSON 中获取特定字段。在 sonic-rs 和 gjson 中，使用 get 或 get_from 时，JSON 应该格式正确且有效。Sonic-rs 利用 SIMD 快速跳过不必要的字段，从而提高性能。

```
twitter/sonic-rs::get_from_str
                        time:   [79.432 µs 80.008 µs 80.738 µs]
twitter/gjson::get      time:   [344.41 µs 351.36 µs 362.03 µs]
```

## 用法

### 对 Rust 类型解析/序列化

直接使用 `Deserialize` 或 `Serialize` trait。

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

### 从 JSON 中获取字段

使用 `pointer` 路径从 JSON 中获取特定字段。返回的是 `LazyValue`，本质上是一段未解析的 JSON 切片。请注意，使用该 API 需要保证 JSON 是格式良好且有效的，否则可能返回非预期结果。

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


# 解析/序列化 document

在 sonic-rs 中，JSON 可以被解析未可修改的document。需要注意，document 是由 bump 分配器管理。建议将 document 转换为 Object/ObjectMut 或 Array/ArrayMut。这样能够确保强类型，同时在使用时可以对 allocator 无感知。

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

将 JSON object 或 Array 解析为惰性迭代器。迭代器的 Item 是 `LazyValue` 或 `Result<LazyValue>`。

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

如果我们需要得到原始的 JSON 文本，可以使用 RawValue。 如果我们需要将 JSON 数字解析为 untyped number，可以使用 Number。 如果我们需要解析 JSON 数字时***不丢失精度**，可以使用 RawNumber，它类似于 Golang 中的 JsonNumber。

详细示例可以在[raw_value.rs](examples/raw_value.rs) 和 [json_number.rs](examples/json_number.rs) 中找到。

## 常见问题

### 关于 UTF-8

sonic-rs 默认并不开启 utf-8 校验，这是为了性能做出的权衡。

- 对于 `from_slice` 和 `dom_from_slice` 接口，默认开启了 `utf8` 校验。如果用户确保是 `utf-8`, 也可以使用 `from_slice_unchecked` 和 `dom_from_slice_unchecked`。

- 对于 `get` 和 `lazyvaue` 相关接口，由于实现算法设计的原因，这些接口***只适合在 valid-json 场景下使用***，我们后续也不会提供 utf-8 校验。

### 关于浮点数精度

sonic-rs 默认使用和 Rust 标准库一致的浮点数精度，无需像 `serde-json` 那样添加额外的 `float_roundtrip` feature 来保证浮点数精度。

如果想在解析浮点数时，做到精度无损失，例如 Golang `JsonNumber` 和 `serde-json arbitrary_precision`，可以使用 `RawNumber`。

## 致谢

Thanks the following open-source libraries. sonic-rs has some references to other open-source libraries like [sonic_cpp](https://github.com/bytedance/sonic-cpp), [serde_json](https://github.com/serde-rs/json), [sonic](https://github.com/bytedance/sonic), [simdjson](https://github.com/simdjson/simdjson), [yyjson](https://github.com/ibireme/yyjson), [rust-std](https://github.com/rust-lang/rust/tree/master/library/core/src/num) and so on.

我们为了性能重写了来自 sonic-cpp/sonic/simdjson/yyjson 的许多 SIMD 算法。我们重用了来自 serde_json 的反/序列化代码，并修改了必要的部分以与 serde 高度兼容。我们重用了来自 rust-std 的部分浮点解析代码，使其结构更准确。

## 如何贡献

Please read `CONTRIBUTING.md` for information on contributing to sonic-rs.
