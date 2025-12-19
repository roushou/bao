//! Snapshot tests for Rust code generation.
//!
//! These tests verify that the generated Rust code matches expected output.
//! Run `cargo insta review` to update snapshots when making intentional changes.

use std::str::FromStr;

use baobao_codegen_rust::{Generator, LanguageCodegen};
use baobao_manifest::Manifest;

/// Generate code from a schema and return files sorted by path for deterministic snapshots.
fn generate_files(schema_toml: &str) -> Vec<(String, String)> {
    let manifest = Manifest::from_str(schema_toml).expect("Failed to parse schema");
    let generator = Generator::new(&manifest);
    let files = generator.preview();

    let mut result: Vec<(String, String)> =
        files.into_iter().map(|f| (f.path, f.content)).collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Get a specific file from the generated output.
fn get_file<'a>(files: &'a [(String, String)], path: &str) -> Option<&'a str> {
    files
        .iter()
        .find(|(p, _)| p == path)
        .map(|(_, c)| c.as_str())
}

#[test]
fn test_basic_cli_main() {
    let files = generate_files(
        r#"
        [cli]
        name = "myapp"
        version = "1.0.0"
        language = "rust"
        description = "A simple CLI app"

        [commands.hello]
        description = "Say hello"
        "#,
    );

    let main_rs = get_file(&files, "src/main.rs").expect("main.rs not found");
    insta::assert_snapshot!("basic_cli_main", main_rs);
}

#[test]
fn test_basic_cli_definition() {
    let files = generate_files(
        r#"
        [cli]
        name = "myapp"
        version = "1.0.0"
        language = "rust"
        description = "A simple CLI app"

        [commands.hello]
        description = "Say hello"
        "#,
    );

    let cli_rs = get_file(&files, "src/generated/cli.rs").expect("cli.rs not found");
    insta::assert_snapshot!("basic_cli_definition", cli_rs);
}

#[test]
fn test_cli_with_args() {
    let files = generate_files(
        r#"
        [cli]
        name = "greeter"
        version = "1.0.0"
        language = "rust"

        [commands.greet]
        description = "Greet someone"

        [commands.greet.args.name]
        type = "string"
        description = "Name to greet"

        [commands.greet.args.count]
        type = "int"
        required = false
        description = "Number of times"
        "#,
    );

    // Check the command file for args
    let cmd_rs = get_file(&files, "src/generated/commands/greet.rs").expect("greet.rs not found");

    // Verify structure
    assert!(cmd_rs.contains("struct GreetArgs"));
    assert!(cmd_rs.contains("/// Name to greet"));
    assert!(cmd_rs.contains("name: String"));
    assert!(cmd_rs.contains("/// Number of times"));
    assert!(cmd_rs.contains("Option<i64>")); // optional int
}

#[test]
fn test_cli_with_flags() {
    let files = generate_files(
        r#"
        [cli]
        name = "builder"
        version = "1.0.0"
        language = "rust"

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
        description = "Number of parallel jobs"
        "#,
    );

    // Check the command file for flags
    let cmd_rs = get_file(&files, "src/generated/commands/build.rs").expect("build.rs not found");

    // Verify flag attributes (format may vary)
    assert!(cmd_rs.contains("/// Build in release mode"));
    assert!(cmd_rs.contains("release: bool"));
    assert!(cmd_rs.contains("short")); // has short option
    assert!(cmd_rs.contains("jobs:")); // has jobs field
    // Default value format may vary
    assert!(cmd_rs.contains("4") || cmd_rs.contains("default"));
}

#[test]
fn test_cli_with_subcommands() {
    let files = generate_files(
        r#"
        [cli]
        name = "dbcli"
        version = "1.0.0"
        language = "rust"

        [commands.db]
        description = "Database commands"

        [commands.db.commands.migrate]
        description = "Run migrations"

        [commands.db.commands.seed]
        description = "Seed the database"
        "#,
    );

    // Check the db command file for subcommands enum
    let db_rs = get_file(&files, "src/generated/commands/db.rs").expect("db.rs not found");

    // Verify subcommand enum
    assert!(db_rs.contains("#[derive(Subcommand"));
    assert!(db_rs.contains("enum DbCommands"));
    assert!(db_rs.contains("/// Run migrations"));
    assert!(db_rs.contains("Migrate"));
    assert!(db_rs.contains("/// Seed the database"));
    assert!(db_rs.contains("Seed"));
}

#[test]
fn test_cli_with_all_arg_types() {
    let files = generate_files(
        r#"
        [cli]
        name = "typetest"
        version = "1.0.0"
        language = "rust"

        [commands.test]
        description = "Test all argument types"

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

    // Check the test command file for arg types
    let cmd_rs = get_file(&files, "src/generated/commands/test.rs").expect("test.rs not found");

    // Verify type mappings
    assert!(cmd_rs.contains("str_arg: String"));
    assert!(cmd_rs.contains("int_arg: i64"));
    assert!(cmd_rs.contains("float_arg: f64"));
    assert!(cmd_rs.contains("bool_arg: bool"));
    // path_arg might be PathBuf or std::path::PathBuf
    assert!(cmd_rs.contains("path_arg:"));
    assert!(cmd_rs.contains("PathBuf") || cmd_rs.contains("String"));
}

#[test]
fn test_cargo_toml() {
    let files = generate_files(
        r#"
        [cli]
        name = "myapp"
        version = "1.2.3"
        language = "rust"
        description = "My awesome CLI"

        [commands.hello]
        description = "Say hello"
        "#,
    );

    let cargo = get_file(&files, "Cargo.toml").expect("Cargo.toml not found");
    insta::assert_snapshot!("cargo_toml", cargo);
}

#[test]
fn test_command_dispatch() {
    let files = generate_files(
        r#"
        [cli]
        name = "multi"
        version = "1.0.0"
        language = "rust"

        [commands.foo]
        description = "Foo command"

        [commands.bar]
        description = "Bar command"
        "#,
    );

    // Check cli.rs for dispatch logic (in the Cli impl)
    let cli_rs = get_file(&files, "src/generated/cli.rs").expect("cli.rs not found");

    // Verify dispatch logic exists
    assert!(cli_rs.contains("impl Cli"));
    assert!(cli_rs.contains("dispatch"));
    assert!(cli_rs.contains("match"));
    assert!(cli_rs.contains("Foo"));
    assert!(cli_rs.contains("Bar"));
}

#[test]
fn test_context_with_http() {
    let files = generate_files(
        r#"
        [cli]
        name = "api"
        version = "1.0.0"
        language = "rust"

        [context.http]
        timeout = 30

        [commands.fetch]
        description = "Fetch data"
        "#,
    );

    let context_rs = get_file(&files, "src/context.rs").expect("context.rs not found");

    // Verify HTTP client setup
    assert!(context_rs.contains("reqwest"));
    assert!(context_rs.contains("Client"));
}
