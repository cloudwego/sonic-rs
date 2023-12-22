use sonic_rs::{get, get_unchecked, pointer, JsonValueTrait};

fn main() {
    let path = pointer!["a", "b", "c", 1];
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}
    "#;
    let target = unsafe { get_unchecked(json, &path).unwrap() };
    assert_eq!(target.as_raw_str(), r#""found""#);
    assert_eq!(target.as_str().unwrap(), "found");

    let target = get(json, &path);
    assert_eq!(target.as_str().unwrap(), "found");
    assert_eq!(target.unwrap().as_raw_str(), r#""found""#);

    let path = pointer!["a", "b", "c", "d"];
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}
    "#;
    // not found from json
    let target = get(json, &path);
    assert!(target.is_err());
}
