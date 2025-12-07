# Bao

Bao is a **very** opinionated tool to generate CLI applications in multiple programming languages from a single configuration file.

## Crates

> [!Note]
> Crates are published as `baobao*` on crates.io instead of `bao*` to avoid confusion with an already existing and unrelated `bao` crate.

| Crate | Description |
|-------|-------------|
| [baobao](https://crates.io/crates/baobao) | CLI tool for generating CLI applications from TOML |
| [baobao-core](https://crates.io/crates/baobao-core) | Core utilities for Bao CLI generator |
| [baobao-schema](https://crates.io/crates/baobao-schema) | TOML schema parsing and validation |
| [baobao-codegen-rust](https://crates.io/crates/baobao-codegen-rust) | Rust code generator |


## Installation

```bash
cargo install baobao
```

## Quick Start

Create a `bao.toml` file:

```toml
[cli]
name = "myapp"
version = "0.1.0"

[commands.hello]
description = "Say hello"
args = ["name"]

[commands.greet]
description = "Greet someone"
args = ["name"]
flags = ["loud"]
```

Generate your CLI:

```bash
bao generate
```

This creates a complete Rust project with:
- Type-safe argument parsing using clap
- Handler stubs for each command
- Context for shared state (database pools, HTTP clients, etc.)

## License

This project is licensed under the [MIT](./LICENSE) LICENSE
