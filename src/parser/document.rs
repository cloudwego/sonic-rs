use super::Parser;
use crate::{
    config::DeserializeCfg, error::make_error, reader::Reader, JsonInput, OwnedLazyValue, Read,
    Result, Value,
};

/// Parse one complete JSON document directly into an owned DOM.
///
/// ```
/// let value = sonic_rs::parse_value(r#"{"items":[1,2]}"#)?;
/// assert_eq!(value["items"][1], 2);
/// # Ok::<(), sonic_rs::Error>(())
/// ```
pub fn parse_value<'de>(input: impl JsonInput<'de>) -> Result<Value> {
    let read = Read::new(input.to_u8_slice(), input.need_utf8_valid());
    let mut parser = Parser::for_document(read)?;
    let value = parser.read_root_value()?;
    parser.finish_document()?;
    Ok(value)
}

/// Parse one complete JSON document into an owned lazy value, retaining raw JSON
/// for containers until their contents are accessed.
///
/// ```
/// let value = sonic_rs::parse_owned_lazy_value("[1,  2]")?;
/// assert_eq!(sonic_rs::to_string(&value)?, "[1,  2]");
/// # Ok::<(), sonic_rs::Error>(())
/// ```
pub fn parse_owned_lazy_value<'de>(input: impl JsonInput<'de>) -> Result<OwnedLazyValue> {
    // Byte inputs need validation before they can back a FastStr. String inputs
    // can retain their original shared backing when the reader takes a subslice.
    let read = if input.need_utf8_valid() {
        Read::new(input.to_u8_slice(), true)
    } else {
        Read::from(input)
    };
    let mut parser = Parser::for_document(read)?;
    let value = parser.get_owned_lazyvalue(true)?;
    parser.finish_document()?;
    Ok(value)
}

impl<'de, R: Reader<'de>> Parser<R> {
    pub(crate) fn for_document(read: R) -> Result<Self> {
        let len = read.as_u8_slice().len();
        if len > u32::MAX as usize {
            return Err(make_error(format!(
                "Only support JSON less than 4 GB, the input JSON is too large here, len is {len}"
            )));
        }
        Ok(Self::new(read).with_config(DeserializeCfg {
            use_rawnumber: cfg!(feature = "arbitrary_precision"),
            utf8_lossy: cfg!(feature = "utf8_lossy"),
        }))
    }

    pub(crate) fn finish_document(&mut self) -> Result<()> {
        self.parse_trailing()?;
        self.read.check_utf8_final()
    }

    pub(crate) fn read_root_value(&mut self) -> Result<Value> {
        debug_assert_eq!(self.read.index(), 0);
        let mut val = Value::new();
        let cfg = self.cfg;
        let json = self.read.as_u8_slice();

        if cfg.utf8_lossy && self.read.next_invalid_utf8() != usize::MAX {
            // repr the invalid utf8, not need to care about the invalid UTF8 char in non-string
            // parts, it will cause errors when parsing.
            let lossy = String::from_utf8_lossy(json);
            let n = val.parse_with_padding(lossy.as_bytes(), cfg)?;
            // `n` is an offset into the lossy copy, where every invalid sequence became the
            // 3-byte U+FFFD, so it is not an offset into `json`. What follows the value is
            // the same in both (whitespace or trailing characters; invalid UTF-8 there is a
            // trailing-character error either way), so count it from the end.
            // `n` past the end means the parser ran into the padding (unterminated input):
            // keep it past the end of `json` by the same amount, so that is still reported.
            let consumed = if n >= lossy.len() {
                json.len() + (n - lossy.len())
            } else {
                json.len() - (lossy.len() - n)
            };
            self.read.eat(consumed);
            // the invalid UTF-8 inside the value has been replaced: look for it only in the
            // part that is still to be read (if any; past the end is reported as EOF)
            if consumed <= json.len() {
                self.read.check_invalid_utf8();
            }
        } else {
            let n = val.parse_with_padding(json, cfg)?;
            self.read.eat(n);
        }
        Ok(val)
    }
}
