use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sonic_rs::{LazyValue, OwnedLazyValue};

fn main() {
    let input = r#"{ "borrowed": "hello", "owned": "world" }"#;

    // use sonic_rs
    #[derive(Debug, Deserialize, Serialize)]
    struct TestLazyValue<'a> {
        #[serde(borrow)]
        borrowed: LazyValue<'a>,
        owned: OwnedLazyValue,
    }
    let data: TestLazyValue = sonic_rs::from_str(input).unwrap();
    assert_eq!(data.borrowed.as_raw_str(), "\"hello\"");

    // use serde_json
    #[derive(Debug, Deserialize, Serialize)]
    struct TestRawValue<'a> {
        #[serde(borrow)]
        borrowed: &'a RawValue,
        owned: Box<RawValue>,
    }

    let data: TestRawValue = serde_json::from_str(input).unwrap();
    assert_eq!(data.borrowed.get(), "\"hello\"");
    assert_eq!(data.owned.get(), "\"world\"");
}
