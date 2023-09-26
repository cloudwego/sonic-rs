//! When serializing or deserializing JSON goes wrong.

// The code is cloned from [serde_json](https://github.com/serde-rs/json) and modified necessary parts.

use core::fmt::{self, Debug, Display};
use core::result;
use serde::{de, ser};
use std::error;
use std::str::FromStr;
use std::string::{String, ToString};

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

    /// Categorizes the cause of this error.
    ///
    /// - `Category::Io` - failure to read or write bytes on an I/O stream
    /// - `Category::Syntax` - input that is not syntactically valid JSON
    /// - `Category::Data` - input data that is semantically incorrect
    /// - `Category::Eof` - unexpected end of the input data
    pub fn classify(&self) -> Category {
        match &self.err.code {
            ErrorCode::Message(_) => Category::Data,
            ErrorCode::Io(_) => Category::Io,
            code => code.classify(),
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
}

/// Categorizes the cause of a `sonic_rs::Error`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Category {
    /// The error was caused by a failure to read or write bytes on an I/O
    /// stream.
    Io,

    /// The error was caused by input that was not syntactically valid JSON.
    Syntax,

    /// The error was caused by input data that was semantically incorrect.
    ///
    /// For example, JSON containing a number is semantically incorrect when the
    /// type being deserialized into holds a String.
    Data,

    /// The error was caused by prematurely reaching the end of the input data.
    ///
    /// Callers that process streaming input may be interested in retrying the
    /// deserialization once more data is available.
    Eof,
}

#[allow(clippy::fallible_impl_from)]
impl From<Error> for std::io::Error {
    /// Convert a `sonic_rs::Error` into an `std::io::Error`.
    ///
    /// JSON syntax and data errors are turned into `InvalidData` I/O errors.
    /// EOF errors are turned into `UnexpectedEof` I/O errors.
    ///
    fn from(j: Error) -> Self {
        if let ErrorCode::Io(err) = j.err.code {
            err
        } else {
            match j.classify() {
                Category::Io => unreachable!(),
                Category::Syntax | Category::Data => {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, j)
                }
                Category::Eof => std::io::Error::new(std::io::ErrorKind::UnexpectedEof, j),
            }
        }
    }
}

struct ErrorImpl {
    code: ErrorCode,
    line: usize,
    column: usize,
}

