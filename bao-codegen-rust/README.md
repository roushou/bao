# baobao-codegen-rust

Rust code generator for [Bao](https://github.com/roushou/bao) CLI generator.

This crate generates Rust CLI applications using [clap](https://crates.io/crates/clap) for argument parsing.

## Usage

This crate is used internally by the `baobao` CLI tool. You typically don't need to use it directly.

```rust
use baobao_codegen_rust::Generator;
use baobao_codegen::LanguageCodegen;
use baobao_manifest::Manifest;
use std::path::Path;

let manifest = Manifest::from_file("bao.toml")?;
let generator = Generator::new(&manifest);

// Preview files without writing
let files = generator.preview();

// Generate files to disk
let result = generator.generate(Path::new("output"))?;
```

## Generated Output

The generator produces a Rust CLI project:

```
output/
├── src/
│   ├── cli.rs          # CLI definition with clap derive macros
│   ├── context.rs      # Shared context (database pools, HTTP clients)
│   ├── main.rs         # Entry point and command dispatch
│   ├── commands/       # Command modules
│   │   └── *.rs
│   └── handlers/       # Handler stubs for implementation
│       └── *.rs
├── Cargo.toml
├── bao.toml
└── .gitignore
```

## Features

- **Type-safe CLI** - Uses clap derive macros for compile-time argument validation
- **Handler Stubs** - Generates handler functions with correct signatures
- **Context Support** - Generates context structs for database pools and HTTP clients
- **Subcommands** - Full support for nested command hierarchies

## License

This project is licensed under the [MIT](https://github.com/roushou/bao/blob/main/LICENSE) license.
