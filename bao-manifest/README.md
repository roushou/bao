# baobao-manifest

TOML manifest parsing and validation for [Bao](https://github.com/roushou/bao) CLI generator.

This crate handles parsing and validation of `bao.toml` configuration files that define CLI structure.

## Features

- **Manifest Parsing** - Parse `bao.toml` files into structured types
- **Command Definitions** - Types for commands, arguments, and flags
- **Context Configuration** - Database and HTTP client configuration
- **Validation** - Error reporting with source context using [miette](https://crates.io/crates/miette)

## Usage

This crate is used internally by the `baobao` CLI tool. You typically don't need to use it directly.

```rust
use baobao_manifest::Manifest;

// Parse a bao.toml file
let manifest = Manifest::from_file("bao.toml")?;

// Access CLI configuration
println!("CLI name: {}", manifest.cli.name);
println!("Version: {}", manifest.cli.version);

// Iterate over commands
for (name, command) in &manifest.commands {
    println!("Command: {} - {}", name, command.description);
}
```

## Manifest Structure

A `bao.toml` file defines:

- **CLI metadata** - Name, version, description
- **Commands** - With arguments, flags, and subcommands
- **Context** - Shared state like database pools and HTTP clients

```toml
[cli]
name = "myapp"
version = "0.1.0"

[commands.hello]
description = "Say hello"
args = ["name"]
flags = ["loud"]

[context.db]
type = "sqlite"
path = "app.db"
```

## License

This project is licensed under the [MIT](https://github.com/roushou/bao/blob/main/LICENSE) license.
