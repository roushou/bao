//! Integration tests that verify bao.toml is not overwritten during code generation.
//!
//! This is a regression test for a bug where `bao bake` would overwrite the user's
//! bao.toml with a default template containing only a "hello" command.

use std::str::FromStr;

use baobao_codegen::language::LanguageCodegen;
use baobao_codegen_typescript::Generator;
use baobao_manifest::Manifest;
use tempfile::TempDir;

/// Verify that generating code does not overwrite an existing bao.toml file.
///
/// This is a regression test for a bug where the TypeScript generator would
/// register bao.toml as a config file with Overwrite::Always, causing it to
/// be overwritten with a default template on every `bao bake` run.
#[test]
fn test_bao_toml_not_overwritten_during_generation() {
    let schema_toml = r#"
        [cli]
        name = "myapp"
        version = "1.2.3"
        language = "typescript"
        description = "My custom CLI app"

        [commands.custom]
        description = "A custom command"

        [commands.custom.args.input]
        type = "string"
        description = "Input file"

        [commands.another]
        description = "Another command"

        [commands.another.flags.verbose]
        type = "bool"
        short = "v"
    "#;

    let schema = Manifest::from_str(schema_toml).expect("Failed to parse schema");
    let generator = Generator::new(&schema);

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path();

    // Create a custom bao.toml BEFORE generation
    let custom_bao_toml = r#"[cli]
name = "myapp"
version = "1.2.3"
language = "typescript"
description = "My custom CLI app"

[commands.custom]
description = "A custom command"

[commands.custom.args.input]
type = "string"
description = "Input file"

[commands.another]
description = "Another command"

[commands.another.flags.verbose]
type = "bool"
short = "v"
"#;

    let bao_toml_path = output_dir.join("bao.toml");
    std::fs::write(&bao_toml_path, custom_bao_toml).expect("Failed to write bao.toml");

    // Generate code
    generator
        .generate(output_dir)
        .expect("Failed to generate code");

    // Verify bao.toml was NOT overwritten
    let content_after =
        std::fs::read_to_string(&bao_toml_path).expect("Failed to read bao.toml after generation");

    assert_eq!(
        content_after, custom_bao_toml,
        "bao.toml should not be modified during code generation!\n\
         Expected (original):\n{}\n\n\
         Got (after generation):\n{}",
        custom_bao_toml, content_after
    );
}

/// Verify that bao.toml is not created if it doesn't exist.
///
/// The bao.toml file is the source of truth and should only be created
/// during `bao init`, not during `bao bake` (code generation).
#[test]
fn test_bao_toml_not_created_during_generation() {
    let schema_toml = r#"
        [cli]
        name = "testapp"
        language = "typescript"

        [commands.hello]
        description = "Say hello"
    "#;

    let schema = Manifest::from_str(schema_toml).expect("Failed to parse schema");
    let generator = Generator::new(&schema);

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path();

    // Do NOT create bao.toml before generation
    let bao_toml_path = output_dir.join("bao.toml");
    assert!(
        !bao_toml_path.exists(),
        "bao.toml should not exist before generation"
    );

    // Generate code
    generator
        .generate(output_dir)
        .expect("Failed to generate code");

    // Verify bao.toml was NOT created
    assert!(
        !bao_toml_path.exists(),
        "bao.toml should not be created during code generation - it's created by `bao init`"
    );
}
