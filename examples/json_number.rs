use sonic_rs::{from_str, to_string, JsonNumberTrait, Number, RawNumber};

fn main() {
    // parse RawNumber from JSON number
    let number: RawNumber = from_str("  123").unwrap();
    assert_eq!(number.as_str(), "123");
    assert_eq!(to_string(&number).unwrap(), "123");

    // parse RawNumber from JSON string
    let number: RawNumber = from_str(r#""0.123""#).unwrap();
    assert_eq!(number.as_str(), "0.123");
    assert_eq!(to_string(&number).unwrap(), "0.123");
    assert!(number.is_f64());
    assert_eq!(number.as_f64().unwrap(), 0.123);
    assert_eq!(number.as_u64(), None);

    // convert RawNumber to Number
    let num: Number = number.try_into().unwrap();
    assert_eq!(num.as_f64().unwrap(), 0.123);
    assert_eq!(num.as_u64(), None);
}
