// Parse json into sonic_rs `Value`.

use sonic_rs::{from_str, json, pointer, JsonValueMutTrait, JsonValueTrait, Value};

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
    let phones = root.pointer(pointer!["phones", 0]);
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
