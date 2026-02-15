# Installation

Pluto is currently installed from source.

## Prerequisites

- Rust toolchain (stable): <https://rustup.rs>
- C toolchain for linking generated binaries (`clang` or `gcc`)

## Building from source

```bash
git clone https://github.com/Mkerian10/pluto.git
cd pluto
cargo build --release
```

The compiler binary will be at `target/release/pluto`.

## Optional: Install to PATH

```bash
cargo install --path .
```

Then verify installation:

```bash
pluto --help
```
