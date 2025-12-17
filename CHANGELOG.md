## [0.4.2] - 2025-12-17

### 🚀 Features

- Setup changelog and display on the documentation website

### 🐛 Bug Fixes

- *(codegen-typescript)* Place `Shebang` at the top of the file
- *(codegen-typescript)* Don't overwrite manifest on bake

### 📚 Documentation

- Update docs landing page

### ⚙️ Miscellaneous Tasks

- Remove roadmap section from documentation website
## [0.4.1] - 2025-12-17

### 🚀 Features

- *(codegen)* Add `Renderable` trait and `CodeFile` abstraction for decoupled code generation
- *(codegen)* Add adapter traits and implementations for decoupled code generation
- *(bao)* Add interactive prompt to `bao init` to select language
- *(codegen)* Add FileRegistry for declarative file generation
- *(codegen)* Support `clean` and `preview_clean` in generators
- *(codegen)* Make handler stub marker configurable per language
- *(docs)* Add documentation website

### 🚜 Refactor

- *(codegen)* Reorganize `codegen` crate into a domain-based structure
- Decouple language-specific code and reduce duplication
- *(codegen)* Replace recursive handler generation with CommandTree iteration

### 📚 Documentation

- Update `bao` README files

### ⚙️ Miscellaneous Tasks

- Match release tag with release workflow
- Release v0.4.1
## [0.4.0] - 2025-12-11

### 🚀 Features

- *(codegen-ts)* Update to `boune` declarative API

### ⚙️ Miscellaneous Tasks

- Release
## [0.3.3] - 2025-12-10

### 🐛 Bug Fixes

- Generate Typescript commands files recursively for nested commands

### ⚙️ Miscellaneous Tasks

- Release
## [0.3.2] - 2025-12-10

### 🐛 Bug Fixes

- Correctly locate reserved keywords in inline table syntax

### ⚙️ Miscellaneous Tasks

- Add configuration files for `cargo release`
- Release
## [baobao-v0.3.1] - 2025-12-10

### 🚀 Features

- Add `Version` struct to work with versions instead of using plain string

### 🚜 Refactor

- Move `BaoToml` to `bao-codegen` as shared implementation

### 📚 Documentation

- Add README files for each crates and update docs

### ⚙️ Miscellaneous Tasks

- Add release workflow
- Release
## [baobao-v0.3.0] - 2025-12-10

### 🚀 Features

- Enforce strict context field names and add SQLite path support
- Add validation for Rust reserved keywords and invalid identifiers
- Add support for Rust `match` expression
- Add support for dashed field names in manifest
- Add `clean` command that removes orphaned generated files
- Add `fmt` command
- *(bao)* Add `info` command
- *(bao)* Add `rename` command
- Add TypeScript codegen

### 🚜 Refactor

- Add language-agnostic abstractions for multi-language codegen
- Add language-agnostic abstractions for multi-language codegen
- Reorganize codegen with AST builders and language-specific modules
- Rename `bao-schema` to `bao-manifest`
- *(manifest)* Split command.rs into module structure
- Add `UnwrapOrExit` trait for more graceful process exits
- *(manifest)* Avoid cloning in `Context::fields()` by returning references
- *(codegen)* Unify command travesal APIs with `CommandTree` abstraction
- *(manifest)* Rename `Schema` to `Manifest` and reorganize module
- *(manifest)* Introduce `ParseContext` for cleaner validation
- *(manifest)* Add `DatabaseConfig` trait
- *(manifest)* Unify `PostgresConfig` and `MySqlConfig` with `BasicDbConfig`
- *(manifest)* Encapsulate source content and filename for error creation
- *(manifest)* Split up `manifest.rs` into module

### 🧪 Testing

- Use shared `target` directory for integration tests

### ⚙️ Miscellaneous Tasks

- Add codegen tests
- Format toml files
- Bump version
- Rename `generate` command to `bake`
- Update README and bump crates versions for release
- *(bao)* Default `init` command to `cwd`
- Bump versions
- Fix lint
- Release
