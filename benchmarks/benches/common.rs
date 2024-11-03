#[derive(Debug, Clone, Copy)]
struct SonicConfig {
    use_raw: bool,
    use_rawnum: bool,
}

static SONIC_DEFAULT_CFG: SonicConfig = SonicConfig {
    use_raw: false,
    use_rawnum: false,
};

static SONIC_USE_RAWNUM_CFG: SonicConfig = SonicConfig {
    use_raw: false,
    use_rawnum: true,
};

static SONIC_USE_RAW_CFG: SonicConfig = SonicConfig {
    use_raw: true,
    use_rawnum: false,
};

fn do_sonic_rs_from_slice(data: &[u8], cfg: SonicConfig) -> sonic_rs::Result<sonic_rs::Value> {
    let mut de = sonic_rs::Deserializer::new(sonic_rs::Read::from(data));
    if cfg.use_rawnum {
        de = de.use_rawnumber();
    }
    if cfg.use_raw {
        de = de.use_raw();
    }
    sonic_rs::Deserialize::deserialize(&mut de)
}
