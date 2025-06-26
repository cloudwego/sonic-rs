//! Errors.

// The code is cloned from [serde_json](https://github.com/serde-rs/json) and modified necessary parts.

use core::{
    fmt::{self, Debug, Display},
    result,
};
use std::{borrow::Cow, error, fmt::Result as FmtResult, str::FromStr};

use serde::{
    de::{self, Unexpected},
    ser,
};
use sonic_number::Error as NumberError;
use thiserror::Error as ErrorTrait;

use crate::reader::Position;

/// This type represents all possible errors that can occur when serializing or
/// deserializing JSON data.
pub struct Error {
    /// This `Box` allows us to keep the size of `Error` as small as possible. A
    /// larger `Error` type was substantially slower due to all the functions
    /// that pass around `Result<T, Error>`.
    err: Box<ErrorImpl>,
}

/// Alias for a `Result` with the error type `sonic_rs::Error`.
pub type Result<T> = result::Result<T, Error>;

impl Error {
    /// One-based line number at which the error was detected.
    ///
    /// Characters in the first line of the input (before the first newline
    /// character) are in line 1.
    pub fn line(&self) -> usize {
        self.err.line
    }

    /// One-based column number at which the error was detected.
    ///
    /// The first character in the input and any characters immediately
    /// following a newline character are in column 1.
    ///
    /// Note that errors may occur in column 0, for example if a read from an
    /// I/O stream fails immediately following a previously read newline
    /// character.
    pub fn column(&self) -> usize {
        self.err.column
    }

    /// The kind reported by the underlying standard library I/O error, if this
    /// error was caused by a failure to read or write bytes on an I/O stream.
    pub fn io_error_kind(&self) -> Option<std::io::ErrorKind> {
        if let ErrorCode::Io(io_error) = &self.err.code {
            Some(io_error.kind())
        } else {
            None
        }
    }

    /// Categorizes the cause of this error.
    ///
    /// - `Category::Io` - failure to read or write bytes on an I/O stream
    /// - `Category::Syntax` - input that is not syntactically valid JSON
    /// - `Category::Data` - input data that is semantically incorrect
    /// - `Category::Eof` - unexpected end of the input data
    pub fn classify(&self) -> Category {
        match self.err.code {
            ErrorCode::Message(_) | ErrorCode::UnexpectedVisitType => Category::TypeUnmatched,
            ErrorCode::GetInEmptyObject
            | ErrorCode::GetInEmptyArray
            | ErrorCode::GetIndexOutOfArray
            | ErrorCode::GetUnknownKeyInObject => Category::NotFound,
            ErrorCode::Io(_) => Category::Io,
            ErrorCode::EofWhileParsing => Category::Eof,
            ErrorCode::ExpectedColon
            | ErrorCode::ExpectedObjectCommaOrEnd
            | ErrorCode::InvalidEscape
            | ErrorCode::InvalidJsonValue
            | ErrorCode::InvalidLiteral
            | ErrorCode::InvalidUTF8
            | ErrorCode::InvalidNumber
            | ErrorCode::NumberOutOfRange
            | ErrorCode::InvalidUnicodeCodePoint
            | ErrorCode::ControlCharacterWhileParsingString
            | ErrorCode::TrailingComma
            | ErrorCode::TrailingCharacters
            | ErrorCode::ExpectObjectKeyOrEnd
            | ErrorCode::ExpectedArrayCommaOrEnd
            | ErrorCode::ExpectedArrayStart
            | ErrorCode::ExpectedObjectStart
            | ErrorCode::InvalidSurrogateUnicodeCodePoint
            | ErrorCode::SerExpectKeyIsStrOrNum(_)
            | ErrorCode::FloatMustBeFinite
            | ErrorCode::ExpectedQuote
            | ErrorCode::ExpectedNumericKey
            | ErrorCode::RecursionLimitExceeded => Category::Syntax,
        }
    }

    /// Returns true if this error was caused by a failure to read or write
    /// bytes on an I/O stream.
    pub fn is_io(&self) -> bool {
        self.classify() == Category::Io
    }

    /// Returns true if this error was caused by input that was not
    /// syntactically valid JSON.
    pub fn is_syntax(&self) -> bool {
        self.classify() == Category::Syntax
    }

    /// Returns true when the input data is unmatched for expected type.
    ///
    /// For example, JSON containing a number  when the type being deserialized into holds a String.
    pub fn is_unmatched_type(&self) -> bool {
        self.classify() == Category::TypeUnmatched
    }

