use bytes::Bytes;
use faststr::FastStr;
use sonic_rs::{to_array_iter, to_object_iter_unchecked, JsonValueTrait};
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
            assert!(elem.err().unwrap().to_string().starts_with(
                "Expected this character to be either a ',' or a ']' while parsing at line 1 \
                 column 17"
            ));
        }
    }

    let json = FastStr::from(r#"{"a": null, "b":[1, 2, 3]}"#);
    let iter = unsafe { to_object_iter_unchecked(&json) };
    for ret in iter {
        // deal with errors
        if let Err(e) = ret {
            println!("{}", e);
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
