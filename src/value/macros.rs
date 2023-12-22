// The file is copied from `serde_json` and modified.

/// Construct a `sonic_rs::Value` from a JSON literal.
///
/// ```
/// # use sonic_rs::json;
/// #
/// let value = json!({
///     "code": 200,
///     "success": true,
///     "payload": {
///         "features": [
///             "serde",
///             "json"
///         ],
///         "homepage": null
///     }
/// });
/// ```
///
/// Variables or expressions can be interpolated into the JSON literal. Any type
/// interpolated into an array element or object value must implement Serde's
/// `Serialize` trait, while any type interpolated into a object key must
/// implement `AsRef<str>`. If the `Serialize` implementation of the
/// interpolated type decides to fail, or if the interpolated type contains a
/// map with non-string keys, the `json!` macro will panic.
///
/// ```
/// # use sonic_rs::json;
/// #
/// let code = 200;
/// let features = vec!["sonic_rs", "json"];
///
/// let value = json!({
///     "code": code,
///     "success": code == 200,
///     "payload": {
///         "features": features,
///         features[0]: features[1]
///     }
/// });
/// assert_eq!(value["code"], 200);
/// assert_eq!(value["payload"]["features"][0], "sonic_rs");
/// ```
///
/// Trailing commas are allowed inside both arrays and objects.
///
/// ```
/// # use sonic_rs::json;
/// #
///
/// let value = json!(["notice", "the", "trailing", "comma -->",]);
/// ```
#[macro_export(local_inner_macros)]
macro_rules! json {
    //////////////////////////////////////////////////////////////////////////
    // The implementation of a static node. It will not create a shared allocator.
    //
    // Must be invoked as: json_internal!($($json)+)
    //////////////////////////////////////////////////////////////////////////
    (true) => {
        $crate::Value::new_bool(true, std::ptr::null())
    };

    (false) => {
        $crate::Value::new_bool(false, std::ptr::null())
    };

    (null) => {
        $crate::Value::new_null(std::ptr::null())
    };

    ([]) => {
        $crate::Array::new().into_value()
    };

    ({}) => {
        $crate::Object::new().into_value()
    };

    // Hide distracting implementation details from the generated rustdoc.
    ($($json:tt)+) => {
        {
            use $crate::JsonValueTrait;
            let shared = unsafe { &*$crate::value::shared::Shared::new_ptr() };
            let mut value = json_internal!(shared, $($json)+);
            if value.is_number() {
                unsafe {
                    drop(Box::from_raw(shared as *const _ as *mut $crate::value::shared::Shared));
                }
                value.mark_shared(std::ptr::null());
            } else {
                value.mark_root();
            }
            value
        }
    };
}

/// Construct a `sonic_rs::value::Array` from a JSON array literal.
///
/// ```
/// use sonic_rs::array;
/// use sonic_rs::json;
/// use sonic_rs::JsonValueTrait; // tait for `is_null()`
///
/// let local = "foo";
/// let array = array![null, local, true, false, 123,  "hello", 1 == 2, array![1, 2, 3], {"key": "value"}];
/// assert!(array[0].is_null());
/// assert_eq!(array[1].as_str(), Some("foo"));
/// assert_eq!(array[array.len() - 2][0].as_u64(), Some(1));
/// assert_eq!(array[array.len() - 1], json!({"key": "value"}));
/// ```
#[macro_export(local_inner_macros)]
macro_rules! array {
    () => {
        $crate::value::Array::new()
    };

    ($($tt:tt)+) => {
        {
            let shared = unsafe { &*$crate::value::shared::Shared::new_ptr() };
            let mut value = json_internal!(shared, [$($tt)+]);
            value.mark_root();
            value.into_array().expect("the literal is not a json array")
        }
    };
}

/// Construct a `sonic_rs::value::Object` from a JSON object literal.
///
/// ```
/// # use sonic_rs::object;
/// #
/// let code = 200;
/// let features = vec!["sonic_rs", "json"];
///
/// let object = object! {
///     "code": code,
///     "success": code == 200,
///     "payload": {
///         "features": features,
///         features[0]: features[1]
///     }
/// };
/// assert_eq!(object["code"], 200);
/// assert_eq!(object["payload"]["features"][0], "sonic_rs");
/// ```
#[macro_export(local_inner_macros)]
macro_rules! object {
    () => {
        $crate::value::Object::new()
    };

    ($($tt:tt)+) => {
        {
            let shared = unsafe { &*$crate::value::shared::Shared::new_ptr() };
            let mut value = json_internal!(shared, {$($tt)+});
            value.mark_root();
            value.into_object().expect("the literal is not a json object")
        }
    };
}