    /// Return true when the target field was not found from JSON.
    ///
    /// For example:
    /// When using `get*` APIs, it gets a unknown keys from JSON text, or get
    /// a index out of the array.
    pub fn is_not_found(&self) -> bool {
        self.classify() == Category::NotFound
    }

    /// Returns true if this error was caused by prematurely reaching the end of
    /// the input data.
    ///
    /// Callers that process streaming input may be interested in retrying the
    /// deserialization once more data is available.
    pub fn is_eof(&self) -> bool {
        self.classify() == Category::Eof
    }

    /// Returens the offset of the error position from the starting of JSON text.
    pub fn offset(&self) -> usize {
        self.err.index
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<Error> for std::io::Error {
    /// Convert a `sonic_rs::Error` into an `std::io::Error`.
    ///
    /// JSON syntax and data errors are turned into `InvalidData` I/O errors.
    /// EOF errors are turned into `UnexpectedEof` I/O errors.
    fn from(j: Error) -> Self {
        match j.err.code {
            ErrorCode::Io(err) => err,
            ErrorCode::EofWhileParsing => std::io::Error::new(std::io::ErrorKind::UnexpectedEof, j),
            _ => std::io::Error::new(std::io::ErrorKind::InvalidData, j),
        }
    }
}

/// Categorizes the cause of a `sonic_rs::Error`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Category {
    /// The error was caused by a failure to read or write bytes on an I/O
    /// stream.
    Io,

    /// The error was caused by input that was not syntactically valid JSON.
    Syntax,

    /// The error was caused when the input data is unmatched for expected type.
    ///
    /// For example, JSON containing a number  when the type being deserialized into holds a
    /// String.
    TypeUnmatched,

    /// The error was caused when the target field was not found from JSON.
    ///
    /// For example:
    /// When using `get*` APIs, it gets a unknown keys from JSON text, or get
    /// a index out of the array.
    NotFound,

    /// The error was caused by prematurely reaching the end of the input data.
    ///
    /// Callers that process streaming input may be interested in retrying the
    /// deserialization once more data is available.
    Eof,
}

struct ErrorImpl {
    code: ErrorCode,
    index: usize,
    line: usize,
    column: usize,
    // the descript of the error position
    descript: Option<String>,
}

#[derive(ErrorTrait, Debug)]
#[non_exhaustive]
pub enum ErrorCode {
    #[error("{0}")]
    Message(Cow<'static, str>),

    #[error("io error while serializing or deserializing")]
    Io(std::io::Error),

    #[error("EOF while parsing")]
    EofWhileParsing,

    #[error("Expected this character to be a ':' while parsing")]
    ExpectedColon,

    #[error("Expected this character to be either a ',' or a ']' while parsing")]
    ExpectedArrayCommaOrEnd,

    #[error("Expected this character to be either a ',' or a '}}' while parsing")]
    ExpectedObjectCommaOrEnd,

    #[error("Invalid literal (`true`, `false`, or a `null`) while parsing")]
    InvalidLiteral,

    #[error("Invalid JSON value")]
    InvalidJsonValue,

    #[error("Expected this character to be '{{'")]
    ExpectedObjectStart,

    #[error("Expected this character to be '['")]
    ExpectedArrayStart,

    #[error("Invalid escape chars")]
    InvalidEscape,

    #[error("Invalid number")]
    InvalidNumber,

    #[error("Number is bigger than the maximum value of its type")]
    NumberOutOfRange,

    #[error("Invalid unicode code point")]
    InvalidUnicodeCodePoint,

    #[error("Invalid UTF-8 characters in json")]
    InvalidUTF8,

    #[error("Control character found while parsing a string")]
    ControlCharacterWhileParsingString,

    #[error("Expected this character to be '\"' or '}}'")]
    ExpectObjectKeyOrEnd,

    #[error("JSON has a comma after the last value in an array or object")]
    TrailingComma,

    #[error("JSON has non-whitespace trailing characters after the value")]
    TrailingCharacters,

    #[error("Encountered nesting of JSON maps and arrays more than 128 layers deep")]
    RecursionLimitExceeded,

    #[error("Get value from an empty object")]
    GetInEmptyObject,

    #[error("Get unknown key from the object")]
    GetUnknownKeyInObject,

    #[error("Get value from an empty array")]
    GetInEmptyArray,

    #[error("Get index out of the array")]
    GetIndexOutOfArray,

    #[error("Unexpected visited type")]
    UnexpectedVisitType,

    #[error("Invalid surrogate Unicode code point")]
    InvalidSurrogateUnicodeCodePoint,

    #[error("Float number must be finite, not be Infinity or NaN")]
    FloatMustBeFinite,

    #[error("Expect a numeric key in Value")]
    ExpectedNumericKey,

    #[error("Expect a quote")]
    ExpectedQuote,

    #[error("Expected the key to be string/bool/number when serializing map, now is {0}")]
    SerExpectKeyIsStrOrNum(Unexpected<'static>),
}

impl From<NumberError> for ErrorCode {
    fn from(err: NumberError) -> Self {
        match err {
            NumberError::InvalidNumber => ErrorCode::InvalidNumber,
            NumberError::FloatMustBeFinite => ErrorCode::FloatMustBeFinite,
        }
    }
}

impl Error {
    #[cold]
    pub(crate) fn syntax(code: ErrorCode, json: &[u8], index: usize) -> Self {
        let position = Position::from_index(index, json);
        // generate descript about 16 characters
        let mut start = index.saturating_sub(8);
        let mut end = if index + 8 > json.len() {
            json.len()
        } else {
            index + 8
        };

        // find the nearest valid utf-8 character
        while start > 0 && index - start <= 16 && (json[start] & 0b1100_0000) == 0b1000_0000 {
            start -= 1;
        }

        // find the nearest valid utf-8 character
        while end < json.len() && end - index <= 16 && (json[end - 1] & 0b1100_0000) == 0b1000_0000
        {
            end += 1;
        }

        let fragment = String::from_utf8_lossy(&json[start..end]).to_string();
        let left = index - start;
        let right = if end - index > 1 {
            end - (index + 1)
        } else {
            0
        };
        let mask = ".".repeat(left) + "^" + &".".repeat(right);
        let descript = format!("\n\n\t{fragment}\n\t{mask}\n");

        Error {
            err: Box::new(ErrorImpl {
                code,
                line: position.line,
                column: position.column,
                index,
                descript: Some(descript),
            }),
        }
    }

    #[cold]
    pub(crate) fn ser_error(code: ErrorCode) -> Self {
        Error {
            err: Box::new(ErrorImpl {
                code,
                line: 0,
                column: 0,
                index: 0,
                descript: None,
            }),
        }
    }

    #[cold]
    pub(crate) fn io(error: std::io::Error) -> Self {
        Error {
            err: Box::new(ErrorImpl {
                code: ErrorCode::Io(error),
                line: 0,
                index: 0,
                column: 0,
                descript: None,
            }),
        }
    }

    #[cold]
    pub(crate) fn error_code(self) -> ErrorCode {
        self.err.code
    }
}

impl serde::de::StdError for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.err.code {
            ErrorCode::Io(err) => err.source(),
            _ => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> FmtResult {
        Display::fmt(&*self.err, f)
    }
}

impl Display for ErrorImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> FmtResult {
        if self.line != 0 {
            write!(
                f,
                "{} at line {} column {}{}",
                self.code,
                self.line,
                self.column,
                self.descript.as_ref().unwrap_or(&"".to_string())
            )
        } else {
            write!(f, "{}", self.code)
        }
    }
}

// Remove two layers of verbosity from the debug representation. Humans often
// end up seeing this representation because it is what unwrap() shows.
impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> FmtResult {
        Display::fmt(&self, f)
    }
}

impl de::Error for Error {
    #[cold]
    fn custom<T: Display>(msg: T) -> Error {
        make_error(msg.to_string())
    }

