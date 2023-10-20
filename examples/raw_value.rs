use sonic_rs::{Deserialize, Serialize};

use sonic_rs::RawValue;

#[derive(Serialize, Deserialize)]
struct Message {
    msg: Box<RawValue>,
}

fn main() {
    let data = r#"{
  "msg": {"id":1, "name": "Xiaoming"}
}"#;
    let p: Message = sonic_rs::from_str(data).unwrap();
    // get msg as &str
    let msg = p.msg.get();
    assert_eq!(msg, r#"{"id":1, "name": "Xiaoming"}"#);
    let out = sonic_rs::to_string_pretty(&p).unwrap();
    assert_eq!(out, data);
}
