# baobao-ir

Intermediate representation types for [Bao](https://github.com/roushou/bao) CLI generator.

This crate provides unified type definitions used across the Bao code generation pipeline. These types serve as the single source of truth for configuration and resource representation, bridging manifest parsing and code generation.

## Architecture

```text
bao.toml (TOML) → bao-manifest (parsing) → bao-ir (unified types) → codegen
```

The IR types are designed to be:
- **Language-agnostic** - No Rust/TypeScript-specific concerns
- **Application-type agnostic** - CLI, HTTP server, etc.
- **Self-contained** - No external dependencies beyond std

## Features

- **Application IR** - Unified representation for CLI applications (`AppIR`, `AppMeta`)
- **Operations** - Command and route abstractions (`Operation`, `CommandOp`)
- **Resources** - Database and HTTP client configuration (`Resource`, `DatabaseResource`, `HttpClientResource`)
- **Inputs** - Type-safe parameter definitions (`Input`, `InputType`, `InputKind`)
- **Pool Configuration** - Connection pool settings (`PoolConfig`)
- **SQLite Options** - SQLite-specific configuration (`SqliteOptions`, `JournalMode`, `SynchronousMode`)

## Usage

This crate is used internally by `bao-manifest` and language-specific code generators. You typically don't need to use it directly unless you're implementing manifest lowering or a new language generator.

```rust
use baobao_ir::{AppIR, AppMeta, CommandOp, Input, InputType, InputKind};

// Create an application IR
let app = AppIR {
    meta: AppMeta {
        name: "myapp".into(),
        version: "0.1.0".into(),
        description: Some("My CLI application".into()),
        author: None,
    },
    resources: vec![],
    operations: vec![],
};
```

## License

This project is licensed under the [MIT](https://github.com/roushou/bao/blob/main/LICENSE) license.
