## Golang to Rust migration

Current version:

`sonic-rs = "0.3"`

Corresponding API references:

- Parsing into Golang structures or strong types:

  sonic-go/encoding-json Unmarshal => sonic_rs::from_str/from_slice

  sonic-go/encoding-json Marshal => sonic_rs::to_string/to_vec, etc.

- Parsing into Golang `interface{}/any` or sonic-go `ast.Node`:

  It is recommended to replace it with `sonic_rs::Value` for better performance.

- Using `gjson.Get` or `jsonparser.Get` APIs:

  The gjson/jsonparser get API itself does not perform strict JSON validation, so you can use `sonic_rs::get_unchecked` for replacement. 
  
  The sonic_rs get API will return a `Result<LazyValue>`.
  
  `LazyValue` can be further ***parsed into the corresponding type*** by using `as_bool, as_str`, etc. 
  
  If you need to get the original raw JSON, ***without parsing***, please use `as_raw_str, as_raw_slice` API. Refer to the example: [get_from.rs](../examples/get_from.rs)

  If you need to get multiple fields from JSON, it is recommended to use `get_many`. Reference example: [get_many.rs](../examples/get_many.rs)

- Using `gjson.ForEach` or `jsonparser.ObjectEach/ArrayEach`

  These APIs also do not perform strict JSON validation, so you can use `sonic_rs::to_object/array_iter_unchecked` for replacement. Refer to the example [iterator.rs](../examples/iterator.rs)

- Parsing into Golang `json.Number`:

  Please use `sonic_rs::RawNumber` directly.

- Parsing into Golang `json.RawMessage`:

  Please use `sonic_rs::LazyValue<'a>` directly. The lifetime is as the origin JSON. If you want to be owned, pls use `sonic_rs::OwnedLazyValue`. For example, [lazyvalue.rs](../examples/lazyvalue.rs)