impl ErrorCode {
    /// Categorizes the cause of this error.
    ///
    /// - `Category::Io` - failure to read or write bytes on an I/O stream
    /// - `Category::Syntax` - input that is not syntactically valid JSON
    /// - `Category::Data` - input data that is semantically incorrect
    /// - `Category::Eof` - unexpected end of the input data
    pub fn classify(&self) -> Category {
        match self {
            ErrorCode::EofWhileParsingArray
            | ErrorCode::EofWhileParsingObject
            | ErrorCode::EofWhileParsingString
            | ErrorCode::EofWhileParsingNumber
            | ErrorCode::EofWhileParsingLiteral
            | ErrorCode::EofAfterSkipSpace => Category::Eof,

            ErrorCode::ExpectedColon
            | ErrorCode::ExpectedArrayCommaOrEnd
            | ErrorCode::ExpectedObjectCommaOrEnd
            | ErrorCode::ExpectedSomeLiteral
            | ErrorCode::ExpectedSomeValue
            | ErrorCode::InvalidEscape
            | ErrorCode::InvalidNumber
            | ErrorCode::NumberOutOfRange
            | ErrorCode::InvalidUnicodeCodePoint
            | ErrorCode::ControlCharacterWhileParsingString
            | ErrorCode::KeyMustBeAString
            | ErrorCode::LoneLeadingSurrogateInHexEscape
            | ErrorCode::TrailingComma
            | ErrorCode::MimatchedNumberFormat
            | ErrorCode::NumberWithLeadingZero
            | ErrorCode::TrailingCharacters
            | ErrorCode::UnexpectedEndOfHexEscape
            | ErrorCode::RecursionLimitExceeded => Category::Syntax,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ErrorCode {
    /// Catchall for syntax error messages
    Message(Box<str>),

    /// Some I/O error occurred while serializing or deserializing.
    Io(std::io::Error),

    /// No errors
    ErrorNone,

    /// Not error,
    HasEsacped,

    /// Number is too long
    NumberTooLong,

    /// EOF while parsing a array.
    EofWhileParsingArray,

    /// EOF while parsing an object.
    EofWhileParsingObject,

    /// EOF while parsing a string.
    EofWhileParsingString,

    /// EOF while parsing a JSON number.
    EofWhileParsingNumber,

    /// EOF while parsing a JSON literal, either a `true`, `false`, or a `null`.
    EofWhileParsingLiteral,

    /// EOF after skip space.
    EofAfterSkipSpace,

    /// Expected this character to be a `':'`.
    ExpectedColon,

    /// Expected this character to be either a `','` or a `']'`.
    ExpectedArrayCommaOrEnd,

    /// Expected this character to be either a `','` or a `'}'`.
    ExpectedObjectCommaOrEnd,

    /// Expected to parse either a `true`, `false`, or a `null`.
    ExpectedSomeLiteral,

    /// Expected this character to start a JSON value.
    ExpectedSomeValue,

    /// Expected this character to start a JSON object.
    ExpectedObject,

    /// Expected this character to start a JSON array.
    ExpectedArray,

    /// Invalid hex escape code.
    InvalidEscape,

    /// Invalid number. such as "-", "0."
    InvalidNumber,

    /// Number with leading zero is not allowed, such as "0123"
    NumberWithLeadingZero,

    /// Mismatched number format, such as parse "0.123" to a integer
    MimatchedNumberFormat,

    /// Number is bigger than the maximum value of its type.
    NumberOutOfRange,

    /// Invalid unicode code point.
    InvalidUnicodeCodePoint,

    /// Control character found while parsing a string.
    ControlCharacterWhileParsingString,

    /// Object key is not a string.
    KeyMustBeAString,

    /// Lone leading surrogate in hex escape.
    LoneLeadingSurrogateInHexEscape,

    /// JSON has a comma after the last value in an array or map.
    TrailingComma,

    /// JSON has non-whitespace trailing characters after the value.
    TrailingCharacters,

    /// Unexpected end of hex escape.
    UnexpectedEndOfHexEscape,

    /// Encountered nesting of JSON maps and arrays more than 128 layers deep.
    RecursionLimitExceeded,

    /// Get value from a empty object
    GetInEmptyObj,

    /// Get unknown key from a object
    GetUnknownKeyInObj,

    /// Get value from a empty array
    GetInEmptyArray,

    /// Get index out of array
    GetIndexOutOfArray,

    /// Unexpected visit type
    UnexpectedVisitType,
}

impl Error {
    #[cold]
    pub(crate) fn syntax(code: ErrorCode, line: usize, column: usize) -> Self {
        Error {
            err: Box::new(ErrorImpl { code, line, column }),
        }
    }

    // Not public API. Should be pub(crate).
    //
    // Update `eager_json` crate when this function changes.
    #[doc(hidden)]
    #[cold]
    pub fn io(error: std::io::Error) -> Self {
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
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorCode::Message(msg) => f.write_str(msg),
            ErrorCode::Io(err) => Display::fmt(err, f),
            code => match code {
                ErrorCode::EofWhileParsingArray => f.write_str("EOF while parsing a array"),
                ErrorCode::EofWhileParsingObject => f.write_str("EOF while parsing an object"),
                ErrorCode::EofWhileParsingString => f.write_str("EOF while parsing a string"),
                ErrorCode::EofWhileParsingNumber => f.write_str("EOF while parsing a number"),
                ErrorCode::EofWhileParsingLiteral => {
                    f.write_str("EOF while parsing a literal, true, false or null")
                }
                ErrorCode::EofAfterSkipSpace => f.write_str("EOF after skip space"),
                ErrorCode::ExpectedColon => f.write_str("expected `:`"),
                ErrorCode::ExpectedArrayCommaOrEnd => f.write_str("expected `,` or `]`"),
                ErrorCode::ExpectedObjectCommaOrEnd => f.write_str("expected `,` or `}`"),
                ErrorCode::ExpectedSomeLiteral => f.write_str("expected literal "),
                ErrorCode::ExpectedSomeValue => f.write_str("expected value"),
                ErrorCode::ExpectedObject => f.write_str("expect '{'"),
                ErrorCode::ExpectedArray => f.write_str("expect '['"),
                ErrorCode::InvalidEscape => f.write_str("invalid escape"),
                ErrorCode::InvalidNumber => f.write_str("invalid number"),
                ErrorCode::NumberOutOfRange => f.write_str("number out of range"),
                ErrorCode::InvalidUnicodeCodePoint => f.write_str("invalid unicode code point"),
                ErrorCode::ControlCharacterWhileParsingString => {
                    f.write_str("control character (\\u0000-\\u001F) found while parsing a string")
                }
                ErrorCode::KeyMustBeAString => f.write_str("key must be a string"),
                ErrorCode::LoneLeadingSurrogateInHexEscape => {
                    f.write_str("lone leading surrogate in hex escape")
                }
                ErrorCode::MimatchedNumberFormat => {
                    f.write_str("Mismatched number format, such as parse \"0.123\" to a integer")
                }
                ErrorCode::NumberWithLeadingZero => f.write_str(
                    "Number with leading zero is not allowed, such as \"0123\", \"0e123\"",
                ),
                ErrorCode::TrailingComma => f.write_str("trailing comma"),
                ErrorCode::TrailingCharacters => f.write_str("trailing characters"),
                ErrorCode::UnexpectedEndOfHexEscape => f.write_str("unexpected end of hex escape"),
                ErrorCode::RecursionLimitExceeded => f.write_str("recursion limit exceeded"),
                _ => f.write_str("unpected error, should not has error here"),
            },
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

impl serde::de::StdError for ErrorCode {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
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
            "Error({:?}, line: {}, column: {})",
            self.err.code.to_string(),
            self.err.line,
            self.err.column
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