#[macro_export(local_inner_macros)]
#[doc(hidden)]
macro_rules! json_internal {
    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an array [...]. Produces a vec![...]
    // of the elements.
    //
    // Must be invoked as: json_internal!(@array [] $($tt)*)
    //////////////////////////////////////////////////////////////////////////

    // Done with trailing comma.
    (@array $shared:expr, [$($elems:expr,)*]) => {
        json_internal_array![$shared, $($elems)*]
    };

    // Done without trailing comma.
    (@array $shared:expr, [$($elems:expr),*]) => {
        json_internal_array![$shared, $($elems)*]
    };

    // Next element is `null`.
    (@array $shared:expr, [$($elems:expr,)*] null $($rest:tt)*) => {
        json_internal!(@array  $shared, [$($elems,)* json_internal!($shared, null)] $($rest)*)
    };

    // Next element is `true`.
    (@array $shared:expr, [$($elems:expr,)*] true $($rest:tt)*) => {
        json_internal!(@array $shared, [$($elems,)* json_internal!($shared, true)] $($rest)*)
    };

    // Next element is `false`.
    (@array $shared:expr, [$($elems:expr,)*] false $($rest:tt)*) => {
        json_internal!(@array $shared, [$($elems,)* json_internal!($shared, false)] $($rest)*)
    };

    // Next element is an array.
    (@array $shared:expr, [$($elems:expr,)*] [$($array:tt)*] $($rest:tt)*) => {
        json_internal!(@array $shared, [$($elems,)* json_internal!($shared, [$($array)*])] $($rest)*)
    };

    // Next element is a map.
    (@array $shared:expr, [$($elems:expr,)*] {$($map:tt)*} $($rest:tt)*) => {
        json_internal!(@array  $shared, [$($elems,)* json_internal!($shared, {$($map)*})] $($rest)*)
    };

    // Next element is an expression followed by comma.
    (@array $shared:expr, [$($elems:expr,)*] $next:expr, $($rest:tt)*) => {
        json_internal!(@array $shared, [$($elems,)* json_internal!($shared, $next),] $($rest)*)
    };

    // Last element is an expression with no trailing comma.
    (@array $shared:expr, [$($elems:expr,)*] $last:expr) => {
        json_internal!(@array $shared, [$($elems,)* json_internal!($shared, $last)])
    };

    // Comma after the most recent element.
    (@array $shared:expr, [$($elems:expr),*] , $($rest:tt)*) => {
        json_internal!(@array $shared, [$($elems,)*] $($rest)*)
    };

    // Unexpected token after most recent element.
    (@array $shared:expr, [$($elems:expr),*] $unexpected:tt $($rest:tt)*) => {
        json_unexpected!($unexpected)
    };

    //////////////////////////////////////////////////////////////////////////
    // TT muncher for parsing the inside of an object {...}. Each entry is
    // inserted into the given map variable.
    //
    // Must be invoked as: json_internal!(@object $map () ($($tt)*) ($($tt)*))
    //
    // We require two copies of the input tokens so that we can match on one
    // copy and trigger errors on the other copy.
    //////////////////////////////////////////////////////////////////////////

    // Done.
    (@object $shared:expr, $object:ident () () ()) => {};

    // Insert the current entry followed by trailing comma.
    (@object $shared:expr, $object:ident [$($key:tt)+] ($value:expr) , $($rest:tt)*) => {
        let key: &str = ($($key)+).as_ref();
        let pair = ($crate::Value::copy_str(key, $shared), $value);
        let _ = $object.append_pair(pair);
        json_internal!(@object $shared, $object () ($($rest)*) ($($rest)*));
    };

    // Current entry followed by unexpected token.
    (@object $shared:expr, $object:ident [$($key:tt)+] ($value:expr) $unexpected:tt $($rest:tt)*) => {
        json_unexpected!($unexpected);
    };

    // Insert the last entry without trailing comma.
    (@object $shared:expr, $object:ident [$($key:tt)+] ($value:expr)) => {
        let key: &str = ($($key)+).as_ref();
        let pair = ($crate::Value::copy_str(key, $shared), $value);
        let _ = $object.append_pair(pair);
    };

    // Next value is `null`.
    (@object $shared:expr, $object:ident ($($key:tt)+) (: null $($rest:tt)*) $copy:tt) => {
        json_internal!(@object $shared, $object [$($key)+] (json_internal!($shared, null)) $($rest)*);
    };

    // Next value is `true`.
    (@object $shared:expr, $object:ident ($($key:tt)+) (: true $($rest:tt)*) $copy:tt) => {
        json_internal!(@object $shared, $object [$($key)+] (json_internal!($shared, true)) $($rest)*);
    };

    // Next value is `false`.
    (@object $shared:expr, $object:ident ($($key:tt)+) (: false $($rest:tt)*) $copy:tt) => {
        json_internal!(@object $shared, $object [$($key)+] (json_internal!($shared, false)) $($rest)*);
    };

    // Next value is an array.
    (@object $shared:expr, $object:ident ($($key:tt)+) (: [$($array:tt)*] $($rest:tt)*) $copy:tt) => {
        json_internal!(@object $shared, $object [$($key)+] (json_internal!($shared, [$($array)*])) $($rest)*);
    };

    // Next value is a map.
    (@object $shared:expr, $object:ident ($($key:tt)+) (: {$($map:tt)*} $($rest:tt)*) $copy:tt) => {
        json_internal!(@object $shared, $object [$($key)+] (json_internal!($shared, {$($map)*})) $($rest)*);
    };

    // Next value is an expression followed by comma.
    (@object $shared:expr, $object:ident ($($key:tt)+) (: $value:expr , $($rest:tt)*) $copy:tt) => {
        json_internal!(@object $shared, $object [$($key)+] (json_internal!($shared, $value)) , $($rest)*);
    };

    // Last value is an expression with no trailing comma.
    (@object $shared:expr, $object:ident ($($key:tt)+) (: $value:expr) $copy:tt) => {
        json_internal!(@object $shared, $object [$($key)+] (json_internal!($shared, $value)));
    };

    // Missing value for last entry. Trigger a reasonable error message.
    (@object $shared:expr, $object:ident ($($key:tt)+) (:) $copy:tt) => {
        // "unexpected end of macro invocation"
        json_internal!();
    };

    // Missing colon and value for last entry. Trigger a reasonable error
    // message.
    (@object $shared:expr, $object:ident ($($key:tt)+) () $copy:tt) => {
        // "unexpected end of macro invocation"
        json_internal!();
    };

    // Misplaced colon. Trigger a reasonable error message.
    (@object $shared:expr, $object:ident () (: $($rest:tt)*) ($colon:tt $($copy:tt)*)) => {
        // Takes no arguments so "no rules expected the token `:`".
        json_unexpected!($colon);
    };

    // Found a comma inside a key. Trigger a reasonable error message.
    (@object $shared:expr, $object:ident ($($key:tt)*) (, $($rest:tt)*) ($comma:tt $($copy:tt)*)) => {
        // Takes no arguments so "no rules expected the token `,`".
        json_unexpected!($comma);
    };

    // Key is fully parenthesized. This avoids clippy double_parens false
    // positives because the parenthesization may be necessary here.
    (@object $shared:expr, $object:ident () (($key:expr) : $($rest:tt)*) $copy:tt) => {
        json_internal!(@object $shared, $object ($key) (: $($rest)*) (: $($rest)*));
    };

    // Refuse to absorb colon token into key expression.
    (@object $shared:expr, $object:ident ($($key:tt)*) (: $($unexpected:tt)+) $copy:tt) => {
        json_expect_expr_comma!($($unexpected)+);
    };

    // Munch a token into the current key.
    (@object $shared:expr, $object:ident ($($key:tt)*) ($tt:tt $($rest:tt)*) $copy:tt) => {
        json_internal!(@object $shared, $object ($($key)* $tt) ($($rest)*) ($($rest)*));
    };

    //////////////////////////////////////////////////////////////////////////
    // The main implementation.
    //
    // Must be invoked as: json_internal!($($json)+)
    //////////////////////////////////////////////////////////////////////////

    ($shared:expr, true) => {
        $crate::Value::new_bool(true, $shared)
    };

    ($shared:expr, false) => {
        $crate::Value::new_bool(false, $shared)
    };

    ($shared:expr, null) => {
        $crate::Value::new_null($shared)
    };

    ($shared:expr, []) => {
        $crate::Value::new_array($shared, 0)
    };

    ($shared:expr, [ $($tt:tt)+ ]) => {
        json_internal!(@array $shared, [] $($tt)+)
    };

    ($shared:expr, {}) => {
        $crate::Value::new_object($shared, 0)
    };

    ($shared:expr, { $($tt:tt)+ }) => {
        {
            let mut obj_value = $crate::Value::new_object($shared, 0);
            json_internal!(@object $shared, obj_value () ($($tt)+) ($($tt)+));
            obj_value
        }
    };

    // Any Serialize type: numbers, strings, struct literals, variables etc.
    // Must be below every other rule.
    ($shared:expr, $other:expr) => {
        $crate::value::to_value_in($shared.into(), &$other).unwrap()
    };
}

