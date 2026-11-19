use std::{
    fs,
    path::{Path, PathBuf},
};

use sonic_rs as json;

fn cases(suite: &str) -> Vec<(PathBuf, Vec<u8>)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/json_test_suite")
        .join(suite);
    let entries =
        fs::read_dir(&directory).unwrap_or_else(|error| panic!("{}: {error}", directory.display()));
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
            .path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            paths.push(path);
        }
    }
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no JSONTestSuite cases in {}",
        directory.display()
    );

    paths
        .into_iter()
        .map(|path| {
            let bytes =
                fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            (path, bytes)
        })
        .collect()
}

#[cfg(not(target_family = "wasm"))]
fn with_large_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn jsontestsuite_parsing() {
    with_large_stack(|| {
        for (path, bytes) in cases("test_parsing") {
            let name = path.file_name().and_then(|name| name.to_str()).unwrap();
            // Skip: optional case exceeds sonic-rs's 255-container limit (src/parser.rs).
            if name == "i_structure_500_nested_arrays.json" {
                continue;
            }
            let result = json::from_slice::<json::Value>(&bytes);
            match name.get(..2) {
                Some("y_") => assert!(result.is_ok(), "{name}: {result:?}"),
                Some("n_") => assert!(result.is_err(), "{name} unexpectedly parsed"),
                Some("i_") => {}
                _ => panic!("unexpected JSONTestSuite case name: {name}"),
            }
        }
    });
}

#[test]
fn jsontestsuite_transform() {
    for (_, bytes) in cases("test_transform") {
        if let Ok(value) = json::from_slice::<json::Value>(&bytes) {
            let _serialized = json::to_vec(&value);
        }
    }
}
