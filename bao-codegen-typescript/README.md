# baobao-codegen-typescript

TypeScript code generator for [Bao](https://github.com/roushou/bao) CLI generator.

This crate generates TypeScript CLI applications using [boune](https://www.npmjs.com/package/boune), a CLI library targeting [Bun](https://bun.com/) runtime.

## Usage

This crate is used internally by the `baobao` CLI tool. You typically don't need to use it directly.

```rust
use baobao_codegen_typescript::Generator;
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

The generator produces a TypeScript CLI project:

```
output/
├── src/
│   ├── cli.ts          # Main CLI setup with boune
│   ├── context.ts      # Shared context (database pools, HTTP clients)
│   ├── index.ts        # Entry point
│   ├── commands/       # Command definitions
│   │   └── *.ts
│   └── handlers/       # Handler stubs for implementation
│       └── *.ts
├── package.json
├── tsconfig.json
├── bao.toml
└── .gitignore
```

## License

This project is licensed under the [MIT](https://github.com/roushou/bao/blob/main/LICENSE) license.
