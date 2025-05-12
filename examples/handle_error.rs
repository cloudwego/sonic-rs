use sonic_rs::{from_slice, from_str, Deserialize};

fn main() {
    #[allow(dead_code)]
    #[derive(Debug, Deserialize)]
    struct Foo {
        a: Vec<i32>,
        c: String,
    }

    // deal with Eof errors
    let err = from_str::<Foo>("{\"a\": [").unwrap_err();
    assert!(err.is_eof());
    eprintln!("{err}");
    // EOF while parsing at line 1 column 7

    //     {"a": [
    //     ......^
    assert_eq!(
        format!("{err}"),
        "EOF while parsing at line 1 column 7\n\n\t{\"a\": [\n\t......^\n"
    );

    // deal with unmatched type errors
    let err = from_str::<Foo>("{ \"b\":[]}").unwrap_err();
    eprintln!("{err}");
    assert!(err.is_unmatched_type());
    // println as follows:
    // missing field `a` at line 1 column 9
    //
    //     { "b":[]}
    //     ........^
    assert_eq!(
        format!("{err}"),
        "missing field `a` at line 1 column 9\n\n\t{ \"b\":[]}\n\t........^\n"
    );

    // deal with Syntax errors
    let err = from_slice::<Foo>(b"{\"b\":\"\x80\"}").unwrap_err();
    eprintln!("{err}");
    assert!(err.is_syntax());
    // println as follows:
    // Invalid UTF-8 characters in json at line 1 column 7
    //
    //     {"b":"�"}
    //     ......^...
    assert_eq!(
        format!("{err}"),
        "Invalid UTF-8 characters in json at line 1 column 7\n\n\t{\"b\":\"�\"}\n\t......^..\n"
    );
}
