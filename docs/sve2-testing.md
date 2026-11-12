# Testing the SVE2 experiment

Ordinary `cargo test` uses the native runner. QEMU and SVE2 code generation are
opt-in, so native ARM Linux tests do not require a cross-compilation setup or
force SVE2 instructions on a CPU that does not support them.

On Debian/Ubuntu, prepare the cross-compilation tools and Rust standard library:

```sh
sudo apt-get install qemu-user gcc-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
```

Run from the repository root:

```sh
cargo --config .cargo/sve2-qemu.toml test --target aarch64-unknown-linux-gnu
```

The config uses `/usr/aarch64-linux-gnu` as the guest sysroot and
`qemu-aarch64 -cpu max,sve=on`. Adjust the explicit config if your tools are
installed elsewhere. The command requires QEMU with SVE2 support.

A compile check alone does not validate SVE execution. This remains an
experiment, with correctness and performance work tracked in PR #209.
