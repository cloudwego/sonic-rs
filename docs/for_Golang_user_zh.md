## Golang 迁移 Rust

目前版本：

` sonic-rs = "0.2.4" `


对应 API 参考:

- 解析到 Golang 结构体等强类型:

sonic-go/encoding-json Unmarshal => sonic_rs::from_str/from_slice

sonic-go/encoding-json Marshal => sonic_rs::to_string/to_vec 等

- 解析到 Golang interface{}/any 或 sonic-go AST:

如果是单独的 `interface{}`, 建议使用 `sonic_rs::Document`，性能更优。

如果是 Golang 结构体中的 `interface{}`, 建议将 `interface{}/any` 替换为 `serde_json::Value` 即可。 注意: `sonic_rs::Value` 和 `sonic_rs::Document` 暂时不支持嵌入到 Rust 结构体中, 后续会进行支持。

- 使用 gjson/jsonparser 按需解析:

关于 gjson/jsonparser get API:

gjson/jsonparser get API 本身未做严格的JSON 校验，因此可以使用 `sonic_rs::get_unchecked` 进行平替。sonic_rs get API 会返回一个 `Result<LazyValue>`, `LazyValue` 可以用 `as_bool, as_str`等将 JSON 进一步***解析成对应的类型**, 如果需要拿到原始的raw JSON, ***不做解析***，请使用 `as_raw_str, as_raw_slice` 等 API. 参考例子: [get_from.rs](examples/get_from.rs)


关于 jsonparser `ArrayEach` 和 `ObjectEach` API:

gjson/jsonparser get API 本身未做严格的JSON 校验，因此可以使用 `sonic_rs::to_object_iter_unchecked` 等进行平替。参考例子 [iterator.rs](examples/iterator.rs)


如果需要从 JSON 中拿到多个字段，推荐使用 `get_many`. 参考例子： [get_many.rs](examples/get_many.rs)


- 解析到 Golang JsonNumber:

请直接使用 `sonic_rs::RawNumber`

- 解析到 Golang RawMessage:

请直接使用 `sonic_rs::RawValue`








