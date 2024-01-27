pub(crate) trait JsonVisitor<'de> {
    fn visit_null(&mut self) -> bool {
        false
    }

    fn visit_bool(&mut self, value: bool) -> bool {
        let _ = value;
        false
    }

    fn visit_u64(&mut self, value: u64) -> bool {
        let _ = value;
        false
    }

    fn visit_i64(&mut self, value: i64) -> bool {
        let _ = value;
        false
    }

    fn visit_f64(&mut self, value: f64) -> bool {
        let _ = value;
        false
    }

    fn visit_str(&mut self, value: &str) -> bool {
        let _ = value;
        false
    }

    /// borrowed str is always without escaped characters
    fn visit_borrowed_str(&mut self, value: &'de str) -> bool {
        let _ = value;
        false
    }

    fn visit_key(&mut self, key: &str) -> bool {
        let _ = key;
        false
    }

    /// borrowed str is always without escaped characters
    fn visit_borrowed_key(&mut self, key: &'de str) -> bool {
        let _ = key;
        false
    }

    fn visit_object_start(&mut self, hint: usize) -> bool {
        let _ = hint;
        false
    }

    fn visit_object_end(&mut self, len: usize) -> bool {
        let _ = len;
        false
    }

    fn visit_array_start(&mut self, hint: usize) -> bool {
        let _ = hint;
        false
    }

    fn visit_array_end(&mut self, len: usize) -> bool {
        let _ = len;
        false
    }

    ///////////////////////////////////////////////////////////////////////////
    /// Vistor for position and string status
    ///
    /// pos: the valueue start position in JSON text
    /// has_escaped: the JSON string has escaped characters
    ///////////////////////////////////////////////////////////////////////////

    fn visit_u64_pos(&mut self, value: u64, pos: usize) -> bool {
        let _ = pos;
        self.visit_u64(value)
    }

    fn visit_i64_pos(&mut self, value: i64, pos: usize) -> bool {
        let _ = pos;
        self.visit_i64(value)
    }

    fn visit_f64_pos(&mut self, value: f64, pos: usize) -> bool {
        let _ = pos;
        self.visit_f64(value)
    }

    fn visit_str_status(&mut self, value: &str, has_escaped: bool) -> bool {
        let _ = has_escaped;
        self.visit_str(value)
    }

    fn visit_object_start_pos(&mut self, hint: usize, pos: usize) -> bool {
        let _ = pos;
        self.visit_object_start(hint)
    }

    fn visit_array_start_pos(&mut self, hint: usize, pos: usize) -> bool {
        let _ = pos;
        self.visit_array_start(hint)
    }

    fn visit_key_status(&mut self, key: &str, has_escaped: bool) -> bool {
        let _ = has_escaped;
        self.visit_key(key)
    }
}
