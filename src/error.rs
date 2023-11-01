//! When serializing or deserializing JSON goes wrong.

// The code is cloned from [serde_json](https://github.com/serde-rs/json) and modified necessary parts.

use core::fmt::{self, Debug, Display};
use core::result;
use serde::{de, ser};
use std::error;
use std::str::FromStr;
use std::string::{String, ToString};

use thiserror::Error as ErrorTrait;

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
    ///
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
            ErrorCode::Message(_)
            | ErrorCode::GetInEmptyObject
            | ErrorCode::GetInEmptyArray
            | ErrorCode::GetIndexOutOfArray
            | ErrorCode::GetUnknownKeyInObject
            | ErrorCode::UnexpectedVisitType => Category::Data,
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

    /// Returns true if this error was caused by input data that was
    /// semantically incorrect.
    ///
    /// For example, JSON containing a number is semantically incorrect when the
    /// type being deserialized into holds a String.
    pub fn is_data(&self) -> bool {
        self.classify() == Category::Data
    }

    /// Returns true if this error was caused by prematurely reaching the end of
    /// the input data.
    ///
    /// Callers that process streaming input may be interested in retrying the
    /// deserialization once more data is available.
    pub fn is_eof(&self) -> bool {
        self.classify() == Category::Eof
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<Error> for std::io::Error {
    /// Convert a `sonic_rs::Error` into an `std::io::Error`.
    ///
    /// JSON syntax and data errors are turned into `InvalidData` I/O errors.
    /// EOF errors are turned into `UnexpectedEof` I/O errors.
    ///
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
    /// TODO: support stream reader in the future
    Io,

    /// The error was caused by input that was not syntactically valid JSON.
    Syntax,

    /// The error was caused by input data that was semantically incorrect.
    ///
    /// For example:
    /// 1. JSON containing a number is semantically incorrect when the
    /// type being deserialized into holds a String.
    /// 2. When using `get*` APIs, it gets a unknown keys from JSON text, or get
    /// a index from empty array.
    Data,

    /// The error was caused by prematurely reaching the end of the input data.
    ///
    /// Callers that process streaming input may be interested in retrying the
    /// deserialization once more data is available.
    Eof,
}

struct ErrorImpl {
    code: ErrorCode,
    line: usize,
    column: usize,
}

#[derive(ErrorTrait, Debug)]
pub(crate) enum ErrorCode {
    #[error("{0}")]
    Message(Box<str>),

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

    #[error("Invalid hex escape code")]
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

    #[error("Unexpected visited type in JSON visitor")]
    UnexpectedVisitType,
}

impl Error {
    #[cold]
    pub(crate) fn syntax(code: ErrorCode, line: usize, column: usize) -> Self {
        Error {
            err: Box::new(ErrorImpl { code, line, column }),
        }
    }

    #[cold]
    pub(crate) fn io(error: std::io::Error) -> Self {
        Error {
            err: Box::new(ErrorImpl {
                code: ErrorCode::Io(error),
                line: 0,
                column: 0,
            }),
        }
    }

    #[cold]
    pub(crate) fn fix_position<F>(self, f: F) -> Self
    where
        F: FnOnce(ErrorCode) -> Error,
    {
        if self.err.line == 0 {
            f(self.err.code)
        } else {
            self
        }
    }

    #[cold]
    pub(crate) fn new(code: ErrorCode, line: usize, column: usize) -> Self {
        Error {
            err: Box::new(ErrorImpl { code, line, column }),
        }
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&*self.err, f)
    }
}

impl Display for ErrorImpl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} at line {} column {}",
            self.code, self.line, self.column
        )
    }
}

// Remove two layers of verbosity from the debug representation. Humans often
// end up seeing this representation because it is what unwrap() shows.
impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Error({}, line: {}, column: {})",
            self.err.code, self.err.line, self.err.column
        )
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
            Error::custom(format_args!("invalid type: null, expected {}", exp))
        } else {
            Error::custom(format_args!("invalid type: {}, expected {}", unexp, exp))
        }
    }
}

impl ser::Error for Error {
    #[cold]
    fn custom<T: Display>(msg: T) -> Error {
        make_error(msg.to_string())
    }
}

// Parse our own error message that looks like "{} at line {} column {}" to work
// around erased-serde round-tripping the error through de::Error::custom.
#[cold]
pub fn make_error(mut msg: String) -> Error {
    let (line, column) = parse_line_col(&mut msg).unwrap_or((0, 0));
    Error {
        err: Box::new(ErrorImpl {
            code: ErrorCode::Message(msg.into_boxed_str()),
            line,
            column,
        }),
    }
}

fn parse_line_col(msg: &mut String) -> Option<(usize, usize)> {
    let start_of_suffix = match msg.rfind(" at line ") {
        Some(index) => index,
        None => return None,
    };

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

    use crate::{from_str, Deserialize};

    #[test]
    fn test_errors_display() {
        #[derive(Debug, Deserialize)]
        struct Foo {
            a: Vec<i32>,
        }
        // test error from `serde` trait
        let err = from_str::<Foo>("{ \"b\":[]}").unwrap_err();
        assert_eq!(format!("{}", err), "missing field `a` at line 1 column 9");
    }
}
