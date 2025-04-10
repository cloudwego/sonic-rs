pub(crate) trait JsonVisitor<'de> {
    fn visit_dom_start(&mut self) -> bool {
        false
    }

    fn visit_null(&mut self) -> bool {
        false
    }

    fn visit_bool(&mut self, _val: bool) -> bool {
        false
    }

    #[allow(dead_code)]
    fn visit_u64(&mut self, _val: u64) -> bool {
        false
    }

    #[allow(dead_code)]
    fn visit_i64(&mut self, _val: i64) -> bool {
        false
    }

    #[allow(dead_code)]
    fn visit_f64(&mut self, _val: f64) -> bool {
        false
    }

    #[allow(dead_code)]
    fn visit_raw_number(&mut self, _val: &str) -> bool {
        false
    }

    #[allow(dead_code)]
    fn visit_borrowed_raw_number(&mut self, _val: &str) -> bool {
        false
    }

    fn visit_str(&mut self, _value: &str) -> bool {
        false
    }

    fn visit_borrowed_str(&mut self, _value: &'de str) -> bool {
        false
    }

    fn visit_object_start(&mut self, _hint: usize) -> bool {
        false
    }

    fn visit_object_end(&mut self, _len: usize) -> bool {
        false
    }

    fn visit_array_start(&mut self, _hint: usize) -> bool {
        false
    }

    fn visit_array_end(&mut self, _len: usize) -> bool {
        false
    }

    #[allow(dead_code)]
    fn visit_key(&mut self, _key: &str) -> bool {
        false
    }

    #[allow(dead_code)]
    fn visit_borrowed_key(&mut self, _key: &'de str) -> bool {
        false
    }

    fn visit_dom_end(&mut self) -> bool {
        false
    }
}
