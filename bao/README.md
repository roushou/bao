# baobao

CLI tool for generating type-safe CLI applications from a simple TOML schema.

This is the main binary crate for [Bao](https://github.com/roushou/bao). For full documentation, see the [main repository](https://github.com/roushou/bao).

## Installation

```bash
cargo install baobao
```

## Usage

```bash
# Initialize a new bao.toml in the current directory
bao init --language rust

# Generate CLI code from bao.toml
bao bake

# Preview what would be generated without writing files
bao bake --dry-run

# Clean generated files
bao clean

# Format the bao.toml file
bao fmt

# Show project info
bao info
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
bao bake
```

## Supported Languages

- **Rust** - Generates CLI applications using [clap](https://crates.io/crates/clap)
- **TypeScript** - Generates CLI applications for [Bun](https://bun.com/) using [boune](https://www.npmjs.com/package/boune)

## License

This project is licensed under the [MIT](https://github.com/roushou/bao/blob/main/LICENSE) license.
