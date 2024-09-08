use crate::{
    parser::Parser, reader::Reader, util::utf8::from_utf8, value::Value, JsonInput, Read, Result,
};

pub fn get_by_schema<'de, Input: JsonInput<'de>>(json: Input, mut schema: Value) -> Result<Value> {
    let slice = json.to_u8_slice();
    let reader = Read::new(slice, false);
    let mut parser = Parser::new(reader);
    parser.get_by_schema(&mut schema)?;

    // validate the utf-8 if slice
    let index = parser.read.index();
    if json.need_utf8_valid() {
        from_utf8(&slice[..index])?;
    }
    Ok(schema)
}
