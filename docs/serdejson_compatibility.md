# A quick guide to migrate from serde_json

The goal of sonic-rs is performance and easiness (more APIs and ALLINONE) to use. Otherwise, recommended to use `serde_json`.

Just replace as follows:

- `&'a serde_json::RawValue` -> `sonic_rs::LazyValue<'a>`

- `Box<serde_json::RawValue>` -> `sonic_rs::OwnedLazyValue`

- `serde_json::Value` -> `sonic_rs::Value` (Note: different when JSON has duplicate keys)

- `serde_json::RawNumber` ->  `sonic_rs::RawNumber`

