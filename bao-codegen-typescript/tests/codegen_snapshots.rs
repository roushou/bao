//! Snapshot tests for TypeScript code generation.
//!
//! These tests verify that the generated TypeScript code matches expected output.
//! Run `cargo insta review` to update snapshots when making intentional changes.

use std::str::FromStr;

use baobao_codegen::pipeline::Pipeline;
use baobao_codegen_typescript::{Generator, LanguageCodegen};
use baobao_manifest::Manifest;

/// Generate code from a schema and return files sorted by path for deterministic snapshots.
fn generate_files(schema_toml: &str) -> Vec<(String, String)> {
    let manifest = Manifest::from_str(schema_toml).expect("Failed to parse schema");
    let pipeline = Pipeline::new();
    let ctx = pipeline.run(manifest).expect("Pipeline failed");
    let generator = Generator::from_context(ctx);
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
fn test_basic_cli_command_file() {
    let files = generate_files(
        r#"
        [cli]
        name = "myapp"
        version = "1.0.0"
        language = "typescript"
        description = "A simple CLI app"

        [commands.hello]
        description = "Say hello"
        "#,
    );

    let command = get_file(&files, "src/commands/hello.ts").expect("Command file not found");
    insta::assert_snapshot!("basic_cli_command", command);
}

#[test]
fn test_basic_cli_main_file() {
    let files = generate_files(
        r#"
        [cli]
        name = "myapp"
        version = "1.0.0"
        language = "typescript"
        description = "A simple CLI app"

        [commands.hello]
        description = "Say hello"
        "#,
    );

    let cli = get_file(&files, "src/cli.ts").expect("CLI file not found");
    insta::assert_snapshot!("basic_cli_main", cli);
}

#[test]
fn test_cli_with_args() {
    let files = generate_files(
        r#"
        [cli]
        name = "greeter"
        version = "1.0.0"
        language = "typescript"

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

    let command = get_file(&files, "src/commands/greet.ts").expect("Command file not found");

    // Verify structure (order may vary due to HashMap)
    assert!(command.contains("const args = {"));
    assert!(command.contains("name:"));
    assert!(command.contains("required: true")); // name is required
    assert!(command.contains("count:"));
    // count is optional so no required: true
    assert!(command.contains("export type GreetArgs = InferArgs<typeof args>"));
}

#[test]
fn test_cli_with_flags() {
    let files = generate_files(
        r#"
        [cli]
        name = "builder"
        version = "1.0.0"
        language = "typescript"

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

    let command = get_file(&files, "src/commands/build.ts").expect("Command file not found");

    // Verify structure (order may vary due to HashMap)
    assert!(command.contains("const options = {"));
    assert!(command.contains("release:"));
    assert!(command.contains("type: \"boolean\""));
    assert!(command.contains("short: \"r\""));
    assert!(command.contains("jobs:"));
    assert!(command.contains("type: \"number\""));
    assert!(command.contains("short: \"j\""));
    assert!(command.contains("default: 4"));
    assert!(command.contains("export type BuildOptions = InferOpts<typeof options>"));
}

#[test]
fn test_cli_with_choices() {
    let files = generate_files(
        r#"
        [cli]
        name = "deployer"
        version = "1.0.0"
        language = "typescript"

        [commands.deploy]
        description = "Deploy the application"

        [commands.deploy.args.environment]
        type = "string"
        description = "Target environment"
        choices = ["dev", "staging", "prod"]

        [commands.deploy.flags.strategy]
        type = "string"
        short = "s"
        description = "Deployment strategy"
        choices = ["rolling", "blue-green", "canary"]
        default = "rolling"
        "#,
    );

    let command = get_file(&files, "src/commands/deploy.ts").expect("Command file not found");

    // Verify choices are rendered with as const
    assert!(command.contains("environment:"));
    assert!(command.contains(r#"choices: ["dev", "staging", "prod"] as const"#));
    assert!(command.contains("strategy:"));
    assert!(command.contains(r#"choices: ["rolling", "blue-green", "canary"] as const"#));
    assert!(command.contains(r#"default: "rolling""#));
}

#[test]
fn test_cli_with_subcommands_structure() {
    let files = generate_files(
        r#"
        [cli]
        name = "dbcli"
        version = "1.0.0"
        language = "typescript"

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

    // Parent command for subcommands - check structure
    let db_command = get_file(&files, "src/commands/db.ts").expect("DB command file not found");
    assert!(db_command.contains("export const dbCommand = defineCommand"));
    assert!(db_command.contains("subcommands:"));
    assert!(db_command.contains("migrate: migrateCommand"));
    assert!(db_command.contains("seed: seedCommand"));

    // Verify subcommand files exist
    assert!(get_file(&files, "src/commands/db/migrate.ts").is_some());
    assert!(get_file(&files, "src/commands/db/seed.ts").is_some());
}

#[test]
fn test_cli_with_all_arg_types() {
    let files = generate_files(
        r#"
        [cli]
        name = "typetest"
        version = "1.0.0"
        language = "typescript"

        [commands.test]
        description = "Test all argument types"

        [commands.test.args.str_arg]
        type = "string"
        description = "A string argument"

        [commands.test.args.int_arg]
        type = "int"
        description = "An integer argument"

        [commands.test.args.float_arg]
        type = "float"
        description = "A float argument"

        [commands.test.args.bool_arg]
        type = "bool"
        description = "A boolean argument"

        [commands.test.args.path_arg]
        type = "path"
        description = "A path argument"
        "#,
    );

    let command = get_file(&files, "src/commands/test.ts").expect("Command file not found");

    // Verify all type mappings are correct (order may vary due to HashMap)
    assert!(command.contains("strArg:"));
    assert!(command.contains("type: \"string\""));
    assert!(command.contains("intArg:"));
    assert!(command.contains("floatArg:"));
    assert!(command.contains("type: \"number\"")); // both int and float map to number
    assert!(command.contains("boolArg:"));
    assert!(command.contains("type: \"boolean\""));
    assert!(command.contains("pathArg:"));
    assert!(command.contains("export type TestArgs = InferArgs<typeof args>"));
}

#[test]
fn test_handler_stub() {
    let files = generate_files(
        r#"
        [cli]
        name = "myapp"
        version = "1.0.0"
        language = "typescript"

        [commands.hello]
        description = "Say hello"

        [commands.hello.args.name]
        type = "string"
        description = "Name to greet"

        [commands.hello.flags.loud]
        type = "bool"
        description = "Shout the greeting"
        "#,
    );

    // Handler stubs are only created when generating to disk (not in preview)
    // since they should not overwrite existing implementations.
    // Test command file instead to verify handler import path is correct.
    let command = get_file(&files, "src/commands/hello.ts").expect("Command file not found");
    insta::assert_snapshot!("command_with_handler_import", command);
}

#[test]
fn test_index_file() {
    let files = generate_files(
        r#"
        [cli]
        name = "myapp"
        version = "1.0.0"
        language = "typescript"

        [commands.hello]
        description = "Say hello"
        "#,
    );

    let index = get_file(&files, "src/index.ts").expect("Index file not found");
    insta::assert_snapshot!("index_file", index);
}

#[test]
fn test_package_json() {
    let files = generate_files(
        r#"
        [cli]
        name = "myapp"
        version = "1.2.3"
        language = "typescript"
        description = "My awesome CLI"

        [commands.hello]
        description = "Say hello"
        "#,
    );

    let package = get_file(&files, "package.json").expect("package.json not found");
    insta::assert_snapshot!("package_json", package);
}
