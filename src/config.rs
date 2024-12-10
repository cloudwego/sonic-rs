#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DeserializeCfg {
    pub use_rawnumber: bool,
    pub use_raw: bool,
    pub utf8_lossy: bool,
    pub skip_strict: bool,
}
