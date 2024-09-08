use sonic_rs::json;

fn main() {
    let schema = json!({
        "a": null, // default value is `null`
        "b": {
            "b1": {},
            "b2": "default string" // default value is string
        },
        "c": [], // default value is []
    });

    let data = r#"
    {
        "a": {},
        "b": {
            "b1": 123
        },
        "c": [1, 2, 3],
        "d": "balabala..."
    }"#;

    // parse json data by schem, we can parse into the schema value inplace
    let got = sonic_rs::get_by_schema(data, schema).unwrap();
    assert_eq!(
        got,
        json!({
            "a": {},
            "b": {
                "b1": 123,
                "b2": "default string"
            },
            "c": [1, 2, 3]
        })
    );
}
