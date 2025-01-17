use std::collections::HashSet;

use faststr::FastStr;
use serde::{ser::SerializeMap, Serializer};
use sonic_rs::{to_object_iter, writer::WriteExt};

#[allow(clippy::mutable_key_type)]
fn filter_json<W: WriteExt>(json: &str, keys: HashSet<FastStr>, w: W) -> sonic_rs::Result<()> {
    // create a new serialize from writer
    let mut outer = sonic_rs::Serializer::new(w);

    // begin to serialize a map
    let mut maper = outer.serialize_map(None)?;
    for ret in to_object_iter(json) {
        let (name, value) = ret.expect("invalid json");
        if keys.contains(name.as_ref()) {
            maper.serialize_entry(&name, &value)?;
        }
    }
    maper.end()
}

fn main() {
    let json = r#"{
        "a": 1,
        "b": "2",
        "c": [3, 4, 5],
        "d": {
            "e": 6,
            "f": "7",
            "g": [8, 9, 10]
        }
    }"#;
    #[allow(clippy::mutable_key_type)]
    let keys = ["a", "c"].iter().map(|s| FastStr::from(*s)).collect();
    let mut buf = Vec::new();
    filter_json(json, keys, &mut buf).unwrap();
    assert_eq!(buf, br#"{"a":1,"c":[3, 4, 5]}"#);
}
