use bytes::Bytes;
use sonic_rs::{to_array_iter, JsonValue};

fn main() {
    let json = Bytes::from(r#"[1, 2, 3, 4, 5, 6]"#);
    let iter = to_array_iter(&json);
    for (i, v) in iter.enumerate() {
        assert_eq!(i + 1, v.as_u64().unwrap() as usize);
    }

    let json = Bytes::from(r#"[1, 2, 3, 4, 5, 6"#);
    let iter = to_array_iter(&json);
    for elem in iter {
        // deal with errors when invalid json
        if elem.is_err() {
            assert_eq!(
                elem.err().unwrap().to_string(),
                "Expected this character to be either a ',' or a ']' while parsing at line 1 column 17"
            );
        }
    }
}
