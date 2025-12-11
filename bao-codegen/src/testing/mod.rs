//! Test utilities for code generators.
//!
//! This module is only available when the `testing` feature is enabled
//! or during tests.

use std::{path::Path, process::Command};

use eyre::{Result, eyre};

/// Error from compile checking.
#[derive(Debug)]
pub struct CompileError {
    pub message: String,
    pub output: String,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n\nOutput:\n{}", self.message, self.output)
    }
}

impl std::error::Error for CompileError {}

/// Trait for verifying generated code compiles/type-checks.
pub trait CompileChecker {
    /// Check that the code in the given directory compiles.
    fn check(&self, dir: &Path) -> Result<(), CompileError>;
}

/// Rust compile checker using `cargo check`.
pub struct RustChecker;

impl CompileChecker for RustChecker {
    fn check(&self, dir: &Path) -> Result<(), CompileError> {
        let output = Command::new("cargo")
            .arg("check")
            .current_dir(dir)
            .output()
            .map_err(|e| CompileError {
                message: format!("Failed to run cargo check: {}", e),
                output: String::new(),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            Err(CompileError {
                message: "cargo check failed".to_string(),
                output: format!("stderr:\n{}\n\nstdout:\n{}", stderr, stdout),
            })
        }
    }
}

/// TypeScript compile checker using `tsc --noEmit`.
pub struct TypeScriptChecker;

impl CompileChecker for TypeScriptChecker {
    fn check(&self, dir: &Path) -> Result<(), CompileError> {
        let output = Command::new("npx")
            .args(["tsc", "--noEmit"])
            .current_dir(dir)
            .output()
            .map_err(|e| CompileError {
                message: format!("Failed to run tsc: {}", e),
                output: String::new(),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            Err(CompileError {
                message: "tsc --noEmit failed".to_string(),
                output: format!("stderr:\n{}\n\nstdout:\n{}", stderr, stdout),
            })
        }
    }
}

/// Go compile checker using `go build`.
pub struct GoChecker;

impl CompileChecker for GoChecker {
    fn check(&self, dir: &Path) -> Result<(), CompileError> {
        let output = Command::new("go")
            .args(["build", "./..."])
            .current_dir(dir)
            .output()
            .map_err(|e| CompileError {
                message: format!("Failed to run go build: {}", e),
                output: String::new(),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            Err(CompileError {
                message: "go build failed".to_string(),
                output: format!("stderr:\n{}\n\nstdout:\n{}", stderr, stdout),
            })
        }
    }
}

/// Assert that two strings are equal, with a nice diff on failure.
pub fn assert_content_eq(expected: &str, actual: &str) {
    if expected != actual {
        // Simple line-by-line diff
        let expected_lines: Vec<&str> = expected.lines().collect();
        let actual_lines: Vec<&str> = actual.lines().collect();

        let mut diff = String::new();
        let max_lines = expected_lines.len().max(actual_lines.len());

        for i in 0..max_lines {
            let exp = expected_lines.get(i).copied().unwrap_or("<missing>");
            let act = actual_lines.get(i).copied().unwrap_or("<missing>");

            if exp != act {
                diff.push_str(&format!("Line {}:\n", i + 1));
                diff.push_str(&format!("  expected: {}\n", exp));
                diff.push_str(&format!("  actual:   {}\n", act));
            }
        }

        panic!("Content mismatch:\n{}", diff);
    }
}

/// Generate code into a temporary directory and return the path.
///
/// The directory will be cleaned up when the returned `TempDir` is dropped.
pub fn generate_to_temp<F>(generate: F) -> Result<tempfile::TempDir>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let temp_dir = tempfile::TempDir::new()?;
    generate(temp_dir.path())?;
    Ok(temp_dir)
}

/// Helper to run a generator and check that it compiles.
pub fn assert_generates_valid_code<C>(
    generate: impl FnOnce(&Path) -> Result<()>,
    checker: &C,
) -> Result<()>
where
    C: CompileChecker,
{
    let temp_dir = generate_to_temp(generate)?;

    checker.check(temp_dir.path()).map_err(|e| {
        // Print the generated files for debugging
        eprintln!("Generated files in {}:", temp_dir.path().display());
        if let Ok(entries) = std::fs::read_dir(temp_dir.path()) {
            for entry in entries.flatten() {
                eprintln!("  {}", entry.path().display());
            }
        }
        eyre!("Compile check failed: {}", e)
    })?;

    Ok(())
}
