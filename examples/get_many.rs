use sonic_rs::pointer;

fn main() {
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}"#;

    // build a pointer tree, representing multiple json path
    let mut tree = sonic_rs::PointerTree::new();

    tree.add_path(&["u"]);
    tree.add_path(&pointer!["a", "b", "c", 1]);

    let nodes = unsafe { sonic_rs::get_many_unchecked(json, &tree) };

    // the node order is as the order of `add_path`
    for val in nodes.unwrap() {
        println!("{}", val.as_raw_str());
        // 123
        // "found"
    }
}
