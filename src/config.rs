#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DeserializeCfg {
    pub use_rawnumber: bool,
    pub utf8_lossy: bool,
}