// The json_internal macro above cannot invoke vec directly because it uses
// local_inner_macros. A vec invocation there would resolve to $crate::vec.
// Instead invoke vec here outside of local_inner_macros.
#[macro_export(local_inner_macros)]
#[doc(hidden)]
macro_rules! json_internal_array {
    ($shared:expr, $($content:tt)*) => {
        {
            let mut arr_value = $crate::Value::new_array($shared, 0);
            $(
                arr_value.append_value($content);
            )*
            arr_value
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! json_unexpected {
    () => {};
}

#[macro_export]
#[doc(hidden)]
macro_rules! json_expect_expr_comma {
    ($e:expr , $($tt:tt)*) => {};
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::value::value_trait::JsonValueTrait;

    #[test]
    fn test_json_memory() {
        assert!(json!(true).is_static());
        assert!(json!(false).is_static());
        assert!(json!(null).is_static());
        assert!(json!([]).is_static());
        assert!(json!({}).is_static());
        assert!(json!(123).is_static());
        assert!(json!(1.23).is_static());
        assert!(json!("123").is_root());
        assert!(json!("").is_root());
        assert!(json!({"1": 123}).is_root());
        assert!(json!([[[]]]).is_root());
    }

    #[test]
    fn test_json_macro() {
        assert!(json!(true).is_true());
        assert!(json!(false).is_false());
        assert!(json!(null).is_null());
        assert!(json!("123").is_str());
        assert!(json!(vec![1]).is_array());
        assert_eq!(json!(vec![1, 2, 3][2]).as_i64(), Some(3));

        let buf = json!([1, 2, 3]);
        let arr = json!([true, false, null, 1, 2, 3, "hi", 1 == 2, buf[1] == buf[2]]);
        assert!(arr.is_array());
        assert!(arr[arr.len() - 1].is_false());

        let key = "i";
        let key2 = "\"i\"";
        let obj = json!({
            "a": true,
            "b": false,
            "c": null,
            "array": vec![1, 2, 3],
            "map": ({
                let mut map = HashMap::<String, String>::new();
                map.insert("a".to_string(), "b".to_string());
                map
            }),
            "f": 2.333,
            "g": "hi",
            "h": 1 == 2,
            key: {
                key2: [buf[1] == buf[2], 1],
            },
        });
        assert!(obj.is_object());
        assert!(obj["a"].is_true());
        assert!(obj["array"][0].as_u64().unwrap() == 1);
        assert!(obj["map"]["a"].as_str().unwrap() == "b");
        assert!(obj[key][key2][1].as_u64().unwrap() == 1);

        let obj = json!({
            "a": { "b" : {"c": [[[]], {}, {}]} }
        });
        assert!(obj["a"]["b"]["c"][0][0].is_array());
    }

    #[test]
    fn test_array_macro() {
        let arr = array![];
        assert!(arr.into_value().is_static());

        let arr = array![true, false, null, 1, 2, 3, "hi", 1 == 2];
        assert!(arr[arr.len() - 1].is_false());

        let buf = array![1, 2, 3];
        let arr = array![true, false, null, 1, 2, 3, "hi", 1 == 2, buf[1] == buf[2]];
        assert!(arr[arr.len() - 1].is_false());
    }

    #[test]
    fn test_object_macro() {
        let obj = object! {};
        assert!(obj.into_value().is_static());

        let obj = object! {
            "a": true,
            "b": false,
            "c": null,
            "d": 1,
            "e": 2,
            "f": 3,
            "g": "hi",
            "h": 1 == 2,
        };
        assert!(obj["a"].is_true());

        let buf = array![1, 2, 3];
        let obj = object! {
            "a": true,
            "b": false,
            "c": null,
            "d": 1,
            "e": 2,
            "f": 3,
            "g": "hi",
            "h": 1 == 2,
            "i": {
                "i": [buf[1] == buf[2], 1],
            },
        };
        assert!(obj["i"]["i"][1].as_u64().unwrap() == 1);
    }
}
