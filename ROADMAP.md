

# Sonic-rs RoadMap

This document shows key roadmap of `sonic-rs` development. It may help users know more about the future features. But the actual work is driven by real-world needs, we may adjust our goals sometimes.

## stability

1. ~~support utf-8 validate~~

2. ~~add more fuzzing tests~~

3. make unittest coverage to 90%


## Portability

0. ~~make sonic-rs support stable Rust~~

1. ~~optimize the performance in aarch64 (WIP: 50%)~~

2. runtime CPU detection

3. ~~support fallback in unsupported arch~~


## Features

1. support more JSON RFC:
- [`JSON Path`](https://datatracker.ietf.org/wg/jsonpath/about/).
- [`JSON Merge Patch`](https://www.rfc-editor.org/rfc/rfc7396).

2. support the `Deserializer` trait for document (document can be deserialized into a Rust type).

## Performance

1. support zero-copy for FastStr

2. maybe reimplement the `Deserialize` or `Serialize` trait ?.