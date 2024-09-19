use crate::{
    parser::Parser, reader::Reader, util::utf8::from_utf8, value::Value, JsonInput, Read, Result,
};

/// get_by_schema returns new Value from the given schema and json data
///
/// This function can be used to query json data on demand.
///
/// If a key path, for example \["b", "b1"\] exists in the schema, but does not exist in the data,
/// then the data will be filled with the corresponding default value in the schema.
///
/// If a key path does'not exists in the schema, but exists in the data, then the
/// value in the schema will be replaced with the corresponding value in the data.
///
/// # Examples
/// ```
/// use sonic_rs::json;
/// let schema = json!({
///     "a": null, // default value is `null`
///     "b": {
///         "b1": {},
///         "b2": "default string" // default value is string
///     },
///     "c": [], // default value is []
/// });
///
/// let data = r#"
/// {
///     "a": {},
///     "b": {
///         "b1": 123
///     },
///     "c": [1, 2, 3],
///     "d": "balabala..."
/// }"#;
///
/// // parse json data by schem, we can parse into the schema value inplace
/// let got = sonic_rs::get_by_schema(data, schema).unwrap();
/// assert_eq!(
///     got,
///     json!({
///         "a": {},
///         "b": {
///             "b1": 123,
///             "b2": "default string"
///         },
///         "c": [1, 2, 3]
///     })
/// );
/// ```
pub fn get_by_schema<'de, Input: JsonInput<'de>>(json: Input, mut schema: Value) -> Result<Value> {
    let slice = json.to_u8_slice();
    let reader = Read::new(slice, false);
    let mut parser = Parser::new(reader);
    parser.get_by_schema(&mut schema)?;

    // validate the utf-8 if slice
    let index = parser.read.index();
    if json.need_utf8_valid() {
        from_utf8(&slice[..index])?;
    }
    Ok(schema)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::from_str;

    fn test_success(schema: &str, json: &str, expected: &str) {
        let schema = from_str(schema).unwrap();
        let actual = get_by_schema(json, schema).unwrap();
        assert_eq!(from_str::<Value>(expected).unwrap(), actual);
    }

    fn test_failed(schema: &str, json: &str) {
        let schema = from_str(schema).unwrap();
        let actual = get_by_schema(json, schema);
        assert!(actual.is_err());
    }

    #[test]
    fn test_success_1() {
        let (schema, json, expected) = (
            r#"{"true": null, "false": null, "null":null, "int": null, "double":null, 
        "string": null, "object": null, "array": null}"#,
            r#"{"true": true, "false": false, "null": null, "int": 1, "double": 1.0, "string": "string", 
        "object": {
            "object": {},
            "array": []
        },
        "array": [{}, [], {"a":1}, [-1]]
    }"#,
            r#"{"true": true, "false": false, "null": null, "int": 1, "double": 1.0, "string": "string", 
        "object": {
            "object": {},
            "array": []
        },
        "array": [{}, [], {"a":1}, [-1]]
    }"#,
        );
        test_success(schema, json, expected);
    }

    #[test]
    fn test_success_2() {
        let (schema, json, expected) = (
            r#"{"true": false, "false": true, "null": {}, "int": 2, "double":2.0, 
        "string": "", "object": null, "array": []}"#,
            r#"{"true": true, "false": false, "null": null, "int": 1, "double": 1.0, "string": "string", 
        "object": {
            "object": {},
            "array": [{}, [], [{}, []], true, null, "str", 1, 1.0]
        },
        "array": [{}, [], [{}, []], true, null, "str", 1, 1.0]
    }"#,
            r#"{"true": true, "false": false, "null": null, "int": 1, "double": 1.0, "string": "string", 
        "object": {
            "object": {},
            "array": [{}, [], [{}, []], true, null, "str", 1, 1.0]
        },
        "array": [{}, [], [{}, []], true, null, "str", 1, 1.0]
    }"#,
        );
        test_success(schema, json, expected);
    }

    #[test]
    fn test_success_3() {
        let (schema, json, expected) = (
            r#"{"true": null, "false": null, "null":null, "int": null, "double":null, 
        "string": null, "object": null, "array": null}"#,
            "[]",
            "[]",
        );
        test_success(schema, json, expected);
    }

    #[test]
    fn test_success_4() {
        let (schema, json, expected) =
            (r#"{"obj":{}}"#, r#"{"obj":{"a":1}}"#, r#"{"obj":{"a":1}}"#);
        test_success(schema, json, expected);
    }
    #[test]
    fn test_success_5() {
        let (schema, json, expected) = (
            r#"{"obj":{"a":2}}"#,
            r#"{"obj":{"a":1, "b":1}}"#,
            r#"{"obj":{"a":1}}"#,
        );
        test_success(schema, json, expected);
    }
    #[test]
    fn test_success_6() {
        let (schema, json, expected) = (
            r#"{"bool2bool":true, "bool2int":1, "bool2dbl": 1.0, "bool2str": "str",
        "bool2null": null, "bool2obj": {}, "bool2arr": [],
        "int2bool":true, "int2int":1, "int2dbl": 1.0, "int2str": "str",
        "int2null": null, "int2obj": {}, "int2arr": [],
        "dbl2bool":true, "dbl2int":1, "dbl2dbl": 1.0, "dbl2str": "str",
        "dbl2null": null, "dbl2obj": {}, "dbl2arr": [],
        "str2bool":true, "str2int":1, "str2dbl": 1.0, "str2str": "str",
        "str2null": null, "str2obj": {}, "str2arr": [],
        "null2bool":true, "null2int":1, "null2dbl": 1.0, "null2str": "str",
        "null2null": null, "null2obj": {}, "null2arr": [],
        "obj2bool":true, "obj2int":1, "obj2dbl": 1.0, "obj2str": "str",
        "obj2null": null, "obj2obj": {}, "obj2arr": [],
        "arr2bool":true, "arr2int":1, "arr2dbl": 1.0, "arr2str": "str",
        "arr2null": null, "arr2obj": {}, "arr2arr": []
        }"#,
            r#"{
       "bool2bool":false, "bool2int":false, "bool2dbl": false, "bool2str": false,
        "bool2null": false, "bool2obj": false, "bool2arr": false,
        "int2bool":2, "int2int":2, "int2dbl": 2, "int2str": 2,
        "int2null": 2, "int2obj": 2, "int2arr": 2,
        "dbl2bool":3.0, "dbl2int":3.0, "dbl2dbl": 3.0, "dbl2str": 3.0,
        "dbl2null": 3.0, "dbl2obj": 3.0, "dbl2arr": 3.0,
        "str2bool":"string", "str2int":"string", "str2dbl": "string", "str2str": "string",
        "str2null": "string", "str2obj": "string", "str2arr": "string",
        "null2bool":null, "null2int":null, "null2dbl": null, "null2str": null,
        "null2null": null, "null2obj": null, "null2arr": null,
        "obj2bool": {"a":1}, "obj2int":{"a":1}, "obj2dbl": {"a":1}, "obj2str":{"a":1},
        "obj2null": {"a":1}, "obj2obj": {"a":1}, "obj2arr": {"a":1},
        "arr2bool":[1], "arr2int":[1], "arr2dbl": [1], "arr2str": [1],
        "arr2null": [1], "arr2obj": [1], "arr2arr": [1] 
    }"#,
            r#"{
       "bool2bool":false, "bool2int":false, "bool2dbl": false, "bool2str": false,
        "bool2null": false, "bool2obj": false, "bool2arr": false,
        "int2bool":2, "int2int":2, "int2dbl": 2, "int2str": 2,
        "int2null": 2, "int2obj": 2, "int2arr": 2,
        "dbl2bool":3.0, "dbl2int":3.0, "dbl2dbl": 3.0, "dbl2str": 3.0,
        "dbl2null": 3.0, "dbl2obj": 3.0, "dbl2arr": 3.0,
        "str2bool":"string", "str2int":"string", "str2dbl": "string", "str2str": "string",
        "str2null": "string", "str2obj": "string", "str2arr": "string",
        "null2bool":null, "null2int":null, "null2dbl": null, "null2str": null,
        "null2null": null, "null2obj": null, "null2arr": null,
        "obj2bool": {"a":1}, "obj2int":{"a":1}, "obj2dbl": {"a":1}, "obj2str":{"a":1},
        "obj2null": {"a":1}, "obj2obj": {"a":1}, "obj2arr": {"a":1},
        "arr2bool":[1], "arr2int":[1], "arr2dbl": [1], "arr2str": [1],
        "arr2null": [1], "arr2obj": [1], "arr2arr": [1] 
    }"#,
        );
        test_success(schema, json, expected);
    }

    #[test]
    fn test_success_7() {
        let (schema, json, expected) = (
            r#"{"a":1, "b":[1], "c": {"d":1}}"#,
            r#"{"o":2, "p":[2], "c":{"k":1}}"#,
            r#"{"a":1,"b":[1],"c":{"d":1}}"#,
        );
        test_success(schema, json, expected);
    }

    #[test]
    fn test_success_8() {
        let (schema, json, expected) = (
            r#"{"a":1, "b":[1], "c": {}}"#,
            r#"{"o":2, "b":[2], "c":{"k":1}}"#,
            r#"{"a":1,"b":[2],"c":{"k":1}}"#,
        );
        test_success(schema, json, expected);
    }

    #[test]
    fn test_failed_1() {
        let (schema, json) = ("{}", "nul");
        test_failed(schema, json);
    }

    #[test]
    fn test_failed_2() {
        let (schema, json) = ("{}", "fals");
        test_failed(schema, json);
    }
    #[test]
    fn test_failed_3() {
        let (schema, json) = ("{}", "tru");
        test_failed(schema, json);
    }
    #[test]
    fn test_failed_4() {
        let (schema, json) = ("{}", "string");
        test_failed(schema, json);
    }
    #[test]
    fn test_failed_5() {
        let (schema, json) = ("{}", r#"{"obj":}"#);
        test_failed(schema, json);
    }
    #[test]
    fn test_failed_6() {
        let (schema, json) = ("{}", "[null,]");
        test_failed(schema, json);
    }
    #[test]
    fn test_failed_7() {
        let (schema, json) = ("true", "{}");
        test_failed(schema, json);
    }
    #[test]
    fn test_failed_8() {
        let (schema, json) = (r#"{"a": 1}"#, "{123}");
        test_failed(schema, json);
    }
}
