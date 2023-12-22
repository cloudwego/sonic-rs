## Golang to Rust migration

Current version:

`sonic-rs = "0.2.4"`

Corresponding API references:

- Parsing into Golang structures or strong types:

  sonic-go/encoding-json Unmarshal => sonic_rs::from_str/from_slice

  sonic-go/encoding-json Marshal => sonic_rs::to_string/to_vec, etc.

- Parsing into Golang interface{}/any or sonic-go AST:

  If it is a standalone `interface{}`, it is recommended to use `sonic_rs::Document` for better performance.

  If it is an `interface{}` inside a Golang structure, it is recommended to replace `interface{}/any` with `serde_json::Value`. Note: `sonic_rs::Value` and `sonic_rs::Document` are not currently supported for embedding into Rust structures but will be supported later.

- Using gjson/jsonparser for on-demand parsing:

  Regarding gjson/jsonparser get API:

  The gjson/jsonparser get API itself does not perform strict JSON validation, so you can use `sonic_rs::get_unchecked` for replacement. The sonic_rs get API will return a `Result<LazyValue>`, `LazyValue` can be further ***parsed into the corresponding type** by using `as_bool, as_str`, etc. If you need to get the original raw JSON, ***without parsing***, please use `as_raw_str, as_raw_slice` API. Refer to the example: [get_from.rs](examples/get_from.rs)

  Regarding jsonparser `ArrayEach` and `ObjectEach` API:

  The gjson/jsonparser get API itself does not perform strict JSON validation, so you can use `sonic_rs::to_object_iter_unchecked` for replacement. Refer to the example [iterator.rs](examples/iterator.rs)

  If you need to get multiple fields from JSON, it is recommended to use `get_many`. Reference example: [get_many.rs](examples/get_many.rs)

- Parsing into Golang JsonNumber:

  Please use `sonic_rs::RawNumber` directly.

- Parsing into Golang RawMessage:

  Please use `sonic_rs::LazyValue` directly.