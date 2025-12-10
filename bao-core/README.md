# baobao-core

Core utilities and types for [Bao](https://github.com/roushou/bao) CLI generator.

This crate provides fundamental types and utilities used across the Bao ecosystem.

## Features

- **File Operations** - Types for managing generated files (`File`, `GeneratedFile`, `WriteResult`)
- **Type Mapping** - Argument type definitions (`ArgType`)
- **Context Types** - Database and context field types (`ContextFieldType`, `DatabaseType`)
- **String Utilities** - Case conversion functions (`to_camel_case`, `to_kebab_case`, `to_pascal_case`, `to_snake_case`)
- **Version Handling** - Semantic version parsing and manipulation (`Version`)

## Usage

This crate is used internally by other `baobao-*` crates. You typically don't need to use it directly.

```rust
use baobao_core::{to_snake_case, to_pascal_case, ArgType, Version};

// Case conversion
assert_eq!(to_snake_case("helloWorld"), "hello_world");
assert_eq!(to_pascal_case("hello_world"), "HelloWorld");

// Version parsing
let version = Version::parse("1.2.3").unwrap();
```

## License

This project is licensed under the [MIT](https://github.com/roushou/bao/blob/main/LICENSE) license.
