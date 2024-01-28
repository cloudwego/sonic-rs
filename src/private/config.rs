#[derive(Debug, Default)]
pub struct Config {
    /// parse number as RawNumber
    pub use_number: bool,
    /// checking and repr invalid UTF-8 chars
    pub validate_string: bool,
    /// not return error when invalid UTF-16 surrogates
    pub disable_surrogates_error: bool,
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn use_number(mut self, use_number: bool) -> Self {
        self.use_number = use_number;
        self
    }

    pub fn validate_string(mut self, validate_string: bool) -> Self {
        self.validate_string = validate_string;
        self
    }

    pub fn disable_surrogates_error(mut self, disable_surrogates_error: bool) -> Self {
        self.disable_surrogates_error = disable_surrogates_error;
        self
    }
}
