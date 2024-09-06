## Golang 迁移 Rust

目前版本：

`sonic-rs = "0.3"`

对应 API 参考:

- 解析到 Golang 结构体等强类型:

  sonic-go/encoding-json Unmarshal => sonic_rs::from_str/from_slice

  sonic-go/encoding-json Marshal => sonic_rs::to_string/to_vec 等

- 解析到 Golang `interface{}/any` 或 sonic-go `ast.Node`

  建议使用 `sonic_rs::Value` 替换，性能更优。

- 使用 `gjson.Get` 或 `jsonparser.Get` 等API:
  gjson/jsonparser get API 本身未做严格的JSON 校验，因此可以使用 `sonic_rs::get_unchecked` 进行平替。 sonic_rs get API 会返回一个 `Result<LazyValue>`. 如果没有找到该字段，会报错。
  
  `LazyValue` 可以用 `as_bool, as_str`等将 JSON 进一步**解析成对应的类型**。
  
  如果只需要拿到原始的raw JSON, ***不做解析***，请使用 `as_raw_str, as_raw_faststr` 等 API. 参考例子: [get_from.rs](../examples/get_from.rs)

  如果需要从 JSON 中拿到多个字段，推荐使用 `get_many`. 参考例子： [get_many.rs](../examples/get_many.rs)

- 使用 `gjson.ForEach` or `jsonparser.ObjectEach/ArrayEach` 等API:

  这些 API 也没有对原始 JSON 做严格校验。因此可以使用 `sonic_rs::to_array/object_iter_unchecked` 等进行平替。参考例子 [iterator.rs](../examples/iterator.rs)

- 解析到 Golang JsonNumber:

  请直接使用 `sonic_rs::RawNumber`

- 解析到 Golang RawMessage:

  请直接使用 `sonic_rs::LazyValue<'a>`, 生命周期和输入的JSON绑定，会尽可能减少拷贝开销。如果不想带生命周期，可以使用 `sonic_rs::OwnedLazyValue`. 例如:  [lazyvalue.rs](../examples/lazyvalue.rs)








