pub(crate) struct MockString(String);

impl std::ops::Deref for MockString {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::ops::DerefMut for MockString {
    fn deref_mut(&mut self) -> &mut str {
        &mut self.0
    }
}

impl From<String> for MockString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MockString {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl Drop for MockString {
    fn drop(&mut self) {
        // clear memory expictly before drop
        let bs = unsafe { self.0.as_bytes_mut() };
        for b in bs.iter_mut() {
            *b = 0;
        }
    }
}
