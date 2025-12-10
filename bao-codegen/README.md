# baobao-codegen

Shared code generation utilities for [Bao](https://github.com/roushou/bao) CLI generator.

This crate provides language-agnostic abstractions and utilities used by language-specific code generators (e.g., `baobao-codegen-rust`, `baobao-codegen-typescript`).

## Features

- **Code Building** - Utilities for generating code (`CodeBuilder`, `FileBuilder`)
- **Command Tree** - Structures for representing command hierarchies (`CommandTree`, `FlatCommand`)
- **Handler Management** - Tools for tracking handler files and detecting orphaned commands
- **Import Collection** - Dependency and import tracking (`DependencyCollector`, `ImportCollector`)
- **Naming Conventions** - Language-specific naming rules (`NamingConvention`)
- **Traits** - Common interfaces for code generators (`LanguageCodegen`, `TypeMapper`)

## Usage

This crate is used by language-specific code generators. You typically don't need to use it directly unless you're implementing a new language generator.

```rust
use baobao_codegen::{LanguageCodegen, PreviewFile, GenerateResult};
use baobao_codegen::{CodeBuilder, FileBuilder, CommandTree};

// Implement LanguageCodegen for a new language
impl LanguageCodegen for MyGenerator {
    fn preview(&self) -> Vec<PreviewFile> {
        // Return files that would be generated
    }

    fn generate(&self, output: &Path) -> eyre::Result<GenerateResult> {
        // Generate files to disk
    }
}
```

## Testing Support

Enable the `testing` feature for test utilities:

```toml
[dev-dependencies]
baobao-codegen = { version = "0.3", features = ["testing"] }
```

## License

This project is licensed under the [MIT](https://github.com/roushou/bao/blob/main/LICENSE) license.
