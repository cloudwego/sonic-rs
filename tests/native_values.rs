use bytes::Bytes;
use sonic_rs::{
    from_slice, from_str, parse_owned_lazy_value, parse_value, to_string, Deserialize, FastStr,
    JsonValueTrait, OwnedLazyValue, Value,
};

#[test]
fn native_and_nested_values_preserve_the_same_data() {
    #[derive(Deserialize)]
    struct Fields {
        dom: Value,
        lazy: OwnedLazyValue,
    }

    let raw = r#"{ "n": 1.230000000000000000000001, "s": "\u0061" }"#;
    let dom = parse_value(raw).unwrap();
    let lazy = {
        let backing = FastStr::new(raw);
        parse_owned_lazy_value(&backing).unwrap()
    };
    let nested: Fields = from_str(&format!(r#"{{"dom":{raw},"lazy":{raw}}}"#)).unwrap();
    assert_eq!(dom, nested.dom);
    assert_eq!(to_string(&lazy).unwrap(), raw);
    assert_eq!(to_string(&nested.lazy).unwrap(), raw);
    assert_eq!(dom["s"].as_str(), Some("a"));
    #[cfg(feature = "arbitrary_precision")]
    assert_eq!(to_string(&dom["n"]).unwrap(), "1.230000000000000000000001");

    let bytes = Bytes::copy_from_slice(raw.as_bytes());
    assert_eq!(parse_value(&bytes).unwrap(), dom);
    assert_eq!(
        to_string(&parse_owned_lazy_value(&bytes).unwrap()).unwrap(),
        raw
    );
}

#[test]
fn native_entry_points_check_the_entire_document() {
    for input in ["", "[", "null null", "[0]x"] {
        assert!(parse_value(input).is_err());
        assert!(parse_owned_lazy_value(input).is_err());
    }

    let input = &[b'"', 0xff, b'"'][..];
    #[cfg(feature = "utf8_lossy")]
    assert_eq!(parse_value(input).unwrap().as_str(), Some("�"));
    #[cfg(not(feature = "utf8_lossy"))]
    assert!(parse_value(input).is_err());
    // Owned lazy strings retain UTF-8 JSON text.
    assert!(parse_owned_lazy_value(input).is_err());
    assert!(from_slice::<Vec<OwnedLazyValue>>(b"[\"\xff\"]").is_err());
}
