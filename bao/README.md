# baobao

Bao is a **very** opinionated tool to generate CLI applications in multiple programming languages from a single configuration file.

## Crates

> [!Note]
> Crates are published as `baobao*` on crates.io instead of `bao*` to avoid confusion with an already existing and unrelated `bao` crate.

| Crate | Description |
|-------|-------------|
| [baobao](https://crates.io/crates/baobao) | CLI tool for generating CLI applications from TOML |
| [baobao-core](https://crates.io/crates/baobao-core) | Core utilities for Bao CLI generator |
| [baobao-manifest](https://crates.io/crates/baobao-manifest) | TOML manifest parsing and validation |
| [baobao-codegen](https://crates.io/crates/baobao-codegen) | Shared code generation utilities |
| [baobao-codegen-rust](https://crates.io/crates/baobao-codegen-rust) | Rust code generator |
| [baobao-codegen-typescript](https://crates.io/crates/baobao-codegen-typescript) | TypeScript code generator |


## Installation

```bash
cargo install baobao
```

## Quick Start

Initialize a new project:

```bash
bao init myapp
# Select language interactively, or use: bao init myapp --language rust
```

This creates a complete project with a `bao.toml` manifest:

```toml
[cli]
name = "myapp"
version = "0.1.0"

[commands.hello]
description = "Say hello"
args = ["name"]
flags = ["uppercase"]
```

Edit `bao.toml` to add commands, then regenerate:

```bash
bao bake
```

## Commands

| Command | Description |
|---------|-------------|
| `bao init [name]` | Initialize a new CLI project |
| `bao bake` | Generate code from bao.toml |
| `bao add command <name>` | Add a new command |
| `bao remove command <name>` | Remove a command |
| `bao list` | List commands and context |
| `bao check` | Validate bao.toml |
| `bao clean` | Remove orphaned generated files |
| `bao run` | Run the CLI (shortcut for `cargo run --`) |

## Features

- Type-safe argument parsing (clap for Rust, boune for TypeScript)
- Handler stubs generated for each command
- Context for shared state (database pools, HTTP clients, etc.)
- Multiple language targets from a single manifest

## License

This project is licensed under the [MIT](../LICENSE) LICENSE
