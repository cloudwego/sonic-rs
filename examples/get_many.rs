use sonic_rs::pointer;

fn main() {
    let json = r#"
        {"u": 123, "a": {"b" : {"c": [null, "found"]}}}"#;

    // build a pointer tree, representing multiple json path
    let mut tree = sonic_rs::PointerTree::new();

    tree.add_path(&["u"]);
    tree.add_path(&["unknown_key"]);
    tree.add_path(pointer!["a", "b", "c", 1]);

    let nodes = unsafe { sonic_rs::get_many_unchecked(json, &tree) };

    match nodes {
        Ok(vals) => {
            assert_eq!(vals[0].as_ref().unwrap().as_raw_str(), "123");
            assert!(vals[1].is_none());
            assert_eq!(vals[2].as_ref().unwrap().as_raw_str(), "\"found\"");
            for val in vals {
                match val {
                    Some(_) => println!("{}", val.as_ref().unwrap().as_raw_str()),
                    None => println!("None"),
                };
            }
        }
        Err(e) => {
            println!("err: {e:?}")
        }
    }
}