    #[cold]
    fn invalid_type(unexp: de::Unexpected, exp: &dyn de::Expected) -> Self {
        if let de::Unexpected::Unit = unexp {
            Error::custom(format_args!("invalid type: null, expected {exp}"))
        } else {
            Error::custom(format_args!("invalid type: {unexp}, expected {exp}"))
        }
    }
}

impl ser::Error for Error {
    #[cold]
    fn custom<T: Display>(msg: T) -> Error {
        make_error(msg.to_string())
    }
}

// TODO: remove me in 0.4 version.
#[cold]
pub(crate) fn make_error(mut msg: String) -> Error {
    let (line, column) = parse_line_col(&mut msg).unwrap_or((0, 0));
    Error {
        err: Box::new(ErrorImpl {
            code: ErrorCode::Message(msg.into()),
            line,
            index: 0,
            column,
            descript: None,
        }),
    }
}

fn parse_line_col(msg: &mut String) -> Option<(usize, usize)> {
    let start_of_suffix = msg.rfind(" at line ")?;

    // Find start and end of line number.
    let start_of_line = start_of_suffix + " at line ".len();
    let mut end_of_line = start_of_line;
    while starts_with_digit(&msg[end_of_line..]) {
        end_of_line += 1;
    }

    if !msg[end_of_line..].starts_with(" column ") {
        return None;
    }

    // Find start and end of column number.
    let start_of_column = end_of_line + " column ".len();
    let mut end_of_column = start_of_column;
    while starts_with_digit(&msg[end_of_column..]) {
        end_of_column += 1;
    }

    if end_of_column < msg.len() {
        return None;
    }

    // Parse numbers.
    let line = match usize::from_str(&msg[start_of_line..end_of_line]) {
        Ok(line) => line,
        Err(_) => return None,
    };
    let column = match usize::from_str(&msg[start_of_column..end_of_column]) {
        Ok(column) => column,
        Err(_) => return None,
    };

    msg.truncate(start_of_suffix);
    Some((line, column))
}

fn starts_with_digit(slice: &str) -> bool {
    match slice.as_bytes().first() {
        None => false,
        Some(&byte) => byte.is_ascii_digit(),
    }
}

#[cfg(test)]
mod test {
    use crate::{from_slice, from_str, Deserialize};

