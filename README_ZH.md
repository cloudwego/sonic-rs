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


***对于 Golang 用户迁移 Rust 使用 `sonic_rs`, 请参考 [for_Golang_user_zh.md](https://github.com/cloudwego/sonic-rs/blob/main/docs/for_Golang_user_zh.md)***

***对于 用户从 `serde_json` 迁移 `sonic_rs`, 请参考 [serdejson_compatibility](https://github.com/cloudwego/sonic-rs/blob/main/docs/serdejson_compatibility.md)***

## ***要求/注意事项***

1. 支持 x86_64 或 aarch64。其他架构下走 fallback 逻辑，性能较差。

2. ~~需要 Rust nightly 版本~~ 已经支持 Rust Stable。

3. 在编译选项中开启 `-C target-cpu=native`

## 如何使用 sonic-rs

要确保在 sonic-rs 中使用 SIMD 指令，您需要添加 rustflags `-C target-cpu=native` 并在主机上进行编译。例如，Rust 标志可以在 Cargo [config](.cargo/config.toml) 中配置。

在 Cargo 依赖中添加 sonic-rs:
```
[dependencies]
sonic-rs = "0.3"
```

## 功能

1. JSON 与 Rust 结构体之间的序列化，基于兼容 `serde_json` 和 `serde`。
2. JSON 与 document 之间的序列化，document是可变数据结构
3. 从 JSON 中获取特定字段
4. 将 JSON 解析为惰性迭代器
5. 在默认情况下支持 `LazyValue`，`Number` 和 `RawNumber`（就像 Golang 的 `JsonNumber`）。
6. 浮点数精度默认和 Rust 标准库对齐


## 基准测试

sonic-rs 的主要优化是使用 SIMD。然而，sonic-rs 没有使用来自`simd-json`的两阶段SIMD算法。sonic-rs 主要在以下场景中使用 SIMD：
1. 解析/序列化长 JSON 字符串
2. 解析浮点数的小数部分
3. 从 JSON 中获取特定元素或字段
4. 在解析JSON时跳过空格

有关优化的更多细节，请参见 [performance_zh.md](docs/performance_zh.md)。

基准测试环境:

```
Architecture:        x86_64
Model name:          Intel(R) Xeon(R) Platinum 8260 CPU @ 2.40GHz
```
AArch64 架构下的测试数据见 [benchmark_aarch64.md](docs/benchmark_aarch64.md)。

基准测试主要有两个方面：

- 解析到结构体：定义的结构体和测试数据来自 [json-benchmark](https://github.com/serde-rs/json-benchmark)

- 解析到 document

序列化基准测试也是如此。

解析相关 benchmark 都开启了 UTF-8 校验，同时 `serde-json` 开启了 `float_roundtrip` feature, 以便解析浮点数具有足够精度，和 Rust 标准库对齐。

### 解析到结构体

基准测试将把 JSON 解析成 Rust 结构体，JSON 文本中没有未知字段。JSON 中的所有字段都被解析为结构体字段。

Sonic-rs 比 simd-json 更快，因为 simd-json (Rust) 首先将 JSON 解析成 `tape`，然后将 `tape` 解析成 Rust 结构体。Sonic-rs 直接将 JSON 解析成 Rust 结构体，没有临时数据结构。在 citm_catalog 案例中对 [flamegraph](assets/pngs/) 进行了分析。

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


### 解析到 document

该测试将把 JSON 解析成 document。由于以下几个原因，Sonic-rs 会看起来更快一些：
- 如上所述，在 sonic-rs 中没有临时数据结构，例如 `tape`。
- Sonic-rs 使用内存池为整个 document 使用内存区，从而减少内存分配、提高缓存友好性和可变性。
- 如果 JSON 是object, 解析到`sonic_rs::Value`后，底层是一个 Key-Value pair 的数组，而不会建立 HashMap 或 BTreeMap, 因此没有建表开销。

`cargo bench --bench deserialize_value -- --quiet`

```
twitter/sonic_rs_dom::from_slice
                        time:   [621.16 µs 624.89 µs 628.91 µs]
twitter/sonic_rs_dom::from_slice_unchecked
                        time:   [588.34 µs 594.28 µs 601.36 µs]
twitter/simd_json::slice_to_borrowed_value
                        time:   [1.3001 ms 1.3400 ms 1.3853 ms]
twitter/serde_json::from_slice
                        time:   [3.9263 ms 3.9822 ms 4.0463 ms]
twitter/serde_json::from_str
                        time:   [2.8608 ms 2.9187 ms 2.9907 ms]
twitter/simd_json::slice_to_owned_value
                        time:   [1.7870 ms 1.8044 ms 1.8230 ms]

citm_catalog/sonic_rs_dom::from_slice
                        time:   [1.8024 ms 1.8234 ms 1.8469 ms]
citm_catalog/sonic_rs_dom::from_slice_unchecked
                        time:   [1.7280 ms 1.7731 ms 1.8235 ms]
citm_catalog/simd_json::slice_to_borrowed_value
                        time:   [3.5792 ms 3.6082 ms 3.6386 ms]
citm_catalog/serde_json::from_slice
                        time:   [8.4606 ms 8.5654 ms 8.6896 ms]
citm_catalog/serde_json::from_str
                        time:   [9.3020 ms 9.4903 ms 9.6760 ms]
citm_catalog/simd_json::slice_to_owned_value
                        time:   [4.3144 ms 4.4268 ms 4.5604 ms]

canada/sonic_rs_dom::from_slice
                        time:   [5.1103 ms 5.1784 ms 5.2654 ms]
canada/sonic_rs_dom::from_slice_unchecked
                        time:   [4.8870 ms 4.9165 ms 4.9499 ms]
canada/simd_json::slice_to_borrowed_value
                        time:   [12.583 ms 12.866 ms 13.178 ms]
canada/serde_json::from_slice
                        time:   [17.054 ms 17.218 ms 17.414 ms]
canada/serde_json::from_str
                        time:   [17.140 ms 17.363 ms 17.614 ms]
canada/simd_json::slice_to_owned_value
                        time:   [12.351 ms 12.503 ms 12.666 ms]
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

基准测试是从 twitter JSON 中获取特定字段。

- sonic-rs::get_unchecked_from_str: 不校验json
- sonic-rs::get_from_str: 校验json
- gjson::get_from_str: 不校验json

在 get_unchecked_from_str 中，Sonic-rs 利用 SIMD 快速跳过不必要的字段，从而提高性能。

```
twitter/sonic-rs::get_unchecked_from_str
                        time:   [75.671 µs 76.766 µs 77.894 µs]
twitter/sonic-rs::get_from_str
                        time:   [430.45 µs 434.62 µs 439.43 µs]
twitter/gjson::get_from_str
                        time:   [359.61 µs 363.14 µs 367.19 µs]
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

### 按路径从 JSON 中获取特定字段

按`pointer` 路径从 JSON 中获取特定字段。返回的是 `LazyValue`，本质上是一段未解析的 JSON 切片。

sonic-rs 提供了 `get` 和 `get_unchecked` 两种接口。请注意，如果使用 `unchecked` 接口，需要保证 输入的JSON 是格式良好且合法的，否则可能返回非预期结果。

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


# 解析/序列化 document

将 JSON 解析为 `sonic_rs::Value`.

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

将 JSON object 或 array 解析为惰性迭代器。

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

如果我们需要得到原始的 JSON 文本，可以使用 LazyValue. 如果我们需要将 JSON 数字解析为 untyped number，可以使用 Number。 如果我们需要解析 JSON 数字时***不丢失精度**，可以使用 RawNumber，它类似于 Golang 中的 JsonNumber。

详细示例可以在[raw_value.rs](examples/raw_value.rs) 和 [json_number.rs](examples/json_number.rs) 中找到。


### 错误处理

sonic-rs的错误处理参考了 serde-json,同时加上了对错误位置的描述, 例子在[handle_error.rs](examples/handle_error.rs).

## 常见问题

### 关于 UTF-8

sonic-rs 默认开启了 UTF-8 校验，在使用 `unsafe` 的 API时，其内部并未校验 UTF-8。

### 关于浮点数精度

sonic-rs 默认使用和 Rust 标准库一致的浮点数精度，无需像 `serde-json` 那样添加额外的 `float_roundtrip` feature 来保证浮点数精度。

如果想在解析浮点数时，做到精度无损失，例如 Golang `JsonNumber` 和 `serde-json arbitrary_precision`，可以使用 `RawNumber`。

## 致谢

Thanks the following open-source libraries. sonic-rs has some references to other open-source libraries like [sonic_cpp](https://github.com/bytedance/sonic-cpp), [serde_json](https://github.com/serde-rs/json), [sonic](https://github.com/bytedance/sonic), [simdjson](https://github.com/simdjson/simdjson), [yyjson](https://github.com/ibireme/yyjson), [rust-std](https://github.com/rust-lang/rust/tree/master/library/core/src/num) and so on.

我们为了性能重写了来自 sonic-cpp/sonic/simdjson/yyjson 的许多 SIMD 算法。我们重用了来自 serde_json 的反/序列化代码，并修改了必要的部分以与 serde 高度兼容。我们重用了来自 rust-std 的部分浮点解析代码，使其结构更准确。

参考论文:
1. [Parsing Gigabytes of JSON per Second](https://arxiv.org/abs/1902.08318)
2. [JSONSki: streaming semi-structured data with bit-parallel fast-forwarding](https://dl.acm.org/doi/10.1145/3503222.3507719)


## 如何贡献

请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。
