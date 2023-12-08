use crate::LazyValue;
use serde::ser::SerializeStruct;

impl<'a> serde::ser::Serialize for LazyValue<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let raw = self.as_raw_str();
        let mut s = serializer.serialize_struct(super::TOKEN, 1)?;
        // will directly write raw in `RawValueStrEmitter::seriazlie_str`
        s.serialize_field(super::TOKEN, raw)?;
        s.end()
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_lazyvalue_serde() {
        let json = r#"{
            "a": 1,
            "b": "2",
            "c": [3, 4, 5],
            "d": {
                "e": 6,
                "f": "7",
                "g": [8, 9, 10]
            }
        }"#;
        let value = crate::from_str::<crate::LazyValue>(json).unwrap();
        let json2 = crate::to_string(&value).unwrap();
        assert_eq!(json, json2);
    }
}
