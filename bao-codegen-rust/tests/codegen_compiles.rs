//! Integration tests that verify generated code compiles successfully.
//!
//! These tests generate Rust code from various schema configurations and run
//! `cargo check` to ensure the generated code is valid Rust.

use std::{path::Path, process::Command};

use baobao_codegen_rust::Generator;
use baobao_schema::parse_str;
use tempfile::TempDir;

/// Generate code from a schema and verify it compiles with `cargo check`
fn assert_generated_code_compiles(schema_toml: &str) {
    let schema = parse_str(schema_toml).expect("Failed to parse schema");
    let generator = Generator::new(&schema);

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path();

    generator
        .generate(output_dir)
        .expect("Failed to generate code");

    // Run cargo check on the generated code
    let status = Command::new("cargo")
        .args(["check", "--message-format=short"])
        .current_dir(output_dir)
        .output()
        .expect("Failed to run cargo check");

    if !status.status.success() {
        let stdout = String::from_utf8_lossy(&status.stdout);
        let stderr = String::from_utf8_lossy(&status.stderr);

        // Print generated files for debugging
        eprintln!("\n=== Generated files ===");
        print_generated_files(output_dir);

        panic!(
            "Generated code failed to compile!\n\nstdout:\n{}\n\nstderr:\n{}",
            stdout, stderr
        );
    }
}

/// Print all generated files for debugging
fn print_generated_files(dir: &Path) {
    fn print_dir(dir: &Path, indent: usize) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().unwrap().to_string_lossy();

                // Skip target directory
                if name == "target" {
                    continue;
                }

                if path.is_dir() {
                    eprintln!("{:indent$}{}/", "", name, indent = indent);
                    print_dir(&path, indent + 2);
                } else if path.extension().is_some_and(|e| e == "rs" || e == "toml") {
                    eprintln!("{:indent$}{}", "", name, indent = indent);
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        for line in content.lines() {
                            eprintln!("{:indent$}  | {}", "", line, indent = indent);
                        }
                    }
                }
            }
        }
    }
    print_dir(dir, 0);
}

#[test]
fn test_basic_cli_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"
        description = "A simple CLI app"

        [commands.hello]
        description = "Say hello"
        "#,
    );
}

#[test]
fn test_cli_with_args_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"

        [commands.greet]
        description = "Greet someone"

        [commands.greet.args.name]
        type = "string"
        description = "Name to greet"

        [commands.greet.args.count]
        type = "int"
        required = false
        "#,
    );
}

#[test]
fn test_cli_with_flags_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"

        [commands.build]
        description = "Build the project"

        [commands.build.flags.release]
        type = "bool"
        short = "r"
        description = "Build in release mode"

        [commands.build.flags.jobs]
        type = "int"
        short = "j"
        default = 4
        "#,
    );
}

#[test]
fn test_cli_with_all_arg_types_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"

        [commands.test]
        description = "Test all arg types"

        [commands.test.args.str_arg]
        type = "string"

        [commands.test.args.int_arg]
        type = "int"

        [commands.test.args.float_arg]
        type = "float"

        [commands.test.args.bool_arg]
        type = "bool"

        [commands.test.args.path_arg]
        type = "path"
        "#,
    );
}

#[test]
fn test_cli_with_subcommands_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"

        [commands.db]
        description = "Database commands"

        [commands.db.commands.migrate]
        description = "Run migrations"

        [commands.db.commands.seed]
        description = "Seed the database"

        [commands.db.commands.seed.args.file]
        type = "path"
        description = "Seed file"
        "#,
    );
}

#[test]
fn test_cli_with_nested_subcommands_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"

        [commands.config]
        description = "Configuration commands"

        [commands.config.commands.user]
        description = "User config"

        [commands.config.commands.user.commands.get]
        description = "Get user config"

        [commands.config.commands.user.commands.set]
        description = "Set user config"

        [commands.config.commands.user.commands.set.args.key]
        type = "string"

        [commands.config.commands.user.commands.set.args.value]
        type = "string"
        "#,
    );
}

#[test]
fn test_cli_with_multiple_commands_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"
        description = "Multi-command CLI"

        [commands.init]
        description = "Initialize a project"

        [commands.build]
        description = "Build the project"

        [commands.build.flags.release]
        type = "bool"

        [commands.test]
        description = "Run tests"

        [commands.test.flags.verbose]
        type = "bool"
        short = "v"

        [commands.clean]
        description = "Clean build artifacts"
        "#,
    );
}

#[test]
fn test_cli_with_optional_args_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"

        [commands.copy]
        description = "Copy files"

        [commands.copy.args.source]
        type = "path"
        description = "Source file"

        [commands.copy.args.dest]
        type = "path"
        required = false
        description = "Destination (optional)"
        "#,
    );
}

#[test]
fn test_cli_with_default_values_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"

        [commands.server]
        description = "Start a server"

        [commands.server.flags.port]
        type = "int"
        short = "p"
        default = 8080

        [commands.server.flags.host]
        type = "string"
        short = "h"
        default = "localhost"
        "#,
    );
}

#[test]
fn test_cli_with_http_context_compiles() {
    assert_generated_code_compiles(
        r#"
        [cli]
        name = "myapp"

        [context.http]
        timeout = 30
        user_agent = "myapp/1.0"

        [commands.fetch]
        description = "Fetch data from API"

        [commands.fetch.args.url]
        type = "string"
        "#,
    );
}

// Note: Database context tests require actual database drivers.
// Skipping them to avoid long compile times in CI.
// Uncomment to test locally if needed.

// #[test]
// fn test_cli_with_postgres_context_compiles() {
//     assert_generated_code_compiles(
//         r#"
//         [cli]
//         name = "myapp"
//
//         [context.database]
//         type = "postgres"
//         env = "DATABASE_URL"
//
//         [commands.query]
//         description = "Run a query"
//         "#,
//     );
// }
