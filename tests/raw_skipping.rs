use serde::Deserialize;
use serde_json::value::RawValue;
use sonic_rs::{self as json, LazyValue};

#[derive(Debug, Deserialize)]
struct Envelope<'a, T> {
    id: &'a str,
    data: T,
    tail: u64,
}

fn compare(frame: &str) {
    let before = serde_json::from_str::<Envelope<'_, &RawValue>>(frame);
    let after = json::from_str::<Envelope<'_, LazyValue<'_>>>(frame);
    assert_eq!(before.is_ok(), after.is_ok(), "{frame:.200}");
    assert_eq!(
        serde_json::from_str::<Envelope<'_, serde::de::IgnoredAny>>(frame).is_ok(),
        json::from_str::<Envelope<'_, serde::de::IgnoredAny>>(frame).is_ok(),
        "ignored: {frame:.200}",
    );
    if let (Ok(before), Ok(after)) = (before, after) {
        assert_eq!(before.id, after.id);
        assert_eq!(before.tail, after.tail);
        assert_eq!(before.data.get(), after.data.as_raw_str());
    }
}

#[test]
fn deeply_nested_raw_and_ignored_values_use_the_normal_test_stack() {
    for depth in [0, 1, 2, 32, 160, 256, 1024, 10_000] {
        for (open, close) in [("[", "]"), (r#"{"a":"#, "}"), (r#"[{"a":"#, "}]")] {
            let token = format!("{}1e400{}", open.repeat(depth), close.repeat(depth));
            for (data, ignored) in [(token.as_str(), "null"), ("[]", token.as_str())] {
                let frame = format!(r#"{{"id":"ok","data":{data},"unknown":{ignored},"tail":7}}"#);
                compare(&frame);
                let decoded: Envelope<'_, LazyValue<'_>> = json::from_str(&frame).unwrap();
                assert_eq!(decoded.tail, 7);
            }
        }
    }
}

#[test]
fn shallow_get_many_skips_deeply_nested_unrelated_values() {
    let mut tree = json::PointerTree::new();
    tree.add_path(["wanted"]);
    for (open, close) in [("[", "]"), (r#"{"a":"#, "}")] {
        let nested = format!("{}0{}", open.repeat(10_000), close.repeat(10_000));
        let frame = format!(r#"{{"ignored":{nested},"wanted":1}}"#);
        let values = json::get_many(&frame, &tree).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].as_ref().unwrap().as_raw_str(), "1");
    }
}

#[test]
fn malformed_nested_json_and_truncated_prefixes_match_serde() {
    for raw in [
        "[]",
        "{}",
        "[{},[],[true,false,null],1e400]",
        r#"{"a":[],"b":{"c":["a\u0062","中"]}}"#,
        "[1,]",
        "[,1]",
        "[}",
        "{]",
        r#"{"a" 1}"#,
        r#"{"a":1,}"#,
        r#"{"a":}"#,
        r#"["\uD800"]"#,
        r#"["\uD800\uDC00"]"#,
        "[01]",
        "[1.]",
        "[1e+]",
        "[1 2]",
        "[+1]",
        "[tru]",
        "[truefalse]",
        "[nul]",
    ] {
        let frame = format!(r#"{{"id":"ok","data":{raw},"tail":7}}"#);
        for n in 0..=frame.len() {
            if frame.is_char_boundary(n) {
                compare(&frame[..n]);
            }
        }
    }
}

#[test]
fn mutated_raw_subtrees_match_serde() {
    let values = [
        r#"{"a":[{"b":1.2e+4},null,true,false,"str"]}"#,
        r#"[[],{},0,-1,"a\\\"b"]"#,
    ];
    for raw in values {
        for index in 0..raw.len() {
            for replacement in b"{}[]:,\"\\0129aetufnl+- .\n\t" {
                let mut bytes = raw.as_bytes().to_vec();
                bytes[index] = *replacement;
                let token = std::str::from_utf8(&bytes).unwrap();
                let frame = format!(r#"{{"id":"ok","data":{token},"tail":7}}"#);
                compare(&frame);
            }
        }
    }
}

#[test]
fn typed_values_return_a_depth_error_on_the_normal_stack() {
    for (open, close) in [("[", "]"), (r#"{"a":"#, "}")] {
        let nested = format!("{}0{}", open.repeat(1000), close.repeat(1000));
        assert!(serde_json::from_str::<serde_json::Value>(&nested).is_err());
        let error = json::from_str::<serde_json::Value>(&nested).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Encountered nesting of JSON maps and arrays"),
            "{error}"
        );
    }
    for depth in 0..=127 {
        for (open, close) in [("[", "]"), (r#"{"a":"#, "}")] {
            let frame = format!("{}0{}", open.repeat(depth), close.repeat(depth));
            let before = serde_json::from_str::<serde_json::Value>(&frame);
            let after = json::from_str::<serde_json::Value>(&frame);
            assert_eq!(before.is_ok(), after.is_ok(), "depth: {depth}");
            if let (Ok(before), Ok(after)) = (before, after) {
                assert_eq!(before, after);
            }
        }
    }
}
