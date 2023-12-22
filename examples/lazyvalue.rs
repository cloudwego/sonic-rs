use serde::{Deserialize, Serialize};
use sonic_rs::{LazyValue, OwnedLazyValue};

fn main() {
    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct TestLazyValue<'a> {
        #[serde(borrow)]
        borrowed_lv: LazyValue<'a>,
        owned_lv: OwnedLazyValue,
    }
    let input = r#"{ "borrowed_lv": "hello", "owned_lv": "world" }"#;
    let data: TestLazyValue = sonic_rs::from_str(input).unwrap();
    assert_eq!(data.borrowed_lv.as_raw_str(), "\"hello\"");
    assert_eq!(data.owned_lv.as_raw_str(), "\"world\"");
}
