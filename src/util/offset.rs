#[macro_export]
macro_rules! field_offset {
    ($type:ty, $field:tt) => {{
        let dummy = std::mem::MaybeUninit::<$type>::uninit();
        let dummy_ptr = dummy.as_ptr();
        let member_ptr = unsafe { std::ptr::addr_of!((*dummy_ptr).$field) };
        member_ptr as usize - dummy_ptr as usize
    }};
}
