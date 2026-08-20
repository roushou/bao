//! Bao's development tasks, runnable with `cargo xtask <task>`.
//!
//! Plain `std::process::Command`, no dependencies — so contributors need
//! nothing beyond `cargo` and the toolchain.

use std::process::{Command, ExitCode};

type Result = std::result::Result<(), ()>;

fn main() -> ExitCode {
    let task = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "help".to_string());
    let result = match task.as_str() {
        "check" => check(),
        "fmt" => run("cargo", &["fmt", "--all"]),
        "clippy" => run(
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "test" => run("cargo", &["test", "--workspace"]),
        "deny" => run("cargo", &["deny", "check"]),
        "changelog" => run("git", &["cliff", "-o", "CHANGELOG.md"]),
        "help" => {
            print_help();
            Ok(())
        }
        other => {
            eprintln!("unknown task `{other}`\n");
            print_help();
            Err(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

/// The full local gate — everything CI runs, minus the tools CI installs.
fn check() -> Result {
    run("cargo", &["fmt", "--all", "--check"])?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["test", "--workspace"])?;
    Ok(())
}

fn run(cmd: &str, args: &[&str]) -> Result {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| eprintln!("xtask: failed to spawn {cmd}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        eprintln!("xtask: `{cmd} {}` exited with {status}", args.join(" "));
        Err(())
    }
}

fn print_help() {
    println!("Bao development tasks:\n");
    println!("  cargo xtask check      fmt --check + clippy -D warnings + test");
    println!("  cargo xtask fmt        format the code");
    println!("  cargo xtask clippy     lint");
    println!("  cargo xtask test       run all tests");
    println!("  cargo xtask deny       license/advisory checks (needs cargo-deny)");
    println!("  cargo xtask changelog  generate CHANGELOG.md (needs git-cliff)");
}