    #[test]
    fn test_serde_errors_display() {
        #[allow(unused)]
        #[derive(Debug, Deserialize)]
        struct Foo {
            a: Vec<i32>,
            c: String,
        }

        let err = from_str::<Foo>("{ \"b\":[]}").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "missing field `a` at line 1 column 9\n\n\t{ \"b\":[]}\n\t........^\n"
        );

        let err = from_str::<Foo>("{\"a\": [1, 2x, 3, 4, 5]}").unwrap_err();
        println!("{err}");
        assert_eq!(
            format!("{err}"),
            "Expected this character to be either a ',' or a ']' while parsing at line 1 column \
             12\n\n\t\": [1, 2x, 3, 4,\n\t........^.......\n"
        );

        let err = from_str::<Foo>("{\"a\": null}").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "invalid type: null, expected a sequence at line 1 column 10\n\n\t\"a\": \
             null}\n\t........^.\n"
        );

        let err = from_str::<Foo>("{\"a\": [1,2,3  }").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "Expected this character to be either a ',' or a ']' while parsing at line 1 column \
             15\n\n\t[1,2,3  }\n\t........^\n"
        );

        let err = from_str::<Foo>("{\"a\": [\"123\"]}").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "invalid type: string \"123\", expected i32 at line 1 column 12\n\n\t\": \
             [\"123\"]}\n\t........^..\n"
        );

        let err = from_str::<Foo>("{\"a\": [").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "EOF while parsing at line 1 column 7\n\n\t{\"a\": [\n\t......^\n"
        );

        let err = from_str::<Foo>("{\"a\": [000]}").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "Expected this character to be either a ',' or a ']' while parsing at line 1 column \
             9\n\n\t{\"a\": [000]}\n\t........^...\n"
        );

        let err = from_str::<Foo>("{\"a\": [-]}").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "Invalid number at line 1 column 8\n\n\t{\"a\": [-]}\n\t.......^..\n"
        );

        let err = from_str::<Foo>("{\"a\": [-1.23e]}").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "Invalid number at line 1 column 13\n\n\t: [-1.23e]}\n\t........^..\n"
        );

        let err = from_str::<Foo>("{\"c\": \"哈哈哈哈哈哈}").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "EOF while parsing at line 1 column 26\n\n\t哈哈哈}\n\t.........^\n"
        );

        let err = from_slice::<Foo>(b"{\"b\":\"\x80\"}").unwrap_err();
        assert_eq!(
            format!("{err}"),
            "Invalid UTF-8 characters in json at line 1 column 7\n\n\t{\"b\":\"�\"}\n\t......^..\n"
        );
    }

    #[test]
    fn test_other_errors() {
        let err = crate::Value::try_from(f64::NAN).unwrap_err();
        assert_eq!(
            format!("{err}"),
            "NaN or Infinity is not a valid JSON value"
        );
    }

    #[test]
    fn test_error_column() {
        let json_str = r#"
{
    "key": [, 1, 2, 3]
}
"#;
        let err = from_str::<crate::Value>(json_str).unwrap_err();
        assert_eq!(err.column(), 13);
    }
}
